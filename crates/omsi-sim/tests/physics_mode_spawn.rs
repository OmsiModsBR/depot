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
