//! The `physics` setting (spec 002, `contracts/settings-cfg.md`): every row of the contract, the
//! round trip, and that a file without the key stays Classic.

use omsi_cfg::PhysicsMode;
use openomsi_game::testing::settings::*;

#[test]
fn a_file_without_physics_is_classic() {
    assert_eq!(Settings::default().physics, PhysicsMode::Classic);
    assert_eq!(Settings::from_text("").physics, PhysicsMode::Classic);
    assert_eq!(
        Settings::from_text("version=3\nmsaa=4\n").physics,
        PhysicsMode::Classic
    );
}

#[test]
fn every_row_of_the_contract_reads_as_it_says() {
    for (saved, want) in [
        ("classic", PhysicsMode::Classic),
        ("omsi", PhysicsMode::Classic),
        ("omsi2", PhysicsMode::Classic),
        ("omsi_classic", PhysicsMode::Classic),
        ("CLASSIC", PhysicsMode::Classic),
        ("depot", PhysicsMode::Depot),
        ("project_depot", PhysicsMode::Depot),
        ("Depot", PhysicsMode::Depot),
        ("nonsense", PhysicsMode::Classic),
        ("", PhysicsMode::Classic),
    ] {
        let s = Settings::from_text(&format!("physics={saved}\n"));
        assert_eq!(s.physics, want, "physics={saved}");
    }
}

#[test]
fn the_mode_survives_a_save() {
    for mode in [PhysicsMode::Classic, PhysicsMode::Depot] {
        let s = Settings {
            physics: mode,
            ..Default::default()
        };
        let text = s.to_text();
        assert!(
            text.contains(&format!("physics={}\n", mode.as_str())),
            "{text}"
        );
        assert_eq!(Settings::from_text(&text), s);
    }
}
