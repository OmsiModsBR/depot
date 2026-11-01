//! Precipitation: rain streaks / snow flakes falling around the camera, drawn through the
//! corona sprite pipeline (cone value -2 marks a streak), and the picture the film on the
//! glass wears in a snow weather.

use glam::{DVec3, Vec3};
use omsi_render::{Corona, Scene};

pub struct Rain {
    particles: Vec<Vec3>,
    kind: i32,
    rate: f32,
    /// Snow is drawn here, as flakes of OMSI 2's snowflake picture (the DirectX9 renderer's
    /// own option); otherwise the renderer draws its snowfall itself.
    flakes: bool,
    rng: u64,
    wiper: WiperWatch,
}

/// Where the wiper of the vehicle the player drives is, 0..1 across its sweep, read from the
/// vehicle's script: the variable its scripts publish for the wiper's position (the first
/// whose name says wiper and a position or an angle). Its range is not known beforehand, so
/// it is taken from the values seen so far; until the wiper has moved there is no position.
#[derive(Default)]
struct WiperWatch {
    /// The vehicle type the slot was looked up for (the address of its `Arc`; only compared:
    /// two vehicles of one type share their variables' layout), and the
    /// slot, `None` when it has no such variable.
    of: usize,
    slot: Option<usize>,
    range: Option<(f32, f32)>,
}

/// The smallest swing, in the script's own units, taken for a wiper's stroke.
const LEAST_SWING: f32 = 0.05;

impl WiperWatch {
    fn is_position(name: &str) -> bool {
        let wiper = name.contains("wiper") || name.contains("wischer");
        let position = ["pos", "angle", "winkel", "deg"]
            .iter()
            .any(|p| name.contains(p));
        let other = [
            "speed", "mode", "run", "switch", "lever", "sound", "delay", "time",
        ]
        .iter()
        .any(|o| name.contains(o))
            || name.ends_with("_on");
        wiper && position && !other
    }

    fn position(&mut self, v: &omsi_sim::VehicleInstance) -> Option<f32> {
        // (keyed by the vehicle's type, not by where the vehicle sits: the player's vehicle is
        // stored inline, so the next bus the player takes lies at the same address)
        self.retarget(std::sync::Arc::as_ptr(&v.ty) as usize, || {
            v.var_slot_where(Self::is_position)
        });
        let value = v.var_by_slot(self.slot?).filter(|x| x.is_finite())?;
        self.seen(value)
    }

    /// Forgets the slot and the range when `key` is not the vehicle type they were learnt for.
    fn retarget(&mut self, key: usize, slot: impl FnOnce() -> Option<usize>) {
        if self.of != key {
            *self = WiperWatch {
                of: key,
                slot: slot(),
                range: None,
            };
        }
    }

    /// `value` as a share of the range seen so far (none until the range is a stroke).
    fn seen(&mut self, value: f32) -> Option<f32> {
        let (lo, hi) = self.range.get_or_insert((value, value));
        *lo = lo.min(value);
        *hi = hi.max(value);
        let swing = *hi - *lo;
        (swing >= LEAST_SWING).then(|| ((value - *lo) / swing).clamp(0.0, 1.0))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_position_variable_is_told_from_the_wipers_other_variables() {
        for yes in [
            "wiper_pos",
            "wiperangle",
            "wischer_winkel",
            "wiper_left_pos",
        ] {
            assert!(WiperWatch::is_position(yes), "{yes}");
        }
        for no in [
            "wiper_running",
            "wiper_mode",
            "wiper_speed_pos",
            "wiper_switch",
            "wiper_on",
            "engine_pos",
            "washer_angle",
        ] {
            assert!(!WiperWatch::is_position(no), "{no}");
        }
    }

    #[test]
    fn the_range_is_learnt_from_the_strokes_seen() {
        let mut w = WiperWatch::default();
        // (at rest: no stroke seen yet, so no position)
        assert_eq!(w.seen(0.0), None);
        assert_eq!(w.seen(0.0), None);
        // the first stroke widens the range as it goes
        assert_eq!(w.seen(30.0), Some(1.0));
        assert_eq!(w.seen(60.0), Some(1.0));
        assert_eq!(w.seen(30.0), Some(0.5));
        assert_eq!(w.seen(0.0), Some(0.0));
        assert_eq!(w.seen(75.0), Some(1.0));
        assert_eq!(w.seen(15.0), Some(0.2));
    }

    #[test]
    fn another_vehicle_starts_from_nothing() {
        let mut w = WiperWatch::default();
        w.retarget(1, || Some(7));
        w.seen(0.0);
        assert_eq!(w.seen(40.0), Some(1.0));
        // the same vehicle again: kept
        w.retarget(1, || panic!("looked up again"));
        assert_eq!((w.slot, w.range), (Some(7), Some((0.0, 40.0))));
        // another: its own slot (here none), no range
        w.retarget(2, || None);
        assert_eq!((w.slot, w.range), (None, None));
    }

    #[test]
    fn a_flicker_is_no_stroke() {
        let mut w = WiperWatch::default();
        assert_eq!(w.seen(1.0), None);
        assert_eq!(w.seen(1.01), None);
    }
}

/// The boxes a vehicle keeps the weather out of, as `Rain::tick` takes them: its own
/// `[boundingbox]` and each coupled part's (an articulated bus's rear section is a part of
/// its own, with its own box: without it, it rained in the rear saloon, #777).
pub fn vehicle_boxes(v: &omsi_sim::VehicleInstance) -> Vec<(DVec3, f64, [f32; 6])> {
    let front = v.ty.def.bounding_box.map(|bb| (v.position, v.heading, bb));
    let parts = v.trailers.iter().filter_map(|t| {
        // a part coupled the other way round stands turned about its own origin
        let heading = if t.reversed {
            t.heading + 180.0
        } else {
            t.heading
        };
        t.ty.def.bounding_box.map(|bb| (t.position, heading, bb))
    });
    front.into_iter().chain(parts).collect()
}

impl Rain {
    pub fn new() -> Rain {
        Rain {
            particles: Vec::new(),
            kind: 0,
            rate: 0.0,
            flakes: false,
            rng: 0xABCDEF12345,
            wiper: WiperWatch::default(),
        }
    }

