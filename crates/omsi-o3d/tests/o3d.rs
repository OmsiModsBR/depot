//! Characterization tests for the `.o3d` reader (`parse_o3d`), built from hand-made files.
//!
//! They pin what the reader does today (headers v1, v3 and v4+, short and long indices, the
//! scrambled vertices of v4+, unknown tags, truncated files), so later changes to the reader
//! can be checked against them. The layout is in `docs/FORMATS.md`.

use glam::{Vec2, Vec3};
use omsi_o3d::{parse_o3d, O3dError};

/// One vertex as the mesh should come out: position, normal, uv.
#[derive(Clone, Copy)]
struct V {
    p: [f32; 3],
    n: [f32; 3],
    uv: [f32; 2],
}

const fn v(p: [f32; 3], n: [f32; 3], uv: [f32; 2]) -> V {
    V { p, n, uv }
}

/// Writes an `.o3d` file byte by byte.
struct File {
    b: Vec<u8>,
    version: u8,
    long_indices: bool,
}

impl File {
    fn new(version: u8, flags: u8, key: u32) -> Self {
        let mut b = vec![0x84, 0x19, version];
        if version >= 3 {
            b.push(flags);
        }
        if version >= 4 {
            b.extend(key.to_le_bytes());
        }
        Self {
            b,
            version,
            long_indices: flags & 1 != 0,
        }
    }
    fn u8(&mut self, v: u8) -> &mut Self {
        self.b.push(v);
        self
    }
    fn u16(&mut self, v: u16) -> &mut Self {
        self.b.extend(v.to_le_bytes());
        self
    }
    fn u32(&mut self, v: u32) -> &mut Self {
        self.b.extend(v.to_le_bytes());
        self
    }
    fn f32(&mut self, v: f32) -> &mut Self {
        self.b.extend(v.to_le_bytes());
        self
    }
    fn count(&mut self, n: usize) -> &mut Self {
        if self.version >= 3 {
            self.u32(n as u32)
        } else {
            self.u16(n as u16)
        }
    }
    fn index(&mut self, i: u32) -> &mut Self {
        if self.long_indices {
            self.u32(i)
        } else {
            self.u16(i as u16)
        }
    }
    fn str8(&mut self, s: &str) -> &mut Self {
        self.u8(s.len() as u8);
        self.b.extend(s.as_bytes());
        self
    }
    /// Vertices exactly as given (no scrambling).
    fn vertices(&mut self, vs: &[V]) -> &mut Self {
        self.u8(0x17).count(vs.len());
        for x in vs {
            for f in x.p.iter().chain(&x.n).chain(&x.uv) {
                self.f32(*f);
            }
        }
        self
    }
    fn triangles(&mut self, ts: &[([u32; 3], u16)]) -> &mut Self {
        self.u8(0x49).count(ts.len());
        for (idx, m) in ts {
            for i in idx {
                self.index(*i);
            }
            self.u16(*m);
        }
        self
    }
    fn material(&mut self, diffuse: [f32; 4], texture: &str) -> &mut Self {
        self.u8(0x26).u16(1);
        for f in diffuse {
            self.f32(f);
        }
        for f in [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 8.0] {
            self.f32(f);
        }
        self.str8(texture)
    }
    fn matrix(&mut self, m: [f32; 16]) -> &mut Self {
        self.u8(0x79);
        for f in m {
            self.f32(f);
        }
        self
    }
    fn bone(&mut self, name: &str, weights: &[(u32, f32)]) -> &mut Self {
        self.u8(0x54).u16(1).str8(name).u16(weights.len() as u16);
        for (i, w) in weights {
            self.index(*i);
            self.f32(*w);
        }
        self
    }
}

