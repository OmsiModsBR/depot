//! `Renderer::render_inner`: one picture, from the camera uniform and the draw lists to
//! the passes it records and submits.

use super::state::{
    Renderer, HDR_FORMAT, SHADOW_CLOSE_MAX, SHADOW_RANGE, SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR,
};
use crate::assets::AlphaMode;
use crate::lighting::{debug_view, lamp_sky_glow, view_lamp_light, Lighting};
use crate::passes::{
    batch_items, culls_back_faces, depth_only_material, depth_prepass_kind, effective_render_phase,
    horizontal_sort_distance, in_scope, instance_depth_bias, nearest_by_origin, origin_key,
    pipe_code, record_bundles, run_parts, surface_depth_coverage, world_surface_phase, Batch,
    DrawItem, PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_KINDS, PIPE_OPAQUE,
    PIPE_SURFACE_DEPTH, PIPE_TERRAIN_PAINT,
};
use crate::scene::{
    point_in_vehicle_box, Camera, CameraUniform, Instance, RenderPhase, Scene, TextureId,
    CULL_BLOCK,
};
use crate::sky::CLOUD_ORIGIN_PERIOD;
use glam::{DVec3, Mat4, Vec3};
use std::collections::HashMap;

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_inner(
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
            *self
                .gpu
                .device_lost
                .lock()
                .unwrap_or_else(|e| e.into_inner()) = Some("test".into());
        }
        // the device is gone: nothing can be drawn, and the readbacks (the exposure meter)
        // would find their buffers taken away - "Error in Buffer::get_mapped_range:
        // Validation Error" ended the game instead of the session ending in order
        if self
            .gpu
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
                format: self.gpu.format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            });
        }
        if self.gpu.rt_error.load(std::sync::atomic::Ordering::Relaxed) {
            self.fall_back_without_ray_tracing(scene);
        } else if self
            .gpu
            .gpu_error
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.fall_back_to_single_sample(scene);
        }
        // OMSI_GPU_TIMERS: GPU time per pass, one frame at a time
        self.collect_gpu_timers();
        let tset: Option<wgpu::QuerySet> = self.diagnostics.gpu_timers[with_overlays as usize]
            .as_ref()
            .filter(|t| !t.waiting && !t.unresolved)
            .map(|t| t.set.clone());
        let mut timed: Vec<&'static str> = Vec::new();
        // OMSI_PROFILE: time per stage, the mirrors apart from the window's picture
        let mut stage_t = std::time::Instant::now();
        let mut stage = |r: &Renderer, window: &'static str, mirror: &'static str| {
            if r.diagnostics.profiling {
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
                && self.shadows.shadow_pipelines.len() > 6,
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
            && self.ssao.ssao_pipeline.is_some()
            && omsi_cfg::env::var_os("OMSI_NO_AO").is_none();
        // the enhanced path's shading is costly: the depth prepass keeps it to the visible
        // surface (without multisampling, see `share_depth`)
        let prepass_on =
            ao_on || puddles_wanted || glass_on || (enhanced && (with_overlays || xr_view));
        if prepass_on && self.ensure_ao(width, height) {
            // a new AO texture: the camera bind group must point at it
            scene.dirty = true;
            scene.model_buf = None;
            self.targets.hdr_targets.clear();
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
                    self.gpu.format
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
            self.shadows
                .xr_shadow_cache
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
        let (near_m, near_age, near_origin, near_sun) = self.shadows.shadow_near_cache.get();
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
            self.shadows
                .shadow_near_cache
                .set((near_wanted, 0, scene.render_origin, sun));
            near_wanted
        } else {
            self.shadows
                .shadow_near_cache
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
        let (far_m, far_age, far_origin, far_sun) = self.shadows.shadow_far_cache.get();
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
            self.shadows
                .shadow_far_cache
                .set((far_wanted, 0, scene.render_origin, sun));
            far_wanted
        } else {
            self.shadows
                .shadow_far_cache
                .set((far_m, far_age + 1, far_origin, far_sun));
            far_m
        };
        if projection.is_some() && !second_eye && shadows {
            self.shadows.xr_shadow_cache.set(Some((
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
            let (lx, ly, side) = self.light_map.lm_place.get();
            let v: [f32; 4] = [
                (lx - ro.x) as f32,
                (ly - ro.y) as f32,
                side as f32,
                if side > 0.0 { 1.0 } else { 0.0 },
            ];
            self.queue
                .write_buffer(&self.light_map.lm_uniform, 0, bytemuck::cast_slice(&v));
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
            self.adaptation.view_lamps = Some(view_lamp_light(
                &scene.lights,
                scene.render_origin,
                cam_rel,
                camera,
                aspect,
                ground,
            ));
            if omsi_cfg::env::var_os("OMSI_DEBUG_VIEW_LAMPS").is_some() {
                log::info!(
                    "view lamps: {:.6}",
                    self.adaptation.view_lamps.unwrap_or(0.0)
                );
            }
        }
        // the night sky's glow from the lamps round the camera (the window's view leads), in
        // steps of a tenth: the sky is recomputed for a new value, not for every metre driven
        if enhanced && lead_view {
            let raw = lamp_sky_glow(scene, cam_rel);
            let target = raw.clamp(0.03, 1.5);
            let step = (target.ln() * 10.0).round() / 10.0;
            let changed = self
                .sky
                .city_glow
                .is_none_or(|g| (g.ln() - step).abs() > 0.15);
            if changed {
                self.sky.city_glow = Some(step.exp());
            }
            if omsi_cfg::env::var_os("OMSI_DEBUG_SKY").is_some() && changed {
                log::info!(
                    "sky glow from the lamps: {raw:.3} (taken {:.3})",
                    step.exp()
                );
            }
        }
        let probe_redraw = enhanced
            && (lead_view || self.sky.sky_state.is_none())
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
                self.gpu
                    .encoding_pool
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
            for part in run_parts(self.gpu.encoding_pool.as_ref(), parts, |p| {
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
            std::mem::take(&mut *self.culling.cull_drawn.borrow_mut())
        } else {
            Vec::new()
        };
        let was_drawn = |i: usize| {
            drawn_before
                .get(i / 64)
                .is_some_and(|w| w & (1u64 << (i % 64)) != 0)
        };
        let (mut sizes_before, mut sizes_now) = if main_view {
            let sizes_before = std::mem::take(&mut *self.culling.object_sizes.borrow_mut());
            let mut sizes_now =
                std::mem::take(&mut *self.culling.object_sizes_scratch.borrow_mut());
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
            self.gpu
                .encoding_pool
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
        for (v, sizes) in run_parts(self.gpu.encoding_pool.as_ref(), parts, |p| {
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
            *self.culling.object_sizes.borrow_mut() = sizes_now;
            sizes_before.clear();
            *self.culling.object_sizes_scratch.borrow_mut() = sizes_before;
        }
        if main_view {
            drawn_before.resize(scene.instances.len().div_ceil(64), 0);
            drawn_before.fill(0);
            for &(i, _, _) in &visible {
                drawn_before[i / 64] |= 1u64 << (i % 64);
            }
            *self.culling.cull_drawn.borrow_mut() = drawn_before;
        }
        // OMSI_DEBUG_FLICKER: a near instance in view in two frames running that is drawn in
        // one and not in the other - the objects blinking in and out as the view moves
        if with_overlays && omsi_cfg::env::var_os("OMSI_DEBUG_FLICKER").is_some() {
            let drawn: std::collections::HashSet<usize> = visible.iter().map(|v| v.0).collect();
            let mut prev = self.culling.flicker.borrow_mut();
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
        let pool = self.gpu.encoding_pool.as_ref();
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
        if self.diagnostics.profiling && !with_overlays {
            let mut c = self.counts.borrow_mut();
            *c.entry("mirror pictures").or_default() += 1.0;
            *c.entry("mirror visible instances").or_default() += visible.len() as f64;
        }
        if self.diagnostics.profiling && with_overlays {
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
        if self.diagnostics.profiling
            && with_overlays
            && self.diagnostics.draw_audit_at.elapsed().as_secs() >= 10
        {
            self.diagnostics.draw_audit_at = std::time::Instant::now();
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
                self.gpu.format
            };
            record_bundles(
                &self.device,
                self.gpu.encoding_pool.as_ref(),
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
        self.encode_shadow_maps(
            scene,
            &mut shadow_encoder,
            &shadow_batches,
            &lamp_shadows,
            draw_shadows,
            redraw_near,
            redraw_far,
            &tset,
            &mut timed,
        );
        self.encode_prepass_and_ao(
            scene,
            &mut prepass_encoder,
            &prepass_batches,
            camera,
            projection,
            aspect,
            width,
            height,
            prepass_on,
            ao_on,
            rt_frame,
            &tset,
            &mut timed,
        );
        self.encode_sky_cube(
            scene,
            &mut prepass_encoder,
            enhanced,
            lead_view,
            probe_redraw,
            redraw_near,
            &tset,
            &mut timed,
        );
        self.encode_probe(scene, &mut prepass_encoder, probe_redraw, &tset, &mut timed);
        let main_parts = self.encode_main_pass(
            scene,
            &mut encoder,
            lighting,
            scene_view,
            &main_batches,
            &cab_batches,
            &prepass_batches,
            &main_bundles,
            &overlays,
            width,
            height,
            enhanced,
            reflection_frame,
            masked_frame,
            has_presurface,
            prepass_on,
            ao_on,
            puddles_wanted,
            scaled,
            with_overlays,
            xr_view,
            &tset,
            &mut timed,
        );
        let puddles_on = self.encode_reflections(
            scene,
            &mut encoder,
            camera,
            lighting,
            &cu,
            &list,
            &main_batches,
            &cab_batches,
            projection,
            ro,
            aspect,
            width,
            height,
            puddles_wanted,
            rt_frame,
            &tset,
            &mut timed,
        );
        self.encode_fog_lamps(
            scene,
            &mut encoder,
            lighting,
            vp_mat,
            width,
            height,
            enhanced,
            masked_frame,
            prepass_on,
            puddles_on,
            with_overlays,
            &tset,
            &mut timed,
        );
        self.encode_glass(
            scene,
            &mut encoder,
            scene_view,
            &rain_batches,
            width,
            height,
            enhanced,
            glass_on,
            masked_frame,
            puddles_on,
            reflection_frame,
        );
        self.encode_reflection_resolve(
            scene,
            &mut encoder,
            scene_view,
            &overlays,
            width,
            height,
            puddles_on,
            reflection_frame,
            scaled,
        );
        self.encode_enhanced_post(
            scene,
            &mut encoder,
            lighting,
            scene_view,
            &overlays,
            width,
            height,
            dt,
            enhanced,
            lead_view,
            puddles_on,
            rt_frame,
            scaled,
            with_overlays,
            &tset,
            &mut timed,
        );
        self.encode_present(
            scene,
            &mut encoder,
            &scene_target,
            target,
            &overlays,
            width,
            height,
            full_w,
            vanilla_fxaa,
            &tset,
            &mut timed,
        );
        stage(self, "encode", "mirror.encode");
        // Turning the recorded passes into Metal commands is the costliest CPU step of a
        // frame (wgpu checks every draw): the shadow maps and the prepass are finished on
        // helper threads while this one finishes the picture.
        let big = shadow_batches.iter().map(|b| b.len()).sum::<usize>() + prepass_batches.len()
            > 64
            || !main_parts.is_empty();
        let profiling = self.diagnostics.profiling;
        let finish = |encoder: wgpu::CommandEncoder| {
            let start = profiling.then(std::time::Instant::now);
            let commands = encoder.finish();
            (
                commands,
                start.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0),
            )
        };
        let (shadow_commands, prepass_commands, part_commands, commands, finish_times) =
            if big && self.gpu.encoding_pool.is_some() {
                self.gpu
                    .encoding_pool
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
        if self.diagnostics.profiling {
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
            self.diagnostics.gpu_timers[with_overlays as usize].as_mut(),
            timed.is_empty(),
        ) {
            t.pending = timed;
            t.unresolved = true;
        }
    }
}