    /// Where the wiper of `vehicle` is, 0..1 across its sweep (see [`WiperWatch`]).
    pub fn wiper_position(&mut self, vehicle: &omsi_sim::VehicleInstance) -> Option<f32> {
        self.wiper.position(vehicle)
    }

    fn rand(&mut self) -> f32 {
        omsi_geometry::util::unit_f32(&mut self.rng)
    }

    /// `kind`: 0 none, 1 rain, 2 snow; `rate` 0..1. `flakes`: snow is drawn here, as sprites
    /// of the snowflake picture ("DirectX9 (D3D9Ex)" with its Snow option on).
    pub fn set(&mut self, kind: i32, rate: f32, flakes: bool) {
        self.kind = kind;
        self.rate = rate.clamp(0.0, 1.0);
        self.flakes = flakes;
        // (snow is otherwise drawn by the renderer itself, from the weather: render snow.wgsl,
        // `Lighting::snowfall`)
        let n = if kind == 0 || (kind == 2 && !flakes) {
            0
        } else {
            (400.0 + 2600.0 * self.rate) as usize
        };
        while self.particles.len() < n {
            let p = Vec3::new(
                self.rand() * 40.0 - 20.0,
                self.rand() * 40.0 - 20.0,
                self.rand() * 20.0,
            );
            self.particles.push(p);
        }
        self.particles.truncate(n);
    }

