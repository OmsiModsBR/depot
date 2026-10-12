//! wgpu renderer.

pub mod atmosphere;
mod camera;
pub mod clouds;
mod draw;
mod frame;
mod gpu;
mod lighting;
mod material;
mod pipelines;
mod puddles;
mod renderer;
mod resources;
mod rt;
mod scene;
mod shaders;
mod surface;
mod targets;
mod triple;
pub use camera::*;
use draw::*;
#[cfg(test)]
use frame::*;
pub use gpu::*;
pub use lighting::*;
pub use material::*;
use pipelines::*;
pub use renderer::*;
pub use resources::*;
pub use scene::*;
use shaders::*;
pub use surface::*;
use targets::*;
pub use triple::{panel_width, ScreenView, TripleScreen};

use anyhow::{anyhow, Context, Result};
use glam::{DVec3, Mat4, Vec3, Vec4};
use omsi_geometry::MeshData;
use std::collections::HashMap;
use std::sync::Arc;

/// The period the sky's cloud patterns repeat with (m): 5 x the cloud field (14 km), 8 x
/// its billow detail (8.75 km) and 28 x the high layer's (2.5 km) - the render origin is
/// taken modulo this for them, which keeps centimetres of precision in 32 bits.
const CLOUD_ORIGIN_PERIOD: f64 = 70000.0;

/// The enhanced path's post passes (post.wgsl `PostParams`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PostUniform {
    a: [f32; 4],
    b: [f32; 4],
    c: [f32; 4],
    d: [f32; 4],
}

