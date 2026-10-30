//! The ray tests the cursor makes in a cockpit: the variants that take the transform's
//! inverse and the ones through a mesh's tree give the same answers bit for bit, and
//! (ignored, run by hand) what a look under the cursor costs in a synthetic cab with the eye
//! inside every mesh's bounding sphere.

use glam::{Mat4, Quat, Vec3};
use omsi_geometry::{
    cone_triangles, cone_triangles_inv, ray_mesh, ray_mesh_inv, ray_triangles, ray_triangles_inv,
    MeshData, TriTree,
};

/// A small deterministic generator, so the meshes are the same on every run.
struct Lcg(u64);

impl Lcg {
    fn next(&mut self) -> f32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        ((self.0 >> 40) as f32) / (1u64 << 24) as f32
    }

    fn vec(&mut self, half: f32) -> Vec3 {
        Vec3::new(
            (self.next() * 2.0 - 1.0) * half,
            (self.next() * 2.0 - 1.0) * half,
            (self.next() * 2.0 - 1.0) * half,
        )
    }
}

/// `n` triangles of up to `size` m scattered in a box of `half` m round `centre`.
fn soup(rng: &mut Lcg, n: usize, centre: Vec3, half: f32, size: f32) -> MeshData {
    let mut m = MeshData::default();
    for _ in 0..n {
        let a = centre + rng.vec(half);
        let base = m.positions.len() as u32;
        m.positions.push(a);
        m.positions.push(a + rng.vec(size));
        m.positions.push(a + rng.vec(size));
        m.indices.extend([base, base + 1, base + 2]);
    }
    m
}

fn posed(rng: &mut Lcg) -> Mat4 {
    Mat4::from_scale_rotation_translation(
        Vec3::splat(0.9 + rng.next() * 0.2),
        Quat::from_rotation_z(rng.next() * 6.0) * Quat::from_rotation_x(rng.next() * 0.5),
        rng.vec(0.5),
    )
}

#[test]
fn the_inverse_taking_tests_give_the_same_answers_bit_for_bit() {
    let mut rng = Lcg(7);
    for _ in 0..40 {
        let mesh = soup(&mut rng, 400, Vec3::ZERO, 1.0, 0.3);
        let xf = posed(&mut rng);
        let inv = xf.inverse();
        for _ in 0..50 {
            let o = rng.vec(2.0);
            let d = (rng.vec(1.0) - o * 0.3).normalize_or(Vec3::Y);
            assert_eq!(
                ray_mesh(o, d, &mesh, &xf).map(f32::to_bits),
                ray_mesh_inv(o, d, &mesh, &inv).map(f32::to_bits)
            );
            let tris = cone_triangles(o, d, 0.05, &mesh, &xf);
            assert_eq!(tris, cone_triangles_inv(o, d, 0.05, &mesh, &inv));
            assert_eq!(
                ray_triangles(o, d, &mesh, &xf, &tris).map(f32::to_bits),
                ray_triangles_inv(o, d, &mesh, &inv, &tris).map(f32::to_bits)
            );
        }
    }
}

/// A panel in a plane of the mesh's frame (every `z` the same), in a grid of triangles that
/// share their edges, as modelled cockpit panels are: rays along the plane and through its
/// edges and corners.
fn panel(rng: &mut Lcg, n: usize) -> MeshData {
    let mut m = MeshData::default();
    let z = rng.next() - 0.5;
    for y in 0..=n {
        for x in 0..=n {
            m.positions
                .push(Vec3::new(x as f32 * 0.05, y as f32 * 0.05, z));
        }
    }
    let w = (n + 1) as u32;
    for y in 0..n as u32 {
        for x in 0..n as u32 {
            let a = y * w + x;
            m.indices.extend([a, a + 1, a + w, a + 1, a + w + 1, a + w]);
        }
    }
    // the same triangles twice, as some exports have them
    let again = m.indices[..12].to_vec();
    m.indices.extend(again);
    m
}

