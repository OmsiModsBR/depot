//! Steerable axles in Project Depot Physics (spec 004): the pure rules of
//! `physics::implementation::depot::steering` against `D-5` and `D-6` of
//! `docs/physics/depot-model.md`, and the conformance vehicles with steerable axles driven
//! through a turn (`D-5` to `D-7`), leading and behind a joint.

mod support;

use omsi_sim::physics::{Engine, RigidBody, SteerAxle, SteerMode, SteerState};
use omsi_sim::testing::physics::implementation::depot::steering::{
    advance, full_lock, scrub_reach, side, target_angle, turning_line,
};
use omsi_sim::testing::physics::RigidBodyInternals;
use support::model::{build, launch, step};
use support::vehicles::{
    tag_6x2_4_long, twin_steer_8x2, v2_front_declared, v2_rigid_6x2, v2_tag_steer,
    v5_articulated_8x2, v5_last_axle_steer,
};

fn axle(lock_deg: f32, centring_deg_s: f32) -> SteerAxle {
    SteerAxle {
        axle: 2,
        lock: lock_deg.to_radians(),
        centring: centring_deg_s.to_radians(),
    }
}

fn in_mode(mode: SteerMode) -> SteerState {
    SteerState {
        mode,
        ..SteerState::default()
    }
}

/// Checks: D-5
#[test]
fn one_rigid_axle_is_the_turning_line() {
    // V2: the middle axle rigid, the tag axle pointing at the centre of a turn about it, the
    // front axle left out
    let k = 0.12;
    let tag = (-3.3f32 * k).atan();
    let line = turning_line(&[(-1.9, 0.306, 0.0), (-5.2, 0.216, tag)], k, 0.0);
    assert!((line - -1.9).abs() < 1e-5, "{line}");
}

/// Checks: D-5
#[test]
fn several_rigid_axles_turn_about_their_load_weighted_middle() {
    let line = turning_line(&[(-1.9, 0.306, 0.0), (-5.2, 0.216, 0.0)], 0.12, 0.0);
    assert!((line - -3.265).abs() < 1e-3, "{line}");
    // (straight on as well: nothing points anywhere else)
    let straight = turning_line(&[(-1.9, 0.306, 0.0), (-5.2, 0.216, 0.0)], 0.0, 0.0);
    assert!((straight - line).abs() < 1e-6);
}

/// Checks: D-5 (an axle that turns less than the centre asks pulls the line its way)
#[test]
fn an_axle_counts_where_it_points() {
    // V2 at full lock with its tag axle at its 15° lock: the line where the slip balances
    let k = 0.1131;
    let line = turning_line(
        &[(-1.9, 0.306, 0.0), (-5.2, 0.216, -15f32.to_radians())],
        k,
        0.0,
    );
    let want = (0.306 * -1.9 + 0.216 * (-5.2 + 15f32.to_radians().tan() / k)) / 0.522;
    assert!((line - want).abs() < 1e-5, "{line}");
    assert!((line - -2.285).abs() < 2e-3, "{line}");
}

/// Checks: D-5
#[test]
fn nothing_to_weigh_falls_back_on_the_c6_line() {
    assert_eq!(turning_line(&[], 0.1, 1.0), 1.0);
}

/// Checks: D-5
#[test]
fn the_full_lock_follows_the_line() {
    let lock = (0.12f32 * 6.3).atan();
    assert!((full_lock(lock, 4.4, -1.9) - 0.12).abs() < 1e-5);
    assert!((full_lock(lock, 4.4, -3.265) - 0.0986).abs() < 1e-4);
    // (a line on the front axle asks no endless curvature)
    assert!(full_lock(lock, 4.4, 4.4).is_finite());
}

/// Checks: D-6
#[test]
fn the_side_of_the_line_sets_the_way_an_axle_turns() {
    assert_eq!(side(-5.2, -1.9), -1.0);
    assert_eq!(side(2.6, -2.7), 1.0);
    assert_eq!(side(-1.9, -1.9), 0.0);
}