const QUAD: [V; 4] = [
    v([0.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0]),
    v([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [1.0, 0.0]),
    v([1.0, 0.0, 1.0], [0.0, 1.0, 0.0], [1.0, 1.0]),
    v([0.0, 0.0, 1.0], [0.0, 1.0, 0.0], [0.0, 1.0]),
];

const TRANSLATED: [f32; 16] = [
    1.0, 0.0, 0.0, 0.0, //
    0.0, 1.0, 0.0, 0.0, //
    0.0, 0.0, 1.0, 0.0, //
    2.0, 3.0, 4.0, 1.0,
];

fn assert_vertices(got: &[omsi_o3d::Vertex], want: &[V]) {
    assert_eq!(got.len(), want.len());
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        assert_eq!(g.position, Vec3::from(w.p), "position of vertex {i}");
        assert_eq!(g.normal, Vec3::from(w.n), "normal of vertex {i}");
        assert!(
            (g.uv - Vec2::from(w.uv)).abs().max_element() < 1e-6,
            "uv of vertex {i}: {} != {:?}",
            g.uv,
            w.uv
        );
    }
}

#[test]
fn version_1_has_16_bit_counts_and_indices() {
    let mut f = File::new(1, 0, 0);
    f.vertices(&QUAD)
        .triangles(&[([0, 1, 2], 0), ([0, 2, 3], 0)])
        .material([1.0, 0.5, 0.25, 1.0], "wall.bmp")
        .matrix(TRANSLATED);
    let mesh = parse_o3d(&f.b).unwrap();

    assert_eq!(mesh.version, 1);
    assert_vertices(&mesh.vertices, &QUAD);
    let tris: Vec<_> = mesh
        .triangles
        .iter()
        .map(|t| (t.indices, t.material))
        .collect();
    assert_eq!(tris, [([0, 1, 2], 0), ([0, 2, 3], 0)]);
    assert_eq!(mesh.materials.len(), 1);
    let m = &mesh.materials[0];
    assert_eq!(m.diffuse, [1.0, 0.5, 0.25, 1.0]);
    assert_eq!(m.specular, [0.1, 0.2, 0.3]);
    assert_eq!(m.emissive, [0.4, 0.5, 0.6]);
    assert_eq!(m.specular_power, 8.0);
    assert_eq!(m.texture, "wall.bmp");
    assert!(mesh.has_transform);
    assert_eq!(mesh.origin(), Vec3::new(2.0, 3.0, 4.0));
    assert!(mesh.bones.is_empty());
}

#[test]
fn no_matrix_section_leaves_the_identity() {
    let mut f = File::new(1, 0, 0);
    f.vertices(&QUAD).triangles(&[([0, 1, 2], 0)]);
    let mesh = parse_o3d(&f.b).unwrap();
    assert!(!mesh.has_transform);
    assert_eq!(mesh.transform, glam::Mat4::IDENTITY);
    assert!(mesh.materials.is_empty());
}

#[test]
fn version_3_has_32_bit_counts_and_16_bit_indices() {
    let mut f = File::new(3, 0, 0);
    f.vertices(&QUAD)
        .triangles(&[([3, 2, 1], 1)])
        .bone("Bip01", &[(0, 1.0), (3, 0.5)]);
    let mesh = parse_o3d(&f.b).unwrap();

    assert_eq!(mesh.version, 3);
    assert_vertices(&mesh.vertices, &QUAD);
    assert_eq!(mesh.triangles.len(), 1);
    assert_eq!(mesh.triangles[0].indices, [3, 2, 1]);
    assert_eq!(mesh.triangles[0].material, 1);
    assert_eq!(mesh.bones.len(), 1);
    assert_eq!(mesh.bones[0].name, "Bip01");
    let w: Vec<_> = mesh.bones[0]
        .weights
        .iter()
        .map(|w| (w.vertex, w.weight))
        .collect();
    assert_eq!(w, [(0, 1.0), (3, 0.5)]);
}

#[test]
fn flag_bit_0_makes_triangle_and_bone_indices_32_bit() {
    // Indices above u16::MAX only fit as u32; they are out of range for this mesh, so the
    // triangle naming them is dropped, but the bone keeps them as written.
    let mut f = File::new(3, 1, 0);
    f.vertices(&QUAD)
        .triangles(&[([0, 1, 2], 0), ([0, 70_000, 2], 0)])
        .bone("Bone", &[(70_000, 0.25)]);
    let mesh = parse_o3d(&f.b).unwrap();

    assert_eq!(mesh.triangles.len(), 1);
    assert_eq!(mesh.triangles[0].indices, [0, 1, 2]);
    assert_eq!(mesh.bones[0].weights[0].vertex, 70_000);
    assert_eq!(mesh.bones[0].weights[0].weight, 0.25);
}

