//! The lights' per-frame state and textures: the near map lights cache, the smoke and cone
//! weather settings, the corona textures and their upload to the renderer, and the gathering
//! of the frame's lights (`collect`). The rules of the lights are in `domain/lights.rs`.

use super::super::domain::lights::*;
use crate::world::scene::{LightSwitch, World};
use glam::{DVec3, Vec3};
use omsi_render::{Corona, PointLight, Scene};
use omsi_sim::{Daylight, VehicleInstance};

/// Map lights and scenery coronas are only drawn this close to the camera.
const MAP_LIGHT_RANGE: f64 = 600.0;

const CORONA_RANGE: f64 = 1500.0;

/// How far the camera may move before the nearby static lights are gathered again.
const NEAR_MARGIN: f64 = 100.0;

/// The map's lights and coronas near the camera, switched for the lamp state. Going
/// through every corona of a city map (traffic lights, street lamps, signs) each frame
/// cost a millisecond; the near ones only change when the camera has moved on, the lamps
/// switch or tiles have loaded or gone (the streamer rebuilds the world's lists then).
#[derive(Default)]
struct NearLights {
    world: usize,
    generation: u64,
    lamps_on: bool,
    centre: DVec3,
    counts: (usize, usize),
    lights: Vec<PointLight>,
    coronas: Vec<Corona>,
}

static NEAR_LIGHTS: std::sync::Mutex<Option<NearLights>> = std::sync::Mutex::new(None);

/// OMSI's "Smoke Systems" options as the scene is gathered: bit 0 the particles are drawn at
/// all, bit 1 only the player's vehicle draws its own.
static SMOKE_OPTIONS: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0b01);

/// Sets the smoke options (see [`omsi_sim::particles::set_max_per_emitter`] for the cap, which
/// `max` sets here too).
pub fn set_smoke_options(active: bool, user_only: bool, max: u32) {
    SMOKE_OPTIONS.store(
        u8::from(active) | u8::from(user_only) << 1,
        std::sync::atomic::Ordering::Relaxed,
    );
    omsi_sim::particles::set_max_per_emitter(max as usize);
}

/// Whether the particles are drawn at all (the game's tyre spray is held back with them).
pub fn smoke_active() -> bool {
    SMOKE_OPTIONS.load(std::sync::atomic::Ordering::Relaxed) & 1 != 0
}

