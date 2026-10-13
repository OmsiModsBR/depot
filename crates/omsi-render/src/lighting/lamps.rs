//! Street lamps and headlights: the light grid, the lamps' shadow maps, the glow they throw
//! on the night sky and the light and glare they give the camera.

use super::model::{LightMode, PointLight, LAMP_E};
use super::smoothstep;
use glam::{DVec3, Mat4, Vec3};

pub(crate) const LIGHT_CELL: f32 = 25.0;

/// Enhanced: how many street lamps cast shadows (their maps are tiles of a quarter of the
/// shadow size under the far map), and how far from the camera a lamp's reach may end.
pub(crate) const LAMP_SHADOWS: usize = 4;

pub(crate) const LAMP_SHADOW_REACH: f32 = 45.0;

/// The far map's height over its width: the lamps' tiles take the quarter under it.
pub(crate) const FAR_MAP_ASPECT: f32 = 1.25;

/// A lamp's shadow map looks straight down from its head over this field of view (a
/// street lamp's light leaves it downwards, see `PointLight::housed`).
pub(crate) const LAMP_SHADOW_FOV: f32 = 150.0;

/// A street lamp that casts shadows this frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LampShadow {
    /// Its index in the light buffer, where it is (render-origin relative) and its reach.
    pub(crate) index: u32,
    pub(crate) position: Vec3,
    pub(crate) range: f32,
}

impl LampShadow {
    /// The shadow map's view and projection: straight down, depth 0..1 over its reach.
    pub(crate) fn view_proj(&self) -> Mat4 {
        let proj =
            Mat4::perspective_rh(LAMP_SHADOW_FOV.to_radians(), 1.0, 0.1, self.range.max(1.0));
        let view = Mat4::look_to_rh(self.position, -Vec3::Z, Vec3::Y);
        proj * view
    }
}

pub(crate) const LIGHT_GRID_SIDE: usize = 64;

/// (32: a depot or a bus interior with many lamps lost the farthest past 16 in a cell)
pub(crate) const LIGHT_CELL_CAP: usize = 32;

/// How much the lamps round the camera light the night sky over it, relative to a city's
/// (Spandau's middle is 1): a sky's glow is the lamps' upward and reflected light scattered
/// back down by the air, each source's part falling off with the distance to it as
/// d^-2.5 (Walker's law, "The effects of urban lighting on the brightness of the night
/// sky", 1977; the near field evened out over the first few hundred metres). A village's
/// handful of lamps leaves the stars; a city's thousands turn the sky orange-grey.
pub(crate) const SKY_GLOW_REF: f32 = 830.0;

pub(crate) fn lamp_sky_glow(lights: &[PointLight], ro: DVec3, cam_rel: Vec3) -> f32 {
    let mut sum = 0.0f32;
    for l in lights {
        if !l.housed || l.intensity <= 0.0 {
            continue;
        }
        let d = ((l.position - ro).as_vec3() - cam_rel).truncate().length();
        let lum = (l.color[0] + l.color[1] + l.color[2]) / 3.0;
        sum += l.intensity * lum * l.core * l.core / (1.0 + (d / 300.0).powf(2.5));
    }
    sum / SKY_GLOW_REF
}

