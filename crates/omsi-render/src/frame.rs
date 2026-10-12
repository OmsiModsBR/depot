//! A frame: preparing the scene, the lights, the sky and the overlays for it, and the
//! entry points that render a window, a triple screen, a VR eye or an image.

use super::*;

/// An overlay's rectangle (physical pixels) moved onto whole pixels, its size kept. The
/// overlays are pictures drawn texel for pixel - a text, a plate - and the linear filter
/// blended every pixel of one placed between pixels with its neighbour: the interface's texts
/// were soft at every size whose layout fell between them (most but 100 %, and the timetable's
/// rows at that too). A line thinner than a pixel stays one pixel wide or high.
pub(crate) fn snap_rect(r: [f32; 4]) -> [f32; 4] {
    // (half up the same way left of the window as right of it: `round` goes away from zero,
    // and a rectangle across the left edge came out a pixel wider)
    let snap = |v: f32| (v + 0.5).floor();
    let (x0, y0) = (snap(r[0]), snap(r[1]));
    let x1 = if r[2] > r[0] {
        snap(r[2]).max(x0 + 1.0)
    } else {
        snap(r[2])
    };
    let y1 = if r[3] > r[1] {
        snap(r[3]).max(y0 + 1.0)
    } else {
        snap(r[3])
    };
    [x0, y0, x1, y1]
}

impl Renderer {
    /// Upload instance transforms (call after building or changing the scene).
    /// The per-draw entries of one instance: one model matrix and two parameter vectors
    /// per material slot.
    pub(crate) fn instance_entries(
        i: &Instance,
        ro: DVec3,
        mats: &mut Vec<[[f32; 4]; 4]>,
        params: &mut Vec<[f32; 4]>,
    ) {
        let m =
            (Mat4::from_translation((i.origin - ro).as_vec3()) * i.transform).to_cols_array_2d();
        let vis = if i.visible { 1.0 } else { 0.0 };
        for (k, a) in i.slot_alpha.iter().enumerate() {
            mats.push(m);
            let uv = i.slot_uv.get(k).copied().unwrap_or([0.0; 2]);
            params.push([*a, vis, uv[0], uv[1]]);
            params.push([
                i.slot_light.get(k).copied().unwrap_or(1.0),
                i.slot_night.get(k).copied().unwrap_or(1.0),
                // (a lamp code is 1 or more, a plain brightness below 1)
                if i.interior_lamps != 0 {
                    i.interior_lamps as f32
                } else {
                    i.interior.min(0.99)
                },
                // the surface flag: 1 ground, 2 a vehicle's shadow blob (no snow on it),
                // 1.25 a legacy pulled decal, 0.9 an OMSI-ordered surface (weather
                // classification without view-space pull), 0.75 a painted ground layer;
                // below -500 a vehicle part, -(5000 + the roof height relative to origin)
                if let Some(roof) = i.roof.filter(|_| !i.blob && !i.surface) {
                    let z = (i.origin - ro).z as f32
                        + i.transform.transform_point3(Vec3::new(0.0, 0.0, roof)).z;
                    -(5000.0 + z.clamp(-4000.0, 4000.0))
                } else {
                    surface_instance_code(
                        i.blob,
                        i.ground_layer,
                        i.decal,
                        i.surface,
                        i.surface_bias,
                    )
                },
            ]);
        }
    }

