//! The ground byte of the face a wheel stands on ([`DriveGrid::kind_at`]).

use glam::{DVec3, Mat4, Vec2, Vec3};
use omsi_geometry::{DriveGrid, MeshData, SurfFaces, TileSurface};

/// A flat square (0..20 m) at `z`.
fn square(z: f32) -> [[Vec3; 3]; 2] {
    let (a, b, c, d) = (
        Vec3::new(0.0, 0.0, z),
        Vec3::new(20.0, 0.0, z),
        Vec3::new(20.0, 20.0, z),
        Vec3::new(0.0, 20.0, z),
    );
    [[a, b, c], [a, c, d]]
}

/// Of the faces over a point, the one at the height asked for gives its byte; a height no
/// road face lies at (the terrain beside the road) gives none, and a wall top is never one.
#[test]
fn the_face_at_the_wheels_height_gives_its_ground() {
    let mut grid = DriveGrid::default();
    for t in square(1.0) {
        grid.push_face(t, false, 0x21);
    }
    for t in square(5.0) {
        grid.push_face(t, false, 2);
    }
    for t in square(3.0) {
        grid.push_face(t, true, 7);
    }
    grid.build(300.0);
    assert_eq!(grid.kind_at(5.0, 5.0, 1.0), Some(0x21));
    assert_eq!(grid.kind_at(5.0, 5.0, 1.002), Some(0x21));
    assert_eq!(grid.kind_at(5.0, 5.0, 5.0), Some(2));
    assert_eq!(grid.kind_at(5.0, 5.0, 3.0), None);
    assert_eq!(grid.kind_at(5.0, 5.0, 0.5), None);
    assert_eq!(grid.kind_at(50.0, 50.0, 1.0), None);
}

/// A mesh added to a tile takes each face's byte from its material slot.
#[test]
fn a_meshs_faces_take_their_textures_ground() {
    let mut mesh = MeshData::default();
    for t in square(0.0) {
        for p in t {
            mesh.positions.push(p);
        }
    }
    mesh.indices = (0..6).collect();
    // first triangle with slot 0, second with slot 1
    mesh.ranges = vec![(0, 3, 0), (3, 3, 1)];
    mesh.uvs = vec![Vec2::ZERO; 6];
    let surf = SurfFaces::of(&mesh, &[None, None], &[0, 0x42]).expect("a ground other than 0");
    assert!(SurfFaces::of(&mesh, &[None, None], &[0, 0]).is_none());
    let mut tile = TileSurface::new(64);
    tile.add_drive_mesh_surf(&mesh, &Mat4::IDENTITY, DVec3::ZERO, 0, 0, Some(&surf));
    tile.finish();
    // (15, 5) lies in the first triangle, (5, 15) in the second
    assert_eq!(tile.drive.kind_at(15.0, 5.0, 0.0), Some(0));
    assert_eq!(tile.drive.kind_at(5.0, 15.0, 0.0), Some(0x42));
}
