//! The pages behind the menu's lines, with placeholder data, drawn by the overlay over a
//! plain picture into PNGs (`DEPOT_PAGES_PNG`: the folder; else they are only drawn), for
//! looking at them next to the design, one test per page. Each also checks a page is drawn
//! where `write_hits` says its rows are.

use glam::Vec2;
use openomsi_game::testing;
use openomsi_game::testing::new_ui::*;
use testing::game_lists::{row, ListKind, HEADING};
use testing::new_ui_pages::{page_frame, test_column, test_frame, PageFrame};
use testing::ui::{DropdownView, MenuKind, Preview};

/// The size of the window the pages are drawn in (pixels).
const SIZE: (u32, u32) = (1672, 941);
/// A mouse away from every page.
const AWAY: Vec2 = Vec2::new(-100.0, -100.0);

fn k(s: &str) -> String {
    s.to_string()
}

/// The HUD's list: (what it does, what it says).
fn refs(v: &[(String, String)]) -> Vec<(String, String)> {
    v.iter().map(|(l, id)| (id.clone(), l.clone())).collect()
}

/// The tabs of the options pages.
fn option_tabs() -> Vec<String> {
    vec![
        k("General"),
        k("Graphics"),
        k("Display"),
        k("Sound"),
        k("Camera"),
        k("Driving"),
        k("Advanced"),
    ]
}

/// A graphics device to draw the pages with, and where to save them.
struct Shots {
    device: wgpu::Device,
    queue: wgpu::Queue,
    out: Option<String>,
}

impl Shots {
    /// `None` without a graphics adapter.
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("no graphics adapter: skipped");
            return None;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        Some(Shots {
            device,
            queue,
            out: std::env::var("DEPOT_PAGES_PNG").ok(),
        })
    }

    /// Draws `page` with the mouse at `mouse` in a window of `size`, saves it as `name`.
    fn shoot(&self, name: &str, (w, h): (u32, u32), page: PageFrame, mouse: Vec2) {
        let Shots { device, queue, out } = self;
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
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
        {
            let mut s = shared.lock().unwrap();
            s.page = Some(page.clone());
            s.mouse = mouse;
        }
        let mut img = Vec::new();
        // (twice: the second frame has every animation where it goes)
        for _ in 0..2 {
            let mut enc = device.create_command_encoder(&Default::default());
            enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        // (a stand-in for the world: a grey-blue picture)
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 0.18,
                            g: 0.24,
                            b: 0.30,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
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
            img.clear();
            for y in 0..h as usize {
                img.extend_from_slice(
                    &data[y * stride as usize..y * stride as usize + (w * 4) as usize],
                );
            }
        }
        if let Some(dir) = out.as_ref() {
            std::fs::create_dir_all(dir).unwrap();
            image::save_buffer(
                format!("{dir}/{name}.png"),
                &img,
                w,
                h,
                image::ExtendedColorType::Rgba8,
            )
            .unwrap();
        }
        // the page's card is where the HUD is told its rows are: dark, over the world
        // (all but one row: the keyboard's or the mouse's line is lit lighter)
        let mut ui = testing::ui::Ui::new().unwrap();
        page.write_hits(&mut ui, Area::whole((w, h)));
        let rows: Vec<[u8; 3]> = ui
            .menu_rects
            .iter()
            .filter(|r| r[0] > 0.0)
            .map(|r| {
                let i = (((r[1] + r[3]) * 0.5) as u32 * w + (r[2] - 4.0) as u32) as usize * 4;
                [img[i], img[i + 1], img[i + 2]]
            })
            .collect();
        assert!(!rows.is_empty(), "{name}: a row");
        let light: Vec<_> = rows.iter().filter(|p| p[2] >= 60).collect();
        assert!(
            light.len() <= 1,
            "{name}: the card under its rows: {light:?} of {rows:?}"
        );
    }
}

