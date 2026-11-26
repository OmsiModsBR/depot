//! `Renderer::render_inner`: one picture, stage by stage. Its start (`begin_frame`) settles
//! what the picture draws and writes the camera uniform into a `FrameContext`; the draw lists
//! (`lists.rs`) and the passes (`encode.rs`) follow, and the commands are submitted at the end.

use super::camera_uniform::UniformInputs;
use super::features::{puddle_weather, BuiltPasses, FrameInputs, FrameSwitches, ViewKind};
use super::state::Renderer;
use crate::gpu::{fault_due, guarded_poll, lost_why, target_2d, FaultHook, HDR_FORMAT};
use crate::interface::{LeadViewParts, PipeSet, RenderMode};
use crate::lighting::{LampShadow, Lighting};
use crate::output::Output;
use crate::passes::{Batch, FrameFeatures, ShadowSchedule};
use crate::scene::{point_in_vehicle_box, Camera, CameraUniform, Scene, TextureId};
use glam::{DVec3, Mat4, Vec3};

/// One picture on its way through the stages of `Renderer::render_inner`: where it goes and
/// what its start decided (its size and targets, what it draws, the shadows' schedule, the
/// camera uniform).
pub struct FrameContext<'a> {
    pub target: &'a wgpu::TextureView,
    pub camera: &'a Camera,
    pub lighting: &'a Lighting,
    /// The scaled (or FXAA's) picture the scene is drawn into before it reaches `target`.
    pub scene_target: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    /// The HUD and overlays (the window's pictures), and the street lamps' shadow maps.
    pub overlays: Vec<(TextureId, [f32; 4])>,
    pub lamp_shadows: Vec<LampShadow>,
    /// The camera uniform as written for this picture.
    pub cu: CameraUniform,
    /// What `Output::flags` says: the window's picture, the texture left out, the projection.
    pub with_overlays: bool,
    /// The window's own picture (`Output::Window`), the only one the overlay hook is called for.
    pub window_picture: bool,
    pub exclude_texture: Option<TextureId>,
    pub projection: Option<Mat4>,
    /// The render origin; the window's width (`full_w`) and the picture's size (smaller with
    /// the render scale); whether it goes through a scaled target, and its aspect.
    pub ro: DVec3,
    pub full_w: u32,
    pub full_h: u32,
    pub width: u32,
    pub height: u32,
    pub features: FrameFeatures,
    /// The pipelines of the main pass this picture is drawn with (the mode's choice).
    pub pipe_set: PipeSet,
    pub aspect: f32,
    /// The camera relative to the render origin.
    pub cam_rel: Vec3,
    /// Seconds since the leading view's last frame.
    pub dt: f32,
    /// The shadows' schedule (`ShadowResources::schedule`).
    pub shadows: bool,
    pub draw_shadows: bool,
    pub redraw_near: bool,
    pub redraw_far: bool,
    pub light_view_proj: Mat4,
    pub light_view_proj_far: Mat4,
    pub light_view_proj_close: Mat4,
    /// The view-projection matrix, and the reflection probe is redrawn this frame.
    pub vp_mat: Mat4,
    pub probe_redraw: bool,
}

impl FrameContext<'_> {
    /// Where the scene is drawn: the scaled picture, else the target itself.
    pub fn scene_view(&self) -> &wgpu::TextureView {
        self.scene_target
            .as_ref()
            .map(|t| &t.0)
            .unwrap_or(self.target)
    }
}

/// A picture's GPU timers and, with OMSI_PROFILE, the time of its stages.
pub struct FrameClock {
    pub tset: Option<wgpu::QuerySet>,
    pub timed: Vec<&'static str>,
    stage_t: std::time::Instant,
    with_overlays: bool,
}

