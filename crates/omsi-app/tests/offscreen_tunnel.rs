//! Characterization of a tunnel mouth cut into a hillside (issue #42), on content the test
//! writes itself: one tile whose ground rises from 0 to 25 m, and one object whose
//! `[terrainhole]` cutter and visible mesh form a 6 m wide tube running into the slope.
//!
//! Looking at the mouth from the road, the tube's inside must show where the cutter took
//! the ground away, and the ground must stay where it was not cut. The report in #42 is
//! ground stretched across such a mouth; this root does not reproduce it, and these tests
//! pin what the offscreen run shows today so a change in the cut or the terrain is noticed.
//!
//! The pictures need a graphics adapter (a software one is enough), so they only run with
//! `OMSI_GOLDEN_RENDER=1`, like `offscreen_golden`:
//!
//! ```text
//! OMSI_GOLDEN_RENDER=1 cargo test -p omsi-app --test offscreen_tunnel -- --nocapture
//! ```

use std::path::PathBuf;
use std::process::Command;

const SIZE: (u32, u32) = (320, 180);

struct Root(PathBuf);

/// A `.x` mesh from quads (four vertex indices each, both sides drawn), flat colour texture.
fn x_mesh(verts: &[(f32, f32, f32)], quads: &[[usize; 4]], texture: &str) -> String {
    let mut faces = Vec::new();
    for q in quads {
        for t in [
            [q[0], q[1], q[2]],
            [q[0], q[2], q[3]],
            [q[2], q[1], q[0]],
            [q[3], q[2], q[0]],
        ] {
            faces.push(format!("3;{},{},{};", t[0], t[1], t[2]));
        }
    }
    let v: Vec<String> = verts
        .iter()
        .map(|(x, y, z)| format!("{x};{y};{z};"))
        .collect();
    let uvs: Vec<String> = (0..verts.len())
        .map(|i| format!("{};{};", i % 2, (i / 2) % 2))
        .collect();
    format!(
        "xof 0303txt 0032\nMesh m {{\n{};\n{};\n{};\n{};\nMeshTextureCoords {{{};\n{};}}\n\
         MeshMaterialList {{1;{};{};;Material {{1;1;1;1;;0;0;0;0;;0;0;0;; TextureFilename {{\"{texture}\";}} }}}}\n}}\n",
        verts.len(),
        v.join(",\n"),
        faces.len(),
        faces.join(",\n"),
        uvs.len(),
        uvs.join(","),
        faces.len(),
        vec!["0"; faces.len()].join(","),
    )
}

impl Root {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "depot-tunnel-offscreen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        for rel in omsi_cfg::ORIGINAL_ESSENTIALS {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            if rel.contains('.') {
                std::fs::write(&path, "").unwrap();
            } else {
                std::fs::create_dir_all(&path).unwrap();
            }
        }
        let map_dir = dir.join("maps").join("Tunnel");
        std::fs::create_dir_all(&map_dir).unwrap();
        let sco_dir = dir.join("Sceneryobjects").join("Tunnel");
        for sub in ["model", "texture"] {
            std::fs::create_dir_all(sco_dir.join(sub)).unwrap();
        }
        let w = |name: &str, text: &str| std::fs::write(map_dir.join(name), text).unwrap();
        let o = |name: &str, text: &str| std::fs::write(sco_dir.join(name), text).unwrap();
        w(
            "global.cfg",
            "[name]\nTunnel offscreen\n\n[map]\n0\n0\ntile_0_0.map\n",
        );
        // the tube stands on the ground at its mouth, 50 m north of the tile's south edge, on x = 150
        w("tile_0_0.map", "[version]\n14\n\n[object]\n0\nSceneryobjects\\Tunnel\\tube.sco\n7\n150\n50\n0\n0\n0\n0\n0\n");
        // the ground: flat to 50 m, then a 1:1 slope up to 25 m at 75 m, flat after that
        let cells = 60usize;
        let step = 300.0 / cells as f32;
        let mut terrain = (cells as u32).to_le_bytes().to_vec();
        for iy in 0..=cells {
            let y = iy as f32 * step;
            let h = (y - 50.0).clamp(0.0, 25.0);
            for _ix in 0..=cells {
                terrain.extend_from_slice(&h.to_le_bytes());
            }
        }
        std::fs::write(map_dir.join("tile_0_0.map.terrain"), terrain).unwrap();
        o(
            "tube.sco",
            "[groups]\n1\nTunnel\n[friendlyname]\nTube\n[terrainhole]\ncut.x\n[mesh]\ntube.x\n",
        );
        // the cutter: a box 8 m wide, 6 high, 60 m long, from 1 m below the road
        let (hw, hh, len) = (4.0, 6.0, 60.0);
        // (x; up; along the tunnel: the `.x` files of the game keep height in the second place)
        let cutter = [
            (-hw, -1.0, 0.0),
            (hw, -1.0, 0.0),
            (hw, -1.0, len),
            (-hw, -1.0, len),
            (-hw, hh, 0.0),
            (hw, hh, 0.0),
            (hw, hh, len),
            (-hw, hh, len),
        ];
        let cutter_quads = [
            [0, 1, 2, 3],
            [4, 7, 6, 5],
            [0, 4, 5, 1],
            [1, 5, 6, 2],
            [2, 6, 7, 3],
            [3, 7, 4, 0],
        ];
        o("model/cut.x", &x_mesh(&cutter, &cutter_quads, "tube.png"));
        // the visible tube: floor, ceiling, two walls and the far end, open at the mouth; the
        // object's depth axis runs into the hill
        let (w2, h2, l2) = (3.0, 5.0, 55.0);
        let tube = [
            (-w2, 0.0, 0.0),
            (w2, 0.0, 0.0),
            (w2, 0.0, l2),
            (-w2, 0.0, l2),
            (-w2, h2, 0.0),
            (w2, h2, 0.0),
            (w2, h2, l2),
            (-w2, h2, l2),
        ];
        let tube_quads = [
            [0, 1, 2, 3],
            [4, 7, 6, 5],
            [0, 4, 7, 3],
            [1, 2, 6, 5],
            [3, 2, 6, 7],
        ];
        o("model/tube.x", &x_mesh(&tube, &tube_quads, "tube.png"));
        image::save_buffer(
            sco_dir.join("texture").join("tube.png"),
            &[220, 40, 40, 255].repeat(16),
            4,
            4,
            image::ColorType::Rgba8,
        )
        .unwrap();
        Root(dir)
    }

    fn render(&self, name: &str, cam: &str) -> Result<image::RgbaImage, String> {
        let out = self.0.join(name);
        let home = self.0.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_depot"))
            .arg("--root")
            .arg(&self.0)
            .args(["--map", "maps/Tunnel/global.cfg", "--offscreen"])
            .arg(&out)
            .args([
                "--size",
                &format!("{}x{}", SIZE.0, SIZE.1),
                "--cam",
                cam,
                "--time",
                "12:00",
            ])
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("OMSI_ROOT", &self.0)
            .output()
            .map_err(|e| e.to_string())?;
        if !run.status.success() {
            return Err(format!(
                "depot exited with {}:\n{}\n{}",
                run.status,
                String::from_utf8_lossy(&run.stdout),
                String::from_utf8_lossy(&run.stderr)
            ));
        }
        Ok(image::open(&out).map_err(|e| e.to_string())?.to_rgba8())
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        if let (Ok(p), Ok(t)) = (self.0.canonicalize(), std::env::temp_dir().canonicalize()) {
            if p.starts_with(&t) && p != t {
                let _ = std::fs::remove_dir_all(p);
            }
        }
    }
}

