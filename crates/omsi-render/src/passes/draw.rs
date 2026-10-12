//! Draw lists: the draw items and their batches, the `pipe_code` kinds, and recording the
//! batches into passes and render bundles.

use super::main::PassPipelines;
use crate::assets::{AlphaMode, Material};
use crate::gpu::CATCHING;
use crate::renderer::{rt_gbuf, AUX_FORMAT, DEPTH_FORMAT, GBUF_FORMAT, HDR_FORMAT, MASK_FORMAT};
use crate::scene::{Instance, MaterialId, RenderPhase, Scene};
use glam::{DVec3, Vec3};
use std::collections::HashMap;

/// One single-sampled full-screen pass.
pub(crate) fn post_pass(
    encoder: &mut wgpu::CommandEncoder,
    view: &wgpu::TextureView,
    timer: Option<wgpu::RenderPassTimestampWrites<'_>>,
    pipeline: &wgpu::RenderPipeline,
    bg: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("post"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view,
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: timer,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, bg, &[]);
    pass.draw(0..3, 0..1);
}

pub(crate) fn origin_key(origin: DVec3) -> [u64; 3] {
    // DVec3 equality treats -0 and +0 alike. Keep that property in the hash key.
    origin
        .to_array()
        .map(|v| if v == 0.0 { 0 } else { v.to_bits() })
}

/// For each distinct origin, the object's nearest blended mesh distance, rather than
/// the single point all its meshes share (a long vehicle's origin can be far from its
/// window nearest the camera). Full coordinates and the existing float equality apply.
pub(crate) fn nearest_by_origin(
    items: impl IntoIterator<Item = (DVec3, f32)>,
) -> HashMap<[u64; 3], f32> {
    let mut out: HashMap<[u64; 3], f32> = HashMap::new();
    for (origin, d) in items {
        // NaN origins never compared equal in the old lookup either.
        if origin.is_nan() {
            continue;
        }
        out.entry(origin_key(origin))
            .and_modify(|best| *best = best.min(d))
            .or_insert(d);
    }
    out
}

/// One draw of a mesh range with a material for one per-draw entry, before batching.
/// `pipe` picks the pipeline within a pass (and orders the batches).
#[derive(Clone, Copy)]
pub(crate) struct DrawItem {
    pub(crate) pipe: u8,
    pub(crate) mesh: u32,
    pub(crate) range: u32,
    pub(crate) material: u32,
    /// The material's look: draws of materials that look alike are batched together.
    pub(crate) look: u32,
    pub(crate) entry: u32,
}

/// Draws of the same mesh range with materials that look alike and the same pipeline, made as one instanced
/// draw: `instances` indexes the frame's draw list, which holds each instance's per-draw
/// entry (the vertex shader looks it up). Thousands of single draws were the biggest CPU
/// cost of a frame - wgpu validates and records every one - and trees, lamps, fences,
/// people and the AI cars' shared meshes collapse into a few hundred batches.
#[derive(Clone)]
pub(crate) struct Batch {
    pub(crate) pipe: u8,
    pub(crate) mesh: u32,
    pub(crate) first: u32,
    pub(crate) count: u32,
    pub(crate) material: u32,
    pub(crate) instances: std::ops::Range<u32>,
}