/// Options: a long page of switches, a slider and a select, the keyboard on a row.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_general_options_page_draws_in_the_new_look() {
    let Some(shots) = Shots::new() else {
        return;
    };
    let general = refs(&[
        (row("Navigator", 's', "on", "Enables/Disables the Minimap", None), k("navigator")),
        (row("AI vehicles on the map", 's', "on", "Shows/hides the other (AI) vehicles on the Minimap and the city map", None), k("nav_ai")),
        (row("Minimap on foot and in the free camera", 's', "off", "On foot the minimap shows a walker icon; in the free camera (F4) it centres on a camera icon and moves with the camera instead of staying on the bus", None), k("nav_follow_view")),
        (row("Terrain contours", 's', "off", "Shows the terrain's contour lines on the city map, when the map ships them (also a button on the map)", None), k("nav_contours")),
        (row("Light map", 's', "off", "The map in a light look: light ground and white roads instead of the dark one", None), k("nav_light")),
        (row("Interface size", 'v', "100 %", "How large the menus and the notes are", Some(0.4)), k("ui_scale")),
        (row("Language", 'o', "Português (Brasil)", "The interface's language", None), k("sel language")),
        (row("Route arrows (as in OMSI 2)", 's', "on", "Shows OMSI 2's route arrows over the road", None), k("nav_arrows")),
        (row("Ability to get up (Ctrl+Shift+G)", 's', "off", "Allows you to get out of the car and explore the world", None), k("get_up")),
        (row("Collisions with objects", 's', "on", "Enables/disables collisions with objects such as buildings, streetlights, etc.", None), k("coll_objects")),
        (row("Collisions with vehicles", 's', "on", "Enables/Disables Collisions with Other Vehicles", None), k("coll_vehicles")),
        (row("Collisions with people", 's', "on", "Enables/disables knocking down people", None), k("collision_pedestrians")),
        (row("Timetable window (Insert)", 's', "off", "Displays a list of all stops (only when a tour is active)", None), k("timetable_win")),
        (row("Information bar (Shift+Y)", 's', "off", "Displays information such as the time, speed, and other details at the top of the screen", None), k("info_bar")),
        (row("Passengers pay the exact fare", 's', "on", "No change is given at the cash desk", None), k("exact_fare")),
    ]);
    let items: Vec<(&str, &str)> = general
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu = Some((4, &items));
    f.menu_tabs = Some((option_tabs(), 0));
    let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true)).unwrap();
    shots.shoot("options-general", SIZE, p.clone(), AWAY);
    shots.shoot("options-general-1280x720", (1280, 720), p.clone(), AWAY);
    shots.shoot(
        "options-general-1920x1080-hover",
        (1920, 1080),
        p,
        Vec2::new(900.0, 330.0),
    );
}

/// Graphics: sliders, a stepper, a select with its drop-down open.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_graphics_options_page_draws_in_the_new_look() {
    let Some(shots) = Shots::new() else {
        return;
    };
    let graphics = refs(&[
        (row("Quality preset", 'o', "Vanilla+", "Sets most of the graphics options at once", None), k("preset")),
        (row("Sun shadows", 's', "on", "Enables/Disabled shadows", None), k("shadows")),
        (row("Night brightness", 'v', "+0.25", "Enhanced graphics: how much brighter the night is shown", Some(0.25)), k("night_brightness")),
        (row("LED glow", 'v', "6/15", "How strongly the dots of LED destination displays glow", Some(0.4)), k("led_glow")),
        (row("Anti-aliasing", 'c', "FXAA", "Smooths the edges", None), k("post_aa")),
        (row("Clouds", 's', "on", "Needs a restart", None), k("clouds")),
        (row("Windy trees", 's', "off", "The trees' leaves bend and sway in the wind and its gusts; with no wind they stand still", None), k("windy_trees")),
    ]);
    let items: Vec<(&str, &str)> = graphics
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu_tabs = Some((option_tabs(), 1));
    let names = ["Vanilla", "Vanilla+", "Enhanced", "Ultra", "Custom"];
    f.dropdown = Some(DropdownView {
        row: 0,
        items: names.to_vec(),
        sel: 2,
        top: 0,
        current: Some(1),
    });
    let p = page_frame(&f, Some(&ListKind::Options(1)), test_column(true)).unwrap();
    shots.shoot("options-graphics-dropdown", SIZE, p, AWAY);
    f.dropdown = None;
    f.menu = Some((3, &items));
    let p = page_frame(&f, Some(&ListKind::Options(1)), test_column(true)).unwrap();
    shots.shoot("options-graphics", SIZE, p, AWAY);
}

