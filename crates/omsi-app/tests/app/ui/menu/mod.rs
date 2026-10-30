mod pages;

use glam::Vec2;
use omsi_ui::pause_menu;
use omsi_ui::pause_menu::Kind;
use openomsi_game::testing;
use openomsi_game::testing::ui::menu::*;
use openomsi_game::testing::ui::overlay::*;
use openomsi_game::testing::ui::report::ReportFrameInternals;
use std::sync::{Arc, Mutex};

#[test]
fn a_new_loading_reads_its_own_bus_and_screenshots_again() {
    let mut ui = NewUi::default();
    let card = testing::ui::loading::CardText {
        title: "First bus".into(),
        ..Default::default()
    };
    // a first drive's loading: started, its bus read, a screenshot waiting
    ui.slides_started = true;
    ui.bus_started = true;
    {
        let mut s = ui.shared.lock().unwrap();
        s.bus_card = Some(card.clone());
        s.slides.push(testing::ui::loading::Decoded {
            w: 1,
            h: 1,
            rgba: vec![0; 4],
        });
        s.loading = Some(Default::default());
    }
    let gen = ui.loading_gen();
    assert_eq!(ui.bus_card(), Some(card));
    // it is over (or the next drive starts): nothing of it is left, and the next
    // loading starts its threads again
    ui.reset_loading();
    assert!(!ui.slides_started && !ui.bus_started);
    assert_eq!(ui.bus_card(), None);
    let s = ui.shared.lock().unwrap();
    assert!(s.slides.is_empty() && s.loading.is_none());
    // what a thread of the first loading still hands over is told apart and dropped
    assert_ne!(s.loading_gen, gen);
}

#[test]
fn the_logo_is_shrunk_and_has_no_black_box_round_it() {
    let img = logo_rgba().expect("the wordmark");
    assert_eq!(img.width(), LOGO_WIDTH);
    // the corner (the black field) is clear, the letters are whole
    assert_eq!(img.get_pixel(0, 0)[3], 0);
    let opaque = img.pixels().filter(|p| p[3] == 255).count();
    assert!(
        opaque > (img.width() * img.height()) as usize / 10,
        "{opaque}"
    );
}

fn items() -> Vec<(&'static str, &'static str)> {
    testing::input::menus::GAME_MENU.to_vec()
}

#[test]
fn on_a_triple_screen_the_menus_stand_on_the_centre_screen() {
    // three 1920x1080 screens: the centre one
    let a = menu_area(true, false, (5760, 1080));
    assert_eq!(a.origin, Vec2::new(1920.0, 0.0));
    assert_eq!(a.size, (1920, 1080));
    assert_eq!(a.local(Vec2::new(2000.0, 50.0)), Vec2::new(80.0, 50.0));
    // a width that does not divide by three: as the renderer lays the panels out
    let a = menu_area(true, false, (5761, 1080));
    assert_eq!(a.origin.x, omsi_render::panel_width(5761) as f32);
    assert_eq!(a.size, (omsi_render::panel_width(5761), 1080));
    // one screen, and the headset (its menu plane shows the whole window): all of it
    assert_eq!(
        menu_area(false, false, (1920, 1080)),
        Area::whole((1920, 1080))
    );
    assert_eq!(
        menu_area(true, true, (1920, 1080)),
        Area::whole((1920, 1080))
    );
    assert_eq!(
        menu_area(false, true, (1920, 1080)),
        Area::whole((1920, 1080))
    );
    // drawn into a picture it is not in (a texture of another size): the whole picture
    let a = menu_area(true, false, (5760, 1080));
    assert_eq!(a.within((5760, 1080)), a);
    assert_eq!(a.within((1920, 1080)), Area::whole((1920, 1080)));
}