    pub fn prepare(&self, scene: &mut Scene) {
        Self::prepare_bounds(scene);
        scene.bind_groups.clear();
        if std::mem::take(&mut scene.origin_moved) && !scene.dirty {
            let ro = scene.render_origin;
            let n = scene.uploaded_entries as usize;
            for i in &scene.instances[..scene.uploaded_instances] {
                let t = (Mat4::from_translation((i.origin - ro).as_vec3()) * i.transform)
                    .w_axis
                    .to_array();
                let (b, k) = (i.base as usize, i.slot_alpha.len());
                if b + k <= n.min(scene.cpu_models.len()) {
                    for m in &mut scene.cpu_models[b..b + k] {
                        m[3] = t;
                    }
                }
            }
            match scene.model_buf.as_ref() {
                Some(buf) if (n * 64) as u64 <= buf.size() && n <= scene.cpu_models.len() => {
                    buf.write(&self.queue, 0, bytemuck::cast_slice(&scene.cpu_models[..n]));
                }
                _ => scene.dirty = true,
            }
        }
        if !scene.dirty
            && scene.instances.len() > scene.uploaded_instances
            && scene.model_buf.is_some()
        {
            // new instances: append their entries if the buffers have room, else rebuild
            let ro = scene.render_origin;
            let mut mats: Vec<[[f32; 4]; 4]> = Vec::new();
            let mut params: Vec<[f32; 4]> = Vec::new();
            let first = scene.uploaded_instances;
            let mut base = scene.uploaded_entries;
            for i in scene.instances[first..].iter_mut() {
                i.base = base;
                Self::instance_entries(i, ro, &mut mats, &mut params);
                base = scene.uploaded_entries + mats.len() as u32;
            }
            let (buf, pbuf) = (
                scene.model_buf.as_ref().unwrap(),
                scene.params_buf.as_ref().unwrap(),
            );
            let mo = scene.uploaded_entries as u64 * 64;
            let po = scene.uploaded_entries as u64 * 32;
            let mb: &[u8] = bytemuck::cast_slice(&mats);
            let pb: &[u8] = bytemuck::cast_slice(&params);
            if mo + mb.len() as u64 <= buf.size()
                && po + pb.len() as u64 <= pbuf.size()
                && scene.cpu_models.len() == scene.uploaded_entries as usize
            {
                if !mb.is_empty() {
                    buf.write(&self.queue, mo, mb);
                    pbuf.write(&self.queue, po, pb);
                }
                scene.cpu_models.extend_from_slice(&mats);
                scene.cpu_params.extend_from_slice(&params);
                scene.uploaded_instances = scene.instances.len();
                scene.uploaded_entries = base;
            } else {
                scene.dirty = true;
            }
        }
        if !scene.dirty {
            // only what moved or changed since the last frame
            if !scene.changed.is_empty() {
                if omsi_cfg::env::var_os("OMSI_DEBUG_DRAWS").is_some() {
                    log::info!(
                        "prepare: {} changed instances of {}",
                        scene.changed.len(),
                        scene.instances.len()
                    );
                }
                if let (Some(buf), Some(pbuf)) = (&scene.model_buf, &scene.params_buf) {
                    let ro = scene.render_origin;
                    // The changed entries go into the CPU copy; the ranges they span are
                    // uploaded merged - the meshes of one vehicle or person sit next to each
                    // other, and neighbouring vehicles are sent together with whatever lies
                    // between them when that is not much.
                    const MERGE_GAP: u32 = 4096;
                    scene.changed.sort_unstable_by_key(|&i| {
                        scene.instances.get(i).map(|x| x.base).unwrap_or(u32::MAX)
                    });
                    let mut mats: Vec<[[f32; 4]; 4]> = Vec::new();
                    let mut params: Vec<[f32; 4]> = Vec::new();
                    let mut ranges: Vec<(u32, u32)> = Vec::new();
                    for &idx in &scene.changed {
                        let Some(i) = scene.instances.get(idx) else {
                            continue;
                        };
                        mats.clear();
                        params.clear();
                        Self::instance_entries(i, ro, &mut mats, &mut params);
                        let (b, n) = (i.base as usize, mats.len());
                        if b + n > scene.cpu_models.len() || (b + n) * 2 > scene.cpu_params.len() {
                            continue;
                        }
                        scene.cpu_models[b..b + n].copy_from_slice(&mats);
                        scene.cpu_params[b * 2..(b + n) * 2].copy_from_slice(&params);
                        let (start, end) = (b as u32, (b + n) as u32);
                        match ranges.last_mut() {
                            Some(r) if start <= r.1 + MERGE_GAP => r.1 = r.1.max(end),
                            _ => ranges.push((start, end)),
                        }
                    }
                    for (start, end) in ranges {
                        let mb: &[u8] =
                            bytemuck::cast_slice(&scene.cpu_models[start as usize..end as usize]);
                        let pb: &[u8] = bytemuck::cast_slice(
                            &scene.cpu_params[start as usize * 2..end as usize * 2],
                        );
                        let (mo, po) = (start as u64 * 64, start as u64 * 32);
                        if mo + mb.len() as u64 <= buf.size() && po + pb.len() as u64 <= pbuf.size()
                        {
                            buf.write(&self.queue, mo, mb);
                            pbuf.write(&self.queue, po, pb);
                        }
                    }
                }
                for &idx in &scene.changed {
                    if let Some(m) = scene.changed_mark.get_mut(idx) {
                        *m = false;
                    }
                }
                scene.changed.clear();
            }
            return;
        }
        scene.changed.clear();
        scene.changed_mark.clear();
        // one entry per (instance, material slot) so every draw call has its own parameters
        let mut mats: Vec<[[f32; 4]; 4]> = Vec::new();
        let mut params: Vec<[f32; 4]> = Vec::new();
        let ro = scene.render_origin;
        for i in scene.instances.iter_mut() {
            i.base = mats.len() as u32;
            Self::instance_entries(i, ro, &mut mats, &mut params);
        }
        if mats.is_empty() {
            mats.push(Mat4::IDENTITY.to_cols_array_2d());
            params.push([1.0, 1.0, 0.0, 0.0]);
            params.push([1.0, 1.0, 0.0, 0.0]);
        }
        scene.uploaded_instances = scene.instances.len();
        scene.uploaded_entries = mats.len() as u32;
        scene.cpu_models = mats;
        scene.cpu_params = params;
        let bytes: &[u8] = bytemuck::cast_slice(&scene.cpu_models);
        let pbytes: &[u8] = bytemuck::cast_slice(&scene.cpu_params);
        if let (Some(buf), Some(pbuf), Some(_)) = (
            &scene.model_buf,
            &scene.params_buf,
            &scene.camera_bind_group,
        ) {
            if buf.size() >= bytes.len() as u64 && pbuf.size() >= pbytes.len() as u64 {
                buf.write(&self.queue, 0, bytes);
                pbuf.write(&self.queue, 0, pbytes);
                scene.dirty = false;
                return;
            }
        }
        // a third more room than needed, so that the cars and people spawned over the
        // next minutes are appended instead of forcing a rebuild each time
        let cap = |n: usize| (((n as f64 * 1.35) as u64 + 65536).max(256)).div_ceil(256) * 256;
        let model_buf = GpuArray::new(&self.device, "models", cap(bytes.len()), 16, true);
        let params_buf = GpuArray::new(&self.device, "params", cap(pbytes.len()), 16, true);
        model_buf.write(&self.queue, 0, bytes);
        params_buf.write(&self.queue, 0, pbytes);
        scene.model_buf = Some(model_buf);
        scene.params_buf = Some(params_buf);
        self.rebuild_camera_bind_group(scene);
        scene.dirty = false;
    }