/// Turn draw items into batches, appending their entries to `list`. `sort`: the order does
/// not matter (depth-tested opaque and alpha-tested draws), so draws alike are gathered;
/// otherwise only neighbours are merged (the blended pass keeps its far-to-near order).
pub(crate) fn batch_items(
    scene: &Scene,
    items: &mut [DrawItem],
    sort: bool,
    list: &mut Vec<u32>,
    out: &mut Vec<Batch>,
) {
    // a batch keeps the material of the one before it when they look alike: no rebinding
    let mut last = (u32::MAX, 0);
    let mut push = |d: &DrawItem, start: u32, list: &Vec<u32>| {
        if last.0 != d.look {
            last = (d.look, d.material);
        }
        let (first, count, _) = scene.meshes[d.mesh as usize].ranges[d.range as usize];
        out.push(Batch {
            pipe: d.pipe,
            mesh: d.mesh,
            first,
            count,
            material: last.1,
            instances: start..list.len() as u32,
        });
    };
    if sort {
        let bits = |n: u32| u32::BITS - n.leading_zeros();
        let (mut look, mut mesh, mut range) = (0, 0, 0);
        for d in items.iter() {
            (look, mesh, range) = (look | d.look, mesh | d.mesh, range | d.range);
        }
        let mesh_shift = bits(range);
        let look_shift = mesh_shift + bits(mesh);
        let pipe_shift = look_shift + bits(look);
        if pipe_shift + 8 <= 64 {
            // one number a draw sorts twice as fast as comparing the four fields
            let mut keys: Vec<(u64, u32, u32)> = items
                .iter()
                .map(|d| {
                    let key = (d.pipe as u64) << pipe_shift
                        | (d.look as u64) << look_shift
                        | (d.mesh as u64) << mesh_shift
                        | d.range as u64;
                    (key, d.entry, d.material)
                })
                .collect();
            keys.sort_unstable_by_key(|k| k.0);
            let field =
                |key: u64, from: u32, to: u32| ((key >> from) & ((1 << (to - from)) - 1)) as u32;
            let mut k = 0;
            while k < keys.len() {
                let (key, _, material) = keys[k];
                let start = list.len() as u32;
                while k < keys.len() && keys[k].0 == key {
                    list.push(keys[k].1);
                    k += 1;
                }
                let d = DrawItem {
                    pipe: (key >> pipe_shift) as u8,
                    mesh: field(key, mesh_shift, look_shift),
                    range: field(key, 0, mesh_shift),
                    material,
                    look: field(key, look_shift, pipe_shift),
                    entry: 0,
                };
                push(&d, start, list);
            }
            return;
        }
        items.sort_unstable_by_key(|d| (d.pipe, d.look, d.mesh, d.range));
    }
    let mut k = 0;
    while k < items.len() {
        let d = items[k];
        let start = list.len() as u32;
        while k < items.len()
            && (items[k].pipe, items[k].mesh, items[k].range, items[k].look)
                == (d.pipe, d.mesh, d.range, d.look)
        {
            list.push(items[k].entry);
            k += 1;
        }
        push(&d, start, list);
    }
}

/// The material and look a depth-only draw (shadow map, depth prepass) is batched with: an
/// opaque surface writes its depth whatever its texture, so all of them share the first
/// material and batch across materials; an alpha-tested one needs its own texture for the cut-out.
pub(crate) fn depth_only_material(kind: u8, material: MaterialId, look: u32) -> (u32, u32) {
    if kind == 0 {
        (0, 0)
    } else {
        (material as u32, look)
    }
}

/// Record batches into a pass or a bundle, setting pipeline, buffers and material only
/// when they change.
pub(crate) fn encode_batches<'a, E: wgpu::util::RenderEncoder<'a>>(
    pass: &mut E,
    scene: &'a Scene,
    batches: &[Batch],
    pipeline: impl Fn(u8) -> &'a wgpu::RenderPipeline,
) {
    encode_batches_filtered(pass, scene, batches, |_| true, pipeline);
}

