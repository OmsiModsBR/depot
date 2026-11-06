#![allow(dead_code)]
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Mutex;
use omsi_render::{FramePlan, MirrorEvent, MirrorTexture, Scene};
// ---- TEMP debug dump (OMSI_D3D9_DUMP=<dir>) ----
type DbgTex = (u32, u32, u8, u64, Option<Vec<u8>>);
static DBG_TEX: Mutex<Vec<Option<DbgTex>>> = Mutex::new(Vec::new());
type DbgMesh = (usize, u64, u64, Vec<[f32; 8]>);
static DBG_MESH: Mutex<Vec<Option<DbgMesh>>> = Mutex::new(Vec::new());
static DBG_FRAMES: AtomicUsize = AtomicUsize::new(0);

fn dbg_on() -> Option<std::path::PathBuf> {
    std::env::var_os("OMSI_D3D9_DUMP").map(std::path::PathBuf::from)
}

fn dbg_hash(b: &[u8]) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    b.hash(&mut h);
    h.finish()
}

pub(crate) fn dbg_asset(event: &MirrorEvent<'_>) {
    if dbg_on().is_none() {
        return;
    }
    {
        let mut ms = DBG_MESH.lock().unwrap();
        let mput = |t: &mut Vec<Option<DbgMesh>>, id: usize, v: Option<DbgMesh>| {
            if t.len() <= id {
                t.resize(id + 1, None);
            }
            t[id] = v;
        };
        let sample = |v: &[f32]| -> Vec<[f32; 8]> {
            let n = v.len() / 8;
            [0usize, 1, 60, 61, n / 2, n.saturating_sub(1)]
                .iter()
                .filter(|&&i| i < n)
                .map(|&i| {
                    let mut a = [0f32; 8];
                    a.copy_from_slice(&v[i * 8..i * 8 + 8]);
                    a
                })
                .collect()
        };
        match event {
            MirrorEvent::Mesh { id, vertices, indices } => {
                let f: &[f32] = bytemuck::cast_slice(vertices);
                let v = (
                    f.len() / 8,
                    dbg_hash(bytemuck::cast_slice(f)),
                    dbg_hash(bytemuck::cast_slice(indices)),
                    sample(f),
                );
                mput(&mut ms, *id, Some(v));
            }
            MirrorEvent::MeshVertices { id, vertices } => {
                let f: &[f32] = bytemuck::cast_slice(vertices);
                if let Some(Some(m)) = ms.get_mut(*id) {
                    m.1 = dbg_hash(bytemuck::cast_slice(f));
                    m.3 = sample(f);
                }
                log::warn!("DUMP mesh-vertices event for {id}");
            }
            MirrorEvent::FreeMesh(id) => mput(&mut ms, *id, None),
            MirrorEvent::MoveMesh { from, into } => {
                let v = ms.get_mut(*from).and_then(Option::take);
                mput(&mut ms, *into, v);
            }
            _ => {}
        }
    }
    let mut t = DBG_TEX.lock().unwrap();
    let put = |t: &mut Vec<Option<DbgTex>>, id: usize, v: Option<DbgTex>| {
        if t.len() <= id {
            t.resize(id + 1, None);
        }
        t[id] = v;
    };
    match event {
        MirrorEvent::Texture { id, texture } => {
            let v = match texture {
                MirrorTexture::Image { image, .. } => (
                    image.width,
                    image.height,
                    0u8,
                    dbg_hash(&image.rgba),
                    (image.width <= 1024).then(|| image.rgba.clone()),
                ),
                MirrorTexture::Data(d) => {
                    let f = match d.format {
                        omsi_texture::PixelFormat::Rgba8 => 0,
                        omsi_texture::PixelFormat::Bc1 => 1,
                        omsi_texture::PixelFormat::Bc2 => 2,
                        omsi_texture::PixelFormat::Bc3 => 3,
                    };
                    let l0 = d.levels.first().cloned().unwrap_or_default();
                    (d.width, d.height, f, dbg_hash(&l0), (d.width <= 1024).then_some(l0))
                }
            };
            put(&mut t, *id, Some(v));
        }
        MirrorEvent::FreeTexture(id) => put(&mut t, *id, None),
        MirrorEvent::MoveTexture { from, into } => {
            let v = t.get_mut(*from).and_then(Option::take);
            put(&mut t, *into, v);
        }
        _ => {}
    }
}

fn dbg_png(dir: &std::path::Path, name: &str, v: &DbgTex) {
    let (w, h, f, _, Some(l0)) = v else { return };
    let rgba = match f {
        0 => l0.clone(),
        1 => omsi_texture::bc::decode(l0, *w, *h, omsi_texture::bc::Bc::Bc1 { punch: true }),
        3 => omsi_texture::bc::decode(l0, *w, *h, omsi_texture::bc::Bc::Bc3),
        _ => return,
    };
    let _ = image::save_buffer(dir.join(name), &rgba, *w, *h, image::ColorType::Rgba8);
    let a: Vec<u8> = rgba.chunks_exact(4).map(|p| p[3]).collect();
    let _ = image::save_buffer(dir.join(format!("a_{name}")), &a, *w, *h, image::ColorType::L8);
}

pub(crate) fn dbg_frame(plan: &FramePlan, scene: &Scene) {
    let Some(dir) = dbg_on() else { return };
    let n = DBG_FRAMES.fetch_add(1, Ordering::Relaxed);
    let at: usize = std::env::var("OMSI_D3D9_DUMP_FRAME").ok().and_then(|v| v.parse().ok()).unwrap_or(600);
    if n != at {
        return;
    }
    let _ = std::fs::create_dir_all(&dir);
    let t = DBG_TEX.lock().unwrap();
    let models = scene.slot_models();
    let params = scene.slot_params();
    log::warn!("DUMP frame {n}: {} main batches, list {}", plan.main.len(), plan.list.len());
    for (bi, b) in plan.main.iter().enumerate() {
        let Some(m) = scene.materials.get(b.material) else { continue };
        let u = m.uniform_vectors();
        if u[2][0] < 0.5 {
            continue;
        }
        let ents: Vec<String> = (b.instances.clone())
            .filter_map(|i| plan.list.get(i as usize))
            .map(|&e| {
                let e = e as usize;
                let mo = models.get(e).copied().unwrap_or_default();
                let p = params.get(e * 2..e * 2 + 2).map(|p| format!("{:?}", p)).unwrap_or_default();
                format!("e{e} t{:?} p{p}", mo)
            })
            .collect();
        let tex = |id: Option<usize>| id.map(|i| (i, t.get(i).cloned().flatten().map(|v| (v.0, v.1, v.2, v.3))));
        log::warn!(
            "DUMP batch {bi} {:?} surf {} mesh {} first {} count {} mat {} tex {:?} trans {:?} light {:?} u1 {:?} u2 {:?} ents {:?}",
            b.kind, b.surface, b.mesh, b.first, b.count, b.material,
            tex(m.texture), tex(m.transmap.map(|x| x.0)), tex(m.lightmap), u[1], u[2], ents
        );
        let ms = DBG_MESH.lock().unwrap();
        log::warn!("DUMP batch {bi} mesh {} = {:?}", b.mesh, ms.get(b.mesh));
        drop(ms);
        for (k, id) in [("tex", m.texture), ("trans", m.transmap.map(|x| x.0))] {
            if let Some(id) = id {
                if let Some(Some(v)) = t.get(id) {
                    dbg_png(&dir, &format!("b{bi}_m{}_{k}_{id}.png", b.material), v);
                }
            }
        }
    }
}
