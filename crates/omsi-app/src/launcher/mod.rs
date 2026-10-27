//! The launcher: where a duty is put together (bus, map, line and tour, time, weather),
//! the driver's profile, the settings and key bindings, the running games and the mods.
//!
//! It is a program of its own (the game binary started without a session), drawn with wgpu
//! on a plain device of its own (see `gfx`): none of the game's renderer is opened for it,
//! so the window is there at once. The chosen bus stands in a picture kept from an earlier
//! time and drawn live by a helper process while a page shows it (see `preview`), and the
//! interface - flat and dark, every control custom - is drawn with `omsi-ui` straight
//! onto the window. A session is a process of its own as well (see `state`). The data side (content
//! lists, timetables, profiles, installs, running games) is `omsi-launcher-core`, the same
//! functions `omsi-launcher --cli` offers a terminal.

mod backdrop;
mod chrome;
mod diagnostics;
pub(crate) mod drive;
mod gfx;
mod integrity;
mod keyboard;
pub(crate) mod mapview;
pub mod mobile;
#[cfg(feature = "lan")]
mod multiplayer;
pub(crate) mod pages;
pub mod phone;
pub(crate) mod preview;
mod prerender;
pub(crate) mod settings;
pub(crate) mod setup;
mod showroom;
mod start;
mod state;
#[cfg_attr(not(target_os = "android"), allow(unused_imports))]
pub(crate) use state::crash_of;
mod dialogs;
mod theme;
mod thumbs;
mod timetable;
mod timetable_plan;
mod tray;
mod ui;

use glam::Vec2;
use omsi_launcher_lib as core;
use omsi_launcher_ui::Page as UiPage;
use omsi_ui::paint::Align;
use omsi_ui::{Draw, Rect, Weight};
use std::sync::Arc;
use std::time::Instant;
use theme::*;
use ui::{Key, Ui};
use winit::application::ApplicationHandler;
use winit::event::{DeviceEvent, ElementState, MouseButton, MouseScrollDelta, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::{Window, WindowId};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Page {
    Drive,
    Vehicles,
    Maps,
    Multiplayer,
    Profile,
    Settings,
    Controls,
    Mods,
    Tutorials,
    Timetable,
    Diagnostics,
    Setup,
}

impl Page {
    /// The sidebar's page (`omsi_launcher_ui`): every page but the hidden Multiplayer.
    fn shown(self) -> Option<UiPage> {
        Some(match self {
            Page::Drive => UiPage::Drive,
            Page::Vehicles => UiPage::Vehicles,
            Page::Maps => UiPage::Maps,
            Page::Multiplayer => return None,
            Page::Profile => UiPage::Profile,
            Page::Settings => UiPage::Settings,
            Page::Controls => UiPage::Controls,
            Page::Mods => UiPage::Mods,
            Page::Tutorials => UiPage::Tutorials,
            Page::Timetable => UiPage::Timetable,
            Page::Diagnostics => UiPage::Diagnostics,
            Page::Setup => UiPage::Setup,
        })
    }

    fn of(p: UiPage) -> Page {
        match p {
            UiPage::Drive => Page::Drive,
            UiPage::Vehicles => Page::Vehicles,
            UiPage::Maps => Page::Maps,
            UiPage::Mods => Page::Mods,
            UiPage::Timetable => Page::Timetable,
            UiPage::Profile => Page::Profile,
            UiPage::Controls => Page::Controls,
            UiPage::Tutorials => Page::Tutorials,
            UiPage::Diagnostics => Page::Diagnostics,
            UiPage::Settings => Page::Settings,
            UiPage::Setup => Page::Setup,
        }
    }

    /// The page called `name` (`OMSI_LAUNCHER_PAGE`, the input script), any case.
    fn named(name: &str) -> Option<Page> {
        UiPage::named(name).map(Page::of)
    }
}

#[cfg(not(target_os = "android"))]
type Clipboard = arboard::Clipboard;

/// A phone: text copied in the launcher can be pasted in it (the system's clipboard is
/// Java's).
#[cfg(target_os = "android")]
struct Clipboard(String);

#[cfg(target_os = "android")]
impl Clipboard {
    fn new() -> Result<Clipboard, ()> {
        Ok(Clipboard(String::new()))
    }
    fn get_text(&mut self) -> Result<String, ()> {
        Ok(self.0.clone())
    }
    fn set_text(&mut self, t: String) -> Result<(), ()> {
        self.0 = t;
        Ok(())
    }
}

/// Width of the sidebar (points).
pub const RAIL_W: f32 = omsi_launcher_ui::look::SIDEBAR_W;

/// How often the launcher made its device again after losing it (see `recover_device`).
static LAUNCHER_RECOVERIES: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);

pub struct Launcher {
    instance: wgpu::Instance,
    window: Option<Arc<Window>>,
    surface: Option<gfx::WinSurface>,
    gfx: Option<gfx::Gfx>,
    gpu: Option<omsi_ui::Gpu>,
    ui: Ui,
    state: state::State,
    preview: preview::Preview,
    page: Page,
    page_anim: f32,
    pub drive: drive::DriveView,
    /// The launcher made for a phone (see `phone`).
    pub phone: phone::PhoneView,
    pub pages: pages::PagesView,
    /// The Vehicles and Maps pages (`omsi_launcher_ui`).
    pub vehicles: omsi_launcher_ui::vehicles::VehiclesView,
    pub maps: omsi_launcher_ui::maps::MapsView,
    /// The logo at the top of the sidebar, in the interface pipeline.
    logo: Option<(usize, f32)>,
    /// The graphics adapter the launcher draws with (shown on Diagnostics).
    adapter: String,
    #[cfg(feature = "lan")]
    pub mp: multiplayer::MultiplayerView,
    /// Server icons in the interface pipeline (by server address), and those decoded but
    /// not yet uploaded.
    pub icons: std::collections::HashMap<String, usize>,
    pub icons_pending: Vec<(String, image::RgbaImage)>,
    last: Instant,
    modifiers: ui::Modifiers,
    /// Right or left drag over the showroom.
    dragging: Option<Vec2>,
    clipboard: Option<Clipboard>,
    exit_after: Option<f32>,
    shot: Option<(f32, std::path::PathBuf)>,
    started: Instant,
    /// `OMSI_LAUNCHER_INPUT="t=2 click 400,300; t=3 type Bauern; t=4 key Enter; t=5 shot a.png;
    /// t=6 wheel -3; t=7 move 900,400"`: the window worked by a script (logical pixels).
    script: Vec<(f32, String)>,
    release_next: bool,
    /// Where the bus preview is this frame, and its texture in the interface pipeline.
    preview_rect: Option<Rect>,
    preview_tex: Option<usize>,
    /// The Maps page's picture of the chosen map (from its folder): the map file it is of, the
    /// pixels not yet on the GPU or being read, and its texture with its width over height.
    map_pic: MapPicture,
    /// The small pictures of the Maps library's rows.
    thumbs: thumbs::Thumbs,
    /// The pictures of the library's vehicles drawn ahead of time, and how many were made
    /// when the small pictures were last asked for again (`paint_thumbs`).
    prerender: prerender::Prerender,
    prerender_seen: usize,
    /// The Scene background's picture (Settings > Interface > Background).
    backdrop: backdrop::Backdrop,
    /// The Maps page's start: its typed date and time, and the window open over the page.
    start: start::StartView,
    /// The chosen map's picture (see `mapview`): where it is this frame and its texture.
    pub mapview: mapview::MapView,
    map_rect: Option<Rect>,
    map_tex: Option<usize>,
    map_gen: u64,
    /// The content reading the Maps and Vehicles pages' lists are of (`State::content_gen`).
    catalog_gen: u64,
    /// The window has the keyboard / is hidden: without focus it is drawn ten times a
    /// second, hidden not at all (a game started from it is being played).
    focused: bool,
    occluded: bool,
    /// The window put away while a game started from it runs (the Windows 11 concept): into
    /// the tray where there is one, else minimized; it comes back when that game ends. The
    /// game: its process and when it started (a later process with the same number is
    /// another), and the minute the tray's menu last showed.
    tucked: Option<(u32, u64, u64)>,
    tray: Option<tray::Tray>,
    /// The player came back to the launcher's window while a game runs (clicked it, Alt+Tab):
    /// it is drawn and answers again until the game has the focus back. Before, it stood
    /// still the whole game long, and the session code could not be copied (#825).
    awake_in_game: bool,
    /// The last mouse or key event (an idle launcher draws less often: it kept the GPU busy
    /// at the screen's rate doing nothing).
    last_input: Instant,
    /// A phone's fingers, its storage browser, how far the page is scrolled (and can be),
    /// and whether the on-screen keyboard is up (see `mobile`).
    fingers: mobile::Fingers,
    pub browser: Option<mobile::Browser>,
    page_scroll: f32,
    page_max: f32,
    ime: bool,
    /// Background drop of the graphics device given up while a game runs (see `frame`).
    /// Joined before a new device is opened so the two do not meet on the card.
    gpu_rest_drop: Option<std::thread::JoinHandle<()>>,
    /// The first picture is in the window (logged with how long it took).
    shown_once: bool,
    /// Whether the OMSI 2 folder chosen has what the game needs, for that folder (looked at
    /// again when the window gets the focus back: the status bar asks for it every frame).
    root_ok: Option<(String, bool)>,
    /// The window's own title bar and edges (see `chrome`).
    chrome: chrome::Chrome,
    /// The accent the sidebar's logo was coloured in.
    logo_accent: omsi_ui::theme::Accent,
}

/// Run the launcher window until it is closed.
pub fn run(instance: wgpu::Instance) -> anyhow::Result<()> {
    let event_loop = EventLoop::new()?;
    let mut app = Launcher::new(instance);
    event_loop.run_app(&mut app)?;
    Ok(())
}

