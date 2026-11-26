//! Meshes: the GPU mesh, the shared mesh pages its vertices and indices live in, preparing
//! meshes off the render thread, and adding and updating them.

use super::Vertex;
use crate::gpu::gl_worker_turn;
use glam::Vec3;
use omsi_geometry::MeshData;

/// Vertex and index buffers holding several meshes; the space a freed mesh leaves is
/// handed to the next that fits.
pub struct MeshPage {
    pub vertex: wgpu::Buffer,
    pub index: wgpu::Buffer,
    pub vertex_top: u64,
    pub index_top: u64,
    pub vertex_free: Vec<(u64, u64)>,
    pub index_free: Vec<(u64, u64)>,
}

pub const MESH_PAGE_VERTEX_BYTES: u64 = 32 << 20;

pub const MESH_PAGE_INDEX_BYTES: u64 = 16 << 20;

/// First fit of `len` bytes in `free` (offset, length) or at `top` under `cap`.
pub fn page_take(free: &mut Vec<(u64, u64)>, top: &mut u64, cap: u64, len: u64) -> Option<u64> {
    if len == 0 {
        return Some(0);
    }
    if let Some(k) = free.iter().position(|&(_, l)| l >= len) {
        let (o, l) = free[k];
        if l == len {
            free.remove(k);
        } else {
            free[k] = (o + len, l - len);
        }
        return Some(o);
    }
    (*top + len <= cap).then(|| {
        *top += len;
        *top - len
    })
}

/// Give `len` bytes at `at` back, joined to the free space beside them.
pub fn page_give(free: &mut Vec<(u64, u64)>, top: &mut u64, at: u64, len: u64) {
    if len == 0 {
        return;
    }
    let k = free.partition_point(|&(o, _)| o < at);
    free.insert(k, (at, len));
    if k + 1 < free.len() && free[k].0 + free[k].1 == free[k + 1].0 {
        free[k].1 += free[k + 1].1;
        free.remove(k + 1);
    }
    if k > 0 && free[k - 1].0 + free[k - 1].1 == free[k].0 {
        free[k - 1].1 += free[k].1;
        free.remove(k);
    }
    if let Some(&(o, l)) = free.last() {
        if o + l == *top {
            *top = o;
            free.pop();
        }
    }
}

impl MeshPage {
    pub fn new(device: &wgpu::Device, vertex_bytes: u64, index_bytes: u64) -> MeshPage {
        let buffer = |size: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("mesh page"),
                size: size.max(32),
                usage: usage | wgpu::BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        MeshPage {
            vertex: buffer(vertex_bytes, wgpu::BufferUsages::VERTEX),
            index: buffer(index_bytes, wgpu::BufferUsages::INDEX),
            vertex_top: 0,
            index_top: 0,
            vertex_free: Vec::new(),
            index_free: Vec::new(),
        }
    }

    /// Room for a mesh of these sizes: its vertex and index offsets.
    pub fn take(&mut self, vertex_bytes: u64, index_bytes: u64) -> Option<(u64, u64)> {
        let v = page_take(
            &mut self.vertex_free,
            &mut self.vertex_top,
            self.vertex.size(),
            vertex_bytes,
        )?;
        match page_take(
            &mut self.index_free,
            &mut self.index_top,
            self.index.size(),
            index_bytes,
        ) {
            Some(i) => Some((v, i)),
            None => {
                page_give(&mut self.vertex_free, &mut self.vertex_top, v, vertex_bytes);
                None
            }
        }
    }

    pub fn give(&mut self, m: &GpuMesh) {
        page_give(
            &mut self.vertex_free,
            &mut self.vertex_top,
            m.vertex_offset,
            m.vertex_bytes,
        );
        page_give(
            &mut self.index_free,
            &mut self.index_top,
            m.first_index as u64 * 4,
            m.index_bytes,
        );
    }

    /// Write the mesh at these offsets.
    pub fn place(
        &self,
        queue: &wgpu::Queue,
        data: &MeshData,
        page: u32,
        at: (u64, u64),
    ) -> GpuMesh {
        let verts = mesh_vertices(data);
        let (vb, ib): (&[u8], &[u8]) = (
            bytemuck::cast_slice(&verts),
            bytemuck::cast_slice(&data.indices),
        );
        if !vb.is_empty() {
            queue.write_buffer(&self.vertex, at.0, vb);
        }
        if !ib.is_empty() {
            queue.write_buffer(&self.index, at.1, ib);
        }
        let (center, radius) = mesh_bounds(data);
        GpuMesh {
            vertex_buf: self.vertex.clone(),
            index_buf: self.index.clone(),
            page,
            base_vertex: (at.0 / std::mem::size_of::<Vertex>() as u64) as i32,
            first_index: (at.1 / 4) as u32,
            vertex_offset: at.0,
            vertex_bytes: vb.len() as u64,
            index_bytes: ib.len() as u64,
            ranges: data.ranges.clone(),
            bounds_center: center,
            bounds_radius: radius,
            one_sided: data.one_sided,
            source: None,
        }
    }
}

