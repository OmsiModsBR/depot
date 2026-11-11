//! Conformance of the player's vehicle in OMSI Classic against the equations of
//! `docs/physics/` (spec 002, phase 2): the engine run on the five conformance vehicles
//! (`support::vehicles`) and compared with the closed forms (`support::closed_form`), by the
//! checks of `support::model`.
//!
//! Each test is named `<check>_<vehicle>` and names the equations it checks.

mod support;

use omsi_sim::physics::Engine;
use support::model::{
    articulation_angles, braking_distance, follower_axle_steering, pull_away, release_by_speed,
    static_balance_train, static_sag_solo, steady_turn,
};
use support::vehicles::{
    v1_solo_4x2, v2_rigid_6x2, v3_articulated_6x2, v4_biarticulated, v5_articulated_8x2,
};

/// Checks: C-3, K-1
#[test]
fn static_sag_v1_solo_4x2() {
    static_sag_solo(v1_solo_4x2(), Engine::Classic);
}

/// Checks: C-3, K-1
#[test]
fn static_sag_v2_rigid_6x2() {
    static_sag_solo(v2_rigid_6x2(), Engine::Classic);
}

/// Checks: C-3, C-9, K-1
#[test]
fn static_balance_v3_articulated_6x2() {
    static_balance_train(v3_articulated_6x2(), Engine::Classic);
}

/// Checks: C-3, C-9, K-1
#[test]
fn static_balance_v4_biarticulated() {
    static_balance_train(v4_biarticulated(), Engine::Classic);
}

/// Checks: C-3, C-9, K-1
#[test]
fn static_balance_v5_articulated_8x2() {
    static_balance_train(v5_articulated_8x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8
#[test]
fn braking_v1_solo_4x2() {
    braking_distance(v1_solo_4x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8
#[test]
fn braking_v2_rigid_6x2() {
    braking_distance(v2_rigid_6x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn braking_v3_articulated_6x2() {
    braking_distance(v3_articulated_6x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn braking_v4_biarticulated() {
    braking_distance(v4_biarticulated(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn braking_v5_articulated_8x2() {
    braking_distance(v5_articulated_8x2(), Engine::Classic);
}

/// Checks: C-6, C-7
#[test]
fn steady_turn_v1_solo_4x2() {
    steady_turn(v1_solo_4x2(), Engine::Classic);
}

/// Checks: C-6, C-7
#[test]
fn steady_turn_v2_rigid_6x2() {
    steady_turn(v2_rigid_6x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8
#[test]
fn pull_away_v1_solo_4x2() {
    pull_away(v1_solo_4x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8
#[test]
fn pull_away_v2_rigid_6x2() {
    pull_away(v2_rigid_6x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn pull_away_v3_articulated_6x2() {
    pull_away(v3_articulated_6x2(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn pull_away_v4_biarticulated() {
    pull_away(v4_biarticulated(), Engine::Classic);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn pull_away_v5_articulated_8x2() {
    pull_away(v5_articulated_8x2(), Engine::Classic);
}

/// Checks: C-6, C-7, C-9
#[test]
fn steady_turn_v3_articulated_6x2() {
    steady_turn(v3_articulated_6x2(), Engine::Classic);
}

/// Checks: C-6, C-7, C-9
#[test]
fn steady_turn_v4_biarticulated() {
    steady_turn(v4_biarticulated(), Engine::Classic);
}

/// Checks: C-6, C-7, C-9
#[test]
fn steady_turn_v5_articulated_8x2() {
    steady_turn(v5_articulated_8x2(), Engine::Classic);
}

/// Checks: C-9
#[test]
fn articulation_angles_v3_articulated_6x2() {
    articulation_angles(v3_articulated_6x2(), Engine::Classic);
}

/// Checks: C-9
#[test]
fn articulation_angles_v4_biarticulated() {
    articulation_angles(v4_biarticulated(), Engine::Classic);
}

/// Checks: C-7
#[test]
fn release_v1_solo_4x2() {
    release_by_speed(v1_solo_4x2(), Engine::Classic, 0.6);
}

/// Checks: C-7
#[test]
fn release_v2_rigid_6x2() {
    release_by_speed(v2_rigid_6x2(), Engine::Classic, 0.6);
}

/// Checks: C-7
#[test]
fn release_v3_articulated_6x2() {
    release_by_speed(v3_articulated_6x2(), Engine::Classic, 0.6);
}

/// Checks: C-7
#[test]
fn release_v4_biarticulated() {
    release_by_speed(v4_biarticulated(), Engine::Classic, 0.6);
}

/// Checks: C-7
#[test]
fn release_v5_articulated_8x2() {
    release_by_speed(v5_articulated_8x2(), Engine::Classic, 0.6);
}

mod model_ai;

/// The AI's rows (OMSI Classic): `KA-1` the rotation point on the lane within 1 cm (SC-006),
/// `KA-4` no roll, `KA-5` the rear sections on their joints.
fn classic_ai_follows_the_lane(defs: Vec<omsi_vehicle::Vehicle>) {
    let (off, roll, gap) = model_ai::route_following(&defs, omsi_cfg::PhysicsMode::Classic);
    assert!(off < 0.01, "the rotation point {off} m off the lane");
    assert_eq!(roll, 0.0);
    assert!(gap < 0.02, "a joint {gap} m apart");
    // `KA-4`, FR-015: the static sag the scripts read is shared about the origin
    assert!(!model_ai::static_loads_follow_cog(&defs, Engine::Classic));
}

/// Checks: KA-1, KA-4
#[test]
fn ai_route_v1_solo_4x2() {
    classic_ai_follows_the_lane(v1_solo_4x2());
}

/// Checks: KA-1, KA-4
#[test]
fn ai_route_v2_rigid_6x2() {
    classic_ai_follows_the_lane(v2_rigid_6x2());
}

/// Checks: KA-1, KA-4, KA-5
#[test]
fn ai_route_v3_articulated_6x2() {
    classic_ai_follows_the_lane(v3_articulated_6x2());
}

/// Checks: KA-1, KA-4, KA-5
#[test]
fn ai_route_v4_biarticulated() {
    classic_ai_follows_the_lane(v4_biarticulated());
}

/// Checks: KA-1, KA-4, KA-5
#[test]
fn ai_route_v5_articulated_8x2() {
    classic_ai_follows_the_lane(v5_articulated_8x2());
}

/// Checks: KA-6, KA-6b, KA-7
#[test]
fn ai_release_v1_solo_4x2() {
    assert!(model_ai::release_and_return(&v1_solo_4x2()) < 4.0);
}

/// Checks: KA-6, KA-6b, KA-7
#[test]
fn ai_release_v3_articulated_6x2() {
    assert!(model_ai::release_and_return(&v3_articulated_6x2()) < 4.0);
}

/// Checks: KA-6, KA-6b, KA-7
#[test]
fn ai_release_v4_biarticulated() {
    assert!(model_ai::release_and_return(&v4_biarticulated()) < 4.0);
}

/// Checks: K-2
#[test]
fn follower_axle_steering_v5_articulated_8x2() {
    follower_axle_steering(v5_articulated_8x2(), Engine::Classic, false);
}