/// The enhanced lighting (enhanced_common.wgsl `Enhanced`).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EnhancedUniform {
    exposure: [f32; 4],
    sun: [f32; 4],
    sh: [[f32; 4]; 9],
    ground: [f32; 4],
    fog: [f32; 4],
    fog_color: [f32; 4],
    weather: [f32; 4],
    lights: [f32; 4],
    sun_disc: [f32; 4],
    debug: [f32; 4],
    /// xyz where the sky cube was drawn from, relative to the camera (the dome looks the
    /// clouds up through it with the parallax taken out)
    eye: [f32; 4],
    /// x how bright an LED panel's dots burn (`Lighting::led_glow`), y how much of the
    /// mip chain an LED panel is held at (`Lighting::led_mips`)
    led: [f32; 4],
    /// xyz towards the moon, w its angular radius
    moon: [f32; 4],
    /// rgb the moon disc's irradiance at the ground before the clouds, w how much of the
    /// starry sky shows (the city's lamps and the haze wash the faint stars out)
    moon_disc: [f32; 4],
    /// rgb the sun's irradiance at the clouds' heights (`atmosphere::CLOUD_SUN_HEIGHTS`)
    cloud_sun: [[f32; 4]; 4],
    /// rgb the moonlight on a surface facing the moon (after the clouds), w 1 while the
    /// shadow maps are the moon's (`Lighting::casts_moon_shadows`)
    moon_light: [f32; 4],
    /// `Lighting::condensation`
    condensation: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct FogLampUniform {
    inv_view_proj: [[f32; 4]; 4],
    size: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SsaoUniform {
    inv_proj: [[f32; 4]; 4],
    params: [f32; 4],
    /// xy: the projection's off-centre shift (0 for the window's own symmetric frustum;
    /// a headset eye or a triple screen's side panel has one)
    shift: [f32; 4],
}

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    fn render_inner(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        with_overlays: bool,
        exclude_texture: Option<TextureId>,
        projection: Option<Mat4>,
        second_eye: bool,
    ) {
        // test hook for a lost device (a driver reset): its resources are taken away and
        // the session has to end in order
        if omsi_cfg::env::var("OMSI_FAKE_GPU_ERROR").as_deref() == Ok("lost")
            && with_overlays
            && self.started.elapsed().as_secs_f32() > 3.0
            && self.device_lost().is_none()
        {
            log::error!("the graphics device was lost (test): OMSI_FAKE_GPU_ERROR=lost");
            self.device.destroy();
            *self.device_lost.lock().unwrap_or_else(|e| e.into_inner()) = Some("test".into());
        }
        // the device is gone: nothing can be drawn, and the readbacks (the exposure meter)
        // would find their buffers taken away - "Error in Buffer::get_mapped_range:
        // Validation Error" ended the game instead of the session ending in order
        if self
            .device_lost
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .is_some()
        {
            return;
        }
        if omsi_cfg::env::var("OMSI_FAKE_GPU_ERROR").as_deref() == Ok("frame")
            && self.options.msaa > 1
            && self.started.elapsed().as_secs_f32() > 3.0
        {
            // test hook for the fallback below
            let _ = self.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("invalid"),
                size: wgpu::Extent3d {
                    width: 4,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 3,
                dimension: wgpu::TextureDimension::D2,
                format: self.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
        }
        if self.rt_error.load(std::sync::atomic::Ordering::Relaxed) {
            self.fall_back_without_ray_tracing(scene);
        } else if self.gpu_error.load(std::sync::atomic::Ordering::Relaxed) {
            self.fall_back_to_single_sample(scene);
        }
        // OMSI_GPU_TIMERS: GPU time per pass, one frame at a time
        self.collect_gpu_timers();
        let tset: Option<wgpu::QuerySet> = self.gpu_timers[with_overlays as usize]
            .as_ref()
            .filter(|t| !t.waiting && !t.unresolved)
            .map(|t| t.set.clone());
        let mut timed: Vec<&'static str> = Vec::new();
        // OMSI_PROFILE: time per stage, the mirrors apart from the window's picture
        let mut stage_t = std::time::Instant::now();
        let mut stage = |r: &Renderer, window: &'static str, mirror: &'static str| {
            if r.profiling {
                let now = std::time::Instant::now();
                *r.stats
                    .borrow_mut()
                    .entry(if with_overlays { window } else { mirror })
                    .or_default() += (now - stage_t).as_secs_f64();
                stage_t = now;
            }
        };
        // render origin: the camera position rounded to 100 m, kept while the camera stays near it
        if (camera.position - scene.render_origin).abs().max_element() > 200.0 {
            self.set_render_origin(scene, (camera.position / 100.0).floor() * 100.0);
        }
        let ro = scene.render_origin;
        // The window's 3D picture may be drawn smaller and scaled up to it (render scale):
        // from here on `width` and `height` are the size of the picture, `full_*` the
        // window's (the HUD is drawn at that size). Mirrors keep their own size.
        let (full_w, full_h) = (width, height);
        let (width, height) = if with_overlays {
            self.scene_size(full_w, full_h)
        } else {
            (full_w, full_h)
        };
        // FXAA on the plain graphics too, where there is no multisampling to smooth the
        // edges (the Enhanced path has it in its post passes): the picture is drawn into a
        // texture of the window's size and smoothed on its way to the window, the HUD after
        let vanilla_fxaa = with_overlays
            && (width, height) == (full_w, full_h)
            && self.options.fxaa
            && self.options.msaa <= 1
            && !(lighting.enhanced
                && self.hdr_pass.is_some()
                && omsi_cfg::env::var_os("OMSI_NO_ENHANCED").is_none())
            && omsi_cfg::env::var_os("OMSI_NO_FXAA").is_none();
        // Rain films are drawn after copying the clean current-frame scene. Classic
        // graphics need a sampleable scene target too, even without puddles or scaling.
        let enhanced_view = lighting.enhanced
            && self.hdr_pass.is_some()
            && omsi_cfg::env::var_os("OMSI_NO_ENHANCED").is_none();
        let glass_on = with_overlays
            && scene.glass_slot.is_some()
            && (lighting.rain > 0.001 || lighting.wetness > 0.02)
            && omsi_cfg::env::var_os("OMSI_NO_GLASS_PICTURE").is_none();
        let scaled =
            (width, height) != (full_w, full_h) || vanilla_fxaa || (glass_on && !enhanced_view);
        let scene_target: Option<(wgpu::TextureView, wgpu::BindGroup)> = if scaled {
            Some(self.scale_target(width, height))
        } else {
            None
        };
        let scene_view: &wgpu::TextureView = scene_target.as_ref().map(|t| &t.0).unwrap_or(target);
        let aspect = self
            .texture_aspect
            .unwrap_or(width as f32 / height.max(1) as f32);
        let cam_rel = (camera.position - ro).as_vec3();
        // The mirrors are drawn with the plain shading even in Enhanced: without the depth
        // prepass (sized for the window) every layer of a mirror's picture ran the enhanced
        // shader, 12.7 ms of GPU time for one 256-pixel mirror against 5.8 ms for the whole
        // window; plainly shaded it is 0.6 ms, and a mirror's small picture shows no
        // difference worth that. `OMSI_MIRROR_ENHANCED=1` draws them enhanced again.
        // The headset's eyes are the real picture as much as the window is (#784: VR showed
        // the plain graphics with Enhanced on); the first eye is the one that moves the
        // exposure, the sky cube and the frame clock on, as the window does without VR.
        let xr_view = projection.is_some();
        let lead_view = (with_overlays || xr_view) && !second_eye;
        let enhanced_frame = lighting.enhanced
            && self.hdr_pass.is_some()
            && omsi_cfg::env::var_os("OMSI_NO_ENHANCED").is_none()
            && (with_overlays
                || xr_view
                || omsi_cfg::env::var_os("OMSI_MIRROR_ENHANCED").is_some());
        // the mirrors are drawn by the same path as the window (their picture graded with
        // the window's exposure, see the post passes)
        let enhanced = enhanced_frame;
        // Rain reflections belong to all graphics modes. Classic shading uses the same
        // scene/mask targets, then presents their linear colour without Enhanced grading.
        let puddles_wanted = with_overlays
            && self.puddles.is_some()
            && self.options.reflections
            && lighting.wetness * (1.0 - lighting.snow.clamp(0.0, 1.0)) > 0.05
            && scene.materials.iter().any(|m| m.uniform.params2[2] > 0.0)
            && debug_view() == 0.0
            && omsi_cfg::env::var_os("OMSI_NO_PUDDLE_REFLECTIONS").is_none();
        let reflection_frame = !enhanced && puddles_wanted && self.reflection_pass.is_some();
        let masked_frame = enhanced || reflection_frame;
        let (grid, lamp_shadows) = self.prepare_lights(
            scene,
            cam_rel,
            enhanced_frame,
            lighting.lamp_shadows
                && with_overlays
                && projection.is_none()
                && self.shadow_pipelines.len() > 6,
        );
        self.prepare_coronas(
            scene,
            lighting.night,
            lighting
                .inside
                .as_ref()
                .filter(|v| point_in_vehicle_box(camera.position, v)),
        );
        self.prepare_smoke(scene, camera.position);
        // ambient occlusion only for the real picture, not for the mirrors
        // Enhanced+: the window's picture traces its sun shadow and ambient occlusion (not the
        // mirrors, a headset's eyes or a triple screen's panels: they keep the shadow map)
        let rt_frame = self.rt.is_some()
            && enhanced
            && with_overlays
            && projection.is_none()
            && omsi_cfg::env::var_os("OMSI_NO_RT_FRAME").is_none();
        let ao_on = with_overlays
            && self.rt.is_none()
            && self.options.ssao
            && self.ssao_pipeline.is_some()
            && omsi_cfg::env::var_os("OMSI_NO_AO").is_none();
        // the enhanced path's shading is costly: the depth prepass keeps it to the visible
        // surface (without multisampling, see `share_depth`)
        let prepass_on =
            ao_on || puddles_wanted || glass_on || (enhanced && (with_overlays || xr_view));
        if prepass_on && self.ensure_ao(width, height) {
            // a new AO texture: the camera bind group must point at it
            scene.dirty = true;
            scene.model_buf = None;
            self.hdr_targets.clear();
        }
        if rt_frame
            && self
                .rt
                .as_mut()
                .is_some_and(|rt| rt.ensure_targets(&self.device, width, height))
        {
            // the camera bind group must point at the new lighting texture
            scene.dirty = true;
            scene.model_buf = None;
        }
        if masked_frame {
            self.hdr_targets(width, height);
        }
        if glass_on {
            self.prepare_glass_behind(
                scene,
                width,
                height,
                if masked_frame {
                    HDR_FORMAT
                } else {
                    self.format
                },
            );
        }
        let dt = {
            let now = std::time::Instant::now();
            let dt = self
                .last_frame
                .map(|t| (now - t).as_secs_f32())
                .unwrap_or(0.0);
            if lead_view {
                self.last_frame = Some(now);
            }
            dt
        };
        stage(self, "setup", "mirror.setup");
        self.prepare(scene);
        stage(self, "prepare", "mirror.prepare");
        // overlay uniforms/bind groups (rects in pixels → NDC)
        let overlays: Vec<(TextureId, [f32; 4])> = if with_overlays {
            scene.overlays.clone()
        } else {
            Vec::new()
        };
        if with_overlays {
            self.prepare_overlays(scene, full_w, full_h);
        }
        // sun shadow map: an orthographic box around the camera, looking along the sun (by
        // night along the moon, see `Lighting::casts_moon_shadows`)
        let moon_shadows = enhanced_frame && lighting.casts_moon_shadows();
        let sun = if moon_shadows {
            lighting.moon_dir.normalize_or_zero()
        } else {
            lighting.sun_dir.normalize_or_zero()
        };
        let shadows = (with_overlays || projection.is_some())
            && (lighting.casts_sun_shadows() || moon_shadows);
        let shared_xr_shadows = if second_eye && shadows {
            self.xr_shadow_cache
                .get()
                .filter(|(origin, previous_sun, _, _, _)| {
                    *origin == scene.render_origin && *previous_sun == sun
                })
        } else {
            None
        };
        let draw_shadows = shadows && shared_xr_shadows.is_none();
        let light_matrix = |range: f32| {
            // Snap the centre to whole texels so the map does not shimmer while driving.
            let texel = range * 2.0 / self.options.shadow_size as f32;
            let up = if sun.z.abs() > 0.95 { Vec3::Y } else { Vec3::Z };
            let raw = cam_rel;
            let view0 = Mat4::look_at_rh(sun * 900.0, Vec3::ZERO, up);
            let ls = view0.transform_point3(raw);
            let snapped = Vec3::new(
                (ls.x / texel).round() * texel,
                (ls.y / texel).round() * texel,
                ls.z,
            );
            let center = view0.inverse().transform_point3(snapped);
            let view = Mat4::look_at_rh(center + sun * 900.0, center, up);
            let proj = Mat4::orthographic_rh(-range, range, -range, range, 1.0, 2200.0);
            proj * view
        };
        // The near cascade (140 m, 4096 texels at the top setting: the costliest shadow
        // pass, a third of it the trees' leaf cards) is drawn every other frame and kept
        // for the next, with the light matrix it was drawn with; the close one - the bus
        // and everything within 30 m - every frame. Redrawn at once when the camera has
        // jumped, the sun has moved or the render origin has (its matrix is relative to it).
        let near_wanted = light_matrix(SHADOW_RANGE);
        let (near_m, near_age, near_origin, near_sun) = self.shadow_near_cache.get();
        let near_jumped =
            (near_m.project_point3(cam_rel) - near_wanted.project_point3(cam_rel)).length() > 0.03;
        let redraw_near = draw_shadows
            && (near_age >= 1
                || near_jumped
                || near_m == Mat4::IDENTITY
                || near_origin != scene.render_origin
                || near_sun.dot(sun) < 0.99999
                || omsi_cfg::env::var_os("OMSI_SHADOW_NEAR_EVERY_FRAME").is_some());
        let light_view_proj = if let Some((_, _, near, _, _)) = shared_xr_shadows {
            near
        } else if !shadows {
            near_wanted
        } else if redraw_near {
            self.shadow_near_cache
                .set((near_wanted, 0, scene.render_origin, sun));
            near_wanted
        } else {
            self.shadow_near_cache
                .set((near_m, near_age + 1, near_origin, near_sun));
            near_m
        };
        let light_view_proj_close = shared_xr_shadows
            .map(|(_, _, _, _, close)| close)
            .unwrap_or_else(|| light_matrix(SHADOW_RANGE_CLOSE));
        // The far cascade (700 m, metre-sized texels) is drawn every 4th frame, or at once
        // when the camera has left the middle of the one drawn, the sun has moved or the
        // render origin has jumped (its matrix is relative to that). Drawn every frame it
        // was 0.6 ms of GPU time for a picture that hardly changes; a car in it is a few
        // texels, and a tile streamed in waits three frames at most for its shadow.
        let far_wanted = light_matrix(SHADOW_RANGE_FAR);
        let (far_m, far_age, far_origin, far_sun) = self.shadow_far_cache.get();
        let far_moved =
            (far_m.project_point3(cam_rel) - far_wanted.project_point3(cam_rel)).length() > 0.12;
        let redraw_far = draw_shadows
            && (far_age >= 3
                || far_moved
                || far_m == Mat4::IDENTITY
                || far_origin != scene.render_origin
                || far_sun.dot(sun) < 0.99999
                || omsi_cfg::env::var_os("OMSI_SHADOW_FAR_EVERY_FRAME").is_some());
        if redraw_far && omsi_cfg::env::var_os("OMSI_DEBUG_SHADOW_FAR").is_some() {
            log::info!(
                "far shadow redrawn: age {far_age} moved {far_moved} origin {} sun {:.6}",
                far_origin != scene.render_origin,
                far_sun.dot(sun)
            );
        }
        // The desktop view and the first XR eye maintain this cache; mirrors and
        // the second XR eye leave it alone.
        let light_view_proj_far = if let Some((_, _, _, far, _)) = shared_xr_shadows {
            far
        } else if !shadows {
            far_wanted
        } else if redraw_far {
            self.shadow_far_cache
                .set((far_wanted, 0, scene.render_origin, sun));
            far_wanted
        } else {
            self.shadow_far_cache
                .set((far_m, far_age + 1, far_origin, far_sun));
            far_m
        };
        if projection.is_some() && !second_eye && shadows {
            self.xr_shadow_cache.set(Some((
                scene.render_origin,
                sun,
                light_view_proj,
                light_view_proj_far,
                light_view_proj_close,
            )));
        }
        let vp_mat = projection
            .map(|p| {
                p * Mat4::look_to_rh(
                    (camera.position - ro).as_vec3(),
                    camera.forward(),
                    camera.up(),
                )
            })
            .unwrap_or_else(|| camera.view_proj(aspect, ro));
        // where the tile light maps lie, relative to the render origin
        {
            let (lx, ly, side) = self.lm_place.get();
            let v: [f32; 4] = [
                (lx - ro.x) as f32,
                (ly - ro.y) as f32,
                side as f32,
                if side > 0.0 { 1.0 } else { 0.0 },
            ];
            self.queue
                .write_buffer(&self.lm_uniform, 0, bytemuck::cast_slice(&v));
        }
        let cu = CameraUniform {
            post: [
                if enhanced { 1.0 } else { 0.0 },
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
            ambient: lighting
                .ambient
                .extend(lighting.snow.clamp(0.0, 1.0))
                .to_array(),
            fog: lighting.fog_color.extend(lighting.fog_density).to_array(),
            sun_color: lighting
                .sun_color
                .extend(lighting.night_maps.unwrap_or(lighting.night))
                .to_array(),
            sky_color: lighting
                .secondary
                .extend(if lighting.classic && !enhanced {
                    1.0
                } else {
                    0.0
                })
                .to_array(),
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
                // (2: the traced lighting, full size, see `ao_at`)
                if rt_frame {
                    2.0
                } else if ao_on {
                    1.0
                } else {
                    0.0
                },
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
                if enhanced { 1.0 } else { 0.0 },
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
        };
        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cu));
        if rt_frame {
            let proj =
                Mat4::perspective_rh(camera.fov_deg.to_radians(), aspect, camera.far, camera.near);
            self.prepare_ray_tracing(scene, camera, lighting, vp_mat, proj, width, height, dt);
            stage(self, "ray tracing", "mirror.ray tracing");
        }
        // (a mirror takes the window's light - its own call would move the exposure on -
        // unless it comes before the window's first frame)
        if enhanced && lead_view {
            let ground = lighting
                .fog_base
                .or(lighting.inside.map(|v| v.0.z))
                .map(|z| (z - scene.render_origin.z) as f32);
            self.view_lamps = Some(view_lamp_light(
                &scene.lights,
                scene.render_origin,
                cam_rel,
                camera,
                aspect,
                ground,
            ));
            if omsi_cfg::env::var_os("OMSI_DEBUG_VIEW_LAMPS").is_some() {
                log::info!("view lamps: {:.6}", self.view_lamps.unwrap_or(0.0));
            }
        }
        // the night sky's glow from the lamps round the camera (the window's view leads), in
        // steps of a tenth: the sky is recomputed for a new value, not for every metre driven
        if enhanced && lead_view {
            let raw = lamp_sky_glow(scene, cam_rel);
            let target = raw.clamp(0.03, 1.5);
            let step = (target.ln() * 10.0).round() / 10.0;
            let changed = self.city_glow.is_none_or(|g| (g.ln() - step).abs() > 0.15);
            if changed {
                self.city_glow = Some(step.exp());
            }
            if omsi_cfg::env::var_os("OMSI_DEBUG_SKY").is_some() && changed {
                log::info!(
                    "sky glow from the lamps: {raw:.3} (taken {:.3})",
                    step.exp()
                );
            }
        }
        let probe_redraw = enhanced
            && (lead_view || self.sky_state.is_none())
            && self.prepare_enhanced(lighting, cam_rel, ro, dt);
        // --- what every pass draws, as batches over one draw list (see `Batch`): the shadow
        // casters of each cascade, the depth prepass and the main pass. The list is built
        // and uploaded before any pass is encoded.
        stage(self, "setup", "mirror.setup");
        let debug_draws = omsi_cfg::env::var_os("OMSI_DEBUG_DRAWS").is_some();
        let debug_cull = omsi_cfg::env::var_os("OMSI_DEBUG_CULL").is_some();
        let mut list: Vec<u32> = Vec::new();
        let mut items: Vec<DrawItem> = Vec::new();
        // near, far, close
        // (3: the street lamps' maps, every caster within a chosen lamp's reach under its head)
        let mut shadow_batches: [Vec<Batch>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let lamp_reach = |c: Vec3, r: f32| {
            lamp_shadows
                .iter()
                .any(|l| c.z - r < l.position.z && (c - l.position).length() < l.range + r)
        };
        let kind_of = |alpha: AlphaMode| -> u8 {
            match alpha {
                AlphaMode::Opaque => PIPE_OPAQUE,
                AlphaMode::Test => PIPE_ALPHA_TEST,
                AlphaMode::Blend => PIPE_BLEND,
            }
        };
        // an instance's screen size as the camera pass measures it for the LOD choice
        let lod_fov = camera.fov_deg.to_radians().max(1e-3);
        let lod_size = |inst: &Instance| -> f32 {
            let scale = Self::instance_scale(scene, inst);
            let radius = if inst.object_radius > 0.0 {
                inst.object_radius
            } else {
                scene.meshes[inst.mesh].bounds_radius
            } * scale;
            let d = ((inst.origin - scene.render_origin).as_vec3() - cam_rel).length();
            if d <= radius {
                f32::MAX
            } else {
                2.0 * radius / (d.max(0.01) * lod_fov)
            }
        };
        let active = [
            draw_shadows && redraw_near,
            draw_shadows && redraw_far,
            draw_shadows,
        ];
        let boxes = [
            (SHADOW_RANGE, light_view_proj, 0.4f32),
            (SHADOW_RANGE_FAR, light_view_proj_far, 6.0),
            (SHADOW_RANGE_CLOSE, light_view_proj_close, 0.1),
        ];
        let dbg_shadow = omsi_cfg::env::var_os("OMSI_DEBUG_SHADOW").is_some();
        let dbg_r: f32 = omsi_cfg::env::var("OMSI_DEBUG_SHADOW")
            .ok()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(3.0);
        let casters = |span: std::ops::Range<usize>| -> [Vec<DrawItem>; 4] {
            let mut out: [Vec<DrawItem>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
            let mut ranges: Vec<(u8, u32, u32, usize, u32)> = Vec::new();
            for inst in &scene.instances[span] {
                if !inst.visible
                    || !inst.casts_shadow
                    || (self.options.omsi_shadow_casters && !inst.omsi_caster)
                {
                    continue;
                }
                // a stand-in for far tiles (`set_near_only`) left out of the picture casts no
                // shadow either: London's bridge lamps, hidden as part of a backdrop, still
                // threw their shadows on the deck (#1545)
                if let Some([x0, y0, x1, y1]) = inst.near_only {
                    let c = camera.position;
                    if c.x < x0 || c.x > x1 || c.y < y0 || c.y > y1 {
                        continue;
                    }
                }
                let m = &scene.meshes[inst.mesh];
                if m.ranges.is_empty() {
                    continue;
                }
                let (c, r) = Self::bounding_sphere(scene, inst);
                if inst.lod.0 > 0.0 || inst.lod.1 < f32::MAX {
                    let size = lod_size(inst);
                    if size < inst.lod.0 || (inst.lod.1 < f32::MAX && size >= inst.lod.1) {
                        if dbg_shadow && r >= dbg_r {
                            log::info!(
                                "shadow: mesh r={r:.1} at {:?} is another LOD than the one shown",
                                inst.origin
                            );
                        }
                        continue;
                    }
                }
                ranges.clear();
                for (ri, (_, _, slot)) in m.ranges.iter().enumerate() {
                    let mat_id = inst.materials.get(*slot as usize).copied().unwrap_or(0);
                    let mat = &scene.materials[mat_id];
                    let mut kind = kind_of(mat.alpha);
                    let cut_body = kind == PIPE_BLEND && mat.transmap.is_some() && !mat.no_z_write;
                    if (kind == PIPE_BLEND && !cut_body) || mat.no_z_check {
                        if dbg_shadow && r >= dbg_r {
                            log::info!("shadow: mesh r={r:.1} at {:?} slot {slot} is blended, never a caster", inst.origin);
                        }
                        continue;
                    }
                    if cut_body {
                        kind = PIPE_ALPHA_TEST;
                    }
                    // (Enhanced+: what is solid casts its shadow by the traced rays; the
                    // close and near maps keep the cut-out leaves and fences, whose texels
                    // the rays cannot see)
                    if rt_frame && kind == PIPE_OPAQUE && !moon_shadows {
                        kind = PIPE_KINDS;
                    }
                    ranges.push((kind, ri as u32, *slot, mat_id, mat.look));
                }
                if !lamp_shadows.is_empty()
                    && !(m.bounds_radius > 0.0 && m.bounds_radius < 0.1)
                    && lamp_reach(c, r)
                {
                    for &(kind, ri, slot, mat_id, look) in &ranges {
                        let kind = if kind == PIPE_KINDS {
                            PIPE_OPAQUE
                        } else {
                            kind
                        };
                        let (material, look) = depth_only_material(kind, mat_id, look);
                        out[3].push(DrawItem {
                            pipe: kind,
                            mesh: inst.mesh as u32,
                            range: ri,
                            material,
                            look,
                            entry: inst.base + slot,
                        });
                    }
                }
                for (cascade, &(range, lvp, min_radius)) in boxes.iter().enumerate() {
                    if !active[cascade] {
                        continue;
                    }
                    let dbg = dbg_shadow && cascade == 0 && r >= dbg_r;
                    if m.bounds_radius > 0.0 && m.bounds_radius < min_radius {
                        if dbg_shadow && cascade == 0 && m.bounds_radius >= dbg_r {
                            log::info!(
                                "shadow: mesh r={:.1} skipped (ranges {})",
                                m.bounds_radius,
                                m.ranges.len()
                            );
                        }
                        continue;
                    }
                    let lc = lvp.project_point3(c);
                    let rr = r / range;
                    if lc.x.abs() > 1.0 + rr || lc.y.abs() > 1.0 + rr {
                        if dbg {
                            log::info!(
                                "shadow: mesh r={r:.1} outside the light box at ({:.2}, {:.2})",
                                lc.x,
                                lc.y
                            );
                        }
                        continue;
                    }
                    if dbg {
                        log::info!(
                            "shadow: caster r={r:.1} at {:?} slots {:?}",
                            inst.origin,
                            ranges.iter().map(|x| x.0).collect::<Vec<_>>()
                        );
                    }
                    for &(kind, ri, slot, mat_id, look) in &ranges {
                        let kind = if kind == PIPE_KINDS {
                            if cascade != 1 {
                                continue;
                            }
                            PIPE_OPAQUE
                        } else {
                            kind
                        };
                        // (after the remap: a traced-opaque one is drawn as the opaque it is)
                        let (material, look) = depth_only_material(kind, mat_id, look);
                        out[cascade].push(DrawItem {
                            pipe: kind,
                            mesh: inst.mesh as u32,
                            range: ri,
                            material,
                            look,
                            entry: inst.base + slot,
                        });
                    }
                }
            }
            out
        };
        if active.iter().any(|a| *a) || !lamp_shadows.is_empty() {
            let n = scene.instances.len();
            let parts = (n / 8192).clamp(
                1,
                self.encoding_pool
                    .as_ref()
                    .map_or(3, |p| p.current_num_threads())
                    + 1,
            );
            let chunk = n.div_ceil(parts).div_ceil(CULL_BLOCK) * CULL_BLOCK;
            let blocks = Self::cull_blocks(scene);
            let lit = |b: usize| {
                blocks.as_ref().is_none_or(|bl| {
                    let (c, r) = bl[b];
                    lamp_reach(c, r)
                        || boxes.iter().enumerate().any(|(k, &(range, lvp, _))| {
                            let lc = lvp.project_point3(c);
                            let rr = r / range;
                            active[k] && lc.x.abs() <= 1.0 + rr && lc.y.abs() <= 1.0 + rr
                        })
                })
            };
            let mut found: [Vec<DrawItem>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
            for part in run_parts(self.encoding_pool.as_ref(), parts, |p| {
                let mut out: [Vec<DrawItem>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
                let end = ((p + 1) * chunk).min(n);
                let mut b = p * chunk;
                while b < end {
                    let next = (b + CULL_BLOCK).min(end);
                    if lit(b / CULL_BLOCK) {
                        for (a, x) in out.iter_mut().zip(casters(b..next)) {
                            a.extend(x);
                        }
                    }
                    b = next;
                }
                out
            }) {
                for (a, b) in found.iter_mut().zip(part) {
                    a.extend(b);
                }
            }
            for cascade in 0..4 {
                if cascade < 3 && !active[cascade] {
                    continue;
                }
                if debug_draws {
                    log::info!("shadow cascade {cascade}: {} draws", found[cascade].len());
                }
                batch_items(
                    scene,
                    &mut found[cascade],
                    true,
                    &mut list,
                    &mut shadow_batches[cascade],
                );
            }
        }
        if shadows && debug_draws {
            log::info!(
                "shadow passes: {} batches",
                shadow_batches.iter().map(|b| b.len()).sum::<usize>()
            );
        }
        stage(self, "shadow items", "mirror.shadow items");
        // Frustum culling by bounding sphere in view space. OpenXR projections
        // are asymmetric; the desktop field of view must not clip an eye's
        // wider side as the head turns.
        let view = Mat4::look_to_rh(cam_rel, camera.forward(), camera.up());
        let (tan_x, tan_y) = if let Some(p) = projection {
            (
                ((p.z_axis.x - 1.0) / p.x_axis.x)
                    .abs()
                    .max(((p.z_axis.x + 1.0) / p.x_axis.x).abs()),
                ((p.z_axis.y - 1.0) / p.y_axis.y)
                    .abs()
                    .max(((p.z_axis.y + 1.0) / p.y_axis.y).abs()),
            )
        } else {
            let tan_y = (camera.fov_deg.to_radians() * 0.5).tan();
            (tan_y * aspect, tan_y)
        };
        let cos_y = 1.0 / (1.0 + tan_y * tan_y).sqrt();
        let cos_x = 1.0 / (1.0 + tan_x * tan_x).sqrt();
        // nothing behind the fog is drawn: where the fog has swallowed 99 % of a thing
        // there is nothing left to see of it (a storm's 700 m of sight cuts the draws
        // of a city map by two thirds)
        //
        // The enhanced picture's fog is another one: none at all below the weather's
        // 1e-4 (the clear air there is the sky model's, kilometres deep), and above it a
        // layer that thins out with height (`layer_depth` in enhanced_common.wgsl, 300 m
        // scale height over the fog's base). Culled by the vanilla density, whatever stood
        // in thin fog went missing in plain sight - most of all seen from above, with the
        // camera zoomed out.
        let fog_far = if enhanced_frame {
            if lighting.fog_density > 1e-4 {
                let base = lighting
                    .fog_base
                    .or(lighting.inside.map(|v| v.0.z))
                    .unwrap_or(camera.position.z - 2.0);
                let kh = ((camera.position.z - base).max(0.0) / 300.0) as f32;
                // a point on the ground seen from the camera's height: the thinnest fog a
                // line of sight down to the scenery passes through
                let thin = if kh < 1e-3 {
                    1.0
                } else {
                    (1.0 - (-kh).exp()) / kh
                };
                (4.6 / (lighting.fog_density * thin)).min(camera.far)
            } else {
                camera.far
            }
        } else if lighting.fog_density > 1e-7 {
            (4.6 / lighting.fog_density).min(camera.far)
        } else {
            camera.far
        };
        if let Some(p) = omsi_cfg::env::var("OMSI_DEBUG_CULL").ok().and_then(|v| {
            let f: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (f.len() == 3).then(|| (DVec3::new(f[0], f[1], 0.0), f[2]))
        }) {
            for (i, inst) in scene.instances.iter().enumerate() {
                if (inst.origin.truncate() - p.0.truncate()).length() > p.1 || !with_overlays {
                    continue;
                }
                let m = &scene.meshes[inst.mesh];
                let (c, _) = Self::bounding_sphere(scene, inst);
                let v = view.transform_point3(c);
                log::info!("cull {i}: mesh {} r {:.2} centre {:?} origin {:?} view {:?} visible {} lod {:?} tan ({tan_x:.2}, {tan_y:.2}) fog {fog_far:.0} surface {} decal {} casts {} phase {:?} source {:?}", inst.mesh, m.bounds_radius, m.bounds_center, inst.origin, v, inst.visible, inst.lod, inst.surface, inst.decal, inst.casts_shadow, inst.render_phase, m.source);
            }
        }
        let fov_y = camera.fov_deg.to_radians().max(1e-3);
        let max_obj_dist = self.options.max_obj_dist;
        let min_obj_size = lighting.min_obj_size.max(self.options.min_obj_size);
        // (instance, distance along the view direction, the camera is inside its bounds)
        // One thread: a few nanoseconds an instance. Spread over the worker pool the
        // hand-over cost more than the work (5 ms a frame for 8 500 instances while the
        // traffic's scripts kept the workers busy).
        // (the main view's last picture, for the hysteresis; the headset's eyes are main
        // views too: without their previous draw list small meshes and LODs blinked at
        // their thresholds while the head turned)
        let main_view = with_overlays || xr_view;
        let mut drawn_before = if main_view {
            std::mem::take(&mut *self.cull_drawn.borrow_mut())
        } else {
            Vec::new()
        };
        let was_drawn = |i: usize| {
            drawn_before
                .get(i / 64)
                .is_some_and(|w| w & (1u64 << (i % 64)) != 0)
        };
        let (mut sizes_before, mut sizes_now) = if main_view {
            let sizes_before = std::mem::take(&mut *self.object_sizes.borrow_mut());
            let mut sizes_now = std::mem::take(&mut *self.object_sizes_scratch.borrow_mut());
            sizes_now.clear();
            (sizes_before, sizes_now)
        } else {
            Default::default()
        };
        let cull_one = |i: usize, sizes: &mut Vec<([u64; 4], f32)>| -> Option<(usize, f32, bool)> {
            let inst = &scene.instances[i];
            let m = &scene.meshes[inst.mesh];
            if m.ranges.is_empty() || !inst.visible || (inst.mirror_only && main_view) {
                return None;
            }
            if let Some([x0, y0, x1, y1]) = inst.near_only {
                let c = camera.position;
                if c.x < x0 || c.x > x1 || c.y < y0 || c.y > y1 {
                    return None;
                }
            }
            // OMSI's `[isshadow]` shadow blobs, switched off (see `RenderOptions::shadow_blobs`)
            if inst.blob && !self.shadow_blobs {
                return None;
            }
            let (c, r) = Self::bounding_sphere(scene, inst);
            let v = view.transform_point3(c);
            let z = -v.z; // distance along the view direction
                          // camera inside the sphere: drawn whatever the frustum says, but the
                          // object's LOD still chooses (all levels of a building stood in at once)
            let inside = v.length() <= r;
            if !inside && z + r < camera.near {
                return None;
            }
            // (the enhanced sky is not the fog's colour below the horizon: ground
            // left out for the fog let it show through, road-shaped holes in the
            // terrain seen from above, so the ground is always drawn there)
            if !inside && z - r > fog_far && !(enhanced_frame && (inst.surface || r > 100.0)) {
                return None;
            }
            if !inside && (v.x.abs() > z * tan_x + r / cos_x || v.y.abs() > z * tan_y + r / cos_y) {
                return None;
            }
            // The screen size: the diameter over the distance to the camera (not the
            // depth along the view: that is largest in the middle of the picture, so
            // a pole looked at straight on shrank below the limit and vanished while
            // it stayed at the edge), as a share of the vertical field of view -
            // of the whole object when the mesh belongs to one (see
            // `set_object_culling`), else of the mesh alone. Surfaces and terrain
            // are never dropped for it.
            let size = if inst.object_radius > 0.0 {
                let scale = Self::instance_scale(scene, inst);
                let radius = inst.object_radius * scale;
                let ov = view.transform_point3((inst.origin - scene.render_origin).as_vec3());
                let (od, oz) = (ov.length(), -ov.z);
                if od <= radius {
                    f32::MAX
                } else {
                    // performance_maxObjDist, by the distance and by the depth
                    let reach = if was_drawn(i) {
                        max_obj_dist * 1.05
                    } else {
                        max_obj_dist
                    };
                    if !inst.any_distance
                        && max_obj_dist > 0.0
                        && (od > radius + reach || oz - radius > reach)
                    {
                        return None;
                    }
                    // one size for the whole object, held while it moves less than
                    // 6 % (the view's jitter); every mesh and level of it computes
                    // the same key and the same size, so they decide alike
                    let key = [
                        inst.origin.x.to_bits(),
                        inst.origin.y.to_bits(),
                        inst.origin.z.to_bits(),
                        radius.to_bits() as u64,
                    ];
                    let fresh = 2.0 * radius / (od.max(0.01) * fov_y);
                    let size = match sizes_before.get(&key) {
                        Some(&last) if fresh > last * 0.94 && fresh < last * 1.06 => last,
                        _ => fresh,
                    };
                    if main_view {
                        sizes.push((key, size));
                    }
                    size
                }
            } else if m.bounds_radius > 0.0 {
                2.0 * r / (v.length().max(0.01) * fov_y)
            } else {
                f32::MAX
            };
            // `OMSI_DEBUG_CULL`: what near and in the picture is left out, and why
            let near_dbg =
                debug_cull && with_overlays && (inst.origin - camera.position).length() < 150.0;
            // (an object's size is held already; a lone mesh's is not)
            let keep = if was_drawn(i) && inst.object_radius <= 0.0 {
                0.85
            } else {
                1.0
            };
            if !inst.surface && size < min_obj_size * inst.detail * keep {
                if near_dbg {
                    log::info!("cull: instance {i} mesh {} at {:.0} m: size {size:.4} < {:.4} (radius {:.1}, detail {})", inst.mesh, (inst.origin - camera.position).length(), min_obj_size * inst.detail, inst.object_radius, inst.detail);
                }
                return None;
            }
            // (the top level runs to f32::MAX, and a camera inside the object's sphere
            // measures it as f32::MAX: `>=` left out both levels of every object one
            // stood next to - the parked cars, lamps and houses that vanished close by)
            if (inst.lod.0 > 0.0 || inst.lod.1 < f32::MAX)
                && (size < inst.lod.0 || (inst.lod.1 < f32::MAX && size >= inst.lod.1))
            {
                if near_dbg && size < inst.lod.0 {
                    log::info!(
                        "cull: instance {i} mesh {} at {:.0} m: lod {:.4}..{:.4}, size {size:.4}",
                        inst.mesh,
                        (inst.origin - camera.position).length(),
                        inst.lod.0,
                        inst.lod.1
                    );
                }
                return None;
            }
            Some((i, z, inside))
        };
        let n = scene.instances.len();
        let parts = (n / 8192).clamp(
            1,
            self.encoding_pool
                .as_ref()
                .map_or(3, |p| p.current_num_threads())
                + 1,
        );
        let chunk = n.div_ceil(parts).div_ceil(CULL_BLOCK) * CULL_BLOCK;
        let blocks = Self::cull_blocks(scene);
        let block_seen = |b: usize| {
            let Some(bl) = blocks.as_ref() else {
                return true;
            };
            let (c, r) = bl[b];
            let v = view.transform_point3(c);
            let z = -v.z;
            if v.length() <= r {
                return true;
            }
            !(z + r < camera.near
                || (!enhanced_frame && z - r > fog_far)
                || v.x.abs() > z * tan_x + r / cos_x
                || v.y.abs() > z * tan_y + r / cos_y)
        };
        let (mut visible, mut found): (Vec<(usize, f32, bool)>, Vec<([u64; 4], f32)>) =
            (Vec::new(), Vec::new());
        for (v, sizes) in run_parts(self.encoding_pool.as_ref(), parts, |p| {
            let mut sizes = Vec::new();
            let mut v = Vec::new();
            let end = ((p + 1) * chunk).min(n);
            let mut b = p * chunk;
            while b < end {
                let next = (b + CULL_BLOCK).min(end);
                if block_seen(b / CULL_BLOCK) {
                    v.extend((b..next).filter_map(|i| cull_one(i, &mut sizes)));
                }
                b = next;
            }
            (v, sizes)
        }) {
            visible.extend(v);
            found.extend(sizes);
        }
        if main_view {
            sizes_now.extend(found);
            *self.object_sizes.borrow_mut() = sizes_now;
            sizes_before.clear();
            *self.object_sizes_scratch.borrow_mut() = sizes_before;
        }
        if main_view {
            drawn_before.resize(scene.instances.len().div_ceil(64), 0);
            drawn_before.fill(0);
            for &(i, _, _) in &visible {
                drawn_before[i / 64] |= 1u64 << (i % 64);
            }
            *self.cull_drawn.borrow_mut() = drawn_before;
        }
        // OMSI_DEBUG_FLICKER: a near instance in view in two frames running that is drawn in
        // one and not in the other - the objects blinking in and out as the view moves
        if with_overlays && omsi_cfg::env::var_os("OMSI_DEBUG_FLICKER").is_some() {
            let drawn: std::collections::HashSet<usize> = visible.iter().map(|v| v.0).collect();
            let mut prev = self.flicker.borrow_mut();
            let mut now: HashMap<usize, bool> = HashMap::new();
            for (i, inst) in scene.instances.iter().enumerate() {
                let m = &scene.meshes[inst.mesh];
                if m.ranges.is_empty() || !inst.visible || inst.surface {
                    continue;
                }
                let (c, r) = Self::bounding_sphere(scene, inst);
                let v = view.transform_point3(c);
                let z = -v.z;
                if v.length() > 150.0
                    || z + r < camera.near
                    || v.x.abs() > z * tan_x + r / cos_x
                    || v.y.abs() > z * tan_y + r / cos_y
                {
                    continue;
                }
                let d = drawn.contains(&i);
                if let Some(&was) = prev.get(&i) {
                    if was != d {
                        let od = (inst.origin - camera.position).length();
                        log::info!("flicker: instance {i} mesh {} {} at {od:.1} m (view z {z:.1}, r {r:.2}, object r {:.2}, detail {}, lod {:.3}..{:.3})", inst.mesh, if d { "appears" } else { "vanishes" }, inst.object_radius, inst.detail, inst.lod.0, inst.lod.1);
                    }
                }
                now.insert(i, d);
            }
            *prev = now;
        }
        stage(self, "cull", "mirror.cull");
        let only_surfaces = omsi_cfg::env::var_os("OMSI_ONLY_SURFACES").is_some();
        let visible: Vec<(usize, f32, bool)> = if only_surfaces {
            visible
                .into_iter()
                .filter(|(i, _, _)| scene.instances[*i].surface)
                .collect()
        } else {
            visible
        };
        if debug_draws {
            let surf = scene.instances.iter().filter(|i| i.surface).count();
            let vis_surf = visible
                .iter()
                .filter(|(i, _, _)| scene.instances[*i].surface)
                .count();
            log::info!(
                "draw: {} instances ({} surface), {} visible ({} surface)",
                scene.instances.len(),
                surf,
                visible.len(),
                vis_surf
            );
        }
        // the depth prepass: opaque and alpha-tested, single-sampled
        let mut prepass_batches: Vec<Batch> = Vec::new();
        let prepass_job = || -> (Vec<u32>, Vec<Batch>) {
            let mut items: Vec<DrawItem> = Vec::new();
            let mut list: Vec<u32> = Vec::new();
            let mut batches: Vec<Batch> = Vec::new();
            for &(i, _, _) in &visible {
                let inst = &scene.instances[i];
                let cull = culls_back_faces(scene, inst);
                for (ri, (_, _, slot)) in scene.meshes[inst.mesh].ranges.iter().enumerate() {
                    let mat_id = inst.materials.get(*slot as usize).copied().unwrap_or(0);
                    let mat = &scene.materials[mat_id];
                    let kind = kind_of(mat.alpha);
                    // Ground blends compose before they may occlude later scenery.
                    // The transmap body prepass is for ordinary meshes, not these
                    // authored surface layers (including their terrain brush masks).
                    if kind == PIPE_BLEND && world_surface_phase(effective_render_phase(inst)) {
                        continue;
                    }
                    if let Some(pre_kind) = depth_prepass_kind(kind, mat, inst.presurface) {
                        // plain/alpha-tested materials use their ordinary depth pass;
                        // blended transmaps use the opaque-pixels-only pass.
                        let (material, look) = depth_only_material(pre_kind, mat_id, mat.look);
                        items.push(DrawItem {
                            pipe: pre_kind * 2 + cull as u8,
                            mesh: inst.mesh as u32,
                            range: ri as u32,
                            material,
                            look,
                            entry: inst.base + *slot,
                        });
                    }
                }
            }
            batch_items(scene, &mut items, true, &mut list, &mut batches);
            (list, batches)
        };
        // The main pass follows the authored world phases. Each phase keeps its
        // opaque/cutout draws followed by its blended draws, far to near. Transparent
        // ground layers never write depth while composing: an alpha-zero junction
        // texel otherwise blocks a later opaque grass spline and exposes the sky
        // wherever that spline's prepass already rejected the terrain underneath.
        let mut main_batches: Vec<Batch> = Vec::new();
        // the blended (and, drawn in model order, all) slots of the vehicle the camera is in
        let mut cab_batches: Vec<Batch> = Vec::new();
        let mut cab_items: Vec<DrawItem> = Vec::new();
        let mut main_draws = [0usize; 2];
        // Keep mesh/material order here: an excavation's floor is drawn before its
        // invisible cover writes depth. Sorting its blended cover after the terrain
        // leaves the terrain's colour in place even though the cover writes depth.
        let has_presurface = visible
            .iter()
            .any(|&(i, _, _)| scene.instances[i].presurface);
        let mut prepass_found: Option<(Vec<u32>, Vec<Batch>)> = None;
        let pool = self.encoding_pool.as_ref();
        in_scope(pool, |scope| {
            if prepass_on {
                let job = &prepass_job;
                let slot = &mut prepass_found;
                scope.spawn(move |_| *slot = Some(job()));
            }
            let mut by_phase: [Vec<(usize, f32, bool)>; RenderPhase::COUNT] =
                std::array::from_fn(|_| Vec::new());
            for &entry in &visible {
                let phase = effective_render_phase(&scene.instances[entry.0]);
                by_phase[phase as usize].push(entry);
            }
            // OMSI draws each authored world phase as its own opaque/cutout pass followed
            // by that phase's blended draws. Keeping the phase boundary here lets later
            // surface markings compose over spline blends without changing the depth test.
            for phase in RenderPhase::DRAW_ORDER {
                // Finish surface composition before committing the fully covered ground
                // pixels to depth. Doing this in the global prepass (or while blending
                // each spline) would reject authored road overlaps and on-surface rails.
                // Later scenery must still be occluded by the solid part of the road:
                // otherwise cutout bushes below a bridge repaint its asphalt.
                if phase == RenderPhase::BeforeNormal {
                    items.clear();
                    for &(i, _, _) in by_phase[..RenderPhase::BeforeNormal as usize]
                        .iter()
                        .flatten()
                    {
                        let inst = &scene.instances[i];
                        for (ri, (_, _, slot)) in scene.meshes[inst.mesh].ranges.iter().enumerate()
                        {
                            let mat_id = inst.materials.get(*slot as usize).copied().unwrap_or(0);
                            let mat = &scene.materials[mat_id];
                            if !surface_depth_coverage(
                                effective_render_phase(inst),
                                mat.alpha,
                                mat.transmap.is_some(),
                                mat.no_z_check,
                            ) || exclude_texture.is_some_and(|t| mat.uses_texture(t))
                            {
                                continue;
                            }
                            items.push(DrawItem {
                                pipe: pipe_code(
                                    PIPE_SURFACE_DEPTH,
                                    culls_back_faces(scene, inst),
                                    instance_depth_bias(inst, mat),
                                ),
                                mesh: inst.mesh as u32,
                                range: ri as u32,
                                material: mat_id as u32,
                                look: mat.look,
                                entry: inst.base + *slot,
                            });
                        }
                    }
                    batch_items(scene, &mut items, true, &mut list, &mut main_batches);
                }
                let visible = &by_phase[phase as usize];
                items.clear();
                let mut blended: Vec<usize> = Vec::new();
                for &(i, _, _) in visible {
                    let inst = &scene.instances[i];
                    if inst.ordered {
                        blended.push(i);
                        continue;
                    }
                    let mut has_blend = false;
                    let cull = culls_back_faces(scene, inst);
                    for (ri, (_, _, slot)) in scene.meshes[inst.mesh].ranges.iter().enumerate() {
                        let mat_id = inst.materials.get(*slot as usize).copied().unwrap_or(0);
                        let mat = &scene.materials[mat_id];
                        let kind = kind_of(mat.alpha);
                        if kind == PIPE_BLEND || mat.no_z_check {
                            has_blend = true;
                            continue;
                        }
                        // a render target cannot be sampled while being drawn into (mirror glass, or a reflection map of it)
                        if exclude_texture.is_some_and(|t| mat.uses_texture(t)) {
                            continue;
                        }
                        items.push(DrawItem {
                            pipe: pipe_code(kind, cull, instance_depth_bias(inst, mat)),
                            mesh: inst.mesh as u32,
                            range: ri as u32,
                            material: mat_id as u32,
                            look: mat.look,
                            entry: inst.base + *slot,
                        });
                    }
                    if has_blend {
                        blended.push(i);
                    }
                }
                main_draws[0] += items.len();
                batch_items(scene, &mut items, true, &mut list, &mut main_batches);
                // Blended draws: objects far to near by the distance of their nearest blended
                // mesh (see `near_by_origin` below - not the single local origin all of an
                // object's meshes share), and within an object in creation order - the
                // model.cfg mesh order, which is what the original relies on (windows are
                // listed last).
                //
                // An object the camera is inside of (the bus seen from the driver's
                // seat) comes after everything outside it, and the player's own vehicle
                // last of all. By its origin alone the bus - whose origin is 4.6 m
                // behind the driver's eye on the NL202 - sorted as farther away than a
                // car right beside the driver's window, so the car was drawn after the
                // bus's window layers (rain film, dirt, door glass), which write depth:
                // its blended body failed the depth test and only the opaque wheels
                // were left, dark behind the tinted glass, exactly while the car was
                // half out of the picture.
                let mut holders: Vec<DVec3> = Vec::new();
                for &(i, _, inside) in visible {
                    let inst = &scene.instances[i];
                    if inside && !inst.surface && !holders.contains(&inst.origin) {
                        holders.push(inst.origin);
                    }
                }
                let player = lighting
                    .inside
                    .filter(|v| point_in_vehicle_box(camera.position, v))
                    .map(|v| v.0);
                // An object's *nearest* blended mesh to the camera, not the single point its
                // meshes all share (`inst.origin`): a long vehicle's own origin can sit well
                // behind (or ahead of) its nearest window, so ranking the whole object by that
                // one point against a much smaller nearby object - a car passing level with the
                // middle of a stopped bus - picked the wrong order even outside the "camera is
                // inside" case above (the bus's origin, metres behind the window nearest the
                // car, sorted as farther away than the car itself, so the car was drawn last and
                // painted over the window instead of being hidden behind the body between the
                // windows). Every blended mesh of the object is a candidate; the closest one's
                // distance, less its own bounding radius, stands for the whole object.
                //
                // Scope, checked systematically while chasing a report of a car showing through
                // a stopped bus's body from outside (never reproduced, before or after this
                // commit): this order only ever decides how mutually-*blended* draws composite
                // where they overlap on screen (a car's own window glass in front of a bus's
                // window + interior, say) - it cannot be why an opaque wall would fail to hide
                // something behind it. Every pipeline the main pass uses, opaque or blended,
                // keeps depth *testing* on (`GreaterEqual`, see the pipeline table above); only
                // depth *writing* differs. Opaque batches are always recorded before blended ones
                // in the same pass (`main_draws[0]` first), so by the time any blended draw runs,
                // the depth buffer already holds every opaque surface in front of it, blend order
                // or not. Dumping the EN92's and the O530 Facelift's per-material alpha mode
                // (`OMSI_ONLY_MESH`) found every body panel `AlphaMode::Opaque`, as OMSI requires
                // (diffuse alpha is a reflection mask, not transparency, unless `[matl_alpha]` 1
                // or 2 says otherwise); an A/B render (this commit vs its parent, same seed, a
                // parked car centred behind a stopped EN92's midsection) came back pixel-identical
                // at the car/bus silhouette - the only measured difference was in the bus's own
                // overlapping window/dirt/interior layers, which is exactly this sort's stated
                // job. If the reported artefact is real, its cause is still open and elsewhere.
                let near_by_origin = if self.blend_by_origin {
                    HashMap::new()
                } else {
                    nearest_by_origin(blended.iter().filter_map(|&i| {
                        let inst = &scene.instances[i];
                        if inst.surface {
                            return None;
                        }
                        let (c, r) = Self::bounding_sphere(scene, inst);
                        Some((inst.origin, (c - cam_rel).length() - r))
                    }))
                };
                let mut keyed: Vec<(u8, f32, usize)> = blended
                    .iter()
                    .map(|&i| {
                        let inst = &scene.instances[i];
                        // Surfaces are ground and go by distance alone: a tile's painted ground
                        // shares its origin with the terrain the camera is always inside of, and
                        // ranked with it, it was drawn after everything blended near it - over the
                        // shadow blobs of the buses standing on it. A blob belongs to the ground
                        // under its vehicle too, drawn before the vehicle's glass.
                        let rank = if self.blend_by_origin || inst.surface {
                            0
                        } else if player == Some(inst.origin) {
                            2
                        } else if holders.contains(&inst.origin) {
                            1
                        } else {
                            0
                        };
                        // A vehicle's shadow blob goes after all the ground: it writes no depth, so
                        // every road piece nearer than its origin, drawn after it by distance,
                        // painted the road back over it (OMSI draws it over the road it lies on).
                        let dist = if inst.blob {
                            -1.0
                        } else if let Some(sort_origin) = inst.blend_sort_origin {
                            // Spline surfaces use the C++ handler's placement-origin distance
                            // in the horizontal plane (Rust's world axes are x/y horizontal,
                            // z vertical).
                            horizontal_sort_distance(sort_origin, ro, cam_rel)
                        } else if self.blend_by_origin || inst.surface {
                            ((inst.origin - ro).as_vec3() - cam_rel).length()
                        } else {
                            near_by_origin
                                .get(&origin_key(inst.origin))
                                .copied()
                                .unwrap_or(0.0)
                        };
                        (rank, dist, i)
                    })
                    .collect();
                // (a total order even where a distance is NaN - an instance at a NaN position:
                // partial_cmp's "equal" for it broke the sort's order, and since Rust 1.81 the
                // sort panics on that, which ended the game)
                keyed.sort_unstable_by(|a, b| {
                    a.0.cmp(&b.0).then(b.1.total_cmp(&a.1)).then(a.2.cmp(&b.2))
                });
                items.clear();
                for (rank, _, i) in keyed {
                    let inst = &scene.instances[i];
                    let cull = culls_back_faces(scene, inst);
                    for (ri, (_, _, slot)) in scene.meshes[inst.mesh].ranges.iter().enumerate() {
                        let mat_id = inst.materials.get(*slot as usize).copied().unwrap_or(0);
                        let mat = &scene.materials[mat_id];
                        if (mat.alpha != AlphaMode::Blend && !mat.no_z_check && !inst.ordered)
                            || exclude_texture.is_some_and(|t| mat.uses_texture(t))
                        {
                            continue;
                        }
                        // A layer its script has faded out (`[alphascale]` at 0: the rain film
                        // on a dry day, the dirt on a clean bus) shows nothing, as in the
                        // original, whose blend takes it out whole; drawn anyway it ran the full
                        // shading over the whole windscreen for nothing - a bus's cab view had
                        // three or four such screen-sized layers.
                        if mat.alpha == AlphaMode::Blend
                            && inst
                                .slot_alpha
                                .get(*slot as usize)
                                .is_some_and(|a| *a < 1.0 / 512.0)
                        {
                            continue;
                        }
                        // Ground blends use the C++ handler's no-write composition;
                        // their opaque coverage is committed after OnSurface. Other
                        // materials retain [matl_noZwrite] (glass, rain, dirt) semantics.
                        // [matl_noZcheck] marks a decal that must win over the surface it lies
                        // on (a bus's shadow blob, the digits on a counter): the original draws
                        // it without a depth test right after that surface, in model order. With
                        // everything opaque drawn first here, no test at all would put it over
                        // the whole bus (the steering wheel in front of the counter, the body over
                        // the shadow), so it is drawn with the surfaces' depth bias instead: on
                        // top of its base, behind whatever really stands in front of it.
                        let kind = if mat.alpha != AlphaMode::Blend && !mat.no_z_check {
                            // (a model drawn in order: its opaque and cut-out slots too)
                            kind_of(mat.alpha)
                        } else if (mat.no_z_write && !mat.writes_depth)
                            || mat.no_z_check
                            || (world_surface_phase(inst.render_phase) && !inst.presurface)
                        {
                            PIPE_BLEND_NO_WRITE
                        } else {
                            PIPE_BLEND
                        };
                        // Only tile painting gets the zero-coverage fast path. Other
                        // no-write blends (splines, glass, decals) keep their shader.
                        let kind = if kind == PIPE_BLEND_NO_WRITE
                            && mat.alpha == AlphaMode::Blend
                            && inst.ground_layer
                            && !inst.presurface
                            && mat.uniform.extra[0] > 0.5
                            && mat.transmap.is_some_and(|(_, alpha)| alpha)
                        {
                            PIPE_TERRAIN_PAINT
                        } else {
                            kind
                        };
                        let item = DrawItem {
                            pipe: pipe_code(kind, cull, instance_depth_bias(inst, mat)),
                            mesh: inst.mesh as u32,
                            range: ri as u32,
                            material: mat_id as u32,
                            look: mat.look,
                            entry: inst.base + *slot,
                        };
                        // the vehicle the camera is in is drawn after everything else
                        if rank == 2 {
                            cab_items.push(item);
                        } else {
                            items.push(item);
                        }
                    }
                }
                main_draws[1] += items.len();
                batch_items(scene, &mut items, false, &mut list, &mut main_batches);
            }
            // Omsi.exe draws the vehicle the camera sits in last of all, with the view mask
            // of its inside (0x6f1520 -> 0x6f0430), after every phase of the map, the other
            // vehicles, the particles and the lamps' flares (0x6f0400/0x6f0418): its glass,
            // whose depth is written unless the model says `[matl_noZwrite]`, then lies over
            // all of that. Its items wait here and are drawn after the coronas and the smoke
            // (see `cab_batches` in the main pass).
            main_draws[1] += cab_items.len();
            batch_items(scene, &mut cab_items, false, &mut list, &mut cab_batches);
        });
        if let Some((pre_list, mut pre_batches)) = prepass_found {
            let offset = list.len() as u32;
            for b in &mut pre_batches {
                b.instances = b.instances.start + offset..b.instances.end + offset;
            }
            list.extend(pre_list);
            prepass_batches = pre_batches;
        }
        // OMSI_SKIP_PIPE=3,1: leave pipeline kinds out of the main pass (0 opaque, 1 alpha
        // tested, 2 blended, 3 blended without depth writes, 4 surface depth, 5 terrain
        // paint; 3 includes its specialized terrain variant) - with
        // OMSI_GPU_TIMERS_RAW, what each kind costs the GPU
        if let Ok(skip) = omsi_cfg::env::var("OMSI_SKIP_PIPE") {
            let skip: Vec<u8> = skip
                .split(',')
                .filter_map(|x| x.trim().parse().ok())
                .collect();
            let include = |b: &Batch| {
                let kind = b.pipe / 4;
                !skip.contains(&kind)
                    && !(kind == PIPE_TERRAIN_PAINT && skip.contains(&PIPE_BLEND_NO_WRITE))
            };
            main_batches.retain(include);
            cab_batches.retain(include);
        }
        if debug_draws {
            log::info!("  main pass: {} opaque/alpha-tested and {} blended draws in {} batches; prepass {} batches; draw list {} entries", main_draws[0], main_draws[1], main_batches.len(), prepass_batches.len(), list.len());
        }
        if self.profiling && !with_overlays {
            let mut c = self.counts.borrow_mut();
            *c.entry("mirror pictures").or_default() += 1.0;
            *c.entry("mirror visible instances").or_default() += visible.len() as f64;
        }
        if self.profiling && with_overlays {
            let mut c = self.counts.borrow_mut();
            *c.entry("scene instances").or_default() += scene.instances.len() as f64;
            *c.entry("visible instances").or_default() += visible.len() as f64;
            *c.entry("bounds users").or_default() += scene.bounds_users.len() as f64;
            *c.entry("main draws").or_default() += (main_draws[0] + main_draws[1]) as f64;
            *c.entry("opaque draws").or_default() += main_draws[0] as f64;
            *c.entry("blended draws").or_default() += main_draws[1] as f64;
            *c.entry("main batches").or_default() += main_batches.len() as f64;
            *c.entry("prepass batches").or_default() += prepass_batches.len() as f64;
            *c.entry("shadow batches").or_default() +=
                (shadow_batches[0].len() + shadow_batches[1].len()) as f64;
            // triangles each pass draws (thousands), the geometry the GPU goes through
            let tris = |bs: &[Batch]| {
                bs.iter()
                    .map(|b| b.count as f64 / 3.0 * b.instances.len() as f64)
                    .sum::<f64>()
                    / 1000.0
            };
            *c.entry("ktris main").or_default() += tris(&main_batches);
            *c.entry("ktris prepass").or_default() += tris(&prepass_batches);
            *c.entry("ktris shadow near").or_default() += tris(&shadow_batches[0]);
            *c.entry("ktris shadow far").or_default() += tris(&shadow_batches[1]);
            *c.entry("ktris shadow close").or_default() += tris(&shadow_batches[2]);
        }
        if self.profiling && with_overlays && self.draw_audit_at.elapsed().as_secs() >= 10 {
            self.draw_audit_at = std::time::Instant::now();
            // (batches, draws, triangles) per asset: what the CPU encodes and what the GPU
            // goes through
            let mut assets: HashMap<&str, (usize, usize, u64)> = HashMap::new();
            for b in &main_batches {
                let source = scene.meshes[b.mesh as usize]
                    .source
                    .as_deref()
                    .unwrap_or("procedural / vehicle");
                let cost = assets.entry(source).or_default();
                cost.0 += 1;
                cost.1 += b.instances.len();
                cost.2 += b.count as u64 / 3 * b.instances.len() as u64;
            }
            let mut assets: Vec<_> = assets.into_iter().collect();
            assets.sort_unstable_by(|a, b| b.1 .0.cmp(&a.1 .0).then(a.0.cmp(b.0)));
            for (source, (batches, draws, tris)) in assets.iter().take(12) {
                log::info!(
                    "draw audit: {batches} batches, {draws} draws, {tris} triangles: {source}"
                );
            }
            assets.sort_unstable_by(|a, b| b.1 .2.cmp(&a.1 .2).then(a.0.cmp(b.0)));
            for (source, (batches, draws, tris)) in assets.iter().take(12) {
                log::info!("triangle audit: {tris} triangles in {draws} draws ({batches} batches): {source}");
            }
        }
        stage(self, "items", "mirror.items");
        let mut rain_batches = Vec::new();
        if glass_on {
            let (rain, main): (Vec<_>, Vec<_>) = main_batches
                .into_iter()
                .partition(|b| scene.materials[b.material as usize].uniform.emissive[3] > 1.5);
            rain_batches = rain;
            main_batches = main;
        }
        self.upload_draw_list(scene, &list);
        stage(self, "upload", "mirror.upload");
        // OMSI_NO_BUNDLES=1 records the main pass directly, for comparison. (Splitting the
        // pass in two to finish the halves side by side was tried as well: the second half
        // has to load the first one's targets back into the GPU's tile memory, which cost
        // more GPU time than it saved on the CPU.)
        let main_bundles = if omsi_cfg::env::var_os("OMSI_NO_BUNDLES").is_none() {
            let pp = self.main_pass(enhanced, reflection_frame);
            let format = if masked_frame {
                HDR_FORMAT
            } else {
                self.format
            };
            record_bundles(
                &self.device,
                self.encoding_pool.as_ref(),
                scene,
                &main_batches,
                pp,
                scene.camera_bind_group.as_ref().expect("camera bind group"),
                format,
                self.options.msaa,
            )
        } else {
            Vec::new()
        };
        stage(self, "bundles", "mirror.bundles");
        // three command buffers, finished side by side (see below): the shadow maps, the
        // depth prepass with the ambient occlusion, and the picture itself
        let mut shadow_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("shadow maps"),
                });
        self.flush_pending_meshes(scene, &mut shadow_encoder);
        let mut prepass_encoder =
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("depth prepass"),
                });
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("picture"),
            });
        for cascade in [0usize, 1] {
            if !draw_shadows || (cascade == 1 && !redraw_far) {
                continue;
            }
            let view = if cascade == 0 {
                &self.shadow_view
            } else {
                &self.shadow_view_far
            };
            let keep_near = cascade == 0 && !redraw_near;
            let mut pass = shadow_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("shadow"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view,
                    depth_ops: Some(wgpu::Operations {
                        load: if keep_near {
                            wgpu::LoadOp::Load
                        } else {
                            wgpu::LoadOp::Clear(1.0)
                        },
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pass_timer(
                    tset.as_ref(),
                    &mut timed,
                    if cascade == 0 {
                        "shadow near"
                    } else {
                        "shadow far"
                    },
                ),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, scene.shadow_bind_group.as_ref().unwrap(), &[]);
            if cascade == 0 {
                // the atlas: near cascade on the left half, close cascade on the right
                let sz = self.options.shadow_size as f32;
                pass.set_viewport(0.0, 0.0, sz, sz, 0.0, 1.0);
                encode_batches(&mut pass, scene, &shadow_batches[0], |pipe| {
                    &self.shadow_pipelines[pipe as usize]
                });
                let csz = self.options.shadow_size.min(SHADOW_CLOSE_MAX) as f32;
                pass.set_viewport(sz, 0.0, csz, csz, 0.0, 1.0);
                if keep_near {
                    // (the near half is last frame's: only the close part is cleared)
                    pass.set_pipeline(&self.shadow_clear_pipeline);
                    pass.draw(0..3, 0..1);
                    pass.set_bind_group(0, scene.shadow_bind_group.as_ref().unwrap(), &[]);
                }
                encode_batches(&mut pass, scene, &shadow_batches[2], |pipe| {
                    &self.shadow_pipelines[4 + pipe as usize]
                });
            } else {
                let sz = self.options.shadow_size as f32;
                pass.set_viewport(0.0, 0.0, sz, sz, 0.0, 1.0);
                encode_batches(&mut pass, scene, &shadow_batches[cascade], |pipe| {
                    &self.shadow_pipelines[cascade * 2 + pipe as usize]
                });
            }
        }
        // the street lamps' maps: tiles of a quarter of the shadow size under the far map,
        // each cleared and drawn every frame
        if !lamp_shadows.is_empty() {
            let mut pass = shadow_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("lamp shadows"),
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.shadow_view_far,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pass_timer(tset.as_ref(), &mut timed, "lamp shadows"),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            let sz = self.options.shadow_size as f32;
            let tile = sz / LAMP_SHADOWS as f32;
            for k in 0..lamp_shadows.len() {
                pass.set_viewport(k as f32 * tile, sz, tile, tile, 0.0, 1.0);
                pass.set_pipeline(&self.shadow_clear_pipeline);
                pass.draw(0..3, 0..1);
                pass.set_bind_group(0, scene.shadow_bind_group.as_ref().unwrap(), &[]);
                encode_batches(&mut pass, scene, &shadow_batches[3], |pipe| {
                    &self.shadow_pipelines[6 + 2 * k + pipe as usize]
                });
            }
        }
        // --- depth prepass + ambient occlusion (single-sampled, camera projection)
        if prepass_on {
            let proj = projection.unwrap_or_else(|| {
                Mat4::perspective_rh(camera.fov_deg.to_radians(), aspect, camera.far, camera.near)
            });
            let u = SsaoUniform {
                inv_proj: proj.inverse().to_cols_array_2d(),
                params: [
                    1.0,
                    1.4,
                    width.div_ceil(2) as f32,
                    height.div_ceil(2) as f32,
                ],
                shift: [proj.z_axis.x, proj.z_axis.y, 0.0, 0.0],
            };
            self.queue
                .write_buffer(&self.ao_buf, 0, bytemuck::bytes_of(&u));
            let ao = self.ao.as_ref().unwrap();
            {
                let mut pass = prepass_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("depth prepass"),
                    color_attachments: &[],
                    depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                        view: &ao.depth_view,
                        depth_ops: Some(wgpu::Operations {
                            load: wgpu::LoadOp::Clear(0.0),
                            store: wgpu::StoreOp::Store,
                        }),
                        stencil_ops: None,
                    }),
                    timestamp_writes: pass_timer(tset.as_ref(), &mut timed, "depth prepass"),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                encode_batches(&mut pass, scene, &prepass_batches, |pipe| {
                    &self.prepass_pipelines[pipe as usize]
                });
            }
            for (pipe, bg, target, pass_label) in [
                (&self.ssao_pipeline, &ao.ssao_bg, &ao.ao_view, "ssao"),
                (&self.blur_pipeline, &ao.blur_bg, &ao.blur_view, "ssao blur"),
            ] {
                let Some(pipe) = pipe.as_ref().filter(|_| ao_on) else {
                    break;
                };
                let mut pass = prepass_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("ssao"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: target,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: pass_timer(tset.as_ref(), &mut timed, pass_label),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, bg, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        if rt_frame {
            self.encode_ray_tracing(&mut prepass_encoder, scene, tset.as_ref(), &mut timed);
        }
        // --- the enhanced sky cube: a face a frame (all six the first time and for a new
        // sky), drawn in the window's frame only
        if enhanced && (lead_view || probe_redraw) {
            if let (Some(probe), Some(sky_bg)) =
                (self.probe.as_mut(), scene.sky_bind_group.as_ref())
            {
                // (face, round, the old picture's share): a whole new cube is every round
                // of every face averaged; afterwards one face a frame, blended in
                let full = !probe.cube_filled || self.instant_exposure;
                probe.cube_wait += 1;
                let recapture = std::mem::take(&mut probe.cube_recapture);
                let draws: Vec<(u32, u32, f64)> = if full {
                    (0..6)
                        .flat_map(|f| {
                            (0..SKY_CUBE_ROUNDS).map(move |r| (f, r, r as f64 / (r as f64 + 1.0)))
                        })
                        .collect()
                } else if recapture {
                    // the eye moved on: every face from the new one, nothing kept from the
                    // old place
                    let round = (probe.cube_round / 6) % SKY_CUBE_ROUNDS;
                    (0..6).map(|f| (f, round, 0.0)).collect()
                } else if (probe.cube_wait >= SKY_CUBE_EVERY && !redraw_near)
                    || probe.cube_wait >= SKY_CUBE_EVERY * 2
                    || !lead_view
                {
                    // (on a frame that keeps the near shadow map: the two costliest
                    // occasional passes never fall on the same frame)
                    vec![(
                        probe.cube_next,
                        (probe.cube_round / 6) % SKY_CUBE_ROUNDS,
                        SKY_CUBE_HISTORY,
                    )]
                } else {
                    Vec::new()
                };
                let single = draws.len() == 1;
                if !draws.is_empty() {
                    probe.cube_wait = 0;
                    probe.cube_next = (probe.cube_next + 1) % 6;
                    probe.cube_round = probe.cube_round.wrapping_add(1);
                }
                probe.cube_filled = true;
                for (f, round, history) in draws {
                    let mut pass = prepass_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sky cube"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: &probe.cube_faces[f as usize],
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: if history > 0.0 {
                                    wgpu::LoadOp::Load
                                } else {
                                    wgpu::LoadOp::Clear(wgpu::Color::BLACK)
                                },
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: if single {
                            pass_timer(tset.as_ref(), &mut timed, "sky cube")
                        } else {
                            None
                        },
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_pipeline(&probe.cube_pipeline);
                    pass.set_blend_constant(wgpu::Color {
                        r: history,
                        g: history,
                        b: history,
                        a: history,
                    });
                    pass.set_bind_group(
                        0,
                        &probe.cube_bind_groups[(f * SKY_CUBE_ROUNDS + round) as usize],
                        &[],
                    );
                    pass.set_bind_group(1, sky_bg, &[]);
                    pass.draw(0..3, 0..1);
                }
            }
        }
        // --- the reflection probe of the enhanced path: the sky into the six faces, then
        // each blurrier level from the sharper ones (three faces a pass)
        if probe_redraw {
            if let (Some(probe), Some(sky_bg)) =
                (self.probe.as_ref(), scene.sky_bind_group.as_ref())
            {
                for (m, faces) in probe.faces.iter().enumerate() {
                    for (half, bg) in probe.bind_groups[m].iter().enumerate() {
                        let attachments: Vec<Option<wgpu::RenderPassColorAttachment>> = faces
                            [half * 3..half * 3 + 3]
                            .iter()
                            .map(|v| {
                                Some(wgpu::RenderPassColorAttachment {
                                    view: v,
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })
                            })
                            .collect();
                        let mut pass =
                            prepass_encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                                label: Some("reflection probe"),
                                color_attachments: &attachments,
                                depth_stencil_attachment: None,
                                timestamp_writes: if m == 0 && half == 0 {
                                    pass_timer(tset.as_ref(), &mut timed, "probe")
                                } else {
                                    None
                                },
                                occlusion_query_set: None,
                                multiview_mask: None,
                            });
                        pass.set_pipeline(if m == 0 {
                            &probe.sky_pipeline
                        } else {
                            &probe.filter_pipeline
                        });
                        pass.set_bind_group(0, bg, &[]);
                        pass.set_bind_group(1, sky_bg, &[]);
                        pass.draw(0..3, 0..1);
                    }
                }
            }
        }
        // Without multisampling the main pass tests against the depth the prepass left
        // (when there was one): the costly shading - lighting, the shadow filter - is then
        // done once per pixel for the surface that is seen, not for every tree and wall
        // hidden behind it.
        let single = self.options.msaa <= 1;
        // A presurface must colour its below-ground faces before its invisible cover
        // seals them. Reusing prepass depth would reject those faces (or let terrain
        // reject them first). The prepass still supplies AO; colour rebuilds its depth.
        // A depth-writing window in the colour pass must not replace the road receiver.
        let share_depth =
            prepass_on && single && self.ao.is_some() && !has_presurface && !puddles_wanted;
        let targets = if share_depth {
            None
        } else {
            Some(self.msaa_targets(width, height))
        };
        // With multisampling the prepass above (single-sampled, for the ambient occlusion)
        // cannot be the main pass's depth: the enhanced picture lays its depth again into
        // the multisampled buffer first. Without it every wall, tree and car hidden behind
        // the one in front ran the whole enhanced shading (11 ms of a 1080p frame in
        // central Spandau with 4x MSAA).
        // Apple's hidden-surface removal already handles ordinary opaque draws.
        // Preserve that fast path. In the measured mixed views, prefilling MSAA
        // depth before cutouts/blends saved more hidden shading than the pass cost.
        let needs_msaa_prepass = !cfg!(target_vendor = "apple")
            || main_batches.iter().chain(&cab_batches).any(|batch| {
                matches!(
                    batch.pipe / 4,
                    PIPE_ALPHA_TEST | PIPE_BLEND | PIPE_BLEND_NO_WRITE
                )
            });
        let msaa_prepass = enhanced
            && !has_presurface
            && (with_overlays || xr_view)
            && !single
            && prepass_on
            && needs_msaa_prepass
            && omsi_cfg::env::var_os("OMSI_NO_MSAA_PREPASS").is_none();
        let parts = if !cfg!(any(target_os = "macos", target_os = "ios"))
            && main_bundles.len() >= 2
            && omsi_cfg::env::var_os("OMSI_NO_MAIN_SPLIT").is_none()
        {
            main_bundles.len().min(2)
        } else {
            1
        };
        let mut lead = (parts > 1).then(|| {
            self.device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("main part"),
                })
        });
        if msaa_prepass {
            if let (Some(pipes), Some(t)) = (self.prepass_msaa_pipelines.as_ref(), targets.as_ref())
            {
                let mut pass = lead.as_mut().unwrap_or(&mut encoder).begin_render_pass(
                    &wgpu::RenderPassDescriptor {
                        label: Some("msaa depth prepass"),
                        color_attachments: &[],
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: &t.1,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(0.0),
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: pass_timer(tset.as_ref(), &mut timed, "msaa prepass"),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    },
                );
                pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                // Alpha-tested colour draws use alpha-to-coverage, but the depth-only
                // prepass uses a binary 0.5 cutoff. Letting those meshes write depth here
                // can hide the opaque geometry behind samples the colour pass leaves
                // uncovered (the sky then shows through buildings/terrain behind foliage).
                // The main alpha-tested pass writes matching depth as it draws the colour.
                encode_batches_filtered(
                    &mut pass,
                    scene,
                    &prepass_batches,
                    |batch| batch.pipe / 2 != PIPE_ALPHA_TEST,
                    |pipe| &pipes[pipe as usize],
                );
            }
        }
        let msaa_prepass =
            msaa_prepass && self.prepass_msaa_pipelines.is_some() && targets.is_some();
        let mut main_parts: Vec<wgpu::CommandEncoder> = Vec::new();
        {
            // The enhanced sky dome was meant to cover everything, so this used to clear to
            // black on that assumption - but the dome is a hemisphere, not a full sphere, and
            // wherever the ground does not quite reach (a streamed tile not loaded yet, a gap
            // right at the horizon) that showed as a stark black void, where vanilla's plain
            // sky colour clear made the very same gap invisible. Using that same colour here
            // (unscaled - multiplying it by the enhanced exposure blew a night sky's dim clear
            // colour out to white instead) keeps a real gap from ever reading as a rendering
            // bug of its own.
            let sky = if lighting.classic && !enhanced {
                lighting.sky_color.map(|v| {
                    if v <= 0.04045 {
                        v / 12.92
                    } else {
                        ((v + 0.055) / 1.055).powf(2.4)
                    }
                })
            } else {
                lighting.sky_color
            };
            let msaa_color = targets.as_ref().map(|t| &t.0);
            let depth_view: &wgpu::TextureView = match &targets {
                Some(t) => &t.1,
                None => &self.ao.as_ref().unwrap().depth_view,
            };
            // the enhanced path draws into a high-range picture the post pass then grades
            let hdr = if masked_frame {
                self.hdr_targets.get(&(width, height))
            } else {
                None
            };
            let (draw_view, resolve_view): (&wgpu::TextureView, Option<&wgpu::TextureView>) =
                match hdr {
                    Some(h) => match &h.msaa_view {
                        Some(m) => (m, Some(&h.view)),
                        None => (&h.view, None),
                    },
                    None => {
                        if single {
                            (scene_view, None)
                        } else {
                            (msaa_color.expect("multisampled target"), Some(scene_view))
                        }
                    }
                };
            let pp = self.main_pass(enhanced, reflection_frame);
            // (a mirror of an Enhanced picture, plainly shaded: the window's sky, see
            // `sky_mirror_pipeline`)
            let sky_pipe = match &self.sky_mirror_pipeline {
                Some(p)
                    if lighting.enhanced
                        && !enhanced
                        && !with_overlays
                        && self.sky_state.is_some()
                        && omsi_cfg::env::var_os("OMSI_NO_ENHANCED").is_none() =>
                {
                    p
                }
                _ => &pp.sky_pipeline,
            };
            // the enhanced pass's screen mask beside the picture (see `MASK_FORMAT`)
            let mask_attachment = hdr.map(|h| wgpu::RenderPassColorAttachment {
                view: h.mask_msaa.as_ref().unwrap_or(&h.mask),
                depth_slice: None,
                resolve_target: h.mask_msaa.as_ref().map(|_| &h.mask),
                ops: wgpu::Operations {
                    load: if parts > 1 {
                        wgpu::LoadOp::Load
                    } else {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    },
                    store: if h.mask_msaa.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            });
            let per_part = main_bundles.len().div_ceil(parts.max(1));
            let sky_clear = wgpu::LoadOp::Clear(wgpu::Color {
                r: sky.x as f64,
                g: sky.y as f64,
                b: sky.z as f64,
                a: 1.0,
            });
            let depth_first = if share_depth || msaa_prepass {
                wgpu::LoadOp::Load
            } else {
                wgpu::LoadOp::Clear(0.0)
            };
            for g in 0..parts.saturating_sub(1) {
                let first = g == 0;
                let mut part = lead.take().unwrap_or_else(|| {
                    self.device
                        .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                            label: Some("main part"),
                        })
                });
                {
                    let part_colors = [
                        Some(wgpu::RenderPassColorAttachment {
                            view: draw_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: if first { sky_clear } else { wgpu::LoadOp::Load },
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                        hdr.map(|h| wgpu::RenderPassColorAttachment {
                            view: h.mask_msaa.as_ref().unwrap_or(&h.mask),
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: if first {
                                    wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                                } else {
                                    wgpu::LoadOp::Load
                                },
                                store: wgpu::StoreOp::Store,
                            },
                        }),
                        hdr.and_then(|h| h.gbuf.as_ref()).map(|g| {
                            wgpu::RenderPassColorAttachment {
                                view: g[0].0.as_ref().unwrap_or(&g[0].1),
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: if first {
                                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                                    } else {
                                        wgpu::LoadOp::Load
                                    },
                                    store: wgpu::StoreOp::Store,
                                },
                            }
                        }),
                        hdr.and_then(|h| h.gbuf.as_ref()).map(|g| {
                            wgpu::RenderPassColorAttachment {
                                view: g[1].0.as_ref().unwrap_or(&g[1].1),
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load: if first {
                                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                                    } else {
                                        wgpu::LoadOp::Load
                                    },
                                    store: wgpu::StoreOp::Store,
                                },
                            }
                        }),
                    ];
                    let mut pass = part.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("main part"),
                        color_attachments: if part_colors[2].is_some() {
                            &part_colors[..]
                        } else if part_colors[1].is_some() {
                            &part_colors[..2]
                        } else {
                            &part_colors[..1]
                        },
                        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                            view: depth_view,
                            depth_ops: Some(wgpu::Operations {
                                load: if first {
                                    depth_first
                                } else {
                                    wgpu::LoadOp::Load
                                },
                                store: wgpu::StoreOp::Store,
                            }),
                            stencil_ops: None,
                        }),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                    if first {
                        if let Some(sky) = &scene.sky_bind_group {
                            pass.set_pipeline(sky_pipe);
                            pass.set_bind_group(1, sky, &[]);
                            pass.set_vertex_buffer(0, self.sky_mesh.0.slice(..));
                            pass.set_index_buffer(
                                self.sky_mesh.1.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            pass.draw_indexed(0..self.sky_mesh.2, 0, 0..1);
                        }
                    }
                    pass.execute_bundles(
                        main_bundles[g * per_part..((g + 1) * per_part).min(main_bundles.len())]
                            .iter(),
                    );
                }
                main_parts.push(part);
            }
            let tail = (parts - 1) * per_part;
            let main_attachment = Some(wgpu::RenderPassColorAttachment {
                view: draw_view,
                depth_slice: None,
                resolve_target: resolve_view,
                ops: wgpu::Operations {
                    load: if parts > 1 {
                        wgpu::LoadOp::Load
                    } else {
                        sky_clear
                    },
                    store: if resolve_view.is_none() {
                        wgpu::StoreOp::Store
                    } else {
                        wgpu::StoreOp::Discard
                    },
                },
            });
            let gbuf_attachments = hdr.and_then(|h| h.gbuf.as_ref()).map(|g| {
                g.each_ref().map(|(msaa, view)| {
                    Some(wgpu::RenderPassColorAttachment {
                        view: msaa.as_ref().unwrap_or(view),
                        depth_slice: None,
                        resolve_target: msaa.as_ref().map(|_| view),
                        ops: wgpu::Operations {
                            load: if parts > 1 {
                                wgpu::LoadOp::Load
                            } else {
                                wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                            },
                            store: if msaa.is_some() {
                                wgpu::StoreOp::Discard
                            } else {
                                wgpu::StoreOp::Store
                            },
                        },
                    })
                })
            });
            let [g0, g1] = gbuf_attachments.unwrap_or([None, None]);
            let colors = [main_attachment, mask_attachment, g0, g1];
            let colors = if colors[2].is_some() {
                &colors[..]
            } else if colors[1].is_some() {
                &colors[..2]
            } else {
                &colors[..1]
            };
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("main"),
                // drawn with MSAA samples and resolved into the real target at the end
                // (without multisampling straight into the target)
                color_attachments: colors,
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: if parts > 1 {
                            wgpu::LoadOp::Load
                        } else {
                            depth_first
                        },
                        // (nothing reads the picture's depth after the pass - the ambient
                        // occlusion reads the prepass's own texture - so where the depth is
                        // not carried over, a tile-based GPU need not flush a full-size
                        // depth buffer back)
                        store: if share_depth || msaa_prepass || ao_on {
                            wgpu::StoreOp::Store
                        } else {
                            wgpu::StoreOp::Discard
                        },
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: pass_timer(
                    tset.as_ref(),
                    &mut timed,
                    if with_overlays { "main" } else { "mirror" },
                ),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
            if let Some(sky) = scene.sky_bind_group.as_ref().filter(|_| parts == 1) {
                pass.set_pipeline(sky_pipe);
                pass.set_bind_group(1, sky, &[]);
                pass.set_vertex_buffer(0, self.sky_mesh.0.slice(..));
                pass.set_index_buffer(self.sky_mesh.1.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.sky_mesh.2, 0, 0..1);
            }
            if main_bundles.is_empty() {
                pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                encode_batches(&mut pass, scene, &main_batches, |pipe| {
                    main_pipeline(pp, pipe)
                });
            } else {
                // the batches, recorded as bundles on several threads (see `record_bundles`)
                pass.execute_bundles(main_bundles[tail..].iter());
                // a bundle leaves the pass without bind groups
                pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
            }
            // smoke, blended over the scene
            if scene.smoke_count > 0 && omsi_cfg::env::var_os("OMSI_NO_SMOKE").is_none() {
                if let Some(sb) = &scene.smoke_buf {
                    pass.set_pipeline(&pp.smoke_pipeline);
                    pass.set_bind_group(1, &self.smoke_bind_group, &[]);
                    pass.set_vertex_buffer(0, sb.slice(..));
                    pass.draw(0..6, 0..scene.smoke_count);
                }
            }
            // the snowfall, every flake worked out on the GPU (snow.wgsl), over the world and
            // under the cab of the vehicle the camera is in
            if let Some(snow) = pp.snow_pipeline.as_ref().filter(|_| {
                lighting.snowfall > 0.01 && omsi_cfg::env::var_os("OMSI_NO_SNOWFALL").is_none()
            }) {
                let (counts, mean) = snowfall_flakes(lighting.snowfall);
                let u = SnowUniform {
                    wind: lighting
                        .wind
                        .extend(self.started.elapsed().as_secs_f32() % 20000.0)
                        .to_array(),
                    fall: [lighting.snowfall, mean, counts[0] as f32, counts[1] as f32],
                };
                self.queue
                    .write_buffer(&self.snow_buf, 0, bytemuck::bytes_of(&u));
                pass.set_pipeline(snow);
                pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                pass.set_bind_group(1, &self.snow_bind_group, &[]);
                pass.draw(0..6, 0..counts.iter().sum::<u32>());
            }
            // light coronas, additive: the world's, then the vehicle the camera is in -
            // drawn over them as Omsi.exe draws it last (see `cab_items`) - then its own
            let coronas_on =
                scene.corona_count > 0 && omsi_cfg::env::var_os("OMSI_NO_CORONAS").is_none();
            for late in [false, true] {
                if late && !cab_batches.is_empty() {
                    pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                    encode_batches(&mut pass, scene, &cab_batches, |pipe| {
                        main_pipeline(pp, pipe)
                    });
                }
                if let Some(cb) = scene.corona_buf.as_ref().filter(|_| coronas_on) {
                    pass.set_pipeline(&pp.corona_pipeline);
                    pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
                    pass.set_vertex_buffer(0, cb.slice(..));
                    // in runs by picture (the standard glow, the lights' own bitmaps, the cone)
                    for &(tex, first, count, _) in scene.corona_runs.iter().filter(|r| r.3 == late)
                    {
                        let bg = self
                            .corona_textures
                            .get(tex as usize)
                            .and_then(|b| b.as_ref())
                            .unwrap_or(&self.corona_bind_group);
                        pass.set_bind_group(1, bg, &[]);
                        pass.draw(0..6, first..first + count);
                    }
                }
            }
            // HUD overlays (on the vanilla path at full size; the enhanced path draws them
            // after grading, a scaled picture after scaling it up)
            if !overlays.is_empty() && !masked_frame && !scaled {
                pass.set_pipeline(&self.overlay_pipeline);
                for (k, _) in overlays.iter().enumerate() {
                    if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
        // Weather alone is insufficient: leave the allocation and reflection passes out when
        // the visible batches contain no moisture-tagged surface (a showroom, bare terrain).
        // Enhanced+: the traced reflections (wet roads' too) in place of the puddles' rays
        if rt_frame {
            self.encode_rt_reflections(&mut encoder, width, height, tset.as_ref(), &mut timed);
        }
        let puddles_on = puddles_wanted
            && !rt_frame
            && main_batches
                .iter()
                .any(|b| scene.materials[b.material as usize].uniform.params2[2] > 0.0)
            && self.prepare_puddle_reflections(
                width, height, camera, aspect, projection, &cu, lighting, ro,
            );
        if puddles_on {
            // (the vehicle the camera is in is drawn into the puddles' picture as well)
            let all: Vec<Batch>;
            let batches = if cab_batches.is_empty() {
                &main_batches
            } else {
                all = main_batches.iter().chain(&cab_batches).cloned().collect();
                &all
            };
            self.encode_puddle_reflections(
                &mut encoder,
                width,
                height,
                scene,
                batches,
                &list,
                lighting,
                camera,
                tset.as_ref(),
                &mut timed,
            );
        }
        // the lamps' light in the weather's fog and the sun's shafts between the shadows
        // (before the rain on the panes, which writes the prepass depth),
        // over the picture before its glow and metering (the window's picture; the mirrors
        // go without)
        let fog_lamps = enhanced_weather_fog(lighting) >= 2e-4 && !scene.lights.is_empty();
        let shafts = lighting.shadows && lighting.sun_dir.z > 0.0;
        let glare = lighting.sun_dir.z > -0.01
            && lighting.sun_intensity > 0.0
            && omsi_cfg::env::var_os("OMSI_NO_GLARE").is_none();
        if omsi_cfg::env::var_os("OMSI_DEBUG_FOG_LAMPS").is_some() {
            log::info!("fog lamps: enhanced {enhanced} prepass {prepass_on} overlays {with_overlays} masked {masked_frame} fog {:.5} lights {} shafts {shafts} glare {glare}", enhanced_weather_fog(lighting), scene.lights.len());
        }
        if enhanced && prepass_on && with_overlays && masked_frame && (fog_lamps || shafts || glare)
        {
            if let (Some(pipes), Some(ao), Some(cam_bg)) = (
                self.fog_lamps_pipeline.as_ref(),
                self.ao.as_ref(),
                scene.camera_bind_group.as_ref(),
            ) {
                if let Some(fog_bg) = ao.fog_bg.as_ref() {
                    // (how much of the weather's extinction is mist and fog - droplets of some
                    // ten micrometres, which scatter a lamp's light into a halo and its beam
                    // into a cone - rather than rain: a raindrop of a millimetre sends what it
                    // scatters on within a few hundredths of a degree, no halo round a lamp,
                    // no cone of a headlight in a drizzle (the rain's share as lights.rs
                    // `apply_weather` lays it on))
                    let fog_all = enhanced_weather_fog(lighting);
                    let rain_part = if lighting.rain > 0.0 && lighting.snowfall <= 0.0 {
                        2.3 / (2500.0 - 1800.0 * lighting.rain.clamp(0.0, 1.0))
                    } else {
                        0.0
                    };
                    let droplets = if fog_all > 0.0 {
                        ((fog_all - rain_part) / fog_all).clamp(0.0, 1.0)
                    } else {
                        0.0
                    };
                    let u = FogLampUniform {
                        inv_view_proj: vp_mat.inverse().to_cols_array_2d(),
                        size: [
                            width as f32,
                            height as f32,
                            if glare { 1.0 } else { 0.0 },
                            droplets,
                        ],
                    };
                    self.queue
                        .write_buffer(&self.fog_lamps_buf, 0, bytemuck::bytes_of(&u));
                    let h = &self.hdr_targets[&(width, height)];
                    let view = h
                        .puddles
                        .as_ref()
                        .filter(|_| puddles_on)
                        .map_or(&h.view, |p| &p.view);
                    for (k, (target, load)) in [
                        (&ao.fog_view, wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)),
                        (view, wgpu::LoadOp::Load),
                    ]
                    .into_iter()
                    .enumerate()
                    {
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("fog lamps"),
                            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                                view: target,
                                depth_slice: None,
                                resolve_target: None,
                                ops: wgpu::Operations {
                                    load,
                                    store: wgpu::StoreOp::Store,
                                },
                            })],
                            depth_stencil_attachment: None,
                            timestamp_writes: if k == 1 {
                                pass_timer(tset.as_ref(), &mut timed, "fog lamps")
                            } else {
                                None
                            },
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        pass.set_pipeline(&pipes[k]);
                        pass.set_bind_group(0, cam_bg, &[]);
                        pass.set_bind_group(1, &fog_bg[k], &[]);
                        pass.draw(0..3, 0..1);
                    }
                }
            }
        }
        if glass_on {
            let hdr = masked_frame.then(|| &self.hdr_targets[&(width, height)]);
            let view = hdr.map_or(scene_view, |h| {
                h.puddles
                    .as_ref()
                    .filter(|_| puddles_on)
                    .map_or(&h.view, |p| &p.view)
            });
            let behind = self.glass_picture.as_ref().unwrap();
            encoder.copy_texture_to_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: view.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyTextureInfo {
                    texture: behind.texture(),
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
            );
            let colours = [
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                hdr.map(|h| wgpu::RenderPassColorAttachment {
                    view: &h.mask,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                }),
                hdr.and_then(|h| h.gbuf.as_ref())
                    .map(|g| wgpu::RenderPassColorAttachment {
                        view: &g[0].1,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
                hdr.and_then(|h| h.gbuf.as_ref())
                    .map(|g| wgpu::RenderPassColorAttachment {
                        view: &g[1].1,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Load,
                            store: wgpu::StoreOp::Store,
                        },
                    }),
            ];
            let pipes = &self.main_pass(enhanced, reflection_frame).rain_pipelines;
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("rain on current scene"),
                color_attachments: &colours[..if colours[2].is_some() {
                    4
                } else if hdr.is_some() {
                    2
                } else {
                    1
                }],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.ao.as_ref().unwrap().depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
            encode_batches(&mut pass, scene, &rain_batches, |pipe| {
                &pipes[pipe as usize]
            });
        }
        if reflection_frame {
            let h = &self.hdr_targets[&(width, height)];
            let bg = h
                .puddles
                .as_ref()
                .filter(|_| puddles_on)
                .map_or(&h.classic_bg, |p| &p.classic_bg);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("classic reflections present"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: scene_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.copy_pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
            if !overlays.is_empty() && !scaled {
                pass.set_pipeline(&self.overlay_pipeline_1x);
                for (k, _) in overlays.iter().enumerate() {
                    if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
        if enhanced {
            // --- the post passes: glow, metering and adaptation, tone curve, FXAA
            let secs = |tau: f32| {
                if dt > 0.0 {
                    1.0 - (-dt / tau).exp()
                } else {
                    1.0
                }
            };
            // (the brightening is for what the light model cannot know by day - a dark cab,
            // an underpass; by night the picture is dark because the night is, and the eye's
            // adaptation to it is the light model's already: lifted by the meter on top, a
            // lamp-lit street came out most of a stop brighter than any eye sees it)
            let mut m = meter_tuning();
            let after_sunset = self.sky_state.as_ref().map_or(0.0, |st| {
                ((0.052 - st.input.sun_dir.normalize_or_zero().z) / 0.157).clamp(0.0, 1.0)
            });
            m[3] *= 1.0
                - atmosphere::smoothstep(
                    3.0,
                    7.0,
                    self.exposure.unwrap_or(0.0) / std::f32::consts::LN_2,
                );
            let pu = PostUniform {
                // the metering may take a little off a bright picture and add a little to a
                // dark one: a night stays a night, snow stays white
                a: [
                    // (the share of the light beyond the screen's white the eye scatters
                    // further than a third of a degree, CIE 146 - see post.wgsl `fs_up`)
                    0.2,
                    m[2],
                    m[3],
                    if self.instant_exposure || dt <= 0.0 {
                        1.0
                    } else {
                        0.0
                    },
                ],
                // darker: the eye takes a few seconds; brighter: under one
                b: [
                    secs(2.5),
                    secs(0.6),
                    m[1],
                    m[4] + lighting.night_brightness * after_sunset,
                ],
                // (w: an LED panel's dots count for this much in the glow's source. The mix
                // the glow lands with is a few per cent - a lamp a hundred times brighter
                // than white spreads, a white wall does not - so the dots are multiplied up
                // there instead of being drawn burning: their halo shows, they don't bleach.
                // `Led glow`, 0 = not at all.)
                c: [
                    m[0],
                    m[5],
                    self.exposure.map(f32::exp).unwrap_or(1.0),
                    lighting.led_glow * 10.0,
                ],
                // Enhanced+: its grade, the vignette and the sharpening; both: the tone
                // curve's contrast (post.wgsl `natural_tone`)
                d: {
                    let contrast = tone_contrast(self.exposure.unwrap_or(0.0));
                    if rt_frame && omsi_cfg::env::var_os("OMSI_NO_RT_GRADE").is_none() {
                        [1.0, 0.08, 0.0, contrast]
                    } else {
                        [0.0, 0.0, 0.0, contrast]
                    }
                },
            };
            self.queue
                .write_buffer(&self.post_buf, 0, bytemuck::bytes_of(&pu));
            // (a mirror's small picture goes without FXAA)
            let fxaa = with_overlays
                && self.options.fxaa
                && self.options.msaa <= 1
                && omsi_cfg::env::var_os("OMSI_NO_FXAA").is_none();
            if let Some(h) = self.hdr_targets.get(&(width, height)) {
                let puddles = h.puddles.as_ref().filter(|_| puddles_on);
                let levels = h.down.len();
                for i in 0..levels {
                    post_pass(
                        &mut encoder,
                        &h.down[i],
                        None,
                        if i == 0 {
                            &self.post.down_first
                        } else {
                            &self.post.down
                        },
                        if i == 0 {
                            puddles.map(|p| &p.down_bg).unwrap_or(&h.down_bg[i])
                        } else {
                            &h.down_bg[i]
                        },
                    );
                }
                // the exposure: meter the smallest level, move the adapted value towards it
                // (the window's picture only: a mirror is graded with the window's exposure,
                // as the eye that looks into it is adapted to the street)
                if lead_view {
                    post_pass(
                        &mut encoder,
                        &self.meter_view,
                        None,
                        &self.post.meter,
                        &h.meter_bg,
                    );
                    let front = self.adapt_front;
                    post_pass(
                        &mut encoder,
                        &self.adapt_views[1 - front],
                        None,
                        &self.post.adapt,
                        &self.adapt_bg[front],
                    );
                    self.adapt_front = 1 - front;
                }
                // (a device lost since this frame began took the meter's buffer as well)
                let lost = self.device_lost().is_some();
                if let Some(log) = self
                    .exposure_log
                    .as_mut()
                    .filter(|_| with_overlays && !lost)
                {
                    let pre = self.exposure.unwrap_or(0.0) / std::f32::consts::LN_2;
                    log.sample(&mut encoder, &self.adapt_views[self.adapt_front], pre, m);
                }
                // (timed on its last pass: the glow chain with the metering, see GpuTimers)
                for i in (0..levels).rev() {
                    let timer = if i == 0 {
                        pass_timer(tset.as_ref(), &mut timed, "glow+meter")
                    } else {
                        None
                    };
                    post_pass(&mut encoder, &h.up[i], timer, &self.post.up, &h.up_bg[i]);
                }
                let final_view = if fxaa { &h.ldr } else { scene_view };
                let tonemap_bg =
                    &puddles.map(|p| &p.tonemap_bg).unwrap_or(&h.tonemap_bg)[self.adapt_front];
                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tone map"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: final_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: pass_timer(tset.as_ref(), &mut timed, "tone map"),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(if fxaa {
                    &self.post.tonemap_encoded
                } else {
                    &self.post.tonemap
                });
                pass.set_bind_group(0, tonemap_bg, &[]);
                pass.draw(0..3, 0..1);
                if fxaa {
                    drop(pass);
                    pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("fxaa"),
                        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                            view: scene_view,
                            depth_slice: None,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                store: wgpu::StoreOp::Store,
                            },
                        })],
                        depth_stencil_attachment: None,
                        timestamp_writes: pass_timer(tset.as_ref(), &mut timed, "fxaa"),
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    pass.set_pipeline(&self.post.fxaa);
                    pass.set_bind_group(0, &h.fxaa_bg, &[]);
                    pass.draw(0..3, 0..1);
                }
                if !overlays.is_empty() && !scaled {
                    pass.set_pipeline(&self.overlay_pipeline_1x);
                    for (k, _) in overlays.iter().enumerate() {
                        if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                            pass.set_bind_group(0, bg, &[]);
                            pass.draw(0..6, 0..1);
                        }
                    }
                }
            }
        }
        if let Some((_, bg)) = &scene_target {
            // --- the smaller picture scaled up to the window, the HUD on top at full size;
            // the smaller the picture, the more it is sharpened
            let sharpen = (1.0 - width as f32 / full_w as f32) * 2.0;
            self.queue.write_buffer(
                &self.upscale_buf,
                0,
                bytemuck::cast_slice(&[
                    width as f32,
                    height as f32,
                    sharpen.clamp(0.0, 0.8),
                    if vanilla_fxaa { 1.0 } else { 0.0 },
                ]),
            );
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("upscale"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: pass_timer(tset.as_ref(), &mut timed, "upscale"),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.upscale_pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.draw(0..3, 0..1);
            if !overlays.is_empty() {
                pass.set_pipeline(&self.overlay_pipeline_1x);
                for (k, _) in overlays.iter().enumerate() {
                    if let Some((_, _, bg, _)) = scene.overlay_res.get(k) {
                        pass.set_bind_group(0, bg, &[]);
                        pass.draw(0..6, 0..1);
                    }
                }
            }
        }
        stage(self, "encode", "mirror.encode");
        // Turning the recorded passes into Metal commands is the costliest CPU step of a
        // frame (wgpu checks every draw): the shadow maps and the prepass are finished on
        // helper threads while this one finishes the picture.
        let big = shadow_batches.iter().map(|b| b.len()).sum::<usize>() + prepass_batches.len()
            > 64
            || !main_parts.is_empty();
        let profiling = self.profiling;
        let finish = |encoder: wgpu::CommandEncoder| {
            let start = profiling.then(std::time::Instant::now);
            let commands = encoder.finish();
            (
                commands,
                start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0),
            )
        };
        let (shadow_commands, prepass_commands, part_commands, commands, finish_times) =
            if big && self.encoding_pool.is_some() {
                self.encoding_pool
                    .as_ref()
                    .unwrap()
                    .in_place_scope_fifo(|scope| {
                        let (shadow_tx, shadow_rx) = std::sync::mpsc::sync_channel(1);
                        let (prepass_tx, prepass_rx) = std::sync::mpsc::sync_channel(1);
                        let parts = main_parts.len();
                        let (part_tx, part_rx) = std::sync::mpsc::sync_channel(parts.max(1));
                        let finish = &finish;
                        scope.spawn_fifo(move |_| {
                            let _ = shadow_tx.send(finish(shadow_encoder));
                        });
                        scope.spawn_fifo(move |_| {
                            let _ = prepass_tx.send(finish(prepass_encoder));
                        });
                        for (k, e) in main_parts.into_iter().enumerate() {
                            let tx = part_tx.clone();
                            scope.spawn_fifo(move |_| {
                                let _ = tx.send((k, finish(e).0));
                            });
                        }
                        let (commands, main_secs) = finish(encoder);
                        let wait = std::time::Instant::now();
                        let (shadow_commands, shadow_secs) =
                            shadow_rx.recv().expect("command encoding worker");
                        let shadow_wait = wait.elapsed().as_secs_f64();
                        let wait = std::time::Instant::now();
                        let (prepass_commands, prepass_secs) =
                            prepass_rx.recv().expect("command encoding worker");
                        let mut part_commands: Vec<Option<wgpu::CommandBuffer>> =
                            (0..parts).map(|_| None).collect();
                        for _ in 0..parts {
                            let (k, c) = part_rx.recv().expect("command encoding worker");
                            part_commands[k] = Some(c);
                        }
                        (
                            shadow_commands,
                            prepass_commands,
                            part_commands
                                .into_iter()
                                .map(|c| c.expect("main pass part"))
                                .collect::<Vec<_>>(),
                            commands,
                            [
                                shadow_secs,
                                prepass_secs,
                                main_secs,
                                shadow_wait,
                                wait.elapsed().as_secs_f64(),
                            ],
                        )
                    })
            } else if big {
                std::thread::scope(|scope| {
                    let finish = &finish;
                    let shadow = scope.spawn(move || finish(shadow_encoder));
                    let prepass = scope.spawn(move || finish(prepass_encoder));
                    let parts: Vec<_> = main_parts
                        .into_iter()
                        .map(|e| scope.spawn(move || finish(e).0))
                        .collect();
                    let (commands, main_secs) = finish(encoder);
                    let wait = std::time::Instant::now();
                    let (shadow_commands, shadow_secs) =
                        shadow.join().expect("command encoding thread");
                    let shadow_wait = wait.elapsed().as_secs_f64();
                    let wait = std::time::Instant::now();
                    let (prepass_commands, prepass_secs) =
                        prepass.join().expect("command encoding thread");
                    (
                        shadow_commands,
                        prepass_commands,
                        parts
                            .into_iter()
                            .map(|h| h.join().expect("command encoding thread"))
                            .collect::<Vec<_>>(),
                        commands,
                        [
                            shadow_secs,
                            prepass_secs,
                            main_secs,
                            shadow_wait,
                            wait.elapsed().as_secs_f64(),
                        ],
                    )
                })
            } else {
                let (shadow, shadow_secs) = finish(shadow_encoder);
                let (prepass, prepass_secs) = finish(prepass_encoder);
                let parts: Vec<_> = main_parts.into_iter().map(|e| finish(e).0).collect();
                let (main, main_secs) = finish(encoder);
                (
                    shadow,
                    prepass,
                    parts,
                    main,
                    [shadow_secs, prepass_secs, main_secs, 0.0, 0.0],
                )
            };
        if self.profiling {
            // These overlap across helper threads; do not add them to the stage totals.
            let keys = if with_overlays {
                [
                    "finish.shadow",
                    "finish.prepass",
                    "finish.main",
                    "finish.wait shadow",
                    "finish.wait prepass",
                ]
            } else {
                [
                    "mirror.finish.shadow",
                    "mirror.finish.prepass",
                    "mirror.finish.main",
                    "mirror.finish.wait shadow",
                    "mirror.finish.wait prepass",
                ]
            };
            for (key, secs) in keys.into_iter().zip(finish_times) {
                *self.stats.borrow_mut().entry(key).or_default() += secs;
            }
        }
        stage(self, "finish", "mirror.finish");
        self.queue.submit(
            [shadow_commands, prepass_commands]
                .into_iter()
                .chain(part_commands)
                .chain([commands]),
        );
        stage(self, "submit", "mirror.submit");
        if let (Some(t), false) = (
            self.gpu_timers[with_overlays as usize].as_mut(),
            timed.is_empty(),
        ) {
            t.pending = timed;
            t.unresolved = true;
        }
    }
}