#[test]
fn the_layers_of_the_centre_screen_are_drawn_on_it() {
    let target = (5760u32, 1080u32);
    let area = menu_area(true, false, target);
    let scale = 1.25;
    let mut layers = vec![omsi_ui::Layer {
        viewport: [0.0, 0.0, 1920.0 / scale, 1080.0 / scale],
        ..omsi_ui::Layer::flat([10.0, 20.0, 300.0, 400.0], 0.0, 1.0)
    }];
    place_layers(&mut layers, area, scale, target);
    let l = &layers[0];
    // the shader's mapping (`ui.wgsl`): a point p lands on the picture's pixel
    // (p - viewport.xy) / viewport.zw * target
    let pixel = |p: Vec2| {
        Vec2::new(
            (p.x - l.viewport[0]) / l.viewport[2] * target.0 as f32,
            (p.y - l.viewport[1]) / l.viewport[3] * target.1 as f32,
        )
    };
    for p in [Vec2::ZERO, Vec2::new(100.0, 50.0), Vec2::new(1536.0, 864.0)] {
        let at = pixel(p);
        let want = area.origin + p * scale;
        assert!((at - want).length() < 1e-2, "{p:?}: {at:?} not {want:?}");
    }
    assert_eq!(l.clip, [1930.0, 20.0, 2220.0, 400.0]);
    // the whole picture: left as laid out
    let mut whole = vec![omsi_ui::Layer {
        viewport: [0.0, 0.0, 100.0, 100.0],
        ..omsi_ui::Layer::flat([1.0, 2.0, 3.0, 4.0], 0.0, 1.0)
    }];
    place_layers(&mut whole, Area::whole(target), scale, target);
    assert_eq!(whole[0].viewport, [0.0, 0.0, 100.0, 100.0]);
    assert_eq!(whole[0].clip, [1.0, 2.0, 3.0, 4.0]);
}

#[test]
fn on_a_triple_screen_the_menu_is_clicked_where_the_centre_screen_shows_it() {
    let f = menu_frame(&items(), None, &|_| false, String::new(), true);
    let one = (1920u32, 1080u32);
    let s = f.scale(one);
    let pts = Vec2::new(one.0 as f32, one.1 as f32) / s;
    let lines: Vec<_> = f.with_view(
        |v| pause_menu::rows(v, pts).iter().map(|r| r.rect).collect(),
        None,
    );
    let area = menu_area(true, false, (5760, 1080));
    let mut found = 0;
    for r in &lines {
        let at = Vec2::new(r.x + r.w * 0.5, r.y + r.h * 0.5) * s;
        let id = f.hit(at, one);
        found += id.is_some() as usize;
        // the same line on the centre screen; the left screen has no menu
        assert_eq!(f.hit(area.local(at + area.origin), area.size), id);
        assert_eq!(
            f.hit(area.local(at), area.size),
            None,
            "{at:?} on the left screen"
        );
    }
    assert!(found > 3, "{found} lines");
}

#[test]
fn the_headset_s_plane_shows_the_window_s_picture_pixel_for_pixel() {
    let size = (1920u32, 1080u32);
    let [back, plane] = headset_rects(size);
    // (the first, over the whole window, is the one the headset draws over its view)
    assert_eq!(back, [0.0, 0.0, 1920.0, 1080.0]);
    // the plane maps the window onto the texture: the cursor at a window pixel is on the
    // same pixel of the texture, where the menu's rectangles put the line under it
    for at in [
        Vec2::ZERO,
        Vec2::new(300.5, 200.25),
        Vec2::new(1919.0, 1079.0),
    ] {
        let u = (at.x - plane[0]) / (plane[2] - plane[0]) * size.0 as f32;
        let v = (at.y - plane[1]) / (plane[3] - plane[1]) * size.1 as f32;
        assert_eq!(Vec2::new(u, v), at);
    }
    let area = menu_area(false, true, size);
    let f = menu_frame(&items(), None, &|_| false, String::new(), true);
    let at = Vec2::new(100.0, 300.0);
    assert_eq!(f.hit(area.local(at), area.size), f.hit(at, size));
}

