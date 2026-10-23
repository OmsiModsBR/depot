//! A picture's draw lists (`Renderer::draw_lists`): the shadow casters, what is in view and
//! the batches of the depth prepass and the main pass.
use super::frame::{DrawLists, FrameClock, FrameContext};
use super::state::Renderer;
use crate::assets::AlphaMode;
use crate::passes::{
    batch_items, culls_back_faces, depth_only_material, depth_prepass_kind, effective_render_phase,
    horizontal_sort_distance, in_scope, instance_depth_bias, nearest_by_origin, origin_key,
    pipe_code, run_parts, surface_depth_coverage, world_surface_phase, Batch, DrawItem,
    PIPE_ALPHA_TEST, PIPE_BLEND, PIPE_BLEND_NO_WRITE, PIPE_KINDS, PIPE_OPAQUE, PIPE_SURFACE_DEPTH,
    PIPE_TERRAIN_PAINT, SHADOW_RANGE, SHADOW_RANGE_CLOSE, SHADOW_RANGE_FAR,
};
use crate::scene::{
    bounding_sphere, cull_blocks, instance_scale, point_in_vehicle_box, Instance, RenderPhase,
    Scene, CULL_BLOCK,
};
use glam::{DVec3, Mat4, Vec3};
use std::collections::HashMap;

/// The shadow pass's `kind_of`: the pipeline a material's alpha mode is drawn with.
fn kind_of(alpha: AlphaMode) -> u8 {
    match alpha {
        AlphaMode::Opaque => PIPE_OPAQUE,
        AlphaMode::Test => PIPE_ALPHA_TEST,
        AlphaMode::Blend => PIPE_BLEND,
    }
}