/// Controls, and the keyboard's bindings (one being changed).
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_controls_pages_draw_in_the_new_look() {
    let Some(shots) = Shots::new() else {
        return;
    };
    let controls = refs(&[
        (
            row(
                "Keyboard",
                'o',
                "",
                "Change driving, vehicle, game and camera key bindings",
                None,
            ),
            k("keyboard"),
        ),
        (
            row(
                "Game controllers",
                'o',
                "",
                "Configure wheels, pedals, gamepads and force feedback",
                None,
            ),
            k("controllers"),
        ),
    ]);
    let items: Vec<(&str, &str)> = controls
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu_head = Some((k("Controls"), String::new()));
    f.menu_tabs = Some((vec![k("Controls")], 0));
    shots.shoot(
        "controls",
        SIZE,
        page_frame(&f, Some(&ListKind::Controls), test_column(true)).unwrap(),
        AWAY,
    );
    let keys = refs(&[
        (
            row(
                "Add a vehicle event…",
                'o',
                "",
                "Choose a control supplied by the bus or its mods",
                None,
            ),
            k("key_events"),
        ),
        (
            row(
                "Accelerate",
                'a',
                "W",
                "Enter to change; Delete clears; Esc cancels",
                None,
            ),
            k("keybind v 0"),
        ),
        (
            row(
                "Brake",
                'a',
                "S",
                "Enter to change; Delete clears; Esc cancels",
                None,
            ),
            k("keybind v 1"),
        ),
        (
            row(
                "Doors front",
                'a',
                "press a key…",
                "Enter to change; Delete clears; Esc cancels",
                None,
            ),
            k("keybind v 2"),
        ),
        (
            row(
                "Horn",
                'a',
                "Shift+H",
                "Enter to change; Delete clears; Esc cancels",
                None,
            ),
            k("keybind v 3"),
        ),
        (
            row(
                "Kneeling",
                'a',
                "(unbound)",
                "Enter to change; Delete clears; Esc cancels",
                None,
            ),
            k("keybind v 4"),
        ),
    ]);
    let items: Vec<(&str, &str)> = keys.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu = Some((3, &items));
    f.menu_head = Some((k("Keyboard"), String::new()));
    f.menu_tabs = Some((vec![k("Driving and bus"), k("Game and camera")], 0));
    shots.shoot(
        "controls-keyboard",
        SIZE,
        page_frame(&f, Some(&ListKind::Keyboard(0)), test_column(true)).unwrap(),
        AWAY,
    );
}

/// A game controller: headings, information, switches, sliders.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_game_controller_page_draws_in_the_new_look() {
    let Some(shots) = Shots::new() else {
        return;
    };
    let pad = refs(&[
        (k("Connected devices"), k(HEADING)),
        (
            row(
                "Logitech G29",
                'o',
                "Wheel",
                "Axes, buttons and force feedback",
                None,
            ),
            k("device 0"),
        ),
        (k("Steering"), k(HEADING)),
        (
            row(
                "Dead zone",
                'v',
                "4 %",
                "Ignore movement around the centre or at pedal rest",
                Some(0.04),
            ),
            k("ctrl_deadzone"),
        ),
        (
            row(
                "Wheel rotation",
                'v',
                "900°",
                "Your wheel's rotation from lock to lock",
                Some(0.75),
            ),
            k("wheel_range"),
        ),
        (k("Pedals"), k(HEADING)),
        (
            row(
                "Connection",
                'i',
                "Connected",
                "Successful changes are saved and applied immediately",
                None,
            ),
            k("noop"),
        ),
        (
            row(
                "Force feedback and vibration",
                's',
                "on",
                "Enable steering forces and gamepad rumble",
                None,
            ),
            k("ff"),
        ),
    ]);
    let items: Vec<(&str, &str)> = pad.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu = Some((1, &items));
    f.menu_head = Some((k("Game controllers"), String::new()));
    f.menu_tabs = Some((
        vec![k("Devices"), k("Steering and pedals"), k("Force feedback")],
        0,
    ));
    shots.shoot(
        "controls-controllers",
        SIZE,
        page_frame(&f, Some(&ListKind::ControllerDevices(0)), test_column(true)).unwrap(),
        AWAY,
    );
}

