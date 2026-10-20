//! The new interface in the game.
//!
//! Today this is the Esc menu: while the game menu is open at its top level, the column of
//! sections from `omsi_ui::pause_menu` is drawn over the window's picture through the
//! renderer's overlay hook (`Renderer::set_overlay_hook`), in place of the HUD's lines. The
//! menu stays the game's own: its lines come from `game_menu_items`, a click does what
//! `menu_choose` does for the same line, the keys are the HUD menu's, and the simulation
//! pauses as it did. What is behind a line ("Options...", "Controls...") is still the HUD's
//! list: while one is open the HUD draws it and this module draws nothing.
//!
//! The overlay runs inside the renderer with no access to the game, so the game hands it the
//! frame to draw through [`Shared`] before each render.

use crate::app::App;
use glam::Vec2;
use omsi_ui::pause_menu::{self, Item, Kind, Section, View};
use omsi_ui::widgets::Ui;
use omsi_ui::{Draw, Gpu};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use winit::event_loop::ActiveEventLoop;

const LOGO: &[u8] = include_bytes!("../../../assets/logos/wordmark-on-dark.png");

/// The logo is shrunk to this width before it goes to the GPU (twice what the screens draw
/// it at, so it stays sharp at 150% and is not thinned out from 2000 pixels at once).
const LOGO_WIDTH: u32 = 640;

/// The wordmark ready for the GPU: shrunk, and its black field made see-through, so that over
/// the loading screen's photos and the dimmed world it stands with no black box round it.
fn logo_rgba() -> Option<image::RgbaImage> {
    let img = image::load_from_memory(LOGO).ok()?;
    let h = (img.height() as u64 * LOGO_WIDTH as u64 / img.width().max(1) as u64).max(1) as u32;
    let mut img = img
        .resize_exact(LOGO_WIDTH, h, image::imageops::FilterType::Lanczos3)
        .to_rgba8();
    omsi_ui::loading::unmatte_black(&mut img);
    Some(img)
}

/// The design the menu is made for, in points (the same as the launcher's).
const DESIGN: (f32, f32) = (1672.0, 941.0);
const MIN_SCALE: f32 = 0.75;

/// Pixels per design point for a window of `w` x `h` pixels.
pub(crate) fn scale_for(w: u32, h: u32) -> f32 {
    (w as f32 / DESIGN.0)
        .min(h as f32 / DESIGN.1)
        .max(MIN_SCALE)
}

#[derive(Clone, Debug, PartialEq)]
struct OwnedItem {
    id: String,
    icon: &'static str,
    label: String,
    key: &'static str,
    kind: Kind,
    disabled: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
struct OwnedSection {
    title: &'static str,
    items: Vec<OwnedItem>,
}

/// What one frame of the menu shows, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct MenuFrame {
    sections: Vec<OwnedSection>,
    title: String,
    clock: String,
    state: &'static str,
    selected: Option<String>,
}

/// Where a game menu line goes in the column, its icon, the key that does the same, and how it
/// is drawn. A line not listed here is shown under "Game" with a plain icon, so that a line
/// added to the game's menu is never lost.
fn place(id: &str) -> (usize, &'static str, &'static str, Kind) {
    match id {
        "resume" => (0, "play_arrow", "Esc", Kind::Primary),
        "tobus" => (0, "directions_bus", "", Kind::Normal),
        "duty" => (0, "route", "", Kind::Normal),
        "skipstop" => (0, "turn_slight_right", "", Kind::Normal),
        "endduty" => (0, "stop_circle", "", Kind::Normal),
        "options" => (1, "settings", "", Kind::Normal),
        "controls" => (1, "keyboard", "", Kind::Normal),
        "camera" => (1, "videocam", "", Kind::Normal),
        "vehicle" => (1, "directions_bus", "", Kind::Normal),
        "world" => (1, "cloud", "", Kind::Normal),
        "map" => (1, "map", "", Kind::Normal),
        "copycode" => (1, "content_copy", "", Kind::Normal),
        "admin" => (1, "settings", "", Kind::Normal),
        "save" => (2, "save", "", Kind::Normal),
        "saveslot" => (2, "download", "", Kind::Normal),
        "load" => (2, "upload", "", Kind::Normal),
        "shot" => (2, "photo_camera", "", Kind::Normal),
        "quit" => (2, "power_settings_new", "", Kind::Danger),
        _ => (2, "settings", "", Kind::Normal),
    }
}