/// Checks: D-6
#[test]
fn the_basic_mode_turns_in_proportion_to_the_lock() {
    let a = axle(15.0, 20.0);
    let basic = in_mode(SteerMode::Basic);
    let lock = 15f32.to_radians();
    assert!((target_angle(&a, &basic, -1.0, 1.0) - -lock).abs() < 1e-6);
    assert!((target_angle(&a, &basic, -1.0, 0.5) - -lock / 2.0).abs() < 1e-6);
    assert!((target_angle(&a, &basic, 1.0, -0.5) - -lock / 2.0).abs() < 1e-6);
    assert_eq!(target_angle(&a, &basic, 0.0, 1.0), 0.0);
    // (the steering's share never passes the full lock)
    assert!((target_angle(&a, &basic, 1.0, 3.0) - lock).abs() < 1e-6);
}

/// Checks: D-6
#[test]
fn locked_is_straight_and_the_script_angle_is_clamped() {
    let a = axle(15.0, 20.0);
    assert_eq!(
        target_angle(&a, &in_mode(SteerMode::Locked), -1.0, 1.0),
        0.0
    );
    let script = SteerState {
        script_angle: 1.0,
        ..in_mode(SteerMode::ScriptAngle)
    };
    assert!((target_angle(&a, &script, -1.0, 1.0) - 15f32.to_radians()).abs() < 1e-6);
}

/// Checks: D-6
#[test]
fn the_angle_moves_at_the_centring_rate() {
    let a = axle(15.0, 30.0);
    let h = 0.1;
    let s = advance(&a, in_mode(SteerMode::Basic), -1.0, 1.0, h);
    // 30°/s for 0.1 s: 3° of the 15° asked
    assert!((s.angle - -3f32.to_radians()).abs() < 1e-6);
    let locked = advance(
        &a,
        SteerState {
            mode: SteerMode::Locked,
            ..s
        },
        -1.0,
        1.0,
        h,
    );
    // back towards straight at the same rate
    assert!(locked.angle.abs() < 1e-6);
}

/// Checks: D-7
#[test]
fn a_tyre_takes_more_grip_the_further_it_points_off_the_centre() {
    assert_eq!(scrub_reach(0.0), 0.0);
    assert!((scrub_reach(2.5f32.to_radians()) - 0.5).abs() < 1e-5);
    assert_eq!(scrub_reach(-5f32.to_radians()), 1.0);
    assert_eq!(scrub_reach(20f32.to_radians()), 1.0);
}

/// The bodies of `defs` under Project Depot Physics, the first one leading.
fn depot(defs: &[omsi_vehicle::Vehicle]) -> (Vec<RigidBody>, Vec<omsi_sim::physics::Joint>) {
    let (mut bodies, joints) = build(defs, Engine::Depot);
    bodies[0].lead();
    (bodies, joints)
}

/// Lock the leading section's steerable axles straight (mode 1, reached).
fn lock_all(body: &mut RigidBody) {
    for (_, s) in body.steer_axles_mut() {
        *s = SteerState {
            angle: 0.0,
            mode: SteerMode::Locked,
            script_angle: 0.0,
        };
    }
}

/// A steady turn at 3 m/s with the steering at `steer`: the leading body after 10 s, and the
/// mean drive torque (N m) of its last 2 s.
fn steady(
    mut bodies: Vec<RigidBody>,
    joints: &[omsi_sim::physics::Joint],
    steer: f32,
) -> (RigidBody, f32) {
    let v = 3.0_f32;
    launch(&mut bodies, v);
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let radius = bodies[0]
        .wheels()
        .iter()
        .find(|w| w.driven())
        .map_or(0.5, |w| w.radius());
    let mut drive = 0.0;
    for frame in 0..600 {
        let b = &bodies[0];
        let mass: f32 = bodies.iter().map(|b| b.mass()).sum();
        let rolling: f32 = bodies.iter().map(|b| b.rolling_resistance()).sum();
        let torque = ((v - b.forward_speed()) * mass * 2.0 + rolling) * radius;
        if frame >= 480 {
            drive += torque / 120.0;
        }
        step(&mut bodies, joints, &brakes, torque, steer);
    }
    (bodies.swap_remove(0), drive)
}

/// The radius (m) the body turns on, at its turning line.
fn radius(b: &RigidBody) -> f32 {
    b.forward_speed() / b.omega().z.abs()
}

