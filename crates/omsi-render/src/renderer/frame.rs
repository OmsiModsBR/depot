//! `Renderer::render_inner`: one picture, stage by stage. Its start (`begin_frame`) settles
//! what the picture draws and writes the camera uniform into a `FrameContext`; the draw lists
//! (`lists.rs`) and the passes (`encode.rs`) follow, and the commands are submitted at the end.

use super::state::Renderer;
use crate::gpu::{fake_due, fake_poll_loss, guarded_poll, lost_why, target_2d, HDR_FORMAT};
use crate::lighting::{debug_view, lamp_sky_glow, view_lamp_light, LampShadow, Lighting};
use crate::output::Output;
use crate::passes::{Batch, ShadowSchedule, SHADOW_CLOSE_MAX, SHADOW_RANGE, SHADOW_RANGE_CLOSE};
use crate::scene::{point_in_vehicle_box, Camera, CameraUniform, Scene, TextureId};
use crate::sky::CLOUD_ORIGIN_PERIOD;
use glam::{DVec3, Mat4, Vec3};

/// What a picture draws, decided at its start (`begin_frame`): the passes and paths the
/// draw lists and the encoders branch on.
#[derive(Clone, Copy)]
pub(crate) struct FrameFeatures {
    /// The FXAA on the plain graphics, the picture drawn into a texture of the window's size.
    pub(crate) vanilla_fxaa: bool,
    /// The rain's glass picture (what is behind the glass is drawn first).
    pub(crate) glass_on: bool,
    /// The picture goes through a scaled target (`scene_view`).
    pub(crate) scaled: bool,
    /// A headset's eye, the window's lead view (the one that moves the clocks), Enhanced.
    pub(crate) xr_view: bool,
    pub(crate) lead_view: bool,
    pub(crate) enhanced_frame: bool,
    pub(crate) enhanced: bool,
    /// The puddles and their reflections (`reflection_frame` when drawn by their own pass).
    pub(crate) puddles_wanted: bool,
    pub(crate) reflection_frame: bool,
    /// The scene drawn into the masked targets (Enhanced or the reflections).
    pub(crate) masked_frame: bool,
    /// The traced sun shadow and ray-traced reflections; the ambient occlusion pass.
    pub(crate) rt_frame: bool,
    pub(crate) ao_on: bool,
    /// The depth prepass before the shading.
    pub(crate) prepass_on: bool,
}

/// One picture on its way through the stages of `Renderer::render_inner`: where it goes and
/// what its start decided (its size and targets, what it draws, the shadows' schedule, the
/// camera uniform).
pub(crate) struct FrameContext<'a> {
    pub(crate) target: &'a wgpu::TextureView,
    pub(crate) camera: &'a Camera,
    pub(crate) lighting: &'a Lighting,
    /// The scaled (or FXAA's) picture the scene is drawn into before it reaches `target`.
    pub(crate) scene_target: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    /// The HUD and overlays (the window's pictures), and the street lamps' shadow maps.
    pub(crate) overlays: Vec<(TextureId, [f32; 4])>,
    pub(crate) lamp_shadows: Vec<LampShadow>,
    /// The camera uniform as written for this picture.
    pub(crate) cu: CameraUniform,
    /// What `Output::flags` says: the window's picture, the texture left out, the projection.
    pub(crate) with_overlays: bool,
    /// The window's own picture (`Output::Window`), the only one the overlay hook is called for.
    pub(crate) window_picture: bool,
    pub(crate) exclude_texture: Option<TextureId>,
    pub(crate) projection: Option<Mat4>,
    /// The render origin; the window's width (`full_w`) and the picture's size (smaller with
    /// the render scale); whether it goes through a scaled target, and its aspect.
    pub(crate) ro: DVec3,
    pub(crate) full_w: u32,
    pub(crate) full_h: u32,
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) features: FrameFeatures,
    pub(crate) aspect: f32,
    /// The camera relative to the render origin.
    pub(crate) cam_rel: Vec3,
    /// Seconds since the leading view's last frame.
    pub(crate) dt: f32,
    /// The shadows' schedule (`ShadowResources::schedule`).
    pub(crate) moon_shadows: bool,
    pub(crate) shadows: bool,
    pub(crate) draw_shadows: bool,
    pub(crate) redraw_near: bool,
    pub(crate) redraw_far: bool,
    pub(crate) light_view_proj: Mat4,
    pub(crate) light_view_proj_far: Mat4,
    pub(crate) light_view_proj_close: Mat4,
    /// The view-projection matrix, and the reflection probe is redrawn this frame.
    pub(crate) vp_mat: Mat4,
    pub(crate) probe_redraw: bool,
}