/// Fill the scene's lights for this frame. `players`: how many of the first `vehicles` are the
/// player's (0 or 1; the smoke option "only the user vehicle" draws just their particles).
pub fn collect(
    world: &World,
    scene: &mut Scene,
    daylight: &Daylight,
    camera_pos: DVec3,
    vehicles: &[&VehicleInstance],
    players: usize,
) {
    let options = SMOKE_OPTIONS.load(std::sync::atomic::Ordering::Relaxed);
    let (smoke_on, user_only) = (options & 1 != 0, options & 2 != 0);
    scene.lights.clear();
    scene.coronas.clear();
    scene.smoke.clear();
    omsi_sim::particles::set_eye(camera_pos);
    let night = daylight.night;
    {
        let mut guard = NEAR_LIGHTS.lock().unwrap_or_else(|e| e.into_inner());
        let near = guard.get_or_insert_with(NearLights::default);
        let static_lights = world.static_lights.lock();
        let static_coronas = world.static_coronas.lock();
        let world_id = world as *const World as usize;
        let generation = world
            .tiles_generation
            .load(std::sync::atomic::Ordering::Relaxed);
        let stale = near.world != world_id
            || near.generation != generation
            || near.lamps_on != daylight.lamps_on
            || near.counts != (static_lights.len(), static_coronas.len())
            || (near.centre - camera_pos).length() > NEAR_MARGIN;
        if stale {
            *near = NearLights {
                world: world_id,
                generation,
                lamps_on: daylight.lamps_on,
                centre: camera_pos,
                counts: (static_lights.len(), static_coronas.len()),
                lights: Vec::new(),
                coronas: Vec::new(),
            };
            if daylight.lamps_on {
                near.lights.extend(static_lights.iter().filter(|l| {
                    (l.position - camera_pos).length() < MAP_LIGHT_RANGE + NEAR_MARGIN
                }));
            }
            for c in static_coronas.iter() {
                let on = match &c.switch {
                    LightSwitch::Constant(x) => *x,
                    LightSwitch::Night => daylight.lamps_on as i32 as f32,
                    LightSwitch::Variable(_) => daylight.lamps_on as i32 as f32,
                };
                if on <= 0.0
                    || (c.corona.position - camera_pos).length() > CORONA_RANGE + NEAR_MARGIN
                {
                    continue;
                }
                let mut corona = c.corona;
                corona.brightness *= on.min(1.0);
                near.coronas.push(corona);
            }
        }
        scene.lights.extend(
            near.lights
                .iter()
                .filter(|l| (l.position - camera_pos).length() < MAP_LIGHT_RANGE),
        );
        scene.coronas.extend(
            near.coronas
                .iter()
                .filter(|c| (c.position - camera_pos).length() <= CORONA_RANGE),
        );
    }
    // traffic lamps glow with what they show, by day as well
    for lamp in world.light_objects.lock().iter() {
        if (lamp.pos - camera_pos).length() > 1500.0 {
            continue;
        }
        for ((c, _), lit) in lamp.coronas.iter().zip(&lamp.lit) {
            if *lit <= 0.0 {
                continue;
            }
            let mut corona = *c;
            corona.brightness *= lit.min(1.0);
            scene.coronas.push(corona);
        }
    }
    for list in world.particle_objects.lock().values() {
        for po in list {
            if smoke_on && !user_only && (po.pos - camera_pos).length() < 1500.0 {
                particle_sprites(&po.set, &mut scene.smoke, &mut scene.coronas);
            }
        }
    }
    if omsi_cfg::flags::DEBUG_PARTICLES.is_set() {
        if let Some(p) = scene.smoke.first() {
            log::info!("smoke: {} particles from objects, first at ({:.1}, {:.1}, {:.1}) size {:.2} alpha {:.2}", scene.smoke.len(), p.position.x, p.position.y, p.position.z, p.size, p.alpha);
        }
    }
    for (i, v) in vehicles.iter().enumerate() {
        vehicle_lights(v, &mut scene.coronas, &mut scene.lights, night);
        if !smoke_on || (user_only && i >= players) {
            continue;
        }
        particle_sprites(&v.particles, &mut scene.smoke, &mut scene.coronas);
        for t in &v.trailers {
            particle_sprites(&t.particles, &mut scene.smoke, &mut scene.coronas);
        }
    }
    // the lamps' cones in fog: drawn only while the visibility is
    // under 2 km, the fan's radius 2 m times 3 sqrt(100 / visibility) times the glow's
    // strength ((1 - ambient)^2 + 0.8) 0.6 brightness and the light's size; its colour the
    // light's times 0.3 (the viewing angle and the distance take their share in the shader,
    // which is told the visibility in `beam_width`)
    let (vis, night) = cone_weather();
    // The halo round a light in fog is half the cone's radius across either way, at 0.2.
    scene.coronas.retain_mut(|c| {
        if !c.beam && !c.halo {
            return true;
        }
        if vis >= 2000.0 {
            return false;
        }
        let glow = (night * night + 0.8) * 0.6 * c.brightness;
        let reach = 3.0 * (100.0 / vis.max(1.0)).sqrt() * glow * c.size;
        c.size = if c.beam { 2.0 * reach } else { reach };
        c.brightness = if c.beam { 0.3 } else { 0.2 };
        c.beam_width = vis.max(1.0);
        c.size > 0.05
    });
    if omsi_cfg::flags::DEBUG_CONES.is_set() {
        log::info!(
            "cones: visibility {vis:.0} m, dark {night:.2}, {} cones of {} coronas",
            scene.coronas.iter().filter(|c| c.beam).count(),
            scene.coronas.len()
        );
        for c in scene.coronas.iter().filter(|c| c.beam).take(4) {
            log::info!("  cone at ({:.1}, {:.1}, {:.1}) dir {:?} radius {:.2} half angles {:.0}/{:.0} deg tex {}", c.position.x, c.position.y, c.position.z, c.direction, c.size, c.inner_cos.to_degrees(), c.cone_cos.to_degrees(), c.texture);
        }
    }
    if omsi_cfg::flags::DEBUG_LIGHT.is_set() {
        scene.lights.push(PointLight {
            position: camera_pos + DVec3::new(0.0, 15.0, -2.0),
            radius: 40.0,
            color: [1.0, 0.9, 0.7],
            intensity: 2.0,
            ..Default::default()
        });
        scene.coronas.push(Corona {
            position: camera_pos + DVec3::new(0.0, 15.0, 0.0),
            size: 1.0,
            color: [1.0, 0.9, 0.7],
            brightness: 1.0,
            direction: Vec3::ZERO,
            cone_cos: -1.0,
            ..Default::default()
        });
        log::info!(
            "static lights: {:?}",
            world
                .static_lights
                .lock()
                .iter()
                .take(3)
                .collect::<Vec<_>>()
        );
        log::info!(
            "static coronas: {:?}",
            world
                .static_coronas
                .lock()
                .iter()
                .take(3)
                .collect::<Vec<_>>()
        );
    }
    // nearest lights first: the grid cells hold a limited number
    // (total_cmp: a light at a NaN position must not end the game)
    scene.lights.sort_by(|a, b| {
        (a.position - camera_pos)
            .length_squared()
            .total_cmp(&(b.position - camera_pos).length_squared())
    });
}

