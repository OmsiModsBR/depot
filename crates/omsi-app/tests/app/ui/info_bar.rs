use openomsi_game::testing;
use openomsi_game::testing::hud::{InfoBar, InfoCell, InfoItem, InfoTone, MenuKind};
use openomsi_game::testing::ui::pages::test_frame;

fn item(text: &str, tone: InfoTone) -> InfoItem {
    InfoItem {
        text: text.into(),
        tone,
        raw: true,
    }
}

fn cell(label: &str, value: &str) -> InfoCell {
    InfoCell {
        items: vec![item(label, InfoTone::Dim), item(value, InfoTone::Plain)],
        ..Default::default()
    }
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
    };
    InfoBar {
        groups: vec![
            vec![item("21:04:10", InfoTone::Plain)],
            vec![
                item("58", InfoTone::Plain),
                item("km/h", InfoTone::Dim),
                item("60", InfoTone::Limit),
            ],
            vec![
                item("829", InfoTone::Badge),
                item("Terminal Sul", InfoTone::Plain),
            ],
            vec![
                item("next: Av. Paulista", InfoTone::Plain),
                item("−0:40", InfoTone::Early),
            ],
        ],
        bottom,
        tier2: if collapsed {
            Vec::new()
        } else {
            vec![
                cell("Trip", "18.2 km"),
                cell("Odometer", "412.9 km"),
                cell("Outside / cabin", "9 °C / 19 °C"),
                cell("Passengers", "104 / 111"),
                tank,
            ]
        },
        split,
        tiered: true,
        collapsed,
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
    let cases = [
        ("twotier-top", bar(false, false, false, false)),
        ("split-top", bar(true, false, false, false)),
        ("twotier-bottom-lowfuel", bar(false, true, false, true)),
        ("twotier-collapsed", bar(false, false, true, false)),
    ];
    let mut heights = Vec::new();
    for (name, info) in cases {
        let mut scene = r.new_scene();
        let mut hud = testing::hud::Ui::new().unwrap();
        let mut f = test_frame(&[], MenuKind::Options);
        f.width = size.0 as f32;
        f.height = size.1 as f32;
        f.menu = None;
        f.paused = false;
        f.menu_head = None;
        f.menu_kbd = false;
        let (collapsed, bottom) = (info.collapsed, info.bottom);
        f.info = Some(info);
        for _ in 0..2 {
            scene.overlays.clear();
            hud.draw(&r, &mut scene, &f, 0.1);
        }
        let chevron = hud.hits.info_chevron.expect("the arrow is there");
        let rect = hud.hits.info_rect.expect("the bar is there");
        assert!(
            chevron[0] > rect[0] && chevron[2] <= rect[2],
            "{chevron:?} {rect:?}"
        );
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
    // (folded away, the bar is just the first tier: shorter than with the second)
    assert!(heights[3].1 < heights[0].1);
}