impl FrameContext<'_> {
    /// Where the scene is drawn: the scaled picture, else the target itself.
    pub(crate) fn scene_view(&self) -> &wgpu::TextureView {
        self.scene_target
            .as_ref()
            .map(|t| &t.0)
            .unwrap_or(self.target)
    }
}

/// A picture's GPU timers and, with OMSI_PROFILE, the time of its stages.
pub(crate) struct FrameClock {
    pub(crate) tset: Option<wgpu::QuerySet>,
    pub(crate) timed: Vec<&'static str>,
    stage_t: std::time::Instant,
    with_overlays: bool,
}

impl FrameClock {
    /// OMSI_PROFILE: the time since the last stage, added to this one's (the mirrors apart
    /// from the window's picture).
    pub(crate) fn stage(&mut self, r: &Renderer, window: &'static str, mirror: &'static str) {
        if r.diagnostics.profiling {
            let now = std::time::Instant::now();
            *r.stats
                .borrow_mut()
                .entry(if self.with_overlays { window } else { mirror })
                .or_default() += (now - self.stage_t).as_secs_f64();
            self.stage_t = now;
        }
    }
}

/// The draw lists of a picture (see `Renderer::draw_lists`).
pub(crate) struct DrawLists {
    pub(crate) list: Vec<u32>,
    /// Near, far, close, the street lamps'.
    pub(crate) shadow_batches: [Vec<Batch>; 4],
    pub(crate) prepass_batches: Vec<Batch>,
    pub(crate) main_batches: Vec<Batch>,
    pub(crate) cab_batches: Vec<Batch>,
    pub(crate) rain_batches: Vec<Batch>,
    pub(crate) has_presurface: bool,
}

/// A picture's recorded commands, waiting to be finished and submitted.
pub(crate) struct FrameEncoders {
    pub(crate) shadow_encoder: wgpu::CommandEncoder,
    pub(crate) prepass_encoder: wgpu::CommandEncoder,
    pub(crate) encoder: wgpu::CommandEncoder,
    pub(crate) main_parts: Vec<wgpu::CommandEncoder>,
}

