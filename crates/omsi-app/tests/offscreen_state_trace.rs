//! The offscreen run's state, frame by frame (B0 of the refactoring plan), on content the
//! test writes itself: a 210 m street of two lanes with a crossing in the middle whose two
//! lights run a program, six AI cars on it and a 12 m bus rolling north from 30 km/h.
//!
//! `depot --offscreen` with `OMSI_STATE_TRACE` writes, for every frame, where the bus is and
//! how fast it goes, every AI car, the lights, the rail signals and the player's duty (see
//! `src/state_trace.rs`). `scripts/offscreen-trace-diff.sh` runs this test on two commits
//! and compares the traces: a change that only moves code leaves them identical, to the bit.
//! The root has no timetable, so the duty line reads `-`; a run on real content with
//! `--schedule --line` records it.
//!
//! The run needs a graphics adapter (a software one is enough), so it only happens when
//! `OMSI_STATE_TRACE_DIR` is set: the trace goes to `<dir>/synthetic.trace`, after the run
//! has been repeated and both runs agree. Without it the test prints `SKIP` and passes; the
//! root's read-back always runs.

use std::path::PathBuf;
use std::process::Command;

/// Seconds the bus rolls (30 frames a second).
const DRIVE: u32 = 20;
const AI_CARS: u32 = 6;

struct Root(PathBuf);