#[test]
fn triangles_naming_missing_vertices_are_dropped() {
    let mut f = File::new(1, 0, 0);
    f.vertices(&QUAD)
        .triangles(&[([0, 1, 4], 0), ([0, 2, 3], 2)]);
    let mesh = parse_o3d(&f.b).unwrap();
    assert_eq!(mesh.triangles.len(), 1);
    assert_eq!(mesh.triangles[0].indices, [0, 2, 3]);
    assert_eq!(mesh.triangles[0].material, 2);
}

#[test]
fn version_4_with_key_ffffffff_is_not_scrambled() {
    let mut f = File::new(4, 0, 0xFFFF_FFFF);
    f.vertices(&QUAD).triangles(&[([0, 1, 2], 0)]);
    let mesh = parse_o3d(&f.b).unwrap();
    assert_eq!(mesh.version, 4);
    assert_vertices(&mesh.vertices, &QUAD);
}

#[test]
fn version_4_key_0_unscrambles_by_hand_worked_values() {
    // Two vertices, key 0, no flags. Worked out from the rules in docs/FORMATS.md:
    // vertex 0: state = 0 -> swap(x,y); negate nx, ny, nz; swap(ny,nz); uv unchanged.
    // vertex 1: b = trunc(0.5*0.5*0.5*600) = 75 from the raw position of vertex 0,
    //           state = (0*2 + 2*75) mod 8000 = 150 -> swap(x,y); negate ny (150 % 6 == 0);
    //           swap(ny,nz); u -= 50²/10000 = 0.25; v -= 0.
    let raw = [
        v([1.5, 2.5, 3.5], [0.1, 0.2, 0.3], [0.25, 0.75]),
        v([10.0, 20.0, 30.0], [0.6, 0.8, 0.0], [0.5, 0.5]),
    ];
    let mut f = File::new(4, 0, 0);
    f.vertices(&raw).triangles(&[([0, 1, 1], 0)]);
    let mesh = parse_o3d(&f.b).unwrap();
    assert_vertices(
        &mesh.vertices,
        &[
            v([2.5, 1.5, 3.5], [-0.1, -0.3, -0.2], [0.25, 0.75]),
            v([20.0, 10.0, 30.0], [0.6, 0.0, -0.8], [0.25, 0.5]),
        ],
    );
}

/// The byte that feeds the next vertex's state, from a raw (scrambled) position.
fn frac_byte(p: [f32; 3]) -> u32 {
    let f = |x: f32| (x as f64) - (x as f64).trunc();
    let prod = (f(p[0]) * f(p[1]) * f(p[2])).abs() * 600.0;
    (prod.trunc() as i64).rem_euclid(256) as u32
}

/// Scramble vertices the way a v4+ exporter does, so that the reader gives them back.
/// It is the reader's per-vertex step run backwards: uv added back, normal swapped and then
/// negated, position swapped.
fn scramble(version: u8, flags: u8, key: u32, vs: &[V]) -> Vec<V> {
    let alt = flags & 2 != 0;
    let k = key as i64 + (version as i64 - 4) + if alt { 0x17D } else { 0 };
    let mut state = k.rem_euclid(0xFDE8) as u16;
    let n16 = (vs.len() as i64 % 0xFDE8) as u32;
    let mut fb = 0u32;
    let mut out = Vec::new();
    for x in vs {
        if key == 0 {
            state = if alt { 0x130 } else { 0 };
        }
        state = ((state as u32 * n16 + n16 * fb) % 8000) as u16;
        let s = state;
        let (mut p, mut n, mut uv) = (x.p, x.n, x.uv);
        if s.is_multiple_of(5) {
            let m = (s % 100) as f32;
            uv[0] += m * m / 10000.0;
        }
        if s.is_multiple_of(3) {
            let m = (s % 50) as f32;
            uv[1] += m * m / 2500.0;
        }
        if s < 600 {
            n.swap(1, 2);
        } else if s > 4500 {
            n.swap(0, 1);
        }
        if s & 3 == 0 {
            n[0] = -n[0];
        }
        if s.is_multiple_of(6) {
            n[1] = -n[1];
        }
        if s.is_multiple_of(7) {
            n[2] = -n[2];
        }
        if s < 1000 {
            p.swap(0, 1);
        } else if s < 3000 {
            p.swap(0, 2);
        } else if s > 7000 {
            p.swap(1, 2);
        }
        fb = frac_byte(p);
        out.push(V { p, n, uv });
    }
    out
}