/// Record the batches accepted by `include`, setting pipeline, buffers and material only
/// when they change.
pub(crate) fn encode_batches_filtered<'a, E: wgpu::util::RenderEncoder<'a>>(
    pass: &mut E,
    scene: &'a Scene,
    batches: &[Batch],
    include: impl Fn(&Batch) -> bool,
    pipeline: impl Fn(u8) -> &'a wgpu::RenderPipeline,
) {
    let (mut pipe, mut mesh, mut material) = (u8::MAX, u32::MAX, u32::MAX);
    let mut page: Option<(&wgpu::Buffer, &wgpu::Buffer)> = None;
    let (mut base_vertex, mut first_index) = (0i32, 0u32);
    for b in batches {
        if !include(b) {
            continue;
        }
        if b.pipe != pipe {
            pass.set_pipeline(pipeline(b.pipe));
            pipe = b.pipe;
        }
        if b.mesh != mesh {
            let m = &scene.meshes[b.mesh as usize];
            if page.is_none_or(|(v, i)| *v != m.vertex_buf || *i != m.index_buf) {
                pass.set_vertex_buffer(0, m.vertex_buf.slice(..));
                pass.set_index_buffer(m.index_buf.slice(..), wgpu::IndexFormat::Uint32);
                page = Some((&m.vertex_buf, &m.index_buf));
            }
            (base_vertex, first_index) = (m.base_vertex, m.first_index);
            mesh = b.mesh;
        }
        if b.material != material {
            pass.set_bind_group(
                1,
                Some(&scene.materials[b.material as usize].bind_group),
                &[],
            );
            material = b.material;
        }
        pass.draw_indexed(
            first_index + b.first..first_index + b.first + b.count,
            base_vertex,
            b.instances.clone(),
        );
    }
}

/// Main pass pipeline kinds (`pipe_code`): opaque, alpha-tested, blended, blended
/// without depth write, depth-only surface coverage, and painted terrain without depth write.
pub(crate) const PIPE_OPAQUE: u8 = 0;

pub(crate) const PIPE_ALPHA_TEST: u8 = 1;

pub(crate) const PIPE_BLEND: u8 = 2;

pub(crate) const PIPE_BLEND_NO_WRITE: u8 = 3;

pub(crate) const PIPE_SURFACE_DEPTH: u8 = 4;

pub(crate) const PIPE_TERRAIN_PAINT: u8 = 5;

pub(crate) const PIPE_KINDS: u8 = 6;

pub(crate) fn effective_render_phase(instance: &Instance) -> RenderPhase {
    if instance.presurface {
        RenderPhase::PreSurface
    } else {
        instance.render_phase
    }
}

pub(crate) fn world_surface_phase(phase: RenderPhase) -> bool {
    matches!(
        phase,
        RenderPhase::PreSurface
            | RenderPhase::Surface
            | RenderPhase::Spline
            | RenderPhase::OnSurface
    )
}

/// Only diffuse-alpha ground coverage is committed after the surface phases. Painted
/// terrain masks, ordinary glass, and explicit no-Z-check materials keep their semantics.
pub(crate) fn surface_depth_coverage(
    phase: RenderPhase,
    alpha: AlphaMode,
    transmap: bool,
    no_z_check: bool,
) -> bool {
    world_surface_phase(phase) && alpha == AlphaMode::Blend && !transmap && !no_z_check
}

/// Which depth-prepass variant a material contributes to. A blended material normally has
/// no prepass because its fragments are see-through; a transmap is the useful exception:
/// fully opaque texels are usually the vehicle body while lower-alpha texels are its windows.
/// Presurfaces also contribute fully transparent blended texels, which seal the ground.
/// Materials explicitly marked no-Z-write/no-Z-check remain excluded, just as they are from
/// the ordinary prepass.
pub(crate) fn depth_prepass_kind(kind: u8, material: &Material, presurface: bool) -> Option<u8> {
    if material.no_z_check {
        return None;
    }
    if kind < PIPE_BLEND {
        Some(kind)
    } else if presurface && !material.no_z_write {
        // Alpha blending preserves colour, but even fully transparent cover texels
        // occlude later ground. AO must see the same cover as the colour pass.
        Some(PIPE_OPAQUE)
    } else if kind == PIPE_BLEND && material.transmap.is_some() && !material.no_z_write {
        Some(2)
    } else {
        None
    }
}

