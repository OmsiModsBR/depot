//! Preparing a frame: the camera bind group, the draw lists, and falling back when the
//! device cannot keep up. The instances' records are uploaded in `upload`.

use super::options::RenderOptions;
use super::state::Renderer;
use crate::assets::RT_BUFFERS;
use crate::gpu::{array_path, sixteen_texture_units, ArrayPath, GpuArray};
use crate::scene::{Scene, ENHANCED_CAMERA_TEXTURES};

impl Renderer {
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
        let rebuilt = Self::build(
            self.device.clone(),
            self.queue.clone(),
            self.adapter_name.clone(),
            self.gpu.format,
            options,
        );
        self.take_over(rebuilt, scene);
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
        let rebuilt = Self::build(
            self.device.clone(),
            self.queue.clone(),
            self.adapter_name.clone(),
            self.gpu.format,
            options,
        );
        self.take_over(rebuilt, scene);
    }

    /// Put `rebuilt` (the same device, other options) in this renderer's place, with what the
    /// game handed this one: the coronas' and the smoke's pictures, the tile light maps and
    /// where they lie, and the settings changed while it ran. The game uploads these once
    /// (at the start, or as the tiles come), so a fresh renderer would draw the coronas
    /// without their pictures and the nights without the light pools from then on.
    fn take_over(&mut self, mut rebuilt: Renderer, scene: &mut Scene) {
        // (the meshes' shared pages stay on)
        rebuilt.assets.mesh_pages = self.assets.mesh_pages;
        // (what the old device context noted goes on: an "out of memory" that switched the
        // multisampling off still has to cut the textures, #48)
        rebuilt.gpu.inherit(&self.gpu);
        // (the pictures' bind groups stay valid: the device hands out the same bind group
        // layout for identical entries, as for the scene's materials)
        rebuilt.sprites.corona_textures = std::mem::take(&mut self.sprites.corona_textures);
        std::mem::swap(
            &mut rebuilt.sprites.smoke_bind_group,
            &mut self.sprites.smoke_bind_group,
        );
        std::mem::swap(&mut rebuilt.light_map, &mut self.light_map);
        rebuilt.env_heading.set(self.env_heading.get());
        rebuilt.dynamic_scale.set(self.dynamic_scale.get());
        rebuilt.blend_by_origin = self.blend_by_origin;
        rebuilt.shadow_blobs = self.shadow_blobs;
        *self = rebuilt;
        scene.dirty = true;
        scene.model_buf = None;
        scene.params_buf = None;
        scene.camera_bind_group = None;
        scene.shadow_bind_group = None;
    }
}

#[cfg(test)]
mod fallback_tests {
    use super::*;

    fn noop_renderer() -> Renderer {
        let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
        descriptor.backends = wgpu::Backends::NOOP;
        descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
        let instance = wgpu::Instance::new(descriptor);
        pollster::block_on(Renderer::new_with(
            &instance,
            None,
            Some(wgpu::TextureFormat::Rgba8UnormSrgb),
            RenderOptions {
                msaa: 1,
                shadow_size: 1024,
                ..Default::default()
            },
        ))
        .expect("noop renderer")
    }

    #[test]
    fn a_fallback_keeps_what_the_game_uploaded() {
        let img = omsi_texture::Image {
            width: 2,
            height: 2,
            rgba: vec![255; 16],
            has_alpha: true,
        };
        for rt in [false, true] {
            let mut r = noop_renderer();
            let mut scene = r.new_scene();
            r.set_corona_texture(3, &img);
            r.set_light_map_tile((1, 2), Some(&img));
            r.set_light_map_place(100.0, 200.0, 300.0);
            r.set_dynamic_scale(0.7);
            r.set_env_heading(Some(12.0));
            r.shadow_blobs = !r.shadow_blobs;
            r.blend_by_origin = true;
            let blobs = r.shadow_blobs;
            if rt {
                r.fall_back_without_ray_tracing(&mut scene);
            } else {
                r.fall_back_to_single_sample(&mut scene);
            }
            assert!(
                r.sprites
                    .corona_textures
                    .get(3)
                    .is_some_and(|t| t.is_some()),
                "rt {rt}"
            );
            assert_eq!(r.light_map.lm_place.get(), (100.0, 200.0, 300.0));
            assert_eq!(r.dynamic_scale(), 0.7);
            assert_eq!(r.env_heading.get(), Some(12.0));
            assert_eq!(r.shadow_blobs, blobs);
            assert!(r.blend_by_origin);
            assert!(scene.dirty && scene.camera_bind_group.is_none());
        }
    }
}