/// Checks: D-5, D-6
#[test]
fn v2_turns_tighter_with_its_tag_axle_steering() {
    let (bodies, joints) = depot(&v2_tag_steer());
    let (b, _) = steady(bodies, &joints, 1.0);
    assert!(b.holding());
    // D-5: the tag axle at its 15° lock turns less than a turn about the middle axle asks
    // (18.2°): the line where the slip balances, at -2.285 m, full lock 0.1131 1/m
    let r = radius(&b);
    assert!(
        (r - 8.843).abs() < 0.02 * 8.843,
        "radius {r} m, want 8.84 m"
    );
    assert!(
        (b.rot_pnt_long() - -2.285).abs() < 0.01,
        "line {}",
        b.rot_pnt_long()
    );
    // D-6: the tag axle at minus its lock, the middle one straight
    let tag = b.axle_steer(4);
    assert!((tag - -15f32.to_radians()).abs() < 1e-4, "tag {tag} rad");
    assert_eq!(b.axle_steer(2), 0.0);
    // D-7: both rear axles scrub a little, the middle 2.5° and the tag 3.2° off the centre
    assert!(b.hold_reach(0).is_infinite());
    // (the mean of an axle's two tyres, each square to its own line to the centre)
    for (w, want) in [(2, 0.50), (4, 0.65)] {
        let reach = (b.hold_reach(w) + b.hold_reach(w + 1)) / 2.0;
        assert!((reach - want).abs() < 0.03, "axle of wheel {w}: {reach}");
    }
}

/// Checks: D-5, D-7
#[test]
fn v2_turns_wider_with_its_tag_axle_locked() {
    let (mut bodies, joints) = depot(&v2_tag_steer());
    lock_all(&mut bodies[0]);
    let (b, _) = steady(bodies, &joints, 1.0);
    assert!(b.holding());
    let r = radius(&b);
    assert!(
        (r - 10.139).abs() < 0.02 * 10.139,
        "radius {r} m, want 10.14 m"
    );
    assert_eq!(b.axle_steer(4), 0.0);
    assert_eq!(b.axle_steer(2), 0.0);
}

/// Checks: D-6
#[test]
fn v2_tag_axle_at_half_steering_turns_half_its_lock() {
    let (bodies, joints) = depot(&v2_tag_steer());
    let (b, _) = steady(bodies, &joints, 0.5);
    let tag = b.axle_steer(4);
    assert!((tag - -7.5f32.to_radians()).abs() < 1e-4, "tag {tag} rad");
}

/// Checks: D-6
#[test]
fn a_twin_steer_axle_turns_the_same_way_as_the_front() {
    let (bodies, joints) = depot(&twin_steer_8x2());
    let (b, _) = steady(bodies, &joints, 1.0);
    let (front, second) = (b.axle_steer(0), b.axle_steer(2));
    assert!(
        front > 0.0 && second > 0.0,
        "front {front}, second {second}"
    );
    assert!((second - 12f32.to_radians()).abs() < 1e-4);
    assert_eq!(b.axle_steer(4), 0.0);
    assert_eq!(b.axle_steer(6), 0.0);
}

/// Checks: D-7
#[test]
fn rigid_axles_off_the_centre_cost_drive() {
    let (bodies, joints) = depot(&v2_tag_steer());
    let (_, steering) = steady(bodies, &joints, 1.0);
    let (mut bodies, joints) = depot(&v2_tag_steer());
    lock_all(&mut bodies[0]);
    let (_, locked) = steady(bodies, &joints, 1.0);
    assert!(
        locked > steering * 1.05,
        "drive {locked} N m with the tag axle locked, {steering} N m steering"
    );
}

/// Checks: D-5, D-7 (sliding: each tyre's force with its own angle)
#[test]
fn sliding_the_tag_axle_angle_changes_the_yaw() {
    let fast = |lock: bool| {
        let (mut bodies, joints) = depot(&v2_tag_steer());
        if lock {
            lock_all(&mut bodies[0]);
        }
        launch(&mut bodies, 14.0);
        let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
        let mut slid = false;
        for _ in 0..60 {
            step(&mut bodies, &joints, &brakes, 0.0, 1.0);
            slid |= !bodies[0].holding();
        }
        assert!(slid, "full lock at 50 km/h must let the tyres go");
        bodies[0].omega().z
    };
    let (steering, locked) = (fast(false), fast(true));
    assert!(
        (steering - locked).abs() > 1e-3,
        "yaw rate {steering} rad/s steering, {locked} rad/s locked"
    );
}

