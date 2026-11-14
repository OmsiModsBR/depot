//! The graphics mode (spec 001, `data-model.md` and `contracts/settings-cfg.md`): how the
//! `graphics` setting reads and how it is saved.

use omsi_cfg::GraphicsMode;

#[test]
fn a_missing_or_empty_value_is_vanilla_plus() {
    assert_eq!(GraphicsMode::parse(""), GraphicsMode::VanillaPlus);
    assert_eq!(GraphicsMode::parse("   "), GraphicsMode::VanillaPlus);
    assert_eq!(GraphicsMode::default(), GraphicsMode::VanillaPlus);
}

#[test]
fn every_spelling_of_vanilla_reads_as_vanilla() {
    for text in [
        "vanilla",
        "Vanilla",
        "VANILLA",
        "classic",
        "original",
        "omsi",
        "omsi2",
        "omsi_2",
        "OMSI 2",
        "omsi-2",
        " vanilla ",
    ] {
        assert_eq!(GraphicsMode::parse(text), GraphicsMode::Vanilla, "{text}");
    }
}

#[test]
fn every_spelling_of_vanilla_plus_reads_as_vanilla_plus() {
    for text in [
        "vanilla_plus",
        "Vanilla+",
        "vanilla-plus",
        "vanilla plus",
        "vanillaplus",
    ] {
        assert_eq!(
            GraphicsMode::parse(text),
            GraphicsMode::VanillaPlus,
            "{text}"
        );
    }
}

#[test]
fn every_spelling_of_enhanced_reads_as_enhanced() {
    for text in ["enhanced", "Enhanced", "ENHANCED", "1"] {
        assert_eq!(GraphicsMode::parse(text), GraphicsMode::Enhanced, "{text}");
    }
}

#[test]
fn the_removed_enhanced_plus_reads_as_enhanced() {
    for text in [
        "enhanced_plus",
        "enhanced+",
        "Enhanced+",
        "enhanced-plus",
        "enhanced plus",
        "enhancedplus",
        "enhanced_+",
        "2",
    ] {
        assert_eq!(GraphicsMode::parse(text), GraphicsMode::Enhanced, "{text}");
    }
}

#[test]
fn an_unknown_value_is_vanilla_plus() {
    for text in ["ultra", "0", "3", "ray_tracing", "vanilla2"] {
        assert_eq!(
            GraphicsMode::parse(text),
            GraphicsMode::VanillaPlus,
            "{text}"
        );
    }
}

#[test]
fn a_mode_is_saved_with_its_canonical_spelling_and_reads_back() {
    assert_eq!(GraphicsMode::Vanilla.as_str(), "vanilla");
    assert_eq!(GraphicsMode::VanillaPlus.as_str(), "vanilla_plus");
    assert_eq!(GraphicsMode::Enhanced.as_str(), "enhanced");
    for mode in GraphicsMode::ALL {
        assert_eq!(GraphicsMode::parse(mode.as_str()), mode);
    }
}
