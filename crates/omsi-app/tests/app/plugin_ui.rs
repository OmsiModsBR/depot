use glam::Vec2;
use omsi_plugin::ui::Panel;
use omsi_plugin::ui::Rgba;
use omsi_plugin::ui::Toast;
use omsi_plugin::ui::{panel_from_source, Anchor};
use omsi_ui::Atlas;
use omsi_ui::Fonts;
use omsi_ui::Gpu;
use omsi_ui::Rect;
use openomsi_game::testing::plugin_ui::*;

fn panel(src: &str) -> Panel {
    panel_from_source(src).unwrap()
}

#[test]
fn a_stack_wraps_its_texts_and_grows_down() {
    let fonts = Fonts::new();
    let p = panel(
        r#"{ width = 200, padding = 10, gap = 5, children = {
        { type = "text", text = "Ein ziemlich langer Satz, der umbrochen werden muss", size = 14 },
        { type = "divider" },
        { type = "button", id = "go", text = "Los" },
    } }"#,
    );
    let l = layout(&p, &fonts, 1.0);
    let texts = l
        .items
        .iter()
        .filter(|i| matches!(i, Item::Text { .. }))
        .count();
    // several lines and the button's label
    assert!(texts >= 3, "{:?}", l.items);
    let lines = texts - 1;
    let want = 10.0 + lines as f32 * 14.0 * LINE + 5.0 + 1.0 + 5.0 + BUTTON_H + 10.0;
    assert!((l.h - want.ceil()).abs() < 1.0, "{} {want}", l.h);
    for i in &l.items {
        if let Item::Text {
            text,
            px,
            weight,
            at,
            ..
        } = i
        {
            assert!(
                at.x >= 10.0 && at.x + fonts.width_as_is(text, *px, *weight) <= 190.5,
                "{text} at {at}"
            );
        }
    }
    // (the card's height is whole pixels, rounded up)
    let [b] = &l.hits[..] else {
        panic!("{:?}", l.hits)
    };
    assert_eq!(
        (b.r.x, b.r.w, b.r.h, b.element.as_deref()),
        (10.0, 180.0, BUTTON_H, Some("go"))
    );
    assert!(
        (b.r.bottom() + 10.0 - l.h).abs() < 1.0,
        "{:?} in {}",
        b.r,
        l.h
    );
}

#[test]
fn a_row_gives_the_rest_to_what_grows() {
    let fonts = Fonts::new();
    let p = panel(
        r#"{ width = 300, padding = 10, children = { { type = "row", gap = 8, children = {
        { type = "icon", name = "schedule", size = 20 },
        { type = "bar", value = 0.5, grow = true },
        { type = "badge", text = "+2" },
    } } } }"#,
    );
    let l = layout(&p, &fonts, 1.0);
    let rounded: Vec<Rect> = l
        .items
        .iter()
        .filter_map(|i| {
            if let Item::Rounded { r, .. } = i {
                Some(*r)
            } else {
                None
            }
        })
        .collect();
    // the track from after the icon to before the badge, the badge at the right edge
    let (track, badge) = (rounded[0], rounded[2]);
    assert_eq!(track.x, 10.0 + 20.0 + 8.0);
    assert!(
        (badge.right() - 290.0).abs() < 0.01 && (track.right() + 8.0 - badge.x).abs() < 0.01,
        "{track:?} {badge:?}"
    );
    assert_eq!(l.h, (10.0 + BADGE_H + 10.0f32).ceil());
}

#[test]
fn panels_are_placed_by_their_anchor_and_stay_on_the_screen() {
    let screen = Vec2::new(1920.0, 1080.0);
    let mut p = panel("{ x = 16, y = 20 }");
    assert_eq!(place(&p, 300.0, 100.0, screen), Vec2::new(16.0, 20.0));
    p.anchor = Anchor::BottomRight;
    assert_eq!(
        place(&p, 300.0, 100.0, screen),
        Vec2::new(1920.0 - 316.0, 1080.0 - 120.0)
    );
    p.anchor = Anchor::Center;
    assert_eq!(
        place(&p, 300.0, 100.0, screen),
        Vec2::new(810.0 + 16.0, 490.0 + 20.0)
    );
    p.anchor = Anchor::TopLeft;
    p.x = -50.0;
    assert_eq!(place(&p, 300.0, 100.0, screen).x, 0.0);
}

