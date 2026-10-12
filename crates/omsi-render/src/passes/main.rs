//! The main pass: the scene's pipelines and drawing the scene into the picture.

use super::draw::{
    encode_batches, encode_batches_filtered, main_pipeline, Batch, PIPE_ALPHA_TEST, PIPE_BLEND,
    PIPE_BLEND_NO_WRITE,
};
use crate::gpu::pass_timer;
use crate::lighting::Lighting;
use crate::renderer::Renderer;
use crate::scene::{Scene, TextureId};
use crate::shaders::{snowfall_flakes, SnowUniform};

/// The pipelines of the main pass for one colour target format: the swap chain's, and
/// the high-range one of the enhanced path.
pub(crate) struct PassPipelines {
    /// Single-sampled films, drawn after resolving the scene and its reflections.
    pub(crate) rain_pipelines: Vec<wgpu::RenderPipeline>,
    /// Indexed by `pipe_code`: 4 depth/blend kinds x culled x depth-biased.
    pub(crate) pipelines: Vec<wgpu::RenderPipeline>,
    pub(crate) corona_pipeline: wgpu::RenderPipeline,
    /// Smoke particles (`[smoke]`): the corona sprite alpha-blended with the smoke texture.
    pub(crate) smoke_pipeline: wgpu::RenderPipeline,
    /// The snowfall (snow.wgsl), opaque over the scene (premultiplied); none with
    /// `basic_pipelines`.
    pub(crate) snow_pipeline: Option<wgpu::RenderPipeline>,
    pub(crate) sky_pipeline: wgpu::RenderPipeline,
}

impl Renderer {
    pub(crate) fn main_pass(&self, enhanced: bool, reflections: bool) -> &PassPipelines {
        if enhanced {
            self.hdr_pass.as_ref().unwrap()
        } else if reflections {
            self.reflection_pass.as_ref().unwrap()
        } else {
            &self.pass
        }
    }

    /// The main pass: the scene over the sky, with its depth from the prepass where it can
    /// share it, split over several encoders when the bundles allow (the parts are returned).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_main_pass(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        lighting: &Lighting,
        scene_view: &wgpu::TextureView,
        main_batches: &[Batch],
        cab_batches: &[Batch],
        prepass_batches: &[Batch],
        main_bundles: &[wgpu::RenderBundle],
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        enhanced: bool,
        reflection_frame: bool,
        masked_frame: bool,
        has_presurface: bool,
        prepass_on: bool,
        ao_on: bool,
        puddles_wanted: bool,
        scaled: bool,
        with_overlays: bool,
        xr_view: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) -> Vec<wgpu::CommandEncoder> {
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
            prepass_on && single && self.targets.ao.is_some() && !has_presurface && !puddles_wanted;
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
            || main_batches.iter().chain(cab_batches).any(|batch| {
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
                let mut pass = lead.as_mut().unwrap_or(&mut *encoder).begin_render_pass(
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
                        timestamp_writes: pass_timer(tset.as_ref(), timed, "msaa prepass"),
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
                None => &self.targets.ao.as_ref().unwrap().depth_view,
            };
            // the enhanced path draws into a high-range picture the post pass then grades
            let hdr = if masked_frame {
                self.targets.hdr_targets.get(&(width, height))
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
                        && self.sky.sky_state.is_some()
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
                            pass.set_vertex_buffer(0, self.sky.sky_mesh.0.slice(..));
                            pass.set_index_buffer(
                                self.sky.sky_mesh.1.slice(..),
                                wgpu::IndexFormat::Uint32,
                            );
                            pass.draw_indexed(0..self.sky.sky_mesh.2, 0, 0..1);
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
                    timed,
                    if with_overlays { "main" } else { "mirror" },
                ),
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_bind_group(0, scene.camera_bind_group.as_ref().unwrap(), &[]);
            if let Some(sky) = scene.sky_bind_group.as_ref().filter(|_| parts == 1) {
                pass.set_pipeline(sky_pipe);
                pass.set_bind_group(1, sky, &[]);
                pass.set_vertex_buffer(0, self.sky.sky_mesh.0.slice(..));
                pass.set_index_buffer(self.sky.sky_mesh.1.slice(..), wgpu::IndexFormat::Uint32);
                pass.draw_indexed(0..self.sky.sky_mesh.2, 0, 0..1);
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
        main_parts
    }
}