/// The depth prepass's draws of what is in view: opaque and alpha-tested, single-sampled, as
/// a draw list of their own and their batches over it.
fn prepass_items(scene: &Scene, visible: &[(usize, f32, bool)]) -> (Vec<u32>, Vec<Batch>) {
    let mut items: Vec<DrawItem> = Vec::new();
    let mut list: Vec<u32> = Vec::new();
    let mut batches: Vec<Batch> = Vec::new();
    for &(i, _, _) in visible {
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
}

impl Renderer {
    /// The draw lists of a picture: the shadow casters of each cascade, what is in view, and
    /// the depth prepass's and the main pass's batches, all over one draw list uploaded at the
    /// end.
    pub(crate) fn draw_lists(
        &mut self,
        scene: &mut Scene,
        f: &FrameContext,
        clock: &mut FrameClock,
    ) -> DrawLists {
        let debug_draws = omsi_cfg::flags::DEBUG_DRAWS.is_set();
        let debug_cull = omsi_cfg::flags::DEBUG_CULL.is_set();
        let with_overlays = f.with_overlays;
        let prepass_on = f.features.prepass_on;
        let glass_on = f.features.glass_on;
        let mut list: Vec<u32> = Vec::new();
        clock.stage(self, "setup", "mirror.setup");
        let shadow_batches = self.shadow_casters(scene, f, &mut list);
        clock.stage(self, "shadow items", "mirror.shadow items");
        let visible = self.visibility(scene, f, clock, debug_cull);
        let only_surfaces = omsi_cfg::flags::ONLY_SURFACES.is_set();
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
        // Keep mesh/material order here: an excavation's floor is drawn before its
        // invisible cover writes depth. Sorting its blended cover after the terrain
        // leaves the terrain's colour in place even though the cover writes depth.
        let has_presurface = visible
            .iter()
            .any(|&(i, _, _)| scene.instances[i].presurface);
        let mut prepass_found: Option<(Vec<u32>, Vec<Batch>)> = None;
        let pool = self.gpu.encoding_pool.as_ref();
        let scene_ref: &Scene = scene;
        let mut main: (Vec<Batch>, Vec<Batch>, [usize; 2]) = (Vec::new(), Vec::new(), [0; 2]);
        in_scope(pool, |scope| {
            if prepass_on {
                let slot = &mut prepass_found;
                let visible = &visible;
                scope.spawn(move |_| *slot = Some(prepass_items(scene_ref, visible)));
            }
            main = self.main_items(scene_ref, f, &visible, &mut list);
        });
        let (mut main_batches, mut cab_batches, main_draws) = main;
        let mut prepass_batches: Vec<Batch> = Vec::new();
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
        if let Some(skip) = omsi_cfg::flags::SKIP_PIPE.text() {
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
        clock.stage(self, "items", "mirror.items");
        let mut rain_batches = Vec::new();
        if glass_on {
            let (rain, main): (Vec<_>, Vec<_>) = main_batches
                .into_iter()
                .partition(|b| scene.materials[b.material as usize].uniform.emissive[3] > 1.5);
            rain_batches = rain;
            main_batches = main;
        }
        self.upload_draw_list(scene, &list);
        clock.stage(self, "upload", "mirror.upload");
        DrawLists {
            list,
            shadow_batches,
            prepass_batches,
            main_batches,
            cab_batches,
            rain_batches,
            has_presurface,
        }
    }

    /// The shadow casters of each cascade (near, far, close, and the street lamps'), with
    /// their draws appended to `list`.
    fn shadow_casters(
        &self,
        scene: &Scene,
        f: &FrameContext,
        list: &mut Vec<u32>,
    ) -> [Vec<Batch>; 4] {
        let debug_draws = omsi_cfg::flags::DEBUG_DRAWS.is_set();
        let camera = f.camera;
        let cam_rel = f.cam_rel;
        let lamp_shadows = &f.lamp_shadows;
        let draw_shadows = f.draw_shadows;
        let redraw_near = f.redraw_near;
        let redraw_far = f.redraw_far;
        let light_view_proj = f.light_view_proj;
        let light_view_proj_far = f.light_view_proj_far;
        let light_view_proj_close = f.light_view_proj_close;
        let moon_shadows = f.moon_shadows;
        let shadows = f.shadows;
        let rt_frame = f.features.rt_frame;
        let omsi_shadow_casters = self.options.omsi_shadow_casters;
        // near, far, close
        // (3: the street lamps' maps, every caster within a chosen lamp's reach under its head)
        let mut shadow_batches: [Vec<Batch>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
        let lamp_reach = |c: Vec3, r: f32| {
            lamp_shadows
                .iter()
                .any(|l| c.z - r < l.position.z && (c - l.position).length() < l.range + r)
        };
        // an instance's screen size as the camera pass measures it for the LOD choice
        let lod_fov = camera.fov_deg.to_radians().max(1e-3);
        let lod_size = |inst: &Instance| -> f32 {
            let scale = instance_scale(scene, inst);
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
        let dbg_shadow = omsi_cfg::flags::DEBUG_SHADOW.is_set();
        let dbg_r: f32 = omsi_cfg::flags::DEBUG_SHADOW
            .text()
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(3.0);
        let casters = |span: std::ops::Range<usize>| -> [Vec<DrawItem>; 4] {
            let mut out: [Vec<DrawItem>; 4] = [Vec::new(), Vec::new(), Vec::new(), Vec::new()];
            let mut ranges: Vec<(u8, u32, u32, usize, u32)> = Vec::new();
            for inst in &scene.instances[span] {
                if !inst.visible || !inst.casts_shadow || (omsi_shadow_casters && !inst.omsi_caster)
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
                let (c, r) = bounding_sphere(scene, inst);
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
            let blocks = cull_blocks(scene);
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
                    list,
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
        shadow_batches
    }

    /// The main pass's batches over `list`: the authored world phases, far to near, and the
    /// vehicle the camera is in, last. Returns the main batches, the vehicle's batches and the
    /// draw counts of the opaque and of the blended draws.
    fn main_items(
        &self,
        scene: &Scene,
        f: &FrameContext,
        visible: &[(usize, f32, bool)],
        list: &mut Vec<u32>,
    ) -> (Vec<Batch>, Vec<Batch>, [usize; 2]) {
        let (camera, lighting) = (f.camera, f.lighting);
        let exclude_texture = f.exclude_texture;
        let ro = f.ro;
        let cam_rel = f.cam_rel;
        let mut items: Vec<DrawItem> = Vec::new();
        let mut main_batches: Vec<Batch> = Vec::new();
        let mut cab_batches: Vec<Batch> = Vec::new();
        let mut cab_items: Vec<DrawItem> = Vec::new();
        let mut main_draws = [0usize; 2];
        let mut by_phase: [Vec<(usize, f32, bool)>; RenderPhase::COUNT] =
            std::array::from_fn(|_| Vec::new());
        for &entry in visible {
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
                    for (ri, (_, _, slot)) in scene.meshes[inst.mesh].ranges.iter().enumerate() {
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
                batch_items(scene, &mut items, true, list, &mut main_batches);
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
            batch_items(scene, &mut items, true, list, &mut main_batches);
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
                    let (c, r) = bounding_sphere(scene, inst);
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
            batch_items(scene, &mut items, false, list, &mut main_batches);
        }
        // Omsi.exe draws the vehicle the camera sits in last of all, with the view mask
        // of its inside (0x6f1520 -> 0x6f0430), after every phase of the map, the other
        // vehicles, the particles and the lamps' flares (0x6f0400/0x6f0418): its glass,
        // whose depth is written unless the model says `[matl_noZwrite]`, then lies over
        // all of that. Its items wait here and are drawn after the coronas and the smoke
        // (see `cab_batches` in the main pass).
        main_draws[1] += cab_items.len();
        batch_items(scene, &mut cab_items, false, list, &mut cab_batches);
        (main_batches, cab_batches, main_draws)
    }

    /// What the picture shows: the instances in the view's frustum, before the fog and big
    /// enough on the screen (with the main view's history, so that an object at a limit does
    /// not pop), with their distance and whether the camera is inside them.
    pub(crate) fn visibility(
        &self,
        scene: &Scene,
        f: &FrameContext,
        clock: &mut FrameClock,
        debug_cull: bool,
    ) -> Vec<(usize, f32, bool)> {
        let (camera, lighting) = (f.camera, f.lighting);
        let with_overlays = f.with_overlays;
        let projection = f.projection;
        let aspect = f.aspect;
        let cam_rel = f.cam_rel;
        let xr_view = f.features.xr_view;
        let enhanced_frame = f.features.enhanced_frame;
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
        if let Some(p) = omsi_cfg::flags::DEBUG_CULL.text().and_then(|v| {
            let f: Vec<f64> = v.split(',').filter_map(|x| x.trim().parse().ok()).collect();
            (f.len() == 3).then(|| (DVec3::new(f[0], f[1], 0.0), f[2]))
        }) {
            for (i, inst) in scene.instances.iter().enumerate() {
                if (inst.origin.truncate() - p.0.truncate()).length() > p.1 || !with_overlays {
                    continue;
                }
                let m = &scene.meshes[inst.mesh];
                let (c, _) = bounding_sphere(scene, inst);
                let v = view.transform_point3(c);
                log::info!("cull {i}: mesh {} r {:.2} centre {:?} origin {:?} view {:?} visible {} lod {:?} tan ({tan_x:.2}, {tan_y:.2}) fog {fog_far:.0} surface {} decal {} casts {} phase {:?} source {:?}", inst.mesh, m.bounds_radius, m.bounds_center, inst.origin, v, inst.visible, inst.lod, inst.surface, inst.decal, inst.casts_shadow, inst.render_phase, m.source);
            }
        }
        let fov_y = camera.fov_deg.to_radians().max(1e-3);
        let max_obj_dist = self.options.max_obj_dist;
        let shadow_blobs = self.shadow_blobs;
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
            if inst.blob && !shadow_blobs {
                return None;
            }
            let (c, r) = bounding_sphere(scene, inst);
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
                let scale = instance_scale(scene, inst);
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
        let blocks = cull_blocks(scene);
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
        if with_overlays && omsi_cfg::flags::DEBUG_FLICKER.is_set() {
            let drawn: std::collections::HashSet<usize> = visible.iter().map(|v| v.0).collect();
            let mut prev = self.culling.flicker.borrow_mut();
            let mut now: HashMap<usize, bool> = HashMap::new();
            for (i, inst) in scene.instances.iter().enumerate() {
                let m = &scene.meshes[inst.mesh];
                if m.ranges.is_empty() || !inst.visible || inst.surface {
                    continue;
                }
                let (c, r) = bounding_sphere(scene, inst);
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
        clock.stage(self, "cull", "mirror.cull");
        visible
    }
}
