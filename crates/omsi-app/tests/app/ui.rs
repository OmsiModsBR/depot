use ab_glyph::{Font, FontVec};
use openomsi_game::testing;
use openomsi_game::testing::ui::*;

/// The information bar is broken between its groups into rows that fit the room it has,
/// as many groups to a row as go in; a group wider than the room alone is a row of its
/// own (#1164).
#[test]
fn the_information_bar_breaks_into_rows_that_fit() {
    let widths = [80.0, 60.0, 210.0, 90.0, 120.0, 150.0, 40.0];
    assert_eq!(
        info_rows(&widths, 20.0, 5000.0),
        vec![(0..7).collect::<Vec<_>>()]
    );
    let rows = info_rows(&widths, 20.0, 400.0);
    assert_eq!(rows, vec![vec![0, 1, 2], vec![3, 4, 5], vec![6]]);
    for r in &rows {
        let used: f32 = r.iter().map(|k| widths[*k]).sum::<f32>() + 20.0 * (r.len() - 1) as f32;
        assert!(used <= 400.0);
    }
    assert_eq!(info_rows(&widths, 20.0, 50.0).len(), 7);
    assert!(info_rows(&[], 20.0, 50.0).is_empty());
}

#[test]
fn the_frame_rate_dot_follows_the_rate() {
    assert_eq!(fps_level(144.0), 0);
    assert_eq!(fps_level(50.0), 0);
    assert_eq!(fps_level(49.0), 1);
    assert_eq!(fps_level(30.0), 1);
    assert_eq!(fps_level(12.0), 2);
    assert_eq!(fps_dot(60.0), 13);
    assert_eq!(fps_dot(12.0), 15);
}

#[test]
fn a_server_notice_is_read_and_fades() {
    let (id, n) = Notice::parse("a1b2 5 warn Service 15/5: departure at 06:07 | depot").unwrap();
    assert_eq!(id, "a1b2");
    assert_eq!(
        (n.kind, n.text.as_str(), n.total, n.left),
        (
            NoticeKind::Warn,
            "Service 15/5: departure at 06:07 | depot",
            5.0,
            5.0
        )
    );
    // seconds held to 2 .. 30, an unknown kind is information
    assert_eq!(Notice::parse("x 0.5 info hi").unwrap().1.total, 2.0);
    assert_eq!(
        Notice::parse("x 600 whatever hi").unwrap().1,
        Notice {
            kind: NoticeKind::Info,
            text: "hi".into(),
            left: 30.0,
            total: 30.0
        }
    );
    // no text, no seconds, nothing: not a notice
    for bad in [
        "",
        "x",
        "x 5",
        "x 5 info",
        "x 5 info   ",
        "x five info hi",
        "x NaN info hi",
    ] {
        assert!(Notice::parse(bad).is_none(), "{bad}");
    }
    // fades in, stays, fades out
    let mut n = Notice::parse("x 5 alert hi").unwrap().1;
    assert_eq!(n.alpha(), 0.0);
    n.left = 4.0;
    assert_eq!(n.alpha(), 1.0);
    n.left = 0.25;
    assert!((n.alpha() - 0.5).abs() < 1e-6);
    // three at most: the oldest goes
    let mut list = Vec::new();
    for k in 0..5 {
        push_notice(
            &mut list,
            Notice::parse(&format!("{k} 5 info n{k}")).unwrap().1,
        );
    }
    assert_eq!(
        list.iter().map(|n| n.text.as_str()).collect::<Vec<_>>(),
        ["n2", "n3", "n4"]
    );
}

#[test]
fn centre_hud_moves_menu_controls_and_chat_hitboxes_together() {
    let mut ui = Ui::new().unwrap();
    ui.menu_rects.push([20.0, 100.0, 900.0, 140.0]);
    ui.menu_ctl.push(Some([600.0, 110.0, 860.0, 130.0]));
    ui.menu_side.push([30.0, 100.0, 200.0, 140.0]);
    ui.dd_rects.push([650.0, 150.0, 850.0, 180.0]);
    ui.menu_arrows.push(Some([600.0, 640.0, 820.0]));
    ui.chat.rect = [10.0, 800.0, 400.0, 1000.0];
    ui.dd_scroll = Some(([850.0, 150.0, 860.0, 300.0], [844.0, 160.0, 864.0, 200.0]));
    ui.menu_pane_scroll = Some((
        [880.0, 150.0, 890.0, 700.0],
        [874.0, 160.0, 894.0, 220.0],
        40,
        10,
    ));
    ui.shift_hitboxes(1920.0);
    let track = ui.menu_ctl[0].unwrap();
    assert_eq!((2650.0 - track[0]) / (track[2] - track[0]), 0.5);
    assert!(ui.chat.contains(2020.0, 900.0));
    assert!(!ui.chat.contains(100.0, 900.0));
    assert_eq!(ui.dd_rects[0], [2570.0, 150.0, 2770.0, 180.0]);
    assert_eq!(ui.menu_arrows[0], Some([2520.0, 2560.0, 2740.0]));
    assert_eq!(ui.dd_scroll.unwrap().0[0], 2770.0);
    assert_eq!(ui.menu_pane_scroll.unwrap().1[0], 2794.0);
    ui.shift_hitboxes(-1920.0);
    assert_eq!(ui.menu_ctl[0], Some([600.0, 110.0, 860.0, 130.0]));
}