impl Launcher {
    /// The launcher, not yet in a window (that comes with `resumed`).
    pub fn new(instance: wgpu::Instance) -> Launcher {
        core::cleanup();
        let mut app = Launcher {
            instance,
            window: None,
            surface: None,
            gfx: None,
            gpu: None,
            ui: Ui::new(),
            state: state::State::new(),
            preview: preview::Preview::new(),
            page: Page::Drive,
            page_anim: 1.0,
            drive: drive::DriveView::default(),
            phone: phone::PhoneView::default(),
            pages: pages::PagesView::default(),
            vehicles: Default::default(),
            maps: Default::default(),
            logo: None,
            adapter: String::new(),
            #[cfg(feature = "lan")]
            mp: multiplayer::MultiplayerView::default(),
            icons: Default::default(),
            icons_pending: Vec::new(),
            last: Instant::now(),
            modifiers: ui::Modifiers::default(),
            dragging: None,
            clipboard: Clipboard::new().ok(),
            // OMSI_LAUNCHER_EXIT=secs, OMSI_LAUNCHER_SHOT=secs:file.png, OMSI_LAUNCHER_PAGE=mods:
            // looking at the window without a person at it
            exit_after: omsi_cfg::flags::LAUNCHER_EXIT.number(),
            shot: omsi_cfg::flags::LAUNCHER_SHOT.text().and_then(|v| {
                v.split_once(':')
                    .map(|(t, f)| (t.parse().unwrap_or(5.0), std::path::PathBuf::from(f)))
            }),
            started: Instant::now(),
            script: omsi_cfg::flags::LAUNCHER_INPUT
                .text()
                .map(|v| {
                    v.split(';')
                        .filter_map(|c| {
                            let c = c.trim();
                            let (t, rest) = c.strip_prefix("t=")?.split_once(' ')?;
                            Some((t.parse().ok()?, rest.trim().to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default(),
            release_next: false,
            preview_rect: None,
            preview_tex: None,
            map_pic: MapPicture::default(),
            thumbs: thumbs::Thumbs::default(),
            prerender: prerender::Prerender::default(),
            prerender_seen: 0,
            backdrop: backdrop::Backdrop::default(),
            start: start::StartView::default(),
            mapview: mapview::MapView::new(),
            map_rect: None,
            map_tex: None,
            map_gen: 0,
            catalog_gen: 0,
            focused: true,
            occluded: false,
            tucked: None,
            tray: None,
            awake_in_game: false,
            last_input: Instant::now(),
            fingers: Default::default(),
            browser: None,
            page_scroll: 0.0,
            page_max: 0.0,
            ime: false,
            gpu_rest_drop: None,
            shown_once: false,
            root_ok: None,
            chrome: Default::default(),
            logo_accent: omsi_ui::theme::current_accent(),
        };
        // (every button of the launcher names itself after the mouse rests on it a second)
        app.ui.auto_tips = true;
        omsi_ui::theme::set_motion(windows_motion());
        // no original installation found anywhere: the launcher still opens, on Setup, and says
        // what it needs (only starting a session needs the game)
        if omsi_cfg::missing_original_essentials(std::path::Path::new(&app.state.config.root)).len()
            > 0
        {
            app.page = Page::Setup;
            let why = state::root_problem(&app.state.config.root);
            app.state.set_status(why, true);
        }
        if let Some(p) = omsi_cfg::flags::LAUNCHER_PAGE.text() {
            if let Some(pg) = Page::named(p.split(':').next().unwrap_or("")) {
                app.go(pg);
                app.page_anim = 1.0;
            }
            // (`OMSI_LAUNCHER_PAGE=more`, `=sheet-bus` …: the phone's More, or one of its sheets)
            match p.as_str() {
                "more" => app.phone.tab = phone::Tab::More,
                "sheet-map" => app.phone.sheet = Some(phone::Sheet::Map),
                "sheet-bus" => app.phone.sheet = Some(phone::Sheet::Bus),
                "sheet-duty" => app.phone.sheet = Some(phone::Sheet::Duty),
                "sheet-time" => app.phone.sheet = Some(phone::Sheet::Time),
                "sheet-livery" => app.phone.sheet = Some(phone::Sheet::Livery),
                _ => {}
            }
            // (the Maps page's windows: maps:start-point, maps:weather, maps:explore)
            let popup = match p.split(':').nth(1) {
                Some("start-point") => Some(start::Popup::StartPoint),
                Some("weather") => Some(start::Popup::Weather),
                Some("explore") => Some(start::Popup::Explore),
                _ => None,
            };
            if let Some(popup) = popup {
                start::open(&mut app, popup);
            }
            // (controls:reload, controls:add: the keyboard's windows)
            match p.split_once(':').filter(|x| x.0 == "controls").map(|x| x.1) {
                Some("reload") => {
                    app.pages.kb.dialog = Some(keyboard::Dialog::Reload {
                        backup: true,
                        pads: true,
                    })
                }
                Some("add") => {
                    app.pages.kb.dialog = Some(keyboard::Dialog::Add {
                        query: String::new(),
                        chosen: None,
                        key: None,
                        waiting: false,
                    })
                }
                // (controls:action=NAME: that action chosen, in the game's keys when it is one of them)
                Some(a) if a.starts_with("action=") => {
                    let a = a["action=".len()..].to_string();
                    let game = app
                        .state
                        .keybindings
                        .get("game")
                        .and_then(|x| x.as_array())
                        .is_some_and(|x| {
                            x.iter().any(|b| {
                                b.get("action").and_then(|v| v.as_str()) == Some(a.as_str())
                            })
                        });
                    app.pages.kb.chosen = Some((game as usize, a));
                }
                _ => {}
            }
            // (settings:q=TEXT: the settings searched for TEXT; settings:changed: the changed ones)
            match p.split(':').nth(1) {
                Some(q) if q.starts_with("q=") => app.pages.set.query = q["q=".len()..].to_string(),
                Some("changed") => app.pages.set.changed_only = true,
                _ => {}
            }
            // (setup:usage: the Content card's disk usage; setup:usage=0,1 a part of it looked into)
            if let Some(u) = p.split_once(':').filter(|x| x.0 == "setup").map(|x| x.1) {
                if let Some(rest) = u.strip_prefix("usage") {
                    app.pages.setup.tab = 1;
                    app.pages.setup.focus = rest
                        .trim_start_matches('=')
                        .split(',')
                        .filter_map(|k| k.parse().ok())
                        .collect();
                }
            }
            // (timetable:stop=NAME: that stop chosen on the map)
            let tt_part = |key: &str| {
                p.strip_prefix("timetable:")
                    .and_then(|rest| rest.split(':').find_map(|x| x.strip_prefix(key)))
            };
            if let Some(stop) = tt_part("stop=") {
                app.pages.tt.set_stop(stop);
            }
            // (timetable:line=NAME: that line chosen, timetable:find=TEXT: that typed in the
            // map's search)
            if let Some(line) = tt_part("line=") {
                app.pages.tt.set_line(line);
            }
            if let Some(q) = tt_part("find=") {
                app.pages.tt.set_find(q);
            }
            // (profile:drive: the chosen drive's details over Profile)
            if p.split(':').nth(1) == Some("drive") {
                app.pages.drive_details = true;
            }
            if let Some(step) = p.split(':').nth(1).and_then(|s| s.parse::<usize>().ok()) {
                // (the Drive page's second part is which of its three steps: drive:2 the map)
                app.drive.tab = step.min(2);
                // (the Controls page's second part is its tab, controls:1 the game controllers; the
                // Settings page's its section, settings:5 Sound)
                app.pages.controls_tab = step;
                app.pages.set.sec = step.min(settings::SECTIONS.len() - 1);
                // (and the Timetable page's its tab, timetable:1 the schematic)
                app.pages.tt.tab = step.min(2);
            }
        }
        app
    }

    /// Everything made on the graphics device goes with it: the interface's textures and every
    /// number kept for one of them - the bus preview, the map picture (and the map's own
    /// drawing), the servers' icons. A number kept over a device made anew pointed past the
    /// new device's textures, and the map was drawn with the font atlas instead: the Drive
    /// page's map full of the interface's words after a game (the launcher gives its device
    /// up while one runs) or a lost device.
    fn drop_gpu(&mut self) {
        self.gpu = None;
        self.preview_tex = None;
        self.preview = preview::Preview::new();
        // (the helper drawing pictures ahead of time is not left up for the game)
        self.prerender.suspend();
        self.map_tex = None;
        self.map_pic = MapPicture::default();
        self.thumbs.forget_textures();
        self.backdrop.forget_texture();
        self.map_gen = 0;
        self.mapview.drop_gpu();
        self.icons.clear();
        self.logo = None;
    }

    /// The window, its surface and the renderer, given up for the game (a phone plays in the
    /// launcher's window).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn release_window(&mut self) -> Option<Arc<Window>> {
        self.pages.pads.release_io();
        self.surface = None;
        self.drop_gpu();
        if let Some(h) = self.gpu_rest_drop.take() {
            let _ = h.join();
        }
        self.gfx = None;
        self.ime = false;
        self.window.take()
    }

    /// Back from a game: the window again (the launcher draws into it from the next resume).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn adopt_window(&mut self, window: Arc<Window>) {
        self.window = Some(window);
        self.surface = None;
        self.gfx = None;
        self.state.poll_now();
        self.state.load_profile();
    }

    /// The surface for the window the launcher has (created again after the app was in the
    /// background: a phone takes the window's surface away meanwhile).
    /// The interface's pipeline on the launcher's device, with the sidebar's logo in it.
    fn new_gpu(&mut self, gfx: &gfx::Gfx) -> omsi_ui::Gpu {
        let mut gpu = omsi_ui::Gpu::new(&gfx.device, gfx.format(), 4, self.ui.atlas.size);
        self.logo = omsi_launcher_ui::offscreen::load_logo(&mut gpu, &gfx.device, &gfx.queue);
        self.logo_accent = omsi_ui::theme::current_accent();
        self.adapter = gfx.adapter_name.clone();
        gpu
    }

    fn make_surface(&mut self) {
        let Some(window) = self.window.clone() else {
            return;
        };
        if self.gfx.is_none() {
            // Finish dropping the device given up for a game before opening another one.
            if let Some(h) = self.gpu_rest_drop.take() {
                let _ = h.join();
            }
            let g = match gfx::Gfx::for_window(&mut self.instance, &window) {
                Ok(g) => g,
                Err(e) => {
                    crate::startup::fatal_message(&format!(
                        "Project Depot cannot draw on this computer: {e:#}"
                    ));
                    std::process::exit(1);
                }
            };
            self.gpu = Some(self.new_gpu(&g));
            self.ui.atlas = omsi_ui::Atlas::new(self.ui.atlas.size);
            self.gfx = Some(g);
        }
        let Some(g) = self.gfx.as_ref() else { return };
        let size = window.inner_size();
        self.surface =
            gfx::WinSurface::new(&self.instance, window.clone(), g, size.width, size.height).ok();
        self.last = Instant::now();
    }
}

impl ApplicationHandler for Launcher {
    fn device_event(
        &mut self,
        _event_loop: &ActiveEventLoop,
        _id: winit::event::DeviceId,
        event: DeviceEvent,
    ) {
        if matches!(event, DeviceEvent::Added | DeviceEvent::Removed) {
            if let Some(io) = self.pages.pads.io.as_ref() {
                io.refresh();
            }
        }
    }

    fn suspended(&mut self, _event_loop: &ActiveEventLoop) {
        // (a phone: the app went to the background and its window's surface goes with it)
        self.surface = None;
    }

    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            if self.surface.is_none() {
                self.make_surface();
            }
            return;
        }
        // (`OMSI_LAUNCHER_SIZE=WxH`: another window size, for looking at the layout)
        let asked = omsi_cfg::flags::LAUNCHER_SIZE.text().and_then(|v| {
            v.split_once('x')
                .and_then(|(a, b)| Some((a.parse::<f64>().ok()?, b.parse::<f64>().ok()?)))
        });
        let (fit, at) = match asked {
            Some((iw, ih)) => (winit::dpi::LogicalSize::new(iw, ih), None),
            None => crate::startup::fit_window(event_loop, 1440.0, 880.0, None),
        };
        let mut attrs = Window::default_attributes()
            .with_title("Project Depot")
            .with_window_icon(crate::startup::window_icon())
            .with_inner_size(fit);
        if !mobile::mobile() {
            // (the launcher draws its own title bar, as Windows 11 apps do: see `chrome`)
            attrs = attrs.with_decorations(!chrome::own_frame());
            // (no bigger than the window fitted to the screen: a small one at 150 % has less)
            attrs = attrs.with_min_inner_size(winit::dpi::LogicalSize::new(
                1080.0f64.min(fit.width),
                680.0f64.min(fit.height),
            ));
            if let Some(at) = at {
                attrs = attrs.with_position(at);
            }
        }
        if omsi_cfg::flags::BACKGROUND.is_set() {
            attrs = attrs.with_active(false);
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => {
                crate::startup::fatal_message(&format!(
                    "Project Depot cannot open its window: {e}"
                ));
                event_loop.exit();
                return;
            }
        };
        let g = match gfx::Gfx::for_window(&mut self.instance, &window) {
            Ok(g) => g,
            Err(e) => {
                crate::startup::fatal_message(&format!(
                    "Project Depot cannot draw on this computer: {e:#}"
                ));
                event_loop.exit();
                return;
            }
        };
        let size = window.inner_size();
        let surface =
            match gfx::WinSurface::new(&self.instance, window.clone(), &g, size.width, size.height)
            {
                Ok(s) => s,
                Err(e) => {
                    crate::startup::fatal_message(&format!(
                        "Project Depot cannot draw into its window: {e:#}"
                    ));
                    event_loop.exit();
                    return;
                }
            };
        log::info!("launcher window {}x{} (scale {:.2}), adapter {}, open {:.0} ms after the program started", size.width, size.height, window.scale_factor(), g.adapter_name, crate::startup::since_start_ms());
        self.gpu = Some(self.new_gpu(&g));
        self.window = Some(window);
        self.surface = Some(surface);
        self.gfx = Some(g);
        self.last = Instant::now();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let scale = self.ui_scale();
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.last_input = Instant::now();
        }
        match event {
            WindowEvent::CloseRequested => self.quit(event_loop),
            WindowEvent::Touch(t) => self.touch(t, scale),
            WindowEvent::Focused(f) => self.set_focus(f),
            WindowEvent::Occluded(o) => {
                self.occluded = o;
                if o {
                    self.pages.pads.cancel_feedback_test();
                } else if self.gfx.is_none() {
                    // Back in view without a graphics device (a game just ended, or the
                    // window was covered while resting): draw again so the device is opened
                    // without waiting for the next slow occluded tick.
                    if let Some(w) = self.window.as_ref() {
                        w.request_redraw();
                    }
                }
            }
            WindowEvent::Resized(s) => {
                if let (Some(sf), Some(r)) = (self.surface.as_mut(), self.gfx.as_ref()) {
                    sf.resize(r, s.width, s.height);
                }
            }
            WindowEvent::ModifiersChanged(m) => {
                self.modifiers.told(m.state());
                self.modifiers.apply(&mut self.ui.input);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let p = Vec2::new(position.x as f32, position.y as f32) / scale;
                if let Some(last) = self.dragging {
                    let d = p - last;
                    self.preview.orbit(d.x, d.y);
                    self.dragging = Some(p);
                }
                self.ui.input.mouse = p;
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let down = state == ElementState::Pressed;
                match button {
                    MouseButton::Left => {
                        if down {
                            self.ui.input.pressed = true;
                            // a drag on the preview turns the bus (the panels lie over it:
                            // what the mouse is over is theirs, not the bus's)
                            if (!self.ui.over_ui || self.page == Page::Vehicles)
                                && self
                                    .preview_rect
                                    .map(|r| r.contains(self.ui.input.mouse))
                                    .unwrap_or(false)
                            {
                                self.dragging = Some(self.ui.input.mouse);
                            }
                        } else {
                            self.ui.input.released = true;
                            self.dragging = None;
                        }
                        self.ui.input.down = down;
                    }
                    MouseButton::Right => {
                        self.ui.input.right_down = down;
                        if down {
                            self.ui.input.right_pressed = true;
                            if self
                                .preview_rect
                                .map(|r| r.contains(self.ui.input.mouse))
                                .unwrap_or(false)
                            {
                                self.dragging = Some(self.ui.input.mouse);
                            }
                        } else {
                            self.dragging = None;
                        }
                    }
                    _ => {}
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let d = match delta {
                    MouseScrollDelta::LineDelta(x, y) => Vec2::new(x, y),
                    MouseScrollDelta::PixelDelta(p) => Vec2::new(p.x as f32, p.y as f32) / 40.0,
                };
                self.ui.input.wheel += d;
            }
            WindowEvent::KeyboardInput { event, .. } => {
                // (Shift, Ctrl, Alt from their keys where the window never says: Android)
                if let PhysicalKey::Code(code) = event.physical_key {
                    if self
                        .modifiers
                        .key(code, event.state == ElementState::Pressed)
                    {
                        self.modifiers.apply(&mut self.ui.input);
                    }
                }
                if event.state != ElementState::Pressed {
                    return;
                }
                let cmd = self.modifiers.command();
                // a phone's back key: out of the storage browser, else like Escape
                if event.physical_key == PhysicalKey::Code(KeyCode::BrowserBack) {
                    if self.browser.is_some() {
                        self.browser = None;
                    } else {
                        self.ui.input.keys.push(Key::Escape);
                    }
                    return;
                }
                if let PhysicalKey::Code(code) = event.physical_key {
                    self.ui.input.raw_key = Some(code);
                    let k = match code {
                        KeyCode::ArrowLeft => Some(Key::Left),
                        KeyCode::ArrowRight => Some(Key::Right),
                        KeyCode::ArrowUp => Some(Key::Up),
                        KeyCode::ArrowDown => Some(Key::Down),
                        KeyCode::Home => Some(Key::Home),
                        KeyCode::End => Some(Key::End),
                        KeyCode::Backspace => Some(Key::Backspace),
                        KeyCode::Delete => Some(Key::Delete),
                        KeyCode::Enter | KeyCode::NumpadEnter => Some(Key::Enter),
                        KeyCode::Escape => Some(Key::Escape),
                        KeyCode::Tab => Some(Key::Tab),
                        KeyCode::KeyA if cmd => Some(Key::SelectAll),
                        KeyCode::KeyC if cmd => Some(Key::Copy),
                        KeyCode::KeyV if cmd => Some(Key::Paste),
                        KeyCode::KeyX if cmd => Some(Key::Cut),
                        _ => None,
                    };
                    if let Some(k) = k {
                        if k == Key::Paste {
                            self.ui.clipboard_in =
                                self.clipboard.as_mut().and_then(|c| c.get_text().ok());
                        }
                        self.ui.input.keys.push(k);
                    }
                }
                if !cmd {
                    if let Some(t) = event.text.as_ref() {
                        self.ui.input.text.push_str(t);
                    }
                }
            }
            WindowEvent::DroppedFile(path) => {
                // a .zip dropped on the window is installed (a folder is refused:
                // `State::install` says why)
                self.page = Page::Mods;
                self.state.install(path.to_string_lossy().to_string());
            }
            WindowEvent::HoveredFile(_) => self.pages.drop_hover = true,
            WindowEvent::HoveredFileCancelled => self.pages.drop_hover = false,
            WindowEvent::RedrawRequested => {
                self.frame(event_loop);
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.check_exit(event_loop);
        self.tray_tick(event_loop);
        // (a screenshot asked for by a script is drawn even when hidden, and so is the frame
        // that gives the graphics device up again when the game is back in front: the
        // game's window hides the launcher's then, and drawn nothing, it kept the device)
        // (likewise the frame that opens it again for a window brought forward)
        let resting =
            !mobile::mobile() && self.gfx.is_some() && self.state.in_game() && !self.awake();
        let waking =
            !mobile::mobile() && self.gfx.is_none() && self.state.in_game() && self.awake();
        // Device given up while a game runs: still need a redraw when the game ends so the
        // device is opened again (Occluded alone used to leave the resting picture forever).
        let resume_needed = !mobile::mobile() && self.gfx.is_none() && !self.state.in_game();
        // (a minimized window is hidden too, whether or not the system said it was covered,
        // and so is one put into the tray)
        let minimized = self
            .window
            .as_ref()
            .and_then(|w| w.is_minimized())
            .unwrap_or(false)
            || self.window.as_ref().and_then(|w| w.is_visible()) == Some(false);
        let occluded = (self.occluded || minimized)
            && self.shot.is_none()
            && !resting
            && !waking
            && !resume_needed
            && !self.script.iter().any(|(_, c)| c.starts_with("shot"));
        let interval = if occluded {
            0.5
        } else if !self.focused && !omsi_cfg::flags::BACKGROUND.is_set() {
            0.1
        } else if self.last_input.elapsed().as_secs_f32() > 3.0
            && !self.ui.moving()
            && self.dragging.is_none()
            && self.script.is_empty()
        {
            // idle: 20 frames a second keep the preview and the progress bars moving
            0.05
        } else {
            0.0
        };
        let since = self.last.elapsed().as_secs_f32();
        if interval > 0.0 && since < interval {
            event_loop.set_control_flow(winit::event_loop::ControlFlow::WaitUntil(
                self.last + std::time::Duration::from_secs_f32(interval),
            ));
            return;
        }
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);
        if occluded {
            // nothing to draw: keep the data side going (polls, installs, the script)
            let dt = since.min(1.0);
            self.last = Instant::now();
            self.run_script();
            self.state.update(dt);
            // (no page shows the preview while the window is hidden: its helper ends at once)
            self.preview.stop();
            self.prerender.suspend();
            self.check_exit(event_loop);
            // A game may have ended on this tick: wake so `frame` opens the device again.
            if !mobile::mobile() && self.gfx.is_none() && !self.state.in_game() {
                if let Some(w) = self.window.as_ref() {
                    w.request_redraw();
                }
            }
        } else if let Some(w) = self.window.as_ref() {
            w.request_redraw();
        }
    }
}

/// Seconds since 1970.
fn now_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// How long a game started at `t` (s since 1970) has run, in whole minutes: the tray's menu
/// shows it, brought up to date once a minute.
fn run_time(t: u64) -> String {
    let m = now_secs().saturating_sub(t) / 60;
    if m < 60 {
        omsi_ui::tr("%{m} min").replace("%{m}", &m.to_string())
    } else {
        omsi_ui::tr("%{h} h %{m} min")
            .replace("%{h}", &(m / 60).to_string())
            .replace("%{m}", &(m % 60).to_string())
    }
}

/// A tour's days as the core puts them ("Mon-Fri & holidays, school days"), in the
/// player's language, piece by piece.
pub(crate) fn days_text(d: &str) -> String {
    let (head, school) = match (
        d.strip_suffix(", school holidays"),
        d.strip_suffix(", school days"),
    ) {
        (Some(h), _) => (h, Some("school holidays")),
        (_, Some(h)) => (h, Some("school days")),
        _ => (d, None),
    };
    let (week, holidays) = match head.strip_suffix(" & holidays") {
        Some(w) => (w, true),
        None if head == "holidays" => ("", true),
        None => (head, false),
    };
    let mut out = week
        .split(", ")
        .filter(|x| !x.is_empty())
        .map(|x| omsi_ui::tr(x).into_owned())
        .collect::<Vec<_>>()
        .join(", ");
    if holidays {
        out = if out.is_empty() {
            omsi_ui::tr("holidays").into_owned()
        } else {
            format!("{out} & {}", omsi_ui::tr("holidays"))
        };
    }
    if let Some(x) = school {
        out = format!("{out}, {}", omsi_ui::tr(x));
    }
    out
}

/// A weather's precipitation as the core puts it ("rain 40%", "snow 10%", "dry"), in the
/// player's language.
pub(crate) fn precip_text(p: &str) -> String {
    match (p.strip_prefix("rain "), p.strip_prefix("snow ")) {
        (Some(n), _) => omsi_ui::tr("rain %{n}%").replace("%{n}", n.trim_end_matches('%')),
        (_, Some(n)) => omsi_ui::tr("snow %{n}%").replace("%{n}", n.trim_end_matches('%')),
        _ => omsi_ui::tr(p).into_owned(),
    }
}

/// Whether the program can bring its window back from the taskbar by itself (not on
/// Wayland, where only the player can).
fn can_restore(w: &Window) -> bool {
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    !matches!(
        w.window_handle().map(|h| h.as_raw()),
        Ok(RawWindowHandle::Wayland(_))
    )
}

impl Launcher {
    /// The launcher ends: the window's close button, or the tray's Quit (the games keep
    /// running).
    fn quit(&mut self, event_loop: &ActiveEventLoop) {
        self.pages.pads.cancel_feedback_test();
        // (the icon leaves the tray before the process ends)
        self.tray = None;
        event_loop.exit();
    }

    /// While a game started from here runs the launcher's window goes into the tray
    /// (Windows; elsewhere it is minimized), and comes back when that game ends; the tray's
    /// menu is answered here.
    fn tray_tick(&mut self, event_loop: &ActiveEventLoop) {
        if mobile::mobile() || self.shot.is_some() {
            return;
        }
        let Some(w) = self.window.clone() else { return };
        let game = |l: &Launcher, pid: u32, started: u64| {
            l.state
                .instances
                .iter()
                .find(|i| i.running && i.pid == pid && i.started == started)
                .cloned()
        };
        let head = |i: &core::Instance| {
            format!(
                "Project Depot  \u{b7}  {}  \u{b7}  {}",
                state::short_map(&i.map),
                run_time(i.started)
            )
        };
        if let Some((pid, started, shown)) = self.tucked {
            match game(self, pid, started) {
                None => {
                    self.tray = None;
                    self.tucked = None;
                    w.set_visible(true);
                    w.set_minimized(false);
                    w.focus_window();
                }
                Some(i) => {
                    // (the menu's first line says how long it has run: brought up to date
                    // once a minute)
                    let minute = now_secs() / 60;
                    if minute != shown {
                        if let Some(t) = self.tray.as_ref() {
                            t.set_head(&head(&i));
                        }
                        self.tucked = Some((pid, started, minute));
                    }
                }
            }
        }
        if self.tucked.is_none() {
            let mine = self.state.started_here.and_then(|pid| {
                self.state
                    .instances
                    .iter()
                    .find(|i| i.running && i.pid == pid)
                    .cloned()
            });
            if let Some(i) = mine {
                self.state.started_here = None;
                self.tray =
                    tray::Tray::new(&omsi_ui::tr("Project Depot: a game is running"), &head(&i));
                if self.tray.is_some() {
                    w.set_visible(false);
                } else if can_restore(&w) {
                    // (where there is no tray; on Wayland a window minimized cannot be brought
                    // back by the program, so it stays where it is there)
                    w.set_minimized(true);
                }
                self.tucked = Some((i.pid, i.started, now_secs() / 60));
            }
        }
        while let Some(cmd) = self.tray.as_ref().and_then(|t| t.take()) {
            let Some((pid, started, _)) = self.tucked else {
                break;
            };
            match cmd {
                tray::TrayCmd::Show => {
                    w.set_visible(true);
                    w.set_minimized(false);
                    w.focus_window();
                    w.request_redraw();
                }
                tray::TrayCmd::OpenLog => {
                    // (the game the menu names)
                    let log = game(self, pid, started).map(|i| i.log);
                    if !log.is_some_and(|l| core::open_file(std::path::Path::new(&l))) {
                        self.state
                            .set_status("The game's log could not be opened.", true);
                    }
                }
                tray::TrayCmd::StopGame => {
                    if game(self, pid, started).is_some() {
                        self.state.stop(pid);
                    }
                }
                tray::TrayCmd::Quit => {
                    self.quit(event_loop);
                    break;
                }
            }
        }
    }

    /// Physical pixels per interface pixel: the screen's scale, times a zoom that makes the
    /// interface (laid out for a 1440 x 880 window) grow with a bigger window and shrink a
    /// little with a smaller one, so that it fills the window the same way at any size.
    fn ui_scale(&self) -> f32 {
        let Some(w) = self.window.as_ref() else {
            return 1.0;
        };
        let dpi = w.scale_factor() as f32;
        let s = w.inner_size();
        let (lw, lh) = (s.width as f32 / dpi, s.height as f32 / dpi);
        if mobile::mobile() {
            // a phone held across: the text at least at the system's own size - smaller, it
            // was hard to read and the buttons hard to hit (the pages scroll where the screen
            // is lower than they are, and lay themselves out for its width), a tablet larger
            return dpi * (lh / 400.0).clamp(1.0, 1.35);
        }
        // (the height counts a little less: on a wide, low screen - 2560 x 1080 - the text
        // stayed the size of a 1440 x 880 window's, tiny across the width; the pages scroll or
        // keep their width, see `draw_ui`)
        dpi * (lw / 1440.0).min(lh / 820.0).clamp(0.8, 2.2)
    }

    /// The graphics device was lost (#274: an AMD Radeon's DX12 driver gave up while the
    /// preview's textures went up, and the launcher then drew on the dead device, with
    /// thousands of errors a second): everything made on it goes, the other interface is
    /// taken - DirectX 12 and Vulkan for each other, remembered in the settings for the game
    /// as well - and the window is drawn again on a new device. Twice at most.
    fn recover_device(&mut self) -> bool {
        let Some(why) = self.gfx.as_ref().and_then(|r| r.device_lost()) else {
            return false;
        };
        let tries = LAUNCHER_RECOVERIES.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let name = self
            .gfx
            .as_ref()
            .map(|r| r.adapter_name.clone())
            .unwrap_or_default();
        let other = if name.contains("(Dx12)") {
            Some("vulkan")
        } else if name.contains("(Vulkan)") && cfg!(windows) {
            Some("dx12")
        } else if name.contains("(Vulkan)") {
            Some("gl")
        } else {
            None
        };
        log::error!(
            "launcher: the graphics device was lost on {name} ({why}); {}",
            match (tries < 2, other) {
                (true, Some(o)) => format!("drawing on {o} from now on"),
                (true, None) => "drawing on a new device".to_string(),
                _ => "giving up".to_string(),
            }
        );
        if tries >= 2 {
            return false;
        }
        if let Some(o) = other {
            std::env::set_var(omsi_cfg::flags::BACKEND.name, o);
            self.state.settings["graphics_api"] = serde_json::json!(o);
            self.state.settings_dirty = 0.3;
        }
        self.surface = None;
        self.drop_gpu();
        if let Some(h) = self.gpu_rest_drop.take() {
            let _ = h.join();
        }
        // (the lost device let go of without a panic of wgpu's reaching the launcher)
        if let Some(g) = self.gfx.take() {
            g.close();
        }
        self.make_surface();
        true
    }

    /// The window got or lost the keyboard (`WindowEvent::Focused`, or `focus 0/1` of a
    /// launcher script).
    fn set_focus(&mut self, f: bool) {
        self.focused = f;
        if f {
            self.root_ok = None;
            // (Windows' "Animation effects" may have changed while the window was away)
            omsi_ui::theme::set_motion(windows_motion());
        }
        if !f {
            self.pages.pads.cancel_feedback_test();
            self.modifiers.release_keys();
            self.modifiers.apply(&mut self.ui.input);
        }
        // (only once the game is on its way: the launcher has the focus while Start is
        // pressed, and gives the device up then as before)
        self.awake_in_game =
            f && self.gfx.is_none() && self.state.in_game() && self.state.queued_launch.is_none();
    }

    /// Looked at while a game runs (see `awake_in_game`): drawn and answering as usual.
    fn awake(&self) -> bool {
        // (the player asked the launcher not to rest while a game runs: it is always awake)
        !self.rests() || (self.awake_in_game && self.focused && self.state.queued_launch.is_none())
    }

    /// Whether the launcher gives the graphics device up while a game runs (#834: the setting
    /// "The launcher rests while a game runs"; on by default).
    fn rests(&self) -> bool {
        self.state
            .settings
            .get("launcher_rest")
            .and_then(|v| v.as_bool())
            .unwrap_or(true)
    }

    fn frame(&mut self, event_loop: &ActiveEventLoop) {
        if self.recover_device() {
            return;
        }
        let desktop = !mobile::mobile() && self.window.is_some();
        // (looked at while a game runs: drawn as usual, see `awake_in_game`)
        let awake = self.awake();
        if desktop && self.gfx.is_none() {
            if self.state.in_game() && !awake {
                // nothing is drawn while a game runs; what is clicked or typed meanwhile is not
                // done once the launcher is back (the first frame pressed Start again)
                let now = Instant::now();
                let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
                self.last = now;
                self.run_script();
                self.state.update(dt);
                self.ui.discard_input();
                // `update` may have seen the game end: open the device this frame instead of
                // returning with the resting picture until another redraw happens to notice.
                if self.state.in_game() && !self.awake() {
                    return;
                }
            }
            if self.state.in_game() {
                log::info!("launcher: its window is looked at while a game runs, the graphics device is opened again");
                // (what was clicked while it stood still is not done: the click that brought
                // it forward pressed whatever lay under it)
                self.ui.discard_input();
            } else {
                self.awake_in_game = false;
                log::info!("launcher: no game runs any more, the graphics device is opened again");
            }
            self.make_surface();
        }
        self.draw_frame(event_loop);
        // a game starts or runs: the frame just drawn says so and stays in the window, and the
        // graphics device is given up until the game ends (with it open, a game on an NVIDIA
        // card without Resizable BAR uploaded at 20 MB/s)
        // (asked again: a script's `focus 0` comes in the frame just drawn)
        if desktop && self.gfx.is_some() && self.state.in_game() && !self.awake() {
            log::info!(
                "launcher: a game starts or runs, the graphics device is given up until it ends"
            );
            self.surface = None;
            self.drop_gpu();
            // Dropping a wgpu device can wait on the GPU for seconds (especially while the
            // game is opening the same card). Do it off the UI thread so Windows does not
            // mark the launcher "Not Responding" over the resting picture.
            if let Some(r) = self.gfx.take() {
                if let Some(prev) = self.gpu_rest_drop.take() {
                    let _ = prev.join();
                }
                self.gpu_rest_drop = std::thread::Builder::new()
                    .name("launcher-gpu-rest".into())
                    .spawn(move || drop(r))
                    .ok();
            }
        }
        if let Some(mut d) = self.state.queued_launch.take() {
            // the game opens where the launcher is
            d.monitor_at = self.window.as_ref().and_then(|w| {
                let (p, s) = (w.outer_position().ok()?, w.inner_size());
                Some((p.x + (s.width / 2) as i32, p.y + (s.height / 2) as i32))
            });
            // The Controls page may still own the same DirectInput wheel non-exclusively.
            // Drop it before the child asks for exclusive foreground access for force feedback.
            self.pages.pads.release_io();
            self.state.spawn_launch(d);
        }
    }

    /// The launcher's picture, put on the window.
    fn draw_frame(&mut self, event_loop: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32().min(0.1);
        self.last = now;
        let (Some(window), Some(_)) = (self.window.clone(), self.surface.as_ref()) else {
            return;
        };
        let scale = self.ui_scale();
        let phys = window.inner_size();
        let (pw, ph) = (phys.width.max(1), phys.height.max(1));
        let size = Vec2::new(pw as f32, ph as f32) / scale;

        self.run_script();
        self.state.update(dt);
        // the preview shows the chosen bus in the chosen light
        // (the Vehicles page shows the bus picked in its library instead, see `vehicles_page`)
        let c = &self.state.choice;
        let look = showroom::Look {
            root: std::path::PathBuf::from(&self.state.config.root),
            map: c.map.clone(),
            bus: c.bus.clone(),
            paint: c.paint.clone(),
            weather: c.weather.clone(),
            time: c.time,
            date: c.date.clone(),
        };
        // (not while a game runs: the launcher looked at meanwhile loads no bus onto the card)
        // (the Vehicles page shows the bus picked in its library, live: see `vehicles_page`)
        if !look.bus.is_empty()
            && !look.map.is_empty()
            && !self.state.in_game()
            && self.page != Page::Vehicles
        {
            self.preview.want_picture(look);
        }

        // --- the interface
        self.preview_rect = None;
        self.map_rect = None;
        let own_frame = chrome::own_frame();
        if own_frame {
            self.chrome.maximized = window.is_maximized();
            self.chrome_before(&window, size);
        }
        self.ui.begin(size, scale, dt);
        self.draw_ui();
        if own_frame && self.chrome_after(&window, size) {
            self.pages.pads.cancel_feedback_test();
            event_loop.exit();
        }
        if mobile::mobile() {
            // what no list took of a finger's drag scrolls the page
            if !self.ui.wheel_taken() && self.browser.is_none() {
                self.page_scroll =
                    (self.page_scroll - self.ui.input.wheel.y * 42.0).clamp(0.0, self.page_max);
            }
            // the on-screen keyboard while a text field has the focus
            let want = self.ui.focus.is_some();
            if want != self.ime {
                self.ime = want;
                window.set_ime_allowed(want);
            }
        }
        window.set_cursor(if self.dragging.is_some() {
            winit::window::CursorIcon::Grabbing
        } else {
            self.ui.cursor
        });
        if let Some(t) = self.ui.clipboard_out.take() {
            if let Some(c) = self.clipboard.as_mut() {
                let _ = c.set_text(t);
            }
        }
        let (layers, verts, ranges) = self.ui.finish();
        self.touch_frame();
        // the helper draws the bus while a page shows it (not while a game runs)
        let shown = self
            .preview_rect
            .filter(|_| !self.state.in_game())
            .map(|r| {
                (
                    ((r.w * scale) as u32).max(16),
                    ((r.h * scale) as u32).max(16),
                )
            });
        self.preview.update(dt, shown);
        self.prerender_step(dt);

        // --- to the GPU: the preview when it changed, then the interface onto the window
        let Some(renderer) = self.gfx.as_mut() else {
            return;
        };
        let frame = self.preview.take_frame();
        // (the bus as the preview drew it, behind the Vehicles page with the Scene background:
        // blurred even while that is off, as no other frame may come once it is switched on)
        if let (Some((w, h, px)), Some(tag)) = (frame.as_ref(), self.preview.showing()) {
            self.backdrop.offer_bus(tag, *w, *h, px);
        }
        self.backdrop.update();
        self.thumbs.update();
        if let (Some((w, h, px)), Some(gpu)) = (frame, self.gpu.as_mut()) {
            match self.preview_tex {
                Some(id) => gpu.update_image(&renderer.device, &renderer.queue, id, w, h, &px),
                None => {
                    self.preview_tex =
                        Some(gpu.add_image(&renderer.device, &renderer.queue, w, h, &px))
                }
            }
        }
        if let Some(gpu) = self.gpu.as_mut() {
            self.map_pic.upload(gpu, &renderer.device, &renderer.queue);
            self.thumbs.upload(gpu, &renderer.device, &renderer.queue);
            self.backdrop.upload(gpu, &renderer.device, &renderer.queue);
            // (the logo's coloured part wears the accent: drawn again when another is chosen)
            let accent = omsi_ui::theme::current_accent();
            if let (Some((tex, _)), true) = (self.logo, accent != self.logo_accent) {
                omsi_launcher_ui::offscreen::tint_logo(gpu, &renderer.device, &renderer.queue, tex);
                self.logo_accent = accent;
            }
        }
        // the map picture, drawn again when what it shows, where it looks or the zoom's own
        // thinness changed
        if self.map_rect.is_some() {
            if let Some(view) = self.mapview.picture(renderer) {
                if let Some(gpu) = self.gpu.as_mut() {
                    if self.map_gen != self.mapview.generation {
                        self.map_gen = self.mapview.generation;
                        let size = self.mapview.pixels();
                        match self.map_tex {
                            Some(id) => gpu.set_view(&renderer.device, id, &view, size),
                            None => {
                                self.map_tex = Some(gpu.add_view(&renderer.device, &view, size))
                            }
                        }
                    }
                }
            }
        }
        // server icons decoded this frame go to the GPU (drawn from the next)
        for (addr, img) in std::mem::take(&mut self.icons_pending) {
            if let Some(gpu) = self.gpu.as_mut() {
                let (w, h) = img.dimensions();
                let tex = renderer.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("server icon"),
                    size: wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: wgpu::TextureFormat::Rgba8UnormSrgb,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                });
                renderer.queue.write_texture(
                    tex.as_image_copy(),
                    &img,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4 * w),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );
                let view = tex.create_view(&Default::default());
                let id = gpu.add_view(&renderer.device, &view, (w, h));
                self.icons.insert(addr, id);
            }
        }
        let draws: Vec<Draw> = ranges
            .iter()
            .enumerate()
            .map(|(k, (r, tex))| Draw {
                buffer: 0,
                range: r.clone(),
                layer: k,
                texture: *tex,
            })
            .collect();
        // (the launcher's background, `look::BG`, in linear light)
        let bg = wgpu::Color {
            r: 0.0052,
            g: 0.0060,
            b: 0.0075,
            a: 1.0,
        };
        if let Some(gpu) = self.gpu.as_mut() {
            gpu.upload(&renderer.device, &renderer.queue, 0, &verts);
            gpu.upload_atlas(&renderer.queue, &mut self.ui.atlas);
        }
        // OMSI_LAUNCHER_SHOT: the window's picture into a file (drawn into a texture of its
        // own, so a hidden window gives one too)
        let t = self.started.elapsed().as_secs_f32();
        if let Some((at, file)) = self.shot.clone() {
            if t >= at {
                self.shot = None;
                if let Some(img) = self.shot_image(pw, ph, &layers, &draws, bg) {
                    let _ = img.save(&file);
                    log::info!("launcher: picture written to {}", file.display());
                }
            }
        }
        let Some(renderer) = self.gfx.as_mut() else {
            return;
        };
        let surface = self.surface.as_mut().unwrap();
        let frame = match surface.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(f)
            | wgpu::CurrentSurfaceTexture::Suboptimal(f) => f,
            wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost => {
                surface.resize(renderer, pw, ph);
                return;
            }
            _ => return,
        };
        let view = frame.texture.create_view(&Default::default());
        if let Some(gpu) = self.gpu.as_mut() {
            let mut enc = renderer
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("launcher"),
                });
            gpu.render(
                &renderer.device,
                &renderer.queue,
                &mut enc,
                &view,
                (pw, ph),
                Some(bg),
                &layers,
                &draws,
            );
            renderer.queue.submit([enc.finish()]);
        }
        window.pre_present_notify();
        frame.present();
        if !self.shown_once {
            self.shown_once = true;
            log::info!(
                "launcher: first picture in the window {:.0} ms after the program started",
                crate::startup::since_start_ms()
            );
        }
        self.check_exit(event_loop);
    }

    fn check_exit(&mut self, event_loop: &ActiveEventLoop) {
        if self
            .exit_after
            .map(|e| self.started.elapsed().as_secs_f32() >= e)
            .unwrap_or(false)
        {
            event_loop.exit();
        }
    }

    fn run_script(&mut self) {
        if self.release_next {
            self.ui.input.released = true;
            self.ui.input.down = false;
            self.release_next = false;
        }
        let t = self.started.elapsed().as_secs_f32();
        while let Some((at, cmd)) = self.script.first().cloned() {
            if at > t {
                break;
            }
            self.script.remove(0);
            log::info!("launcher input t={at}: {cmd}");
            let (verb, arg) = cmd.split_once(' ').unwrap_or((cmd.as_str(), ""));
            let xy = || {
                let mut it = arg
                    .split(',')
                    .map(|v| v.trim().parse::<f32>().unwrap_or(0.0));
                Vec2::new(it.next().unwrap_or(0.0), it.next().unwrap_or(0.0))
            };
            match verb {
                "move" => self.ui.input.mouse = xy(),
                "click" => {
                    self.ui.input.mouse = xy();
                    self.ui.input.pressed = true;
                    self.ui.input.down = true;
                    self.release_next = true;
                }
                "wheel" => self.ui.input.wheel.y += arg.trim().parse::<f32>().unwrap_or(0.0),
                // a drag: down at a place, `move` with the button still held, `up` at the end
                "down" => {
                    self.ui.input.mouse = xy();
                    self.ui.input.pressed = true;
                    self.ui.input.down = true;
                    self.release_next = false;
                }
                "up" => {
                    self.ui.input.released = true;
                    self.ui.input.down = false;
                }
                "type" => self.ui.input.text.push_str(arg),
                "key" => {
                    let k = match arg.trim() {
                        "Enter" => Some(Key::Enter),
                        "Escape" => Some(Key::Escape),
                        "Backspace" => Some(Key::Backspace),
                        _ => None,
                    };
                    if let Some(k) = k {
                        self.ui.input.keys.push(k);
                    }
                }
                "shot" => self.shot = Some((0.0, std::path::PathBuf::from(arg.trim()))),
                // `focus 0` / `focus 1`: the window loses or gets the keyboard
                "focus" => self.set_focus(arg.trim() != "0"),
                "page" => {
                    if let Some(pg) = Page::named(arg.trim()) {
                        self.go(pg);
                    }
                }
                _ => log::warn!("launcher input: what is '{cmd}'?"),
            }
        }
    }

    /// The window's picture drawn again into a texture and read back (for OMSI_LAUNCHER_SHOT).
    fn shot_image(
        &mut self,
        w: u32,
        h: u32,
        layers: &[omsi_ui::Layer],
        draws: &[Draw],
        bg: wgpu::Color,
    ) -> Option<image::RgbaImage> {
        let r = self.gfx.as_mut()?;
        let gpu = self.gpu.as_mut()?;
        let tex = r.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("launcher shot"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: r.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = tex.create_view(&Default::default());
        let mut enc = r.device.create_command_encoder(&Default::default());
        gpu.render(
            &r.device,
            &r.queue,
            &mut enc,
            &view,
            (w, h),
            Some(bg),
            layers,
            draws,
        );
        let stride = (w * 4).div_ceil(256) * 256;
        let buf = r.device.create_buffer(&wgpu::BufferDescriptor {
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
                    rows_per_image: None,
                },
            },
            wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
        );
        r.queue.submit([enc.finish()]);
        // (a device lost on the way: no picture, instead of wgpu's panic in reading it back)
        let data = r.read_buffer(&buf)?;
        let bgra = matches!(
            r.format(),
            wgpu::TextureFormat::Bgra8UnormSrgb | wgpu::TextureFormat::Bgra8Unorm
        );
        let mut img = image::RgbaImage::new(w, h);
        for y in 0..h {
            for x in 0..w {
                let i = (y * stride + x * 4) as usize;
                let (r_, g, b) = if bgra {
                    (data[i + 2], data[i + 1], data[i])
                } else {
                    (data[i], data[i + 1], data[i + 2])
                };
                img.put_pixel(x, y, image::Rgba([r_, g, b, 255]));
            }
        }
        Some(img)
    }

    fn draw_ui(&mut self) {
        // (the accent the settings hold: chosen on Settings > Interface, or back to amber
        // after "Reset all settings")
        let accent = self
            .state
            .settings
            .get("ui_accent")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        omsi_ui::theme::set_accent(omsi_ui::theme::Accent::named(accent).unwrap_or_default());
        // (and its corners: square ones are flatter, closer to Content Manager)
        omsi_ui::theme::set_square(
            self.state
                .settings
                .get("ui_corners")
                .and_then(|v| v.as_str())
                == Some("square"),
        );
        if self.page != Page::Controls || self.pages.controls_tab != 1 {
            self.pages.pads.cancel_feedback_test();
        }
        let size = self.ui.size;
        let mobile = mobile::mobile();
        // the storage browser lies over the page: the page sees no
        // finger meanwhile
        let disconnected = self.state.disconnected.is_some();
        let crash = !disconnected && self.state.crash.is_some();
        let reset = !crash && !disconnected && self.pages.confirm_reset;
        // (a window of the Maps page's start lies over the page, as a dialog does)
        let window = !mobile && self.page == Page::Maps && self.start.popup.is_some();
        // (and a drive's details over Profile)
        let drive = !mobile && self.page == Page::Profile && self.pages.drive_details;
        // (and the keyboard's Reload and Add a control over Controls)
        let keys = !mobile
            && self.page == Page::Controls
            && self.pages.controls_tab == 0
            && self.pages.kb.dialog.is_some();
        if self.browser.is_some() || crash || reset || disconnected {
            self.pages.pads.cancel_feedback_test();
        }
        let saved =
            (self.browser.is_some() || crash || reset || disconnected || window || drive || keys)
                .then(|| {
                    let i = self.ui.input.clone();
                    self.ui.input.mouse = Vec2::new(-1e4, -1e4);
                    self.ui.input.pressed = false;
                    self.ui.input.released = false;
                    self.ui.input.wheel = Vec2::ZERO;
                    self.ui.input.keys.clear();
                    self.ui.input.text.clear();
                    i
                });
        // a phone: the launcher made for it, not the desktop's pages
        if mobile {
            if self.page == Page::Setup && !self.root_ok() && self.phone.page.is_none() {
                self.phone.tab = phone::Tab::More;
                self.phone.page = Some(Page::Setup);
            }
            phone::draw(self);
        } else {
            // (sessions start from Maps on a computer; the Drive page is the phone's - through
            // `go`, so that Maps shows the map the choice holds, not the library's first)
            if self.page == Page::Drive {
                self.go(Page::Maps);
                self.page_anim = 1.0;
            }
            let rail_w = self.rail_w();
            self.page_anim = if omsi_ui::theme::motion() {
                (self.page_anim + self.ui.dt / omsi_ui::theme::MOTION_NORMAL).min(1.0)
            } else {
                1.0
            };
            // (no wider than a page reads well: on a wide screen the rest is margin, the page
            // in the middle - the panels stretched across 2000 px with their text at one end;
            // under the top bar, the heading a little closer to it than to the window's edge)
            let (margin, top) = if mobile { (36.0, 14.0) } else { (64.0, 28.0) };
            let top = if self.top_nav() {
                top + omsi_launcher_ui::look::TOPBAR_H - 8.0
            } else {
                top
            };
            let avail = size.x - rail_w - margin;
            let w = avail.min(1760.0);
            let seen = size.y - top - 40.0;
            // (a phone: laid out for a taller screen, scrolled)
            let h = if mobile {
                seen.max(mobile::PAGE_H)
            } else {
                seen
            };
            self.page_max = (h - seen).max(0.0);
            self.page_scroll = self.page_scroll.clamp(0.0, self.page_max);
            let content = Rect::new(
                rail_w + margin * 0.5 + (avail - w) * 0.5,
                top - self.page_scroll,
                w,
                h,
            );
            // (a page comes in as Windows 11's do: rising 12 px into place; its panels' glass
            // stays as it is, nothing fades)
            let e = omsi_ui::theme::ease_out(self.page_anim);
            let content = Rect::new(
                content.x,
                content.y + 12.0 * (1.0 - e),
                content.w,
                content.h,
            );
            let glass = self.draw_backdrop();
            omsi_ui::theme::set_glass(glass);
            match self.page {
                Page::Drive => drive::draw(self, content),
                Page::Vehicles => self.vehicles_page(content),
                Page::Maps => self.maps_page(content),
                Page::Diagnostics => diagnostics::draw(self, content),
                #[cfg(feature = "lan")]
                Page::Multiplayer => multiplayer::draw(self, content),
                #[cfg(not(feature = "lan"))]
                Page::Multiplayer => {}
                Page::Profile => pages::profile(self, content),
                Page::Settings => settings::settings(self, content),
                Page::Controls => pages::controls(self, content),
                Page::Mods => pages::mods(self, content),
                Page::Tutorials => pages::tutorials(self, content),
                Page::Timetable => timetable::draw(self, content),
                Page::Setup => setup::setup(self, content),
            }
            // the rail over the page (a scrolled page passes under it)
            self.rail();
            self.status_bar();
            // (dialogs and what lies over the pages stay solid)
            omsi_ui::theme::set_glass(false);
        }
        if let Some(i) = saved {
            self.ui.input = i;
            // (a window over the page fades in with the page's darkening, as Windows 11's
            // dialogs do; each one again when it opens)
            let which = if disconnected {
                "disconnect"
            } else if crash {
                "crash"
            } else if reset {
                "reset"
            } else if self.browser.is_none() && window {
                match self.start.popup {
                    Some(start::Popup::StartPoint) => "start-point",
                    Some(start::Popup::Weather) => "weather",
                    _ => "explore",
                }
            } else if self.browser.is_none() && drive {
                "drive"
            } else if self.browser.is_none() && keys {
                "keys"
            } else {
                "browser"
            };
            let a = self.ui.appear(
                omsi_ui::widgets::id_of(&format!("dialog:{which}")),
                omsi_ui::theme::MOTION_NORMAL,
            );
            self.ui.push_fade(a);
            if disconnected {
                self.draw_disconnect_dialog();
            } else if crash {
                self.draw_crash_dialog();
            } else if reset {
                settings::reset_dialog(self);
            } else if self.browser.is_none() && window {
                start::popup(self);
            } else if self.browser.is_none() && drive {
                pages::drive_dialog(self);
            } else if self.browser.is_none() && keys {
                keyboard::dialog(self);
            } else {
                self.draw_browser();
            }
            self.ui.pop_fade();
        }
        // a game starts: the last picture before the launcher gives its graphics device up
        // (see `frame`), which stays in the window until the game ends
        if !mobile && self.state.in_game() && !self.awake() {
            self.draw_game_banner();
        }
        // (over everything, dialogs too: the window can always be moved, sized and closed)
        if chrome::own_frame() {
            let w = self.ui.size.x;
            self.chrome.clicked =
                omsi_launcher_ui::shell::caption_buttons(&mut self.ui, w, self.chrome.maximized);
        }
    }

    /// Over the launcher's last picture while a game runs: why the launcher does not move.
    fn draw_game_banner(&mut self) {
        let size = self.ui.size;
        let full = Rect::new(0.0, 0.0, size.x, size.y);
        self.ui.solid(full);
        self.ui.p().rect(full, omsi_ui::Color::rgba(0, 0, 0, 0.62));
        let text = "The launcher rests while you drive, so that the game has the graphics card to itself. It is back as soon as the game ends.";
        let w = (size.x - 48.0).min(520.0);
        let th = self
            .ui
            .paragraph_height(text, w - 48.0, 13.0, Weight::Regular);
        let h = 80.0 + th;
        let r = Rect::new((size.x - w) * 0.5, (size.y - h) * 0.5, w, h);
        self.ui.panel(r);
        let inner = Rect::new(r.x + 24.0, r.y + 20.0, r.w - 48.0, r.h - 40.0);
        self.ui.icon(
            "directions_bus",
            Vec2::new(inner.x + 14.0, inner.y + 14.0),
            26.0,
            accent(),
        );
        self.ui.text_in(
            "The game is running",
            Rect::new(inner.x + 38.0, inner.y, inner.w - 38.0, 28.0),
            18.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        self.ui.paragraph(
            text,
            Vec2::new(inner.x, inner.y + 40.0),
            inner.w,
            13.0,
            Weight::Regular,
            TEXT_DIM,
        );
    }

    /// The bus preview in `r`: the game's picture of it, or a word while it loads. The mouse
    /// dragged on it turns the bus, the wheel zooms.
    pub fn preview(&mut self, r: Rect) {
        self.preview_rect = Some(r);
        self.ui.solid(r);
        self.ui.p().rounded(r, RADIUS, FIELD);
        match (self.preview_tex, self.preview.has_picture()) {
            (Some(tex), true) => self.ui.image_cover(r, tex, self.preview.aspect, 1.0),
            _ => {
                let t = if self.preview.error.is_some() {
                    "No preview"
                } else {
                    "Loading…"
                };
                self.ui
                    .text_in(t, r, 13.0, Weight::Regular, TEXT_FAINT, Align::Center);
            }
        }
        if self.preview.busy && self.preview.has_picture() {
            let c = Vec2::new(r.right() - 16.0, r.y + 16.0);
            let a = self.ui.time * 5.0;
            self.ui.p().arc(c, 6.0, 8.0, a, a + 4.2, TEXT_SOFT);
        }
        self.preview_badge(r);
        if self.ui.hover(r) && self.ui.input.wheel.y.abs() > 0.0 {
            self.preview
                .zoom_by((1.0 - self.ui.input.wheel.y * 0.08).clamp(0.8, 1.25));
        }
        if self.ui.hover(r) {
            self.ui.cursor = winit::window::CursorIcon::Grab;
        }
    }

    /// The chosen map across `r`, the whole page behind the panels: every road the map has,
    /// its entry points and the chosen trip's route, read from the tile files (see
    /// `mapview`), or a word while it is read. `map_interact` gives it the mouse afterwards.
    pub fn map_background(&mut self, r: Rect) {
        self.map_rect = Some(r);
        let status = self.mapview.status();
        match (self.map_tex, status.is_empty()) {
            (Some(tex), true) => {
                self.ui.image(r, tex, RADIUS);
                // (the data's licence asks for it where its surface is shown)
                if let Some(credit) = self.mapview.credit().map(str::to_string) {
                    self.ui.text_in(
                        &credit,
                        Rect::new(r.x + 12.0, r.bottom() - 24.0, (r.w - 24.0).max(0.0), 18.0),
                        10.5,
                        Weight::Medium,
                        TEXT_FAINT,
                        Align::Left,
                    );
                }
            }
            _ => {
                self.ui.solid(r);
                self.ui
                    .p()
                    .rounded(r, RADIUS, omsi_ui::Color::rgba(13, 13, 13, 1.0));
                let t = if status.is_empty() {
                    "Loading…"
                } else {
                    status
                };
                self.ui.text_in(
                    t,
                    Rect::new(r.x, r.y + r.h * 0.5 - 12.0, r.w, 24.0),
                    13.5,
                    Weight::Regular,
                    TEXT_FAINT,
                    Align::Center,
                );
            }
        }
        if self.mapview.busy() {
            // (out of the way of the panels: the map is being read, the page is not)
            let c = Vec2::new(r.right() - 26.0, r.y + r.h - 26.0);
            let a = self.ui.time * 5.0;
            self.ui.p().arc(c, 6.0, 8.0, a, a + 4.2, TEXT_SOFT);
        }
    }

    /// The mouse over the map: what it drags, where it zooms, and the entry point a click
    /// takes. Called once the page's panels are drawn - they have the first claim on it.
    pub fn map_interact(&mut self, r: Rect, window: Rect) {
        self.map_interact_with(r, window, true);
    }

    /// `map_interact` for a map inside a window of its own: the window's darkened frame claims
    /// the whole screen (`over_ui`), so only the map's own rectangle decides what the mouse
    /// does to it.
    pub fn map_interact_in_window(&mut self, r: Rect, window: Rect) {
        self.map_interact_with(r, window, false);
    }

    fn map_interact_with(&mut self, r: Rect, window: Rect, panels_first: bool) {
        let p = mapview::Pointer {
            at: self.ui.input.mouse,
            pressed: self.ui.input.pressed,
            released: self.ui.input.released,
            down: self.ui.input.down,
            wheel: self.ui.input.wheel.y,
            blocked: (panels_first && self.ui.over_ui) || !r.contains(self.ui.input.mouse),
        };
        self.mapview.think(r, window, self.ui.scale, p);
        if let Some(i) = self.mapview.take_clicked() {
            if self.state.choice.entry != i as i32 {
                log::info!(
                    "launcher map: entry point {} of the map's list taken from the map",
                    i + 1
                );
                self.state.choice.entry = i as i32;
                self.state.touched();
            }
        }
    }

    /// The bus across `r`, the whole page behind the panels. `focus` is where the panels end,
    /// as a share of the window: the showroom frames the bus in what is left of it.
    pub fn preview_full(&mut self, r: Rect, focus: f32) {
        self.preview_rect = Some(r);
        self.preview.focus_x = focus.clamp(0.2, 0.95);
        match (self.preview_tex, self.preview.has_picture()) {
            (Some(tex), true) => self.ui.image_cover(r, tex, self.preview.aspect, 1.0),
            _ => {
                self.ui.solid(r);
                self.ui.p().rounded(r, RADIUS, FIELD);
                let t = if self.preview.error.is_some() {
                    "No preview"
                } else {
                    "Loading…"
                };
                self.ui.text_in(
                    t,
                    Rect::new(r.x, r.y + r.h * 0.5 - 12.0, r.w, 24.0),
                    13.0,
                    Weight::Regular,
                    TEXT_FAINT,
                    Align::Center,
                );
            }
        }
        // (left of the panels, which lie over the right part of the page)
        let w = (r.w * self.preview.focus_x * 2.0 - r.w).clamp(r.w * 0.3, r.w);
        self.preview_badge(Rect::new(r.x, r.y, w, r.h));
    }

    /// Where the preview's picture comes from, in the lower left of `r`: kept from an earlier
    /// time (the helper draws the live one only while the page shows it) or drawn live.
    fn preview_badge(&mut self, r: Rect) {
        let t = if self.preview.live {
            "Preview helper: running"
        } else if self.preview.has_picture() && self.preview.picture_only() {
            // (a page that shows the kept picture: the live view when the player turns it)
            "Image from cache · drag it to see it live"
        } else if self.preview.has_picture() && self.preview.from_cache {
            "Image from cache · live view only on this page"
        } else {
            return;
        };
        let w = self.ui.width(t, 11.5, Weight::Regular) + 20.0;
        let b = Rect::new(r.x + 14.0, r.y + r.h - 38.0, w, 24.0);
        self.ui
            .p()
            .rounded(b, 12.0, omsi_ui::Color::rgba(10, 10, 12, 0.72));
        self.ui
            .text_in(t, b, 11.5, Weight::Regular, TEXT_DIM, Align::Center);
    }

    /// The wheel and the cursor over the showroom, once the panels have had the mouse.
    pub fn showroom_pointer(&mut self, r: Rect) {
        if self.ui.over_ui || !r.contains(self.ui.input.mouse) {
            return;
        }
        if self.ui.input.wheel.y.abs() > 0.0 {
            self.preview
                .zoom_by((1.0 - self.ui.input.wheel.y * 0.08).clamp(0.8, 1.25));
        }
        self.ui.cursor = winit::window::CursorIcon::Grab;
    }

    pub fn go(&mut self, p: Page) {
        if self.page != p {
            self.page = p;
            self.page_anim = 0.0;
            self.page_scroll = 0.0;
            self.phone.page = match p {
                Page::Drive => {
                    self.phone.tab = phone::Tab::Play;
                    None
                }
                Page::Multiplayer => {
                    self.phone.tab = phone::Tab::Online;
                    None
                }
                Page::Mods => {
                    self.phone.tab = phone::Tab::Mods;
                    None
                }
                other => {
                    self.phone.tab = phone::Tab::More;
                    Some(other)
                }
            };
            match p {
                Page::Profile => {
                    self.state.load_profile();
                    self.state.poll_now();
                }
                Page::Mods => self.state.load_mods(),
                // (counted again each time the page opens: a mod installed meanwhile counts)
                Page::Setup => {
                    self.state.usage_asked = false;
                    self.state.squeeze.asked = false;
                }
                Page::Vehicles => {
                    self.vehicles.retry_if_failed();
                    self.refresh_catalogs();
                    self.vehicles
                        .select(&self.state.choice.bus, &self.state.choice.paint);
                }
                Page::Maps => {
                    self.maps.retry_if_failed();
                    self.refresh_catalogs();
                    self.maps.select(&self.state.choice.map);
                }
                _ => {}
            }
        }
    }

    /// The Scene background is chosen (Settings > Interface > Background).
    fn scene(&self) -> bool {
        !mobile::mobile()
            && self
                .state
                .settings
                .get("ui_background")
                .and_then(|v| v.as_str())
                == Some("scene")
    }

    /// With the Scene background, the page's picture behind it, blurred and darkened: true
    /// when there is one (the panels are drawn as glass then), false for the plain dark.
    fn draw_backdrop(&mut self) -> bool {
        if !self.scene() {
            return false;
        }
        let root = std::path::PathBuf::from(&self.state.config.root);
        let map = |file: &str| backdrop::Want::Map {
            root: root.clone(),
            file: file.to_string(),
        };
        self.backdrop.game_running(self.state.in_game());
        let want = match self.page {
            Page::Maps => self.maps.current().map(map),
            Page::Vehicles if self.preview.has_picture() => {
                self.preview.showing().map(backdrop::Want::Bus)
            }
            // (while the next bus is drawn, the picture before stays: not a screenshot between)
            Page::Vehicles if self.preview.loading() => self.backdrop.wanted().cloned(),
            Page::Vehicles => None,
            Page::Drive => Some(&self.state.choice.map)
                .filter(|m| !m.is_empty())
                .map(|m| map(m)),
            _ => None,
        };
        self.backdrop.want(want.unwrap_or(backdrop::Want::Latest));
        let Some((tex, aspect)) = self.backdrop.shown() else {
            return false;
        };
        self.backdrop.fade = (self.backdrop.fade + self.ui.dt / 0.35).min(1.0);
        let full = Rect::new(0.0, 0.0, self.ui.size.x, self.ui.size.y);
        self.ui.image_cover(full, tex, aspect, self.backdrop.fade);
        // (the dimming as the eye sees it: the veil is laid in linear light, where the same
        // cover darkens far less)
        let dim = core::dimming_of(
            self.state
                .settings
                .get("ui_dimming")
                .unwrap_or(&serde_json::Value::Null),
        ) as f32
            / 100.0;
        let cover = 1.0 - (1.0 - dim).powf(2.2);
        self.ui.p().rect(
            full,
            omsi_ui::Color::rgba(0, 0, 0, cover * self.backdrop.fade),
        );
        true
    }

    /// The pages are listed in a bar along the top (Settings > Interface > Navigation), not in
    /// the sidebar.
    pub(crate) fn top_nav(&self) -> bool {
        !mobile::mobile()
            && self.state.settings.get("ui_nav").and_then(|v| v.as_str()) == Some("top")
    }

    /// The width the sidebar takes from the pages (none under a top bar).
    pub(crate) fn rail_w(&self) -> f32 {
        if self.top_nav() {
            0.0
        } else {
            RAIL_W
        }
    }

    /// The sidebar over the page's left side (a scrolled page passes under it), or the top
    /// bar over its top: the pages, and the driver at the sidebar's foot.
    fn rail(&mut self) {
        let size = self.ui.size;
        let jobs = self
            .state
            .jobs
            .iter()
            .filter(|j| j.finished.is_none())
            .count();
        let mut badges = Vec::new();
        if jobs > 0 {
            badges.push((UiPage::Mods, jobs.to_string(), accent()));
        }
        let (level, name) = match &self.state.profile {
            Some(p) => (p.level, p.name.clone()),
            None => (1, self.state.config.profile.clone()),
        };
        // (no driver yet: no level to show, and the foot asks for one - it opens Profile)
        let ask = name.is_empty();
        let name = if ask { "No driver".to_string() } else { name };
        let level = if ask {
            "Create".to_string()
        } else {
            omsi_ui::tr("Level %{level}").replace("%{level}", &level.to_string())
        };
        let page = self.page.shown().unwrap_or(UiPage::Drive);
        let nav = if self.top_nav() {
            omsi_launcher_ui::shell::TopBar {
                page,
                logo: self.logo,
                badges: &badges,
            }
            .draw(&mut self.ui, size.x)
        } else {
            omsi_launcher_ui::shell::Sidebar {
                page,
                logo: self.logo,
                badges: &badges,
                footer: (&name, &level),
                footer_ask: ask,
            }
            .draw(&mut self.ui, size.y)
        };
        match nav {
            Some(omsi_launcher_ui::shell::Nav::Page(p)) => self.go(Page::of(p)),
            Some(omsi_launcher_ui::shell::Nav::Footer) => self.go(Page::Profile),
            None => {}
        }
    }

    /// Give the Vehicles and Maps pages the host's catalogs when those differ from what the
    /// pages show (the host reads them again after Setup or an install, and fills them as its
    /// scan goes on).
    fn refresh_catalogs(&mut self) {
        // (as many as before, but other ones: another content folder chosen under Setup)
        let other = self.catalog_gen != self.state.content_gen;
        self.catalog_gen = self.state.content_gen;
        let host = self.state.vehicles.len();
        if host > 0 && (other || self.vehicles.catalog_len() != Some(host)) {
            self.vehicles.set_catalog(self.state.vehicles.clone());
        }
        let host = self.state.maps.len();
        if host > 0 && (other || self.maps.catalog_len() != Some(host)) {
            self.maps.set_catalog(self.state.maps.clone());
        }
    }

    /// The Vehicles page; "Drive this bus" takes the bus and its paint to the Drive page.
    fn vehicles_page(&mut self, area: Rect) {
        self.refresh_catalogs();
        omsi_launcher_ui::shell::page_heading(
            &mut self.ui,
            UiPage::Vehicles,
            area.x,
            area.y - 28.0,
        );
        let body = Rect::new(area.x, area.y - 28.0, area.w, area.h + 28.0);
        // the bus picked in the library, in the Drive page's light, in the preview (kept picture
        // at once, live from the helper while the page is open)
        let picture = omsi_launcher_ui::vehicles::Picture {
            texture: self
                .preview_tex
                .filter(|_| self.preview.has_picture())
                .map(|t| (t, self.preview.aspect)),
            from_cache: self.preview.from_cache,
            live: self.preview.live,
            busy: self.preview.busy,
            error: self.preview.error.clone(),
        };
        let has_bus = self.vehicles.current().is_some() && !self.state.choice.map.is_empty();
        self.vehicles.set_picture(has_bus.then_some(picture));
        let none = !self.root_ok();
        self.vehicles.set_no_folder(none);
        self.vehicles.draw(&mut self.ui, body);
        if self.vehicles.take_open_setup() {
            self.go(Page::Setup);
        }
        // (the bus the next start uses: its card shows what is chosen for it)
        if let Some(file) = self
            .vehicles
            .fresh_bus()
            .filter(|f| f.eq_ignore_ascii_case(&self.state.choice.bus))
        {
            let c = &self.state.choice;
            self.vehicles.prepare(
                &file,
                c.hof_manual.then_some(c.hof.as_str()),
                &c.number,
                &c.plate,
            );
        }
        if let (Some((bus, paint)), false) =
            (self.vehicles.current().cloned(), self.state.in_game())
        {
            let c = &self.state.choice;
            if !c.map.is_empty() {
                self.preview.want(showroom::Look {
                    root: std::path::PathBuf::from(&self.state.config.root),
                    map: c.map.clone(),
                    bus,
                    paint,
                    weather: c.weather.clone(),
                    time: c.time,
                    date: c.date.clone(),
                });
                self.preview.focus_x = 0.5;
                self.preview_rect = self.vehicles.preview_area();
                // (the picture lies in the page's card, not under it: the card does not take the
                // mouse from it)
                if self
                    .preview_rect
                    .is_some_and(|r| r.contains(self.ui.input.mouse))
                {
                    if self.ui.input.wheel.y.abs() > 0.0 {
                        self.preview
                            .zoom_by((1.0 - self.ui.input.wheel.y * 0.08).clamp(0.8, 1.25));
                    }
                    self.ui.cursor = winit::window::CursorIcon::Grab;
                }
            }
        }
        self.paint_thumbs();
        self.vehicles.set_prerender(self.prerender.status());
        match self.vehicles.take_prerender() {
            Some(omsi_launcher_ui::vehicles::PrerenderRequest::Start) => {
                // (the content is read again first: new vehicles are found, the others come
                // from the index)
                self.prerender.looking();
                self.state.load_content();
            }
            Some(omsi_launcher_ui::vehicles::PrerenderRequest::Cancel) => self.prerender.cancel(),
            None => {}
        }
        if let Some(u) = self.vehicles.take_use() {
            self.state.use_bus(u);
            self.go(Page::Maps);
        }
    }

    /// The small pictures of the paint tiles in view: each is the picture drawn ahead of time
    /// for that paint (`prerender`), made smaller on the thumbnails' thread. A paint with no
    /// picture yet shows its initial, and its picture comes in once it has been drawn.
    fn paint_thumbs(&mut self) {
        let shape = prerender::shape();
        let root = std::path::PathBuf::from(&self.state.config.root);
        let looks: Vec<showroom::Look> = self
            .vehicles
            .visible_paints()
            .iter()
            .map(|(bus, paint)| showroom::Look {
                root: root.clone(),
                bus: bus.clone(),
                paint: paint.clone(),
                ..Default::default()
            })
            .collect();
        // (a picture just drawn: the small ones of the paints in view, empty so far, are made
        // again)
        let progress = self.prerender.progress();
        let again = progress != self.prerender_seen;
        self.prerender_seen = progress;
        for look in looks {
            let key = preview::kept_key(&look, shape);
            if again {
                self.thumbs.retry(&key);
            }
            let (bus, paint) = (look.bus.clone(), look.paint.clone());
            self.thumbs
                .want(&key, move || preview::kept_picture(&look, shape));
            self.vehicles
                .set_paint_thumb(&bus, &paint, self.thumbs.get(&key));
        }
    }

    /// Drawing the vehicles' pictures ahead of time: starts a run once the content is read
    /// (after another OMSI 2 folder was saved, or the page's button), follows what the page's
    /// buttons ask for, and gives the page how it stands.
    fn prerender_step(&mut self, dt: f32) {
        if mobile::mobile() {
            // (a phone's helper is a thread of the launcher, not a process of its own)
            self.prerender.cancel();
            return;
        }
        if self.prerender.content_ready(
            self.state.loading_content,
            self.state.vehicles.is_empty(),
        ) {
            let c = &self.state.choice;
            // (the light the pictures are drawn in: the chosen map, else the first there is)
            let map = if c.map.is_empty() {
                self.state.maps.first().map(|m| m.file.clone())
            } else {
                Some(c.map.clone())
            };
            match map {
                Some(map) => {
                    let scene = showroom::Look {
                        root: std::path::PathBuf::from(&self.state.config.root),
                        map,
                        weather: c.weather.clone(),
                        time: c.time,
                        date: c.date.clone(),
                        ..Default::default()
                    };
                    let vehicles = self.state.vehicles.clone();
                    self.prerender.start(&vehicles, scene);
                }
                None => {
                    log::warn!("launcher: no map to draw the vehicles' pictures on");
                    self.prerender.cancel();
                }
            }
        }
        self.prerender.update(dt, self.state.in_game());
    }

    /// The Maps page: the library, the chosen map's picture and description, and the start
    /// (`start`), which holds the session's choice - the map chosen in the library is the one
    /// a session starts on.
    fn maps_page(&mut self, area: Rect) {
        self.refresh_catalogs();
        omsi_launcher_ui::shell::page_heading(&mut self.ui, UiPage::Maps, area.x, area.y - 28.0);
        let body = Rect::new(area.x, area.y - 28.0, area.w, area.h + 28.0);
        let none = !self.root_ok();
        self.maps.set_no_folder(none);
        self.maps.draw(&mut self.ui, body);
        if self.maps.take_open_setup() {
            self.go(Page::Setup);
        }
        // the chosen map's picture, read from its folder on a thread
        if let Some(file) = self.maps.current().map(str::to_string) {
            self.map_pic
                .want(std::path::Path::new(&self.state.config.root), &file);
            if !file.eq_ignore_ascii_case(&self.state.choice.map) {
                self.state.select_map(&file);
            }
        }
        self.maps.set_picture(
            self.map_pic.shown(),
            self.map_pic.reading.is_some() || self.map_pic.pending.is_some(),
        );
        // the rows' small pictures, the same file read on a thread of their own
        let root = std::path::PathBuf::from(&self.state.config.root);
        for file in self.maps.visible().to_vec() {
            let dir = MapPicture::folder(&root, &file);
            self.thumbs.want(&file, move || {
                dir.and_then(|d| MapPicture::find(&d))
                    .and_then(|f| MapPicture::read(&f))
            });
            self.maps.set_thumb(&file, self.thumbs.get(&file));
        }
        if let Some(r) = self.maps.start_area() {
            start::card(self, r);
        }
        if self.maps.take_explore() {
            self.start.popup = Some(start::Popup::Explore);
        }
    }

    /// Whether the OMSI 2 folder chosen has what the game needs, kept per folder and looked at
    /// again when the window has the focus back (a folder repaired outside the launcher).
    fn root_ok(&mut self) -> bool {
        let root = &self.state.config.root;
        match &self.root_ok {
            Some((r, ok)) if r == root => *ok,
            _ => {
                let ok =
                    omsi_cfg::missing_original_essentials(std::path::Path::new(root)).is_empty();
                self.root_ok = Some((root.clone(), ok));
                ok
            }
        }
    }

    /// The foot of the page (on a computer): what runs and what was read, as chips, as the new
    /// look has them; returns where the message line can start.
    fn status_chips(&mut self, x: f32, cy: f32) -> f32 {
        let games = self.state.instances.iter().filter(|i| i.running).count();
        let root_ok = self.root_ok();
        // (each chip's dot says how it stands at a glance: green when all is well, amber when
        // something is missing; none while there is nothing to say - a game not running)
        let chips = [
            (
                if games == 0 {
                    "Game: not running".to_string()
                } else {
                    "Game: running".to_string()
                },
                (games > 0).then_some(OK),
            ),
            (
                (if root_ok {
                    "OMSI 2 folder: read"
                } else {
                    "OMSI 2 folder: not chosen"
                })
                .to_string(),
                Some(if root_ok { OK } else { WARN }),
            ),
            (
                (if self.preview.live {
                    "Preview helper: running, stops when the page closes"
                } else {
                    "Preview helper: stopped"
                })
                .to_string(),
                self.preview.live.then_some(OK),
            ),
        ];
        let mut at = x;
        for (t, dot_c) in chips {
            // (a chip with a dot leads with it, the words after it)
            let dot = if dot_c.is_some() { 12.0 } else { 0.0 };
            let w = self.ui.width(&t, 12.0, Weight::Regular) + 22.0 + dot;
            let r = Rect::new(at, cy - 12.0, w, 24.0);
            self.ui.p().rounded(r, 12.0, PANEL);
            self.ui.p().rounded_border(r, 12.0, 1.0, EDGE);
            if let Some(c) = dot_c {
                self.ui.p().circle(Vec2::new(r.x + 14.0, cy), 3.0, c);
            }
            let tx = Rect::new(r.x + dot, r.y, w - dot, 24.0);
            self.ui
                .text_in(&t, tx, 12.0, Weight::Regular, TEXT_DIM, Align::Center);
            at += w + 10.0;
        }
        at + 6.0
    }

    fn status_bar(&mut self) {
        let size = self.ui.size;
        let rail_w = if mobile::mobile() {
            mobile::RAIL_W_MOBILE
        } else {
            self.rail_w()
        };
        let left = if mobile::mobile() {
            rail_w + 20.0
        } else {
            self.status_chips(rail_w + 28.0, size.y - 22.0)
        };
        let (text, err, at) = self.state.status.clone();
        if text.is_empty() {
            return;
        }
        let fade = if err {
            1.0
        } else {
            (1.0 - (at.elapsed().as_secs_f32() - 6.0) / 1.5).clamp(0.0, 1.0)
        };
        if fade <= 0.0 {
            return;
        }
        let first = text.lines().next().unwrap_or("").to_string();
        let r = Rect::new(left, size.y - 34.0, (size.x - left - 20.0).max(0.0), 24.0);
        if mobile::mobile() {
            // (readable over a page scrolled under it)
            self.ui.p().rect(
                Rect::new(rail_w, size.y - 34.0, size.x - rail_w, 34.0),
                RAIL.alpha(0.92),
            );
        }
        let c = if err { DANGER } else { TEXT_DIM };
        self.ui
            .text_in(&first, r, 12.0, Weight::Regular, c.alpha(fade), Align::Left);
        self.ui.tooltip(r, &text);
    }

    /// A page's heading in the new look, as Vehicles and Maps draw theirs (see
    /// `omsi_launcher_ui::shell::heading`): "PAGE › PLACE" small and letter-spaced, the title
    /// large and a line on what the page is for (`UiPage::heading`). Returns the page's body
    /// under it.
    pub fn page_title(&mut self, r: Rect, page: UiPage) -> Rect {
        let h = page.heading().unwrap_or((page.title(), page.title(), ""));
        self.page_title_with(r, page.title(), h)
    }

    /// `page_title` with its parts given: the page's name, (where on it, the title, the line).
    /// A phone keeps its compact heading: the name and the line.
    pub fn page_title_with(&mut self, r: Rect, name: &str, parts: (&str, &str, &str)) -> Rect {
        self.page_title_beside(r, name, parts, 0.0)
    }

    /// `page_title_with` for a page whose tabs, `tabs_w` wide, sit at the right of its title
    /// (`title_tabs`): the title is cut short of them.
    pub fn page_title_beside(
        &mut self,
        r: Rect,
        name: &str,
        (place, title, line): (&str, &str, &str),
        tabs_w: f32,
    ) -> Rect {
        if mobile::mobile() {
            self.ui.text(
                name,
                Vec2::new(r.x, r.y + 22.0),
                22.0,
                Weight::Bold,
                TEXT,
                Align::Left,
            );
            if !line.is_empty() {
                // (a narrow window: the line stops short of the tabs some pages put top right,
                // it ran under them on a phone)
                let w = if r.w < 1100.0 { r.w - 340.0 } else { r.w };
                self.ui.text_in(
                    line,
                    Rect::new(r.x, r.y + 34.0, w, 20.0),
                    12.5,
                    Weight::Regular,
                    TEXT_DIM,
                    Align::Left,
                );
            }
            return Rect::new(r.x, r.y + 64.0, r.w, (r.h - 64.0).max(0.0));
        }
        // (the heading starts at the window's top, as Vehicles' and Maps': the page's area
        // leaves a margin above it)
        let top = r.y - 28.0;
        let title_w = if tabs_w > 0.0 {
            (r.w - tabs_w - 24.0).max(0.0)
        } else {
            r.w
        };
        omsi_launcher_ui::shell::heading_beside(
            &mut self.ui,
            name,
            (place, title, line),
            r.x,
            top,
            r.w,
            title_w,
        );
        let y = top + omsi_launcher_ui::shell::BODY_TOP;
        Rect::new(r.x, y, r.w, (r.bottom() - y).max(0.0))
    }

    /// Where a page puts its tabs, `w` wide, over its `body` (as `page_title` returned it): at
    /// the right of the title on a computer, of the line on a phone.
    pub fn title_tabs(&self, body: Rect, w: f32) -> Rect {
        let y = if mobile::mobile() {
            body.y - 46.0
        } else {
            body.y - omsi_launcher_ui::shell::BODY_TOP + 56.0
        };
        Rect::new(body.right() - w, y, w, 34.0)
    }

    /// A card's title (`omsi_launcher_ui::kit::heading`, letter-spaced) at the top of `r`, the
    /// panel less its padding; returns the rest of `r` under it. A phone keeps the compact one.
    pub fn card_title(ui: &mut Ui, r: Rect, title: &str) -> Rect {
        if mobile::mobile() {
            ui.heading(r, title, None)
        } else {
            omsi_launcher_ui::kit::heading(ui, r, title)
        }
    }

    /// The title of a part further down a card, letter-spaced and quiet (a phone: as a card's).
    pub fn part_title(ui: &mut Ui, r: Rect, title: &str) {
        if mobile::mobile() {
            ui.heading(r, title, None);
        } else {
            omsi_launcher_ui::kit::subheading(ui, r, title);
        }
    }
}