/// Is `p` inside a vehicle's `[boundingbox]` (origin, heading in degrees, w l h cx cy cz)?
/// The same box the shader keeps the weather out of, without its margins.
fn point_in_vehicle_box(p: DVec3, (origin, heading, bb): &(DVec3, f64, [f32; 6])) -> bool {
    let d = (p - *origin).as_vec3();
    let (sh, ch) = (*heading as f32).to_radians().sin_cos();
    let x = d.x * ch - d.y * sh - bb[3];
    let y = d.x * sh + d.y * ch - bb[4];
    let z = d.z - bb[5];
    x.abs() < bb[0] * 0.5 && y.abs() < bb[1] * 0.5 && z.abs() < bb[2] * 0.5
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The night meter takes the street the picture shows: from 30 m over a lot lit by a
    /// grid of street lamps, looking down at it, the eye adapts to the lit lot - not, as
    /// when it sampled a "ground" 1.6 m under the camera (in the air, out of every lamp's
    /// reach), to a moonless night that lifted the lot's light to white.
    #[test]
    fn the_night_meter_sees_a_lamp_lit_lot_from_above() {
        let mut lights = Vec::new();
        for i in 0..4 {
            for j in 0..4 {
                lights.push(PointLight {
                    position: DVec3::new(i as f64 * 12.0 - 18.0, j as f64 * 12.0 - 18.0, 8.0),
                    radius: 24.0,
                    color: [1.0, 0.8, 0.5],
                    intensity: 1.0,
                    core: 3.0,
                    housed: true,
                    mode: LightMode::Both,
                    ..Default::default()
                });
            }
        }
        let cam = Camera {
            position: DVec3::new(0.0, -35.0, 30.0),
            yaw: 0.0,
            pitch: -40.0,
            roll: 0.0,
            fov_deg: 60.0,
            near: 0.1,
            far: 1000.0,
        };
        let cam_rel = cam.position.as_vec3();
        let seen = view_lamp_light(&lights, DVec3::ZERO, cam_rel, &cam, 16.0 / 9.0, Some(0.0));
        let in_air = view_lamp_light(&lights, DVec3::ZERO, cam_rel, &cam, 16.0 / 9.0, None);
        // (the lot's ground: some 10-30 lux under the lamps, 1e-3 .. 3e-3, less between and
        // round them - a log-average of some 10 lux; a moonless night is 2e-6)
        assert!(seen > 5e-5, "{seen}");
        assert!(in_air < seen * 0.2, "{in_air} vs {seen}");
        // and the lamps seen from beside the lot, at eye height, veil the eye as well
        let eye = Camera {
            position: DVec3::new(0.0, -40.0, 1.7),
            pitch: 0.0,
            ..cam
        };
        assert!(glare_veil(&lights, DVec3::ZERO, eye.position.as_vec3(), eye.forward()) > 1e-5);
    }

    /// A textured material can have black diffuse but white ambient (depot interiors).
    /// Enhanced must not turn it into a black surface or silently replace its diffuse.
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn enhanced_respects_material_ambient() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let mesh = renderer.add_mesh(
            &mut scene,
            &MeshData {
                positions: vec![
                    Vec3::new(-8.0, 4.0, -8.0),
                    Vec3::new(8.0, 4.0, -8.0),
                    Vec3::new(8.0, 4.0, 8.0),
                    Vec3::new(-8.0, 4.0, 8.0),
                ],
                normals: vec![-Vec3::Y; 4],
                uvs: vec![glam::Vec2::ZERO; 4],
                ranges: vec![(0, 6, 0)],
                indices: vec![0, 1, 2, 0, 2, 3],
                one_sided: false,
            },
        );
        let texture = renderer.add_texture(
            &mut scene,
            &omsi_texture::Image {
                width: 1,
                height: 1,
                rgba: vec![180, 180, 180, 255],
                has_alpha: true,
            },
            false,
        );
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 90.0,
            near: 0.1,
            far: 100.0,
        };
        let lighting = Lighting {
            enhanced: true,
            shadows: false,
            fog_density: 0.0,
            sun_dir: -Vec3::Y,
            ..Default::default()
        };
        let mut pixels = Vec::new();
        for ambient in [[0.0; 3], [1.0; 3], [1.0, 0.0, 0.0]] {
            let material = renderer.add_material_extra(
                &mut scene,
                Some(texture),
                AlphaMode::Blend,
                [0.0, 0.0, 0.0, 1.0],
                false,
                None,
                None,
                None,
                None,
                [0.0; 3],
                MaterialExtra {
                    ambient: Some(ambient),
                    ..Default::default()
                },
            );
            let id = renderer.add_instance(
                &mut scene,
                mesh,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![material],
            );
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            pixels.push(rgba[(32 * 64 + 32) * 4..][..3].to_vec());
            scene.instances[id].visible = false;
        }
        assert!(
            pixels[1].iter().map(|&v| v as u32).sum::<u32>()
                > pixels[0].iter().map(|&v| v as u32).sum::<u32>() + 60,
            "white ambient must illuminate black diffuse: {pixels:?}"
        );
        assert!(
            pixels[2][0] > pixels[2][1].saturating_add(20)
                && pixels[2][0] > pixels[2][2].saturating_add(20),
            "the material's ambient tint must be preserved: {pixels:?}"
        );
    }

    /// A road must cover flush terrain without covering a building floor above it.
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn surface_bias_keeps_raised_floors_visible_at_a_distance() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let mesh = renderer.add_mesh(
            &mut scene,
            &MeshData {
                positions: vec![
                    Vec3::new(-1000.0, -1000.0, 0.0),
                    Vec3::new(1000.0, -1000.0, 0.0),
                    Vec3::new(1000.0, 1000.0, 0.0),
                    Vec3::new(-1000.0, 1000.0, 0.0),
                ],
                normals: vec![Vec3::Z; 4],
                uvs: vec![glam::Vec2::ZERO; 4],
                ranges: vec![(0, 6, 0)],
                indices: vec![0, 1, 2, 0, 2, 3],
                one_sided: false,
            },
        );
        let blue = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.0, 0.0, 1.0, 1.0],
            true,
        );
        let green = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.0, 1.0, 0.0, 1.0],
            true,
        );
        let red = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [1.0, 0.0, 0.0, 1.0],
            true,
        );
        let ground =
            renderer.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![blue]);
        scene.instances[ground].render_phase = RenderPhase::Terrain;
        let road = renderer.add_surface_instance(
            &mut scene,
            mesh,
            DVec3::ZERO,
            Mat4::IDENTITY,
            vec![green],
        );
        scene.instances[road].render_phase = RenderPhase::Spline;
        let floor = renderer.add_instance(
            &mut scene,
            mesh,
            DVec3::new(0.0, 0.0, 0.03),
            Mat4::IDENTITY,
            vec![red],
        );
        scene.instances[floor].render_phase = RenderPhase::AfterVehicles;
        let lighting = Lighting {
            shadows: false,
            fog_density: 0.0,
            ..Default::default()
        };
        for (pitch, fov) in [(-5.0, 60.0), (-5.0, 20.0), (-30.0, 60.0)] {
            let camera = Camera {
                position: DVec3::new(0.0, 0.0, 5.0),
                yaw: 0.0,
                pitch,
                roll: 0.0,
                fov_deg: fov,
                near: 0.1,
                far: 2000.0,
            };
            for show_floor in [false, true] {
                scene.instances[floor].visible = show_floor;
                scene.dirty = true;
                let rgba = renderer
                    .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                    .unwrap();
                let pixel = &rgba[(32 * 64 + 32) * 4..][..3];
                let wanted = if show_floor { 0 } else { 1 };
                assert!(
                    pixel[wanted] > 200 && pixel[1 - wanted] < 20 && pixel[2] < 20,
                    "pitch {pitch}, fov {fov}, floor {show_floor}: {pixel:?}"
                );
            }
        }
    }

    /// An untextured slot (a Blender export's Base Color alone) keeps its colour in Enhanced:
    /// the white ambient Omsi.exe gives every o3d slot turned it white under the sky (#1737).
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn enhanced_untextured_keeps_its_colour() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let mesh = renderer.add_mesh(
            &mut scene,
            &MeshData {
                positions: vec![
                    Vec3::new(-8.0, 4.0, -8.0),
                    Vec3::new(8.0, 4.0, -8.0),
                    Vec3::new(8.0, 4.0, 8.0),
                    Vec3::new(-8.0, 4.0, 8.0),
                ],
                normals: vec![-Vec3::Y; 4],
                uvs: vec![glam::Vec2::ZERO; 4],
                ranges: vec![(0, 6, 0)],
                indices: vec![0, 1, 2, 0, 2, 3],
                one_sided: false,
            },
        );
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 90.0,
            near: 0.1,
            far: 100.0,
        };
        let lighting = Lighting {
            enhanced: true,
            shadows: false,
            fog_density: 0.0,
            sun_dir: -Vec3::Y,
            ..Default::default()
        };
        let material = renderer.add_material_extra(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.8, 0.05, 0.05, 1.0],
            false,
            None,
            None,
            None,
            None,
            [0.0; 3],
            MaterialExtra {
                ambient: Some([1.0; 3]),
                ..Default::default()
            },
        );
        renderer.add_instance(
            &mut scene,
            mesh,
            DVec3::ZERO,
            Mat4::IDENTITY,
            vec![material],
        );
        let rgba = renderer
            .render_to_image(&mut scene, 64, 64, &camera, &lighting)
            .unwrap();
        let px = &rgba[(32 * 64 + 32) * 4..][..3];
        assert!(
            px[0] as u32 > px[1] as u32 * 2 + 10 && px[0] as u32 > px[2] as u32 * 2 + 10,
            "an untextured red slot must stay red: {px:?}"
        );
    }

    /// Overlays drawn texel for pixel: onto whole pixels, their size kept.
    #[test]
    fn overlays_land_on_whole_pixels() {
        // (a 120 x 26 text a quarter and a half pixel off: moved, the same size)
        assert_eq!(
            snap_rect([25.25, 40.5, 145.25, 66.5]),
            [25.0, 41.0, 145.0, 67.0]
        );
        assert_eq!(
            snap_rect([10.0, 20.0, 30.0, 40.0]),
            [10.0, 20.0, 30.0, 40.0]
        );
        // (left of the window as well: the same size)
        assert_eq!(snap_rect([-0.5, -2.5, 19.5, 7.5]), [0.0, -2.0, 20.0, 8.0]);
        // (a separator 0.6 px high stays a line)
        let line = snap_rect([16.0, 100.3, 300.0, 100.9]);
        assert_eq!(line[3] - line[1], 1.0);
        // (an empty rectangle stays empty)
        assert_eq!(snap_rect([5.2, 5.2, 5.2, 5.2]), [5.0, 5.0, 5.0, 5.0]);
    }

    /// The enhanced street is lit by the sun that gets through the weather: none from
    /// under an overcast deck, next to none through a dense fog (which glowed white round
    /// the sun), nearly all of it on a clear or a hazy summer's day (#1106).
    #[test]
    fn enhanced_sun_does_not_shine_through_overcast_or_dense_fog() {
        // an evening sun 27 degrees high, a weather's visibility as the app turns it into
        // a density (`lights::lighting_from`), the cover and the sun `apply_weather` leaves
        let light = |visibility: f32, cloud_density: f32, sun_intensity: f32| Lighting {
            sun_dir: Vec3::new(-0.89, 0.0, 0.454),
            fog_density: (2.3 / visibility).max(0.00005),
            cloud_density,
            sun_intensity,
            ..Default::default()
        };
        let reaching = |l: &Lighting| enhanced_sky_input(l, 1.0).0.sun_visibility;
        // #CAVOK, a cumulus sky, Sommerlich's summer haze of 2 km
        assert!(reaching(&light(50_000.0, 0.0, 1.0)) > 0.99);
        assert!(reaching(&light(50_000.0, 0.75, 0.54)) > 0.53);
        assert!(reaching(&light(2_000.0, 0.35, 1.0)) > 0.4);
        // Bodennebel's 75 m, Schmuddelwetter's 200 m: the sun is gone
        assert!(reaching(&light(75.0, 0.0, 1.0)) < 1e-4);
        assert!(reaching(&light(200.0, 0.0, 1.0)) < 1e-3);
        // an Overcast cloud type (the 15 % of the sun `apply_weather` keeps): gone from the
        // street, while the clouds keep their light from above
        let (input, clouds) = enhanced_sky_input(&light(50_000.0, 1.0, 0.15), 1.0);
        assert_eq!(input.sun_visibility, 0.0);
        assert!((clouds - 0.15).abs() < 1e-6);
    }

    #[test]
    #[ignore = "requires a graphics adapter; renders terrain and foliage lighting"]
    fn enhanced_masked_and_uncut_ground_share_lighting() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let mut texture = |rgba: [u8; 4]| {
            renderer.add_texture(
                &mut scene,
                &omsi_texture::Image {
                    width: 1,
                    height: 1,
                    rgba: rgba.to_vec(),
                    has_alpha: true,
                },
                false,
            )
        };
        let grey = texture([100, 100, 100, 255]);
        let opaque = texture([255; 4]);
        let transparent = texture([100, 100, 100, 0]);
        let masked = renderer.add_terrain_material(
            &mut scene,
            Some(grey),
            Some(opaque),
            None,
            1.0,
            None,
            0.0,
        );
        let uncut =
            renderer.add_terrain_material(&mut scene, Some(grey), None, None, 1.0, None, 0.0);
        let cut = renderer.add_terrain_material(
            &mut scene,
            Some(grey),
            Some(transparent),
            None,
            1.0,
            None,
            0.0,
        );
        let foliage =
            renderer.add_material(&mut scene, Some(grey), AlphaMode::Test, [1.0; 4], false);
        let cut_foliage = renderer.add_material(
            &mut scene,
            Some(transparent),
            AlphaMode::Test,
            [1.0; 4],
            false,
        );
        let backdrop = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [1.0, 0.0, 0.0, 1.0],
            true,
        );
        let mut quad = |left: f32, right: f32, z: f32, material| {
            let mesh = renderer.add_mesh(
                &mut scene,
                &MeshData {
                    positions: vec![
                        Vec3::new(left, -5.0, z),
                        Vec3::new(right, -5.0, z),
                        Vec3::new(right, 5.0, z),
                        Vec3::new(left, 5.0, z),
                    ],
                    normals: vec![Vec3::Z; 4],
                    uvs: vec![glam::Vec2::splat(0.5); 4],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    ranges: vec![(0, 6, 0)],
                    one_sided: false,
                },
            );
            renderer.add_instance(
                &mut scene,
                mesh,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![material],
            )
        };
        // Symmetric samples in the same image share exposure, view and lamp distances.
        quad(-6.0, 6.0, -1.0, backdrop);
        let ground = quad(-5.0, -0.5, 0.0, masked);
        let mapped = quad(0.5, 5.0, 0.0, uncut);
        scene.instances[ground].render_phase = RenderPhase::Terrain;
        scene.instances[mapped].render_phase = RenderPhase::Spline;
        scene.instances[mapped].surface = true;
        let camera = Camera {
            position: DVec3::new(0.0, -0.105, 6.0),
            yaw: 0.0,
            pitch: -89.0,
            roll: 0.0,
            fov_deg: 90.0,
            near: 0.1,
            far: 100.0,
        };
        let day = Lighting {
            enhanced: true,
            sun_dir: Vec3::Z,
            sun_intensity: 1.0,
            shadows: false,
            detail: false,
            fog_density: 0.0,
            ..Default::default()
        };
        let night = Lighting {
            sun_dir: -Vec3::Z,
            sun_intensity: 0.0,
            night: 1.0,
            ..day.clone()
        };
        let pixel = |rgba: &[u8], x: usize| -> [u8; 3] {
            rgba[(32 * 64 + x) * 4..(32 * 64 + x) * 4 + 3]
                .try_into()
                .unwrap()
        };
        for (name, lighting) in [("sun", &day), ("lamp", &night)] {
            scene.lights = if name == "lamp" {
                vec![PointLight {
                    position: DVec3::new(0.0, 0.0, 4.0),
                    radius: 20.0,
                    core: 10.0,
                    intensity: 1.0,
                    ..Default::default()
                }]
            } else {
                Vec::new()
            };
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, lighting)
                .unwrap();
            let (a, b) = (pixel(&rgba, 16), pixel(&rgba, 47));
            assert!(
                a.iter().all(|v| *v > 10 && *v < 245),
                "lit, unclipped {name}: {a:?}"
            );
            assert!(
                a.iter().zip(b).all(|(a, b)| a.abs_diff(b) <= 2),
                "masked terrain and uncut mapped ground differ under {name}: {a:?} / {b:?}"
            );
        }
        // A road cut still reveals the geometry beneath it.
        renderer.set_material(&mut scene, ground, 0, cut);
        let rgba = renderer
            .render_to_image(&mut scene, 64, 64, &camera, &night)
            .unwrap();
        let a = pixel(&rgba, 16);
        assert!(
            a[0] > a[1] + 30 && a[0] > a[2] + 30,
            "road cut must reveal red: {a:?}"
        );

        // Real foliage still scatters light arriving from behind its normal; ground does not.
        renderer.set_material(&mut scene, ground, 0, masked);
        renderer.set_material(&mut scene, mapped, 0, foliage);
        scene.lights[0].position.z = -4.0;
        let rgba = renderer
            .render_to_image(&mut scene, 64, 64, &camera, &night)
            .unwrap();
        let (a, b) = (pixel(&rgba, 16), pixel(&rgba, 47));
        assert!(
            b[1] > a[1] + 8,
            "foliage retains backlighting: ground {a:?}, foliage {b:?}"
        );
        renderer.set_material(&mut scene, mapped, 0, cut_foliage);
        let rgba = renderer
            .render_to_image(&mut scene, 64, 64, &camera, &night)
            .unwrap();
        let b = pixel(&rgba, 47);
        assert!(
            b[0] > b[1] + 30 && b[0] > b[2] + 30,
            "foliage cutout must reveal red: {b:?}"
        );
    }

    #[test]
    fn a_mesh_page_hands_freed_space_to_the_next_mesh_and_joins_its_holes() {
        let (mut free, mut top) = (Vec::new(), 0u64);
        let a = page_take(&mut free, &mut top, 1000, 100).unwrap();
        let b = page_take(&mut free, &mut top, 1000, 200).unwrap();
        let c = page_take(&mut free, &mut top, 1000, 300).unwrap();
        assert_eq!((a, b, c, top), (0, 100, 300, 600));
        page_give(&mut free, &mut top, b, 200);
        assert_eq!(free, vec![(100, 200)]);
        assert_eq!(page_take(&mut free, &mut top, 1000, 150), Some(100));
        assert_eq!(free, vec![(250, 50)]);
        page_give(&mut free, &mut top, a, 100);
        page_give(&mut free, &mut top, 100, 150);
        assert_eq!(free, vec![(0, 300)]);
        page_give(&mut free, &mut top, c, 300);
        assert_eq!((free.len(), top), (0, 0));
        assert_eq!(page_take(&mut free, &mut top, 1000, 1001), None);
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn meshes_coming_and_going_tile_by_tile_keep_the_pages_from_growing() {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = pollster::block_on(Renderer::new_with(
            &adapter,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        if !renderer.mesh_pages() {
            return;
        }
        let mut scene = renderer.new_scene();
        let mesh = |n: usize| {
            let positions: Vec<Vec3> = (0..n)
                .map(|i| Vec3::new(i as f32, (i % 7) as f32, 0.0))
                .collect();
            let indices: Vec<u32> = (0..n as u32 / 3 * 3).collect();
            MeshData {
                normals: vec![Vec3::Z; n],
                uvs: vec![glam::Vec2::ZERO; n],
                ranges: vec![(0, indices.len() as u32, 0)],
                positions,
                indices,
                ..Default::default()
            }
        };
        // tiles of a few thousand meshes of every size, loaded and unloaded in no order, as
        // a long drive across a map brings them; the scene's pages must not keep growing
        let mut tiles: Vec<Vec<MeshId>> = Vec::new();
        let mut seed = 0x1234_5678u64;
        let mut rand = move || {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            seed
        };
        let mut most = 0;
        for round in 0..300 {
            let tile: Vec<MeshId> = (0..400)
                .map(|_| renderer.add_mesh(&mut scene, &mesh(3 + (rand() % 600) as usize)))
                .collect();
            tiles.push(tile);
            if tiles.len() > 12 {
                let gone = tiles.remove((rand() % tiles.len() as u64) as usize);
                for id in gone {
                    renderer.free_mesh(&mut scene, id);
                }
            }
            if round == 50 {
                most = scene.mesh_pages.len();
            }
        }
        assert!(
            scene.mesh_pages.len() <= most + 1,
            "the pages grew from {most} to {} over the drive",
            scene.mesh_pages.len()
        );
        let held = scene.mesh_page_bytes();
        let used: u64 = scene
            .meshes
            .iter()
            .filter(|m| m.page != u32::MAX)
            .map(|m| m.vertex_bytes + m.index_bytes)
            .sum();
        assert!(
            held < used * 3,
            "{held} bytes of pages held for {used} in use"
        );
        for t in &tiles {
            for &id in t {
                let m = &scene.meshes[id];
                assert_eq!(m.vertex_offset % std::mem::size_of::<Vertex>() as u64, 0);
                assert!(m.vertex_offset + m.vertex_bytes <= m.vertex_buf.size());
                assert!(m.first_index as u64 * 4 + m.index_bytes <= m.index_buf.size());
            }
        }
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn msaa_depth_prepass_preserves_layered_surfaces() {
        fn quad(renderer: &Renderer, scene: &mut Scene, z: f32, mat: MaterialId) -> usize {
            let mesh = renderer.add_mesh(
                scene,
                &MeshData {
                    positions: vec![
                        Vec3::new(-4.0, -4.0, z),
                        Vec3::new(4.0, -4.0, z),
                        Vec3::new(4.0, 4.0, z),
                        Vec3::new(-4.0, 4.0, z),
                    ],
                    normals: vec![Vec3::Z; 4],
                    uvs: vec![
                        glam::Vec2::ZERO,
                        glam::Vec2::X,
                        glam::Vec2::ONE,
                        glam::Vec2::Y,
                    ],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    ranges: vec![(0, 6, 0)],
                    one_sided: false,
                },
            );
            renderer.add_surface_instance(scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![mat])
        }
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 4,
                ssao: true,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        if renderer.options.msaa <= 1 {
            return; // This adapter cannot exercise a multisampled depth buffer.
        }
        assert!(renderer.prepass_msaa_pipelines.is_some());
        let mut scene = renderer.new_scene();
        let mask = renderer.add_texture(
            &mut scene,
            &omsi_texture::Image {
                width: 7,
                height: 1,
                rgba: [0, 0, 112, 128, 160, 255, 255]
                    .into_iter()
                    .flat_map(|a| [255, 255, 255, a])
                    .collect(),
                has_alpha: true,
            },
            false,
        );
        let base = renderer.add_terrain_material(&mut scene, None, None, None, 1.0, None, 1.0);
        let ground = quad(&renderer, &mut scene, 0.0, base);
        scene.instances[ground].render_phase = RenderPhase::Terrain;
        let paint =
            renderer.add_terrain_layer_material(&mut scene, None, mask, None, 1.0, None, 1.0);
        let layer = quad(&renderer, &mut scene, 0.0, paint);
        scene.instances[layer].ground_layer = true;
        scene.instances[layer].render_phase = RenderPhase::Terrain;
        let road_mat = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.0, 0.0, 1.0, 1.0],
            true,
        );
        let road = quad(&renderer, &mut scene, 0.02, road_mat);
        scene.instances[road].render_phase = RenderPhase::Spline;
        let cutout = renderer.add_material(
            &mut scene,
            Some(mask),
            AlphaMode::Test,
            [0.0, 1.0, 0.0, 1.0],
            false,
        );
        let fence = quad(&renderer, &mut scene, 0.3, cutout);
        scene.instances[fence].render_phase = RenderPhase::Normal;
        let blend = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Blend,
            [1.0, 0.0, 0.0, 0.25],
            false,
        );
        let pane = quad(&renderer, &mut scene, 0.5, blend);
        scene.instances[pane].render_phase = RenderPhase::Normal;
        let camera = Camera {
            position: DVec3::new(0.0, -0.105, 6.0),
            yaw: 0.0,
            pitch: -89.0,
            roll: 0.0,
            fov_deg: 90.0,
            near: 0.1,
            far: 100.0,
        };
        for wetness in [0.0, 1.0] {
            let lighting = Lighting {
                enhanced: true,
                shadows: false,
                fog_density: 0.0,
                wetness,
                ..Default::default()
            };
            let enabled = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            let pipelines = renderer.prepass_msaa_pipelines.take().unwrap();
            let disabled = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            renderer.prepass_msaa_pipelines = Some(pipelines);
            let delta = enabled
                .iter()
                .zip(&disabled)
                .map(|(a, b)| a.abs_diff(*b))
                .max()
                .unwrap();
            assert!(
                delta <= 2,
                "MSAA prepass changed layered surfaces by {delta}: wetness {wetness}"
            );
            let pixel = |x: usize| &enabled[(32 * 64 + x) * 4..(32 * 64 + x) * 4 + 3];
            assert!(
                pixel(12)[2] > pixel(12)[1] + 30,
                "cutout hid the road: {:?}",
                pixel(12)
            );
            assert!(
                pixel(52)[1] > pixel(52)[2] + 30,
                "opaque cutout disappeared: {:?}",
                pixel(52)
            );
        }
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn a_reshaped_mesh_moves_the_bounds_of_every_instance_drawing_it() {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = pollster::block_on(Renderer::new_with(
            &adapter,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        scene.cache_bounds = true;
        let material = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [1.0; 4], true);
        let data = MeshData {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            uvs: vec![glam::Vec2::ZERO; 3],
            indices: vec![0, 1, 2],
            ranges: vec![(0, 3, 0)],
            ..Default::default()
        };
        let (skinned, plain) = (
            renderer.add_mesh(&mut scene, &data),
            renderer.add_mesh(&mut scene, &data),
        );
        let mut add = |scene: &mut Scene, mesh, x: f64| {
            renderer.add_instance(
                scene,
                mesh,
                DVec3::new(x, 0.0, 0.0),
                Mat4::IDENTITY,
                vec![material],
            )
        };
        let first = add(&mut scene, skinned, 0.0);
        let others: Vec<usize> = (0..300)
            .map(|k| add(&mut scene, plain, k as f64 * 10.0))
            .collect();
        renderer.prepare(&mut scene);
        let pose =
            |scale: f32| -> Vec<Vec3> { data.positions.iter().map(|p| *p * scale).collect() };
        renderer.update_mesh(&mut scene, skinned, &pose(5.0), &data.normals, &data.uvs);
        renderer.prepare(&mut scene);
        // one added after the first pose, one switched to the skinned mesh
        let late = add(&mut scene, skinned, 50.0);
        renderer.set_instance_mesh(&mut scene, others[7], skinned);
        renderer.prepare(&mut scene);
        renderer.update_mesh(&mut scene, skinned, &pose(9.0), &data.normals, &data.uvs);
        renderer.prepare(&mut scene);
        let expected = InstanceBounds::new(&scene.meshes[skinned], Mat4::IDENTITY);
        for i in [first, late, others[7]] {
            assert_eq!(
                scene.instances[i].bounds.radius, expected.radius,
                "instance {i}"
            );
        }
        assert_ne!(scene.instances[others[8]].bounds.radius, expected.radius);
        assert!(
            scene.bounds_users.len() <= 4,
            "{} instances rescanned",
            scene.bounds_users.len()
        );
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn terrain_paint_fast_path_preserves_composition() {
        fn quad(
            renderer: &Renderer,
            scene: &mut Scene,
            rect: [f32; 4],
            z: f32,
            mat: MaterialId,
        ) -> usize {
            let [left, right, bottom, top] = rect;
            let mesh = renderer.add_mesh(
                scene,
                &MeshData {
                    positions: vec![
                        Vec3::new(left, bottom, z),
                        Vec3::new(right, bottom, z),
                        Vec3::new(right, top, z),
                        Vec3::new(left, top, z),
                    ],
                    normals: vec![Vec3::Z; 4],
                    uvs: vec![
                        glam::Vec2::ZERO,
                        glam::Vec2::X,
                        glam::Vec2::ONE,
                        glam::Vec2::Y,
                    ],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    ranges: vec![(0, 6, 0)],
                    one_sided: false,
                },
            );
            renderer.add_surface_instance(scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![mat])
        }
        for msaa in [1, 4] {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let mut renderer = pollster::block_on(Renderer::new_with(
                &instance,
                None,
                Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                RenderOptions {
                    msaa,
                    ssao: false,
                    shadow_size: 1024,
                    fxaa: false,
                    render_scale: 1.0,
                    ..Default::default()
                },
            ))
            .expect("test renderer");
            let mut scene = renderer.new_scene();
            let base = renderer.add_terrain_material(&mut scene, None, None, None, 1.0, None, 1.0);
            let ground = quad(&renderer, &mut scene, [-6.0, 6.0, -6.0, 6.0], 0.0, base);
            scene.instances[ground].render_phase = RenderPhase::Terrain;
            let mut paint_materials = Vec::new();
            for (rgb, alphas) in [
                ([160, 40, 40], [0, 0, 112, 128, 160, 255, 255]),
                ([40, 160, 40], [255, 255, 160, 128, 112, 0, 0]),
            ] {
                let diffuse = renderer.add_texture(
                    &mut scene,
                    &omsi_texture::Image {
                        width: 4,
                        height: 4,
                        rgba: (0..16)
                            .flat_map(|i| {
                                let c =
                                    rgb.map(|v| if (i / 4 + i % 4) % 2 == 0 { v } else { v / 2 });
                                [c[0], c[1], c[2], 255]
                            })
                            .collect(),
                        has_alpha: false,
                    },
                    false,
                );
                let mask = renderer.add_texture(
                    &mut scene,
                    &omsi_texture::Image {
                        width: 7,
                        height: 1,
                        rgba: alphas
                            .into_iter()
                            .flat_map(|a| [255, 255, 255, a])
                            .collect(),
                        has_alpha: true,
                    },
                    false,
                );
                // Generated tiled detail exercises sampling beside empty mask pixels.
                let detail = renderer.add_texture(
                    &mut scene,
                    &omsi_texture::Image {
                        width: 4,
                        height: 4,
                        rgba: (0..16)
                            .flat_map(|i| {
                                let v = if (i / 4 + i % 4) % 2 == 0 { 160 } else { 255 };
                                [v, v, v, 255]
                            })
                            .collect(),
                        has_alpha: false,
                    },
                    false,
                );
                let mat = renderer.add_terrain_layer_material(
                    &mut scene,
                    Some(diffuse),
                    mask,
                    Some((detail, 12.0)),
                    8.0,
                    None,
                    1.0,
                );
                paint_materials.push(mat);
                let layer = quad(&renderer, &mut scene, [-6.0, 6.0, -6.0, 6.0], 0.0, mat);
                scene.instances[layer].ground_layer = true;
                scene.instances[layer].render_phase = RenderPhase::Terrain;
            }
            let blue = renderer.add_material(
                &mut scene,
                None,
                AlphaMode::Opaque,
                [0.0, 0.0, 1.0, 1.0],
                true,
            );
            let road = quad(&renderer, &mut scene, [-6.0, 6.0, -0.7, 0.7], 0.02, blue);
            scene.instances[road].render_phase = RenderPhase::Spline;
            let camera = Camera {
                position: DVec3::new(0.0, -0.105, 6.0),
                yaw: 0.0,
                pitch: -89.0,
                roll: 0.0,
                fov_deg: 90.0,
                near: 0.1,
                far: 100.0,
            };
            // Compare identical scene data through the specialized and original no-write
            // pipelines, including water's opacity and wet-ground reflection coverage.
            for (wetness, water) in [(0.0, false), (1.0, false), (1.0, true)] {
                if water {
                    for &mat in &paint_materials {
                        scene.materials[mat].uniform.ambient[3] = 2.0;
                        renderer.queue.write_buffer(
                            &scene.materials[mat].buf,
                            0,
                            bytemuck::bytes_of(&scene.materials[mat].uniform),
                        );
                    }
                }
                let lighting = Lighting {
                    enhanced: true,
                    shadows: false,
                    fog_density: 0.0,
                    wetness,
                    ..Default::default()
                };
                let actual = renderer
                    .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                    .unwrap();
                let pp = renderer.hdr_pass.as_mut().unwrap();
                let mut saved = Vec::new();
                for variant in 0..4 {
                    let original = pp.pipelines[PIPE_BLEND_NO_WRITE as usize * 4 + variant].clone();
                    saved.push(std::mem::replace(
                        &mut pp.pipelines[PIPE_TERRAIN_PAINT as usize * 4 + variant],
                        original,
                    ));
                }
                let expected = renderer
                    .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                    .unwrap();
                for (variant, pipeline) in saved.into_iter().enumerate() {
                    renderer.hdr_pass.as_mut().unwrap().pipelines
                        [PIPE_TERRAIN_PAINT as usize * 4 + variant] = pipeline;
                }
                let delta = actual
                    .iter()
                    .zip(&expected)
                    .map(|(a, b)| a.abs_diff(*b))
                    .max()
                    .unwrap();
                assert!(
                    delta <= 2,
                    "paint composition changed by {delta}: MSAA {msaa}, wet {wetness}, water {water}"
                );
                let centre = &actual[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 3];
                if debug_view() <= 0.5 {
                    assert!(
                        centre[2] > centre[0] + 30 && centre[2] > centre[1] + 30,
                        "paint hid the road: {centre:?}"
                    );
                }
            }
        }
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn a_freed_and_recycled_mesh_slot_is_no_longer_counted_as_reshaped() {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = pollster::block_on(Renderer::new_with(
            &adapter,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        scene.cache_bounds = true;
        let material = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [1.0; 4], true);
        let data = MeshData {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            uvs: vec![glam::Vec2::ZERO; 3],
            indices: vec![0, 1, 2],
            ranges: vec![(0, 3, 0)],
            ..Default::default()
        };
        let pose =
            |scale: f32| -> Vec<Vec3> { data.positions.iter().map(|p| *p * scale).collect() };
        let (once_skinned, skinned) = (
            renderer.add_mesh(&mut scene, &data),
            renderer.add_mesh(&mut scene, &data),
        );
        let first = renderer.add_instance(
            &mut scene,
            skinned,
            DVec3::ZERO,
            Mat4::IDENTITY,
            vec![material],
        );
        renderer.update_mesh(
            &mut scene,
            once_skinned,
            &pose(2.0),
            &data.normals,
            &data.uvs,
        );
        renderer.prepare(&mut scene);
        // the slot is freed and taken by a plain mesh, which a whole tile then draws
        renderer.free_mesh(&mut scene, once_skinned);
        renderer.prepare(&mut scene);
        let plain = renderer.add_mesh(&mut scene, &data);
        assert_eq!(
            renderer.recycle_mesh(&mut scene, plain, once_skinned),
            once_skinned
        );
        let tile: Vec<usize> = (0..200)
            .map(|k| {
                renderer.add_instance(
                    &mut scene,
                    once_skinned,
                    DVec3::new(k as f64, 0.0, 0.0),
                    Mat4::IDENTITY,
                    vec![material],
                )
            })
            .collect();
        renderer.prepare(&mut scene);
        renderer.update_mesh(&mut scene, skinned, &pose(7.0), &data.normals, &data.uvs);
        renderer.prepare(&mut scene);
        assert!(
            scene.bounds_users.len() <= 2,
            "{} instances rescanned",
            scene.bounds_users.len()
        );
        let expected = InstanceBounds::new(&scene.meshes[skinned], Mat4::IDENTITY);
        assert_eq!(scene.instances[first].bounds.radius, expected.radius);
        let plain_bounds = InstanceBounds::new(&scene.meshes[once_skinned], Mat4::IDENTITY);
        assert_eq!(scene.instances[tile[42]].bounds.radius, plain_bounds.radius);
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn materials_that_look_alike_are_batched_together_whenever_they_were_made() {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = pollster::block_on(Renderer::new_with(
            &adapter,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let image = omsi_texture::Image {
            width: 1,
            height: 1,
            rgba: vec![200, 100, 50, 255],
            has_alpha: false,
        };
        let texture = renderer.add_texture(&mut scene, &image, true);
        let a = renderer.add_material(
            &mut scene,
            Some(texture),
            AlphaMode::Opaque,
            [1.0; 4],
            false,
        );
        renderer.prepare(&mut scene);
        let b = renderer.add_material(
            &mut scene,
            Some(texture),
            AlphaMode::Opaque,
            [1.0; 4],
            false,
        );
        let c = renderer.add_material(
            &mut scene,
            Some(texture),
            AlphaMode::Opaque,
            [0.5, 0.5, 0.5, 1.0],
            false,
        );
        let other = renderer.add_texture(&mut scene, &image, true);
        let d = renderer.add_material(&mut scene, Some(other), AlphaMode::Opaque, [1.0; 4], false);
        let look = |scene: &Scene, m: MaterialId| scene.materials[m].look;
        assert_ne!(scene.materials[a].bind_group, scene.materials[b].bind_group);
        assert_eq!(look(&scene, a), look(&scene, b));
        assert_ne!(look(&scene, a), look(&scene, c));
        assert_ne!(look(&scene, a), look(&scene, d));
        let data = MeshData {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            uvs: vec![glam::Vec2::ZERO; 3],
            indices: vec![0, 1, 2],
            ranges: vec![(0, 3, 0)],
            ..Default::default()
        };
        let mesh = renderer.add_mesh(&mut scene, &data);
        let instances: Vec<usize> = [a, c, b, d]
            .iter()
            .map(|&m| renderer.add_instance(&mut scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![m]))
            .collect();
        renderer.prepare(&mut scene);
        let mut items: Vec<DrawItem> = instances
            .iter()
            .map(|&i| {
                let material = scene.instances[i].materials[0];
                DrawItem {
                    pipe: 0,
                    mesh: mesh as u32,
                    range: 0,
                    material: material as u32,
                    look: look(&scene, material),
                    entry: scene.instances[i].base,
                }
            })
            .collect();
        let (mut list, mut batches) = (Vec::new(), Vec::new());
        batch_items(&scene, &mut items, true, &mut list, &mut batches);
        assert_eq!(batches.len(), 3);
        let shared = batches
            .iter()
            .find(|b| b.instances.len() == 2)
            .expect("a and b in one batch");
        assert!(shared.material == a as u32 || shared.material == b as u32);
        let mut entries: Vec<u32> =
            list[shared.instances.start as usize..shared.instances.end as usize].to_vec();
        entries.sort_unstable();
        assert_eq!(
            entries,
            vec![
                scene.instances[instances[0]].base,
                scene.instances[instances[2]].base
            ]
        );
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn cached_bounds_follow_transforms_skinning_and_recycled_resources() {
        let adapter = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let renderer = pollster::block_on(Renderer::new_with(
            &adapter,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        scene.cache_bounds = true;
        let material = renderer.add_material(&mut scene, None, AlphaMode::Opaque, [1.0; 4], true);
        let data = MeshData {
            positions: vec![Vec3::ZERO, Vec3::X, Vec3::Y],
            normals: vec![Vec3::Z; 3],
            uvs: vec![glam::Vec2::ZERO; 3],
            indices: vec![0, 1, 2],
            ranges: vec![(0, 3, 0)],
            ..Default::default()
        };
        let mesh = renderer.add_mesh(&mut scene, &data);
        let origin = DVec3::new(1_000_000.000_001, 2_000_000.0, 12.0);
        let i = renderer.add_instance(&mut scene, mesh, origin, Mat4::IDENTITY, vec![material]);
        let check = |scene: &Scene, i: usize| {
            let inst = &scene.instances[i];
            let expected = InstanceBounds::new(&scene.meshes[inst.mesh], inst.transform);
            assert_eq!(
                Renderer::bounding_sphere(scene, inst),
                (
                    expected.centre + (inst.origin - scene.render_origin).as_vec3(),
                    expected.radius
                )
            );
            assert_eq!(Renderer::instance_scale(scene, inst), expected.scale);
        };
        renderer.prepare(&mut scene);
        check(&scene, i);
        let transform = Mat4::from_scale_rotation_translation(
            Vec3::new(-2.0, 3.0, 4.0),
            glam::Quat::from_rotation_z(0.7),
            Vec3::new(10.0, 20.0, 30.0),
        );
        renderer.set_transform(&mut scene, i, origin, transform);
        renderer.prepare(&mut scene);
        check(&scene, i);
        let posed: Vec<_> = data.positions.iter().map(|p| *p * 5.0 + Vec3::Z).collect();
        renderer.update_mesh(&mut scene, mesh, &posed, &data.normals, &data.uvs);
        renderer.prepare(&mut scene);
        check(&scene, i);
        renderer.set_render_origin(&mut scene, origin - DVec3::new(0.25, 0.5, 0.75));
        renderer.prepare(&mut scene);
        check(&scene, i);
        renderer.free_mesh(&mut scene, mesh);
        renderer.prepare(&mut scene);
        check(&scene, i);
        let new = renderer.add_mesh(&mut scene, &data);
        assert_eq!(renderer.recycle_mesh(&mut scene, new, mesh), mesh);
        renderer.prepare(&mut scene);
        check(&scene, i);
        let replacement =
            renderer.add_instance(&mut scene, mesh, origin, Mat4::IDENTITY, vec![material]);
        assert_eq!(renderer.recycle_instance(&mut scene, replacement, i), i);
        renderer.prepare(&mut scene);
        check(&scene, i);
        let other = renderer.add_mesh(&mut scene, &data);
        renderer.set_instance_mesh(&mut scene, i, other);
        renderer.prepare(&mut scene);
        check(&scene, i);
    }

    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn presurface_reveals_excavation_before_terrain_is_drawn() {
        fn quad(renderer: &Renderer, scene: &mut Scene, y: f32, half: f32) -> MeshId {
            renderer.add_mesh(
                scene,
                &MeshData {
                    positions: vec![
                        Vec3::new(-half, y, -half),
                        Vec3::new(half, y, -half),
                        Vec3::new(half, y, half),
                        Vec3::new(-half, y, half),
                    ],
                    normals: vec![-Vec3::Y; 4],
                    uvs: vec![glam::Vec2::ZERO; 4],
                    ranges: vec![(0, 6, 0)],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    one_sided: false,
                },
            )
        }
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 90.0,
            near: 0.1,
            far: 100.0,
        };
        for (msaa, ssao, enhanced) in [
            (1, false, false),
            (1, true, false),
            (1, true, true),
            (4, true, true),
        ] {
            let instance =
                wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
            let mut renderer = pollster::block_on(Renderer::new_with(
                &instance,
                None,
                Some(wgpu::TextureFormat::Rgba8UnormSrgb),
                RenderOptions {
                    msaa,
                    ssao,
                    shadow_size: 1024,
                    fxaa: false,
                    render_scale: 1.0,
                    ..Default::default()
                },
            ))
            .expect("test renderer");
            let mut scene = renderer.new_scene();
            let green = renderer.add_material(
                &mut scene,
                None,
                AlphaMode::Opaque,
                [0.0, 1.0, 0.0, 1.0],
                true,
            );
            let blue = renderer.add_material(
                &mut scene,
                None,
                AlphaMode::Opaque,
                [0.0, 0.0, 1.0, 1.0],
                true,
            );
            let red = renderer.add_material(
                &mut scene,
                None,
                AlphaMode::Opaque,
                [1.0, 0.0, 0.0, 1.0],
                true,
            );
            let texture = renderer.add_texture(
                &mut scene,
                &omsi_texture::Image {
                    width: 1,
                    height: 1,
                    rgba: vec![255, 255, 255, 0],
                    has_alpha: true,
                },
                false,
            );
            let transparent =
                renderer.add_material(&mut scene, Some(texture), AlphaMode::Blend, [1.0; 4], true);
            let cutout =
                renderer.add_material(&mut scene, Some(texture), AlphaMode::Test, [1.0; 4], true);
            // Put terrain first in the scene to catch reliance on insertion order. The
            // excavation floor is behind it, with a transparent cover in front of both.
            let terrain = quad(&renderer, &mut scene, 6.0, 10.0);
            renderer.add_instance(
                &mut scene,
                terrain,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![green],
            );
            let floor = quad(&renderer, &mut scene, 8.0, 10.0);
            let floor = renderer.add_surface_instance(
                &mut scene,
                floor,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![blue],
            );
            scene.instances[floor].presurface = true;
            let cover_mesh = quad(&renderer, &mut scene, 4.0, 1.5);
            let cover = renderer.add_surface_instance(
                &mut scene,
                cover_mesh,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![transparent],
            );
            scene.instances[cover].presurface = true;
            let foreground_mesh = quad(&renderer, &mut scene, 2.0, 0.25);
            let foreground = renderer.add_instance(
                &mut scene,
                foreground_mesh,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![red],
            );
            scene.instances[foreground].visible = false;
            let lighting = Lighting {
                enhanced,
                shadows: false,
                fog_density: 0.0,
                ..Default::default()
            };
            let pixel = |rgba: &[u8], x: usize| -> [u8; 3] {
                rgba[(32 * 64 + x) * 4..(32 * 64 + x) * 4 + 3]
                    .try_into()
                    .unwrap()
            };
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            let centre = pixel(&rgba, 32);
            assert!(
                centre[2] > centre[1] + 40,
                "floor must show through cover: {centre:?}; {msaa}/{ssao}/{enhanced}"
            );
            let outside = pixel(&rgba, 4);
            assert!(
                outside[1] > outside[2] + 40,
                "terrain outside cover: {outside:?}"
            );
            scene.instances[foreground].visible = true;
            Renderer::mark_changed(&mut scene, foreground);
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            let centre = pixel(&rgba, 32);
            assert!(
                centre[0] > centre[2] + 40,
                "foreground stays visible: {centre:?}"
            );
            scene.instances[foreground].visible = false;
            Renderer::mark_changed(&mut scene, foreground);
            // Ordinary blended surfaces must keep showing the terrain, as must covers
            // whose material explicitly disables depth writes or uses alpha testing.
            for (presurface, alpha, no_z_write) in [
                (false, AlphaMode::Blend, false),
                (true, AlphaMode::Blend, true),
                (true, AlphaMode::Test, false),
            ] {
                scene.instances[cover].presurface = presurface;
                scene.materials[transparent].no_z_write = no_z_write;
                renderer.set_material(
                    &mut scene,
                    cover,
                    0,
                    if alpha == AlphaMode::Test {
                        cutout
                    } else {
                        transparent
                    },
                );
                let rgba = renderer
                    .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                    .unwrap();
                let centre = pixel(&rgba, 32);
                assert!(
                    centre[1] > centre[2] + 40,
                    "terrain should show: {centre:?}; {presurface}/{alpha:?}/{no_z_write}"
                );
            }
        }
    }

    /// Two blended panes of one model, the near one listed first, both marked see-through
    /// (`no_z_write`) for the shading: written into the depth buffer as Omsi.exe writes it
    /// (`writes_depth`, no `[matl_noZwrite]` in the model), the far pane drawn after it is
    /// hidden behind it; left out of it (`[matl_noZwrite]`), it is blended over it (#211).
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn stacked_panes_hide_each_other_in_model_order_where_they_write_depth() {
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 90.0,
            near: 0.1,
            far: 100.0,
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let quad = |y: f32, half: f32| -> [Vec3; 4] {
            [
                Vec3::new(-half, y, -half),
                Vec3::new(half, y, -half),
                Vec3::new(half, y, half),
                Vec3::new(-half, y, half),
            ]
        };
        let wall = renderer.add_mesh(
            &mut scene,
            &MeshData {
                positions: quad(8.0, 20.0).to_vec(),
                normals: vec![-Vec3::Y; 4],
                uvs: vec![glam::Vec2::ZERO; 4],
                ranges: vec![(0, 6, 0)],
                indices: vec![0, 1, 2, 0, 2, 3],
                one_sided: false,
            },
        );
        let green = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.0, 1.0, 0.0, 1.0],
            true,
        );
        renderer.add_instance(&mut scene, wall, DVec3::ZERO, Mat4::IDENTITY, vec![green]);
        // the near pane (slot 0) and the far one (slot 1), half transparent
        let panes = renderer.add_mesh(
            &mut scene,
            &MeshData {
                positions: [quad(2.0, 4.0), quad(4.0, 8.0)].concat(),
                normals: vec![-Vec3::Y; 8],
                uvs: vec![glam::Vec2::ZERO; 8],
                ranges: vec![(0, 6, 0), (6, 6, 1)],
                indices: vec![0, 1, 2, 0, 2, 3, 4, 5, 6, 4, 6, 7],
                one_sided: false,
            },
        );
        let half = renderer.add_texture(
            &mut scene,
            &omsi_texture::Image {
                width: 1,
                height: 1,
                rgba: vec![255, 255, 255, 128],
                has_alpha: true,
            },
            false,
        );
        let near = renderer.add_material(
            &mut scene,
            Some(half),
            AlphaMode::Blend,
            [1.0, 0.0, 0.0, 1.0],
            true,
        );
        let far = renderer.add_material(
            &mut scene,
            Some(half),
            AlphaMode::Blend,
            [0.0, 0.0, 1.0, 1.0],
            true,
        );
        renderer.add_instance(
            &mut scene,
            panes,
            DVec3::ZERO,
            Mat4::IDENTITY,
            vec![near, far],
        );
        let lighting = Lighting {
            shadows: false,
            fog_density: 0.0,
            ..Default::default()
        };
        for writes_depth in [true, false] {
            for m in [near, far] {
                scene.materials[m].no_z_write = true;
                scene.materials[m].writes_depth = writes_depth;
            }
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            let c = &rgba[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 3];
            if writes_depth {
                assert!(
                    c[2] < 20 && c[0] > 60 && c[1] > 60,
                    "far pane hidden behind the near one: {c:?}"
                );
            } else {
                assert!(
                    c[2] > 40 && c[0] > 60,
                    "far pane blended over the near one: {c:?}"
                );
            }
        }
    }

    /// A `[matl_envmap]` reads its sphere map where Direct3D's D3DTSS_TCI_SPHEREMAP puts it
    /// (u = Rx/m + 0.5, v = Ry/m + 0.5, m = 2|R - (0, 0, 1)| in camera space): a pane
    /// turned 30 degrees from the view mirrors a ray 60 degrees off it, a quarter of the
    /// way out from the map's middle (0.75), not near its rim (0.93) as the u = Rx/2 + 0.5
    /// taken before had it; turned up, the same below the middle (the sky's half, #1193).
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn vanilla_sphere_map_is_read_where_direct3d_generates_it() {
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 60.0,
            near: 0.1,
            far: 100.0,
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        // the sphere map: red counts u, green v
        let size = 64usize;
        let mut rgba = Vec::with_capacity(size * size * 4);
        for y in 0..size {
            for x in 0..size {
                let at = |i: usize| ((i as f32 + 0.5) / size as f32 * 255.0).round() as u8;
                rgba.extend_from_slice(&[at(x), at(y), 0, 255]);
            }
        }
        let env = renderer.add_texture(
            &mut scene,
            &omsi_texture::Image {
                width: size as u32,
                height: size as u32,
                rgba,
                has_alpha: false,
            },
            false,
        );
        let white = renderer.add_texture(
            &mut scene,
            &omsi_texture::Image {
                width: 1,
                height: 1,
                rgba: vec![255; 4],
                has_alpha: false,
            },
            false,
        );
        let paint = renderer.add_material_env(
            &mut scene,
            Some(white),
            AlphaMode::Opaque,
            [1.0; 4],
            false,
            None,
            None,
            None,
            Some((env, 1.0)),
        );
        // a pane 5 m ahead, its normal turned 30 degrees from the view: about z (a side
        // to the right) or about x (the pane tipped back, mirroring the sky)
        let pane = |renderer: &Renderer, scene: &mut Scene, n: Vec3, along: Vec3| {
            let c = Vec3::new(0.0, 5.0, 0.0);
            let across = n.cross(along);
            let mesh = renderer.add_mesh(
                scene,
                &MeshData {
                    positions: vec![
                        c - along - across,
                        c + along - across,
                        c + along + across,
                        c - along + across,
                    ],
                    normals: vec![n; 4],
                    uvs: vec![glam::Vec2::splat(0.5); 4],
                    ranges: vec![(0, 6, 0)],
                    indices: vec![0, 1, 2, 0, 2, 3],
                    one_sided: false,
                },
            );
            renderer.add_instance(scene, mesh, DVec3::ZERO, Mat4::IDENTITY, vec![paint])
        };
        let (s, c) = 30f32.to_radians().sin_cos();
        let lighting = Lighting {
            shadows: false,
            fog_density: 0.0,
            classic: true,
            ..Default::default()
        };
        let mut read = |n: Vec3, along: Vec3| {
            let id = pane(&renderer, &mut scene, n, along);
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            scene.instances[id].visible = false;
            let p = &rgba[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 3];
            (p[0] as f32 / 255.0, p[1] as f32 / 255.0)
        };
        let (u, v) = read(Vec3::new(s, -c, 0.0), Vec3::Z);
        assert!(
            (u - 0.75).abs() < 0.03 && (v - 0.5).abs() < 0.03,
            "turned to the side: ({u}, {v})"
        );
        let (u, v) = read(Vec3::new(0.0, -c, s), Vec3::X);
        assert!(
            (u - 0.5).abs() < 0.03 && (v - 0.75).abs() < 0.03,
            "tipped back: ({u}, {v})"
        );
    }

    /// A lamp's flare behind a window that writes its depth: seen from inside the vehicle
    /// it shows through the glass, which Omsi.exe draws after the flares (0x6f0430 after
    /// 0x6f0400/0x6f0418); seen from outside the vehicle, drawn before the flares, the
    /// window hides it as in the original.
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn flares_show_through_the_glass_of_the_vehicle_the_camera_is_in() {
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 90.0,
            near: 0.1,
            far: 100.0,
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let quad = |y: f32, half: f32| MeshData {
            positions: vec![
                Vec3::new(-half, y, -half),
                Vec3::new(half, y, -half),
                Vec3::new(half, y, half),
                Vec3::new(-half, y, half),
            ],
            normals: vec![-Vec3::Y; 4],
            uvs: vec![glam::Vec2::ZERO; 4],
            ranges: vec![(0, 6, 0)],
            indices: vec![0, 1, 2, 0, 2, 3],
            one_sided: false,
        };
        let wall = renderer.add_mesh(&mut scene, &quad(30.0, 60.0));
        let black = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.0, 0.0, 0.0, 1.0],
            true,
        );
        renderer.add_instance(
            &mut scene,
            wall,
            DVec3::new(0.0, 0.0, 0.0),
            Mat4::IDENTITY,
            vec![black],
        );
        let pane = renderer.add_mesh(&mut scene, &quad(2.0, 4.0));
        let half = renderer.add_texture(
            &mut scene,
            &omsi_texture::Image {
                width: 1,
                height: 1,
                rgba: vec![255, 255, 255, 64],
                has_alpha: true,
            },
            false,
        );
        let glass = renderer.add_material(
            &mut scene,
            Some(half),
            AlphaMode::Blend,
            [0.2, 0.2, 0.2, 1.0],
            true,
        );
        scene.materials[glass].no_z_write = true;
        scene.materials[glass].writes_depth = true;
        renderer.add_instance(&mut scene, pane, DVec3::ZERO, Mat4::IDENTITY, vec![glass]);
        scene.coronas = vec![Corona {
            position: DVec3::new(0.0, 10.0, 0.0),
            size: 3.0,
            color: [1.0; 3],
            brightness: 1.0,
            ..Default::default()
        }];
        for inside in [true, false] {
            let lighting = Lighting {
                shadows: false,
                fog_density: 0.0,
                night: 1.0,
                inside: inside.then_some((DVec3::ZERO, 0.0, [6.0, 6.0, 6.0, 0.0, 0.0, 0.0])),
                ..Default::default()
            };
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            let c = &rgba[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 3];
            if inside {
                assert!(c[0] > 120, "the flare shows through the windscreen: {c:?}");
            } else {
                assert!(
                    c[0] < 80,
                    "the flare behind a bus's window seen from outside: {c:?}"
                );
            }
        }
    }

    /// Rails laid a millimetre over a road as a spline of their own, seen from the cab: the
    /// road's 10 m stretch between two cross-sections and the rails' metre-long ones are
    /// pulled towards the eye alike, so the rails lie on the road right up to the bus. With a
    /// fixed 2 cm pull per vertex the road's long triangles bowed over them near the eye and
    /// the rails went under the road there (#1196).
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn a_spline_a_millimetre_over_a_road_stays_over_it_near_the_eye() {
        let camera = Camera {
            position: DVec3::new(0.0, 0.0, 2.5),
            yaw: 0.0,
            pitch: -25.0,
            roll: 0.0,
            fov_deg: 60.0,
            near: 0.1,
            far: 200.0,
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        let mut scene = renderer.new_scene();
        let strip = |half: f32, z: f32, stations: usize| {
            let mut m = MeshData {
                ranges: vec![(0, 6 * stations as u32, 0)],
                ..Default::default()
            };
            for i in 0..=stations {
                let y = 2.0 + 10.0 * i as f32 / stations as f32;
                m.positions
                    .extend([Vec3::new(-half, y, z), Vec3::new(half, y, z)]);
                m.normals.extend([Vec3::Z; 2]);
                m.uvs.extend([glam::Vec2::ZERO; 2]);
            }
            for i in 0..stations as u32 {
                let a = 2 * i;
                m.indices.extend([a, a + 1, a + 2, a + 1, a + 3, a + 2]);
            }
            m
        };
        let road = renderer.add_mesh(&mut scene, &strip(3.0, 0.0, 1));
        let rails = renderer.add_mesh(&mut scene, &strip(0.2, 0.001, 10));
        let grey = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [0.3, 0.3, 0.3, 1.0],
            true,
        );
        let red = renderer.add_material(
            &mut scene,
            None,
            AlphaMode::Opaque,
            [1.0, 0.0, 0.0, 1.0],
            true,
        );
        for (mesh, mat) in [(road, grey), (rails, red)] {
            let i = renderer.add_surface_instance(
                &mut scene,
                mesh,
                DVec3::ZERO,
                Mat4::IDENTITY,
                vec![mat],
            );
            scene.instances[i].render_phase = RenderPhase::Spline;
        }
        let lighting = Lighting {
            shadows: false,
            fog_density: 0.0,
            ..Default::default()
        };
        let rgba = renderer
            .render_to_image(&mut scene, 128, 128, &camera, &lighting)
            .unwrap();
        // the middle column from 3 m (row 93) to 11 m (row 40) in front of the camera
        for row in 42..92 {
            let c = &rgba[(row * 128 + 64) * 4..(row * 128 + 64) * 4 + 3];
            assert!(
                c[0] > c[1] + 60,
                "row {row}: the road over the rails: {c:?}"
            );
        }
    }

    /// A `[smoke]` puff (an exhaust's, a wheel's spray) is drawn where it is, in its colour,
    /// in every graphics mode (#949, #948).
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn smoke_puffs_are_drawn() {
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 60.0,
            near: 0.1,
            far: 100.0,
        };
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let mut renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer");
        renderer.set_smoke_texture(&omsi_texture::Image {
            width: 4,
            height: 4,
            rgba: vec![255; 64],
            has_alpha: true,
        });
        let mut scene = renderer.new_scene();
        for (classic, enhanced) in [(true, false), (false, false), (false, true)] {
            scene.smoke = vec![SmokeParticle {
                position: DVec3::new(0.0, 5.0, 0.0),
                size: 1.0,
                color: [1.0, 0.0, 0.0],
                alpha: 1.0,
                ..Default::default()
            }];
            let lighting = Lighting {
                shadows: false,
                fog_density: 0.0,
                classic,
                enhanced,
                ..Default::default()
            };
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            let c = &rgba[(32 * 64 + 32) * 4..(32 * 64 + 32) * 4 + 3];
            assert!(
                c[0] > 120 && c[1] < 60 && c[2] < 60,
                "a red puff 5 m ahead (classic {classic}, enhanced {enhanced}): {c:?}"
            );
        }
    }

    fn smoke_test_renderer() -> Renderer {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                ssao: false,
                shadow_size: 1024,
                fxaa: false,
                render_scale: 1.0,
                ..Default::default()
            },
        ))
        .expect("test renderer")
    }

    /// A puff that knows the ground under it fades out into it over its lowest part - no
    /// line where the road would cut it off - and keeps its colour higher up, in every
    /// graphics mode; one wholly under its ground is not drawn.
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn a_smoke_puff_fades_into_its_ground() {
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 60.0,
            near: 0.1,
            far: 100.0,
        };
        let mut renderer = smoke_test_renderer();
        renderer.set_smoke_texture(&omsi_texture::Image {
            width: 4,
            height: 4,
            rgba: vec![255; 64],
            has_alpha: true,
        });
        let mut scene = renderer.new_scene();
        let red = |row: usize, rgba: &[u8]| rgba[(row * 64 + 32) * 4] as i32;
        for (classic, enhanced) in [(true, false), (false, false), (false, true)] {
            let lighting = Lighting {
                shadows: false,
                fog_density: 0.0,
                classic,
                enhanced,
                ..Default::default()
            };
            // 5 m ahead with the ground at its middle: drawn 0.9 m round, a pixel row 9 cm
            // there (60 degrees over 64 rows); the fade reaches 0.18 m up
            let profile = |scene: &mut Scene, renderer: &mut Renderer| -> Vec<i32> {
                let rgba = renderer
                    .render_to_image(scene, 64, 64, &camera, &lighting)
                    .unwrap();
                let bg = red(56, &rgba);
                (24..=40).map(|row| red(row, &rgba) - bg).collect()
            };
            scene.smoke = vec![SmokeParticle {
                position: DVec3::new(0.0, 5.0, 0.0),
                size: 1.0,
                color: [1.0, 0.0, 0.0],
                alpha: 1.0,
                ..Default::default()
            }];
            let plain = profile(&mut scene, &mut renderer);
            scene.smoke[0].ground = Some(0.0);
            let faded = profile(&mut scene, &mut renderer);
            let (at, full) = (|row: usize| faded[row - 24], |row: usize| plain[row - 24]);
            let mode =
                format!("classic {classic}, enhanced {enhanced}: {faded:?} against {plain:?}");
            assert!(
                full(25) > 30 && full(33) > 30,
                "the plain puff is drawn ({mode})"
            );
            assert!(
                at(34).abs() < 4 && at(38).abs() < 4,
                "under its ground ({mode})"
            );
            assert!(
                at(31) * 2 < full(31),
                "a few centimetres over the ground it is all but gone ({mode})"
            );
            assert!(
                (at(28) - full(28)).abs() <= 3 && (at(25) - full(25)).abs() <= 3,
                "30 cm up it is as it was ({mode})"
            );
            for row in 28..31 {
                assert!(
                    at(row) >= at(row + 1) - 2,
                    "fading down into the ground, no step ({mode})"
                );
            }
            // and one sunk wholly into its ground is left out
            scene.smoke[0].ground = Some(0.95);
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            assert!(
                (red(30, &rgba) - red(56, &rgba)).abs() < 6,
                "a puff under its ground drawn (classic {classic}, enhanced {enhanced})"
            );
        }
    }

    /// A puff's picture is turned by its spin about the line of sight (Omsi.exe turns every
    /// one its own way): the top half of rauch.tga comes to the bottom at half a turn.
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn a_smoke_puffs_picture_turns_with_its_spin() {
        let camera = Camera {
            position: DVec3::ZERO,
            yaw: 0.0,
            pitch: 0.0,
            roll: 0.0,
            fov_deg: 60.0,
            near: 0.1,
            far: 100.0,
        };
        let mut renderer = smoke_test_renderer();
        let mut rgba = vec![255u8; 64];
        for px in 8..16 {
            rgba[px * 4 + 3] = 0;
        }
        renderer.set_smoke_texture(&omsi_texture::Image {
            width: 4,
            height: 4,
            rgba,
            has_alpha: true,
        });
        let mut scene = renderer.new_scene();
        let red = |row: usize, rgba: &[u8]| rgba[(row * 64 + 32) * 4] as i32;
        let lighting = Lighting {
            shadows: false,
            fog_density: 0.0,
            classic: true,
            ..Default::default()
        };
        for (spin, top) in [(0.0, true), (std::f32::consts::PI, false)] {
            scene.smoke = vec![SmokeParticle {
                position: DVec3::new(0.0, 5.0, 0.0),
                size: 1.0,
                color: [1.0, 0.0, 0.0],
                alpha: 1.0,
                spin,
                ..Default::default()
            }];
            let rgba = renderer
                .render_to_image(&mut scene, 64, 64, &camera, &lighting)
                .unwrap();
            let bg = red(56, &rgba);
            let (up, down) = (red(25, &rgba) - bg, red(39, &rgba) - bg);
            assert_eq!(
                (up > 60, down > 60),
                (top, !top),
                "spin {spin}: above {up}, below {down}"
            );
        }
    }

    /// A smoke particle's sprite: its spin, its ground relative to the render origin and its
    /// z offset where the smoke branch of the shader reads them; none when it is too faint or
    /// wholly under its ground.
    #[test]
    fn a_smoke_sprite_carries_its_spin_ground_and_z_offset() {
        let ro = DVec3::new(1000.0, 2000.0, 30.0);
        let p = SmokeParticle {
            position: ro + DVec3::new(1.0, 2.0, 3.5),
            size: 1.0,
            color: [0.5; 3],
            alpha: 0.8,
            spin: std::f32::consts::FRAC_PI_2,
            z_offset: 0.1,
            ground: Some(33.0),
        };
        let g = smoke_sprite(&p, ro).unwrap();
        assert_eq!(g.pos, [1.0, 2.0, 3.5]);
        assert_eq!(g.extra[3], 3.0);
        assert!(
            g.dir[0].abs() < 1e-6 && (g.dir[1] - 1.0).abs() < 1e-6,
            "{:?}",
            g.dir
        );
        assert_eq!(g.up[..3], [3.0, 1.0, 0.1]);
        assert!((g.up[3] - 0.18).abs() < 1e-6, "fade {}", g.up[3]);
        // the fade into the ground: a fifth of the drawn radius, 6 to 25 cm
        assert_eq!(
            (smoke_ground_fade(0.1), smoke_ground_fade(5.0)),
            (0.06, 0.25)
        );
        // no ground: not faded
        let free = smoke_sprite(&SmokeParticle { ground: None, ..p }, ro).unwrap();
        assert_eq!(free.up[1], 0.0);
        // its top (drawn at 0.9 of its half width) under the ground: left out
        assert!(smoke_sprite(
            &SmokeParticle {
                ground: Some(30.0 + 3.5 + 0.91),
                ..p
            },
            ro
        )
        .is_none());
        assert!(smoke_sprite(
            &SmokeParticle {
                ground: Some(30.0 + 3.5 + 0.89),
                ..p
            },
            ro
        )
        .is_some());
        assert!(smoke_sprite(&SmokeParticle { alpha: 0.0, ..p }, ro).is_none());
    }

    #[test]
    fn noop_backend_initializes_renderer() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
        let instance = wgpu::Instance::new(descriptor);
        let res = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 2,
                shadow_size: 1024,
                ..Default::default()
            },
        ));
        assert!(
            res.is_ok(),
            "renderer should initialize on noop backend: {:?}",
            res.err()
        );
    }

    #[test]
    fn declared_transmap_ignores_slot_alpha() {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
        let instance = wgpu::Instance::new(descriptor);
        let renderer = pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("noop renderer");
        let mut scene = renderer.new_scene();
        let blended =
            |scene: &mut Scene, transmap: Option<(TextureId, bool)>, extra: MaterialExtra| {
                renderer.add_material_extra(
                    scene,
                    None,
                    AlphaMode::Blend,
                    [1.0; 4],
                    true,
                    transmap,
                    None,
                    None,
                    None,
                    [0.0; 3],
                    extra,
                )
            };
        // declared, its file missing: no transmap texture bound
        let declared = blended(
            &mut scene,
            None,
            MaterialExtra {
                transmap_declared: true,
                ..Default::default()
            },
        );
        let map = renderer.add_blank_texture(&mut scene, 1, 1);
        let bound = blended(&mut scene, Some((map, true)), MaterialExtra::default());
        // another bit of the same flags, no transmap
        let metal = blended(
            &mut scene,
            None,
            MaterialExtra {
                metal_ok: true,
                ..Default::default()
            },
        );
        let plain = blended(&mut scene, None, MaterialExtra::default());
        assert_eq!(scene.materials[metal].uniform.params2[3], 4.0);
        let flags: Vec<bool> = [declared, bound, metal, plain]
            .iter()
            .map(|&m| scene.materials[m].transmap_declared())
            .collect();
        assert_eq!(flags, [true, true, false, false]);
        let corner = [Vec3::ZERO, Vec3::X, Vec3::Y];
        let data = MeshData {
            positions: corner.repeat(4),
            normals: vec![Vec3::Z; 12],
            uvs: vec![glam::Vec2::ZERO; 12],
            indices: (0..12).collect(),
            ranges: (0..4).map(|s| (s * 3, 3, s)).collect(),
            ..Default::default()
        };
        let mesh = renderer.add_mesh(&mut scene, &data);
        let i = renderer.add_instance(
            &mut scene,
            mesh,
            DVec3::ZERO,
            Mat4::IDENTITY,
            vec![declared, bound, metal, plain],
        );
        renderer.set_params(&mut scene, i, &[0.0, 0.0, 0.35, 0.35], true, &[]);
        assert_eq!(scene.instances[i].slot_alpha, vec![1.0, 1.0, 0.35, 0.35]);
    }

    #[test]
    fn omsi_render_phases_are_monotonic_and_complete() {
        assert_eq!(
            RenderPhase::DRAW_ORDER.map(|phase| phase as usize),
            [0, 1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(RenderPhase::default(), RenderPhase::Normal);
    }

    #[test]
    fn metric_lifted_surfaces_keep_shading_class_without_view_space_pull() {
        assert_eq!(surface_instance_code(false, false, false, true, false), 0.9);
        assert_eq!(surface_instance_code(false, false, false, true, true), 1.0);
        assert_eq!(surface_instance_code(false, false, true, false, false), 0.9);
        assert_eq!(surface_instance_code(false, true, false, true, false), 0.75);
    }

    #[test]
    fn spline_blend_sort_ignores_height_and_camera_pitch() {
        let origin = DVec3::new(120.0, 45.0, 0.0);
        let render_origin = DVec3::new(100.0, 40.0, 0.0);
        let level_camera = Vec3::new(3.0, 1.0, 4.0);
        let high_camera = Vec3::new(3.0, 1.0, 80.0);
        assert_eq!(
            horizontal_sort_distance(origin, render_origin, level_camera),
            horizontal_sort_distance(origin + DVec3::Z * 60.0, render_origin, high_camera)
        );
    }

    #[test]
    fn render_scale_auto_keeps_ordinary_windows_sharp() {
        // the default window and a 2560x1080 screen are drawn at full size
        assert_eq!(scene_scale_for(0.0, 1600, 900), 1.0);
        assert_eq!(scene_scale_for(0.0, 2560, 1080), 1.0);
        // Mac/Android cap this at 2.8 million pixels; desktops keep it at full size.
        let s = scene_scale_for(0.0, 3200, 1800);
        if cfg!(target_os = "macos") || cfg!(target_os = "android") {
            assert!((s - 0.697).abs() < 0.01, "{s}");
            assert!((3200.0 * s * 1800.0 * s - AUTO_SCALE_PIXELS).abs() < 1.0);
        } else {
            assert_eq!(s, 1.0);
        }
        // never below half size
        assert_eq!(scene_scale_for(0.0, 16384, 16384), 0.5);
        // what is asked for, within 0.5..1 - on a Mac or a phone a scale over the pixel
        // budget is capped like the automatic one
        let s = scene_scale_for(0.75, 3200, 1800);
        if cfg!(target_os = "macos") || cfg!(target_os = "android") {
            assert!((s - 0.697).abs() < 0.01, "{s}");
            assert!((3200.0 * s * 1800.0 * s - AUTO_SCALE_PIXELS).abs() < 1.0);
        } else {
            assert_eq!(s, 0.75);
        }
        // a small window under the budget keeps what it asked for
        assert_eq!(scene_scale_for(0.3, 1600, 900), 0.5);
        assert_eq!(scene_scale_for(1.4, 1600, 900), 1.0);
    }

    #[test]
    fn surface_depth_coverage_excludes_glass_and_terrain_masks() {
        assert!(surface_depth_coverage(
            RenderPhase::Spline,
            AlphaMode::Blend,
            false,
            false
        ));
        for phase in RenderPhase::DRAW_ORDER {
            if !world_surface_phase(phase) {
                assert!(!surface_depth_coverage(
                    phase,
                    AlphaMode::Blend,
                    false,
                    false
                ));
            } else {
                assert!(surface_depth_coverage(
                    phase,
                    AlphaMode::Blend,
                    false,
                    false
                ));
            }
        }
        assert!(!surface_depth_coverage(
            RenderPhase::Spline,
            AlphaMode::Blend,
            true,
            false
        ));
        assert!(!surface_depth_coverage(
            RenderPhase::Spline,
            AlphaMode::Blend,
            false,
            true
        ));
        assert!(!surface_depth_coverage(
            RenderPhase::Spline,
            AlphaMode::Opaque,
            false,
            false
        ));
        assert!(!surface_depth_coverage(
            RenderPhase::Spline,
            AlphaMode::Test,
            false,
            false
        ));
        let order = RenderPhase::DRAW_ORDER;
        assert!(
            order
                .iter()
                .position(|p| *p == RenderPhase::OnSurface)
                .unwrap()
                < order
                    .iter()
                    .position(|p| *p == RenderPhase::BeforeNormal)
                    .unwrap()
        );
    }

    #[test]
    fn pipeline_codes_cover_the_table() {
        // every kind x culling x depth bias has its own pipeline, and the opaque and
        // alpha-tested codes sort before the blended ones
        let mut seen = std::collections::HashSet::new();
        for kind in 0..PIPE_KINDS {
            for cull in [false, true] {
                for surface in [false, true] {
                    let code = pipe_code(kind, cull, surface);
                    assert!((code as usize) < PIPE_KINDS as usize * 4);
                    assert!(seen.insert(code));
                    assert_eq!(
                        code < pipe_code(PIPE_BLEND, false, false),
                        kind < PIPE_BLEND
                    );
                }
            }
        }
        assert_eq!(seen.len(), PIPE_KINDS as usize * 4);
        assert_eq!(pipe_code(PIPE_OPAQUE, false, false), 0);
        assert!(
            pipe_code(PIPE_ALPHA_TEST, true, true) < pipe_code(PIPE_BLEND_NO_WRITE, false, false)
        );
    }

    #[test]
    fn nearest_by_origin_picks_the_closest_mesh_of_each_object() {
        // A long bus (one origin) has a window 3 m from the camera and another 14 m away;
        // ranking the whole object by its shared origin (here 8 m) instead of its nearest
        // blended mesh is exactly what let a car beside the near window sort as farther
        // away than the bus and disappear behind it.
        let bus = DVec3::new(0.0, 0.0, 0.0);
        let car = DVec3::new(1.0, 0.0, 0.0);
        let by_origin = nearest_by_origin([(bus, 14.0), (bus, 3.0), (car, 8.0)]);
        let bus_dist = by_origin[&origin_key(bus)];
        let car_dist = by_origin[&origin_key(car)];
        assert_eq!(
            bus_dist, 3.0,
            "the object's distance is its nearest mesh, not the first or an average"
        );
        assert_eq!(car_dist, 8.0);
        // the car (8 m) is nearer than the bus's near window (3 m) is far: with the old
        // origin-only distance (bus origin 8.5 m, say) the two could tie or invert
        assert!(car_dist > bus_dist);
        assert_eq!(
            by_origin.len(),
            2,
            "one entry per distinct origin, not per mesh"
        );
    }

    #[test]
    fn origin_lookup_keeps_float_equality_and_large_coordinates() {
        let a = DVec3::new(-0.0, 1_000_000.000_001, 2.0);
        let b = DVec3::new(0.0, a.y, 2.0);
        let c = DVec3::new(0.0, a.y + 0.000_001, 2.0);
        let distances = nearest_by_origin([(a, 8.0), (b, 3.0), (c, 5.0), (DVec3::NAN, 1.0)]);
        assert_eq!(distances.len(), 2);
        assert_eq!(distances[&origin_key(a)], 3.0);
        assert_eq!(distances[&origin_key(c)], 5.0);
    }

    /// Every shader module parses and validates as the device will see it, translates to
    /// Metal and Vulkan SPIR-V, and its uniform structs are laid out as the Rust side writes them.
    #[test]
    fn shaders_validate_and_match_the_uniforms() {
        use wgpu::naga;
        let modules = [
            ("scene", scene_shader_source(false)),
            ("scene opaque", scene_shader_source(false)),
            ("scene terrain paint", scene_shader_source(false)),
            ("sky", sky_shader_source()),
            ("corona", corona_shader_source()),
            ("fog lamps", fog_lamps_shader_source()),
            ("snow", snow_shader_source()),
            ("post", include_str!("post.wgsl").to_string()),
            ("ssao", include_str!("ssao.wgsl").to_string()),
            ("puddles", puddles::shader_source()),
            ("upscale", include_str!("upscale.wgsl").to_string()),
            ("mip", include_str!("mip.wgsl").to_string()),
            ("xr_ui", include_str!("xr_ui.wgsl").to_string()),
            // Enhanced+: the scene's pass with its reflection targets, the ray tracing
            (
                "scene (Enhanced+)",
                scene_shader_source(false).replace("//RT ", ""),
            ),
            ("ray-traced lighting", rt::lighting_source()),
            ("ray-traced reflections", rt::reflect_source()),
            ("texture means", include_str!("rt_avg.wgsl").to_string()),
        ];
        let sizes: &[(&str, usize)] = &[
            ("Enhanced", std::mem::size_of::<EnhancedUniform>()),
            ("PostParams", std::mem::size_of::<PostUniform>()),
            ("FogLampParams", std::mem::size_of::<FogLampUniform>()),
            ("SnowParams", std::mem::size_of::<SnowUniform>()),
            ("PuddleParams", std::mem::size_of::<puddles::Uniform>()),
            (
                "VehicleReflection",
                std::mem::size_of::<puddles::VehicleUniform>(),
            ),
            ("PointLight", std::mem::size_of::<GpuPointLight>()),
            ("Camera", std::mem::size_of::<CameraUniform>()),
            ("MaterialParams", std::mem::size_of::<MaterialUniform>()),
            ("RtParams", rt::PARAMS_SIZE),
        ];
        let mut checked = std::collections::HashSet::new();
        for (name, src) in &modules {
            let module = naga::front::wgsl::parse_str(src)
                .unwrap_or_else(|e| panic!("{name}: {}", e.emit_to_string(src)));
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
            // Exercise the painted-ground specialization as well as the default shader.
            let mut constants = naga::back::PipelineConstants::default();
            if name.starts_with("scene ") {
                constants.insert("ALPHA_TEST".into(), 0.0);
            }
            if *name == "scene terrain paint" {
                constants.insert("TERRAIN_PAINT".into(), 1.0);
            }
            let (module, info) =
                naga::back::pipeline_constants::process_overrides(&module, &info, None, &constants)
                    .unwrap_or_else(|e| panic!("{name}: overrides: {e:?}"));
            let (module, info) = (module.into_owned(), info.into_owned());
            #[cfg(target_os = "macos")]
            let options = naga::back::msl::Options {
                lang_version: (2, 4),
                ..Default::default()
            };
            #[cfg(target_os = "macos")]
            naga::back::msl::write_string(
                &module,
                &info,
                &options,
                &naga::back::msl::PipelineOptions::default(),
            )
            .unwrap_or_else(|e| panic!("{name}: Metal: {e:?}"));
            for entry in &module.entry_points {
                let pipeline = naga::back::spv::PipelineOptions {
                    shader_stage: entry.stage,
                    entry_point: entry.name.clone(),
                };
                naga::back::spv::write_vec(&module, &info, &Default::default(), Some(&pipeline))
                    .unwrap_or_else(|e| panic!("{name}/{}: Vulkan: {e:?}", entry.name));
            }
            let mut layouter = naga::proc::Layouter::default();
            layouter.update(module.to_ctx()).expect("layout");
            for (ty_name, rust) in sizes {
                if *ty_name == "Camera" && *name == "corona" {
                    // Corona owns just the prefix through `cam_up`.  It must still include
                    // every member in that prefix: a missing `world_origin`, for example,
                    // makes a subsequently-added lighting use read the wrong vec4.
                    if let Some((h, _)) = module
                        .types
                        .iter()
                        .find(|(_, t)| t.name.as_deref() == Some(*ty_name))
                    {
                        assert_eq!(
                            layouter[h].size as usize,
                            std::mem::offset_of!(CameraUniform, clouds),
                            "corona: Camera prefix"
                        );
                    }
                    continue;
                }
                if let Some((h, _)) = module
                    .types
                    .iter()
                    .find(|(_, t)| t.name.as_deref() == Some(*ty_name))
                {
                    assert_eq!(layouter[h].size as usize, *rust, "{name}: {ty_name}");
                    checked.insert(*ty_name);
                }
            }
        }
        assert_eq!(checked.len(), sizes.len(), "structs checked: {checked:?}");
    }

    /// The scene module as the OpenGL backend gets it translates to GLSL ES 3.10 and desktop
    /// GLSL 4.30 for every entry point, with no `invariant gl_FragCoord` (rejected by AMD's
    /// desktop GL and by GLES) and no texture read through two samplers (#617, #610).
    #[test]
    fn the_scene_shader_translates_to_glsl() {
        use wgpu::naga;
        use wgpu::naga::back::glsl;
        let src = scene_shader_source(true);
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("validate");
        let constants = [("ALPHA_TEST".into(), 0.0), ("TERRAIN_PAINT".into(), 1.0)]
            .into_iter()
            .collect();
        let (module, info) =
            naga::back::pipeline_constants::process_overrides(&module, &info, None, &constants)
                .expect("overrides");
        for version in [
            glsl::Version::Embedded {
                version: 310,
                is_webgl: false,
            },
            glsl::Version::Desktop(430),
        ] {
            let options = glsl::Options {
                version,
                ..Default::default()
            };
            for entry in &module.entry_points {
                let pipeline = glsl::PipelineOptions {
                    shader_stage: entry.stage,
                    entry_point: entry.name.clone(),
                    multiview: None,
                };
                let mut out = String::new();
                glsl::Writer::new(
                    &mut out,
                    &module,
                    &info,
                    &options,
                    &pipeline,
                    Default::default(),
                )
                .and_then(|mut w| w.write())
                .unwrap_or_else(|e| panic!("{version:?} {}: {e:?}", entry.name));
                assert!(
                    !out.contains("invariant gl_FragCoord"),
                    "{version:?} {}",
                    entry.name
                );
            }
        }
    }

    /// The corona and smoke module (its smoke branch with the fade into the ground among
    /// them) translates to the GLSL of the oldest OpenGL chips the renderer runs on, GLES 3.0
    /// and GL 3.3, as to GLES 3.1 and GL 4.3.
    #[test]
    fn the_corona_shader_translates_to_glsl() {
        use wgpu::naga;
        use wgpu::naga::back::glsl;
        let src = corona_shader_source();
        let module = naga::front::wgsl::parse_str(&src)
            .unwrap_or_else(|e| panic!("{}", e.emit_to_string(&src)));
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .expect("validate");
        let (module, info) = naga::back::pipeline_constants::process_overrides(
            &module,
            &info,
            None,
            &Default::default(),
        )
        .expect("overrides");
        let versions = [
            glsl::Version::Embedded {
                version: 300,
                is_webgl: false,
            },
            glsl::Version::Desktop(330),
            glsl::Version::Embedded {
                version: 310,
                is_webgl: false,
            },
            glsl::Version::Desktop(430),
        ];
        for version in versions {
            let options = glsl::Options {
                version,
                ..Default::default()
            };
            let mut done = 0;
            for entry in module.entry_points.iter().filter(|e| {
                ["vs_main", "fs_main", "fs_smoke", "fs_smoke_enhanced"].contains(&e.name.as_str())
            }) {
                let pipeline = glsl::PipelineOptions {
                    shader_stage: entry.stage,
                    entry_point: entry.name.clone(),
                    multiview: None,
                };
                let mut out = String::new();
                glsl::Writer::new(
                    &mut out,
                    &module,
                    &info,
                    &options,
                    &pipeline,
                    Default::default(),
                )
                .and_then(|mut w| w.write())
                .unwrap_or_else(|e| panic!("{version:?} {}: {e:?}", entry.name));
                done += 1;
            }
            assert_eq!(done, 4, "{version:?}");
        }
    }

    /// Where the device cannot read storage buffers in a vertex shader, or none at all, the
    /// scene module reads its arrays from textures: it validates and translates to the GLSL
    /// of such chips (GLES 3.1 / GL 4.3 with storage for the lights, GLES 3.0 / GL 3.3
    /// without), and nothing of it is a storage buffer where it must not be (#770, #316).
    #[test]
    fn the_scene_shader_reads_its_arrays_from_textures_without_vertex_storage() {
        use wgpu::naga;
        use wgpu::naga::back::glsl;
        for (path, versions) in [
            (
                ArrayPath::VertexTextures,
                [
                    glsl::Version::Embedded {
                        version: 310,
                        is_webgl: false,
                    },
                    glsl::Version::Desktop(430),
                ],
            ),
            (
                ArrayPath::NoStorage,
                [
                    glsl::Version::Embedded {
                        version: 300,
                        is_webgl: false,
                    },
                    glsl::Version::Desktop(330),
                ],
            ),
        ] {
            let src = arrays_as_textures(&scene_shader_text(true), path);
            assert!(
                !src.contains("models[")
                    && !src.contains("inst_params[")
                    && !src.contains("draw_list[")
            );
            assert_eq!(
                src.contains("var<storage"),
                path == ArrayPath::VertexTextures,
                "{path:?}"
            );
            let module = naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("{path:?}: {}", e.emit_to_string(&src)));
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap_or_else(|e| panic!("{path:?}: {e:?}"));
            let (module, info) = naga::back::pipeline_constants::process_overrides(
                &module,
                &info,
                None,
                &Default::default(),
            )
            .expect("overrides");
            for version in versions {
                let options = glsl::Options {
                    version,
                    ..Default::default()
                };
                for entry in &module.entry_points {
                    let pipeline = glsl::PipelineOptions {
                        shader_stage: entry.stage,
                        entry_point: entry.name.clone(),
                        multiview: None,
                    };
                    let mut out = String::new();
                    glsl::Writer::new(
                        &mut out,
                        &module,
                        &info,
                        &options,
                        &pipeline,
                        Default::default(),
                    )
                    .and_then(|mut w| w.write())
                    .unwrap_or_else(|e| panic!("{path:?} {version:?} {}: {e:?}", entry.name));
                    let vertex = entry.stage == naga::ShaderStage::Vertex;
                    if vertex || path == ArrayPath::NoStorage {
                        assert!(
                            !out.contains(" buffer "),
                            "{path:?} {version:?} {}: a storage block",
                            entry.name
                        );
                    }
                }
            }
        }
        // the rewrite leaves names that only end alike alone, and nests
        assert_eq!(
            indexing_as_calls(
                "a = my_models[1]; b = models[models[i + 1u] .x];",
                "models",
                "f"
            ),
            "a = my_models[1]; b = f(f(i + 1u) .x);"
        );
    }

    /// wgpu's OpenGL backend numbers the textures of all a pipeline's groups into sixteen
    /// units: the scene's camera and material groups stay within them on every array path
    /// there, and what is left out of the camera group for it no pipeline made there reads
    /// (#1133, #1154, #1162, #1176).
    #[test]
    fn the_scene_groups_fit_the_sixteen_texture_units_of_opengl() {
        use wgpu::naga;
        let textures = |e: &[wgpu::BindGroupLayoutEntry]| {
            e.iter()
                .filter(|e| matches!(e.ty, wgpu::BindingType::Texture { .. }))
                .count()
        };
        let material = textures(&material_layout_entries());
        for path in [
            ArrayPath::Storage,
            ArrayPath::VertexTextures,
            ArrayPath::NoStorage,
        ] {
            let sixteen = path != ArrayPath::Storage;
            let n = textures(&camera_layout_entries(path, sixteen)) + material;
            assert!(n <= 16, "{path:?}: {n} textures");
        }
        // (and nothing is left out where storage buffers carry the arrays)
        assert_eq!(
            textures(&camera_layout_entries(ArrayPath::Storage, false)) + material,
            16
        );
        // the entry points that read a texture left out are the enhanced path's, the
        // probe's, the puddles' and the reflection pass's - none made there
        let left_out = |src: &str| -> Vec<String> {
            let module = naga::front::wgsl::parse_str(src).unwrap();
            let info = naga::valid::Validator::new(
                naga::valid::ValidationFlags::all(),
                naga::valid::Capabilities::all(),
            )
            .validate(&module)
            .unwrap();
            let mut out = Vec::new();
            for (i, ep) in module.entry_points.iter().enumerate() {
                let used = info.get_entry_point(i);
                let reads = module.global_variables.iter().any(|(h, g)| {
                    !used[h].is_empty()
                        && g.binding.as_ref().is_some_and(|b| {
                            b.group == 0 && ENHANCED_CAMERA_TEXTURES.contains(&b.binding)
                        })
                });
                if reads {
                    out.push(ep.name.clone());
                }
            }
            out
        };
        let only_enhanced = |name: &str| {
            name.contains("enhanced")
                || name.contains("probe")
                || name.contains("puddle")
                || name.contains("reflections")
                || name == "fs_sky_cube"
        };
        for path in [ArrayPath::VertexTextures, ArrayPath::NoStorage] {
            let scene = arrays_as_textures(&scene_shader_text(true), path);
            for src in [scene, sky_shader_source(), corona_shader_source()] {
                for name in left_out(&src) {
                    assert!(
                        only_enhanced(&name),
                        "{path:?}: {name} reads an enhanced texture"
                    );
                }
            }
        }
    }

    /// An array texture is written in the rest of a row, whole rows and the start of the
    /// last row, and never past its end.
    #[test]
    fn array_texture_writes_cover_the_range_once() {
        let w = ARRAY_TEX_WIDTH;
        assert_eq!(array_tex_spans(0, 10, 4 * w as u64), vec![(0, 0, 10, 1)]);
        assert_eq!(
            array_tex_spans(w as u64 - 2, 5, 4 * w as u64),
            vec![(w - 2, 0, 2, 1), (0, 1, 3, 1)]
        );
        assert_eq!(
            array_tex_spans(5, 3 * w as u64, 4 * w as u64),
            vec![(5, 0, w - 5, 1), (0, 1, w, 2), (0, 3, 5, 1)]
        );
        assert_eq!(
            array_tex_spans(0, 2 * w as u64, 4 * w as u64),
            vec![(0, 0, w, 2)]
        );
        // past the end: cut
        assert_eq!(
            array_tex_spans(2 * w as u64 - 1, 10, 2 * w as u64),
            vec![(w - 1, 1, 1, 1)]
        );
        assert!(array_tex_spans(3 * w as u64, 10, 2 * w as u64).is_empty());
        for (at, n) in [(0u64, 1u64), (7, 9000), (2047, 2049), (4096, 4096)] {
            let covered: u64 = array_tex_spans(at, n, 64 * w as u64)
                .iter()
                .map(|s| s.2 as u64 * s.3 as u64)
                .sum();
            assert_eq!(covered, n);
        }
    }

    #[test]
    fn the_sky_is_recomputed_only_when_it_has_moved_on() {
        let a = atmosphere::SkyInput::default();
        assert!(!sky_input_differs(&a, &a));
        // the sun a hundredth of a degree on, a tint that drifts by a thousandth: the same sky
        let turn = |deg: f32| {
            glam::Quat::from_axis_angle(a.sun_dir.any_orthonormal_vector(), deg.to_radians())
                * a.sun_dir
        };
        assert!(!sky_input_differs(
            &a,
            &atmosphere::SkyInput {
                sun_dir: turn(0.01),
                tint: [Vec3::splat(1.001); 3],
                ..a
            }
        ));
        // a quarter of a degree, some rain, another envir.cfg: a new sky
        assert!(sky_input_differs(
            &a,
            &atmosphere::SkyInput {
                sun_dir: turn(0.25),
                ..a
            }
        ));
        assert!(sky_input_differs(
            &a,
            &atmosphere::SkyInput { rain: 0.3, ..a }
        ));
        assert!(sky_input_differs(
            &a,
            &atmosphere::SkyInput {
                tint: [Vec3::new(1.2, 1.0, 0.9), Vec3::ONE, Vec3::ONE],
                ..a
            }
        ));
    }

    #[test]
    fn each_path_gets_its_own_headlights() {
        let lamp = PointLight {
            radius: 30.0,
            color: [1.0, 0.78, 0.46],
            core: 5.0,
            ..Default::default()
        };
        let stand_in = PointLight {
            radius: 18.0,
            intensity: 0.8,
            mode: LightMode::Vanilla,
            ..Default::default()
        };
        let spot = PointLight {
            radius: 60.0,
            intensity: 20.0,
            direction: Vec3::new(0.0, 2.0, -0.6),
            cone: [0.97, 0.82],
            core: 1.0,
            beam: 24.0,
            mode: LightMode::Enhanced,
            ..Default::default()
        };
        assert!(drawn_by(&lamp, false) && drawn_by(&lamp, true));
        assert!(drawn_by(&stand_in, false) && !drawn_by(&stand_in, true));
        assert!(!drawn_by(&spot, false) && drawn_by(&spot, true));
        assert!(!drawn_by(
            &PointLight {
                intensity: 0.0,
                ..lamp
            },
            true
        ));
        // a map light reads as before in the vanilla shader (position, radius, colour)
        let g = gpu_light(&lamp, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(g.pos, [1.0, 2.0, 3.0, 30.0]);
        assert_eq!(g.color, [1.0, 0.78, 0.46, 1.0]);
        assert_eq!(g.dir[3], -2.0);
        assert_eq!(g.extra[1..], [5.0, 0.0, 30.0]);
        // a spot: no radius for the vanilla shader, the cone and the range for the enhanced
        let g = gpu_light(&spot, Vec3::ZERO);
        assert_eq!(g.pos[3], 0.0);
        assert_eq!(g.extra[3], 60.0);
        assert!(
            (Vec3::from_slice(&g.dir[..3]).length() - 1.0).abs() < 1e-5
                && g.dir[3] == 0.82
                && g.extra[0] == 0.97
        );
        // a headlight's beam gain rides along; a plain lamp has none
        assert_eq!(g.extra[2], 24.0);
        assert_eq!(gpu_light(&lamp, Vec3::ZERO).extra[2], 0.0);
    }

    #[test]
    fn metering_defaults_are_gentle() {
        // the light model's exposure leads: the metering corrects well under a stop either
        // way, so snow stays white and a night stays dark
        let m = meter_tuning();
        assert!(m[0] > 0.0 && m[0] < 0.6, "{m:?}");
        assert!(m[2] <= 2.5 && m[3] <= 2.0, "{m:?}");
        // a snow field metered 1.5 EV over the target is darkened by well under a stop
        let ev = ((m[1] - (m[1] + 1.5)) * m[0]).clamp(-m[2], m[3]);
        assert!(ev < 0.0 && ev >= -0.75, "{ev}");
    }

    #[test]
    fn half_floats_read_back() {
        for v in [0.0f32, 1.0, -2.5, 0.5, 1e-3, -3.46] {
            assert!(
                (half_to_f32(atmosphere::f16_bits(v)) - v).abs() <= v.abs() * 1e-3 + 1e-6,
                "{v}"
            );
        }
    }

    #[test]
    fn vehicle_box_contains_the_driver() {
        // an NL202: 2.5 x 12 x 3 m, the box centre 0.4 m ahead of the origin and 1.5 m up,
        // heading east; the driver sits 4.6 m ahead of the origin, 1.8 m up
        let bus = (
            DVec3::new(100.0, 200.0, 30.0),
            90.0,
            [2.5, 12.0, 3.0, 0.0, 0.4, 1.5],
        );
        assert!(point_in_vehicle_box(DVec3::new(104.6, 200.7, 31.8), &bus));
        // beside the bus, above it, behind it
        assert!(!point_in_vehicle_box(DVec3::new(104.6, 197.0, 31.8), &bus));
        assert!(!point_in_vehicle_box(DVec3::new(104.6, 200.0, 34.0), &bus));
        assert!(!point_in_vehicle_box(DVec3::new(93.0, 200.0, 31.0), &bus));
    }

    #[test]
    fn opaque_and_transmapped_materials_ignore_dynamic_alpha() {
        assert_eq!(
            Renderer::clamp_slot_alpha(0.0, AlphaMode::Opaque, false),
            1.0
        );
        assert_eq!(
            Renderer::clamp_slot_alpha(0.35, AlphaMode::Opaque, false),
            1.0
        );
        assert_eq!(Renderer::clamp_slot_alpha(0.0, AlphaMode::Test, false), 1.0);
        assert_eq!(
            Renderer::clamp_slot_alpha(0.35, AlphaMode::Test, false),
            1.0
        );
        assert_eq!(
            Renderer::clamp_slot_alpha(0.85, AlphaMode::Blend, false),
            0.85
        );
        assert_eq!(Renderer::clamp_slot_alpha(0.0, AlphaMode::Blend, true), 1.0);
        assert_eq!(
            Renderer::clamp_slot_alpha(0.0, AlphaMode::Opaque, true),
            1.0
        );
        assert_eq!(Renderer::clamp_slot_alpha(0.0, AlphaMode::Test, true), 1.0);
    }
}

thread_local! {
    /// A panic on this thread now is caught and handled (see [`catching`]).
    static CATCHING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Whether a panic on this thread is being caught by the renderer (the game's panic hook
/// does not report it as the end of the game).
pub fn catching() -> bool {
    CATCHING.with(|c| c.get())
}

/// Run `f`; a panic in it (a driver wgpu cannot use, taking it down in a way it does not
/// turn into an error) is caught and gives None, and is not reported as the end of the game.
pub fn catch<R>(f: impl FnOnce() -> R) -> Option<R> {
    let was = CATCHING.with(|c| c.replace(true));
    let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));
    CATCHING.with(|c| c.set(was));
    r.ok()
}
