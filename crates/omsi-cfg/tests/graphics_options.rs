//! The graphics option table (spec 001, `data-model.md`): what each mode offers and the value
//! the game draws with.

use omsi_cfg::graphics_options::{
    allows, applies, fit, graphics_option, offered_in_game, offered_in_launcher, Applies, Effect,
    GRAPHICS_OPTIONS,
};
use omsi_cfg::GraphicsMode::{self, Enhanced, Vanilla, VanillaPlus};

const MODES: [GraphicsMode; 3] = [Vanilla, VanillaPlus, Enhanced];

#[test]
fn every_key_has_one_row() {
    for (i, o) in GRAPHICS_OPTIONS.iter().enumerate() {
        assert!(
            GRAPHICS_OPTIONS[i + 1..].iter().all(|p| p.key != o.key),
            "{} has two rows",
            o.key
        );
    }
}

#[test]
fn the_mode_and_the_api_take_effect_at_the_next_start() {
    for key in ["graphics", "graphics_api"] {
        assert_eq!(graphics_option(key).unwrap().effect, Effect::NextStart);
        for mode in MODES {
            assert!(!offered_in_game(key, mode), "{key} in {mode:?}");
        }
    }
    assert!(!offered_in_launcher("graphics_api", Vanilla));
}

#[test]
fn offered_in_the_game_means_offered_in_the_launcher() {
    for o in GRAPHICS_OPTIONS {
        for mode in MODES {
            if offered_in_game(o.key, mode) {
                assert!(offered_in_launcher(o.key, mode), "{} in {mode:?}", o.key);
            }
        }
    }
}

#[test]
fn a_key_without_a_row_is_offered_everywhere() {
    for mode in MODES {
        assert_eq!(applies("volume", mode), Applies::Offered);
        assert!(offered_in_game("volume", mode));
        assert_eq!(fit("volume", mode, "0.4"), "0.4");
    }
}

#[test]
fn choices_fit_to_the_nearest_allowed_value() {
    // (Vanilla's mirror sizes and render scale steps)
    assert_eq!(fit("mirror_size", Vanilla, "256"), "256");
    assert_eq!(fit("mirror_size", Vanilla, "1024"), "512");
    assert_eq!(fit("mirror_size", Vanilla, "200"), "256");
    assert_eq!(fit("render_scale", Vanilla, "0.6"), "0.67");
    assert_eq!(fit("render_scale", Vanilla, "auto"), "auto");
    // (not a number: the last choice)
    assert_eq!(fit("mirror_refresh", Vanilla, "eco"), "full");
    assert!(allows("mirror_refresh", Vanilla, "off"));
    assert!(!allows("mirror_refresh", Vanilla, "eco"));
    assert!(allows("mirror_refresh", VanillaPlus, "eco"));
}

#[test]
fn a_positive_value_never_fits_to_off() {
    assert_eq!(fit("mirror_size", Vanilla, "16"), "128");
    assert_eq!(fit("mirror_size", Vanilla, "0"), "0");
}

#[test]
fn a_fixed_option_draws_with_its_value() {
    assert_eq!(fit("clouds", Vanilla, "false"), "true");
    assert_eq!(fit("windy_trees", Vanilla, "true"), "false");
    assert_eq!(fit("shadow_blobs", Vanilla, "false"), "true");
    assert!(offered_in_launcher("clouds", Vanilla));
    assert_eq!(fit("clouds", Enhanced, "false"), "false");
}

#[test]
fn an_option_not_offered_keeps_its_saved_value_unless_the_mode_holds_it() {
    // (Vanilla draws no sun shadows, ambient occlusion or detail grain, and no brighter night)
    assert_eq!(fit("shadows", Vanilla, "true"), "false");
    assert_eq!(fit("ssao", Vanilla, "true"), "false");
    assert_eq!(fit("detail_textures", Vanilla, "true"), "false");
    assert_eq!(fit("night_brightness", Vanilla, "1.5"), "0");
    // (kept as saved, for the mode that offers it)
    assert_eq!(fit("night_brightness", VanillaPlus, "1.5"), "1.5");
    assert_eq!(fit("led_glow", Vanilla, "9"), "9");
    assert_eq!(fit("dx9_stars", Enhanced, "false"), "false");
}

