//! The camera uniform of a picture: what the shaders read of the camera, the lights, the
//! weather and the shadows' schedule, written once per picture.

use super::state::Renderer;
use crate::lighting::{LampShadow, Lighting};
use crate::passes::{
    glass_map_size, FrameFeatures, ShadowSchedule, SHADOW_CLOSE_MAX, SHADOW_RANGE,
    SHADOW_RANGE_CLOSE,
};
use crate::scene::{Camera, CameraUniform, Scene};
use crate::sky::CLOUD_ORIGIN_PERIOD;
use glam::{DVec3, Mat4, Vec3};

/// What the camera uniform is made of, settled by the earlier stages of the picture.
pub(super) struct UniformInputs<'a> {
    pub(super) scene: &'a Scene,
    pub(super) camera: &'a Camera,
    pub(super) lighting: &'a Lighting,
    pub(super) features: &'a FrameFeatures,
    pub(super) ro: DVec3,
    pub(super) cam_rel: Vec3,
    pub(super) vp_mat: Mat4,
    pub(super) grid: [f32; 4],
    pub(super) lamp_shadows: &'a [LampShadow],
    pub(super) schedule: &'a ShadowSchedule,
}

impl Renderer {
    /// The uniform for this picture (the caller writes it to `camera_buf`).
    pub(super) fn camera_uniform(&self, inputs: &UniformInputs) -> CameraUniform {
        let UniformInputs {
            scene,
            camera,
            lighting,
            features,
            ro,
            cam_rel,
            vp_mat,
            grid,
            lamp_shadows,
            schedule,
        } = *inputs;
        let FrameFeatures {
            glass_on,
            own_shading,
            ao_on,
            ..
        } = *features;
        let ShadowSchedule {
            shadows,
            light_view_proj,
            light_view_proj_far,
            light_view_proj_close,
            ..
        } = *schedule;
        CameraUniform {
            post: [
                if own_shading { 1.0 } else { 0.0 },
                self.started.elapsed().as_secs_f32(),
                // (z: the roads are kept clear of the snow, `Lighting::roads_clear`)
                if lighting.roads_clear { 1.0 } else { 0.0 },
                // (the sun's height on the screen is read by no shader any more: the close
                // shadow map's share of its half of the atlas)
                self.options.shadow_size.min(SHADOW_CLOSE_MAX) as f32
                    / self.options.shadow_size.max(1) as f32,
            ],
            view_proj: vp_mat.to_cols_array_2d(),
            cam_pos: cam_rel.extend(1.0).to_array(),
            // (modulo the shaders' PATTERN_PERIOD, 1000 m: the patterns repeat with it, and
            // the whole map coordinate has no precision left for them in 32 bits)
            // (zw: the origin modulo CLOUD_ORIGIN_PERIOD for the sky's clouds, which are
            // drawn over ground points: taken relative to the floating origin, the whole
            // cloud field jumped by the origin's step each time it moved on)
            world_origin: [
                ro.x.rem_euclid(1000.0) as f32,
                ro.y.rem_euclid(1000.0) as f32,
                ro.x.rem_euclid(CLOUD_ORIGIN_PERIOD) as f32,
                ro.y.rem_euclid(CLOUD_ORIGIN_PERIOD) as f32,
            ],
            sun_dir: lighting
                .sun_dir
                .normalize()
                .extend(lighting.sun_intensity)
                .to_array(),
            // (Vanilla+: the moonlight added; nothing in Vanilla and Enhanced)
            ambient: (lighting.ambient + lighting.moonlight())
                .extend(lighting.snow.clamp(0.0, 1.0))
                .to_array(),
            fog: lighting.fog_color.extend(lighting.fog_density).to_array(),
            sun_color: lighting
                .sun_color
                .extend(lighting.night_maps.unwrap_or(lighting.night))
                .to_array(),
            sky_color: lighting.secondary.extend(0.0).to_array(),
            light_grid: grid,
            sky: [
                lighting.sun_azimuth,
                lighting.sky_weights[0],
                lighting.sky_weights[1],
                lighting.sky_weights[2],
            ],
            clouds: [
                lighting.cloud_density,
                lighting.cloud_offset[0],
                lighting.cloud_offset[1],
                if ao_on { 1.0 } else { 0.0 },
            ],
            // (w: the heading the sphere maps are laid out by in the headset, see
            // `set_env_heading`; flagged by cam_up.w)
            cam_right: camera
                .right()
                .extend(
                    self.env_heading
                        .get()
                        .map(|h| h.to_radians())
                        .unwrap_or(0.0),
                )
                .to_array(),
            cam_up: camera
                .right()
                .cross(camera.forward())
                .normalize_or_zero()
                .extend(if self.env_heading.get().is_some() {
                    1.0
                } else {
                    0.0
                })
                .to_array(),
            light_view_proj: light_view_proj.to_cols_array_2d(),
            light_view_proj_far: light_view_proj_far.to_cols_array_2d(),
            shadow: [
                if shadows { 1.0 } else { 0.0 },
                1.0 / self.options.shadow_size as f32,
                SHADOW_RANGE,
                lighting.wetness.clamp(0.0, 1.0),
            ],
            inside_a: match lighting.inside {
                Some((o, h, _)) => {
                    let r = (o - ro).as_vec3();
                    [r.x, r.y, r.z, (h as f32).to_radians().sin()]
                }
                None => [0.0; 4],
            },
            inside_b: match lighting.inside {
                Some((_, h, bb)) => [
                    (h as f32).to_radians().cos(),
                    bb[0] * 0.5,
                    bb[1] * 0.5,
                    bb[2] * 0.5,
                ],
                None => [1.0, 0.0, 0.0, 0.0],
            },
            inside_c: match lighting.inside {
                Some((_, _, bb)) => [bb[3], bb[4], bb[5], 1.0],
                None => [0.0; 4],
            },
            flags: [
                if lighting.detail { 1.0 } else { 0.0 },
                if own_shading { 1.0 } else { 0.0 },
                // (below zero: the rain films have the clean current picture to look through,
                // see `rain_behind`; above zero is an old branch never taken)
                if glass_on { -1.0 } else { 0.0 },
                if shadows { SHADOW_RANGE_CLOSE } else { 0.0 },
            ],
            light_view_proj_close: light_view_proj_close.to_cols_array_2d(),
            wind: [
                lighting.glass_wind.x,
                lighting.glass_wind.y,
                lighting.glass_wind.z,
                1.0,
            ],
            lamp_view_proj: std::array::from_fn(|k| {
                lamp_shadows
                    .get(k)
                    .map_or(Mat4::IDENTITY, |l| l.view_proj())
                    .to_cols_array_2d()
            }),
            lamp_shadow: std::array::from_fn(|k| {
                lamp_shadows.get(k).map_or(-1.0, |l| l.index as f32)
            }),
            tree_wind: {
                // the gusts drift with the wind: summed over the frames (the wind changes
                // with the weather), in f64 and modulo the shaders' pattern period
                let now = self.started.elapsed().as_secs_f64();
                let (drift, last) = self.tree_gust_drift.get();
                let drift = (drift
                    + lighting.wind.truncate().as_dvec2() * (now - last).clamp(0.0, 1.0))
                .rem_euclid(glam::DVec2::splat(1000.0));
                self.tree_gust_drift.set((drift, now));
                let wind = if lighting.windy_trees {
                    lighting.wind.truncate()
                } else {
                    glam::Vec2::ZERO
                };
                [wind.x, wind.y, drift.x as f32, drift.y as f32]
            },
            // (w 0 when the game has set neither picture: the sky skips them altogether)
            moon: lighting
                .moon_dir
                .normalize_or_zero()
                .extend(if scene.sky_sun.is_some() || scene.sky_moon.is_some() {
                    1.0
                } else {
                    0.0
                })
                .to_array(),
            stars: {
                let f = lighting.star_frame;
                let cells = if scene.sky_stars.is_some() {
                    scene.star_cells as f32
                } else {
                    0.0
                };
                [
                    [
                        f[0][0],
                        f[0][1],
                        f[0][2],
                        lighting.star_brightness.clamp(0.0, 1.0),
                    ],
                    [f[1][0], f[1][1], f[1][2], cells],
                    [f[2][0], f[2][1], f[2][2], 0.0],
                ]
            },
            glass: [
                if own_shading && shadows { 1.0 } else { 0.0 },
                glass_map_size(self.options.shadow_size) as f32,
                0.0,
                0.0,
            ],
        }
    }
}
