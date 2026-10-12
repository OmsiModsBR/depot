//! Characterization of the offscreen run (`depot --offscreen`), on content the test writes
//! itself: a one-tile map with one box, a texture and nothing else.
//!
//! What runs where:
//!
//! * Always (no graphics adapter needed): the synthetic root reads back through the map
//!   crate as written, and the one known difference between the window and the offscreen
//!   run is pinned (see `window_and_offscreen_differ_in_the_rail_given_to_signal_aspects`).
//! * Only when `OMSI_GOLDEN_RENDER=1` (or `OMSI_GOLDEN_BLESS=1`) is set and a graphics
//!   adapter exists (a software one is enough): the real `depot` binary renders the
//!   synthetic root twice. CI runs it on the software Vulkan adapter (lavapipe, job
//!   `offscreen-golden`); the refactoring plan also runs it on the maintainer's machine,
//!   before and after each step. The two pictures must be byte-identical, must show
//!   the box, and, when a golden file for this adapter exists, must match it within a
//!   tolerance. Without an adapter these tests print `SKIP` and pass.
//!
//! Regenerating the golden: on the machine whose pictures should be the reference run
//!
//! ```text
//! OMSI_GOLDEN_RENDER=1 cargo test -p omsi-app --test offscreen_golden -- --nocapture
//! OMSI_GOLDEN_BLESS=1 cargo test -p omsi-app --test offscreen_golden -- --nocapture
//! ```
//!
//! and commit the file it writes in `tests/golden/offscreen_synthetic/`. Pictures differ
//! between drivers, so there is one file per adapter, named after the adapter's backend and
//! name (`vulkan-llvmpipe-llvm-20-1-2-256-bits.txt`), and each holds that name and backend
//! and a 16x9 thumbnail of the picture. The thumbnail is compared only with the file of the
//! running adapter; without one that comparison is skipped (the structural checks still
//! run), unless `OMSI_GOLDEN_REQUIRE=1` is set, as on CI: then a missing file fails the test
//! and the failure prints the file to commit, so a new driver on the runner is noticed
//! instead of skipped (a missing adapter fails too). `OMSI_GOLDEN_TOLERANCE` (mean absolute
//! difference per channel, 0..255, default 6) widens or narrows the comparison.

use std::path::{Path, PathBuf};
use std::process::Command;

const SIZE: (u32, u32) = (320, 180);
const THUMB: (u32, u32) = (16, 9);
const GOLDEN_DIR: &str = "tests/golden/offscreen_synthetic";

struct Root(PathBuf);