#[test]
fn arrow_keys_follow_the_displayed_order() {
    // the tour lines sit in "Session", right under Resume, though the game lists them later
    let list = [
        ("resume", "Resume"),
        ("options", "Options..."),
        ("map", "City map"),
        ("duty", "Line and tour..."),
        ("skipstop", "Skip the next stop"),
        ("quit", "End the session"),
    ];
    let none = |_: usize| false;
    assert_eq!(display_order(&list), vec![0, 3, 4, 1, 2, 5]);
    assert_eq!(step_displayed(&list, 0, true, &none), 3);
    assert_eq!(step_displayed(&list, 4, true, &none), 1);
    assert_eq!(step_displayed(&list, 1, false, &none), 4);
    assert_eq!(step_displayed(&list, 5, true, &none), 0);
    assert_eq!(step_displayed(&list, 0, false, &none), 5);
    // a greyed-out line is stepped over
    assert_eq!(step_displayed(&list, 0, true, &|k| k == 3), 4);
}

#[test]
fn a_long_menu_fits_a_short_window_and_its_last_line_can_be_clicked() {
    let mut list = items();
    list.push(("copycode", "Copy server code"));
    list.push(("admin", "Administration..."));
    let f = menu_frame(&list, None, &|_| false, String::new(), true);
    for size in [(720u32, 405u32), (960, 540), (1280, 720), (1672, 941)] {
        let s = f.scale(size);
        assert!(s > 0.0 && s <= scale_for(size.0, size.1) + 1e-6);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / s;
        let bottom = f.with_view(
            |v| {
                pause_menu::rows(v, pts)
                    .iter()
                    .map(|r| (r.rect.y + r.rect.h) * s)
                    .fold(0.0, f32::max)
            },
            None,
        );
        assert!(bottom <= size.1 as f32, "{size:?}: lines end at {bottom}");
        // the last line, found where it is drawn
        let last = f.with_view(
            |v| {
                let r = pause_menu::rows(v, pts);
                r.last().map(|r| (r.id.to_string(), r.rect.center() * s))
            },
            None,
        );
        let (id, at) = last.unwrap();
        assert_eq!(f.hit(at, size).as_deref(), Some(id.as_str()), "{size:?}");
    }
    // (where the usual scale would run off the bottom, the menu gets smaller)
    assert!(f.scale((720, 405)) < scale_for(720, 405));
    // a window that is large enough keeps the usual scale
    assert_eq!(f.scale((1672, 941)), scale_for(1672, 941));
}

#[test]
fn mouse_led_menu_lights_no_keyboard_line() {
    let list = [("resume", "Resume"), ("quit", "Quit")];
    let f = menu_frame(&list, None, &|_| false, String::new(), true);
    assert!(f.selected.is_none());
}

#[test]
fn every_line_of_the_game_menu_is_placed_once_in_the_order_the_game_has_them() {
    let list = items();
    let f = menu_frame(&list, Some(0), &|_| false, "08:42".into(), true);
    let ids: Vec<&str> = f
        .sections
        .iter()
        .flat_map(|s| s.items.iter().map(|i| i.id.as_str()))
        .collect();
    assert_eq!(ids.len(), list.len());
    for (id, _) in &list {
        assert_eq!(ids.iter().filter(|i| *i == id).count(), 1, "{id}");
    }
    // (the way back into the game leads the first section)
    assert_eq!(f.sections[0].items[0].id, "resume");
    assert_eq!(f.sections[0].items[0].kind, Kind::Primary);
    assert_eq!(f.state, "paused");
}

#[test]
fn every_icon_the_menu_uses_exists() {
    let mut ids: Vec<&str> = items().iter().map(|m| m.0).collect();
    ids.extend(["tobus", "copycode", "admin", "no such line"]);
    for id in ids {
        let (_, icon, _, _) = place(id);
        assert!(omsi_ui::icons::svg(icon).is_some(), "{id}: {icon}");
    }
}

#[test]
fn the_keyboard_line_is_lit_and_a_greyed_out_line_cannot_be_hit() {
    let list = items();
    let skip = list.iter().position(|m| m.0 == "options").unwrap();
    let f = menu_frame(&list, Some(skip), &|k| k == skip, String::new(), false);
    assert_eq!(f.selected.as_deref(), Some("options"));
    assert_eq!(f.state, "menu open");
    let size = (1672, 941);
    // the resume line is the first row; the greyed-out line returns nothing
    let rows = f.with_view(
        |v| {
            pause_menu::rows(v, Vec2::new(1672.0, 941.0))
                .into_iter()
                .map(|r| (r.id.to_string(), r.rect))
                .collect::<Vec<_>>()
        },
        None,
    );
    let at = |id: &str| {
        let r = rows.iter().find(|r| r.0 == id).unwrap().1;
        Vec2::new(r.x + 20.0, r.y + r.h * 0.5)
    };
    assert_eq!(f.hit(at("resume"), size).as_deref(), Some("resume"));
    assert_eq!(f.hit(at("options"), size), None);
    assert_eq!(f.hit(Vec2::new(1000.0, 500.0), size), None);
}