/// OMSI's picture for every `[smoke]` particle: `Texture\rauch.tga` of the game folder.
pub fn load_smoke_texture(renderer: &mut omsi_render::Renderer, root: &std::path::Path) {
    let path = omsi_cfg::resolve_path(root, "Texture/rauch.tga");
    match omsi_texture::decode_file(&path) {
        Ok(img) => renderer.set_smoke_texture(&img),
        Err(e) => log::warn!("smoke texture {}: {e}", path.display()),
    }
}

/// The pictures of the coronas besides the standard glow: a light's own `bitmap` and the
/// fog cone's `Texture/light_cone.bmp`, numbered as they are first asked for and uploaded
/// before the next frame is drawn (`upload_corona_textures`).
struct CoronaTextures {
    ids: std::collections::HashMap<std::path::PathBuf, u16>,
    pending: Vec<(u16, std::path::PathBuf)>,
    root: Option<std::path::PathBuf>,
}

static CORONA_TEXTURES: std::sync::Mutex<Option<CoronaTextures>> = std::sync::Mutex::new(None);

/// The game folder the standard pictures (`Texture\…`) are found in.
pub fn set_corona_root(root: &std::path::Path) {
    let mut g = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner());
    let t = g.get_or_insert_with(|| CoronaTextures {
        ids: Default::default(),
        pending: Vec::new(),
        root: None,
    });
    t.root = Some(root.to_path_buf());
}

fn texture_id_of(path: std::path::PathBuf) -> u16 {
    let mut g = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner());
    let t = g.get_or_insert_with(|| CoronaTextures {
        ids: Default::default(),
        pending: Vec::new(),
        root: None,
    });
    if let Some(id) = t.ids.get(&path) {
        return *id;
    }
    let id = (t.ids.len() + 1).min(u16::MAX as usize) as u16;
    t.ids.insert(path.clone(), id);
    t.pending.push((id, path));
    id
}

