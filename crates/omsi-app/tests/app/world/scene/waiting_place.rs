//! #189: the floor of somebody standing at a stop's waiting place on a sloping pavement.
use glam::{DVec3, Mat4};
use omsi_geometry::TileSurface;
use openomsi_game::testing::scene::*;
use std::path::PathBuf;
use std::sync::Arc;

struct Fixture(PathBuf);

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A 6 m wide strip along y through `stations` of (y, height); .x order is x, up, y.
fn strip(stations: &[(f32, f32)]) -> omsi_geometry::MeshData {
    let vertices: Vec<_> = stations
        .iter()
        .flat_map(|&(y, z)| [-3.0, 3.0].map(|x| format!("{x};{z};{y};")))
        .collect();
    let faces: Vec<_> = (0..stations.len() - 1)
        .flat_map(|i| {
            let a = 2 * i;
            [
                format!("3;{a},{},{};", a + 2, a + 1),
                format!("3;{},{},{};", a + 1, a + 2, a + 3),
            ]
        })
        .collect();
    let x = format!(
        "xof 0303txt 0032\nMesh strip {{\n{};\n{};\n{};\n{};\n}}\n",
        vertices.len(),
        vertices.join(",\n"),
        faces.len(),
        faces.join(",\n")
    );
    omsi_geometry::mesh_from_o3d(&omsi_o3d::xfile::parse_x(x.as_bytes()).unwrap())
}

const ORIGIN: DVec3 = DVec3::new(100.0, 100.0, 0.0);
/// The pavement: 15 cm up at the stop object's origin, rising 15 % along y.
fn pavement(y: f64) -> f64 {
    0.15 + 0.15 * y
}

/// A sloping pavement over a level face (the ground under it, 0 m), and a shelter's roof
/// 2.7 m up over its first 2 m.
fn stop_world() -> (Fixture, World) {
    let fixture = Fixture(std::env::temp_dir().join(format!(
        "depot-waiting-place-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
    )));
    std::fs::create_dir_all(&fixture.0).unwrap();
    std::fs::write(fixture.0.join("global.cfg"), "[name]\nSloping stop\n").unwrap();
    let world = World::open(&fixture.0, &fixture.0.join("global.cfg"), 20261001).unwrap();
    let mut surface = TileSurface::new(128);
    let ys = [0.0, 2.0, 4.0, 6.0, 8.0, 10.0];
    let slope: Vec<_> = ys.iter().map(|&y| (y, pavement(y as f64) as f32)).collect();
    for mesh in [
        strip(&ys.map(|y| (y, 0.0))),
        strip(&slope),
        strip(&[(0.0, 2.7), (2.0, 2.7)]),
    ] {
        surface.add_drive_mesh(&mesh, &Mat4::IDENTITY, ORIGIN, 0, 0);
    }
    surface.finish();
    world.surfaces.write().insert((0, 0), Arc::new(surface));
    (fixture, world)
}

/// A stop object's waiting places lie at the height of its origin. On a pavement that
/// rises, the places along it lie under the pavement (0.9 m at 6 m) and the people there
/// stood on the ground under it, sunk in the pavement; now they stand on the pavement.
#[test]
fn waiting_places_under_a_rising_pavement_stand_on_it() {
    let (_fixture, world) = stop_world();
    for y in [2.0, 4.0, 6.0, 7.0] {
        let place = ORIGIN + DVec3::new(1.0, y, pavement(0.0));
        let floor = world.waiting_place_floor(place).unwrap();
        assert!(
            (floor - pavement(y)).abs() < 1e-3,
            "{y} m along: floor {floor:.3}, pavement {:.3}",
            pavement(y)
        );
    }
    // (the walk's own half a metre up is what sank them)
    let deep = ORIGIN + DVec3::new(1.0, 6.0, pavement(0.0));
    assert!(world.walk_height_near(deep.x, deep.y, deep.z).unwrap() < 0.01);
}

/// Under the shelter the floor is the pavement, not the roof: a place at the floor and a
/// place 0.8 m over it (the far end of a stop whose pavement falls away).
#[test]
fn waiting_places_under_a_shelter_stay_off_its_roof() {
    let (_fixture, world) = stop_world();
    for over in [0.0, 0.8] {
        let place = ORIGIN + DVec3::new(-1.0, 1.0, pavement(1.0) + over);
        let floor = world.waiting_place_floor(place).unwrap();
        assert!(
            (floor - pavement(1.0)).abs() < 1e-3,
            "{over} m over: floor {floor:.3}"
        );
    }
}