const SECTIONS: [&str; 3] = ["Session", "Settings", "Game"];

/// The menu for the game menu's lines `items` ((what, label), as `game_menu_items` gives
/// them), the line `selected` on the keyboard (none while the mouse leads), and `off(k)` for a greyed-out line `k`.
pub(crate) fn menu_frame(
    items: &[(&str, &str)],
    selected: Option<usize>,
    off: &dyn Fn(usize) -> bool,
    clock: String,
    paused: bool,
) -> MenuFrame {
    let mut sections: Vec<OwnedSection> = SECTIONS
        .iter()
        .map(|t| OwnedSection {
            title: t,
            items: Vec::new(),
        })
        .collect();
    for (k, (id, label)) in items.iter().enumerate() {
        let (at, icon, key, kind) = place(id);
        sections[at].items.push(OwnedItem {
            id: (*id).to_string(),
            icon,
            // (English: the widgets translate what they draw, a second time would garble it)
            label: (*label).to_string(),
            key,
            kind,
            disabled: off(k).then(|| "Not available now".to_string()),
        });
    }
    sections.retain(|s| !s.items.is_empty());
    MenuFrame {
        sections,
        title: "Game menu".to_string(),
        clock,
        state: if paused { "paused" } else { "menu open" },
        selected: selected.and_then(|k| items.get(k)).map(|m| m.0.to_string()),
    }
}

/// The game menu's lines in the order the column shows them (section by section, each in the
/// game's own order), as indices into `items`.
pub(crate) fn display_order(items: &[(&str, &str)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&k| place(items[k].0).0);
    order
}

/// The line after (or before) `from` in the displayed order, skipping greyed-out ones.
pub(crate) fn step_displayed(
    items: &[(&str, &str)],
    from: usize,
    down: bool,
    off: &dyn Fn(usize) -> bool,
) -> usize {
    let order = display_order(items);
    let n = order.len();
    let Some(at) = order.iter().position(|&k| k == from) else {
        return from;
    };
    for i in 1..=n {
        let next = if down {
            (at + i) % n
        } else {
            (at + n - i % n) % n
        };
        if !off(order[next]) {
            return order[next];
        }
    }
    from
}

impl MenuFrame {
    fn with_view<R>(&self, f: impl FnOnce(&View) -> R, logo: Option<(usize, f32)>) -> R {
        let sections: Vec<Section> = self
            .sections
            .iter()
            .map(|s| Section {
                title: s.title,
                items: s
                    .items
                    .iter()
                    .map(|i| Item {
                        id: &i.id,
                        icon: i.icon,
                        label: &i.label,
                        key: i.key,
                        kind: i.kind,
                        disabled: i.disabled.as_deref(),
                    })
                    .collect(),
            })
            .collect();
        let view = View {
            logo,
            sections: &sections,
            title: &self.title,
            subtitle: "",
            clock: &self.clock,
            state: self.state,
            selected: self.selected.as_deref(),
        };
        f(&view)
    }

    /// Pixels per design point for this menu in a window of `size` pixels: the usual one, or
    /// less when the lines would run off the bottom of a short window.
    pub(crate) fn scale(&self, size: (u32, u32)) -> f32 {
        let (w, h) = (size.0 as f32, size.1 as f32);
        let natural = (w / DESIGN.0).min(h / DESIGN.1);
        let mut s = natural.max(MIN_SCALE);
        for _ in 0..4 {
            let pts = Vec2::new(w, h) / s;
            let bottom = self.with_view(
                |v| {
                    pause_menu::rows(v, pts)
                        .iter()
                        .map(|r| r.rect.y + r.rect.h)
                        .fold(0.0, f32::max)
                },
                None,
            );
            let need = (bottom + 16.0) * s;
            if need <= h {
                return s;
            }
            s = (s * h / need).max(natural);
        }
        natural
    }