#[test]
fn the_shadow_options_are_not_called_improvements_in_vanilla() {
    // (OMSI 2 has sun shadows: Vanilla does not draw them yet)
    for key in ["shadows", "shadow_size", "shadow_casters"] {
        match applies(key, Vanilla) {
            Applies::NotOffered { reason, .. } => {
                assert_eq!(reason, "not drawn by Vanilla yet", "{key}")
            }
            other => panic!("{key} in Vanilla: {other:?}"),
        }
    }
}

#[test]
fn vr_and_the_triple_screen_are_not_offered_in_vanilla() {
    for o in GRAPHICS_OPTIONS
        .iter()
        .filter(|o| o.key == "vr" || o.key.starts_with("vr_") || o.key.starts_with("triple_"))
    {
        assert!(!offered_in_launcher(o.key, Vanilla), "{}", o.key);
        assert!(offered_in_launcher(o.key, VanillaPlus), "{}", o.key);
    }
}

#[test]
fn vanillas_own_options_are_only_in_vanilla_and_enhanceds_only_in_enhanced() {
    for key in [
        "rain_reflections",
        "fewer_night_lights",
        "dx9_stars",
        "dx9_texture_limit",
    ] {
        assert!(offered_in_launcher(key, Vanilla), "{key}");
        assert!(!offered_in_launcher(key, VanillaPlus), "{key}");
        assert!(!offered_in_launcher(key, Enhanced), "{key}");
    }
    for key in ["night_brightness", "led_glow", "led_mips"] {
        assert!(!offered_in_launcher(key, Vanilla), "{key}");
        assert!(!offered_in_launcher(key, VanillaPlus), "{key}");
        assert!(offered_in_launcher(key, Enhanced), "{key}");
    }
}

/// The table as the user guide prints it ("Graphics options per mode").
fn guide_table() -> String {
    let cell = |a: Applies| match a {
        Applies::Offered => "yes".to_string(),
        Applies::Choices { values, .. } => format!("only {}", values.join(", ")),
        Applies::Fixed { shown, .. } => format!("fixed: {shown}"),
        Applies::NotOffered { .. } => "no".to_string(),
    };
    let mut out = String::from(
        "| Setting | Vanilla | Vanilla+ | Enhanced | Takes effect |\n|---|---|---|---|---|\n",
    );
    for o in GRAPHICS_OPTIONS {
        out.push_str(&format!(
            "| `{}` | {} | {} | {} | {} |\n",
            o.key,
            cell(o.applies(Vanilla)),
            cell(o.applies(VanillaPlus)),
            cell(o.applies(Enhanced)),
            match o.effect {
                Effect::Live => "at once",
                Effect::NextStart => "next start",
            }
        ));
    }
    out
}

/// The user guide's table is printed from the code's (spec 001, T034): run with
/// `UPDATE_USER_GUIDE=1` to write it again after a change of the table.
#[test]
fn the_user_guide_prints_the_table() {
    const BEGIN: &str =
        "<!-- graphics-options:begin (printed by crates/omsi-cfg/tests/graphics_options.rs) -->\n";
    const END: &str = "<!-- graphics-options:end -->";
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/USER_GUIDE.md");
    // (a Windows checkout may end its lines with CRLF)
    let guide = std::fs::read_to_string(&path)
        .unwrap()
        .replace("\r\n", "\n");
    let start = guide
        .find(BEGIN)
        .expect("the user guide has no graphics options table")
        + BEGIN.len();
    let end = start + guide[start..].find(END).unwrap();
    let want = guide_table();
    if guide[start..end] != want {
        if std::env::var_os("UPDATE_USER_GUIDE").is_some() {
            std::fs::write(&path, format!("{}{want}{}", &guide[..start], &guide[end..])).unwrap();
        } else {
            panic!("docs/USER_GUIDE.md's graphics options table is out of date; run this test with UPDATE_USER_GUIDE=1");
        }
    }
}