/// Checks: D-5 (the front axle is the driver's)
#[test]
fn the_parameters_on_the_front_axle_change_nothing() {
    let (bodies, joints) = depot(&v2_front_declared());
    assert!(bodies[0].steer_axles().is_empty());
    let (a, _) = steady(bodies, &joints, 1.0);
    let (plain, joints) = depot(&v2_rigid_6x2());
    let (b, _) = steady(plain, &joints, 1.0);
    assert_eq!(a.omega(), b.omega());
    assert_eq!(a.position(), b.position());
}

/// Checks: D-5 (every axle but the front one steering: the `C-6` line)
#[test]
fn every_rear_axle_steering_turns_about_the_c6_line() {
    let defs = support::vehicles::steerable(v2_tag_steer(), 0, 1, 15.0, 200.0);
    let (bodies, joints) = depot(&defs);
    assert_eq!(bodies[0].steer_axles().len(), 2);
    let (b, _) = steady(bodies, &joints, 1.0);
    // the file's line is the middle axle: the full lock of `[inv_min_turnradius]`
    let r = radius(&b);
    assert!((r - 8.333).abs() < 0.02 * 8.333, "radius {r} m");
    assert!((b.axle_steer(2)).abs() < 1e-6, "on the line: straight");
}

/// Checks: D-7 (a slight steer, the tag axle a fraction of a degree off the centre, drags no
/// more than the rigid bus: no Coulomb fight between the axles)
#[test]
fn a_slight_steer_costs_no_drive() {
    let speed = |defs: Vec<omsi_vehicle::Vehicle>| {
        let (mut bodies, joints) = depot(&defs);
        let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
        for _ in 0..900 {
            step(&mut bodies, &joints, &brakes, 9000.0, 0.05);
        }
        bodies[0].forward_speed()
    };
    let (rigid, tag) = (speed(v2_rigid_6x2()), speed(v2_tag_steer()));
    assert!(
        (tag - rigid).abs() < 0.01 * rigid,
        "after 15 s {tag} m/s with the tag axle steering, {rigid} m/s rigid"
    );
}

/// Pulling away with the steering at `steer` and 9 kN m on the driven wheels for 8 s: the
/// speed (m/s) at the end and the fastest roll (rad/s) in the first 2 s.
fn pull_away_turning(defs: &[omsi_vehicle::Vehicle], steer: f32) -> (f32, f32) {
    let (mut bodies, joints) = depot(defs);
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let mut roll = 0.0_f32;
    for frame in 0..480 {
        step(&mut bodies, &joints, &brakes, 9000.0, steer);
        if frame < 120 {
            roll = roll.max(bodies[0].omega().y.abs());
        }
    }
    (bodies[0].forward_speed(), roll)
}

/// Checks: D-5, D-7 (the tyres' slip balances: a tag axle that turns less than the centre
/// asks costs about as much drive as a rigid one, not the whole road's grip)
#[test]
fn a_steered_tag_axle_pulls_away_in_a_turn_like_a_rigid_one() {
    let (rigid, _) = pull_away_turning(&v2_rigid_6x2(), 0.5);
    let (tag, _) = pull_away_turning(&v2_tag_steer(), 0.5);
    assert!(
        tag > 0.97 * rigid,
        "after 8 s {tag} m/s with the tag axle steering, {rigid} m/s rigid"
    );
}

/// Checks: D-7 (holding, the scrub rolls the body at no speed: at walking pace it rocked
/// back as the body began to follow its wheels)
#[test]
fn a_steered_tag_axle_does_not_rock_the_body_pulling_away() {
    let (_, rigid) = pull_away_turning(&v2_rigid_6x2(), 0.7);
    let (_, tag) = pull_away_turning(&v2_tag_steer(), 0.7);
    assert!(
        tag < 3.0 * rigid.max(1e-3),
        "roll {tag} rad/s with the tag axle, {rigid} rigid"
    );
}

/// Checks: D-7 (holding with steerable axles, the bend's pull rolls the body whatever its
/// middle's speed across: with the turning line 1.4 m behind `[cog]`, that speed passed
/// `GRIPPED_ACROSS` in a turn at 40 km/h and the roll swapped from the bend's pull to the
/// tyres and back in one step, rocking the body)
#[test]
fn a_turning_line_far_behind_the_cog_does_not_rock_the_body_at_speed() {
    let (mut bodies, joints) = depot(&tag_6x2_4_long());
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let mut roll = 0.0_f32;
    for frame in 0..600 {
        step(&mut bodies, &joints, &brakes, 10000.0, 0.3);
        if frame >= 120 {
            roll = roll.max(bodies[0].omega().y.abs());
        }
    }
    assert!(bodies[0].holding());
    assert!(roll < 0.03, "roll {roll} rad/s");
}