#[test]
fn a_long_chat_line_goes_on_in_rows() {
    // (10 px a character)
    let m = |t: &str| t.chars().count() as f32 * 10.0;
    // fits: one row, as it was
    assert_eq!(wrap_rows("Anton: hello", 200.0, 20.0, &m), ["Anton: hello"]);
    assert_eq!(wrap_rows("", 200.0, 20.0, &m), [""]);
    // between words; the rows after the first are 20 px narrower
    let rows = wrap_rows(
        "Admin (private): take tour 13/1 at 04:47 from the depot",
        200.0,
        20.0,
        &m,
    );
    assert_eq!(
        rows,
        [
            "Admin (private):",
            "take tour 13/1 at",
            "04:47 from the",
            "depot"
        ]
    );
    assert!(rows[0].chars().count() <= 20 && rows[1..].iter().all(|r| r.chars().count() <= 18));
    // nothing lost: the words come back in order
    assert_eq!(
        rows.join(" "),
        "Admin (private): take tour 13/1 at 04:47 from the depot"
    );
    // a word longer than a row is cut inside
    let rows = wrap_rows("x: https://example.org/a/very/long/link", 100.0, 20.0, &m);
    assert_eq!(
        rows,
        ["x:", "https://", "example.", "org/a/ve", "ry/long/", "link"]
    );
    assert_eq!(rows.concat(), "x:https://example.org/a/very/long/link");
    // a box narrower than a character still ends (a character a row)
    assert_eq!(wrap_rows("abc", 5.0, 0.0, &m), ["a", "b", "c"]);
}

#[test]
fn vr_settings_sidebar_keeps_back_clear() {
    for scale in [0.5, 1.0, 2.0] {
        for height in [220.0, 440.0] {
            let pages_top = (72.0 + 6.0) * scale;
            let back_top = (height - PAD - 6.0 - 38.0) * scale;
            for pages in [7, 8, 10] {
                let step =
                    vr_settings_sidebar_step(back_top - 6.0 * scale - pages_top, pages, scale);
                let last_bottom =
                    pages_top + (pages - 1) as f32 * step + (step - 4.0 * scale).max(1.0);
                assert!(last_bottom < back_top);
            }
        }
    }
    assert_eq!(vr_settings_sidebar_step(1000.0, 8, 1.0), 42.0);
}

#[test]
fn text_renders_with_an_outline() {
    let f = FontVec::try_from_vec(ROBOTO.to_vec()).unwrap();
    let img = render_text(&f, "Savva: hi", 20.0, [255, 255, 255, 220]);
    assert!(img.width > 40 && img.height > 14);
    // white text and dark outline pixels are both there
    let px: Vec<&[u8]> = img.rgba.chunks(4).collect();
    assert!(px.iter().any(|p| p[3] > 200 && p[0] > 200));
    assert!(px.iter().any(|p| p[3] > 100 && p[0] < 40));
}

/// The Esc menu in Chinese, Korean and Thai: the system's fonts, not boxes.
#[test]
fn scripts_roboto_lacks_come_from_the_system() {
    let f = FontVec::try_from_vec(ROBOTO.to_vec()).unwrap();
    for t in ["继续", "繼續", "계속", "ดำเนินการต่อ"] {
        if t.chars()
            .next()
            .and_then(omsi_ui::text::fallback_font)
            .is_none()
        {
            continue;
        }
        for c in t.chars() {
            let g = font_for(&f, c);
            assert!(!std::ptr::eq(g, &f) && g.glyph_id(c).0 != 0, "{t}: {c}");
        }
        let img = render_text(&f, t, 20.0, [255, 255, 255, 220]);
        let ink = img
            .rgba
            .chunks(4)
            .filter(|p| p[3] > 128 && p[0] > 128)
            .count();
        assert!(ink > 30, "{t}: {ink}");
    }
}