pub fn mesh_vertices(data: &MeshData) -> Vec<Vertex> {
    data.positions
        .iter()
        .zip(&data.normals)
        .zip(&data.uvs)
        .map(|((p, n), uv)| Vertex {
            pos: p.to_array(),
            normal: n.to_array(),
            uv: uv.to_array(),
        })
        .collect()
}

pub fn mesh_page_bytes(data: &MeshData) -> (u64, u64) {
    let v = data
        .positions
        .len()
        .min(data.normals.len())
        .min(data.uvs.len());
    (
        (v * std::mem::size_of::<Vertex>()) as u64,
        (data.indices.len() * 4) as u64,
    )
}

/// The sphere round what the mesh draws: the vertices its triangles use. A vertex no triangle
/// uses is never drawn, and o3d files carry such leftovers - an articulated bus's lamp mesh
/// has four at up to 6e25 m. Taken in, they made the sphere's radius infinite: the camera was
/// always inside it, so the bus was ranked nearest of all and drawn over every pane in front
/// of it (its own rear section's glass included), and the culling block holding the mesh got
/// a centre of NaN, which the shadow pass never finds in its light box - every mesh beside it
/// in the block (the bus's body among them) cast no shadow.
pub fn mesh_bounds(data: &MeshData) -> (Vec3, f32) {
    let (mut lo, mut hi) = (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN));
    let mut any = false;
    for p in data
        .indices
        .iter()
        .filter_map(|&i| data.positions.get(i as usize))
    {
        lo = lo.min(*p);
        hi = hi.max(*p);
        any = true;
    }
    if !any {
        lo = Vec3::ZERO;
        hi = Vec3::ZERO;
    }
    let center = (lo + hi) * 0.5;
    (center, (hi - center).length())
}

/// The meshes in one page of their own, sized to them (each in a page of its own where the
/// adapter cannot draw with a base vertex: `paged` false).
pub fn make_meshes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[&MeshData],
    paged: bool,
) -> Vec<GpuMesh> {
    if data.is_empty() {
        return Vec::new();
    }
    if !paged {
        return data
            .iter()
            .map(|d| {
                make_meshes(device, queue, &[d], true)
                    .pop()
                    .expect("one mesh")
            })
            .collect();
    }
    let sizes: Vec<(u64, u64)> = data.iter().map(|d| mesh_page_bytes(d)).collect();
    let page = MeshPage::new(
        device,
        sizes.iter().map(|s| s.0).sum(),
        sizes.iter().map(|s| s.1).sum(),
    );
    let mut at = (0, 0);
    data.iter()
        .zip(&sizes)
        .map(|(d, s)| {
            let m = page.place(queue, d, u32::MAX, at);
            at = (at.0 + s.0, at.1 + s.1);
            m
        })
        .collect()
}

/// A mesh on the GPU, made on a worker thread; [`Renderer::add_prepared_mesh`] puts it into
/// a scene.
pub struct PreparedMesh(
    pub GpuMesh,
    /// The vertices and indices, kept while a renderer mirrors its assets.
    pub Option<Box<(Vec<Vertex>, Vec<u32>)>>,
);

/// Make a mesh's GPU buffers on any thread (the device takes calls from all of them).
pub fn prepare_mesh(device: &wgpu::Device, queue: &wgpu::Queue, data: &MeshData) -> PreparedMesh {
    prepare_meshes(device, queue, &[data], false)
        .pop()
        .expect("one mesh")
}

/// [`prepare_mesh`] for several meshes at once, which share one page of buffers.
/// `paged`: [`Renderer::mesh_pages`] of the renderer the meshes are for.
pub fn prepare_meshes(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    data: &[&MeshData],
    paged: bool,
) -> Vec<PreparedMesh> {
    let _turn = gl_worker_turn();
    let copies = super::keep_cpu_copies();
    make_meshes(device, queue, data, paged)
        .into_iter()
        .zip(data)
        .map(|(m, d)| {
            PreparedMesh(
                m,
                copies.then(|| Box::new((mesh_vertices(d), d.indices.clone()))),
            )
        })
        .collect()
}

pub struct GpuMesh {
    pub vertex_buf: wgpu::Buffer,
    pub index_buf: wgpu::Buffer,
    /// Where the mesh lies in its page's buffers (pages hold many meshes); `page` is its
    /// page in the scene's list, `u32::MAX` for a page of its own.
    pub page: u32,
    pub base_vertex: i32,
    pub first_index: u32,
    pub vertex_offset: u64,
    pub vertex_bytes: u64,
    pub index_bytes: u64,
    pub ranges: Vec<(u32, u32, u32)>,
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
    /// Back faces are culled (a content mesh, see `MeshData::one_sided`).
    pub one_sided: bool,
    /// Source asset for the optional draw-cost audit.
    pub source: Option<String>,
}