/// The preview helper's renderer (see `preview`): a bus on a floor needs none of the game's costly passes - no
/// ambient occlusion, a small shadow map, 4x MSAA for the edges whatever the game uses.
fn showroom_options(settings: &crate::settings::Settings) -> omsi_render::RenderOptions {
    omsi_render::RenderOptions {
        msaa: 4,
        ssao: false,
        shadow_size: 1024,
        render_scale: 1.0,
        ..settings.render_options()
    }
}

/// Whether Windows animates its controls (Settings > Accessibility > Visual effects >
/// Animation effects): off, the launcher shows every change at once too. Elsewhere, on.
fn windows_motion() -> bool {
    #[cfg(windows)]
    {
        use windows::Win32::UI::WindowsAndMessaging::{
            SystemParametersInfoW, SPI_GETCLIENTAREAANIMATION, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS,
        };
        let mut on = windows::core::BOOL(1);
        let read = unsafe {
            SystemParametersInfoW(
                SPI_GETCLIENTAREAANIMATION,
                0,
                Some(&mut on as *mut _ as *mut std::ffi::c_void),
                SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
            )
        };
        read.is_err() || on.as_bool()
    }
    #[cfg(not(windows))]
    {
        true
    }
}

#[cfg(test)]
mod page_tests {
    use super::{Page, UiPage};

