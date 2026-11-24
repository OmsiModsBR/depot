//! The mode of a vehicle at spawn (spec 002, D4 and D6): the front section's file decides, an AI
//! copy follows the setting alone.

mod support;

use omsi_cfg::{Driver, PhysicsMode};
use omsi_sim::resolve_physics_mode;
use omsi_vehicle::Vehicle;
use support::vehicles::gn92;

fn keyed(v: Vehicle) -> Vehicle {
    Vehicle {
        depot_physics: true,
        ..v
    }
}

#[test]
fn the_player_with_the_key_runs_depot_in_a_classic_session() {
    let (front, rear) = gn92(false);
    let mode = resolve_physics_mode(
        "t",
        PhysicsMode::Classic,
        &keyed(front),
        &[&rear],
        Driver::Player,
    );
    assert_eq!(mode, PhysicsMode::Depot);
}

#[test]
fn an_ai_copy_of_the_same_bus_follows_the_setting() {
    let (front, rear) = gn92(false);
    let front = keyed(front);
    assert_eq!(
        resolve_physics_mode("t", PhysicsMode::Classic, &front, &[&rear], Driver::Ai),
        PhysicsMode::Classic
    );
    assert_eq!(
        resolve_physics_mode("t", PhysicsMode::Depot, &front, &[&rear], Driver::Ai),
        PhysicsMode::Depot
    );
}

#[test]
fn the_key_only_in_a_rear_sections_file_changes_nothing() {
    let (front, rear) = gn92(false);
    let rear = keyed(rear);
    let mode = resolve_physics_mode("t", PhysicsMode::Classic, &front, &[&rear], Driver::Player);
    assert_eq!(mode, PhysicsMode::Classic);
}

#[test]
fn the_setting_depot_gives_depot_without_the_key() {
    let (front, rear) = gn92(false);
    let mode = resolve_physics_mode("t", PhysicsMode::Depot, &front, &[&rear], Driver::Player);
    assert_eq!(mode, PhysicsMode::Depot);
}

#[test]
fn no_setting_and_no_key_is_classic() {
    let (front, rear) = gn92(false);
    let mode = resolve_physics_mode(
        "t",
        PhysicsMode::default(),
        &front,
        &[&rear],
        Driver::Player,
    );
    assert_eq!(mode, PhysicsMode::Classic);
}

mod steer_axles {
    //! Steerable axles at spawn (spec 004, T009-T011): only Project Depot Physics has them, the
    //! leading section's front axle never, and each starts straight and steering.

    use super::support::vehicles::bus;
    use omsi_sim::physics::{Engine, PhysicsSpec, RigidBody, SteerMode, SteerState};
    use omsi_sim::testing::physics::RigidBodyInternals;
    use omsi_vehicle::vehicle::AxleSteer;
    use omsi_vehicle::Vehicle;

    const STEER: Option<AxleSteer> = Some(AxleSteer {
        lock_deg: 15.0,
        centring_deg_s: 20.0,
    });

    /// The test bus with a tag axle at -4 m, declared steerable; `front` declares the front
    /// axle too.
    fn tagged(front: bool) -> Vehicle {
        let mut v = bus();
        let mut tag = v.axles[1].clone();
        tag.long = -4.0;
        tag.driven = false;
        tag.steer = STEER;
        v.axles.push(tag);
        if front {
            v.axles[0].steer = STEER;
        }
        v
    }

    #[test]
    fn depot_reads_the_declaration_in_si_units() {
        let spec = PhysicsSpec::for_engine(&tagged(false), Engine::Depot);
        assert_eq!(spec.steer_axles.len(), 1);
        let s = spec.steer_axles[0];
        assert_eq!(s.axle, 2);
        assert!((s.lock - 15f32.to_radians()).abs() < 1e-6);
        assert!((s.centring - 20f32.to_radians()).abs() < 1e-6);
    }

    #[test]
    fn classic_leaves_the_declaration_out() {
        assert!(PhysicsSpec::for_engine(&tagged(true), Engine::Classic)
            .steer_axles
            .is_empty());
        let rb = RigidBody::for_engine(&tagged(true), &[], Engine::Classic);
        assert!(rb.steer_axles().is_empty());
    }

    #[test]
    fn classic_spec_with_the_parameters_is_the_spec_without_them() {
        let mut plain = tagged(true);
        for a in &mut plain.axles {
            a.steer = None;
        }
        assert_eq!(
            PhysicsSpec::for_engine(&tagged(true), Engine::Classic),
            PhysicsSpec::for_engine(&plain, Engine::Classic)
        );
    }

    #[test]
    fn each_steerable_axle_starts_straight_and_steering() {
        let rb = RigidBody::for_engine(&tagged(false), &[], Engine::Depot);
        assert_eq!(rb.steer_axles().len(), 1);
        let (axle, state) = rb.steer_axles()[0];
        assert_eq!(axle.axle, 2);
        assert_eq!(state, SteerState::default());
        assert_eq!(state.angle, 0.0);
        assert_eq!(state.mode, SteerMode::Basic);
    }

    #[test]
    fn the_leading_front_axle_is_the_drivers() {
        let mut rb = RigidBody::for_engine(&tagged(true), &[], Engine::Depot);
        assert_eq!(rb.steer_axles().len(), 2);
        rb.lead();
        let left: Vec<usize> = rb.steer_axles().iter().map(|(a, _)| a.axle).collect();
        assert_eq!(left, vec![2]);
    }

    #[test]
    fn a_section_behind_keeps_its_front_axle_steerable() {
        // (`lead` is for the leading section only: a rear section's first axle may steer)
        let rb = RigidBody::for_engine(&tagged(true), &[], Engine::Depot);
        assert!(rb.steer_axles().iter().any(|(a, _)| a.axle == 0));
    }

    #[test]
    fn a_script_value_picks_the_mode_and_anything_else_is_basic() {
        assert_eq!(SteerMode::from_value(0.0), SteerMode::Basic);
        assert_eq!(SteerMode::from_value(1.0), SteerMode::Locked);
        assert_eq!(SteerMode::from_value(2.0), SteerMode::ScriptAngle);
        for v in [3.0, -1.0, 0.5, 1.5, f32::NAN] {
            assert_eq!(SteerMode::from_value(v), SteerMode::Basic, "{v}");
        }
    }
}