#[test]
fn the_tree_gives_the_full_scans_answers_bit_for_bit() {
    let mut rng = Lcg(23);
    let mut hits = 0;
    for round in 0..60 {
        let mesh = if round % 3 == 0 {
            panel(&mut rng, 12)
        } else {
            soup(&mut rng, 50 + 40 * round, Vec3::ZERO, 1.0, 0.3)
        };
        let tree = TriTree::build(&mesh);
        assert_eq!(tree.triangles(), mesh.indices.len() / 3);
        let xf = if round % 4 == 0 {
            Mat4::IDENTITY
        } else {
            posed(&mut rng)
        };
        let inv = xf.inverse();
        let mut tested = 0u64;
        for n in 0..200 {
            let o = match n % 4 {
                // inside the mesh, as the eye in a cab
                0 => rng.vec(0.5),
                1 => rng.vec(3.0),
                // on the panel's grid points
                2 => xf.transform_point3(Vec3::new(
                    (n % 13) as f32 * 0.05,
                    (n % 7) as f32 * 0.05,
                    -1.0,
                )),
                _ => rng.vec(2.0),
            };
            let d = match n % 5 {
                // along the axes of the mesh's frame, and in its planes
                0 => xf.transform_vector3(Vec3::Z).normalize(),
                1 => xf.transform_vector3(Vec3::new(1.0, 0.0, 0.0)).normalize(),
                2 => xf
                    .transform_vector3(Vec3::new(rng.next(), 0.0, rng.next() - 0.5))
                    .normalize_or(Vec3::Y),
                _ => (rng.vec(1.0) - o * 0.3).normalize_or(Vec3::Y),
            };
            let full = ray_mesh_inv(o, d, &mesh, &inv);
            hits += full.is_some() as usize;
            assert_eq!(
                full.map(f32::to_bits),
                tree.ray_mesh_inv(o, d, &mesh, &inv, &mut tested)
                    .map(f32::to_bits),
                "round {round} ray {n}"
            );
            for spread in [0.0, 0.01, 0.05, 0.3] {
                assert_eq!(
                    cone_triangles_inv(o, d, spread, &mesh, &inv),
                    tree.cone_triangles_inv(o, d, spread, &mesh, &inv, &mut tested),
                    "round {round} cone {n} spread {spread}"
                );
            }
        }
    }
    assert!(
        hits > 1000,
        "{hits} hits: the rays should meet the meshes often"
    );
}

/// `cargo test -p omsi-geometry --release --test cursor_rays -- --ignored --nocapture`: a
/// cab of 300 meshes and about 200 000 triangles round an eye that is inside each mesh's
/// bounding sphere, and one look under the cursor (25 rays) through all of them - what the
/// blocker test of a look costs with full scans and through the meshes' trees.
#[test]
#[ignore]
fn a_look_through_a_synthetic_cab() {
    let mut rng = Lcg(11);
    let meshes: Vec<(MeshData, Mat4)> = (0..300)
        .map(|k| {
            let n = if k < 20 { 6000 } else { 260 };
            (soup(&mut rng, n, Vec3::ZERO, 2.5, 0.2), posed(&mut rng))
        })
        .collect();
    let invs: Vec<Mat4> = meshes.iter().map(|m| m.1.inverse()).collect();
    let t = std::time::Instant::now();
    let trees: Vec<TriTree> = meshes.iter().map(|m| TriTree::build(&m.0)).collect();
    eprintln!(
        "trees built in {:.1} ms",
        t.elapsed().as_secs_f64() * 1000.0
    );
    let triangles: usize = meshes.iter().map(|m| m.0.indices.len() / 3).sum();
    let eye = Vec3::new(0.0, 0.0, 0.3);
    let rays: Vec<Vec3> = (0..25)
        .map(|k| Vec3::new(0.01 * k as f32, 1.0, -0.2).normalize())
        .collect();
    let looks = 20;
    for (label, how) in [
        ("posed each ray", 0),
        ("inverse once", 1),
        ("through the trees", 2),
    ] {
        let t = std::time::Instant::now();
        let mut hits = 0usize;
        for _ in 0..looks {
            for d in &rays {
                for (i, (mesh, xf)) in meshes.iter().enumerate() {
                    let hit = match how {
                        0 => ray_mesh(eye, *d, mesh, xf),
                        1 => ray_mesh_inv(eye, *d, mesh, &invs[i]),
                        _ => trees[i].ray_mesh_inv(eye, *d, mesh, &invs[i], &mut 0),
                    };
                    hits += hit.is_some() as usize;
                }
            }
        }
        eprintln!(
            "{label}: {:.2} ms per look of 25 rays through {} meshes, {triangles} triangles ({hits} hits)",
            t.elapsed().as_secs_f64() * 1000.0 / looks as f64,
            meshes.len()
        );
    }
}
