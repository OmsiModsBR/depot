//! The sphere a mesh is culled and sorted by takes in what the mesh draws: the vertices its
//! triangles use, not leftovers in the file that no triangle names.

use glam::{Vec2, Vec3};
use omsi_geometry::MeshData;
use omsi_render::testing::mesh_bounds;

/// One triangle a metre wide, plus `extra` vertices no triangle uses.
fn triangle_with(extra: &[Vec3]) -> MeshData {
    let mut positions = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
    ];
    positions.extend_from_slice(extra);
    let n = positions.len();
    MeshData {
        positions,
        normals: vec![Vec3::Z; n],
        uvs: vec![Vec2::ZERO; n],
        ranges: vec![(0, 3, 0)],
        indices: vec![0, 1, 2],
        one_sided: true,
    }
}

#[test]
fn a_vertex_no_triangle_uses_leaves_the_sphere_alone() {
    let clean = mesh_bounds(&triangle_with(&[]));
    // (an o3d lamp mesh of an articulated bus carries four such vertices, the farthest 6e25 m
    // out: its sphere took in the whole world)
    let stray = mesh_bounds(&triangle_with(&[
        Vec3::new(5.8e25, 9.2e14, 0.0),
        Vec3::new(-5.4e11, 0.0, -3.0),
    ]));
    assert_eq!(stray, clean);
    assert!(clean.1 < 1.0, "radius {}", clean.1);
}

#[test]
fn a_mesh_without_triangles_has_a_point_sphere() {
    let mut data = triangle_with(&[]);
    data.indices.clear();
    data.ranges.clear();
    assert_eq!(mesh_bounds(&data), (Vec3::ZERO, 0.0));
}
