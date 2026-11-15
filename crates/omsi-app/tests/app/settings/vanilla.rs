//! Vanilla is the Direct3D 9Ex renderer (spec 001, phase A5, `contracts/settings-cfg.md`): a
//! saved `graphics_api=d3d9` opens Vanilla, `graphics_api` keeps only the wgpu interfaces,
//! Vanilla opens without VR and the triple screen (kept in the file), and its renderer's
//! limits (`api_limits`) are applied to what it loads.

use openomsi_game::testing::settings::*;

/// The limits of the table are what `fit_vanilla` sets, option by option; a new table entry
/// needs its check here.
#[test]
fn the_vanilla_limits_set_every_option_of_the_table() {
    let mut s = Settings::default();
    s.msaa = 8;
    s.anisotropy = 16;
    s.render_scale = 0.5;
    s.mirror_size = 64;
    s.mirror_refresh = "eco".into();
    s.mirror_hud = 3;
    s.windy_trees = true;
    s.night_brightness = 1.0;
    s.clouds = false;
    s.fit_vanilla();
    for l in openomsi_game::testing::api_limits::D3D9 {
        let ok = match l.key {
            // (the player's choice, held to the scales the table offers)
            "render_scale" => s.render_scale == 0.5 && l.allows("0.5"),
            "mirror_size" => {
                s.mirror_size == 128 && l.allows(&s.mirror_size.to_string()) && s.mirror_hud == 0
            }
            "mirror_refresh" => s.mirror_refresh == "full" && l.allows(&s.mirror_refresh),
            "windy_trees" => !s.windy_trees,
            "clouds" => s.clouds,
            "shadows" => !s.shadows,
            "ssao" => !s.ssao,
            "detail_textures" => !s.detail_textures,
            "night_brightness" => s.night_brightness == 0.0,
            // (hidden, and with nothing of their own to switch off under Vanilla)
            "preset" | "shadow_size" | "shadow_casters" | "led_glow" | "led_mips" => true,
            other => panic!("{other} is in the table but `fit_vanilla` is not checked for it"),
        };
        assert!(ok, "{} is not set by the limits", l.key);
    }
    // (the mode is the player's: the limits never choose it)
    let mut other = Settings::from_text("graphics=enhanced\n");
    other.fit_vanilla();
    assert_eq!(other.graphics, GraphicsMode::Enhanced);
}

#[test]
fn vanilla_takes_the_render_scale_saved_in_another_mode() {
    // (the same key serves every mode: a value saved in Vanilla+ or Enhanced carries over,
    // fitted to the steps Vanilla offers; Auto and 100 % leave the picture at the window's size)
    for (saved, want) in [
        ("0.5", 0.5),
        ("0.6", 0.67),
        ("0.7", 0.67),
        ("0.9", 0.85),
        ("75%", 0.75),
        ("auto", 0.0),
    ] {
        let mut s = Settings::from_text(&format!("graphics=vanilla\nrender_scale={saved}\n"));
        s.fit_vanilla();
        assert_eq!(s.render_scale, want, "saved {saved}");
        // (Vanilla is drawn by Direct3D 9Ex on Windows only, where its scale is its own)
        if cfg!(windows) {
            assert_eq!(s.dx9_render_scale(), want, "saved {saved}");
        }
    }
    let mut full = Settings::from_text("graphics=vanilla\nrender_scale=1\n");
    full.fit_vanilla();
    assert_eq!(full.dx9_render_scale(), 0.0);
}