fn adapter_present() -> bool {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .any(|a| a.get_info().backend != wgpu::Backend::Noop)
}

fn wanted() -> bool {
    if std::env::var_os("OMSI_GOLDEN_RENDER").is_none() {
        eprintln!(
            "SKIP: set OMSI_GOLDEN_RENDER=1 to render the tunnel root (needs a graphics adapter)"
        );
        return false;
    }
    if !adapter_present() {
        assert!(
            std::env::var_os("OMSI_GOLDEN_REQUIRE").is_none(),
            "OMSI_GOLDEN_REQUIRE is set but there is no graphics adapter"
        );
        eprintln!("SKIP: no graphics adapter here");
        return false;
    }
    true
}

#[test]
fn tunnel_root_reads_back_as_written() {
    let root = Root::new();
    let global = omsi_map::GlobalCfg::load(&root.0.join("maps/Tunnel/global.cfg")).unwrap();
    assert_eq!(global.tiles.len(), 1);
    let terrain =
        omsi_map::Terrain::load(&root.0.join("maps/Tunnel/tile_0_0.map.terrain")).unwrap();
    assert_eq!(terrain.cells, 60);
    assert_eq!(terrain.sample(150.0, 40.0), 0.0);
    assert!((terrain.sample(150.0, 62.5) - 12.5).abs() < 0.01);
    assert_eq!(terrain.sample(150.0, 200.0), 25.0);
    let tile = omsi_map::Tile::load(&root.0.join("maps/Tunnel/tile_0_0.map")).unwrap();
    assert_eq!(tile.objects.len(), 1);
}

/// What the run shows today, on a tube 6 m wide cut by an 8 m wide cutter into a 25 m slope:
///
/// * the ground stays where it was not cut (grey on both sides of the mouth);
/// * the tube's inside shows in the middle of the mouth (the red of its texture, dark);
/// * the cutter takes the ground away over the whole length of the tube, up to the sky: the
///   cut is made on the tile's plan, whatever the ground's height, so above the cutter the
///   slope is gone and the sky shows through the slot. (This root does not reproduce the
///   ground stretched over a mouth reported in #42; it pins this behaviour so a change in
///   the cut is noticed.)
#[test]
fn tunnel_mouth_shows_the_tube_and_the_cut_reaches_the_sky() {
    if !wanted() {
        return;
    }
    let root = Root::new();
    // 30 m south of the mouth, on the tube's axis, a little above the road, looking north
    let img = root
        .render("mouth.png", "150,20,1.7,0,0")
        .unwrap_or_else(|e| panic!("{e}"));
    let px = |x: u32, y: u32| {
        let p = img.get_pixel(x, y).0;
        [p[0] as i32, p[1] as i32, p[2] as i32]
    };
    let (centre, left, right, above) = (
        px(SIZE.0 / 2, SIZE.1 / 2),
        px(40, SIZE.1 / 2),
        px(SIZE.0 - 40, SIZE.1 / 2),
        px(SIZE.0 / 2, 20),
    );
    eprintln!("centre {centre:?} left {left:?} right {right:?} above {above:?}");
    for (name, c) in [("left", left), ("right", right)] {
        assert!(
            (c[0] - c[2]).abs() < 15 && c[0] > 40,
            "ground beside the mouth ({name}): {c:?}"
        );
    }
    assert!(
        centre[0] > centre[1] + 6,
        "the tube's inside at the mouth: {centre:?}"
    );
    assert!(
        above[2] > above[0] + 15 && above[2] > 150,
        "sky over the mouth through the cut: {above:?}"
    );
}
