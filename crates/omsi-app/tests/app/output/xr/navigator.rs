use glam::DVec3;
use glam::Mat4;
use glam::Vec3;
use openomsi_game::testing::output::xr::navigator::*;
use std::path::Path;

#[test]
fn menu_values_keep_visibility_and_clamp_to_placement_limits() {
    let mut p = Placement::default();
    p.set_value("x", 0.42);
    p.set_value("opacity", 0.7);
    assert_eq!(p.value("x"), Some(0.42));
    assert_eq!(p.value("opacity"), Some(0.7));
    assert!(!p.enabled);
    p.set_value("width", 20.0);
    p.set_value("tilt", -100.0);
    assert_eq!(p.width, 0.65);
    assert_eq!(p.tilt, -80.0);
    let saved = p;
    p.set_value("unknown", 1.0);
    assert_eq!(p, saved);
}

#[test]
fn navigator_translations_are_loaded_for_all_languages() {
    let keys = [
        "Navigator position (this bus)",
        "Position right / left",
        "Position forward / back",
        "Position up / down",
        "Display width",
        "Display rotation",
        "Display tilt",
        "Display roll",
        "Move and rotate with the mouse...",
        "Positioning navigator - changes apply to this bus",
        "Hold left mouse: move | Hold right mouse: rotate",
        "Wheel: distance | Ctrl+wheel: size | Shift+right drag: roll",
        "Esc / Enter: save and finish | R: reset position",
        "Reset navigator position",
        "VR: Toggle navigator",
        "VR: Position navigator",
        "Could not save navigator position",
    ];
    for &(_, _, language, _) in omsi_launcher_lib::LANGUAGES.iter().filter(|l| l.2 != "en") {
        for key in keys {
            let translated = openomsi_game::testing::translate(language, key);
            assert!(
                translated
                    .as_ref()
                    .is_some_and(|text| !text.trim().is_empty() && text.as_str() != key),
                "Missing navigator translation: {language} / {key}"
            );
        }
    }
}

#[test]
fn navigator_starts_hidden_and_reset_preserves_visibility() {
    let mut p = Placement::default();
    assert!(!p.enabled);
    p.adjust("enabled", 1.0);
    p.adjust("x", 1.0);
    p.adjust("reset", 1.0);
    assert!(p.enabled);
    assert_eq!(p.offset, Placement::default().offset);
}

#[test]
fn standard_driver_camera_distance_is_respected() {
    let camera = omsi_vehicle::Camera {
        pos: [1.0, 2.0, 3.0],
        yaw: 90.0,
        pitch: 0.0,
        dist: 0.4,
        ..Default::default()
    };
    assert!(driver_origin(&camera).abs_diff_eq(Vec3::new(0.6, 2.0, 3.0), 0.0001));
}

fn camera(position: DVec3, yaw: f32) -> omsi_render::Camera {
    omsi_render::Camera {
        position,
        yaw,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 90.0,
        near: 0.1,
        far: 1000.0,
    }
}

#[test]
fn cockpit_panel_follows_the_bus_without_drifting() {
    let display = Display {
        placement: Placement::default(),
        local_center: Vec3::new(0.3, 1.0, -0.3),
    };
    let projection = Mat4::perspective_rh(90.0_f32.to_radians(), 1.0, 1000.0, 0.1);
    let first = display.transform(
        DVec3::ZERO,
        Mat4::IDENTITY,
        &camera(DVec3::ZERO, 0.0),
        projection,
        1.2,
    );
    // Heading is clockwise, whereas the vehicle rotation uses mathematical angles.
    let position = DVec3::new(1_000_000.0, -2_000_000.0, 15.0);
    let moved = display.transform(
        position,
        Mat4::from_rotation_z(-0.7),
        &camera(position, 0.7_f32.to_degrees()),
        projection,
        1.2,
    );
    assert!(first.abs_diff_eq(moved, 0.00001));
    let looked = display.transform(
        DVec3::ZERO,
        Mat4::IDENTITY,
        &camera(DVec3::ZERO, 25.0),
        projection,
        1.2,
    );
    assert!(
        !first.abs_diff_eq(looked, 0.01),
        "the panel must not follow head rotation"
    );
}

#[test]
fn stereo_panel_keeps_aspect_and_has_parallax() {
    let placement = Placement {
        yaw: 0.0,
        tilt: 0.0,
        roll: 0.0,
        ..Placement::default()
    };
    let display = Display {
        placement,
        local_center: Vec3::Y,
    };
    let projection = Mat4::perspective_rh(90.0_f32.to_radians(), 1.0, 1000.0, 0.1);
    let left = display.transform(
        DVec3::ZERO,
        Mat4::IDENTITY,
        &camera(DVec3::new(-0.032, 0.0, 0.0), 0.0),
        projection,
        1.4,
    );
    let right = display.transform(
        DVec3::ZERO,
        Mat4::IDENTITY,
        &camera(DVec3::new(0.032, 0.0, 0.0), 0.0),
        projection,
        1.4,
    );
    assert!(left.project_point3(Vec3::ZERO).x > right.project_point3(Vec3::ZERO).x);
    let width = (left.project_point3(Vec3::X) - left.project_point3(-Vec3::X)).length();
    let height = (left.project_point3(Vec3::Y) - left.project_point3(-Vec3::Y)).length();
    assert!((width / height - 1.4).abs() < 0.0001);
}

#[test]
fn wheel_changes_depth_and_ctrl_wheel_only_changes_size() {
    let mut p = Placement::default();
    let original = p;
    let direction = Vec3::from(p.offset).normalize();
    p.scroll(2.0, false, Vec3::ZERO, Vec3::ZERO);
    assert!(
        (Vec3::from(original.offset).length() - Vec3::from(p.offset).length() - 0.08).abs()
            < 0.0001
    );
    assert!(direction.abs_diff_eq(Vec3::from(p.offset).normalize(), 0.0001));
    assert_eq!(p.width, original.width);
    let moved = p;
    p.scroll(2.0, true, Vec3::ZERO, Vec3::ZERO);
    assert_eq!(p.offset, moved.offset);
    assert!(p.width > moved.width);
    p.scroll(1000.0, false, Vec3::ZERO, Vec3::ZERO);
    assert!((Vec3::from(p.offset).length() - 0.25).abs() < 0.0001);
    p.scroll(f32::NAN, true, Vec3::ZERO, Vec3::ZERO);
    assert!(p.width.is_finite());
}

#[test]
fn profiles_survive_replacing_the_file_and_stay_separate_per_bus() {
    let dir = std::env::temp_dir().join(format!(
        "depot-vr-nav-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let path = dir.join("profiles.json");
    let mut profiles = Profiles::default();
    let a = bus_key(Path::new("OMSI/Vehicles/Mod/Bus.bus"), Path::new("OMSI"));
    let b = bus_key(
        Path::new("OMSI/Vehicles/Payware/Bus.bus"),
        Path::new("OMSI"),
    );
    assert_ne!(a, b);
    let mut placement = Placement::default();
    placement.adjust("x", -1.0);
    profiles.buses.insert(a.clone(), placement);
    profiles.save_to(&path).unwrap();
    profiles.buses.insert(
        b.clone(),
        Placement {
            enabled: false,
            ..Placement::default()
        },
    );
    profiles.save_to(&path).unwrap();
    let restored: Profiles = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    assert_eq!(restored.get(&a), placement);
    assert!(!restored.get(&b).enabled);
    assert_eq!(restored.get("another bus"), Placement::default());
    std::fs::remove_file(path).unwrap();
    std::fs::remove_dir(dir).unwrap();
}