    /// The id of the clickable line under `px` (window pixels) in a window of `size` pixels.
    pub(crate) fn hit(&self, px: Vec2, size: (u32, u32)) -> Option<String> {
        let scale = self.scale(size);
        let pts = Vec2::new(size.0 as f32, size.1 as f32) / scale;
        self.with_view(
            |v| pause_menu::hit(v, pts, px / scale).map(String::from),
            None,
        )
    }
}

/// What the game tells the overlay each frame.
#[derive(Default)]
pub(crate) struct State {
    menu: Option<MenuFrame>,
    alt: Option<crate::new_ui_alt::AltFrame>,
    /// The loading screen, while the first area of the map loads.
    loading: Option<crate::new_ui_loading::LoadingFrame>,
    /// Screenshots decoded on their thread and not yet in the GPU.
    slides: Vec<crate::new_ui_loading::Decoded>,
    /// The bus's block of the loading card, once its file has been read.
    bus_card: Option<crate::new_ui_loading::CardText>,
    /// The cursor, in window pixels.
    mouse: Vec2,
}

type Shared = Arc<Mutex<State>>;

/// The overlay's own state, owned by the closure the renderer calls.
struct Overlay {
    shared: Shared,
    format: wgpu::TextureFormat,
    ui: Ui,
    gpu: Option<Gpu>,
    logo: Option<(usize, f32)>,
    last: Instant,
    /// The loading screen's pictures in the GPU, and how far its slideshow is.
    slide_tex: Vec<omsi_ui::loading::Slide>,
    show: omsi_ui::loading::Slideshow,
}

impl Overlay {
    fn new(shared: Shared, format: wgpu::TextureFormat) -> Overlay {
        Overlay {
            shared,
            format,
            ui: Ui::new(),
            gpu: None,
            logo: None,
            last: Instant::now(),
            slide_tex: Vec::new(),
            show: omsi_ui::loading::Slideshow::new(0),
        }
    }

    fn draw(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        view: &wgpu::TextureView,
        (w, h): (u32, u32),
    ) {
        let (menu, alt, loading, mouse, arrived) = match self.shared.lock() {
            Ok(mut s) => {
                let arrived = std::mem::take(&mut s.slides);
                (
                    s.menu.clone(),
                    s.alt.clone(),
                    s.loading.clone(),
                    s.mouse,
                    arrived,
                )
            }
            Err(_) => return,
        };
        if loading.is_none() && !self.slide_tex.is_empty() {
            // (the loading screen is over: its pictures leave the GPU)
            if let Some(gpu) = self.gpu.as_mut() {
                for s in self.slide_tex.drain(..) {
                    gpu.free(s.tex);
                }
            }
            self.show = omsi_ui::loading::Slideshow::new(0);
        }
        if menu.is_none() && alt.is_none() && loading.is_none() {
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        let format = self.format;
        let atlas = self.ui.atlas.size;
        let gpu = self
            .gpu
            .get_or_insert_with(|| Gpu::new(device, format, 1, atlas));
        if self.logo.is_none() {
            if let Some(img) = logo_rgba() {
                let tex = gpu.add_image(device, queue, img.width(), img.height(), img.as_raw());
                self.logo = Some((tex, img.height() as f32 / img.width() as f32));
            }
        }
        let scale = menu
            .as_ref()
            .map_or_else(|| scale_for(w, h), |m| m.scale((w, h)));
        let size = Vec2::new(w as f32, h as f32) / scale;
        self.ui.begin(size, scale, dt);
        self.ui.input.mouse = mouse / scale;
        if let Some(l) = loading {
            for d in arrived.into_iter().take(crate::new_ui_loading::MAX_SLIDES) {
                if self.slide_tex.len() >= crate::new_ui_loading::MAX_SLIDES {
                    break;
                }
                let tex = gpu.add_image(device, queue, d.w, d.h, &d.rgba);
                self.slide_tex.push(omsi_ui::loading::Slide {
                    tex,
                    aspect: d.w as f32 / d.h as f32,
                });
            }
            self.show.set_count(self.slide_tex.len());
            self.show.update(dt);
            fn card<'a>(
                c: &'a Option<crate::new_ui_loading::CardText>,
                icon: &'a str,
            ) -> Option<omsi_ui::loading::Card<'a>> {
                c.as_ref().map(|c| omsi_ui::loading::Card {
                    title: &c.title,
                    sub: &c.sub,
                    description: &c.description,
                    icon,
                })
            }
            let view = omsi_ui::loading::View {
                version: &l.version,
                heading: &l.heading,
                stage: &l.stage,
                activity: &l.activity,
                progress: l.progress,
                logo: self.logo,
                bus: card(&l.bus, "directions_bus"),
                map: card(&l.map, "map"),
                slides: &self.slide_tex,
            };
            omsi_ui::loading::draw(&mut self.ui, &view, &self.show);
        } else if let Some(menu) = menu {
            // (the world behind the menu is dimmed, the column and the bar drawn over it)
            self.ui.p().rect(
                omsi_ui::Rect::new(0.0, 0.0, size.x, size.y),
                omsi_ui::Color::hex(0x000000).alpha(0.35),
            );
            menu.with_view(
                |v| {
                    pause_menu::draw(&mut self.ui, v);
                },
                self.logo,
            );
        } else if let Some(alt) = alt {
            // (the world is not dimmed: the game goes on behind the bar)
            alt.draw(&mut self.ui);
        }
        let (layers, verts, ranges) = self.ui.finish();
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
        gpu.upload(device, queue, 0, &verts);
        gpu.upload_atlas(queue, &mut self.ui.atlas);
        // (no clear: this is drawn over the finished picture)
        gpu.render(device, queue, encoder, view, (w, h), None, &layers, &draws);
    }
}

