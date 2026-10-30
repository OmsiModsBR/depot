//! The ray tests the cursor makes in a cockpit: the variants that take the transform's
//! inverse give the same answers bit for bit, and (ignored, run by hand) what a look under
//! the cursor costs in a synthetic cab with the eye inside every mesh's bounding sphere.

use glam::{Mat4, Quat, Vec3};
use omsi_geometry::{
    cone_triangles, cone_triangles_inv, ray_mesh, ray_mesh_inv, ray_triangles, ray_triangles_inv,
    MeshData,
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

/// `cargo test -p omsi-geometry --release --test cursor_rays -- --ignored --nocapture`: a
/// cab of 300 meshes and about 200 000 triangles round an eye that is inside each mesh's
/// bounding sphere, and one look under the cursor (25 rays) through all of them - what the
/// blocker test of a look costs today, before any tree over the triangles.
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
    let triangles: usize = meshes.iter().map(|m| m.0.indices.len() / 3).sum();
    let eye = Vec3::new(0.0, 0.0, 0.3);
    let rays: Vec<Vec3> = (0..25)
        .map(|k| Vec3::new(0.01 * k as f32, 1.0, -0.2).normalize())
        .collect();
    let looks = 20;
    for (label, with_inverse) in [("posed each ray", false), ("inverse once", true)] {
        let t = std::time::Instant::now();
        let mut hits = 0usize;
        for _ in 0..looks {
            for d in &rays {
                for (i, (mesh, xf)) in meshes.iter().enumerate() {
                    let hit = if with_inverse {
                        ray_mesh_inv(eye, *d, mesh, &invs[i])
                    } else {
                        ray_mesh(eye, *d, mesh, xf)
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
