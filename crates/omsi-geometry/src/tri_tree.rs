//! A bounding-volume tree over the triangles of one mesh, in the mesh's own frame, so a ray
//! (the cursor in a cockpit) looks at a few dozen triangles instead of all of them.
//!
//! The tree only decides which triangles are looked at: those it keeps go through the same
//! tests as the full scans ([`ray_mesh_inv`](crate::ray_mesh_inv),
//! [`cone_triangles_inv`](crate::cone_triangles_inv)), and its boxes and spheres are grown
//! by a margin so that no triangle the full scan would find is left out. The answers are the
//! full scans' answers, bit for bit.

use crate::{cone_keeps, ray_triangle, MeshData};
use glam::{Mat4, Vec3};

/// Triangles in a leaf.
const LEAF: usize = 8;

/// A node: its box and the sphere round its triangles' cone spheres (both grown by the
/// margin), and either its triangles (`len > 0`: `first..first + len` of `TriTree::tris`) or
/// its second child (the first follows it).
#[derive(Debug, Clone, Copy)]
struct Node {
    lo: Vec3,
    hi: Vec3,
    centre: Vec3,
    radius: f32,
    first: u32,
    len: u32,
}

/// See the module documentation. Built by [`TriTree::build`] from the mesh it then answers
/// for; the mesh must not change afterwards.
#[derive(Debug, Clone, Default)]
pub struct TriTree {
    nodes: Vec<Node>,
    /// The first index (in `MeshData::indices`) of each triangle, in leaf order.
    tris: Vec<u32>,
    /// Triangles with a coordinate that is no number: no ray meets them, the cone test keeps
    /// them all, so they stay out of the nodes.
    odd: Vec<u32>,
}

/// One triangle while building: its first index, box, and the centre and radius the cone
/// test gives it.
struct Item {
    k: u32,
    lo: Vec3,
    hi: Vec3,
    c: Vec3,
    r: f32,
}