/// Laid out for 1080p: a lower window as it always was, a taller one in proportion.
#[test]
fn the_interface_grows_with_tall_windows() {
    assert_eq!(size_factor(900.0, 1.0, 1.0, true), 1.0);
    assert_eq!(size_factor(1080.0, 1.0, 1.0, true), 1.0);
    assert_eq!(size_factor(2160.0, 1.0, 1.0, true), 2.0);
    assert_eq!(size_factor(4320.0, 1.0, 1.0, true), 2.0);
    // (4K at 150 % display scaling is 1440 logical lines: with the screen's scale the
    // same size as at 100 %)
    assert!((1.5 * size_factor(2160.0, 1.5, 1.0, true) - 2.0).abs() < 1e-5);
    assert_eq!(size_factor(1080.0, 1.0, 1.5, true), 1.5);
    assert_eq!(size_factor(2160.0, 1.0, 0.5, true), 1.0);
    // (switched off: the size alone)
    assert_eq!(size_factor(2160.0, 1.0, 1.0, false), 1.0);
    assert_eq!(size_factor(2160.0, 1.0, 1.25, false), 1.25);
}

/// A thinned panel's light texts get an outline; dark ones - the highlighted menu line's,
/// on the solid accent - do not (it came out as a smear round them, like a shadow).
#[test]
fn panel_texts_get_an_outline_only_where_it_helps() {
    // (full panels: as asked)
    assert_eq!(outline_for([235, 235, 235, 0], 1.0), 0);
    // (thinned: the light text gets one, the dark text none)
    assert!(outline_for([235, 235, 235, 0], 0.47) > 100);
    assert!(outline_for([140, 140, 140, 0], 0.47) > 100);
    assert_eq!(outline_for([20, 20, 20, 0], 0.47), 0);
    // (texts with an outline of their own keep it)
    assert_eq!(outline_for([255, 255, 255, 235], 0.47), 235);
}

/// The opacity's default is the design; below it the backgrounds fade, never quite away.
#[test]
fn backgrounds_follow_the_opacity_setting() {
    assert_eq!(backdrop(0.85), 1.0);
    assert!((backdrop(0.425) - 0.5).abs() < 1e-6);
    assert_eq!(backdrop(0.2), 0.3);
    assert!(backdrop(1.0) > 1.0);
}

#[test]
fn the_game_menu_grows_with_the_interface_size_as_far_as_it_fits() {
    // (it ignored the setting: 1.5 drew the menu as 1.0 while the HUD grew)
    assert_eq!(
        testing::ui::menu_scale_for(1.0, 1.0, 1280.0, 720.0, false),
        1.0
    );
    let s = testing::ui::menu_scale_for(1.0, 1.5, 1280.0, 720.0, false);
    assert!(
        s > 1.1 && s < 1.5 && 880.0 * s + 24.0 <= 1280.0 && 600.0 * s <= 720.0 * 0.94 + 0.01,
        "{s}"
    );
    assert_eq!(
        testing::ui::menu_scale_for(1.0, 1.5, 3840.0, 2160.0, false),
        1.5
    );
    // never smaller than it always was, and the headset keeps its own size
    assert_eq!(
        testing::ui::menu_scale_for(1.0, 1.5, 800.0, 500.0, false),
        1.0
    );
    assert_eq!(
        testing::ui::menu_scale_for(1.0, 1.5, 1920.0, 1080.0, true),
        1.0
    );
}

#[test]
fn destinations_split_into_code_and_name() {
    assert_eq!(
        testing::ui::code_and_name(" 13  BETRIEBSFAHRT"),
        Some(("13", "BETRIEBSFAHRT"))
    );
    assert_eq!(
        testing::ui::code_and_name("1001  Blanko.tga"),
        Some(("1001", "Blanko.tga"))
    );
    assert_eq!(testing::ui::code_and_name("Route number: 5..."), None);
    assert_eq!(testing::ui::code_and_name("Line 76  (1 tours)"), None);
}

#[test]
fn chat_filter_stars_out_swearing() {
    assert_ne!(
        filter_chat("you are a fucking idiot"),
        "you are a fucking idiot"
    );
    assert_eq!(
        filter_chat("next stop Rathaus Spandau"),
        "next stop Rathaus Spandau"
    );
}
