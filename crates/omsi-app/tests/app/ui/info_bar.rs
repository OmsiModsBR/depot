use openomsi_game::testing;
use openomsi_game::testing::hud::{InfoBar, InfoCell, InfoItem, InfoTone, MenuKind};
use openomsi_game::testing::ui::pages::test_frame;

fn item(text: &str, tone: InfoTone) -> InfoItem {
    InfoItem {
        text: text.into(),
        tone,
        raw: true,
        icon: None,
        tip: None,
        clip: false,
    }
}

fn with_icon(mut it: InfoItem, icon: &'static str, tip: &str) -> InfoItem {
    it.icon = Some(icon);
    it.tip = Some(tip.into());
    it
}

fn cell(label: &str, value: &str, icon: &'static str) -> InfoCell {
    InfoCell {
        items: vec![item(label, InfoTone::Dim), item(value, InfoTone::Plain)],
        icon: Some(icon),
        tip: Some(label.into()),
        ..Default::default()
    }
}

/// The line's badge and where it goes (cut short when long, whole in its tooltip).
fn route(line: &str, terminus: &str) -> Vec<InfoItem> {
    let mut to = item(terminus, InfoTone::Plain);
    to.clip = true;
    to.tip = Some(terminus.into());
    vec![item(line, InfoTone::Badge), to]
}

/// A two-tier bar as the game builds one, without the vehicle.
fn bar(split: bool, bottom: bool, collapsed: bool, low: bool) -> InfoBar {
    let tank = InfoCell {
        items: vec![
            item("Tank", InfoTone::Dim),
            item(
                if low { "12 %" } else { "99 %" },
                if low { InfoTone::Late } else { InfoTone::Plain },
            ),
        ],
        meter: Some(if low { 0.12 } else { 0.99 }),
        alert: low,
        icon: Some("hud-fuel"),
        tip: Some("Tank".into()),
    };
    InfoBar {
        groups: vec![
            route("829", "Terminal Sul"),
            vec![with_icon(
                item("21:04:10", InfoTone::Plain),
                "hud-time",
                "Current time",
            )],
            vec![
                item("58", InfoTone::Speed),
                item("km/h", InfoTone::Dim),
                item("60", InfoTone::Limit),
            ],
            vec![
                with_icon(
                    item("Av. Paulista", InfoTone::Plain),
                    "hud-stop",
                    "Next stop",
                ),
                item("−1:40", InfoTone::Delay(-1)),
            ],
        ],
        bottom,
        tier2: if collapsed {
            Vec::new()
        } else {
            vec![
                cell("Trip", "18.2 km", "hud-trip"),
                cell("Odometer", "18\u{2009}427.3 km", "hud-odometer"),
                cell("Outside / cabin", "9 / 19 °C", "hud-temperature"),
                cell("Passengers", "104 / 111", "hud-passengers"),
                tank,
            ]
        },
        split,
        tiered: true,
        collapsed,
        large: false,
    }
}

/// A one-row layout as the game builds one: Full (`full`) with every readout, else Compact
/// (the badge and the next stop as one group) or Essentials (`large`).
fn one_row(full: bool, large: bool) -> InfoBar {
    let clock = with_icon(
        item("21:04:10", InfoTone::Plain),
        "hud-time",
        "Current time",
    );
    let speed = vec![
        item("58", InfoTone::Speed),
        item("km/h", InfoTone::Dim),
        item("60", InfoTone::Limit),
    ];
    let next = with_icon(
        item("Av. Paulista", InfoTone::Plain),
        "hud-stop",
        "Next stop",
    );
    let late = item("+3:12", InfoTone::Delay(1));
    let groups = if large {
        vec![speed, vec![late]]
    } else if full {
        vec![
            vec![clock],
            speed,
            vec![with_icon(
                item("18.2 km", InfoTone::Plain),
                "hud-trip",
                "Trip",
            )],
            vec![with_icon(
                item("18\u{2009}427.3 km", InfoTone::Plain),
                "hud-odometer",
                "Odometer",
            )],
            vec![with_icon(
                item("9 / 19 °C", InfoTone::Plain),
                "hud-temperature",
                "Outside / cabin",
            )],
            vec![with_icon(item("82 %", InfoTone::Plain), "hud-fuel", "Tank")],
            vec![with_icon(
                item("0 / 40", InfoTone::Plain),
                "hud-passengers",
                "Passengers",
            )],
            route("829", "Terminal Sul"),
            vec![next, late],
            vec![item("74 fps", InfoTone::Fps(0))],
        ]
    } else {
        vec![
            vec![clock],
            speed,
            vec![
                item("829", InfoTone::Badge),
                next,
                item("+0:20", InfoTone::Delay(0)),
            ],
            vec![item("74 / 52 fps", InfoTone::Fps(0))],
        ]
    };
    InfoBar {
        groups,
        large,
        ..Default::default()
    }
}