    /// Every page of the sidebar is a page of the launcher and back, and the names the input
    /// script and `OMSI_LAUNCHER_PAGE` use find them.
    #[test]
    fn the_sidebar_pages_are_the_launchers() {
        for p in UiPage::ALL {
            let page = Page::of(p);
            assert_eq!(page.shown(), Some(p));
            assert_eq!(Page::named(&p.title().to_lowercase()), Some(page));
        }
        assert_eq!(Page::Multiplayer.shown(), None);
        assert_eq!(Page::named("nope"), None);
    }
}

/// The picture in a map's folder (`picture.jpg` and the like, as a map ships it), for the
/// Maps page: read and made smaller on a thread, then put on the GPU.
#[derive(Default)]
pub(crate) struct MapPicture {
    /// The map file it is of.
    file: String,
    reading: Option<std::sync::mpsc::Receiver<Option<image::RgbaImage>>>,
    pending: Option<image::RgbaImage>,
    texture: Option<usize>,
    /// Width over height, while the texture shows this map's picture.
    aspect: Option<f32>,
}

impl MapPicture {
    fn want(&mut self, root: &std::path::Path, file: &str) {
        if self.file != file {
            self.file = file.to_string();
            self.aspect = None;
            self.pending = None;
            // (found through the content roots as the map list found it: a mod's map, or one
            // read in place from an archive, has its own picture)
            let dir = Self::folder(root, file);
            let (tx, rx) = std::sync::mpsc::channel();
            std::thread::spawn(move || {
                let img = dir
                    .and_then(|d| Self::find(&d))
                    .and_then(|f| Self::read(&f));
                let _ = tx.send(img);
            });
            self.reading = Some(rx);
        }
        if let Some(rx) = self.reading.as_ref() {
            match rx.try_recv() {
                Ok(img) => {
                    self.pending = img;
                    self.reading = None;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
                Err(std::sync::mpsc::TryRecvError::Disconnected) => self.reading = None,
            }
        }
    }

    /// The folder of the map `file`, found through the content roots as the map list found it.
    pub(crate) fn folder(root: &std::path::Path, file: &str) -> Option<std::path::PathBuf> {
        let global = omsi_cfg::find_in_roots(file)
            .map(|(_, p)| p)
            .unwrap_or_else(|| omsi_cfg::resolve_path(root, file));
        global.parent().map(|d| d.to_path_buf())
    }

    /// The picture in `dir`: a file named "picture" (any case) the image reader knows.
    pub(crate) fn find(dir: &std::path::Path) -> Option<std::path::PathBuf> {
        let mut files: Vec<_> = omsi_cfg::vfs::list_dir(dir)?
            .into_iter()
            .filter(|(_, is_dir)| !is_dir)
            .map(|(name, _)| dir.join(name))
            .filter(|p| {
                p.file_stem()
                    .is_some_and(|s| s.eq_ignore_ascii_case("picture"))
                    && p.extension().and_then(|x| x.to_str()).is_some_and(|x| {
                        ["jpg", "jpeg", "png", "bmp"].contains(&x.to_ascii_lowercase().as_str())
                    })
            })
            .collect();
        files.sort();
        files.into_iter().next()
    }

    /// The picture decoded within bounds (a map's file is anyone's: no more than 8192 pixels a
    /// side and 256 MB decoded, and a file on disk over 64 MB is not read at all), then made to
    /// fit 1280 x 1280 (the page shows it a few hundred points wide; a texture of that size goes
    /// on any card).
    pub(crate) fn read(file: &std::path::Path) -> Option<image::RgbaImage> {
        if std::fs::metadata(file).is_ok_and(|m| m.len() > 64 << 20) {
            return None;
        }
        let bytes = omsi_cfg::vfs::read(file).ok()?;
        let mut reader = image::ImageReader::new(std::io::Cursor::new(bytes))
            .with_guessed_format()
            .ok()?;
        let mut limits = image::Limits::default();
        limits.max_image_width = Some(8192);
        limits.max_image_height = Some(8192);
        limits.max_alloc = Some(256 << 20);
        reader.limits(limits);
        let img = reader.decode().ok()?;
        let img = if img.width() > 1280 || img.height() > 1280 {
            img.resize(1280, 1280, image::imageops::FilterType::Triangle)
        } else {
            img
        };
        Some(img.to_rgba8())
    }

    fn upload(&mut self, gpu: &mut omsi_ui::Gpu, device: &wgpu::Device, queue: &wgpu::Queue) {
        let Some(img) = self.pending.take() else {
            return;
        };
        let (w, h) = img.dimensions();
        match self.texture {
            Some(id) => gpu.update_image(device, queue, id, w, h, img.as_raw()),
            None => self.texture = Some(gpu.add_image(device, queue, w, h, img.as_raw())),
        }
        self.aspect = Some(w as f32 / h.max(1) as f32);
    }

    fn shown(&self) -> Option<(usize, f32)> {
        Some((self.texture?, self.aspect?))
    }
}
