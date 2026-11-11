//! Conformance of the player's vehicle in Project Depot Physics against the equations of
//! `docs/physics/` (spec 002, phase 7): the engine run on the five conformance vehicles
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

/// Checks: C-3, D-1
#[test]
fn static_sag_v1_solo_4x2() {
    static_sag_solo(v1_solo_4x2(), Engine::Depot);
}

/// Checks: C-3, D-1
#[test]
fn static_sag_v2_rigid_6x2() {
    static_sag_solo(v2_rigid_6x2(), Engine::Depot);
}

/// Checks: C-3, C-9, D-1
#[test]
fn static_balance_v3_articulated_6x2() {
    static_balance_train(v3_articulated_6x2(), Engine::Depot);
}

/// Checks: C-3, C-9, D-1
#[test]
fn static_balance_v4_biarticulated() {
    static_balance_train(v4_biarticulated(), Engine::Depot);
}

/// Checks: C-3, C-9, D-1
#[test]
fn static_balance_v5_articulated_8x2() {
    static_balance_train(v5_articulated_8x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8
#[test]
fn braking_v1_solo_4x2() {
    braking_distance(v1_solo_4x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8
#[test]
fn braking_v2_rigid_6x2() {
    braking_distance(v2_rigid_6x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn braking_v3_articulated_6x2() {
    braking_distance(v3_articulated_6x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn braking_v4_biarticulated() {
    braking_distance(v4_biarticulated(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn braking_v5_articulated_8x2() {
    braking_distance(v5_articulated_8x2(), Engine::Depot);
}

/// Checks: C-6, C-7
#[test]
fn steady_turn_v1_solo_4x2() {
    steady_turn(v1_solo_4x2(), Engine::Depot);
}

/// Checks: C-6, C-7
#[test]
fn steady_turn_v2_rigid_6x2() {
    steady_turn(v2_rigid_6x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8
#[test]
fn pull_away_v1_solo_4x2() {
    pull_away(v1_solo_4x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8
#[test]
fn pull_away_v2_rigid_6x2() {
    pull_away(v2_rigid_6x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn pull_away_v3_articulated_6x2() {
    pull_away(v3_articulated_6x2(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn pull_away_v4_biarticulated() {
    pull_away(v4_biarticulated(), Engine::Depot);
}

/// Checks: C-4, C-5, C-8, C-9
#[test]
fn pull_away_v5_articulated_8x2() {
    pull_away(v5_articulated_8x2(), Engine::Depot);
}

/// Checks: C-6, C-7, C-9
#[test]
fn steady_turn_v3_articulated_6x2() {
    steady_turn(v3_articulated_6x2(), Engine::Depot);
}

/// Checks: C-6, C-7, C-9
#[test]
fn steady_turn_v4_biarticulated() {
    steady_turn(v4_biarticulated(), Engine::Depot);
}

/// Checks: C-6, C-7, C-9
#[test]
fn steady_turn_v5_articulated_8x2() {
    steady_turn(v5_articulated_8x2(), Engine::Depot);
}

/// Checks: C-9
#[test]
fn articulation_angles_v3_articulated_6x2() {
    articulation_angles(v3_articulated_6x2(), Engine::Depot);
}

/// Checks: C-9
#[test]
fn articulation_angles_v4_biarticulated() {
    articulation_angles(v4_biarticulated(), Engine::Depot);
}

/// Checks: C-7
#[test]
fn release_v1_solo_4x2() {
    release_by_speed(v1_solo_4x2(), Engine::Depot, 0.6);
}

/// Checks: C-7
#[test]
fn release_v2_rigid_6x2() {
    release_by_speed(v2_rigid_6x2(), Engine::Depot, 0.6);
}

/// Checks: C-7
#[test]
fn release_v3_articulated_6x2() {
    release_by_speed(v3_articulated_6x2(), Engine::Depot, 0.6);
}

/// Checks: C-7
#[test]
fn release_v4_biarticulated() {
    // the weight shared about `[cog]` loads the last section's axle with 0.84 of its weight
    // (0.92 about the origin), so the bend the tyres hold lies lower: 0.45 of the limit
    release_by_speed(v4_biarticulated(), Engine::Depot, 0.45);
}

/// Checks: C-7
#[test]
fn release_v5_articulated_8x2() {
    release_by_speed(v5_articulated_8x2(), Engine::Depot, 0.6);
}

mod model_ai;

/// The AI's rows (Project Depot Physics, today's body unchanged): `DA-1` to `DA-3` the rear
/// axle led along the lane by pure pursuit, within about a metre in a 12 m bend; `DA-5` the
/// sprung body leaning into the bend; `DA-6` the static sag shared about `[cog]`.
fn depot_ai_follows_the_lane(defs: Vec<omsi_vehicle::Vehicle>) {
    let (off, roll, gap) = model_ai::route_following(&defs, omsi_cfg::PhysicsMode::Depot);
    assert!(off < 1.05, "the rotation point {off} m off the lane");
    assert!(roll > 0.3 && roll < 5.0, "roll {roll} deg");
    assert!(gap < 0.02, "a joint {gap} m apart");
    assert!(model_ai::static_loads_follow_cog(&defs, Engine::Depot));
}

/// Checks: DA-1, DA-2, DA-3, DA-5, DA-6
#[test]
fn ai_route_v1_solo_4x2() {
    depot_ai_follows_the_lane(v1_solo_4x2());
}

/// Checks: DA-1, DA-2, DA-3, DA-5, DA-6
#[test]
fn ai_route_v2_rigid_6x2() {
    depot_ai_follows_the_lane(v2_rigid_6x2());
}

/// Checks: DA-1, DA-2, DA-3, DA-5, DA-6
#[test]
fn ai_route_v3_articulated_6x2() {
    depot_ai_follows_the_lane(v3_articulated_6x2());
}

/// Checks: DA-1, DA-2, DA-3, DA-5, DA-6
#[test]
fn ai_route_v4_biarticulated() {
    depot_ai_follows_the_lane(v4_biarticulated());
}

/// Checks: DA-1, DA-2, DA-3, DA-5, DA-6
#[test]
fn ai_route_v5_articulated_8x2() {
    depot_ai_follows_the_lane(v5_articulated_8x2());
}

/// Checks: D-2
#[test]
fn follower_axle_steering_v5_articulated_8x2() {
    follower_axle_steering(v5_articulated_8x2(), Engine::Depot, true);
}
