//! Where instances are: their bounds and scale, the culling blocks, and the render origin
//! the scene is drawn relative to.

use super::model::{Instance, MeshId, Scene};
use crate::assets::GpuMesh;
use crate::renderer::Renderer;
use glam::{DVec3, Mat4, Vec3};

#[derive(Clone, Copy, Default)]
pub(crate) struct InstanceBounds {
    pub(crate) centre: Vec3,
    pub(crate) radius: f32,
    pub(crate) scale: f32,
}

impl InstanceBounds {
    pub(crate) fn new(mesh: &GpuMesh, transform: Mat4) -> Self {
        let scale = transform_scale(transform);
        Self {
            centre: transform.transform_point3(mesh.bounds_center),
            radius: mesh.bounds_radius * scale,
            scale,
        }
    }
}

pub(crate) const CULL_BLOCK: usize = 128;

pub(crate) const CULL_BLOCK_REBUILDS: usize = 8;

pub(crate) fn transform_scale(transform: Mat4) -> f32 {
    transform
        .x_axis
        .truncate()
        .length_squared()
        .max(transform.y_axis.truncate().length_squared())
        .max(transform.z_axis.truncate().length_squared())
        .sqrt()
}

/// Is `p` inside a vehicle's `[boundingbox]` (origin, heading in degrees, w l h cx cy cz)?
/// The same box the shader keeps the weather out of, without its margins.
pub(crate) fn point_in_vehicle_box(
    p: DVec3,
    (origin, heading, bb): &(DVec3, f64, [f32; 6]),
) -> bool {
    let d = (p - *origin).as_vec3();
    let (sh, ch) = (*heading as f32).to_radians().sin_cos();
    let x = d.x * ch - d.y * sh - bb[3];
    let y = d.x * sh + d.y * ch - bb[4];
    let z = d.z - bb[5];
    x.abs() < bb[0] * 0.5 && y.abs() < bb[1] * 0.5 && z.abs() < bb[2] * 0.5
}

impl Renderer {
    /// Bounding sphere of an instance relative to the render origin: the mesh's sphere
    /// through the instance transform (the model matrix is that transform moved by the
    /// origin, so no matrix product is needed), the radius scaled by the largest axis.
    pub(crate) fn bounding_sphere(scene: &Scene, i: &Instance) -> (Vec3, f32) {
        let b = if scene.cache_bounds {
            i.bounds
        } else {
            InstanceBounds::new(&scene.meshes[i.mesh], i.transform)
        };
        (
            b.centre + (i.origin - scene.render_origin).as_vec3(),
            b.radius,
        )
    }

    pub(crate) fn instance_scale(scene: &Scene, i: &Instance) -> f32 {
        if scene.cache_bounds {
            i.bounds.scale
        } else {
            transform_scale(i.transform)
        }
    }

    pub(crate) fn mesh_bounds_changed(scene: &mut Scene, mesh: MeshId) {
        scene.bounds_meshes.resize(scene.meshes.len(), false);
        scene.bounds_meshes[mesh] = true;
        scene.bounds_dirty = true;
        scene.bounds_known.resize(scene.meshes.len(), false);
        if !scene.bounds_known[mesh] {
            scene.bounds_known[mesh] = true;
            scene.bounds_users_stale = true;
        }
    }

    /// Slot `mesh` holds another mesh now (freed, or recycled): no longer a reshaped one.
    pub(crate) fn mesh_slot_replaced(scene: &mut Scene, mesh: MeshId) {
        scene.bounds_meshes.resize(scene.meshes.len(), false);
        scene.bounds_meshes[mesh] = true;
        scene.bounds_dirty = true;
        scene.bounds_rescan = true;
        if scene.bounds_known.get(mesh).copied().unwrap_or(false) {
            scene.bounds_known[mesh] = false;
            scene.bounds_users_stale = true;
        }
    }