impl Root {
    /// A minimal OMSI-style root: `global.cfg`, one tile with one object, the object's
    /// description, its mesh and its texture.
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "depot-golden-offscreen-{}-{}",
            std::process::id(),
            std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        // the stock files the start-up check asks for (`omsi_cfg::ORIGINAL_ESSENTIALS`), as empty
        // placeholders: the run only checks that they exist
        for rel in omsi_cfg::ORIGINAL_ESSENTIALS {
            let path = dir.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            if rel.contains('.') {
                std::fs::write(&path, "").unwrap();
            } else {
                std::fs::create_dir_all(&path).unwrap();
            }
        }
        let map_dir = dir.join("maps").join("Golden");
        std::fs::create_dir_all(&map_dir).unwrap();
        // the object where OMSI keeps one: the map names it from the root, its mesh lies in
        // `model/` and its texture in `texture/` next to the description
        let sco_dir = dir.join("Sceneryobjects").join("Golden");
        for sub in ["model", "texture"] {
            std::fs::create_dir_all(sco_dir.join(sub)).unwrap();
        }
        let w = |name: &str, text: &str| std::fs::write(map_dir.join(name), text).unwrap();
        let o = |name: &str, text: &str| std::fs::write(sco_dir.join(name), text).unwrap();
        w("global.cfg", "[name]\nGolden offscreen\n\n[map]\n0\n0\ntile_0_0.map\n");
        // one object 20 m north of the camera, at the tile's origin height
        w("tile_0_0.map", "[version]\n14\n\n[object]\n0\nSceneryobjects\\Golden\\box.sco\n7\n10\n20\n0\n0\n0\n0\n0\n");
        o("box.sco", "[groups]\n1\nGolden\n[friendlyname]\nBox\n[mesh]\nbox.x\n");
        // a 4 m cube, x; up; depth
        let v = [
            (-2, 0, -2), (2, 0, -2), (2, 0, 2), (-2, 0, 2),
            (-2, 4, -2), (2, 4, -2), (2, 4, 2), (-2, 4, 2),
        ];
        let quads = [[0, 1, 2, 3], [4, 7, 6, 5], [0, 4, 5, 1], [1, 5, 6, 2], [2, 6, 7, 3], [3, 7, 4, 0]];
        let verts: Vec<String> = v.iter().map(|(x, y, z)| format!("{x};{y};{z};")).collect();
        let faces: Vec<String> = quads
            .iter()
            .flat_map(|q| [format!("3;{},{},{};", q[0], q[1], q[2]), format!("3;{},{},{};", q[0], q[2], q[3])])
            .collect();
        let uvs: Vec<String> = (0..8).map(|i| format!("{};{};", i % 2, (i / 2) % 2)).collect();
        o(
            "model/box.x",
            &format!(
                "xof 0303txt 0032\nMesh box {{\n{};\n{};\n{};\n{};\nMeshTextureCoords {{{};\n{};}}\n\
                 MeshMaterialList {{1;{};{};;Material {{1;0.2;0.2;1;;0;0;0;0;;0;0;0;; TextureFilename {{\"box.png\";}} }}}}\n}}\n",
                v.len(),
                verts.join(",\n"),
                faces.len(),
                faces.join(",\n"),
                uvs.len(),
                uvs.join(","),
                faces.len(),
                vec!["0"; faces.len()].join(","),
            ),
        );
        image::save_buffer(sco_dir.join("texture").join("box.png"), &[200, 40, 40, 255].repeat(16), 4, 4, image::ColorType::Rgba8).unwrap();
        Root(dir)
    }

    /// Render `name` with the real binary; the picture on success.
    fn render(&self, name: &str) -> Result<image::RgbaImage, String> {
        let out = self.0.join(name);
        let home = self.0.join("home");
        std::fs::create_dir_all(&home).unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_depot"))
            .arg("--root")
            .arg(&self.0)
            .args(["--map", "maps/Golden/global.cfg", "--offscreen"])
            .arg(&out)
            .args(["--size", &format!("{}x{}", SIZE.0, SIZE.1), "--cam", "10,0,2,0,0", "--time", "12:00"])
            // settings, the remembered root and the content folder stay out of the user's home
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

/// The adapter the offscreen run would find, if any: (name, backend). Software adapters
/// count; the no-op backend does not.
fn adapter() -> Option<(String, String)> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    pollster::block_on(instance.enumerate_adapters(wgpu::Backends::all()))
        .into_iter()
        .map(|a| a.get_info())
        .find(|i| i.backend != wgpu::Backend::Noop)
        .map(|i| (i.name, format!("{:?}", i.backend)))
}

