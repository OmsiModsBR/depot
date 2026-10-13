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
                resource: wgpu::BindingResource::TextureView(&self.shadows.shadow_view),
            },
            wgpu::BindGroupEntry {
                binding: 6,
                resource: wgpu::BindingResource::Sampler(&self.shadows.shadow_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 7,
                resource: wgpu::BindingResource::TextureView(&self.shadows.shadow_view_far),
            },
            wgpu::BindGroupEntry {
                binding: 8,
                // (Enhanced+: the traced lighting, full size, in place of the SSAO)
                resource: wgpu::BindingResource::TextureView(
                    self.rt
                        .as_ref()
                        .and_then(|r| r.targets.as_ref())
                        .map(|t| &t.out)
                        .or(self.targets.ao.as_ref().map(|a| &a.blur_view))
                        .unwrap_or(&self.assets.white_texture.view),
                ),
            },
            wgpu::BindGroupEntry {
                binding: 9,
                resource: wgpu::BindingResource::Sampler(&self.ssao.ao_sampler),
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
                    &self.sky.probe.as_ref().expect("reflection probe").view,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 13,
                resource: wgpu::BindingResource::Sampler(&self.samplers.lin_sampler),
            },
            wgpu::BindGroupEntry {
                binding: 14,
                resource: wgpu::BindingResource::TextureView(&self.sky.sky_lut_view),
            },
            wgpu::BindGroupEntry {
                binding: 17,
                resource: wgpu::BindingResource::TextureView(
                    &self.sky.probe.as_ref().expect("reflection probe").cube_view,
                ),
            },
            wgpu::BindGroupEntry {
                binding: 18,
                resource: wgpu::BindingResource::TextureView(&self.light_map.lm_atlas_view),
            },
            wgpu::BindGroupEntry {
                binding: 19,
                resource: self.light_map.lm_uniform.as_entire_binding(),
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
            layout: &self.layouts.camera_layout,
            entries: &entries,
        });
        scene.camera_bind_group = Some(bg);
        let sbg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("shadow camera"),
            layout: &self.shadows.shadow_layout,
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
        let mesh_pages = self.assets.mesh_pages;
        let mut rebuilt = Self::build(
            self.device.clone(),
            self.queue.clone(),
            self.adapter_name.clone(),
            self.gpu.format,
            options,
        );
        rebuilt.assets.mesh_pages = mesh_pages;
        *self = rebuilt;
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
        self.targets.depth = None;
        self.targets.msaa_targets.clear();
        self.targets.ao = None;
        self.targets.hdr_targets.clear();
        self.targets.scale_targets.clear();
        self.presenter.triple_targets = None;
        self.targets.glass_picture = None;
        self.targets.target_use.clear();
        let options = RenderOptions {
            msaa: 1,
            ..self.options
        };
        let mesh_pages = self.assets.mesh_pages;
        let mut rebuilt = Self::build(
            self.device.clone(),
            self.queue.clone(),
            self.adapter_name.clone(),
            self.gpu.format,
            options,
        );
        rebuilt.assets.mesh_pages = mesh_pages;
        *self = rebuilt;
        scene.dirty = true;
        scene.model_buf = None;
        scene.params_buf = None;
        scene.camera_bind_group = None;
        scene.shadow_bind_group = None;
    }
}