/// Vertices with varied fractional positions, so the states run through every branch.
fn varied(count: usize) -> Vec<V> {
    (0..count)
        .map(|i| {
            let t = i as f32;
            v(
                [t * 1.37 + 0.11, t * -2.71 + 0.53, t * 0.89 + 0.97],
                [0.25, -0.5, 0.75],
                [t * 0.125, 1.0 - t * 0.0625],
            )
        })
        .collect()
}

#[test]
fn scrambled_vertices_come_back_for_every_key_and_flag() {
    let plain = varied(37);
    for (version, flags, key) in [
        (4, 0, 0),
        (4, 2, 0),
        (5, 0, 12_345),
        (7, 1, 0xDEAD_BEEF),
        (7, 3, 977),
    ] {
        let mut f = File::new(version, flags, key);
        f.vertices(&scramble(version, flags, key, &plain))
            .triangles(&[([0, 1, 2], 0)]);
        let mesh = parse_o3d(&f.b).unwrap();
        assert_vertices(&mesh.vertices, &plain);
        assert_eq!(mesh.triangles.len(), 1, "v{version} flags {flags}");
    }
}

#[test]
fn unknown_bytes_between_sections_are_passed_over() {
    // Like the protected v7 files: junk where a tag is expected, between and after sections.
    let mut f = File::new(1, 0, 0);
    f.vertices(&QUAD).u8(0x00).u8(0xAB).u8(0x01);
    f.triangles(&[([0, 1, 2], 0)]).material([1.0; 4], "a.tga");
    f.u8(0x00).u8(0x00);
    let mesh = parse_o3d(&f.b).unwrap();
    assert_vertices(&mesh.vertices, &QUAD);
    assert_eq!(mesh.triangles.len(), 1);
    assert_eq!(mesh.materials[0].texture, "a.tga");
}

#[test]
fn bad_magic_is_refused() {
    let mut f = File::new(1, 0, 0);
    f.vertices(&QUAD);
    f.b[1] = 0x20;
    assert!(matches!(parse_o3d(&f.b), Err(O3dError::BadMagic)));
    assert!(matches!(parse_o3d(&[0x84]), Err(O3dError::Eof(1))));
    assert!(matches!(parse_o3d(&[]), Err(O3dError::Eof(0))));
}

#[test]
fn truncated_files_fail_with_the_offset() {
    let mut f = File::new(3, 0, 0);
    f.vertices(&QUAD).triangles(&[([0, 1, 2], 0)]);
    // header (4) + tag (1) + count (4): the vertex block starts at 9 and needs 4 × 32 bytes.
    let cut = &f.b[..9 + 100];
    assert!(matches!(parse_o3d(cut), Err(O3dError::Eof(9))));
    // Inside the triangle count.
    let tri_tag = 9 + 4 * 32;
    let cut = &f.b[..tri_tag + 3];
    assert!(matches!(parse_o3d(cut), Err(O3dError::Eof(p)) if p == tri_tag + 1));
    // A v4 header without the whole key.
    assert!(matches!(
        parse_o3d(&[0x84, 0x19, 4, 0, 0xFF, 0xFF]),
        Err(O3dError::Eof(4))
    ));
}

#[test]
fn empty_body_gives_an_empty_mesh() {
    let f = File::new(7, 0, 0xFFFF_FFFF);
    let mesh = parse_o3d(&f.b).unwrap();
    assert_eq!(mesh.version, 7);
    assert!(mesh.vertices.is_empty() && mesh.triangles.is_empty());
    assert_eq!(mesh.bounds(), (Vec3::ZERO, Vec3::ZERO));
}
