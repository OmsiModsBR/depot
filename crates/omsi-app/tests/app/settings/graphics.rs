//! The `graphics` setting after Enhanced+ was removed (spec 001, `contracts/settings-cfg.md`
//! and `contracts/cli-env.md`): the old name reads as Enhanced in the game and in the
//! launcher, a save writes Enhanced, and what the removed mode forced on is the player's
//! choice again.

use omsi_launcher_lib::{settings_from_text, settings_to_text};
use openomsi_game::testing::env_file::parse;
use openomsi_game::testing::settings::*;

/// The ways an old file spells Enhanced+.
const OLD_NAMES: [&str; 6] = [
    "enhanced_plus",
    "enhanced+",
    "Enhanced+",
    "enhanced-plus",
    "enhancedplus",
    "2",
];

#[test]
fn enhanced_plus_reads_as_enhanced_in_the_game() {
    for name in OLD_NAMES {
        assert_eq!(graphics_mode(name), "enhanced", "{name}");
        let s = Settings::from_text(&format!("graphics={name}\n"));
        assert_eq!(s.graphics, "enhanced", "{name}");
        assert!(s.enhanced && !s.classic(), "{name}");
        assert!(!s.render_options().no_enhanced, "{name}");
    }
}

#[test]
fn enhanced_plus_reads_as_enhanced_in_the_launcher() {
    for name in OLD_NAMES {
        assert_eq!(omsi_launcher_lib::graphics_mode(name), "enhanced", "{name}");
        let v = settings_from_text(Some(&format!("graphics={name}\n")));
        assert_eq!(v["graphics"], "enhanced", "{name}");
        assert_eq!(v["enhanced"], true, "{name}");
    }
}

#[test]
fn a_saved_enhanced_plus_becomes_enhanced() {
    let old = "graphics=enhanced_plus\n";
    let game = Settings::from_text(old).to_text();
    assert!(game.lines().any(|l| l == "graphics=enhanced"), "{game}");
    assert!(!game.contains("enhanced_plus"), "{game}");
    let launcher = settings_to_text(&settings_from_text(Some(old)), Some(old));
    assert!(
        launcher.lines().any(|l| l == "graphics=enhanced"),
        "{launcher}"
    );
    assert!(!launcher.contains("enhanced_plus"), "{launcher}");
}

#[test]
fn what_enhanced_plus_forced_on_is_the_players_choice_again() {
    // (Enhanced+ traced its shadows, occlusion and reflections and switched all three on)
    let s = Settings::from_text("graphics=enhanced_plus\nshadows=0\nssao=0\nreflections=0\n");
    assert!(!s.shadows && !s.ssao && !s.reflections);
}

#[test]
fn a_removed_variable_in_env_txt_is_skipped_not_an_error() {
    let mut skipped = Vec::new();
    let got = parse(
        "OMSI_NO_RT=1\nOMSI_NO_RT_GRADE=1\nOMSI_ENHANCED_PLUS=1\nOMSI_NO_SHADOWS=1\n",
        &mut skipped,
    );
    assert_eq!(got, vec![("OMSI_NO_SHADOWS".into(), "1".into())]);
    assert_eq!(
        skipped,
        vec!["OMSI_NO_RT", "OMSI_NO_RT_GRADE", "OMSI_ENHANCED_PLUS"]
    );
}

#[test]
fn the_enhanced_plus_argument_is_still_taken() {
    use clap::Parser;
    let args =
        openomsi_game::testing::core::cli::Args::try_parse_from(["depot", "--enhanced-plus"])
            .expect("an old shortcut still starts the game");
    assert!(args.enhanced_plus && !args.enhanced);
}