/// Vehicle options, World options (a time being typed).
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_vehicle_and_world_options_pages_draw_in_the_new_look() {
    let Some(shots) = Shots::new() else {
        return;
    };
    let vehicle = refs(&[
        (
            row(
                "Destination display",
                'o',
                "",
                "Change the current destination",
                None,
            ),
            k("dest"),
        ),
        (
            row(
                "Depot file (HOF)",
                'o',
                "",
                "Change the current depot file (used for the timetable)",
                None,
            ),
            k("hof"),
        ),
        (
            row(
                "Fleet number",
                'o',
                "",
                "Change the fleet number and the registration",
                None,
            ),
            k("number"),
        ),
        (
            row("Driver", 'o', "", "Change the current driver profile", None),
            k("driver"),
        ),
        (
            row(
                "Repair",
                'a',
                "Repair now",
                "Repairs every damage of the vehicle",
                None,
            ),
            k("repair"),
        ),
    ]);
    let items: Vec<(&str, &str)> = vehicle
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu_head = Some((k("Vehicle options"), String::new()));
    f.menu_tabs = Some((vec![k("Display"), k("Fleet"), k("Service")], 0));
    shots.shoot(
        "vehicle-options",
        SIZE,
        page_frame(&f, Some(&ListKind::Vehicle(0)), test_column(true)).unwrap(),
        AWAY,
    );
    let world = refs(&[
        (
            row(
                "Exact time",
                'E',
                "0842",
                "Press Enter to change, Esc to cancel",
                None,
            ),
            k("time_edit"),
        ),
        (
            row(
                "Time speed",
                'v',
                "x1",
                "How fast the clock runs",
                Some(0.1),
            ),
            k("time_speed"),
        ),
        (
            row(
                "Real-time sync",
                's',
                "off",
                "The clock follows the computer's",
                None,
            ),
            k("time_sync"),
        ),
        (row("Date", 'i', "14.10.2026", "", None), k("noop")),
        (
            row("Season", 'c', "Autumn", "Leaves and snow", None),
            k("season"),
        ),
    ]);
    let items: Vec<(&str, &str)> = world
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Options);
    f.menu_head = Some((k("World options"), String::new()));
    f.menu_tabs = Some((vec![k("Time"), k("Weather"), k("Traffic"), k("Tools")], 0));
    shots.shoot(
        "world-options",
        SIZE,
        page_frame(&f, Some(&ListKind::World(0)), test_column(true)).unwrap(),
        AWAY,
    );
}