impl Root {
    fn new() -> Self {
        let dir = std::env::temp_dir().join(format!(
            "depot-state-trace-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let root = Root(dir);
        let w = |rel: &str, text: &str| {
            let path = root.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        };
        // the stock files the start-up check asks for (`omsi_cfg::ORIGINAL_ESSENTIALS`), as empty
        // placeholders: the run only checks that they exist
        for rel in omsi_cfg::ORIGINAL_ESSENTIALS {
            if rel.contains('.') {
                w(rel, "");
            } else {
                std::fs::create_dir_all(root.0.join(rel)).unwrap();
            }
        }
        let png = |rel: &str, rgba: [u8; 4]| {
            let path = root.0.join(rel);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            image::save_buffer(path, &rgba.repeat(16), 4, 4, image::ColorType::Rgba8).unwrap();
        };
        // the street: 8 m of asphalt, a lane each way 1.75 m off the middle
        w(
            "Splines/Trace/road.sli",
            &format!(
                "[texture]\nroad.png\n\n[profile]\n0\n\n[profilepnt]\n{}[profilepnt]\n{}[path]\n{}[path]\n{}",
                lines(&["-4", "0", "0", "1"]),
                lines(&["4", "0", "1", "1"]),
                lines(&["0", "-1.75", "0", "3.5", "0"]),
                lines(&["0", "1.75", "0", "3.5", "1"]),
            ),
        );
        png("Splines/Trace/texture/road.png", [60, 60, 60, 255]);
        // the bus and the AI car: the 12 m solo bus of the physics traces (`physics_traces.rs`,
        // `bus()`), the car the same at 0.4 of its size
        vehicle(&w, &png, "TraceBus", "trace.bus", 1.0);
        vehicle(&w, &png, "TraceCar", "car.ovh", 0.4);
        // the crossing, 10 m long between the two halves of the street: light 0 for the lane
        // north (red 15 s, green 12 s, yellow 3 s), light 1 for the lane south the other
        // way round
        let phase = |state: &str, secs: &str| format!("[phase]\n{}", lines(&[state, secs]));
        w(
            "Sceneryobjects/Trace/crossing.sco",
            &format!(
                "[groups]\n1\nTrace\n\n[friendlyname]\nCrossing\n\n[trafficlight]\n\n\
                 [traffic_light]\nNorth\n\n{}{}{}[traffic_light]\nSouth\n\n{}{}{}\
                 [path]\n{}[use_traffic_light]\n0\n\n[path]\n{}[use_traffic_light]\n1\n",
                phase("0", "15"),
                phase("6", "12"),
                phase("10", "3"),
                phase("6", "12"),
                phase("10", "3"),
                phase("0", "15"),
                lines(&["-1.75", "0", "0", "0", "0", "10", "0", "0", "0", "3.5", "0", "0"]),
                lines(&["1.75", "0", "0", "0", "0", "10", "0", "0", "0", "3.5", "1", "0"]),
            ),
        );
        // the map: the street north from (150, 50) to (150, 260) in the middle of tile 0, 0, the
        // crossing at y 150..160, the bus's entry point on the lane north at y 70 (an object id
        // no object has: the recorded place counts)
        w(
            "maps/Trace/global.cfg",
            &format!(
                "[name]\nState trace\n\n[entrypoints]\n{}[map]\n0\n0\ntile_0_0.map\n",
                lines(&[
                    "1", "0", "99", "0", "148.25", "0", "70", "0", "0", "0", "1", "0", "Start"
                ])
            ),
        );
        let spline = |id: &str, y: &str| {
            format!(
                "[spline]\n{}",
                lines(&[
                    "0",
                    r"Splines\Trace\road.sli",
                    id,
                    "0",
                    "0",
                    "150",
                    "0",
                    y,
                    "0",
                    "100",
                    "0",
                    "0",
                    "0",
                    "0",
                    "0",
                    "0",
                    "0",
                    "0"
                ])
            )
        };
        w(
            "maps/Trace/tile_0_0.map",
            &format!(
                "[version]\n14\n\n{}{}[object]\n{}",
                spline("1", "50"),
                spline("2", "160"),
                lines(&[
                    "0",
                    r"Sceneryobjects\Trace\crossing.sco",
                    "3",
                    "150",
                    "150",
                    "0",
                    "0",
                    "0",
                    "0",
                    "0"
                ])
            ),
        );
        w(
            "maps/Trace/ailists.cfg",
            "[aigroup_2]\nCars\n\nVehicles\\TraceCar\\car.ovh\t1\n[end]\n",
        );
        root
    }

    /// One run of the real binary with the state trace on; the trace.
    fn trace(&self, name: &str) -> Result<String, String> {
        let out = self.0.join(name);
        // a fresh home each run: settings and the personnel file start the same
        let home = self.0.join(format!("home-{name}"));
        std::fs::create_dir_all(&home).unwrap();
        let run = Command::new(env!("CARGO_BIN_EXE_depot"))
            .arg("--root")
            .arg(&self.0)
            .args([
                "--map",
                "maps/Trace/global.cfg",
                "--bus",
                r"Vehicles\TraceBus\trace.bus",
                "--offscreen",
            ])
            .arg(self.0.join(format!("{name}.png")))
            .args(["--size", "320x180", "--view", "outside", "--time", "12:00"])
            .args([
                "--drive",
                &DRIVE.to_string(),
                "--traffic",
                &AI_CARS.to_string(),
            ])
            .env("OMSI_STATE_TRACE", &out)
            // the bus has no engine: it rolls on from 30 km/h
            .env("OMSI_DRIVE_V0", "30")
            .env("OMSI_SEED", "1")
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
        std::fs::read_to_string(&out).map_err(|e| format!("no trace at {}: {e}", out.display()))
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

/// Each value on a line of its own, and a blank line after the block.
fn lines(values: &[&str]) -> String {
    let mut s = values.join("\n");
    s.push_str("\n\n");
    s
}

/// A vehicle on two axles with a box for a body, `scale` times the 12 m bus.
fn vehicle(w: &dyn Fn(&str, &str), png: &dyn Fn(&str, [u8; 4]), dir: &str, file: &str, scale: f32) {
    let axle = |long: f32, spring: u32, max_force: u32, driven: u32| {
        format!(
            "[newachse]\nachse_long\n{long}\nachse_maxwidth\n2.4\nachse_minwidth\n1.76\nachse_raddurchmesser\n0.94\n\
             achse_feder\n{spring}\nachse_maxforce\n{max_force}\nachse_daempfer\n20\nachse_antrieb\n{driven}\n\n"
        )
    };
    w(
        &format!("Vehicles/{dir}/{file}"),
        &format!(
            "[friendlyname]\nTrace\n{dir}\nDefault\n\n[model]\nmodel\\model.cfg\n\n[mass]\n{}\n\n[momentofintertia]\n{}\n{}\n{}\n\n\
             [schwerpunkt]\n1.2\n\n[rollwiderstand]\n{}\n\n[rot_pnt_long]\n{}\n\n[inv_min_turnradius]\n0.13\n\n\
             [boundingbox]\n2.472\n{}\n2.491\n0\n0.001\n1.622\n\n{}{}",
            10.2 * scale,
            600.0 * scale,
            90.0 * scale,
            400.0 * scale,
            1000.0 * scale,
            -2.7 * scale,
            11.663 * scale,
            axle(3.238 * scale, 240, 90, 0),
            axle(-2.637 * scale, 280, 116, 1),
        ),
    );
    w(
        &format!("Vehicles/{dir}/model/model.cfg"),
        "[mesh]\nbody.x\n",
    );
    let (x, z) = (1.2, 6.0 * scale);
    let v = [
        (-x, 0.0, -z),
        (x, 0.0, -z),
        (x, 0.0, z),
        (-x, 0.0, z),
        (-x, 3.0, -z),
        (x, 3.0, -z),
        (x, 3.0, z),
        (-x, 3.0, z),
    ];
    let quads = [
        [0, 1, 2, 3],
        [4, 7, 6, 5],
        [0, 4, 5, 1],
        [1, 5, 6, 2],
        [2, 6, 7, 3],
        [3, 7, 4, 0],
    ];
    let verts: Vec<String> = v.iter().map(|(x, y, z)| format!("{x};{y};{z};")).collect();
    let faces: Vec<String> = quads
        .iter()
        .flat_map(|q| {
            [
                format!("3;{},{},{};", q[0], q[1], q[2]),
                format!("3;{},{},{};", q[0], q[2], q[3]),
            ]
        })
        .collect();
    let uvs: Vec<String> = (0..8)
        .map(|i| format!("{};{};", i % 2, (i / 2) % 2))
        .collect();
    w(
        &format!("Vehicles/{dir}/model/body.x"),
        &format!(
            "xof 0303txt 0032\nMesh body {{\n{};\n{};\n{};\n{};\nMeshTextureCoords {{{};\n{};}}\n\
             MeshMaterialList {{1;{};{};;Material {{1;1;1;1;;0;0;0;0;;0;0;0;; TextureFilename {{\"body.png\";}} }}}}\n}}\n",
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
    png(
        &format!("Vehicles/{dir}/texture/body.png"),
        [200, 200, 40, 255],
    );
}

#[test]
fn synthetic_root_reads_back_as_written() {
    let root = Root::new();
    let global = omsi_map::GlobalCfg::load(&root.0.join("maps/Trace/global.cfg")).unwrap();
    assert_eq!(global.entry_points.len(), 1);
    assert_eq!(global.entry_points[0].pos, [148.25, 70.0, 0.0]);
    let tile = omsi_map::Tile::load(&root.0.join("maps/Trace/tile_0_0.map")).unwrap();
    assert_eq!(tile.splines.len(), 2);
    assert_eq!(tile.objects.len(), 1);
    let crossing =
        omsi_scenery::SceneryObject::load(&root.0.join("Sceneryobjects/Trace/crossing.sco"))
            .unwrap();
    assert_eq!(crossing.traffic_lights.len(), 2);
    assert_eq!(crossing.paths.len(), 2);
    let road = omsi_scenery::Spline::load(&root.0.join("Splines/Trace/road.sli")).unwrap();
    assert_eq!(road.paths.len(), 2);
    let bus = omsi_vehicle::Vehicle::load(&root.0.join("Vehicles/TraceBus/trace.bus")).unwrap();
    assert_eq!((bus.mass, bus.axles.len()), (10.2, 2));
}

#[test]
fn offscreen_state_trace_is_repeatable_and_shows_the_street() {
    let Some(dir) = std::env::var_os("OMSI_STATE_TRACE_DIR").map(PathBuf::from) else {
        eprintln!("SKIP: set OMSI_STATE_TRACE_DIR to run the synthetic street and write its trace (needs a graphics adapter)");
        return;
    };
    let root = Root::new();
    let a = root
        .trace("a.trace")
        .unwrap_or_else(|e| panic!("offscreen run failed: {e}"));
    let b = root.trace("b.trace").unwrap();
    if a != b {
        let line = a
            .lines()
            .zip(b.lines())
            .position(|(x, y)| x != y)
            .unwrap_or(0);
        panic!(
            "two runs of the same root differ, first at line {}:\n- {}\n+ {}",
            line + 1,
            a.lines().nth(line).unwrap_or(""),
            b.lines().nth(line).unwrap_or("")
        );
    }
    // the street is not empty: every frame, the bus, the cars and both lights
    let frames = a.lines().filter(|l| l.starts_with("frame ")).count();
    // (`--drive` counts its frames as the seconds over 1/30 s, rounded down)
    assert!(
        (DRIVE * 30..=DRIVE * 30 + 1).contains(&(frames as u32)),
        "{frames} blocks: not one per frame and the last state"
    );
    assert!(
        a.lines()
            .filter(|l| l.starts_with("  cars "))
            .all(|l| l != "  cars 0"),
        "a frame without AI cars"
    );
    let lights: std::collections::HashSet<&str> =
        a.lines().filter(|l| l.starts_with("  lights 0:")).collect();
    assert!(
        lights.len() >= 3,
        "the lights did not go through their program: {lights:?}"
    );
    let y = |l: &str| {
        l.split_whitespace()
            .nth(3)
            .and_then(|v| v.parse::<f64>().ok())
    };
    let players: Vec<f64> = a
        .lines()
        .filter(|l| l.starts_with("  player p "))
        .filter_map(y)
        .collect();
    assert!(
        players.last().unwrap() - players[0] > 20.0,
        "the bus did not roll north: {:?} .. {:?}",
        players.first(),
        players.last()
    );
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("synthetic.trace"), a).unwrap();
}