    pub(crate) fn rebuild_camera_bind_group(&self, scene: &mut Scene) {
        let (Some(model_buf), Some(params_buf), Some(light_buf), Some(grid_buf), Some(draw_buf)) = (
            &scene.model_buf,
            &scene.params_buf,
            &scene.light_buf,
            &scene.grid_buf,
            &scene.draw_buf,
        ) else {
            return;
        };
        let mut entries = vec![
            wgpu::BindGroupEntry {
                binding: 0,
                resource: self.camera_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: model_buf.binding(),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: params_buf.binding(),
            },
            wgpu::BindGroupEntry {
                binding: 5,
                resource: wgpu::BindingResource::TextureView(&self.shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(&self.shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&self.shadow_view_far),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                // (Enhanced+: the traced lighting, full size, in place of the SSAO)
                resource: wgpu::BindingResource::TextureView(
                    self.rt
                        .as_ref()
                        .and_then(|r| r.targets.as_ref())
                        .map(|t| &t.out)
                        .or(self.ao.as_ref().map(|a| &a.blur_view))
                        .unwrap_or(&self.white_texture.view),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::Sampler(&self.ao_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 10,
                resource: draw_buf.binding(),
            },
            wgpu::BindGroupEntry {
                binding: 11,
                resource: self.enh_buf.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 12,
                resource: wgpu::BindingResource::TextureView(
                    &self.probe.as_ref().expect("reflection probe").view,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::Sampler(&self.lin_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: wgpu::BindingResource::TextureView(&self.sky_lut_view),
            },
            wgpu::BindGroupEntry {
                binding: 17,
                resource: wgpu::BindingResource::TextureView(
                    &self.probe.as_ref().expect("reflection probe").cube_view,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 18,
                resource: wgpu::BindingResource::TextureView(&self.lm_atlas_view),
            },
            wgpu::BindGroupEntry {
                binding: 19,
                resource: self.lm_uniform.as_entire_binding(),
            },
        ];
        // (the point lights where the device has storage buffers for them)
        if array_path() != ArrayPath::NoStorage {
            entries.push(wgpu::BindGroupEntry {
                binding: 3,
                resource: light_buf.binding(),
            });
            entries.push(wgpu::BindGroupEntry {
                binding: 4,
                resource: grid_buf.binding(),
            });
        }
        if sixteen_texture_units() {
            entries.retain(|e| !ENHANCED_CAMERA_TEXTURES.contains(&e.binding));
        }
        let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("camera"),
            layout: &self.camera_layout,
            entries: &entries,
        });
        scene.camera_bind_group = Some(bg);
        let sbg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow camera"),
            layout: &self.shadow_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: self.camera_buf.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: model_buf.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: params_buf.binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 10,
                    resource: draw_buf.binding(),
                },
            ],
        });
        scene.shadow_bind_group = Some(sbg);
    }

