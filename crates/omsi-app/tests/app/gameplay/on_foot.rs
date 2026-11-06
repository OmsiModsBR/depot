use glam::{DVec2, DVec3};
use openomsi_game::testing::humans::BusId;
use openomsi_game::testing::on_foot::*;

fn walker() -> OnFoot {
    OnFoot {
        pos: DVec3::ZERO,
        heading: 0.0,
        vel: DVec2::ZERO,
        lift: 0.0,
        vz: 0.0,
        seat: None,
        inside: None,
        cam: FootCam::First,
        yaw: 0.0,
        pitch: 0.0,
        eye: None,
        eye_yaw: 0.0,
        lag: DVec3::ZERO,
        settle: 0.0,
        view_before: "driver".into(),
        kind: 0,
        face_seat: false,
        transit: None,
        arrive: None,
        kneel: false,
        crouch: 0.0,
    }
}

/// C kneels: the eyes go down to about a metre over a moment (a picture from below,
/// #1148), the walk becomes a shuffle, and C again stands up; not on a seat.
#[test]
fn kneeling_lowers_the_eyes_and_slows_the_walk() {
    let mut f = walker();
    assert_eq!(f.eye_drop(), DVec3::ZERO);
    assert_eq!(f.pace(false), WALK);
    assert!(f.toggle_kneel() && f.kneel);
    f.ease_crouch(0.05);
    assert!(f.crouch > 0.2 && f.crouch < 0.9, "{}", f.crouch);
    for _ in 0..60 {
        f.ease_crouch(1.0 / 60.0);
    }
    assert_eq!(f.crouch, 1.0);
    assert_eq!(f.eye_drop().z, -KNEEL_DROP);
    assert_eq!((f.pace(false), f.pace(true)), (KNEEL_WALK, KNEEL_WALK));
    assert!(f.toggle_kneel() && !f.kneel);
    for _ in 0..60 {
        f.ease_crouch(1.0 / 60.0);
    }
    assert_eq!(f.eye_drop(), DVec3::ZERO);
    assert_eq!(f.pace(true), RUN);
    // (seated: nothing to kneel on)
    f.seat = Some((BusId::Player, 3));
    assert!(!f.toggle_kneel() && !f.kneel);
}