impl TriTree {
    pub fn build(mesh: &MeshData) -> TriTree {
        let mut items: Vec<Item> = mesh
            .indices
            .chunks_exact(3)
            .enumerate()
            .map(|(k, tri)| {
                let (p0, p1, p2) = (
                    mesh.positions[tri[0] as usize],
                    mesh.positions[tri[1] as usize],
                    mesh.positions[tri[2] as usize],
                );
                // as the cone test works them out
                let c = (p0 + p1 + p2) / 3.0;
                let r = (p0 - c)
                    .length()
                    .max((p1 - c).length())
                    .max((p2 - c).length());
                Item {
                    k: (k * 3) as u32,
                    lo: p0.min(p1).min(p2),
                    hi: p0.max(p1).max(p2),
                    c,
                    r,
                }
            })
            .collect();
        let odd: Vec<u32> = items
            .iter()
            .filter(|it| !(it.c.is_finite() && it.r.is_finite()))
            .map(|it| it.k)
            .collect();
        items.retain(|it| it.c.is_finite() && it.r.is_finite());
        if items.is_empty() {
            return TriTree {
                odd,
                ..TriTree::default()
            };
        }
        // a millimetre, and more for a mesh far from its origin (the rounding of a point
        // there): far more than the rounding of any hit point or box edge
        let (lo, hi) = items.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), it| (lo.min(it.lo), hi.max(it.hi)),
        );
        let margin = 1e-3 + 1e-5 * lo.abs().max(hi.abs()).max_element();
        let mut tree = TriTree {
            nodes: Vec::with_capacity(2 * items.len() / LEAF + 1),
            tris: Vec::with_capacity(items.len()),
            odd,
        };
        tree.split(&mut items, margin);
        tree
    }

    /// Add the node over `items` and, under it, its children.
    fn split(&mut self, items: &mut [Item], margin: f32) {
        let (lo, hi) = items.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), it| (lo.min(it.lo), hi.max(it.hi)),
        );
        let centre = (lo + hi) * 0.5;
        let radius = items
            .iter()
            .map(|it| (it.c - centre).length() + it.r)
            .fold(0.0f32, f32::max);
        let at = self.nodes.len();
        self.nodes.push(Node {
            lo: lo - Vec3::splat(margin),
            hi: hi + Vec3::splat(margin),
            centre,
            radius: radius * (1.0 + 1e-5) + margin,
            first: 0,
            len: 0,
        });
        if items.len() <= LEAF {
            self.nodes[at].first = self.tris.len() as u32;
            self.nodes[at].len = items.len() as u32;
            self.tris.extend(items.iter().map(|it| it.k));
            return;
        }
        // halves along the longest side of the triangles' centres
        let (clo, chi) = items.iter().fold(
            (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
            |(lo, hi), it| (lo.min(it.c), hi.max(it.c)),
        );
        let size = chi - clo;
        let axis = if size.x >= size.y && size.x >= size.z {
            0
        } else if size.y >= size.z {
            1
        } else {
            2
        };
        let mid = items.len() / 2;
        items.select_nth_unstable_by(mid, |a, b| a.c[axis].total_cmp(&b.c[axis]));
        let (left, right) = items.split_at_mut(mid);
        self.split(left, margin);
        self.nodes[at].first = self.nodes.len() as u32;
        self.split(right, margin);
    }

    /// The triangles the tree holds (the mesh's, when it was built from it).
    pub fn triangles(&self) -> usize {
        self.tris.len() + self.odd.len()
    }

    /// [`ray_mesh_inv`](crate::ray_mesh_inv) through the tree: the same `t`, bit for bit.
    /// `tested` counts the triangles looked at.
    pub fn ray_mesh_inv(
        &self,
        origin: Vec3,
        dir: Vec3,
        mesh: &MeshData,
        inv: &Mat4,
        tested: &mut u64,
    ) -> Option<f32> {
        let o = inv.transform_point3(origin);
        let d = inv.transform_vector3(dir);
        let scale = d.length();
        if scale < 1e-9 || self.nodes.is_empty() {
            return None;
        }
        let dn = d / scale;
        // the nearest hit so far, along `dn` and as the full scan gives it
        let mut near = f32::INFINITY;
        let mut best: Option<f32> = None;
        // (halving the triangles at each level, the tree is never 64 levels deep)
        let mut stack = [(0u32, 0.0f32); 128];
        let mut top = 0;
        if let Some(t) = self.enters(0, o, dn) {
            stack[0] = (0, t);
            top = 1;
        }
        while top > 0 {
            top -= 1;
            let (n, enter) = stack[top];
            if enter > near {
                continue;
            }
            let node = self.nodes[n as usize];
            if node.len > 0 {
                let ks = &self.tris[node.first as usize..(node.first + node.len) as usize];
                *tested += ks.len() as u64;
                for &k in ks {
                    let k = k as usize;
                    let (a, b, c) = (
                        mesh.positions[mesh.indices[k] as usize],
                        mesh.positions[mesh.indices[k + 1] as usize],
                        mesh.positions[mesh.indices[k + 2] as usize],
                    );
                    if let Some(t) = ray_triangle(o, dn, a, b, c) {
                        near = near.min(t);
                        let t = t / scale;
                        if best.map(|bt| t < bt).unwrap_or(true) {
                            best = Some(t);
                        }
                    }
                }
                continue;
            }
            // the nearer child is looked at first, so the farther one is often passed by
            let (l, r) = (n + 1, node.first);
            let (tl, tr) = (self.enters(l, o, dn), self.enters(r, o, dn));
            let (first, second) = match (tl, tr) {
                (Some(a), Some(b)) if a > b => (Some((l, a)), Some((r, b))),
                _ => (tr.map(|b| (r, b)), tl.map(|a| (l, a))),
            };
            for e in [first, second].into_iter().flatten() {
                stack[top] = e;
                top += 1;
            }
        }
        best
    }

    /// Where the ray enters node `n`'s box (0 when it starts inside), None when it misses it.
    fn enters(&self, n: u32, o: Vec3, d: Vec3) -> Option<f32> {
        let node = &self.nodes[n as usize];
        let (mut near, mut far) = (0.0f32, f32::INFINITY);
        for axis in 0..3 {
            let (o, d, lo, hi) = (o[axis], d[axis], node.lo[axis], node.hi[axis]);
            if d == 0.0 {
                if o < lo || o > hi {
                    return None;
                }
                continue;
            }
            let (a, b) = ((lo - o) / d, (hi - o) / d);
            let (a, b) = if a <= b { (a, b) } else { (b, a) };
            near = near.max(a);
            far = far.min(b);
            if near > far {
                return None;
            }
        }
        Some(near)
    }

    /// [`cone_triangles_inv`](crate::cone_triangles_inv) through the tree: the same
    /// triangles in the same order. `tested` counts the triangles looked at.
    pub fn cone_triangles_inv(
        &self,
        origin: Vec3,
        axis: Vec3,
        tan_half: f32,
        mesh: &MeshData,
        inv: &Mat4,
        tested: &mut u64,
    ) -> Vec<u32> {
        let o = inv.transform_point3(origin);
        let a = inv.transform_vector3(axis).normalize_or_zero();
        let mut out = self.odd.clone();
        let mut stack: Vec<u32> = if self.nodes.is_empty() {
            vec![]
        } else {
            vec![0]
        };
        while let Some(n) = stack.pop() {
            let node = self.nodes[n as usize];
            // the cone test of a sphere holding every triangle's sphere: it lets through
            // every node with a triangle the test lets through
            let (c, r) = (node.centre, node.radius);
            let w = c - o;
            let along = w.dot(a);
            if along < -r {
                continue;
            }
            let across = (w - a * along).length();
            if across > (along + r).max(0.0) * tan_half + r + 1e-3 {
                continue;
            }
            if node.len == 0 {
                stack.extend([node.first, n + 1]);
                continue;
            }
            let ks = &self.tris[node.first as usize..(node.first + node.len) as usize];
            *tested += ks.len() as u64;
            for &k in ks {
                let i = k as usize;
                let (p0, p1, p2) = (
                    mesh.positions[mesh.indices[i] as usize],
                    mesh.positions[mesh.indices[i + 1] as usize],
                    mesh.positions[mesh.indices[i + 2] as usize],
                );
                if cone_keeps(o, a, tan_half, p0, p1, p2) {
                    out.push(k);
                }
            }
        }
        out.sort_unstable();
        out
    }
}