    pub(crate) fn prepare_bounds(scene: &mut Scene) {
        if !scene.cache_bounds {
            return;
        }
        let blocks = scene.instances.len().div_ceil(CULL_BLOCK);
        let all = scene.dirty || scene.block_bounds.len() != blocks;
        scene
            .block_bounds
            .resize(blocks, (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)));
        scene.block_dirty.resize(blocks, false);
        let grow = |bb: &mut (DVec3, DVec3), i: &Instance| {
            let c = i.origin + i.bounds.centre.as_dvec3();
            let r = i.bounds.radius as f64;
            bb.0 = bb.0.min(c - r);
            bb.1 = bb.1.max(c + r);
        };
        if scene.dirty {
            for i in &mut scene.instances {
                i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
            }
        } else {
            for k in scene.uploaded_instances..scene.instances.len() {
                let i = &mut scene.instances[k];
                i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
                grow(&mut scene.block_bounds[k / CULL_BLOCK], i);
            }
            for &idx in &scene.changed {
                if let Some(i) = scene.instances.get_mut(idx) {
                    i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
                    grow(&mut scene.block_bounds[idx / CULL_BLOCK], i);
                    scene.block_dirty[idx / CULL_BLOCK] = true;
                }
            }
            if scene.bounds_dirty {
                // Skinning can alter a shared mesh without changing any model matrix.
                // Scan once per pose update, rather than once per shadow/mirror view.
                if scene.bounds_users_stale {
                    let known = &scene.bounds_known;
                    scene.bounds_users = (0..scene.instances.len())
                        .filter(|&k| known.get(scene.instances[k].mesh).copied().unwrap_or(false))
                        .collect();
                    scene.bounds_users_stale = false;
                }
                let all: Vec<usize>;
                let scan: &[usize] = if std::mem::take(&mut scene.bounds_rescan) {
                    all = (0..scene.instances.len()).collect();
                    &all
                } else {
                    &scene.bounds_users
                };
                for &k in scan {
                    let Some(i) = scene.instances.get_mut(k) else {
                        continue;
                    };
                    if scene.bounds_meshes.get(i.mesh).copied().unwrap_or(false) {
                        i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
                        grow(&mut scene.block_bounds[k / CULL_BLOCK], i);
                        scene.block_dirty[k / CULL_BLOCK] = true;
                    }
                }
            }
        }
        if scene.bounds_dirty {
            scene.bounds_meshes.fill(false);
            scene.bounds_dirty = false;
        }
        let rebuild = |scene: &mut Scene, b: usize| {
            let mut bb = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
            for i in
                &scene.instances[b * CULL_BLOCK..((b + 1) * CULL_BLOCK).min(scene.instances.len())]
            {
                grow(&mut bb, i);
            }
            scene.block_bounds[b] = bb;
            scene.block_dirty[b] = false;
        };
        if all {
            for b in 0..blocks {
                rebuild(scene, b);
            }
            return;
        }
        let mut left = CULL_BLOCK_REBUILDS;
        for _ in 0..blocks {
            if left == 0 {
                break;
            }
            scene.block_cursor = (scene.block_cursor + 1) % blocks.max(1);
            if scene.block_dirty[scene.block_cursor] {
                rebuild(scene, scene.block_cursor);
                left -= 1;
            }
        }
    }

    pub(crate) fn cull_blocks(scene: &Scene) -> Option<Vec<(Vec3, f32)>> {
        (scene.cache_bounds
            && scene.block_bounds.len() == scene.instances.len().div_ceil(CULL_BLOCK))
        .then(|| {
            scene
                .block_bounds
                .iter()
                .map(|&(lo, hi)| {
                    if lo.x > hi.x {
                        (Vec3::ZERO, 0.0)
                    } else {
                        (
                            ((lo + hi) * 0.5 - scene.render_origin).as_vec3(),
                            ((hi - lo).length() * 0.5) as f32,
                        )
                    }
                })
                .collect()
        })
    }

    /// Choose the render origin. Everything is re-uploaded when it moves.
    pub fn set_render_origin(&self, scene: &mut Scene, origin: DVec3) {
        if scene.render_origin != origin {
            if origin.z != scene.render_origin.z || scene.model_buf.is_none() {
                scene.dirty = true;
            } else {
                scene.origin_moved = true;
            }
            scene.render_origin = origin;
        }
    }
}