impl FrameClock {
    /// OMSI_PROFILE: the time since the last stage, added to this one's (the mirrors apart
    /// from the window's picture).
    pub fn stage(&mut self, r: &Renderer, window: &'static str, mirror: &'static str) {
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
pub struct DrawLists {
    pub list: Vec<u32>,
    /// Near, far, close, the street lamps'.
    pub shadow_batches: [Vec<Batch>; 4],
    pub prepass_batches: Vec<Batch>,
    pub main_batches: Vec<Batch>,
    pub cab_batches: Vec<Batch>,
    pub rain_batches: Vec<Batch>,
    pub has_presurface: bool,
}

/// A picture's recorded commands, waiting to be finished and submitted.
pub struct FrameEncoders {
    pub shadow_encoder: wgpu::CommandEncoder,
    pub prepass_encoder: wgpu::CommandEncoder,
    pub encoder: wgpu::CommandEncoder,
    pub main_parts: Vec<wgpu::CommandEncoder>,
}

/// What the picture's targets and lights are prepared from (see `Renderer::prepare_picture`).
struct PictureInputs<'a> {
    camera: &'a Camera,
    lighting: &'a Lighting,
    mode: &'static dyn RenderMode,
    features: &'a FrameFeatures,
    width: u32,
    height: u32,
    with_overlays: bool,
    projection: Option<Mat4>,
    exclude_texture: Option<TextureId>,
    lead_view: bool,
    ro: DVec3,
}

/// What `Renderer::prepare_picture` settled for the rest of the picture.
struct PictureSetup {
    scene_target: Option<(wgpu::TextureView, wgpu::BindGroup)>,
    aspect: f32,
    cam_rel: Vec3,
    /// The light grid for the camera uniform, and the street lamps that cast a shadow.
    grid: [f32; 4],
    lamp_shadows: Vec<LampShadow>,
    dt: f32,
}

impl Renderer {
    /// One picture into `target` for `output`, stage by stage.
    #[allow(clippy::too_many_arguments)]
    pub fn render_inner(
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
        if (f.window_picture || f.exclude_texture.is_some()) && self.plan_sink.is_some() {
            let light_map = self.light_map_place(f.ro);
            // (the window's picture lays the rain reflection drawn before it on its puddles)
            let rain_reflection = self
                .rain_reflection
                .filter(|_| f.window_picture)
                .map(|(t, cam, aspect)| (t, cam.view_proj(aspect, f.ro).to_cols_array_2d()));
            if let Some(sink) = self.plan_sink.as_mut() {
                let mut plan = super::mirror::plan_of(&f, &lists, light_map);
                plan.rain_reflection = rain_reflection;
                sink(&plan, scene);
            }
        }
        let encoders = self.encode_frame(scene, &f, &mut clock, &lists);
        self.submit_frame(&f, clock, &lists, encoders);
        // the draw list's capacity serves the next picture
        self.scratch.list = lists.list;
    }

    /// test hooks for a device the driver loses (`OMSI_FAKE_GPU_ERROR=lost`, `lost-poll`)
    fn fake_device_loss(&mut self, with_overlays: bool) {
        // test hook for a lost device (a driver reset): its resources are taken away and
        // the session has to end in order
        if fault_due(FaultHook::Lost, self.started) && with_overlays && self.device_lost().is_none()
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
        if with_overlays && fault_due(FaultHook::LostPoll, self.started) {
            let _ = guarded_poll(Some(&self.gpu.device_lost), true, || {
                self.device.poll(wgpu::PollType::Poll)
            });
        }
    }