/// The game's side of the overlay.
#[derive(Default)]
pub(crate) struct NewUi {
    shared: Shared,
    installed: bool,
    slides_started: bool,
    bus_started: bool,
    pub(crate) alt: crate::new_ui_alt::AltState,
}

impl NewUi {
    /// Hand the renderer the overlay (once).
    pub(crate) fn install(&mut self, renderer: &mut omsi_render::Renderer) {
        if self.installed {
            return;
        }
        let mut overlay = Overlay::new(self.shared.clone(), renderer.format());
        renderer.set_overlay_hook(Some(Box::new(move |device, queue, encoder, view, size| {
            overlay.draw(device, queue, encoder, view, size)
        })));
        self.installed = true;
    }

    /// What the loading screen says now (`None`: it is over).
    pub(crate) fn set_loading(&self, frame: Option<crate::new_ui_loading::LoadingFrame>) {
        if let Ok(mut s) = self.shared.lock() {
            s.loading = frame;
        }
    }

    /// Once per session: list the map's screenshots and decode the latest on a thread of
    /// their own, handing them to the overlay one by one.
    pub(crate) fn start_slides(&mut self, map_folder: &str) {
        if self.slides_started {
            return;
        }
        self.slides_started = true;
        let shared = self.shared.clone();
        let map = map_folder.to_string();
        let _ = std::thread::Builder::new()
            .name("loading-slides".into())
            .spawn(move || {
                let paths = crate::new_ui_loading::recent(
                    omsi_launcher_lib::screenshots::for_map(&map),
                    crate::new_ui_loading::MAX_SLIDES,
                );
                for p in paths {
                    if let Some(d) = crate::new_ui_loading::decode(&p) {
                        match shared.lock() {
                            Ok(mut s) => s.slides.push(d),
                            Err(_) => return,
                        }
                    }
                }
            });
    }