#[test]
fn vanilla_has_no_graphics_improvements_to_choose() {
    let saved = "msaa=8\nanisotropy=16\nrender_scale=0.5\nmirror_size=1024\nmirror_refresh=eco\nmirror_hud=3\nwindy_trees=1\npost_aa=fxaa\nclouds=0\n";
    let loaded = Settings::from_text(&format!("graphics=vanilla\n{saved}")).fitted_to_mode();
    assert_eq!(loaded.graphics, GraphicsMode::Vanilla);
    assert!(!loaded.shadows && !loaded.ssao && !loaded.detail_textures);
    if cfg!(windows) {
        // what Vanilla does not draw yet is off, whatever was saved (`api_limits`);
        // anti-aliasing and anisotropic filtering are drawn: kept as saved
        assert_eq!(
            (loaded.msaa, loaded.anisotropy, loaded.render_scale),
            (8, 16, 0.5)
        );
        // (the render scale is Direct3D 9Ex's own: the picture it draws is scaled there, the
        // default renderer behind it is not told)
        assert_eq!(loaded.dx9_render_scale(), 0.5);
        assert_eq!(loaded.render_options().render_scale, 0.0);
        // (mirrors are drawn: a saved value Vanilla does not offer is fitted)
        assert_eq!(
            (
                loaded.mirror_size,
                loaded.mirror_refresh.as_str(),
                loaded.mirror_hud
            ),
            (512, "full", 0)
        );
        // (what it does draw stays as saved: env maps and shadow meshes)
        assert!(loaded.reflections && loaded.shadow_blobs);
        assert_eq!(
            (loaded.windy_trees, loaded.post_aa.as_str()),
            (false, "off")
        );
        // (clouds are on, as OMSI 2 has the weather's own: a saved "off" is not taken)
        assert!(loaded.clouds);
    }
    // the other modes keep every choice
    for mode in [GraphicsMode::VanillaPlus, GraphicsMode::Enhanced] {
        let chosen = Settings::from_text(&format!("graphics={}\n{saved}", mode.as_str()));
        assert_eq!(chosen.clone().fitted_to_mode(), chosen, "{mode:?}");
    }
}

/// A saved `graphics_api=d3d9` (any spelling) is `graphics=vanilla` on Windows, whatever
/// `graphics` said, with `graphics_api=auto`; elsewhere the API is Auto and the mode stays.
#[test]
fn a_saved_directx9_api_is_read_as_vanilla() {
    for api in ["d3d9", "D3D9Ex", "dx9"] {
        for mode in GraphicsMode::ALL {
            let s =
                Settings::from_text(&format!("graphics={}\ngraphics_api={api}\n", mode.as_str()));
            let want = if cfg!(windows) {
                GraphicsMode::Vanilla
            } else {
                mode
            };
            assert_eq!(
                (s.graphics, s.graphics_api.as_str()),
                (want, "auto"),
                "{api} {mode:?}"
            );
            // (written so when the player saves; "d3d9" is never written again)
            let again = Settings::from_text(&s.to_text());
            assert_eq!(
                (again.graphics, again.graphics_api.as_str()),
                (want, "auto")
            );
        }
    }
}

/// `graphics_api` takes the wgpu interfaces only, kept through a save in every mode.
#[test]
fn the_graphics_api_keeps_the_wgpu_interfaces() {
    for api in ["auto", "vulkan", "dx12", "gl"] {
        for mode in GraphicsMode::ALL {
            let s =
                Settings::from_text(&format!("graphics={}\ngraphics_api={api}\n", mode.as_str()));
            assert_eq!((s.graphics, s.graphics_api.as_str()), (mode, api));
            assert_eq!(Settings::from_text(&s.to_text()).graphics_api, api);
        }
    }
    assert_eq!(Settings::default().graphics_api, "auto");
}

/// Vanilla opens without VR and the triple screen, says so, and keeps both in the file for the
/// other modes (FR-018a).
#[test]
fn vanilla_opens_without_vr_and_the_triple_screen() {
    let s = Settings::from_text("graphics=vanilla\nvr=1\ntriple_screen=1\n");
    assert!(!s.vr_requested());
    assert!(!s.triple_screen());
    assert!(s
        .left_out_by_vanilla()
        .is_some_and(|l| l.contains("VR") && l.contains("triple")));
    assert_eq!(s.hud_viewport((5760, 1080)), [0.0, 0.0, 5760.0, 1080.0]);
    let again = Settings::from_text(&s.to_text());
    assert!(again.vr && again.triple.enabled);
    // one of them alone
    let triple = Settings::from_text("graphics=vanilla\ntriple_screen=1\n");
    assert!(triple
        .left_out_by_vanilla()
        .is_some_and(|l| !l.contains("VR")));
    // the other modes apply them
    let plus = Settings::from_text("graphics=vanilla_plus\ntriple_screen=1\n");
    assert!(plus.triple_screen());
    assert!(plus.left_out_by_vanilla().is_none());
    let vr = Settings::from_text("graphics=enhanced\nvr=1\n");
    assert_eq!(vr.vr_requested(), cfg!(windows));
    assert!(Settings::from_text("graphics=vanilla\n")
        .left_out_by_vanilla()
        .is_none());
}