/// Line and tour: the lines with the tours of the chosen one beside them.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_line_and_tour_pages_draw_in_the_new_look() {
    let Some(shots) = Shots::new() else {
        return;
    };
    let lines: Vec<(String, String)> = ["50", "51", "N30", "107", "M49"]
        .iter()
        .enumerate()
        .map(|(i, n)| {
            (
                format!("line {n}"),
                format!("Line {n}  ({} tours)", 3 + i * 2),
            )
        })
        .chain([(k("back"), k("Back"))])
        .collect();
    let items: Vec<(&str, &str)> = lines
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Lines);
    f.menu_head = Some((k("Line and tour"), String::new()));
    f.menu_preview = Some(Preview {
        title: k("Line 50"),
        meta: k("3 tours"),
        rows: vec![
            (k("Tour 1  ›  Central Station"), k("08:42")),
            (k("Tour 2  ›  Market"), k("09:02")),
            (k("Tour 10  ›  Depot"), k("09:22")),
        ],
        chosen: None,
        button: None,
        time: None,
    });
    shots.shoot(
        "line-and-tour",
        SIZE,
        page_frame(&f, Some(&ListKind::Lines), test_column(true)).unwrap(),
        AWAY,
    );
    // a line's tours, the stops of the chosen one to start from
    let tours: Vec<(String, String)> = (1..=6)
        .map(|t| (format!("tour 50\u{1}{t}"), format!("Tour {t}")))
        .chain([(k("back"), k("Back"))])
        .collect();
    let items: Vec<(&str, &str)> = tours
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::Tours);
    f.menu = Some((1, &items));
    f.menu_head = Some((k("Line and tour"), k("Line 50")));
    let stops = [
        "Depot",
        "Market",
        "Main Street",
        "Central Station",
        "Hospital",
        "University",
        "Airport road",
        "Old town",
        "Harbour",
        "Stadium",
        "Park",
        "Library",
        "Bus station",
        "Cemetery",
        "Terminus",
        "Industrial estate",
        "Riverside",
        "North gate",
        "Ring road",
        "Last stop",
    ];
    f.menu_preview = Some(Preview {
        title: k("Tour 2"),
        meta: k("Line 50  ·  Trip 2/9  ·  Choose the stop to start from"),
        rows: stops
            .iter()
            .enumerate()
            .map(|(i, s)| {
                (
                    k(s),
                    format!("{:02}:{:02}", 9 + (2 + i * 3) / 60, (2 + i * 3) % 60),
                )
            })
            .collect(),
        chosen: Some(2),
        button: Some(k("Start trip")),
        time: Some(k("09:02")),
    });
    shots.shoot(
        "line-and-tour-tours",
        SIZE,
        page_frame(&f, Some(&ListKind::Tours(k("50"), None)), test_column(true)).unwrap(),
        AWAY,
    );
}

/// A long list of codes and names (the destinations), scrolled, the way back under it.
#[test]
#[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
fn the_vehicle_list_pages_draw_in_the_new_look() {
    let Some(shots) = Shots::new() else {
        return;
    };
    let names = [
        "Central Station",
        "Market",
        "Depot",
        "Not in service",
        "Hospital via Main Street",
    ];
    let dest: Vec<(String, String)> = std::iter::once((k("routes"), k("Route number: 50...")))
        .chain((0..40).map(|i| {
            (
                format!("dest {i}"),
                format!("{:>3}  {}", 100 + i * 7, names[i % 5]),
            )
        }))
        .chain([(k("back"), k("Back"))])
        .collect();
    let items: Vec<(&str, &str)> = dest.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
    let mut f = test_frame(&items, MenuKind::List);
    f.menu = Some((22, &items));
    f.menu_head = Some((k("Destination display"), String::new()));
    shots.shoot(
        "vehicle-destinations",
        SIZE,
        page_frame(&f, Some(&ListKind::Destinations), test_column(true)).unwrap(),
        AWAY,
    );
    // a short list (the drivers, the mouse on one), and placing a vehicle (manufacturers)
    let drivers: Vec<(String, String)> = vec![
        (k("driver Ana"), k("Ana  (now)")),
        (k("driver Bruno"), k("Bruno")),
        (k("driver Carla"), k("Carla")),
        (k("back"), k("Back")),
    ];
    let items: Vec<(&str, &str)> = drivers
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::List);
    f.menu_kbd = false;
    f.menu_head = Some((k("Driver"), String::new()));
    shots.shoot(
        "vehicle-drivers-hover",
        SIZE,
        page_frame(&f, Some(&ListKind::Drivers), test_column(true)).unwrap(),
        Vec2::new(700.0, 172.0),
    );
    let makers: Vec<(String, String)> = vec![
        (k("maker man"), k("MAN  (4 models)")),
        (k("maker mb"), k("Mercedes-Benz  (7 models)")),
        (
            k("bus Vehicles/Volvo/7900.bus"),
            k("Volvo  ·  7900 Electric"),
        ),
        (k("maker x"), k("Unknown manufacturer  (2 models)")),
        (k("back"), k("Back")),
    ];
    let items: Vec<(&str, &str)> = makers
        .iter()
        .map(|(a, b)| (a.as_str(), b.as_str()))
        .collect();
    let mut f = test_frame(&items, MenuKind::List);
    f.menu_head = Some((k("Place a vehicle"), String::new()));
    shots.shoot(
        "vehicle-place",
        SIZE,
        page_frame(&f, Some(&ListKind::PlaceMaker), test_column(true)).unwrap(),
        AWAY,
    );
}
