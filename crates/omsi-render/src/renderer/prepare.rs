//! Preparing a frame: the instances' records, the camera bind group, the draw lists, and
//! falling back when the device cannot keep up.

use super::options::RenderOptions;
use super::state::Renderer;
use crate::assets::RT_BUFFERS;
use crate::gpu::{array_path, sixteen_texture_units, ArrayPath, GpuArray};
use crate::passes::surface_instance_code;
use crate::scene::{Instance, Scene, ENHANCED_CAMERA_TEXTURES};
use glam::{DVec3, Mat4, Vec3};

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
}