#[test]
fn a_click_is_found_at_any_window_size() {
    let list = items();
    let f = menu_frame(&list, Some(0), &|_| false, String::new(), true);
    for size in [(1280u32, 720u32), (1672, 941), (3840, 2160)] {
        let scale = scale_for(size.0, size.1);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
        let rows = f.with_view(
            |v| {
                pause_menu::rows(v, pts)
                    .into_iter()
                    .map(|r| (r.id.to_string(), r.rect))
                    .collect::<Vec<_>>()
            },
            None,
        );
        let r = rows.iter().find(|r| r.0 == "quit").unwrap().1;
        let px = Vec2::new(r.x + 20.0, r.y + r.h * 0.5) * scale;
        assert_eq!(f.hit(px, size).as_deref(), Some("quit"), "{size:?}");
    }
}

/// Under "DirectX9 (D3D9Ex)" the overlay's draws go to the mirror too: the whole atlas
/// first, then the vertices and the draw, and nothing while no screen is shown.
#[test]
fn the_overlay_hands_its_draws_to_the_mirror() {
    let mut descriptor = wgpu::InstanceDescriptor::new_without_display_handle();
    descriptor.backends = wgpu::Backends::NOOP;
    descriptor.backend_options.noop = wgpu::NoopBackendOptions { enable: true };
    let instance = wgpu::Instance::new(descriptor);
    let adapter = pollster::block_on(instance.request_adapter(&Default::default())).unwrap();
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width: 640,
            height: 360,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let view = target.create_view(&Default::default());
    let seen: Arc<Mutex<Vec<String>>> = Default::default();
    let log = seen.clone();
    let mirror: omsi_ui::UiMirror = Box::new(move |e| {
        let what = match e {
            omsi_ui::UiEvent::Image { .. } => "image".to_string(),
            omsi_ui::UiEvent::Free(_) => "free".to_string(),
            omsi_ui::UiEvent::Atlas { size, region, .. } => {
                format!("atlas {}", region == [0, 0, size, size])
            }
            omsi_ui::UiEvent::Vertices { vertices, .. } => {
                format!("vertices {}", !vertices.is_empty())
            }
            omsi_ui::UiEvent::Render { size, draws, .. } => {
                format!("render {size:?} {}", !draws.is_empty())
            }
            omsi_ui::UiEvent::Gone => "gone".to_string(),
        };
        log.lock().unwrap().push(what);
    });
    let shared: Shared = Default::default();
    let mut overlay = Overlay::new(shared.clone(), format, Some(mirror));
    let draw = |overlay: &mut Overlay| {
        let mut enc = device.create_command_encoder(&Default::default());
        overlay.draw(&device, &queue, &mut enc, &view, (640, 360));
        queue.submit([enc.finish()]);
    };
    draw(&mut overlay);
    assert!(seen.lock().unwrap().is_empty(), "no screen, nothing drawn");
    shared.lock().unwrap().loading = Some(testing::ui::loading::LoadingFrame {
        heading: "Example map".into(),
        progress: 0.5,
        ..Default::default()
    });
    draw(&mut overlay);
    let events = seen.lock().unwrap().clone();
    let at = |what: &str| events.iter().position(|e| e == what);
    let (atlas, vertices, render) = (
        at("atlas true").expect("the whole atlas"),
        at("vertices true").expect("the vertices"),
        at("render (640, 360) true").expect("the draw"),
    );
    assert!(atlas < render && vertices < render, "{events:?}");
    drop(overlay);
    assert_eq!(
        seen.lock().unwrap().last().map(String::as_str),
        Some("gone")
    );
}