/// Mean colour of each cell of a `THUMB` grid, rows then columns, as RGB bytes.
fn thumbnail(img: &image::RgbaImage) -> Vec<u8> {
    let (cw, ch) = (img.width() / THUMB.0, img.height() / THUMB.1);
    let mut out = Vec::new();
    for ty in 0..THUMB.1 {
        for tx in 0..THUMB.0 {
            let mut sum = [0u64; 3];
            for y in ty * ch..(ty + 1) * ch {
                for x in tx * cw..(tx + 1) * cw {
                    let p = img.get_pixel(x, y).0;
                    for c in 0..3 {
                        sum[c] += p[c] as u64;
                    }
                }
            }
            out.extend(sum.iter().map(|s| (s / (cw * ch) as u64) as u8));
        }
    }
    out
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn unhex(s: &str) -> Vec<u8> {
    (0..s.len() / 2).map(|i| u8::from_str_radix(&s[2 * i..2 * i + 2], 16).unwrap()).collect()
}

/// The golden file of one adapter: backend and name, lower case, every run of other
/// characters turned into one `-`.
fn golden_path(name: &str, backend: &str) -> PathBuf {
    let mut slug = String::new();
    for c in format!("{backend} {name}").to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    Path::new(env!("CARGO_MANIFEST_DIR")).join(GOLDEN_DIR).join(format!("{}.txt", slug.trim_end_matches('-')))
}

#[test]
fn synthetic_root_reads_back_as_written() {
    let root = Root::new();
    let global = omsi_map::GlobalCfg::load(&root.0.join("maps/Golden/global.cfg")).unwrap();
    assert_eq!(global.name, "Golden offscreen");
    assert_eq!(global.tiles.len(), 1);
    assert_eq!((global.tiles[0].x, global.tiles[0].y, global.tiles[0].file.as_str()), (0, 0, "tile_0_0.map"));
    let tile = omsi_map::Tile::load(&root.0.join("maps/Golden/tile_0_0.map")).unwrap();
    assert_eq!(tile.objects.len(), 1);
    let o = &tile.objects[0];
    assert_eq!((o.file.as_str(), o.id, o.pos), ("Sceneryobjects\\Golden\\box.sco", 7, [10.0, 20.0, 0.0]));
}

#[test]
fn offscreen_picture_is_stable_and_shows_the_box() {
    if std::env::var_os("OMSI_GOLDEN_RENDER").is_none() && std::env::var_os("OMSI_GOLDEN_BLESS").is_none() {
        eprintln!("SKIP: set OMSI_GOLDEN_RENDER=1 to render the synthetic root (needs a graphics adapter)");
        return;
    }
    let Some((name, backend)) = adapter() else {
        assert!(std::env::var_os("OMSI_GOLDEN_REQUIRE").is_none(), "OMSI_GOLDEN_REQUIRE is set but there is no graphics adapter");
        eprintln!("SKIP: no graphics adapter here; the offscreen picture cannot be rendered");
        return;
    };
    let root = Root::new();
    let a = root.render("a.png").unwrap_or_else(|e| panic!("offscreen run on {name} ({backend}) failed: {e}"));
    let b = root.render("b.png").unwrap();
    assert_eq!(a.dimensions(), SIZE);
    assert_eq!(a.as_raw(), b.as_raw(), "two offscreen runs of the same root differ");
    // the box stands in the middle of the picture; the corner shows only what is behind it
    let (centre, corner) = (a.get_pixel(SIZE.0 / 2, SIZE.1 / 2).0, a.get_pixel(2, 2).0);
    assert_ne!(centre[..3], corner[..3], "the box is not in the picture");

    let thumb = thumbnail(&a);
    let path = golden_path(&name, &backend);
    let golden = format!("adapter={name}\nbackend={backend}\nthumb={}\n", hex(&thumb));
    if std::env::var_os("OMSI_GOLDEN_BLESS").is_some() {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, &golden).unwrap();
        eprintln!("golden written to {}", path.display());
        return;
    }
    let shown = path.strip_prefix(env!("CARGO_MANIFEST_DIR")).unwrap_or(&path).display().to_string();
    let Ok(text) = std::fs::read_to_string(&path) else {
        if std::env::var_os("OMSI_GOLDEN_REQUIRE").is_some() {
            panic!("no golden for {name} ({backend}); if this picture is right, commit {shown} as:\n{golden}");
        }
        eprintln!("SKIP golden comparison: {shown} does not exist (OMSI_GOLDEN_BLESS=1 writes it)");
        return;
    };
    let field = |k: &str| text.lines().find_map(|l| l.strip_prefix(&format!("{k}="))).unwrap_or_default().to_string();
    assert_eq!((field("adapter"), field("backend")), (name.clone(), backend.clone()), "{shown} was recorded on another adapter");
    let want = unhex(&field("thumb"));
    assert_eq!(want.len(), thumb.len(), "golden has another thumbnail size");
    let tolerance: f64 = std::env::var("OMSI_GOLDEN_TOLERANCE").ok().and_then(|v| v.parse().ok()).unwrap_or(6.0);
    let mean = want.iter().zip(&thumb).map(|(w, g)| (*w as f64 - *g as f64).abs()).sum::<f64>() / want.len() as f64;
    assert!(mean <= tolerance, "picture differs from the golden: mean difference {mean:.2} > {tolerance}");
}

/// The window hands the rail the player drives on to `Traffic::signal_aspects`, so a train
/// signal clears for a player's train; the offscreen run hands it `None`, so the same signal
/// stays at stop there for a scripted player's train (only AI trains clear it). The two
/// are kept apart on purpose until the offscreen run is changed deliberately: this test
/// reads both call sites and fails when either changes, so whoever unifies them also
/// updates this note and the golden.
#[test]
fn window_and_offscreen_differ_in_the_rail_given_to_signal_aspects() {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let offscreen = std::fs::read_to_string(src.join("offscreen.rs")).unwrap();
    let window = std::fs::read_to_string(src.join("app_events.rs")).unwrap();
    let calls = |s: &str| s.matches(".signal_aspects(").count();
    assert_eq!(calls(&offscreen), 1);
    assert!(offscreen.contains("signal_aspects(&world.signal_routes, None)"), "offscreen no longer passes no rail");
    assert_eq!(calls(&window), 1);
    assert!(window.contains("signal_aspects(&w.signal_routes, rail)"), "the window no longer passes the player's rail");
    assert!(window.contains("p.rail.as_ref()).map(|r| (r.lane, r.along))"), "the window's rail is no longer the player's");
}