/// The light of the lamps and the headlights on the ground the camera looks at (irradiance,
/// 1 = 10 000 lux): the log-average over the points where a grid of rays through the
/// picture meets the ground (`ground`: the street's height relative to the render origin,
/// the player's vehicle's), each lit as the enhanced pass lights it (a street lamp down
/// and out, a headlamp ahead and under its cut-off). What the eye adapts to by night,
/// rather than a fixed guess.
///
/// The rays are the picture's own: a dozen points 5 to 30 m ahead on a ground taken 1.6 m
/// under the camera missed the street the eye actually sees from any camera higher than a
/// driver's (it lay in the air there, out of every lamp's reach) and the lamp-lit pools to
/// the sides of a narrow view - the meter read a moonless night while the picture was full
/// of lit streets, and the eye's adaptation to that dark lifted every lamp's light to
/// white (the modded maps' lamps, spaced and placed differently from the stock ones, most).
/// `(f, r, u)`: the camera's forward, right and up.
pub(crate) fn view_lamp_light(
    lights: &[PointLight],
    ro: DVec3,
    cam_rel: Vec3,
    (f, r, u): (Vec3, Vec3, Vec3),
    fov_deg: f32,
    aspect: f32,
    ground: Option<f32>,
) -> f32 {
    let ground = ground
        .filter(|g| g.is_finite() && *g < cam_rel.z - 0.3)
        .unwrap_or(cam_rel.z - 1.6);
    let ty = (fov_deg.to_radians() * 0.5).tan();
    let tx = ty * aspect.max(0.1);
    let mut points = Vec::with_capacity(35);
    for j in 0..5 {
        for i in 0..7 {
            let x = -0.9 + 1.8 * i as f32 / 6.0;
            let y = -0.95 + 1.9 * j as f32 / 4.0;
            let dir = (f + r * (x * tx) + u * (y * ty)).normalize();
            if dir.z >= -0.01 {
                continue;
            }
            // (as far as a lamp's light still shapes what the eye takes to: a pool 300 m
            // off is a point of light, not the scene)
            let t = (ground - cam_rel.z) / dir.z;
            if t > 0.0 && t < 300.0 {
                points.push(cam_rel + dir * t);
            }
        }
    }
    if points.is_empty() {
        // (looking up: the ground round the camera, which the eye has just seen)
        let f = Vec3::new(f.x, f.y, 0.0).normalize_or(Vec3::Y);
        for d in [5.0f32, 10.0, 18.0, 30.0] {
            for a in [-0.5f32, 0.0, 0.5] {
                let (s, c) = a.sin_cos();
                let dir = Vec3::new(f.x * c - f.y * s, f.x * s + f.y * c, 0.0);
                points.push(Vec3::new(cam_rel.x, cam_rel.y, ground) + dir * d);
            }
        }
    }
    let mut per_point = vec![0.0f32; points.len()];
    for l in lights {
        if l.intensity <= 0.0 || l.mode == LightMode::Vanilla {
            continue;
        }
        let p = (l.position - ro).as_vec3();
        let lum = 0.2126 * l.color[0] + 0.7152 * l.color[1] + 0.0722 * l.color[2];
        let core = if l.core > 0.0 {
            l.core
        } else {
            l.radius * 0.125
        };
        for (pi, x) in points.iter().enumerate() {
            let to = *x - p;
            let d2 = to.length_squared();
            if d2 >= l.radius * l.radius {
                continue;
            }
            let q = d2 / (l.radius * l.radius);
            let window = (1.0 - q * q) * (1.0 - q * q);
            let t = to / d2.sqrt().max(1e-3);
            let e = if l.beam != 0.0 {
                // a headlamp lights the road it stands on (some 0.8 m under it), by its
                // profile (lamp_air.wgsl `headlamp`)
                let g = Vec3::new(x.x, x.y, p.z - 0.8) - p;
                let gd2 = g.length_squared();
                headlamp_profile(g / gd2.sqrt().max(1e-3), l.direction, l.beam > 0.0) / gd2.max(0.3)
                    * window
            } else {
                let mut e = core * core / (d2 * d2 + core.powi(4)).sqrt() * window;
                if l.direction.length_squared() > 1e-6 {
                    e *= smoothstep(l.cone[1], l.cone[0], t.dot(l.direction.normalize()));
                } else if l.housed {
                    e *= 0.05 + 0.95 * smoothstep(-0.1, 0.3, -t.z);
                }
                e
            };
            per_point[pi] += LAMP_E * l.intensity * lum * e;
        }
    }
    // the log-average, as the eye adapts to a field of view (Reinhard's and Krawczyk's
    // adapting luminance): a headlight's pool in front of the camera does not decide it
    // alone, as the plain mean let it (facing a bus's lights, all else went black). The
    // floor is a moonless night's light, which every point has.
    let floor = 2e-6f32;
    let log_sum: f32 = per_point.iter().map(|v| (v + floor).ln()).sum();
    (log_sum / per_point.len() as f32).exp() - floor + glare_veil(lights, ro, cam_rel, f)
}