    /// test hooks for a card out of memory and for a failing frame target
    fn fake_gpu_errors(&mut self, with_overlays: bool) {
        // test hook for a card out of memory: reported once, as a texture it could not make
        // is, so that the game keeps fewer textures (`take_out_of_memory`)
        if with_overlays && fault_due(FaultHook::Oom, self.started) {
            self.gpu.errors.fake_out_of_memory_once();
        }
        if fault_due(FaultHook::Frame, self.started) && self.options.msaa > 1 {
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

    /// The picture's first stage: the device's state, the fallbacks and the timers. None when
    /// the device is lost.
    fn begin_device(&mut self, scene: &mut Scene, with_overlays: bool) -> Option<FrameClock> {
        self.fake_device_loss(with_overlays);
        // the device is gone: nothing can be drawn, and the readbacks (the exposure meter)
        // would find their buffers taken away - "Error in Buffer::get_mapped_range:
        // Validation Error" ended the game instead of the session ending in order
        if lost_why(&self.gpu.device_lost).is_some() {
            return None;
        }
        self.fake_gpu_errors(with_overlays);
        if self
            .gpu
            .gpu_error
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            self.fall_back_to_single_sample(scene);
        }
        // OMSI_GPU_TIMERS: GPU time per pass, one frame at a time
        self.collect_gpu_timers();
        Some(FrameClock {
            tset: self.diagnostics.gpu_timers[with_overlays as usize]
                .as_ref()
                .filter(|t| !t.waiting && !t.unresolved)
                .map(|t| t.set.clone()),
            timed: Vec::new(),
            // OMSI_PROFILE: time per stage, the mirrors apart from the window's picture
            stage_t: std::time::Instant::now(),
            with_overlays,
        })
    }

    /// The picture's targets and what it needs ready: the scaled target, the lights, the coronas,
    /// the smoke, the ambient occlusion and glass targets, and the time since the last frame.
    fn prepare_picture(&mut self, scene: &mut Scene, p: &PictureInputs) -> PictureSetup {
        let PictureInputs {
            camera,
            lighting,
            mode,
            features,
            width,
            height,
            with_overlays,
            projection,
            exclude_texture,
            lead_view,
            ro,
        } = *p;
        let FrameFeatures {
            glass_on,
            scaled,
            masked_frame,
            prepass_on,
            ..
        } = *features;
        let scene_target: Option<(wgpu::TextureView, wgpu::BindGroup)> = if scaled {
            Some(self.scale_target(width, height))
        } else {
            None
        };
        let aspect = self
            .texture_aspect
            .unwrap_or(width as f32 / height.max(1) as f32);
        let cam_rel = (camera.position - ro).as_vec3();
        let (grid, lamp_shadows) = self.prepare_lights(
            scene,
            cam_rel,
            mode,
            features,
            lighting.lamp_shadows
                && with_overlays
                && projection.is_none()
                && self.shadows.shadow_pipelines.len()
                    > crate::passes::shadow_pipe(crate::passes::LAMP_CASCADE, 0),
        );
        self.prepare_coronas(
            scene,
            lighting,
            camera.position,
            lighting
                .inside
                .as_ref()
                .filter(|v| point_in_vehicle_box(camera.position, v)),
        );
        self.prepare_smoke(scene, camera.position, exclude_texture.is_some());
        if prepass_on && self.ensure_ao(width, height) {
            // a new AO texture: the camera bind group must point at it
            scene.dirty = true;
            scene.model_buf = None;
            self.targets.hdr_targets.clear();
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
        PictureSetup {
            scene_target,
            aspect,
            cam_rel,
            grid,
            lamp_shadows,
            dt,
        }
    }

    /// The start of a picture: the device's state and the fallbacks, the picture's size and
    /// targets, what it draws (Enhanced, the puddles, the glass, ambient
    /// occlusion), the lights, the shadows' schedule and the camera uniform. None when the
    /// device is lost.
    #[allow(clippy::too_many_arguments)]
    pub fn begin_frame<'a>(
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
        let mut clock = self.begin_device(scene, with_overlays)?;
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
        let xr_view = projection.is_some();
        let lead_view = (with_overlays || xr_view) && !second_eye;
        // what the picture draws (and, with it, whether it goes through a scaled target)
        let mode = super::build::mode_for(lighting.path);
        let features = FrameFeatures::decide_with(
            mode,
            &self.options,
            &FrameInputs {
                view: ViewKind::of(output),
                lead_view,
                full_size: (full_w, full_h),
                picture_size: (width, height),
                rain: lighting.rain,
                wetness: lighting.wetness,
                snow: lighting.snow,
                has_glass_slot: scene.glass_slot.is_some(),
                // (looked for only where the weather could use it)
                has_puddle_material: with_overlays
                    && puddle_weather(lighting.wetness, lighting.snow)
                    && scene.materials.iter().any(|m| m.uniform.params2[2] > 0.0),
                built: BuiltPasses {
                    own_mode: self.own_mode.is_some(),
                    reflections: self.reflection_pass.is_some(),
                    puddles: self.puddles.is_some(),
                    ssao: self.ssao.ssao_pipeline.is_some(),
                },
                switches: FrameSwitches::from_environment(),
            },
        );
        let FrameFeatures { own_shading, .. } = features;
        let PictureSetup {
            scene_target,
            aspect,
            cam_rel,
            grid,
            lamp_shadows,
            dt,
        } = self.prepare_picture(
            scene,
            &PictureInputs {
                camera,
                lighting,
                mode,
                features: &features,
                width,
                height,
                with_overlays,
                projection,
                exclude_texture,
                lead_view,
                ro,
            },
        );
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
        let schedule = self.shadows.schedule(
            self.options.shadow_size,
            self.options.sun_shadows,
            scene.render_origin,
            lighting,
            cam_rel,
            own_shading,
            with_overlays,
            projection,
            second_eye,
        );
        let ShadowSchedule {
            shadows,
            draw_shadows,
            redraw_near,
            redraw_far,
            light_view_proj,
            light_view_proj_far,
            light_view_proj_close,
        } = schedule;
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
        let cu = self.camera_uniform(&UniformInputs {
            scene,
            camera,
            lighting,
            features: &features,
            ro,
            cam_rel,
            vp_mat,
            grid,
            lamp_shadows: &lamp_shadows,
            schedule: &schedule,
        });
        self.queue
            .write_buffer(&self.camera_buf, 0, bytemuck::bytes_of(&cu));
        mode.lead_view(
            &features,
            LeadViewParts {
                scene,
                lighting,
                camera,
                cam_rel,
                aspect,
                adaptation: &mut self.adaptation,
                sky: &mut self.sky,
            },
        );
        let probe_redraw = own_shading
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
                features,
                pipe_set: mode.pipe_set(&features),
                aspect,
                cam_rel,
                dt,
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
    pub fn submit_frame(
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
