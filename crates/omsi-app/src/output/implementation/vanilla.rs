//! Vanilla's side of the frame plan. `omsi-render` hands each picture's [`FramePlan`] to the
//! sink; the functions here translate it into what the Direct3D 9Ex device of
//! `omsi-render-vanilla` draws: a [`SceneFrame`], its batches, materials and HUD overlays.
//! They touch no device, so they build and are tested on every platform; the device that
//! takes them is [`device`] (Windows only).

use crate::output::VanillaEffects;
use omsi_render::{FramePlan, PlanBatch, PlanKind, Scene, TexAddressing};
use omsi_render_vanilla::{
    DrawKind, SceneBatch, SceneClouds, SceneCoronas, SceneFrame, SceneLamps, SceneLight,
    SceneMaterial, SceneRain, SceneSky, SceneSmoke, TexAddress, UiOverlay,
};

#[cfg(windows)]
mod device;
#[cfg(windows)]
pub(crate) use device::VanillaOutput;

/// What a [`SceneFrame`] borrows besides the plan and the scene: the plan's batches in the
/// device's terms and the lights as the scene lists them for this picture.
pub struct FrameParts<'a> {
    pub main: &'a [SceneBatch],
    pub cab: &'a [SceneBatch],
    pub lights: &'a [[f32; 4]],
}

/// One batch of the plan in the device's terms.
pub fn scene_batch(b: &PlanBatch) -> SceneBatch {
    SceneBatch {
        kind: match b.kind {
            PlanKind::Opaque => DrawKind::Opaque,
            PlanKind::AlphaTest => DrawKind::AlphaTest,
            PlanKind::Blend => DrawKind::Blend,
            PlanKind::BlendNoWrite => DrawKind::BlendNoWrite,
            PlanKind::SurfaceDepth => DrawKind::SurfaceDepth,
            PlanKind::TerrainPaint => DrawKind::TerrainPaint,
        },
        cull: b.cull,
        surface: b.surface,
        mesh: b.mesh,
        first: b.first,
        count: b.count,
        material: b.material,
        instances: b.instances.clone(),
    }
}

/// The plan's batches, in order, appended to `into`.
pub fn scene_batches(batches: &[PlanBatch], into: &mut Vec<SceneBatch>) {
    into.extend(batches.iter().map(scene_batch));
}

/// The frame the device draws: the plan's camera, light and lists, and what the scene holds for
/// this picture, with what `effects` leaves out left out. `seconds` is the time since the
/// last window picture (the raindrops on glass move on by it).
pub fn scene_frame<'a>(
    plan: &'a FramePlan,
    scene: &'a Scene,
    parts: FrameParts<'a>,
    effects: &VanillaEffects,
    seconds: f32,
) -> SceneFrame<'a> {
    SceneFrame {
        view_proj: plan.view_proj,
        light: SceneLight {
            cam_pos: plan.cam_pos,
            sun_dir: plan.sun_dir,
            sun_color: plan.sun_color,
            ambient: plan.ambient,
            fog: plan.fog,
            sky_color: plan.sky_color,
            cam_right: plan.cam_right,
            cam_up: plan.cam_up,
            weather: [
                plan.wetness,
                0.0,
                plan.world_origin[0],
                plan.world_origin[1],
            ],
        },
        list: &plan.list,
        models: scene.slot_models(),
        params: scene.slot_params(),
        main: parts.main,
        cab: parts.cab,
        surface_bias: omsi_cfg::flags::SURFACE_BIAS.number::<i32>().unwrap_or(-24),
        coronas: scene_coronas(scene, effects),
        smoke: scene_smoke(scene, effects),
        lamps: SceneLamps {
            lights: parts.lights,
            grid: scene.frame_light_grid(),
            place: plan.light_grid,
            tile_map: plan.light_map,
            cell_cap: plan.lamp_cap as usize,
        },
        rain: scene_rain(plan, scene, effects, seconds),
        sky: scene_sky(plan, scene, effects),
        rain_reflection: plan.rain_reflection,
    }
}

fn scene_coronas<'a>(scene: &'a Scene, effects: &VanillaEffects) -> SceneCoronas<'a> {
    if !effects.coronas {
        return SceneCoronas::default();
    }
    SceneCoronas {
        sprites: scene.frame_coronas(),
        runs: scene.frame_corona_runs(),
    }
}

/// OMSI's own particles; the renderer leaves the mirrors' out when the options say so, and the
/// game's tyre spray is no OMSI smoke: it is not among them.
fn scene_smoke<'a>(scene: &'a Scene, effects: &VanillaEffects) -> SceneSmoke<'a> {
    if !effects.smoke {
        return SceneSmoke::default();
    }
    SceneSmoke {
        sprites: scene.frame_smoke(),
    }
}

/// No rain on the glass: the drops never gather, and what had gathered dries.
fn scene_rain(
    plan: &FramePlan,
    scene: &Scene,
    effects: &VanillaEffects,
    seconds: f32,
) -> SceneRain {
    if effects.glass_rain {
        SceneRain {
            intensity: plan.rain,
            wiper: scene.glass_wiper,
            seconds,
        }
    } else {
        SceneRain {
            intensity: 0.0,
            wiper: None,
            seconds,
        }
    }
}

fn scene_sky(plan: &FramePlan, scene: &Scene, effects: &VanillaEffects) -> SceneSky {
    SceneSky {
        textures: scene.sky_textures(),
        params: plan.sky,
        // (no cloud picture: the sky is drawn without the layer; the clouds' cover still dims
        // the light, as the plan has it)
        clouds: SceneClouds {
            texture: scene.sky_cloud_picture(),
            params: plan.clouds,
        },
        no_cloud_layer: !effects.cloud_picture,
        no_stars: !effects.stars,
        bodies: {
            let (sun, moon) = scene.sky_bodies();
            [sun, moon]
        },
        moon_dir: plan.moon,
        stars: scene.sky_stars().map(|(t, _)| t),
        star_frame: plan.stars,
    }
}

/// The material `id` of the scene as the device takes it.
pub fn scene_material(scene: &Scene, id: usize) -> Option<SceneMaterial<'_>> {
    scene.materials.get(id).map(|m| SceneMaterial {
        texture: m.texture,
        transmap: m.transmap.map(|t| t.0),
        nightmap: m.nightmap,
        lightmap: m.lightmap,
        envmap: m.envmap.map(|e| e.0),
        env_mask: m.env_mask,
        bump: m.bump.map(|b| b.0),
        raindrop_map: m.raindrop_map,
        address: match m.addressing() {
            TexAddressing::Wrap => TexAddress::Wrap,
            TexAddressing::Mirror => TexAddress::Mirror,
            TexAddressing::Clamp => TexAddress::Clamp,
            TexAddressing::MirrorOnce => TexAddress::MirrorOnce,
        },
        uniform: m.uniform_vectors(),
    })
}

/// The HUD's pictures over the scene, appended to `into`.
pub fn ui_overlays(plan: &FramePlan, scene: &Scene, into: &mut Vec<UiOverlay>) {
    into.extend(plan.overlays.iter().map(|&(texture, rect)| UiOverlay {
        texture,
        rect,
        premultiplied: scene.premultiplied.contains(&texture),
        transposed: scene.transposed.contains(&texture),
    }));
}