/// The overlay itself, on a real adapter: the column is drawn over a picture and nothing
/// is drawn while no menu is open.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_overlay_draws_the_menu_and_the_loading_screen_and_nothing_else() {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
        eprintln!("no graphics adapter: skipped");
        return;
    };
    let (device, queue) = pollster::block_on(adapter.request_device(&Default::default())).unwrap();
    let format = wgpu::TextureFormat::Rgba8UnormSrgb;
    let (w, h) = (1672u32, 941u32);
    let tex = device.create_texture(&wgpu::TextureDescriptor {
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
    let view = tex.create_view(&Default::default());
    let shared: Shared = Default::default();
    let mut overlay = Overlay::new(shared.clone(), format, None);
    let clear = |enc: &mut wgpu::CommandEncoder| {
        enc.begin_render_pass(&wgpu::RenderPassDescriptor {
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.6,
                        b: 0.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
    };
    let read = |overlay: &mut Overlay| -> Vec<u8> {
        let mut enc = device.create_command_encoder(&Default::default());
        clear(&mut enc);
        overlay.draw(&device, &queue, &mut enc, &view, (w, h));
        let stride = (w * 4).next_multiple_of(256);
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: (stride * h) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        enc.copy_texture_to_buffer(
            tex.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(h),
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
        device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let data = buf.slice(..).get_mapped_range().to_vec();
        let mut out = Vec::new();
        for y in 0..h as usize {
            out.extend_from_slice(
                &data[y * stride as usize..y * stride as usize + (w * 4) as usize],
            );
        }
        out
    };
    let px = |img: &[u8], x: u32, y: u32| {
        let i = ((y * w + x) * 4) as usize;
        [img[i], img[i + 1], img[i + 2]]
    };
    // no menu: the green picture is untouched
    let plain = read(&mut overlay);
    assert_eq!(px(&plain, 20, 20), px(&plain, 1500, 800));
    assert!(plain
        .chunks(4)
        .all(|p| p[0] == 0 && p[2] == 0 && p[1] > 100));
    // a menu: the column (left) is dark, the world to its right is the picture dimmed
    shared.lock().unwrap().menu = Some(menu_frame(
        &items(),
        Some(0),
        &|_| false,
        "08:42".into(),
        true,
    ));
    let drawn = read(&mut overlay);
    let column = px(&drawn, 300, 700);
    let world = px(&drawn, 1500, 800);
    assert!(column[1] < 40, "the column is dark: {column:?}");
    assert!(
        world[1] > 40 && world[1] < plain.chunks(4).next().unwrap()[1],
        "the world is dimmed, not covered: {world:?}"
    );
    // and the rows are where the hit test says: the accent row of "Resume" is orange
    let rows = menu_frame(&items(), Some(0), &|_| false, String::new(), true).with_view(
        |v| {
            pause_menu::rows(v, Vec2::new(w as f32, h as f32))
                .into_iter()
                .map(|r| (r.id.to_string(), r.rect))
                .collect::<Vec<_>>()
        },
        None,
    );
    let r = rows.iter().find(|r| r.0 == "resume").unwrap().1;
    let resume = px(&drawn, (r.x + r.w - 40.0) as u32, (r.y + r.h * 0.5) as u32);
    assert!(
        resume[0] > 200 && resume[2] < 60,
        "Resume is the accent row: {resume:?}"
    );
    // the loading screen: with no screenshot yet the default background covers the picture
    {
        let mut s = shared.lock().unwrap();
        s.menu = None;
        s.loading = Some(testing::ui::loading::LoadingFrame {
            heading: "Example map".into(),
            progress: 0.5,
            ..Default::default()
        });
    }
    let empty = read(&mut overlay);
    let corner = px(&empty, 1600, 60);
    assert!(
        corner[1] < 100,
        "the default background, not the picture: {corner:?}"
    );
    // a screenshot arrives (all red): it is the background now
    shared
        .lock()
        .unwrap()
        .slides
        .push(testing::ui::loading::Decoded {
            w: 16,
            h: 9,
            rgba: [255, 0, 0, 255].repeat(16 * 9),
        });
    let shown = read(&mut overlay);
    let corner = px(&shown, 1600, 60);
    assert!(
        corner[0] > corner[1] + 40 && corner[0] > corner[2] + 40,
        "the screenshot shows: {corner:?}"
    );
    // the map is there: the screen is gone and the picture is the game's again
    shared.lock().unwrap().loading = None;
    let after = read(&mut overlay);
    assert_eq!(px(&after, 1600, 60), px(&plain, 1600, 60));
    assert!(overlay.slide_tex.is_empty());
}

/// The Esc menu and a page on the centre screen of a triple screen (the renderer's own
/// three panels, the overlay drawn once they are in the window), and drawn into the
/// headset's picture of them (`NewUi::headset_surface`, the texture laid over the window
/// here as the menu plane shows it), into PNGs (`DEPOT_PAGES_PNG`: the folder; else they
/// are only drawn). Also checks they are where the clicks are taken.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_menus_draw_on_the_centre_screen_and_into_the_headset_s_picture() {
    use testing::hud::lists::{row, ListKind};
    use testing::hud::MenuKind;
    use testing::ui::pages::{page_frame, test_column, test_frame};
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
    let mut scene = r.new_scene();
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
    let rig = omsi_render::TripleScreen {
        enabled: true,
        ..Default::default()
    };
    let out = std::env::var("DEPOT_PAGES_PNG").ok();
    let save = |name: &str, img: &[u8], (w, h): (u32, u32)| {
        if let Some(dir) = out.as_ref() {
            std::fs::create_dir_all(dir).unwrap();
            let path = format!("{dir}/{name}.png");
            image::save_buffer(path, img, w, h, image::ExtendedColorType::Rgba8).unwrap();
        }
    };
    // how far two pictures of width `w` are apart between columns x0 and x1
    let apart = |a: &[u8], b: &[u8], w: u32, x0: u32, x1: u32| -> u64 {
        let rows = a.len() as u32 / (w * 4);
        (0..rows)
            .flat_map(|y| (x0..x1).map(move |x| ((y * w + x) * 4) as usize))
            .map(|i| {
                (0..3)
                    .map(|c| a[i + c].abs_diff(b[i + c]) as u64)
                    .sum::<u64>()
            })
            .sum()
    };
    let away = Vec2::new(-100.0, -100.0);
    let options_rows: Vec<(String, String)> = [
        (
            "Navigator",
            's',
            "on",
            "Enables/Disables the Minimap",
            "navigator",
        ),
        (
            "Interface size",
            'v',
            "100 %",
            "How large the menus are",
            "ui_scale",
        ),
        (
            "Language",
            'o',
            "English",
            "The interface's language",
            "sel language",
        ),
        (
            "Information bar",
            's',
            "off",
            "Time and speed at the top",
            "info_bar",
        ),
    ]
    .iter()
    .map(|(l, k, v, d, id)| {
        let slider = (*k == 'v').then_some(0.4);
        (id.to_string(), row(l, *k, v, d, slider))
    })
    .collect();
    let items: Vec<(&str, &str)> = options_rows
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu_tabs = Some((vec!["General".into(), "Graphics".into()], 0));
    let page = page_frame(&f, Some(&ListKind::Options(0)), test_column(true)).unwrap();
    let mut ui = NewUi::default();
    ui.install(&mut r, None);

    // three 1920x1080 screens: the world alone, then the menu, then the page
    let size = (5760u32, 1080u32);
    let area = menu_area(true, false, size);
    let mut shoot = |scene: &mut omsi_render::Scene| {
        let mut img = Vec::new();
        for _ in 0..3 {
            img = r
                .render_triple_to_image(scene, size.0, size.1, &camera, &lighting, &rig)
                .unwrap();
        }
        img
    };
    ui.set(None, None, None, None, None, away, Some(area));
    let world = shoot(&mut scene);
    ui.set(
        Some(test_column(true)),
        None,
        None,
        None,
        None,
        away,
        Some(area),
    );
    let menu = shoot(&mut scene);
    save("triple-5760x1080-menu", &menu, size);
    ui.page = Some(page.clone());
    ui.set(None, None, None, None, None, away, Some(area));
    let shown = shoot(&mut scene);
    save("triple-5760x1080-page", &shown, size);
    ui.page = None;
    // the end-of-trip report, on the centre screen too
    let report = testing::ui::report::ReportFrame::for_test();
    ui.set(None, None, None, None, Some(report), away, Some(area));
    let reported = shoot(&mut scene);
    save("triple-5760x1080-report", &reported, size);
    ui.set(None, None, None, None, None, away, Some(area));
    // on the centre screen only: the side screens are the world's as they were
    for (name, img) in [("menu", &menu), ("page", &shown), ("report", &reported)] {
        assert_eq!(
            apart(img, &world, size.0, 0, 1920),
            0,
            "{name}: the left screen"
        );
        assert_eq!(
            apart(img, &world, size.0, 3840, 5760),
            0,
            "{name}: the right one"
        );
        assert!(
            apart(img, &world, size.0, 1920, 3840) > 0,
            "{name}: the centre one"
        );
    }
    // the column at the centre screen's left edge, the page's rows right of it, where
    // the clicks are taken
    let s = test_column(true).scale(area.size);
    let col = (1920.0 + pause_menu::COLUMN_W * s) as u32;
    assert!(apart(&menu, &world, size.0, 1920, col) > 0);
    let mut hud = testing::hud::Ui::new().unwrap();
    page.write_hits(&mut hud.hits, area);
    for rect in hud.hits.menu_rects.iter().filter(|r| r[0] > 0.0) {
        assert!(rect[0] >= col as f32 - 1.0 && rect[2] <= 3840.0, "{rect:?}");
        let (x0, x1) = (rect[0] as u32, rect[2] as u32);
        assert!(apart(&shown, &world, size.0, x0, x1) > 0, "{rect:?}");
    }

    // the headset: the menu and the page drawn into a texture of the window's size,
    // laid over the window behind the dimming plate as the menu plane shows it
    let size = (1920u32, 1080u32);
    let area = menu_area(false, true, size);
    let plain = r
        .render_to_image(&mut scene, size.0, size.1, &camera, &lighting)
        .unwrap();
    let headset = |ui: &mut NewUi,
                   r: &mut omsi_render::Renderer,
                   scene: &mut omsi_render::Scene,
                   menu: Option<MenuFrame>| {
        let mut range = 0..0;
        for _ in 0..3 {
            scene.overlays.truncate(range.start);
            ui.set(menu.clone(), None, None, None, None, away, Some(area));
            range = ui
                .headset_surface(r, scene, size)
                .expect("the headset's picture");
        }
        assert_eq!(range.len(), 2);
        let [back, plane] = headset_rects(size);
        assert_eq!(scene.overlays[range.start].1, back);
        assert_eq!(scene.overlays[range.start + 1].1, plane);
        // (the window itself shows nothing of it: only the texture)
        let page = ui.page.take();
        ui.set(None, None, None, None, None, away, Some(area));
        let img = r
            .render_to_image(scene, size.0, size.1, &camera, &lighting)
            .unwrap();
        ui.page = page;
        scene.overlays.truncate(range.start);
        img
    };
    let vr_menu = headset(&mut ui, &mut r, &mut scene, Some(test_column(true)));
    save("headset-1920x1080-menu", &vr_menu, size);
    ui.page = Some(page.clone());
    let vr_page = headset(&mut ui, &mut r, &mut scene, None);
    save("headset-1920x1080-page", &vr_page, size);
    // nothing open: no picture for the plane
    ui.page = None;
    ui.set(None, None, None, None, None, away, Some(area));
    assert!(ui.headset_surface(&mut r, &mut scene, size).is_none());
    let s = test_column(true).scale(size);
    let col = (pause_menu::COLUMN_W * s) as u32;
    assert!(apart(&vr_menu, &plain, size.0, 0, col) > 0);
    let mut hud = testing::hud::Ui::new().unwrap();
    page.write_hits(&mut hud.hits, area);
    for rect in hud.hits.menu_rects.iter().filter(|r| r[0] > 0.0) {
        let (x0, x1) = (rect[0] as u32, rect[2] as u32);
        assert!(apart(&vr_page, &plain, size.0, x0, x1) > 0, "{rect:?}");
    }
}