/// The information bar's items say what they are under the mouse; a route cut short says
/// itself in full, one that fits says nothing more.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_information_bars_items_have_tooltips_and_a_long_route_is_cut_short() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(r) = pollster::block_on(omsi_render::Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        omsi_render::RenderOptions {
            msaa: 1,
            ssao: false,
            shadow_size: 1024,
            fxaa: false,
            render_scale: 1.0,
            ..Default::default()
        },
    )) else {
        eprintln!("no graphics adapter: skipped");
        return;
    };
    let long = "Terminal Rodoviário Intermunicipal Professor Doutor Ernesto Geisel via Centro";
    for (terminus, cut) in [("Terminal Sul", false), (long, true)] {
        let mut scene = r.new_scene();
        let mut hud = testing::hud::Ui::new().unwrap();
        let mut f = test_frame(&[], MenuKind::Options);
        f.width = 1280.0;
        f.height = 360.0;
        f.menu = None;
        f.paused = false;
        f.menu_head = None;
        let mut info = bar(false, false, false, false);
        info.groups[0] = route("829", terminus);
        f.info = Some(info);
        hud.draw(&r, &mut scene, &f, 0.1);
        let tips: Vec<&str> = hud.hits.info_tips.iter().map(|t| t.1.as_str()).collect();
        for name in ["Current time", "Next stop", "Trip", "Odometer", "Tank"] {
            assert!(tips.contains(&name), "{name}: {tips:?}");
        }
        assert_eq!(tips.contains(&terminus), cut, "{tips:?}");
        // (cut short, the bar still fits the window)
        let rect = hud.hits.info_rect.unwrap();
        assert!(rect[2] - rect[0] < 1280.0 - 24.0, "{rect:?}");
        for (r, _) in &hud.hits.info_tips {
            assert!(r[0] >= rect[0] && r[2] <= rect[2], "{r:?} {rect:?}");
        }
    }
}

/// The two-tier layouts draw: the first tier, the arrow (whose rectangle the mouse reads back),
/// the second tier above or under it. Set `DEPOT_INFO_BAR_PNG` to a folder to keep the pictures.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_two_tier_information_bar_draws_with_its_arrow() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let options = omsi_render::RenderOptions {
        msaa: 1,
        ssao: false,
        shadow_size: 1024,
        fxaa: false,
        render_scale: 1.0,
        ..Default::default()
    };
    let Ok(mut r) = pollster::block_on(omsi_render::Renderer::new_with(
        &instance,
        None,
        Some(wgpu::TextureFormat::Rgba8UnormSrgb),
        options,
    )) else {
        eprintln!("no graphics adapter: skipped");
        return;
    };
    let camera = omsi_render::Camera {
        position: glam::DVec3::new(0.0, 0.0, 2.0),
        yaw: 0.0,
        pitch: 0.0,
        roll: 0.0,
        fov_deg: 60.0,
        near: 0.1,
        far: 2000.0,
    };
    let lighting = omsi_render::Lighting {
        shadows: false,
        ..Default::default()
    };
    let out = std::env::var("DEPOT_INFO_BAR_PNG").ok();
    let size = (1280u32, 360u32);
    use omsi_ui::theme::{set_accent, Accent};
    let long = {
        let mut b = bar(false, false, false, false);
        b.groups[0] = route(
            "829",
            "Terminal Rodoviário Intermunicipal Professor Doutor Ernesto Geisel via Centro",
        );
        b
    };
    let cases = [
        (
            "twotier-top",
            bar(false, false, false, false),
            Accent::Amber,
        ),
        ("split-top", bar(true, false, false, false), Accent::Amber),
        (
            "twotier-bottom-lowfuel",
            bar(false, true, false, true),
            Accent::Amber,
        ),
        (
            "twotier-collapsed",
            bar(false, false, true, false),
            Accent::Amber,
        ),
        ("twotier-long-names", long, Accent::Amber),
        (
            "twotier-accent-blue",
            bar(false, false, false, false),
            Accent::Blue,
        ),
        (
            "twotier-accent-purple",
            bar(true, false, false, false),
            Accent::Purple,
        ),
        ("full-top", one_row(true, false), Accent::Amber),
        ("compact-top", one_row(false, false), Accent::Amber),
        ("essentials-top", one_row(false, true), Accent::Amber),
    ];
    let mut heights = Vec::new();
    for (name, info, accent) in cases {
        set_accent(accent);
        let mut scene = r.new_scene();
        let mut hud = testing::hud::Ui::new().unwrap();
        let mut f = test_frame(&[], MenuKind::Options);
        f.width = size.0 as f32;
        f.height = size.1 as f32;
        f.menu = None;
        f.paused = false;
        f.menu_head = None;
        f.menu_kbd = false;
        let (collapsed, bottom, tiered) = (info.collapsed, info.bottom, info.tiered);
        f.info = Some(info);
        for _ in 0..2 {
            scene.overlays.clear();
            hud.draw(&r, &mut scene, &f, 0.1);
        }
        let rect = hud.hits.info_rect.expect("the bar is there");
        if tiered {
            let chevron = hud.hits.info_chevron.expect("the arrow is there");
            assert!(
                chevron[0] > rect[0] && chevron[2] <= rect[2],
                "{chevron:?} {rect:?}"
            );
        } else {
            assert!(hud.hits.info_chevron.is_none());
        }
        assert!(rect[0] >= 0.0 && rect[2] <= size.0 as f32, "{rect:?}");
        assert!(rect[1] >= 0.0 && rect[3] <= size.1 as f32, "{rect:?}");
        if bottom {
            assert!(rect[3] > size.1 as f32 * 0.9, "{rect:?}");
        }
        heights.push((collapsed, rect[3] - rect[1]));
        if let Some(dir) = out.as_ref() {
            let img = r
                .render_to_image(&mut scene, size.0, size.1, &camera, &lighting)
                .unwrap();
            std::fs::create_dir_all(dir).unwrap();
            image::save_buffer(
                format!("{dir}/{name}.png"),
                &img,
                size.0,
                size.1,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        }
    }
    set_accent(Accent::Amber);
    // (folded away, the bar is just the first tier: shorter than with the second)
    assert!(heights[3].1 < heights[0].1);
}