impl Renderer {
    /// One picture into `target` for `output`, stage by stage.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn render_inner(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        output: Output,
    ) {
        let Some((f, mut clock)) =
            self.begin_frame(scene, target, width, height, camera, lighting, output)
        else {
            return;
        };
        let lists = self.draw_lists(scene, &f, &mut clock);
        if f.window_picture && self.plan_sink.is_some() {
            let light_map = self.light_map_place(f.ro);
            if let Some(sink) = self.plan_sink.as_mut() {
                sink(&super::mirror::plan_of(&f, &lists, light_map), scene);
            }
        }
        let encoders = self.encode_frame(scene, &f, &mut clock, &lists);
        self.submit_frame(&f, clock, &lists, encoders);
    }

    /// test hooks for a device the driver loses (`OMSI_FAKE_GPU_ERROR=lost`, `lost-poll`)
    fn fake_device_loss(&mut self, with_overlays: bool) {
        // test hook for a lost device (a driver reset): its resources are taken away and
        // the session has to end in order
        if omsi_cfg::flags::FAKE_GPU_ERROR.text().as_deref() == Some("lost")
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
        // test hook for a device the driver loses while the game runs (`lost-poll`): its loss
        // handler is told and the poll of it panics in wgpu, as on Vulkan - the panic is
        // caught, and the frame below sees the device lost
        if with_overlays && fake_poll_loss(self.started) {
            let _ = guarded_poll(Some(&self.gpu.device_lost), true, || {
                self.device.poll(wgpu::PollType::Poll)
            });
        }
    }

    /// test hooks for a card out of memory and for a failing frame target
    fn fake_gpu_errors(&mut self, with_overlays: bool) {
        // test hook for a card out of memory: reported once, as a texture it could not make
        // is, so that the game keeps fewer textures (`take_out_of_memory`)
        if with_overlays
            && fake_due(
                omsi_cfg::flags::FAKE_GPU_ERROR.text().as_deref(),
                "oom",
                self.started.elapsed().as_secs_f32(),
            )
        {
            self.gpu.errors.fake_out_of_memory_once();
        }
        if omsi_cfg::flags::FAKE_GPU_ERROR.text().as_deref() == Some("frame")
            && self.options.msaa > 1
            && self.started.elapsed().as_secs_f32() > 3.0
        {
            // test hook for the fallback below
            let _ = target_2d(
                &self.device,
                "invalid",
                wgpu::Extent3d {
                    width: 4,
                    height: 4,
                    depth_or_array_layers: 1,
                },
                3,
                self.gpu.format,
                wgpu::TextureUsages::RENDER_ATTACHMENT,
            );
        }
    }

    /// The start of a picture: the device's state and the fallbacks, the picture's size and
    /// targets, what it draws (Enhanced, the puddles, the glass, ray tracing, ambient
    /// occlusion), the lights, the shadows' schedule and the camera uniform. None when the
    /// device is lost.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn begin_frame<'a>(
        &mut self,
        scene: &mut Scene,
        target: &'a wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &'a Camera,
        lighting: &'a Lighting,
        output: Output,
    ) -> Option<(FrameContext<'a>, FrameClock)> {
        let (with_overlays, exclude_texture, projection, second_eye) = output.flags();
        self.fake_device_loss(with_overlays);
        // the device is gone: nothing can be drawn, and the readbacks (the exposure meter)
        // would find their buffers taken away - "Error in Buffer::get_mapped_range:
        // Validation Error" ended the game instead of the session ending in order
        if lost_why(&self.gpu.device_lost).is_some() {
            return None;
        }
        self.fake_gpu_errors(with_overlays);
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
        let mut clock = FrameClock {
            tset: self.diagnostics.gpu_timers[with_overlays as usize]
                .as_ref()
                .filter(|t| !t.waiting && !t.unresolved)
                .map(|t| t.set.clone()),
            timed: Vec::new(),
            // OMSI_PROFILE: time per stage, the mirrors apart from the window's picture
            stage_t: std::time::Instant::now(),
            with_overlays,
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
                && !omsi_cfg::flags::NO_ENHANCED.is_set())
            && !omsi_cfg::flags::NO_FXAA.is_set();
        // Rain films are drawn after copying the clean current-frame scene. Classic
        // graphics need a sampleable scene target too, even without puddles or scaling.
        let enhanced_view =
            lighting.enhanced && self.hdr_pass.is_some() && !omsi_cfg::flags::NO_ENHANCED.is_set();
        let glass_on = with_overlays
            && scene.glass_slot.is_some()
            && (lighting.rain > 0.001 || lighting.wetness > 0.02)
            && !omsi_cfg::flags::NO_GLASS_PICTURE.is_set();
        let scaled =
            (width, height) != (full_w, full_h) || vanilla_fxaa || (glass_on && !enhanced_view);
        let scene_target: Option<(wgpu::TextureView, wgpu::BindGroup)> = if scaled {
            Some(self.scale_target(width, height))
        } else {
            None
        };
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
            && !omsi_cfg::flags::NO_ENHANCED.is_set()
            && (with_overlays || xr_view || omsi_cfg::flags::MIRROR_ENHANCED.is_set());
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
            && !omsi_cfg::flags::NO_PUDDLE_REFLECTIONS.is_set();
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
            && !omsi_cfg::flags::NO_RT_FRAME.is_set();
        let ao_on = with_overlays
            && self.rt.is_none()
            && self.options.ssao
            && self.ssao.ssao_pipeline.is_some()
            && !omsi_cfg::flags::NO_AO.is_set();
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
        clock.stage(self, "setup", "mirror.setup");
        self.prepare(scene);
        clock.stage(self, "prepare", "mirror.prepare");
        // overlay uniforms/bind groups (rects in pixels → NDC)
        let overlays: Vec<(TextureId, [f32; 4])> = if with_overlays {
            scene.overlays.clone()
        } else {
            Vec::new()
        };
        if with_overlays {
            self.prepare_overlays(scene, full_w, full_h);
        }
        let ShadowSchedule {
            moon_shadows,
            shadows,
            draw_shadows,
            redraw_near,
            redraw_far,
            light_view_proj,
            light_view_proj_far,
            light_view_proj_close,
        } = self.shadows.schedule(
            self.options.shadow_size,
            scene.render_origin,
            lighting,
            cam_rel,
            enhanced_frame,
            with_overlays,
            projection,
            second_eye,
        );
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
            let v = self.light_map_place(ro);
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
            clock.stage(self, "ray tracing", "mirror.ray tracing");
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
                (camera.forward(), camera.right(), camera.up()),
                camera.fov_deg,
                aspect,
                ground,
            ));
            if omsi_cfg::flags::DEBUG_VIEW_LAMPS.is_set() {
                log::info!(
                    "view lamps: {:.6}",
                    self.adaptation.view_lamps.unwrap_or(0.0)
                );
            }
        }
        // the night sky's glow from the lamps round the camera (the window's view leads), in
        // steps of a tenth: the sky is recomputed for a new value, not for every metre driven
        if enhanced && lead_view {
            let raw = lamp_sky_glow(&scene.lights, scene.render_origin, cam_rel);
            let target = raw.clamp(0.03, 1.5);
            let step = (target.ln() * 10.0).round() / 10.0;
            let changed = self
                .sky
                .city_glow
                .is_none_or(|g| (g.ln() - step).abs() > 0.15);
            if changed {
                self.sky.city_glow = Some(step.exp());
            }
            if omsi_cfg::flags::DEBUG_SKY.is_set() && changed {
                log::info!(
                    "sky glow from the lamps: {raw:.3} (taken {:.3})",
                    step.exp()
                );
            }
        }
        let probe_redraw = enhanced
            && (lead_view || self.sky.sky_state.is_none())
            && self.prepare_enhanced(lighting, cam_rel, ro, dt);
        Some((
            FrameContext {
                target,
                camera,
                lighting,
                scene_target,
                overlays,
                lamp_shadows,
                cu,
                with_overlays,
                window_picture: output.is_window(),
                exclude_texture,
                projection,
                ro,
                full_w,
                full_h,
                width,
                height,
                features: FrameFeatures {
                    vanilla_fxaa,
                    glass_on,
                    scaled,
                    xr_view,
                    lead_view,
                    enhanced_frame,
                    enhanced,
                    puddles_wanted,
                    reflection_frame,
                    masked_frame,
                    rt_frame,
                    ao_on,
                    prepass_on,
                },
                aspect,
                cam_rel,
                dt,
                moon_shadows,
                shadows,
                draw_shadows,
                redraw_near,
                redraw_far,
                light_view_proj,
                light_view_proj_far,
                light_view_proj_close,
                vp_mat,
                probe_redraw,
            },
            clock,
        ))
    }

    /// The recorded commands finished (on helper threads when there are many) and submitted.
    pub(crate) fn submit_frame(
        &mut self,
        f: &FrameContext,
        mut clock: FrameClock,
        lists: &DrawLists,
        encoders: FrameEncoders,
    ) {
        let with_overlays = f.with_overlays;
        let (shadow_batches, prepass_batches) = (&lists.shadow_batches, &lists.prepass_batches);
        let FrameEncoders {
            shadow_encoder,
            prepass_encoder,
            encoder,
            main_parts,
        } = encoders;
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
        clock.stage(self, "finish", "mirror.finish");
        self.queue.submit(
            [shadow_commands, prepass_commands]
                .into_iter()
                .chain(part_commands)
                .chain([commands]),
        );
        clock.stage(self, "submit", "mirror.submit");
        if let (Some(t), false) = (
            self.diagnostics.gpu_timers[with_overlays as usize].as_mut(),
            clock.timed.is_empty(),
        ) {
            t.pending = clock.timed;
            t.unresolved = true;
        }
    }
}