#[test]
fn clicks_find_the_innermost_part() {
    let fonts = Fonts::new();
    let p = panel(
        r#"{ width = 300, clickable = true, children = {
        { type = "row", id = "line", clickable = true, children = {
            { type = "text", text = "Linie 5", grow = true },
            { type = "button", id = "take", text = "Nehmen" },
        } },
        { type = "text", text = "nicht klickbar" },
    } }"#,
    );
    let l = layout(&p, &fonts, 1.0);
    let at = |i: usize| l.hits[i].r.center();
    let element = |p: Vec2| hit_at(&l, p).and_then(|i| l.hits[i].element.clone());
    let button = l
        .hits
        .iter()
        .position(|h| h.element.as_deref() == Some("take"))
        .unwrap();
    assert_eq!(element(at(button)).as_deref(), Some("take"));
    assert_eq!(
        element(Vec2::new(20.0, at(button).y)).as_deref(),
        Some("line")
    );
    // below the row: the panel itself
    assert_eq!(hit_at(&l, Vec2::new(20.0, l.h - 14.0)), Some(0));
    assert_eq!(element(Vec2::new(20.0, l.h - 14.0)), None);
}

/// A picture of a few panels and notifications over a stand-in for the road, drawn as the
/// game draws them (needs a graphics adapter):
/// `cargo test -p omsi-app --lib plugin_ui::tests::preview -- --ignored`, written to
/// `OMSI_UI_PREVIEW` or `target/ui-preview.png`.
#[test]
#[ignore]
fn preview() {
    let (w, h, k) = (1600u32, 900u32, 1.0f32);
    let instance = wgpu::Instance::default();
    let adapter =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
            .expect("adapter");
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("device");
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let mut gpu = Gpu::new(&device, format, 1, 1024);
    let fonts = Fonts::new();
    let mut atlas = Atlas::new(1024);
    atlas.begin_frame();
    let screen = Vec2::new(w as f32 / k, h as f32 / k);
    let panels = [
        (
            r##"{ anchor = "top_left", x = 16, y = 60, width = 340, accent = "#F47F30", children = {
                { type = "row", children = { { type = "icon", name = "directions_bus", color = "#F47F30" }, { type = "text", text = "Linie 42 · Kurs 3", size = 16, weight = "bold", grow = true }, { type = "badge", text = "+1:20", color = "#C62828" } } },
                { type = "text", text = "Nächster Halt: Grundorf, Krankenhaus Nord (Wendeschleife am Haupteingang)", color = "#C8C8C8" },
                { type = "row", gap = 6, children = { { type = "icon", name = "schedule", size = 16, color = "#8E8E8E" }, { type = "text", text = "ab 08:59", size = 13, color = "#8E8E8E" }, { type = "space", size = 8 }, { type = "icon", name = "group", size = 16, color = "#8E8E8E" }, { type = "text", text = "23 Fahrgäste", size = 13, color = "#8E8E8E" } } },
                { type = "bar", value = 0.62 },
            } }"##,
            None,
        ),
        (
            r##"{ anchor = "bottom_left", x = 16, y = 16, width = 260, children = {
                { type = "row", align = "between", children = { { type = "text", text = "Tagesverdienst", color = "#8E8E8E" }, { type = "text", text = "184,50 €", size = 18, weight = "bold" } } },
                { type = "divider" },
                { type = "row", align = "between", children = { { type = "text", text = "Pünktlichkeit" }, { type = "badge", text = "94 %", color = "#2E7D32" } } },
                { type = "row", align = "between", children = { { type = "text", text = "Fahrstil" }, { type = "badge", text = "B", color = "#F9A825" } } },
            } }"##,
            None,
        ),
        (
            r##"{ anchor = "center", width = 380, padding = 16, gap = 10, clickable = true, children = {
                { type = "text", text = "Schicht beendet", size = 20, weight = "bold", align = "center" },
                { type = "text", text = "Du hast 7 von 7 Fahrten gefahren. Möchtest du die nächste Schicht direkt annehmen?", align = "center", color = "#C8C8C8" },
                { type = "space", size = 4 },
                { type = "row", gap = 8, children = {
                    { type = "button", id = "later", text = "Später", grow = true },
                    { type = "button", id = "take", text = "Annehmen", icon = "check", color = "#E8A030", grow = true },
                } },
                { type = "button", id = "details", text = "Details", icon = "receipt_long" },
            } }"##,
            Some("details"),
        ),
    ];
    let mut first = true;
    for (src, hot) in panels {
        let p = panel(src);
        let laid = layout(&p, &fonts, 1.0);
        let at = place(&p, laid.w, laid.h, screen);
        let hot = hot.and_then(|id| {
            laid.hits
                .iter()
                .position(|h| h.element.as_deref() == Some(id))
        });
        draw_card(
            &mut gpu,
            &device,
            &queue,
            &mut atlas,
            &fonts,
            &view,
            (w, h),
            k,
            &laid,
            at,
            hot,
            first,
        );
        first = false;
    }
    let toasts = [
        Toast {
            owner: 1,
            serial: 1,
            text: "Fahrt 3 pünktlich beendet: +12,40 €".into(),
            title: Some("Karriere".into()),
            icon: Some("payments".into()),
            color: Some(Rgba([46, 125, 50, 255])),
            seconds: 5.0,
            age: 1.0,
        },
        Toast {
            owner: 1,
            serial: 2,
            text: "Rote Ampel überfahren".into(),
            title: None,
            icon: Some("warning".into()),
            color: Some(Rgba([198, 40, 40, 255])),
            seconds: 5.0,
            age: 1.0,
        },
        Toast {
            owner: 1,
            serial: 3,
            text: "Schichtbeginn in 10 Minuten am Betriebshof".into(),
            title: None,
            icon: None,
            color: None,
            seconds: 5.0,
            age: 1.0,
        },
    ];
    let mut y = TOAST_TOP;
    for t in &toasts {
        let laid = layout(&toast_panel(t), &fonts, 1.0);
        draw_card(
            &mut gpu,
            &device,
            &queue,
            &mut atlas,
            &fonts,
            &view,
            (w, h),
            k,
            &laid,
            Vec2::new(screen.x - 16.0 - laid.w, y),
            None,
            false,
        );
        y += laid.h + TOAST_GAP;
    }
    // read back and laid over a stand-in for the road: sky, buildings, the street
    let stride = (w * 4).div_ceil(256) * 256;
    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: (stride * h) as u64,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: None,
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    device.poll(wgpu::PollType::wait_indefinitely()).ok();
    let data = buf.slice(..).get_mapped_range();
    let mut img = image::RgbaImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let t = y as f32 / h as f32;
            let bg: [f32; 3] = if t < 0.45 {
                [120.0 + 80.0 * t, 165.0 + 60.0 * t, 225.0]
            } else if t < 0.7 && (x / 140) % 3 != 0 {
                [150.0 + (x % 140) as f32 * 0.3, 140.0, 128.0]
            } else if t < 0.7 {
                [205.0, 200.0, 190.0]
            } else {
                [70.0, 72.0, 76.0]
            };
            let i = (y * stride + x * 4) as usize;
            let a = data[i + 3] as f32 / 255.0;
            let px = |c: usize| (data[i + c] as f32 + bg[c] * (1.0 - a)).min(255.0) as u8;
            img.put_pixel(x, y, image::Rgba([px(0), px(1), px(2), 255]));
        }
    }
    let out = std::env::var("OMSI_UI_PREVIEW")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/ui-preview.png")
        });
    img.save(&out).unwrap();
    println!("wrote {}", out.display());
}

#[test]
fn a_toast_is_a_card_with_its_colour() {
    let fonts = Fonts::new();
    let t = Toast {
        owner: 1,
        serial: 1,
        text: "Fahrt beendet".into(),
        title: Some("Karriere".into()),
        icon: Some("payments".into()),
        color: None,
        seconds: 5.0,
        age: 0.0,
    };
    let l = layout(&toast_panel(&t), &fonts, 1.0);
    assert_eq!(l.w, TOAST_W);
    assert!(l.accent.is_some());
    assert_eq!(
        l.items
            .iter()
            .filter(|i| matches!(i, Item::Icon { .. }))
            .count(),
        1
    );
    assert_eq!(
        l.items
            .iter()
            .filter(|i| matches!(i, Item::Text { .. }))
            .count(),
        2
    );
}