    /// Take in the pass times of the last timed frame once its readback has arrived.
    pub(crate) fn collect_gpu_timers(&mut self) {
        let period = self.queue.get_timestamp_period() as f64;
        for t in self.gpu_timers.iter_mut().flatten() {
            t.collect(period);
            // The last frame's stamps are read in a command buffer of their own, submitted
            // after that frame's: resolved in the frame's own buffer, the end stamp of its
            // last pass was often not written yet (Metal) and read as a stale number.
            if t.unresolved {
                t.unresolved = false;
                let n = t.pending.len() as u32 * 2;
                let mut enc = self
                    .device
                    .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("pass timers"),
                    });
                enc.resolve_query_set(&t.set, 0..n, &t.resolve, 0);
                enc.copy_buffer_to_buffer(&t.resolve, 0, &t.read, 0, n as u64 * 8);
                self.queue.submit([enc.finish()]);
                let ready = t.ready.clone();
                t.read.map_async(wgpu::MapMode::Read, .., move |r| {
                    ready.store(r.is_ok(), std::sync::atomic::Ordering::Relaxed)
                });
                t.waiting = true;
            }
        }
    }

    /// Average GPU time per pass so far: (pass, milliseconds, frames measured).
    /// The mirrors' frames (drawn without the overlays) are timed on their own.
    pub fn gpu_pass_times(&self) -> Vec<(String, f64, u32)> {
        let mut out = Vec::new();
        for (k, t) in self.gpu_timers.iter().enumerate() {
            let Some(t) = t else { continue };
            for (label, v) in &t.totals {
                let name = if k == 0 {
                    format!("mirrors: {label}")
                } else {
                    label.to_string()
                };
                out.push((name, v.0 / v.1.max(1) as f64 * 1000.0, v.1));
            }
        }
        out
    }

    /// Upload the frame's draw list, growing its buffer (and the bind groups that hold it)
    /// when it no longer fits.
    pub(crate) fn upload_draw_list(&self, scene: &mut Scene, list: &[u32]) {
        let bytes: &[u8] = bytemuck::cast_slice(if list.is_empty() { &[0u32] } else { list });
        let fits = scene
            .draw_buf
            .as_ref()
            .map(|b| b.size() >= bytes.len() as u64)
            .unwrap_or(false);
        if !fits {
            let cap = (bytes.len() as u64 * 3 / 2).max(1 << 16).div_ceil(4) * 4;
            scene.draw_buf = Some(GpuArray::new(&self.device, "draw list", cap, 4, true));
            self.rebuild_camera_bind_group(scene);
        } else if scene.camera_bind_group.is_none() || scene.shadow_bind_group.is_none() {
            self.rebuild_camera_bind_group(scene);
        }
        if let Some(b) = &scene.draw_buf {
            b.write(&self.queue, 0, bytes);
        }
    }

    /// One step lighter on the graphics card, for a picture the card cannot keep up with at
    /// the smallest render scale: SSAO off. What was switched off, or None when it is off.
    pub fn lighten(&mut self) -> Option<&'static str> {
        if self.options.ssao {
            self.options.ssao = false;
            return Some("ambient occlusion (SSAO) off");
        }
        None
    }

    /// Rebuild without the ray tracing after a GPU error while it was on (see `rt_error`).
    pub(crate) fn fall_back_without_ray_tracing(&mut self, scene: &mut Scene) {
        log::error!(
            "ray tracing failed on {}; Enhanced+ draws as Enhanced from now on",
            self.adapter_name
        );
        let options = RenderOptions {
            ray_tracing: false,
            ..self.options
        };
        RT_BUFFERS.store(false, std::sync::atomic::Ordering::Relaxed);
        // (the meshes' shared pages stay on, as after the multisampling fallback below)
        let mesh_pages = self.mesh_pages;
        *self = Renderer {
            mesh_pages,
            ..Self::build(
                self.device.clone(),
                self.queue.clone(),
                self.adapter_name.clone(),
                self.format,
                options,
            )
        };
        scene.dirty = true;
        scene.model_buf = None;
        scene.params_buf = None;
        scene.camera_bind_group = None;
        scene.shadow_bind_group = None;
    }

    /// Rebuild the pipelines and targets without multisampling after a GPU error with it.
    /// The scene's materials stay valid (the device hands out the same bind group layout
    /// for identical entries); its per-draw buffers and camera bind group are made anew,
    /// because the camera buffer belongs to the renderer.
    pub(crate) fn fall_back_to_single_sample(&mut self, scene: &mut Scene) {
        log::error!(
            "{}x MSAA failed on {}; drawing without multisampling from now on",
            self.options.msaa,
            self.adapter_name
        );
        // Release render targets before building their single-sample replacements. Keeping
        // the old targets alive while the new renderer is built can turn a recoverable OOM
        // into another allocation failure.
        self.depth = None;
        self.msaa_targets.clear();
        self.ao = None;
        self.hdr_targets.clear();
        self.scale_targets.clear();
        self.triple_targets = None;
        self.glass_picture = None;
        self.target_use.clear();
        let options = RenderOptions {
            msaa: 1,
            ..self.options
        };
        let mesh_pages = self.mesh_pages;
        *self = Renderer {
            mesh_pages,
            ..Self::build(
                self.device.clone(),
                self.queue.clone(),
                self.adapter_name.clone(),
                self.format,
                options,
            )
        };
        scene.dirty = true;
        scene.model_buf = None;
        scene.params_buf = None;
        scene.camera_bind_group = None;
        scene.shadow_bind_group = None;
    }

    /// Render the scene into `target` (which must have the renderer's format).
    pub fn render(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
    ) {
        // (the triple screen's panels are not kept once it is turned off)
        if self.triple_targets.take().is_some() {
            self.triple_culling = Default::default();
        }
        self.render_inner(
            scene, target, width, height, camera, lighting, true, None, None, false,
        );
    }

    /// Three independently culled and shaded physical panels, composited into a
    /// spanning window. HUD and menu remain in window pixels and are drawn once.
    pub fn render_triple(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        rig: &TripleScreen,
    ) {
        if width < 3 || height == 0 || self.device_lost().is_some() {
            return;
        }
        let views = rig.views(camera, width, height);
        // all three panels have the same size: they share the size-keyed targets
        let pw = panel_width(width);
        if self.triple_targets.as_ref().map(|t| t.0) != Some((width, height)) {
            let targets = views
                .iter()
                .map(|v| {
                    let w = pw;
                    let texture = self.device.create_texture(&wgpu::TextureDescriptor {
                        label: Some("triple screen panel"),
                        size: wgpu::Extent3d {
                            width: w,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: 1,
                        dimension: wgpu::TextureDimension::D2,
                        format: self.format,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | wgpu::TextureUsages::TEXTURE_BINDING,
                        view_formats: &[],
                    });
                    let view = texture.create_view(&Default::default());
                    // (z: where the panel starts in the window, for `fs_panel`)
                    let params = [w as f32, height as f32, v.viewport[0] as f32, 0.0];
                    let buf = buffer_init(
                        &self.device,
                        &self.queue,
                        Some("triple screen composite"),
                        bytemuck::cast_slice(&params),
                        wgpu::BufferUsages::UNIFORM,
                    );
                    let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                        label: Some("triple screen composite"),
                        layout: &self.upscale_layout,
                        entries: &[
                            wgpu::BindGroupEntry {
                                binding: 0,
                                resource: buf.as_entire_binding(),
                            },
                            wgpu::BindGroupEntry {
                                binding: 1,
                                resource: wgpu::BindingResource::TextureView(&view),
                            },
                            wgpu::BindGroupEntry {
                                binding: 2,
                                resource: wgpu::BindingResource::Sampler(&self.sky_sampler),
                            },
                        ],
                    });
                    (view, group)
                })
                .collect();
            self.triple_targets = Some(((width, height), targets));
        }
        // the HUD is drawn once over the whole window, after the panels: they draw none
        // (its rect buffers are kept aside, not rebuilt each frame)
        let overlays = std::mem::take(&mut scene.overlays);
        let overlay_res = std::mem::take(&mut scene.overlay_res);
        let env_heading = self.env_heading.replace(Some(camera.yaw));
        // Centre meters exposure and updates shared lighting once. All panels
        // then use that same exposure and sun shadow atlas.
        for (turn, i) in [1, 0, 2].into_iter().enumerate() {
            let view = self.triple_targets.as_ref().unwrap().1[i].0.clone();
            self.swap_triple_culling(i);
            self.render_inner(
                scene,
                &view,
                pw,
                height,
                &views[i].camera,
                lighting,
                true,
                None,
                Some(views[i].projection),
                turn != 0,
            );
            self.swap_triple_culling(i);
        }
        self.env_heading.set(env_heading);
        scene.overlays = overlays;
        scene.overlay_res = overlay_res;
        self.prepare_overlays(scene, width, height);
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("triple screen composite"),
            });
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("triple screen composite"),
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
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.panel_pipeline);
            for (i, v) in views.iter().enumerate() {
                // (the right panel loses its last pixel or two at the window's edge)
                let [x, y, w, h] = v.viewport;
                pass.set_scissor_rect(x, y, w.min(width - x), h);
                pass.set_bind_group(0, &self.triple_targets.as_ref().unwrap().1[i].1, &[]);
                pass.draw(0..3, 0..1);
            }
            pass.set_scissor_rect(0, 0, width, height);
            pass.set_pipeline(&self.overlay_pipeline_1x);
            for (_, _, group, _) in &scene.overlay_res {
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }

    pub(crate) fn swap_triple_culling(&mut self, panel: usize) {
        // Visibility and LOD hysteresis belong to one projection. A side panel
        // must not replace the previous centre view's history each frame.
        let history = &mut self.triple_culling[panel];
        std::mem::swap(self.cull_drawn.get_mut(), &mut history.drawn);
        std::mem::swap(self.object_sizes.get_mut(), &mut history.sizes);
        std::mem::swap(self.object_sizes_scratch.get_mut(), &mut history.scratch);
    }

    /// Render one OpenXR view using the headset's asymmetric projection matrix.
    /// The matrix uses the same reversed depth range as the desktop camera. The
    /// second eye reuses the first eye's shadow atlas when both share an origin.
    pub fn render_xr_eye(
        &mut self,
        scene: &mut Scene,
        target: &wgpu::TextureView,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        projection: Mat4,
        second_eye: bool,
    ) {
        self.render_inner(
            scene,
            target,
            width,
            height,
            camera,
            lighting,
            false,
            None,
            Some(projection),
            second_eye,
        );
    }

    /// Composite the shared game menu and a world-positioned pointer into each eye.
    /// Eye-specific positions make the menu fuse into one virtual panel.
    pub fn render_xr_ui(
        &self,
        scene: &Scene,
        eyes: &[wgpu::TextureView; 2],
        desktop_size: (u32, u32),
        eye_size: (u32, u32),
        menu_range: std::ops::Range<usize>,
        menu_transforms: [Mat4; 2],
        cursor_overlay: Option<usize>,
        tooltip_overlay: Option<usize>,
        cursor_transforms: [Option<Mat4>; 2],
        navigator: Option<(TextureId, [Mat4; 2])>,
    ) {
        let Some(menu) = scene.overlays.get(menu_range) else {
            return;
        };
        if menu.is_empty() && cursor_transforms.iter().all(Option::is_none) && navigator.is_none() {
            return;
        }
        let (w, h) = (desktop_size.0.max(1) as f32, desktop_size.1.max(1) as f32);
        let (eye_w, eye_h) = (eye_size.0.max(1) as f32, eye_size.1.max(1) as f32);
        let prepare = |id: &TextureId, quad: [Vec4; 4]| {
            let texture = scene.textures.get(*id)?;
            let mut uniform = [0.0f32; 20];
            for (index, corner) in quad.iter().enumerate() {
                uniform[index * 4..index * 4 + 4].copy_from_slice(&corner.to_array());
            }
            uniform[16] = scene.premultiplied.contains(id) as u8 as f32;
            let buffer = buffer_init(
                &self.device,
                &self.queue,
                Some("OpenXR menu rectangle"),
                bytemuck::cast_slice(&uniform),
                wgpu::BufferUsages::UNIFORM,
            );
            let group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("OpenXR menu rectangle"),
                layout: &self.overlay_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sky_sampler),
                    },
                ],
            });
            Some((buffer, group))
        };
        let mut prepared = [Vec::new(), Vec::new()];
        for eye in 0..2 {
            if let Some((id, transforms)) = navigator.as_ref() {
                let quad = [
                    Vec4::new(-1.0, 1.0, 0.0, 1.0),
                    Vec4::new(1.0, 1.0, 0.0, 1.0),
                    Vec4::new(1.0, -1.0, 0.0, 1.0),
                    Vec4::new(-1.0, -1.0, 0.0, 1.0),
                ]
                .map(|p| transforms[eye] * p);
                if let Some(item) = prepare(id, quad) {
                    prepared[eye].push(item);
                }
            }
            for (index, (id, rect)) in menu.iter().enumerate() {
                // The first rectangle dims the view. Every other rectangle is
                // projected from the same menu plane in world space.
                let transform = if index == 0
                    && rect[0] <= 0.0
                    && rect[1] <= 0.0
                    && rect[2] >= w
                    && rect[3] >= h
                {
                    Mat4::IDENTITY
                } else {
                    menu_transforms[eye]
                };
                let x0 = rect[0] / w * 2.0 - 1.0;
                let y0 = 1.0 - rect[1] / h * 2.0;
                let x1 = rect[2] / w * 2.0 - 1.0;
                let y1 = 1.0 - rect[3] / h * 2.0;
                let quad = [
                    transform * Vec4::new(x0, y0, 0.0, 1.0),
                    transform * Vec4::new(x1, y0, 0.0, 1.0),
                    transform * Vec4::new(x1, y1, 0.0, 1.0),
                    transform * Vec4::new(x0, y1, 0.0, 1.0),
                ];
                if let Some(item) = prepare(id, quad) {
                    prepared[eye].push(item);
                }
            }
            if let (Some(index), Some(transform)) = (cursor_overlay, cursor_transforms[eye]) {
                if let Some((id, rect)) = scene.overlays.get(index) {
                    let half_x = (rect[2] - rect[0]) / eye_w;
                    let half_y = (rect[3] - rect[1]) / eye_h;
                    let quad = [
                        transform * Vec4::new(-half_x, half_y, 0.0, 1.0),
                        transform * Vec4::new(half_x, half_y, 0.0, 1.0),
                        transform * Vec4::new(half_x, -half_y, 0.0, 1.0),
                        transform * Vec4::new(-half_x, -half_y, 0.0, 1.0),
                    ];
                    if let Some(item) = prepare(id, quad) {
                        prepared[eye].push(item);
                    }
                }
                if let Some((id, rect)) = tooltip_overlay.and_then(|i| scene.overlays.get(i)) {
                    let pointer_width = scene
                        .overlays
                        .get(index)
                        .map(|(_, r)| r[2] - r[0])
                        .unwrap_or(24.0);
                    let left = (pointer_width * 0.5 + 8.0) * 2.0 / eye_w;
                    let right = left + (rect[2] - rect[0]) * 2.0 / eye_w;
                    let bottom = -(rect[3] - rect[1]) * 2.0 / eye_h;
                    let quad = [
                        transform * Vec4::new(left, 0.0, 0.0, 1.0),
                        transform * Vec4::new(right, 0.0, 0.0, 1.0),
                        transform * Vec4::new(right, bottom, 0.0, 1.0),
                        transform * Vec4::new(left, bottom, 0.0, 1.0),
                    ];
                    if let Some(item) = prepare(id, quad) {
                        prepared[eye].push(item);
                    }
                }
            }
        }
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("OpenXR menu"),
            });
        for (eye, target) in eyes.iter().enumerate() {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("OpenXR menu"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&self.xr_ui_pipeline);
            for (_, group) in &prepared[eye] {
                pass.set_bind_group(0, group, &[]);
                pass.draw(0..6, 0..1);
            }
        }
        self.queue.submit(Some(encoder.finish()));
    }

    pub(crate) fn prepare_overlays(&self, scene: &mut Scene, full_w: u32, full_h: u32) {
        let overlays = &scene.overlays;
        // the HUD's rect buffers and bind groups live on between frames: making them
        // anew for every overlay of every frame was a steady stream of GPU allocations
        scene.overlay_res.truncate(overlays.len());
        for (k, (tex, r)) in overlays.iter().copied().enumerate() {
            let r = snap_rect(r);
            let ndc = [
                r[0] / full_w as f32 * 2.0 - 1.0,
                1.0 - r[1] / full_h as f32 * 2.0,
                r[2] / full_w as f32 * 2.0 - 1.0,
                1.0 - r[3] / full_h as f32 * 2.0,
                scene.premultiplied.contains(&tex) as u8 as f32,
                scene.transposed.contains(&tex) as u8 as f32,
                0.0,
                0.0,
            ];
            if let Some((_, buf, _, last)) = scene.overlay_res.get_mut(k).filter(|o| o.0 == tex) {
                if *last != ndc {
                    self.queue.write_buffer(buf, 0, bytemuck::cast_slice(&ndc));
                    *last = ndc;
                }
                continue;
            }
            let buf = buffer_init(
                &self.device,
                &self.queue,
                Some("overlay rect"),
                bytemuck::cast_slice(&ndc),
                wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            );
            let bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("overlay"),
                layout: &self.overlay_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: buf.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&scene.textures[tex].view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::Sampler(&self.sky_sampler),
                    },
                ],
            });
            if k < scene.overlay_res.len() {
                scene.overlay_res[k] = (tex, buf, bg, ndc);
            } else {
                scene.overlay_res.push((tex, buf, bg, ndc));
            }
        }
    }

    /// Render off-screen and return RGBA8 pixels.
    pub fn render_to_image(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
    ) -> Result<Vec<u8>> {
        self.render_image(scene, width, height, camera, lighting, None)
    }

    /// Capture all three physical screen projections, including the shared HUD.
    pub fn render_triple_to_image(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        rig: &TripleScreen,
    ) -> Result<Vec<u8>> {
        self.render_image(scene, width, height, camera, lighting, Some(rig))
    }

    pub(crate) fn render_image(
        &mut self,
        scene: &mut Scene,
        width: u32,
        height: u32,
        camera: &Camera,
        lighting: &Lighting,
        rig: Option<&TripleScreen>,
    ) -> Result<Vec<u8>> {
        let tex = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("offscreen"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        // a picture on its own: the enhanced exposure is where the light puts it at once
        self.instant_exposure = true;
        if let Some(rig) = rig {
            self.render_triple(scene, &view, width, height, camera, lighting, rig);
        } else {
            self.render(scene, &view, width, height, camera, lighting);
        }
        self.instant_exposure = false;
        let mut out = self.read_texture(&tex, wgpu::TextureAspect::All)?;
        // BGRA surfaces → swap
        if matches!(
            self.format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for px in out.chunks_exact_mut(4) {
                px.swap(0, 2);
            }
        }
        Ok(out)
    }

    pub(crate) fn read_texture(
        &self,
        texture: &wgpu::Texture,
        aspect: wgpu::TextureAspect,
    ) -> Result<Vec<u8>> {
        let (width, height) = (texture.width(), texture.height());
        let row_bytes = width
            * texture
                .format()
                .block_copy_size(Some(aspect))
                .context("unsupported readback format")?;
        let bpr = row_bytes.div_ceil(256) * 256;
        let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: (bpr * height) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut enc = self.device.create_command_encoder(&Default::default());
        enc.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(bpr),
                    rows_per_image: Some(height),
                },
            },
            wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
        );
        let idx = self.queue.submit([enc.finish()]);
        let slice = buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        wait_gpu(&self.device, Some(idx)).map_err(|e| anyhow!("poll: {e:?}"))?;
        rx.recv()
            .context("map")?
            .map_err(|e| anyhow!("map: {e:?}"))?;
        let data = slice.get_mapped_range();
        let mut out = Vec::with_capacity((row_bytes * height) as usize);
        for row in 0..height {
            let start = (row * bpr) as usize;
            out.extend_from_slice(&data[start..start + row_bytes as usize]);
        }
        drop(data);
        buf.unmap();
        Ok(out)
    }

    /// A texture the scene can be rendered into (`render_to_texture`), e.g. a rear-view mirror.
    pub fn add_render_texture(&self, scene: &mut Scene, width: u32, height: u32) -> TextureId {
        let (width, height) = fit_size(
            width.max(1),
            height.max(1),
            self.device.limits().max_texture_dimension_2d,
        );
        let size = wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("render target"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        // (its depth buffer is the pass's own, see `msaa_targets`)
        let bytes = texture_bytes(self.format, width, height, 1);
        scene.textures.push(GpuTexture {
            texture,
            view,
            size: (width, height),
            bytes,
            gen: next_gen(),
        });
        scene.textures.len() - 1
    }

    /// Render the scene from `camera` into a texture made by `add_render_texture`.
    /// `aspect`: the projection's width over height (OMSI draws its mirrors 1.6 wide into
    /// square textures, and their meshes show the middle of that).
    pub fn render_to_texture(
        &mut self,
        scene: &mut Scene,
        id: TextureId,
        camera: &Camera,
        lighting: &Lighting,
        aspect: f32,
    ) {
        let Some(t) = scene.textures.get(id) else {
            return;
        };
        let view = t.view.clone();
        let (w, h) = t.size;
        self.texture_aspect = Some(aspect);
        self.render_inner(
            scene,
            &view,
            w,
            h,
            camera,
            lighting,
            false,
            Some(id),
            None,
            false,
        );
        self.texture_aspect = None;
    }
}