    /// Move the particles (camera-relative box) and push them as sprites.
    /// `inside`: the buses near the camera as (origin, heading in degrees, `[boundingbox]`),
    /// the player's first; no drop or flake is drawn within any, so that it does not rain
    /// in a cab (only the player's own had been left dry: riding in another player's bus or
    /// a timetable bus it snowed in the saloon).
    pub fn tick(
        &mut self,
        dt: f32,
        camera: DVec3,
        wind: Vec3,
        scene: &mut Scene,
        inside: &[(DVec3, f64, [f32; 6])],
    ) {
        if self.particles.is_empty() {
            return;
        }
        let buses: Vec<(DVec3, f64, [f32; 6])> = inside
            .iter()
            .filter(|b| (b.0 - camera).length() < 40.0)
            .map(|&(o, h, bb)| (o, h.to_radians(), bb))
            .collect();
        let in_one = |p: DVec3, (o, h, bb): (DVec3, f64, [f32; 6])| -> bool {
            let d = p - o;
            let (sh, ch) = (h.sin(), h.cos());
            let x = d.x * ch - d.y * sh;
            let y = d.x * sh + d.y * ch;
            let z = d.z;
            // a flat 0.3 m margin left flakes rendering on the inside of the glass near a
            // window sill (the ledge below a side window sits close to the body's outer
            // envelope, closer than the margin covered): widened so the whole cabin,
            // ledges included, sits inside the excluded box.
            (x - bb[3] as f64).abs() < bb[0] as f64 * 0.5 + 0.6
                && (y - bb[4] as f64).abs() < bb[1] as f64 * 0.5 + 0.6
                && (z - bb[5] as f64).abs() < bb[2] as f64 * 0.5 + 0.6
        };
        let in_bus = |p: DVec3| buses.iter().any(|b| in_one(p, *b));
        let fall = if self.kind == 2 { 1.5 } else { 9.0 };
        for p in self.particles.iter_mut() {
            p.z -= fall * dt;
            p.x += wind.x * dt;
            p.y += wind.y * dt;
            if p.z < -2.0 {
                p.z += 22.0;
            }
            if p.x < -20.0 {
                p.x += 40.0;
            }
            if p.x > 20.0 {
                p.x -= 40.0;
            }
            if p.y < -20.0 {
                p.y += 40.0;
            }
            if p.y > 20.0 {
                p.y -= 40.0;
            }
        }
        let (size, color, brightness) = if self.kind == 2 {
            (0.06, [1.0, 1.0, 1.0], 0.9)
        } else {
            (0.05, [0.75, 0.8, 0.9], 0.35)
        };
        // (a snowflake is a sprite of the game's snowflake picture, a cone of -3; a raindrop
        // a streak, -2)
        let (cone_cos, texture) = if self.kind == 2 {
            (-3.0, crate::lights::snowflake_texture_id())
        } else {
            (-2.0, 0)
        };
        let mut excluded = 0usize;
        for p in &self.particles {
            let w = camera + p.as_dvec3();
            if in_bus(w) {
                excluded += 1;
                continue;
            }
            scene.coronas.push(Corona {
                position: w,
                size,
                color,
                brightness,
                direction: Vec3::ZERO,
                cone_cos,
                texture,
                ..Default::default()
            });
        }
        if omsi_cfg::flags::DEBUG_RAIN.is_set() {
            log::info!(
                "rain: {} particles, {excluded} inside the buses (boxes {:?})",
                self.particles.len(),
                buses
                    .iter()
                    .map(|b| (b.0, b.1.to_degrees(), b.2))
                    .collect::<Vec<_>>()
            );
        }
    }
}

/// The picture the windscreen and window layers wear while it snows.
///
/// Every bus carries the same rain film: a mesh in front of the glass textured with
/// `regen.tga` (running drops) and faded in by `[alphascale] Rain_Window_*_Wetness`, which
/// `rain.osc` fills from `PrecipRate` - and `rain.osc` never asks what is falling, so in a
/// snowstorm the original shows raindrops on every pane, in the cab and along the saloon.
/// OMSI's own way out is the seasonal texture folder (`texture\WinterSnow\`), which no stock
/// vehicle fills in, so openOMSI builds the winter picture itself: crystals settled on
/// the glass, thickest along the rim where a pane collects them, made from the game's own
/// `Texture\snowflake.tga` where it is there and from soft specks of its own where it is not.
pub fn snow_on_glass(root: &std::path::Path) -> omsi_texture::Image {
    // the same grain as the rain film it stands in for (`regen.tga`, 1024 x 1024 with
    // drops a dozen pixels across and a mean alpha of 7 %)
    const SIDE: usize = 512;
    // white where it is clear as well, so that the smaller mip levels stay white specks
    // and do not grey towards the black of transparent texels
    let mut rgba = [255u8, 255, 255, 0].repeat(SIDE * SIDE);
    let flake =
        omsi_texture::decode_file(&omsi_cfg::resolve_path(root, "Texture\\snowflake.tga")).ok();
    let mut rng = 0x9E37_79B9_7F4A_7C15u64;
    let mut rand = move || omsi_geometry::util::unit_f32(&mut rng);
    // a pane holds more snow at its rim than in the middle, where the airstream and the
    // wipers take it away
    let edge = |x: f32, y: f32| {
        let d = (x - 0.5).abs().max((y - 0.5).abs()) * 2.0;
        0.35 + 0.65 * d * d
    };
    for _ in 0..1800 {
        let (cx, cy) = (rand() * SIDE as f32, rand() * SIDE as f32);
        if rand() > edge(cx / SIDE as f32, cy / SIDE as f32) {
            continue;
        }
        let r = 1.5 + rand() * 3.5;
        let strength = 0.35 + 0.65 * rand();
        let lo = |v: f32| (v - r).max(0.0) as usize;
        let hi = |v: f32| ((v + r) as usize).min(SIDE - 1);
        for y in lo(cy)..=hi(cy) {
            for x in lo(cx)..=hi(cx) {
                let (dx, dy) = (x as f32 + 0.5 - cx, y as f32 + 0.5 - cy);
                let d = (dx * dx + dy * dy).sqrt() / r;
                if d > 1.0 {
                    continue;
                }
                // the flake's own picture where the game has one, else a soft round speck
                let a = match &flake {
                    Some(f) => {
                        let u = ((dx / r * 0.5 + 0.5) * (f.width - 1) as f32) as usize;
                        let v = ((dy / r * 0.5 + 0.5) * (f.height - 1) as f32) as usize;
                        let p = (v * f.width as usize + u) * 4;
                        // the stock flake is white on black; its alpha channel, or its
                        // brightness where it has none
                        (f.rgba[p + 3] as f32 / 255.0).max(f.rgba[p] as f32 / 255.0)
                    }
                    None => (1.0 - d * d).powf(1.5),
                };
                let a = (a * strength * 255.0) as u32;
                let p = (y * SIDE + x) * 4;
                rgba[p] = 255;
                rgba[p + 1] = 255;
                rgba[p + 2] = 255;
                rgba[p + 3] = rgba[p + 3].max(a.min(255) as u8);
            }
        }
    }
    omsi_texture::Image {
        width: SIDE as u32,
        height: SIDE as u32,
        rgba,
        has_alpha: true,
    }
}

/// The snow-on-glass picture (`snow_on_glass`) on the GPU, with its mip chain: one level
/// alone, its specks of a pixel or two were point-sampled across a whole windscreen and
/// glittered with every move of the head (#946).
pub fn add_snow_on_glass(
    renderer: &omsi_render::Renderer,
    scene: &mut omsi_render::Scene,
    root: &std::path::Path,
) -> omsi_render::TextureId {
    renderer.add_texture(scene, &snow_on_glass(root), true)
}