pub(crate) fn instance_depth_bias(instance: &Instance, material: &Material) -> bool {
    // Surfaces already receive a planar view-space pull in vs_main. Adding a
    // slope-scaled raster bias lets roads cover raised floors at grazing angles.
    // Keep raster bias for explicit material overrides only.
    material.z_bias > 0 || (material.no_z_check && !instance.surface)
}

pub(crate) fn surface_instance_code(
    blob: bool,
    ground_layer: bool,
    decal: bool,
    surface: bool,
    surface_bias: bool,
) -> f32 {
    if blob {
        2.0
    } else if ground_layer {
        0.75
    } else if decal {
        if surface_bias {
            1.25
        } else {
            0.9
        }
    } else if surface {
        if surface_bias {
            1.0
        } else {
            0.9
        }
    } else {
        0.0
    }
}

/// OMSI's spline blend sort is horizontal in the x/z ground plane; Rust's vertical axis is z.
pub(crate) fn horizontal_sort_distance(
    origin: DVec3,
    render_origin: DVec3,
    camera_relative: Vec3,
) -> f32 {
    let p = (origin - render_origin).as_vec3() - camera_relative;
    glam::Vec2::new(p.x, p.y).length()
}

/// A main-pass draw's pipeline: the kind, whether back faces are culled, and whether the
/// raster depth bias applies (`[matl_Zbias]` and no-depth-check decals). The opaque
/// and alpha-tested draws are batched in this order (kinds 0 and 1 first).
pub(crate) fn pipe_code(kind: u8, cull: bool, surface: bool) -> u8 {
    debug_assert!(kind < PIPE_KINDS);
    kind * 4 + (cull as u8) * 2 + surface as u8
}

/// The pipeline of a main-pass batch (`DrawItem::pipe`).
pub(crate) fn main_pipeline(pp: &PassPipelines, pipe: u8) -> &wgpu::RenderPipeline {
    &pp.pipelines[pipe as usize]
}

/// Rasterizer state of a scene pipeline. The content meshes keep Direct3D's winding: the
/// visible side of a triangle is the one it shows clockwise on the screen (the side its
/// normals face). Turning y and z round for the right-handed world (`mesh_from_o3d`)
/// mirrors the mesh, but the right-handed camera mirrors the picture back, so the visible
/// side still arrives clockwise here (`o3d_front_faces_arrive_clockwise`). D3D culls the
/// other side by default, and models rely on it: the SD202's front flap click spot is an
/// inside-out shell over the right headlight, invisible from outside, and windows are
/// modelled twice, once per side, with different glass.
pub(crate) fn one_sided_primitive(cull: bool) -> wgpu::PrimitiveState {
    wgpu::PrimitiveState {
        topology: wgpu::PrimitiveTopology::TriangleList,
        cull_mode: cull.then_some(wgpu::Face::Back),
        front_face: wgpu::FrontFace::Cw,
        ..Default::default()
    }
}

/// Are the back faces of this instance culled? Content meshes only, and not when the
/// instance is mirrored (a negative determinant turns the winding round).
/// `OMSI_NO_CULL=1` draws every mesh from both sides, for comparison.
pub(crate) fn culls_back_faces(scene: &Scene, inst: &Instance) -> bool {
    static NO_CULL: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    scene.meshes[inst.mesh].one_sided
        && !*NO_CULL.get_or_init(|| omsi_cfg::env::var_os("OMSI_NO_CULL").is_some())
        && glam::Mat3::from_mat4(inst.transform).determinant() > 0.0
}

pub(crate) fn in_scope<'s, R>(
    pool: Option<&rayon::ThreadPool>,
    op: impl FnOnce(&rayon::Scope<'s>) -> R,
) -> R {
    match pool {
        Some(p) => p.in_place_scope(op),
        None => rayon::in_place_scope(op),
    }
}

