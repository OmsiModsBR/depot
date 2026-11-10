//! The physics mode of a vehicle (spec 002, `contracts/settings-cfg.md` and `bus-key.md`):
//! how the `physics` setting reads, and which mode a vehicle gets from it and from the
//! `[depot_physics]` key of its file.

use omsi_cfg::{Driver, PhysicsMode, Reason};

#[test]
fn a_missing_or_empty_value_is_classic() {
    assert_eq!(PhysicsMode::parse(""), PhysicsMode::Classic);
    assert_eq!(PhysicsMode::parse("   "), PhysicsMode::Classic);
    assert_eq!(PhysicsMode::default(), PhysicsMode::Classic);
}

#[test]
fn every_spelling_of_classic_reads_as_classic() {
    for text in [
        "classic",
        "Classic",
        "CLASSIC",
        "omsi",
        "omsi2",
        "omsi_classic",
        "OMSI-Classic",
        "omsi classic",
        "  classic  ",
    ] {
        assert_eq!(PhysicsMode::parse(text), PhysicsMode::Classic, "{text:?}");
    }
}

#[test]
fn every_spelling_of_depot_reads_as_depot() {
    for text in [
        "depot",
        "Depot",
        "DEPOT",
        "project_depot",
        "Project-Depot",
        "project depot",
    ] {
        assert_eq!(PhysicsMode::parse(text), PhysicsMode::Depot, "{text:?}");
    }
}

#[test]
fn anything_else_is_classic() {
    for text in ["auto", "depot2", "0", "physics", "omsi3"] {
        assert_eq!(PhysicsMode::parse(text), PhysicsMode::Classic, "{text:?}");
    }
}

#[test]
fn only_the_two_canonical_spellings_are_written() {
    assert_eq!(PhysicsMode::Classic.as_str(), "classic");
    assert_eq!(PhysicsMode::Depot.as_str(), "depot");
    for mode in [PhysicsMode::Classic, PhysicsMode::Depot] {
        assert_eq!(PhysicsMode::parse(mode.as_str()), mode);
    }
}

#[test]
fn the_key_makes_the_players_vehicle_depot_whatever_the_setting_says() {
    for global in [PhysicsMode::Classic, PhysicsMode::Depot] {
        assert_eq!(
            PhysicsMode::for_vehicle(global, true, Driver::Player),
            (PhysicsMode::Depot, Reason::BusKey)
        );
    }
}

#[test]
fn without_the_key_the_vehicle_follows_the_setting() {
    for global in [PhysicsMode::Classic, PhysicsMode::Depot] {
        for driver in [Driver::Player, Driver::Ai] {
            assert_eq!(
                PhysicsMode::for_vehicle(global, false, driver),
                (global, Reason::Setting)
            );
        }
    }
}

#[test]
fn an_ai_vehicle_never_takes_the_key() {
    for global in [PhysicsMode::Classic, PhysicsMode::Depot] {
        assert_eq!(
            PhysicsMode::for_vehicle(global, true, Driver::Ai),
            (global, Reason::Setting)
        );
    }
}

#[test]
fn no_input_makes_a_vehicle_classic_when_the_setting_or_key_says_depot() {
    for driver in [Driver::Player, Driver::Ai] {
        for key in [false, true] {
            let (mode, _) = PhysicsMode::for_vehicle(PhysicsMode::Depot, key, driver);
            assert_eq!(mode, PhysicsMode::Depot);
        }
    }
}
