//! The `graphics` setting (spec 001, `contracts/settings-cfg.md` and `contracts/cli-env.md`):
//! every row of the contract in the game and in the launcher, the old `enhanced` key, the
//! round trip, the mode resolved for a run, and what is left of the removed Enhanced+.

use omsi_launcher_lib::{settings_from_text, settings_to_text};
use openomsi_game::testing::core::startup::resolved_graphics;
use openomsi_game::testing::env_file::parse;
use openomsi_game::testing::settings::*;

/// The rows of the contract: a saved value and the mode it opens.
const ROWS: [(&str, GraphicsMode); 17] = [
    ("vanilla", GraphicsMode::Vanilla),
    ("classic", GraphicsMode::Vanilla),
    ("original", GraphicsMode::Vanilla),
    ("omsi", GraphicsMode::Vanilla),
    ("omsi2", GraphicsMode::Vanilla),
    ("OMSI 2", GraphicsMode::Vanilla),
    ("vanilla_plus", GraphicsMode::VanillaPlus),
    ("Vanilla+", GraphicsMode::VanillaPlus),
    ("", GraphicsMode::VanillaPlus),
    ("nonsense", GraphicsMode::VanillaPlus),
    ("enhanced", GraphicsMode::Enhanced),
    ("1", GraphicsMode::Enhanced),
    ("enhanced_plus", GraphicsMode::Enhanced),
    ("enhanced+", GraphicsMode::Enhanced),
    ("Enhanced+", GraphicsMode::Enhanced),
    ("enhancedplus", GraphicsMode::Enhanced),
    ("2", GraphicsMode::Enhanced),
];

#[test]
fn every_row_of_the_contract_reads_as_it_says_in_the_game() {
    for (saved, want) in ROWS {
        let s = Settings::from_text(&format!("graphics={saved}\n"));
        assert_eq!(s.graphics, want, "{saved:?}");
        assert_eq!(
            s.render_options().path == omsi_render::ShadingPath::Hdr,
            want == GraphicsMode::Enhanced,
            "{saved:?}"
        );
    }
}

#[test]
fn every_row_of_the_contract_reads_as_it_says_in_the_launcher() {
    for (saved, want) in ROWS {
        let v = settings_from_text(Some(&format!("graphics={saved}\n")));
        assert_eq!(v["graphics"], want.as_str(), "{saved:?}");
    }
}

#[test]
fn a_file_without_graphics_follows_the_old_enhanced_key() {
    for (text, want) in [
        ("", GraphicsMode::VanillaPlus),
        ("enhanced=0\n", GraphicsMode::VanillaPlus),
        ("enhanced=1\n", GraphicsMode::Enhanced),
        // (`graphics` decides when both are there)
        ("graphics=enhanced\nenhanced=0\n", GraphicsMode::Enhanced),
        ("graphics=vanilla\nenhanced=1\n", GraphicsMode::Vanilla),
        (
            "enhanced=1\ngraphics=vanilla_plus\n",
            GraphicsMode::VanillaPlus,
        ),
    ] {
        assert_eq!(Settings::from_text(text).graphics, want, "{text:?}");
        let v = settings_from_text(Some(text));
        assert_eq!(v["graphics"], want.as_str(), "{text:?}");
    }
}

#[test]
fn a_save_writes_the_canonical_spelling_and_not_the_old_key() {
    for mode in GraphicsMode::ALL {
        let text = format!("graphics={}\n", mode.as_str());
        let game = Settings::from_text(&text).to_text();
        let launcher = settings_to_text(&settings_from_text(Some(&text)), Some(&text));
        for saved in [&game, &launcher] {
            assert!(
                saved.lines().any(|l| l == text.trim_end()),
                "{mode:?}: {saved}"
            );
            assert!(
                !saved.lines().any(|l| l.starts_with("enhanced=")),
                "{mode:?}: {saved}"
            );
            assert_eq!(Settings::from_text(saved).graphics, mode);
        }
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
fn reading_does_not_rewrite_the_file() {
    // (FR-012: an old spelling stays in the file until the player saves)
    let dir = std::env::temp_dir().join(format!("depot-graphics-read-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("settings.cfg");
    let old = "graphics=enhanced_plus\nenhanced=1\n";
    std::fs::write(&path, old).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert_eq!(Settings::from_text(&text).graphics, GraphicsMode::Enhanced);
    assert_eq!(settings_from_text(Some(&text))["graphics"], "enhanced");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), old);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn the_mode_of_a_run_is_the_settings_or_enhanced_with_the_flag() {
    for mode in GraphicsMode::ALL {
        let s = Settings::from_text(&format!(
            "graphics={}\ngraphics_api=vulkan\n",
            mode.as_str()
        ));
        assert_eq!(resolved_graphics(&s, false), mode);
        // (a Vanilla drawn by Direct3D 9Ex, on Windows, has its window before the flag is read)
        let flagged = if cfg!(windows) && mode == GraphicsMode::Vanilla {
            GraphicsMode::Vanilla
        } else {
            GraphicsMode::Enhanced
        };
        assert_eq!(resolved_graphics(&s, true), flagged, "{mode:?}");
    }
}

#[test]
fn a_saved_directx9_api_runs_vanilla() {
    // (a saved `graphics_api=d3d9` is Vanilla on Windows, whatever `graphics` says; elsewhere
    // it is Auto and the mode stays)
    for mode in GraphicsMode::ALL {
        let s = Settings::from_text(&format!("graphics={}\ngraphics_api=d3d9\n", mode.as_str()));
        for flag in [false, true] {
            let want = if cfg!(windows) {
                GraphicsMode::Vanilla
            } else if flag {
                GraphicsMode::Enhanced
            } else {
                mode
            };
            assert_eq!(resolved_graphics(&s, flag), want, "{mode:?} {flag}");
        }
    }
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
        "OMSI_NO_RT=1\nOMSI_NO_RT_GRADE=1\nOMSI_ENHANCED_PLUS=1\nOMSI_NO_ENHANCED=1\nOMSI_NO_SHADOWS=1\n",
        &mut skipped,
    );
    assert_eq!(got, vec![("OMSI_NO_SHADOWS".into(), "1".into())]);
    assert_eq!(
        skipped,
        vec![
            "OMSI_NO_RT",
            "OMSI_NO_RT_GRADE",
            "OMSI_ENHANCED_PLUS",
            "OMSI_NO_ENHANCED"
        ]
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

/// What the renderer leaves out is set once, here, from the settings and the environment
/// (`OMSI_NO_*`): with none of them set, nothing is left out, and the occlusion and the FXAA
/// follow the settings.
#[test]
fn the_render_options_draw_everything_the_environment_does_not_leave_out() {
    let mut s = Settings::default();
    let o = s.render_options();
    assert!(o.sun_shadows && o.coronas && o.smoke && o.snowfall);
    assert!(o.glass_picture && o.puddle_reflections && !o.mirror_enhanced);
    s.ssao = false;
    assert!(!s.render_options().ssao);
    s.ssao = true;
    assert!(s.render_options().ssao);
}