/// V5's train after 25 s at 3 m/s with the steering at half; `lock` locks the rear section's
/// steerable axle straight first.
fn v5_at_half_steering(defs: &[omsi_vehicle::Vehicle], lock: bool) -> (Vec<RigidBody>, f32) {
    let (mut bodies, joints) = depot(defs);
    if lock {
        lock_all(&mut bodies[1]);
    }
    let v = 3.0_f32;
    launch(&mut bodies, v);
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let radius = bodies[1].wheels()[0].radius();
    for _ in 0..1500 {
        let mass: f32 = bodies.iter().map(|b| b.mass()).sum();
        let rolling: f32 = bodies.iter().map(|b| b.rolling_resistance()).sum();
        let torque = ((v - bodies[0].forward_speed()) * mass * 2.0 + rolling) * radius;
        step(&mut bodies, &joints, &brakes, torque, 0.5);
    }
    let alpha = joints[0].angles(&bodies).0;
    (bodies, alpha)
}

/// `C-9`'s steady articulation angle (rad) of V5 at half steering with the rear section
/// turning about `y_turn`: `R sin α + D cos α = d`, the ball 1.9 m behind the front
/// section's line and `d` ahead of the rear one's.
fn v5_closed_form_alpha(y_turn: f32) -> f32 {
    let (r, ball, d) = (1.0 / (0.5 * 0.12_f32), -4.4 - -2.5_f32, 5.6 - y_turn);
    (d / r.hypot(ball)).asin() - ball.atan2(r)
}

/// Checks: D-5, D-6, D-7, C-9 (SC-006a: the rear section turns about its rigid axle's line, its
/// last axle against the bend, and its articulation angle is the closed form's)
#[test]
fn v5_rear_section_turns_about_its_steered_line() {
    let (bodies, alpha) = v5_at_half_steering(&v5_last_axle_steer(), false);
    let rear = &bodies[1];
    assert!(rear.holding());
    assert!(
        (rear.rot_pnt_long() - 1.2).abs() < 0.05,
        "line {}",
        rear.rot_pnt_long()
    );
    // (21.7°)
    let want = v5_closed_form_alpha(1.2);
    assert!(
        (alpha - want).abs() < 0.02 * want,
        "alpha {} deg, want {} deg",
        alpha.to_degrees(),
        want.to_degrees()
    );
    // the fourth axle at -s_f lock, s_f the joint's curvature over the full lock (0.70)
    let s_f = alpha.sin() / (5.6 - 1.2) / 0.12;
    let fourth = rear.axle_steer(2);
    let want = -s_f * 34.85f32.to_radians();
    assert!(
        (fourth - want).abs() < 0.02 * want.abs(),
        "fourth axle {} deg, want {} deg",
        fourth.to_degrees(),
        want.to_degrees()
    );
    assert!(rear.wheels()[2].steered());
    // the rigid third axle stands square and shows 0
    assert_eq!(rear.axle_steer(0), 0.0);
    assert!(!rear.wheels()[0].steered());
}

/// The load-weighted middle (m) of V5's rear axles at rest, the joint carrying its share.
fn v5_rear_middle(rear: &RigidBody) -> f32 {
    let w = rear.wheels();
    let (a, b) = (w[0].rest_load(), w[2].rest_load());
    (a * w[0].attach().y + b * w[2].attach().y) / (a + b)
}

/// Checks: D-5, D-7, C-9 (SC-006a: locked, the rear section turns about the load-weighted
/// middle of its axles, and its articulation angle is the closed form's)
#[test]
fn v5_rear_section_with_its_last_axle_locked_turns_about_the_middle() {
    let (bodies, alpha) = v5_at_half_steering(&v5_last_axle_steer(), true);
    let rear = &bodies[1];
    assert!(rear.holding());
    let middle = v5_rear_middle(rear);
    assert!(
        (rear.rot_pnt_long() - middle).abs() < 0.05,
        "line {}, want {middle}",
        rear.rot_pnt_long()
    );
    // (30.1°)
    let want = v5_closed_form_alpha(middle);
    assert!(
        (alpha - want).abs() < 0.02 * want,
        "alpha {} deg, want {} deg",
        alpha.to_degrees(),
        want.to_degrees()
    );
    assert_eq!(rear.axle_steer(2), 0.0);
}

