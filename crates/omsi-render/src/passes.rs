//! The passes of a frame, as `Renderer::render_inner` records them.

use super::*;

impl Renderer {
    /// The sun's shadow maps (the near and far cascades, the close one beside the near) and
    /// the street lamps' maps under the far one.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_shadow_maps(
        &self,
        scene: &Scene,
        shadow_encoder: &mut wgpu::CommandEncoder,
        shadow_batches: &[Vec<Batch>; 4],
        lamp_shadows: &[LampShadow],
        draw_shadows: bool,
        redraw_near: bool,
        redraw_far: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
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
                    timed,
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
                timestamp_writes: pass_timer(tset.as_ref(), timed, "lamp shadows"),
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
    }
}

impl Renderer {
    /// The depth prepass and the ambient occlusion (single-sampled, with the camera's
    /// projection).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_prepass_and_ao(
        &mut self,
        scene: &Scene,
        prepass_encoder: &mut wgpu::CommandEncoder,
        prepass_batches: &[Batch],
        camera: &Camera,
        projection: Option<Mat4>,
        aspect: f32,
        width: u32,
        height: u32,
        prepass_on: bool,
        ao_on: bool,
        rt_frame: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
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
                    timestamp_writes: pass_timer(tset.as_ref(), timed, "depth prepass"),
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
                    timestamp_writes: pass_timer(tset.as_ref(), timed, pass_label),
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(pipe);
                pass.set_bind_group(0, bg, &[]);
                pass.draw(0..3, 0..1);
            }
        }
        if rt_frame {
            self.encode_ray_tracing(prepass_encoder, scene, tset.as_ref(), timed);
        }
    }
}

impl Renderer {
    /// The enhanced sky cube: a face a frame, all six the first time and for a new sky.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_sky_cube(
        &mut self,
        scene: &Scene,
        prepass_encoder: &mut wgpu::CommandEncoder,
        enhanced: bool,
        lead_view: bool,
        probe_redraw: bool,
        redraw_near: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
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
                            pass_timer(tset.as_ref(), timed, "sky cube")
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
    }
}

impl Renderer {
    /// The enhanced path's reflection probe: the sky into its six faces, then the blurred
    /// mip levels.
    pub(crate) fn encode_probe(
        &mut self,
        scene: &Scene,
        prepass_encoder: &mut wgpu::CommandEncoder,
        probe_redraw: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
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
                                    pass_timer(tset.as_ref(), timed, "probe")
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
    }
}

impl Renderer {
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
        main_parts
    }
}

impl Renderer {
    /// With a render scale: the smaller picture scaled up to the window, the overlays on top
    /// at full size.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_present(
        &self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        scene_target: &Option<(wgpu::TextureView, wgpu::BindGroup)>,
        target: &wgpu::TextureView,
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        full_w: u32,
        vanilla_fxaa: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
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
                timestamp_writes: pass_timer(tset.as_ref(), timed, "upscale"),
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
    }
}

impl Renderer {
    /// The enhanced path's post passes: glow, metering, adaptation, tone curve and FXAA.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_enhanced_post(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        lighting: &Lighting,
        scene_view: &wgpu::TextureView,
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        dt: f32,
        enhanced: bool,
        lead_view: bool,
        puddles_on: bool,
        rt_frame: bool,
        scaled: bool,
        with_overlays: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
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
                        encoder,
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
                        encoder,
                        &self.meter_view,
                        None,
                        &self.post.meter,
                        &h.meter_bg,
                    );
                    let front = self.adapt_front;
                    post_pass(
                        encoder,
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
                    log.sample(encoder, &self.adapt_views[self.adapt_front], pre, m);
                }
                // (timed on its last pass: the glow chain with the metering, see GpuTimers)
                for i in (0..levels).rev() {
                    let timer = if i == 0 {
                        pass_timer(tset.as_ref(), timed, "glow+meter")
                    } else {
                        None
                    };
                    post_pass(encoder, &h.up[i], timer, &self.post.up, &h.up_bg[i]);
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
                    timestamp_writes: pass_timer(tset.as_ref(), timed, "tone map"),
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
                        timestamp_writes: pass_timer(tset.as_ref(), timed, "fxaa"),
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
    }
}

impl Renderer {
    /// Vanilla with reflections: the high-range picture brought into the target, the overlays
    /// on top when the picture is not scaled.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_reflection_resolve(
        &self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        overlays: &[(TextureId, [f32; 4])],
        width: u32,
        height: u32,
        puddles_on: bool,
        reflection_frame: bool,
        scaled: bool,
    ) {
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
    }
}

impl Renderer {
    /// The lamps' light in the weather's fog and the sun's shafts between the shadows, over the
    /// picture before its glow and metering.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_fog_lamps(
        &self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        lighting: &Lighting,
        vp_mat: Mat4,
        width: u32,
        height: u32,
        enhanced: bool,
        masked_frame: bool,
        prepass_on: bool,
        puddles_on: bool,
        with_overlays: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) {
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
                                pass_timer(tset.as_ref(), timed, "fog lamps")
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
    }
}

impl Renderer {
    /// The reflections: Enhanced+'s traced ones, or the puddles' (whether they are drawn is
    /// returned).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_reflections(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        camera: &Camera,
        lighting: &Lighting,
        cu: &CameraUniform,
        list: &[u32],
        main_batches: &[Batch],
        cab_batches: &[Batch],
        projection: Option<Mat4>,
        ro: DVec3,
        aspect: f32,
        width: u32,
        height: u32,
        puddles_wanted: bool,
        rt_frame: bool,
        tset: &Option<wgpu::QuerySet>,
        timed: &mut Vec<&'static str>,
    ) -> bool {
        // Weather alone is insufficient: leave the allocation and reflection passes out when
        // the visible batches contain no moisture-tagged surface (a showroom, bare terrain).
        // Enhanced+: the traced reflections (wet roads' too) in place of the puddles' rays
        if rt_frame {
            self.encode_rt_reflections(encoder, width, height, tset.as_ref(), timed);
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
                main_batches
            } else {
                all = main_batches.iter().chain(cab_batches).cloned().collect();
                &all
            };
            self.encode_puddle_reflections(
                encoder,
                width,
                height,
                scene,
                batches,
                &list,
                lighting,
                camera,
                tset.as_ref(),
                timed,
            );
        }
        puddles_on
    }
}

impl Renderer {
    /// The rain films on the glass: a copy of the clean picture behind them, then the panes'
    /// drops drawn over it.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn encode_glass(
        &mut self,
        scene: &Scene,
        encoder: &mut wgpu::CommandEncoder,
        scene_view: &wgpu::TextureView,
        rain_batches: &[Batch],
        width: u32,
        height: u32,
        enhanced: bool,
        glass_on: bool,
        masked_frame: bool,
        puddles_on: bool,
        reflection_frame: bool,
    ) {
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
    }
}
