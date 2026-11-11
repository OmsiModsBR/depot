//! The line "Physics: <engine> (<reason>)" of the menu's "Vehicle" list (spec 002, FR-008a).

use omsi_cfg::PhysicsMode;
use openomsi_game::testing::hud::lists::physics_line;

/// The engine the player's vehicle runs, and why: its file's `[depot_physics]` key, or the
/// setting.
#[test]
fn the_vehicle_list_names_the_engine_and_why() {
    assert_eq!(
        physics_line(PhysicsMode::Classic, false),
        ("OMSI Classic", "setting")
    );
    assert_eq!(
        physics_line(PhysicsMode::Depot, false),
        ("Project Depot Physics", "setting")
    );
    assert_eq!(
        physics_line(PhysicsMode::Depot, true),
        ("Project Depot Physics", "bus key")
    );
}