/// The picture id of a light's `bitmap`, looked for as the model's own textures are (its
/// folder's `texture\`, the folder, the vehicle's `texture\`, the game's `Texture\`); 0 (the
/// standard glow) when there is no such file.
pub fn corona_texture_id(model_dir: &std::path::Path, name: &str) -> u16 {
    // (every lit lamp of every vehicle asks every frame: looked up once)
    static KNOWN: std::sync::Mutex<
        Option<std::collections::HashMap<(std::path::PathBuf, String), u16>>,
    > = std::sync::Mutex::new(None);
    let key = (model_dir.to_path_buf(), name.to_string());
    if let Some(&id) = KNOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(&key))
    {
        return id;
    }
    let root = CORONA_TEXTURES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|t| t.root.clone())
        .unwrap_or_default();
    let mut id = 0;
    for d in crate::world::scene::texture_dirs(&root, model_dir) {
        let p = omsi_cfg::resolve_path(&d, name);
        if omsi_cfg::vfs::is_file(&p) {
            id = texture_id_of(p);
            break;
        }
    }
    KNOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(Default::default)
        .insert(key, id);
    id
}

/// One of the game's own light pictures in `Texture\` (0, the built-in glow, if missing).
fn stock_texture_id(name: &str) -> u16 {
    // (asked for every light of every vehicle every frame: the file is looked up once per
    // game folder, not each time - the lookups were a fifth of a frame's CPU time)
    static KNOWN: std::sync::Mutex<
        Option<std::collections::HashMap<(std::path::PathBuf, String), u16>>,
    > = std::sync::Mutex::new(None);
    let root = CORONA_TEXTURES
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|t| t.root.clone())
        .unwrap_or_default();
    let key = (root, name.to_string());
    if let Some(&id) = KNOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .as_ref()
        .and_then(|m| m.get(&key))
    {
        return id;
    }
    let p = omsi_cfg::resolve_path(&key.0, &format!("Texture/{name}"));
    let id = if omsi_cfg::vfs::is_file(&p) {
        texture_id_of(p)
    } else {
        0
    };
    KNOWN
        .lock()
        .unwrap_or_else(|e| e.into_inner())
        .get_or_insert_with(Default::default)
        .insert(key, id);
    id
}

/// The picture of a snowflake (`Texture\snowflake.tga`, the original; 0, the built-in glow, if
/// missing).
pub fn snowflake_texture_id() -> u16 {
    stock_texture_id("snowflake.tga")
}

/// The fog cone's picture (`Texture\light_cone.bmp`).
pub fn cone_texture_id() -> u16 {
    stock_texture_id("light_cone.bmp")
}

/// A light's glow when it names no bitmap of its own, and the halo round it in fog
/// (`Texture\licht.bmp`, the original).
pub fn glow_texture_id() -> u16 {
    stock_texture_id("licht.bmp")
}

/// The star of a light with effect bit 1 (`Texture\light_effect1.bmp`).
pub fn star_texture_id() -> u16 {
    stock_texture_id("light_effect1.bmp")
}

/// Upload the pictures asked for since the last frame.
pub fn upload_corona_textures(renderer: &mut omsi_render::Renderer) {
    let pending = {
        let mut g = CORONA_TEXTURES.lock().unwrap_or_else(|e| e.into_inner());
        match g.as_mut() {
            Some(t) => std::mem::take(&mut t.pending),
            None => return,
        }
    };
    for (id, path) in pending {
        match omsi_texture::decode_file(&path) {
            Ok(img) => renderer.set_corona_texture(id, &img),
            Err(e) => log::warn!("corona picture {}: {e}", path.display()),
        }
    }
}

/// How strongly the lamps' cones show (0..1): fog, falling rain or snow, and the dark.
static CONE: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

static CONE_NIGHT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

/// The weather the lamps' cones are drawn in: the fog's visibility (m) and how dark it is
/// (0 day … 1 night). Falling rain or snow counts only as far as it lowers the visibility.
pub fn set_cone_strength(fog_visibility_m: f32, _precip: f32, night: f32) {
    CONE.store(
        fog_visibility_m.to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
    CONE_NIGHT.store(
        night.clamp(0.0, 1.0).to_bits(),
        std::sync::atomic::Ordering::Relaxed,
    );
}

fn cone_weather() -> (f32, f32) {
    let vis = f32::from_bits(CONE.load(std::sync::atomic::Ordering::Relaxed));
    let night = f32::from_bits(CONE_NIGHT.load(std::sync::atomic::Ordering::Relaxed));
    (if vis > 0.0 { vis } else { 1.0e6 }, night)
}