/// Checks: SC-006a (the rear section cuts the corner less with its last axle steering: its
/// line, the nearest it comes to the centre, runs on a wider circle)
#[test]
fn v5_rear_section_cuts_the_corner_less_with_its_last_axle_steering() {
    let (steering, _) = v5_at_half_steering(&v5_last_axle_steer(), false);
    let (locked, _) = v5_at_half_steering(&v5_last_axle_steer(), true);
    let (r_steering, r_locked) = (radius(&steering[1]), radius(&locked[1]));
    // (16.19 m and 15.37 m)
    assert!(
        r_steering > r_locked + 0.5,
        "rear line on {r_steering} m steering, {r_locked} m locked"
    );
}

/// Checks: C-9 (a rear section without declarations turns about its file's line as before)
#[test]
fn v5_rear_section_without_declarations_keeps_its_file_line() {
    let (bodies, alpha) = v5_at_half_steering(&v5_articulated_8x2(), false);
    assert!(bodies[1].steer_axles().is_empty());
    let want = v5_closed_form_alpha(1.2);
    assert!(
        (alpha - want).abs() < 0.02 * want,
        "alpha {}",
        alpha.to_degrees()
    );
    assert!(bodies[1].wheels().iter().all(|w| !w.steered()));
}

/// Checks: D-6 (behind a joint the axle follows the joint, not the steering wheel: with the
/// joint straight it stands straight at full lock)
#[test]
fn v5_rear_axle_stands_straight_while_the_joint_is() {
    let (mut bodies, joints) = depot(&v5_last_axle_steer());
    launch(&mut bodies, 3.0);
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    for _ in 0..60 {
        step(&mut bodies, &joints, &brakes, 0.0, 0.0);
    }
    assert_eq!(bodies[1].axle_steer(2), 0.0);
    step(&mut bodies, &joints, &brakes, 0.0, 1.0);
    let alpha = joints[0].angles(&bodies).0;
    assert!(
        alpha.abs() < 0.5f32.to_radians(),
        "alpha {}",
        alpha.to_degrees()
    );
    assert!(
        bodies[1].axle_steer(2).abs() < 0.5f32.to_radians(),
        "fourth axle {} deg with the joint at {} deg",
        bodies[1].axle_steer(2).to_degrees(),
        alpha.to_degrees()
    );
}

/// Checks: D-5, D-6 (locking the rear axle in a turn moves the rear section's line
/// continuously, from its rigid axle to the load-weighted middle)
#[test]
fn locking_the_rear_axle_in_a_turn_moves_the_line_continuously() {
    let (mut bodies, _) = v5_at_half_steering(&v5_last_axle_steer(), false);
    let joints = depot(&v5_last_axle_steer()).1;
    bodies[1].set_steer_input(1, SteerMode::Locked, 0.0);
    let brakes: Vec<Vec<f32>> = bodies.iter().map(|b| vec![0.0; b.wheels().len()]).collect();
    let radius = bodies[1].wheels()[0].radius();
    let mut line = bodies[1].rot_pnt_long();
    let mut worst = 0.0_f32;
    for _ in 0..600 {
        let mass: f32 = bodies.iter().map(|b| b.mass()).sum();
        let rolling: f32 = bodies.iter().map(|b| b.rolling_resistance()).sum();
        let torque = ((3.0 - bodies[0].forward_speed()) * mass * 2.0 + rolling) * radius;
        step(&mut bodies, &joints, &brakes, torque, 0.5);
        worst = worst.max((bodies[1].rot_pnt_long() - line).abs());
        line = bodies[1].rot_pnt_long();
    }
    assert!(bodies[1].steer_axle_locked(1));
    let middle = v5_rear_middle(&bodies[1]);
    assert!((line - middle).abs() < 0.05, "line {line}, want {middle}");
    // (the axle's 60°/s moves it about 0.1 m a frame over the 2.3 m; a jump would be all of it)
    assert!(worst < 0.25, "the line moved {worst} m in a frame");
}