pub(crate) fn run_parts<T: Send>(
    pool: Option<&rayon::ThreadPool>,
    parts: usize,
    f: impl Fn(usize) -> T + Sync,
) -> Vec<T> {
    let Some(pool) = pool else {
        return std::thread::scope(|s| {
            let f = &f;
            let helpers: Vec<_> = (1..parts.max(1)).map(|k| s.spawn(move || f(k))).collect();
            let mut out = vec![f(0)];
            out.extend(helpers.into_iter().map(|h| h.join().expect("render part")));
            out
        });
    };
    let mut out: Vec<Option<T>> = (0..parts.max(1)).map(|_| None).collect();
    pool.in_place_scope(|s| {
        let f = &f;
        let (first, rest) = out.split_first_mut().expect("one part at least");
        for (k, slot) in rest.iter_mut().enumerate() {
            s.spawn(move |_| *slot = Some(f(k + 1)));
        }
        *first = Some(f(0));
    });
    out.into_iter().map(|o| o.expect("render part")).collect()
}

/// The main pass's batches as render bundles, recorded on up to four threads. Checking
/// every draw is the costliest CPU step of a frame; a bundle is checked when it is made,
/// and the pass that runs it only replays it. The bundles keep the batches' order (the
/// blended ones are far to near).
pub(crate) fn record_bundles(
    device: &wgpu::Device,
    pool: Option<&rayon::ThreadPool>,
    scene: &Scene,
    batches: &[Batch],
    pp: &PassPipelines,
    camera: &wgpu::BindGroup,
    format: wgpu::TextureFormat,
    samples: u32,
) -> Vec<wgpu::RenderBundle> {
    let color_formats = [
        Some(format),
        Some(MASK_FORMAT),
        Some(GBUF_FORMAT),
        Some(AUX_FORMAT),
    ];
    let record = |chunk: &[Batch]| -> wgpu::RenderBundle {
        let mut bundle =
            device.create_render_bundle_encoder(&wgpu::RenderBundleEncoderDescriptor {
                label: Some("main pass part"),
                color_formats: &color_formats[..if format != HDR_FORMAT {
                    1
                } else if rt_gbuf() {
                    4
                } else {
                    2
                }],
                depth_stencil: Some(wgpu::RenderBundleDepthStencil {
                    format: DEPTH_FORMAT,
                    depth_read_only: false,
                    stencil_read_only: true,
                }),
                sample_count: samples,
                multiview: None,
            });
        bundle.set_bind_group(0, camera, &[]);
        encode_batches(&mut bundle, scene, chunk, |pipe| main_pipeline(pp, pipe));
        bundle.finish(&wgpu::RenderBundleDescriptor {
            label: Some("main pass part"),
        })
    };
    // a bundle wgpu refuses (a buffer it names could not be made: the card ran out of
    // memory) panics in `finish`: that part of the picture is left out for the frame and
    // the game goes on - it ended the game on Windows right after an "Out of memory"
    let record = |chunk: &[Batch]| -> Option<wgpu::RenderBundle> {
        CATCHING.with(|c| c.set(true));
        let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| record(chunk)));
        CATCHING.with(|c| c.set(false));
        match r {
            Ok(b) => Some(b),
            Err(_) => {
                static SAID: std::sync::atomic::AtomicBool =
                    std::sync::atomic::AtomicBool::new(false);
                if !SAID.swap(true, std::sync::atomic::Ordering::Relaxed) {
                    log::error!("a part of the picture could not be recorded (the graphics card is out of memory?); left out");
                }
                None
            }
        }
    };
    let max_parts = pool
        .map(|p| (p.current_num_threads() + 1).clamp(4, 8))
        .unwrap_or(4);
    let parts = (batches.len() / 250).clamp(1, max_parts);
    if parts == 1 {
        return record(batches).into_iter().collect();
    }
    let chunks: Vec<&[Batch]> = batches.chunks(batches.len().div_ceil(parts)).collect();
    run_parts(pool, chunks.len(), |k| record(chunks[k]))
        .into_iter()
        .flatten()
        .collect()
}

/// A fragment shader's filterable 2D texture at `binding`.
pub(crate) fn float_tex(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}
