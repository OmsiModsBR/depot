//! What lies under the cursor in real cockpits, checked against a recording: a grid of rays
//! from each driver's camera, with the switch, the part and the hand cursor each one finds.
//! A faster pick must find exactly the same, and the pick through the meshes' trees the same
//! as the one through every triangle, ray by ray. It needs installed content, so it runs only
//! with `OMSI_ROOT` and `OMSI_HOVER_GOLDEN` (a file outside the repository): the first run
//! writes the recording, every later run compares with it. `OMSI_HOVER_BUSES` (paths under
//! `OMSI_ROOT`, separated by `;`) replaces the buses tried.

use openomsi_game::testing::player::hovered_part_split;
use openomsi_game::testing::spawn::load_coupled_parts;
use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::sync::Arc;

/// Buses with many switches, a VDV dashboard and articulated rear sections.
const BUSES: &[&str] = &[
    "Vehicles/MAN_NL_NG/MAN_EN92_main.bus",
    "Vehicles/MAN_NL_NG/MAN_GN92_main.bus",
    "Vehicles/Caio TopbusPB by Zane/_TopbusPB_A.bus",
];

/// The rings' half-angle the window passes: six pixels of a 60-degree view 1440 pixels high.
const SPREAD: f32 = 2.0 * 0.577_350_3 / 1440.0 * 6.0;

fn record(root: &Path, rel: &str, out: &mut String) {
    let path = root.join(rel);
    if !path.exists() {
        eprintln!("skipped: no {}", path.display());
        return;
    }
    let ty = Arc::new(omsi_sim::VehicleType::load(root, &path).expect(rel));
    let mut vehicle =
        omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()));
    load_coupled_parts(root, &mut vehicle);
    vehicle.host.clock.timegap = 1.0 / 60.0;
    vehicle.update(1.0 / 60.0);
    // every triangle first, then through the meshes' trees: the same, ray by ray
    let full = looks(rel, &vehicle);
    vehicle.ty.build_trees();
    for t in &vehicle.trailers {
        t.ty.build_trees();
    }
    let started = std::time::Instant::now();
    let through_trees = looks(rel, &vehicle);
    eprintln!(
        "{rel}: {} looks through the trees in {:.0} ms",
        through_trees.len(),
        started.elapsed().as_secs_f64() * 1000.0
    );
    for (a, b) in full.iter().zip(&through_trees) {
        assert_eq!(a, b, "{rel}: the trees found something else");
    }
    for (line, found, hand) in through_trees {
        if found.is_some() || hand {
            writeln!(out, "{line}\t{found:?}\t{hand}").unwrap();
        }
    }
}

/// The grid of rays from the first two driver cameras: (where, what is found, hand).
#[allow(clippy::type_complexity)]
fn looks(
    rel: &str,
    vehicle: &omsi_sim::VehicleInstance,
) -> Vec<(String, Option<(String, bool)>, bool)> {
    let mut all = Vec::new();
    let cameras = vehicle.ty.def.cameras_driver.clone();
    for (c, cam) in cameras.iter().enumerate().take(2) {
        let (eye, _, _) = vehicle.camera_world(cam);
        let rot = vehicle.body_rotation();
        for pitch in (-50..=20).step_by(2) {
            for yaw in (-80..=80).step_by(2) {
                let (sy, cy) = (cam.yaw + yaw as f32).to_radians().sin_cos();
                let (sp, cp) = (cam.pitch + pitch as f32).to_radians().sin_cos();
                let dir = rot
                    .transform_vector3(glam::Vec3::new(sy * cp, cy * cp, sp))
                    .normalize();
                let ((found, hand), _) = hovered_part_split(vehicle, eye, dir, SPREAD, true, true);
                // in a drive no part that is no switch is named, and nothing else changes
                let drive = hovered_part_split(vehicle, eye, dir, SPREAD, true, false).0;
                let unnamed = found.clone().filter(|f| f.1);
                assert_eq!(drive, (unnamed, hand), "{rel} camera {c} at {yaw},{pitch}");
                all.push((format!("{rel}\t{c}\t{yaw}\t{pitch}"), found, hand));
            }
        }
    }
    all
}

#[test]
fn the_cursor_finds_what_it_found_before_in_real_cockpits() {
    let (Some(root), Some(golden)) = (
        omsi_cfg::flags::ROOT.os().map(PathBuf::from),
        std::env::var_os("OMSI_HOVER_GOLDEN").map(PathBuf::from),
    ) else {
        eprintln!("skipped: OMSI_ROOT and OMSI_HOVER_GOLDEN are not both set");
        return;
    };
    let buses: Vec<String> = match std::env::var("OMSI_HOVER_BUSES") {
        Ok(list) => list.split(';').map(|s| s.trim().to_string()).collect(),
        Err(_) => BUSES.iter().map(|s| s.to_string()).collect(),
    };
    let mut out = String::new();
    for bus in &buses {
        record(&root, bus, &mut out);
    }
    assert!(!out.is_empty(), "no bus found under {}", root.display());
    match std::fs::read_to_string(&golden) {
        Ok(before) => {
            let (before, now): (Vec<&str>, Vec<&str>) =
                (before.lines().collect(), out.lines().collect());
            let changed: Vec<String> = before
                .iter()
                .zip(&now)
                .filter(|(a, b)| a != b)
                .take(10)
                .map(|(a, b)| format!("  was {a}\n  now {b}"))
                .collect();
            assert!(
                changed.is_empty() && before.len() == now.len(),
                "{} lines before, {} now; first differences:\n{}",
                before.len(),
                now.len(),
                changed.join("\n")
            );
        }
        Err(_) => {
            std::fs::write(&golden, &out).unwrap();
            eprintln!(
                "recorded {} hits to {}",
                out.lines().count(),
                golden.display()
            );
        }
    }
}