/// The lamps' and headlights' glare in view as the illuminance whose mid grey has the same
/// luminance (1 = 10 000 lux): the light of every lamp the eye sees, scattered in the eye
/// over the field of view, is a veil the eye adapts to as to any other light - the
/// Stiles-Holladay disability glare, L = 10 x the illuminance at the eye (lux) / the angle
/// off the line of sight squared (degrees), summed over the sources (CIE 146). Metered on
/// the ground between the lamps alone, a village full of street lamps in view read as a
/// moonless night, and the eye's adaptation to that dark lifted every lamp's light to white.
pub(crate) fn glare_veil(lights: &[PointLight], ro: DVec3, cam_rel: Vec3, forward: Vec3) -> f32 {
    let f = forward.normalize_or(Vec3::Y);
    let mut veil = 0.0f32; // cd/m²
    for l in lights {
        if l.intensity <= 0.0 || l.mode == LightMode::Vanilla {
            continue;
        }
        let to_lamp = (l.position - ro).as_vec3() - cam_rel;
        let d2 = to_lamp.length_squared();
        // (a lamp out past a few hundred metres is a point among the stars to the eye; one
        // within a metre is the bus's own)
        if !(1.0..300.0 * 300.0).contains(&d2) {
            continue;
        }
        let dist = d2.sqrt();
        let theta = f.dot(to_lamp / dist).clamp(-1.0, 1.0).acos().to_degrees();
        if theta >= 90.0 {
            continue;
        }
        let lum = 0.2126 * l.color[0] + 0.7152 * l.color[1] + 0.0722 * l.color[2];
        let core = if l.core > 0.0 {
            l.core
        } else {
            l.radius * 0.125
        };
        // towards the eye: from the lamp
        let t = -to_lamp / dist;
        let k = if l.beam != 0.0 {
            headlamp_profile(t, l.direction, l.beam > 0.0) / (core * core).max(1e-3)
        } else if l.direction.length_squared() > 1e-6 {
            smoothstep(l.cone[1], l.cone[0], t.dot(l.direction.normalize()))
        } else if l.housed {
            0.05 + 0.95 * smoothstep(-0.1, 0.3, -t.z)
        } else {
            1.0
        };
        // the lamp's illuminance at the eye (lux): its core's level, falling off with the
        // square of the distance as on the street
        let e_eye = LAMP_E * 1e4 * l.intensity * lum * k * core * core / d2;
        veil += 10.0 * e_eye / theta.max(1.5).powi(2);
    }
    // (the luminance of a mid-grey surface, 18 %, under this illuminance)
    veil * std::f32::consts::PI / 0.18 / 1e4
}

/// A headlamp's intensity towards `t` (unit, from the lamp): lamp_air.wgsl `headlamp`.
pub(crate) fn headlamp_profile(t: Vec3, dir: Vec3, low: bool) -> f32 {
    let fwd = (dir.truncate() + glam::Vec2::new(1e-6, 0.0)).normalize();
    let ahead = t.truncate().dot(fwd);
    if ahead <= 0.0 {
        return 0.0;
    }
    let across = (t.x * fwd.y - t.y * fwd.x).abs() / ahead;
    let wide = 0.12 * smoothstep(1.0, 0.45, across) + 0.88 * (-across * across / 0.06).exp();
    let drop = -t.z / t.truncate().length().max(1e-3);
    let mut up = (0.06 / drop.abs().max(1e-4)).powf(3.4).min(1.0);
    if low {
        up *= smoothstep(-0.012, 0.025, drop);
        return wide * up;
    }
    let hot = (-across * across / 0.012 - drop * drop / 0.0004).exp();
    wide * up + 6.0 * hot
}