    /// Once per session: read the bus's file (`bus`, under `root`) on a thread of its own for
    /// the loading card's name, maker, paint and description.
    pub(crate) fn start_bus(&mut self, root: &std::path::Path, bus: &str, paint: Option<&str>) {
        if self.bus_started {
            return;
        }
        self.bus_started = true;
        let shared = self.shared.clone();
        let path = omsi_cfg::resolve_path(root, bus);
        let folder = crate::screenshot_name::folder_of(bus).unwrap_or_default();
        let paint = paint.map(str::to_string);
        let _ = std::thread::Builder::new()
            .name("loading-bus".into())
            .spawn(move || {
                let Ok(v) = omsi_vehicle::Vehicle::load(&path) else {
                    return;
                };
                let card = crate::new_ui_loading::bus_card(&v, &folder, paint.as_deref());
                if let Ok(mut s) = shared.lock() {
                    s.bus_card = Some(card);
                }
            });
    }

    /// The bus's block of the loading card, when its file has been read.
    pub(crate) fn bus_card(&self) -> Option<crate::new_ui_loading::CardText> {
        self.shared.lock().ok().and_then(|s| s.bus_card.clone())
    }

    fn set(&self, menu: Option<MenuFrame>, alt: Option<crate::new_ui_alt::AltFrame>, mouse: Vec2) {
        if let Ok(mut s) = self.shared.lock() {
            s.menu = menu;
            s.alt = alt;
            s.mouse = mouse;
        }
    }
}

impl App {
    /// Whether the new Esc menu is the one showing: the game menu is open at its top level (no
    /// list opened from it), and the picture is the plain window's (the triple screen and the
    /// headset keep the HUD's menu, their own layouts).
    pub(crate) fn new_menu_active(&self) -> bool {
        self.game_menu.is_some()
            && self.chooser.is_none()
            && self.list_kind.is_none()
            && !self.vr_active()
            && !self.settings.triple.enabled
    }

    /// The menu as it is drawn now.
    pub(crate) fn new_menu_frame(&self) -> MenuFrame {
        let items = self.game_menu_items();
        menu_frame(
            &items,
            self.menu_kbd.then(|| self.game_menu.unwrap_or(0)),
            &|k| self.menu_item_off(k),
            crate::schedule::hhmm(self.clock.time),
            self.paused,
        )
    }

    /// Arrow keys in the new menu walk the lines as they are shown.
    pub(crate) fn new_menu_step(&self, from: usize, down: bool) -> usize {
        step_displayed(&self.game_menu_items(), from, down, &|k| {
            self.menu_item_off(k)
        })
    }

    /// Before each render: give the renderer the overlay and the frame it should draw.
    pub(crate) fn new_ui_update(&mut self) {
        if let Some(r) = self.renderer.as_mut() {
            self.new_ui.install(r);
        }
        let menu = self.new_menu_active().then(|| self.new_menu_frame());
        // (the Esc menu hides the bar and takes the keys back)
        if self.game_menu.is_some() {
            self.new_ui.alt.focus = None;
        }
        let alt = self.alt_bar_active().then(|| self.alt_frame());
        self.new_ui
            .set(menu, alt, Vec2::new(self.cursor.0, self.cursor.1));
    }

    /// A left click while the game menu is open: when the new menu is showing, a click on one of
    /// its lines does what the HUD's menu does for that line. Returns whether the new menu took
    /// the click (it takes every click while it is showing, so that none reaches the HUD's rows
    /// that are not drawn).
    pub(crate) fn new_menu_click(&mut self, event_loop: &ActiveEventLoop) -> bool {
        if !self.new_menu_active() {
            return false;
        }
        let Some(s) = self.surface.as_ref() else {
            return false;
        };
        let size = (s.config.width, s.config.height);
        let frame = self.new_menu_frame();
        if let Some(id) = frame.hit(Vec2::new(self.cursor.0, self.cursor.1), size) {
            if let Some(k) = self.game_menu_items().iter().position(|m| m.0 == id) {
                self.menu_kbd = false;
                self.game_menu = Some(k);
                self.menu_choose(event_loop, k);
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        crate::input::menus::GAME_MENU.to_vec()
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
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
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
        let mut overlay = Overlay::new(shared.clone(), format);
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
            s.loading = Some(crate::new_ui_loading::LoadingFrame {
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
            .push(crate::new_ui_loading::Decoded {
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
}
