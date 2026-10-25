//! The new interface in the game.
//!
//! Today this is the Esc menu: while the game menu is open at its top level, the column of
//! sections from `omsi_ui::pause_menu` is drawn over the window's picture through the
//! renderer's overlay hook (`Renderer::set_overlay_hook`), in place of the HUD's lines. The
//! menu stays the game's own: its lines come from `game_menu_items`, a click does what
//! `menu_choose` does for the same line, the keys are the HUD menu's, and the simulation
//! pauses as it did. What is behind a line ("Options...", "Controls...", the lists) is drawn
//! here too, beside the column: see `new_ui_pages`.
//!
//! Every picture has them: on a triple screen they stand on the centre screen (the hook is
//! called once the three panels are in the window, and draws in that screen's [`Area`]), and
//! in the headset they are drawn into a texture of the window's size that the menu plane
//! shows where the HUD's menu was (`NewUi::headset_surface`).
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

/// Where the new interface is drawn in the window, in pixels: the whole window, or the
/// centre screen of a triple screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Area {
    /// Its top-left corner in the window.
    pub(crate) origin: Vec2,
    pub(crate) size: (u32, u32),
}

impl Area {
    /// The whole of a window of `size` pixels.
    pub(crate) fn whole(size: (u32, u32)) -> Area {
        Area {
            origin: Vec2::ZERO,
            size,
        }
    }

    /// Window pixels `at` as pixels of the area.
    pub(crate) fn local(&self, at: Vec2) -> Vec2 {
        at - self.origin
    }

    /// The area where it lies inside a picture of `size` pixels; else (another picture) the
    /// whole of it.
    fn within(self, size: (u32, u32)) -> Area {
        let fits = self.size.0 > 0
            && self.size.1 > 0
            && self.origin.x >= 0.0
            && self.origin.y >= 0.0
            && self.origin.x + self.size.0 as f32 <= size.0 as f32
            && self.origin.y + self.size.1 as f32 <= size.1 as f32;
        if fits {
            self
        } else {
            Area::whole(size)
        }
    }
}

/// Where the menus are drawn in a window of `size` pixels: on a triple screen (`triple`, not
/// in the headset: `vr`) the centre screen, as the renderer lays the panels out
/// (`omsi_render::panel_width`) - the menus are made for one screen's shape, so they stand
/// there whether or not the rest of the HUD spans the three; else the whole window (the
/// headset's menu plane shows the whole window's picture).
pub(crate) fn menu_area(triple: bool, vr: bool, size: (u32, u32)) -> Area {
    if triple && !vr && size.0 >= 3 {
        let w = omsi_render::panel_width(size.0);
        Area {
            origin: Vec2::new(w as f32, 0.0),
            size: (w, size.1),
        }
    } else {
        Area::whole(size)
    }
}

/// Move the layers of a frame laid out in `area` (at `scale` pixels per point) to where the
/// area is in a picture of `target` pixels: a point `p` is drawn at `origin + p * scale`, and
/// the clips go with it. (A layer maps its viewport onto the whole picture: see `ui.wgsl`.)
fn place_layers(layers: &mut [omsi_ui::Layer], area: Area, scale: f32, target: (u32, u32)) {
    if area == Area::whole(target) {
        return;
    }
    let o = area.origin;
    for l in layers {
        l.viewport = [
            -o.x / scale,
            -o.y / scale,
            target.0 as f32 / scale,
            target.1 as f32 / scale,
        ];
        l.clip = [
            l.clip[0] + o.x,
            l.clip[1] + o.y,
            l.clip[2] + o.x,
            l.clip[3] + o.y,
        ];
    }
}

/// Where the headset's menu plane takes its two pictures from, as the HUD's menu had them
/// (`omsi_render`'s `render_xr_ui`): the first, over the whole window, dims the view; the
/// second (the new interface's texture) covers the plane, which shows the whole window in
/// window pixels - so a cursor at a window pixel is on the same pixel of the texture.
#[cfg_attr(not(windows), allow(dead_code))]
pub(crate) fn headset_rects(size: (u32, u32)) -> [[f32; 4]; 2] {
    let all = [0.0, 0.0, size.0 as f32, size.1 as f32];
    [all, all]
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
    subtitle: String,
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
        subtitle: String::new(),
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
    pub(crate) fn with_view<R>(&self, f: impl FnOnce(&View) -> R, logo: Option<(usize, f32)>) -> R {
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
            subtitle: &self.subtitle,
            clock: &self.clock,
            state: self.state,
            selected: self.selected.as_deref(),
        };
        f(&view)
    }

    /// The column beside an open page: the line the page belongs to marked as the one shown
    /// (the first of `lines` the menu has), no line lit by the keyboard, and the bar saying
    /// what the page is.
    pub(crate) fn for_page(mut self, lines: &[&str], title: String, subtitle: String) -> MenuFrame {
        let have = |id: &str| {
            self.sections
                .iter()
                .any(|s| s.items.iter().any(|i| i.id == id))
        };
        if let Some(id) = lines.iter().copied().find(|id| have(id)) {
            for item in self.sections.iter_mut().flat_map(|s| s.items.iter_mut()) {
                if item.id == id {
                    item.kind = Kind::Current;
                }
            }
        }
        self.selected = None;
        self.title = title;
        self.subtitle = subtitle;
        self
    }

    /// The line marked as the page shown, if any.
    #[cfg(test)]
    pub(crate) fn current(&self) -> Option<&str> {
        self.sections
            .iter()
            .flat_map(|s| s.items.iter())
            .find(|i| i.kind == Kind::Current)
            .map(|i| i.id.as_str())
    }

    /// What the bar over the page says: the title, the subtitle and the state ("paused").
    #[cfg(test)]
    pub(crate) fn bar(&self) -> (&str, &str, &str) {
        (&self.title, &self.subtitle, self.state)
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
    /// A page behind one of the menu's lines, while one is open.
    page: Option<crate::new_ui_pages::PageFrame>,
    alt: Option<crate::new_ui_alt::AltFrame>,
    /// The "Add vehicle" window.
    add: Option<crate::new_ui_add::AddFrame>,
    /// The end-of-trip report.
    report: Option<crate::new_ui_report::ReportFrame>,
    /// "Continue" was clicked in the report.
    report_done: bool,
    /// Clicks and the wheel over the window showing, not yet handed to it (one a frame).
    win_input: std::collections::VecDeque<crate::new_ui_add::WinInput>,
    /// What clicks in that window chose, for the game to apply.
    add_events: Vec<omsi_ui::ingame::AddVehicleEvent>,
    /// A dropdown of that window has its list open: every click goes to the window then.
    add_modal: bool,
    /// The picture of the vehicle and paint chosen there, read on its thread and not yet in
    /// the GPU (none inside: no picture was drawn of them).
    add_picture: Option<(u64, Option<crate::new_ui_add::Picture>)>,
    /// The picture the window wants now: a picture read for one chosen before is dropped.
    add_picture_want: Option<u64>,
    /// The window is open (also while the Esc menu or the report hides it: its picture stays
    /// in the GPU then).
    add_open: bool,
    /// The Alt bar's window open (reposition, date and time, weather...).
    tool: Option<crate::new_ui_windows::ToolFrame>,
    /// What clicks in it asked for, for the game to apply.
    tool_events: Vec<omsi_ui::alt_windows::Event>,
    /// The loading screen, while the first area of the map loads.
    loading: Option<crate::new_ui_loading::LoadingFrame>,
    /// Screenshots decoded on their thread and not yet in the GPU.
    slides: Vec<crate::new_ui_loading::Decoded>,
    /// The bus's block of the loading card, once its file has been read.
    bus_card: Option<crate::new_ui_loading::CardText>,
    /// The map's own picture for its block of the card, decoded and not yet in the GPU.
    map_picture: Option<crate::new_ui_loading::Decoded>,
    /// Which loading the screenshots and the bus card being read belong to (one more each
    /// time a loading is over).
    loading_gen: u64,
    /// The cursor, in window pixels.
    mouse: Vec2,
    /// Where in the window the interface is drawn (none: the whole picture).
    area: Option<Area>,
}

type Shared = Arc<Mutex<State>>;

/// The overlay's own state, owned by the closure the renderer calls.
struct Overlay {
    shared: Shared,
    format: wgpu::TextureFormat,
    ui: Ui,
    gpu: Option<Gpu>,
    /// Handed to the Gpu when it is made: a second renderer draws the same ("DirectX9
    /// (D3D9Ex)").
    mirror: Option<omsi_ui::UiMirror>,
    logo: Option<(usize, f32)>,
    last: Instant,
    /// The loading screen's pictures in the GPU, and how far its slideshow is.
    slide_tex: Vec<omsi_ui::loading::Slide>,
    /// The map's picture in the GPU for the card's thumbnail.
    map_tex: Option<omsi_ui::loading::Slide>,
    show: omsi_ui::loading::Slideshow,
    /// The loading those pictures belong to (`State::loading_gen`).
    gen: u64,
    /// The "Add vehicle" window's picture: which vehicle and paint it is, and in the GPU its
    /// texture, its width over its height and where the vehicle stands in it (none: there is
    /// no picture of them).
    add_picture: Option<(u64, Option<(usize, f32, [f32; 4])>)>,
    /// The window was drawn last time (a list of it left open is closed when it goes).
    add_drawn: bool,
}

impl Overlay {
    fn new(
        shared: Shared,
        format: wgpu::TextureFormat,
        mirror: Option<omsi_ui::UiMirror>,
    ) -> Overlay {
        Overlay {
            shared,
            format,
            ui: Ui::new(),
            gpu: None,
            mirror,
            logo: None,
            last: Instant::now(),
            slide_tex: Vec::new(),
            map_tex: None,
            show: omsi_ui::loading::Slideshow::new(0),
            gen: 0,
            add_picture: None,
            add_drawn: false,
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
        let (
            menu,
            page,
            alt,
            add,
            tool,
            report,
            win_input,
            loading,
            mouse,
            (arrived, map_picture),
            gen,
            area,
            (add_picture, add_open),
        ) = match self.shared.lock() {
            Ok(mut s) => {
                let arrived = std::mem::take(&mut s.slides);
                let add_picture = s.add_picture.take();
                let map_picture = s.map_picture.take();
                let input = if s.add.is_some() || s.tool.is_some() || s.report.is_some() {
                    s.win_input.pop_front()
                } else {
                    s.win_input.clear();
                    None
                };
                (
                    s.menu.clone(),
                    s.page.clone(),
                    s.alt.clone(),
                    s.add.clone(),
                    s.tool.clone(),
                    s.report.clone(),
                    input,
                    s.loading.clone(),
                    s.mouse,
                    (arrived, map_picture),
                    s.loading_gen,
                    s.area,
                    (add_picture, s.add_open),
                )
            }
            Err(_) => return,
        };
        if (loading.is_none() || gen != self.gen) && self.map_tex.is_some() {
            if let (Some(gpu), Some(t)) = (self.gpu.as_mut(), self.map_tex.take()) {
                gpu.free(t.tex);
            }
        }
        if (loading.is_none() || gen != self.gen) && !self.slide_tex.is_empty() {
            // (the loading screen is over, or another one began: its pictures leave the GPU)
            if let Some(gpu) = self.gpu.as_mut() {
                for s in self.slide_tex.drain(..) {
                    gpu.free(s.tex);
                }
            }
            self.show = omsi_ui::loading::Slideshow::new(0);
        }
        self.gen = gen;
        if menu.is_none()
            && page.is_none()
            && alt.is_none()
            && add.is_none()
            && tool.is_none()
            && report.is_none()
            && loading.is_none()
        {
            return;
        }
        let now = Instant::now();
        let dt = now.duration_since(self.last).as_secs_f32();
        self.last = now;
        let format = self.format;
        let atlas = self.ui.atlas.size;
        let mirror = &mut self.mirror;
        let gpu = self.gpu.get_or_insert_with(|| {
            let mut gpu = Gpu::new(device, format, 1, atlas);
            gpu.set_mirror(mirror.take());
            gpu
        });
        if self.logo.is_none() {
            if let Some(img) = logo_rgba() {
                let tex = gpu.add_image(device, queue, img.width(), img.height(), img.as_raw());
                self.logo = Some((tex, img.height() as f32 / img.width() as f32));
            }
        }
        // the "Add vehicle" window's picture: a new one in the GPU, the one before out of it
        // (and out of it too once the window is closed; not while it is only hidden)
        let stale = add_picture.is_some() || !add_open;
        if stale {
            if let Some((_, Some((tex, _, _)))) = self.add_picture.take() {
                gpu.free(tex);
            }
        }
        if let Some((key, pic)) = add_picture {
            let up = pic.map(|p| {
                let tex = gpu.add_image(device, queue, p.w, p.h, &p.rgba);
                (tex, p.w as f32 / p.h.max(1) as f32, p.part)
            });
            self.add_picture = Some((key, up));
        }
        // (a list of the window left open when it closed or was hidden is closed: it would stay
        // drawn over what comes next and take its first click)
        if add.is_none() && std::mem::replace(&mut self.add_drawn, false) {
            self.ui.close_popup();
        }
        self.add_drawn = add.is_some();
        // (laid out in the area: the centre screen of a triple screen, else the picture)
        let area = area.map_or(Area::whole((w, h)), |a| a.within((w, h)));
        let (aw, ah) = area.size;
        let scale = match (menu.as_ref(), page.as_ref()) {
            (_, Some(p)) => p.scale(area.size),
            (Some(m), None) => m.scale(area.size),
            (None, None) => scale_for(aw, ah),
        };
        let size = Vec2::new(aw as f32, ah as f32) / scale;
        // (the window's click or wheel of this frame, see `NewUi::push_input`)
        match win_input {
            Some(crate::new_ui_add::WinInput::Press) => {
                self.ui.input.pressed = true;
                self.ui.input.down = true;
            }
            Some(crate::new_ui_add::WinInput::Release) => {
                self.ui.input.released = true;
                self.ui.input.down = false;
            }
            Some(crate::new_ui_add::WinInput::Wheel(y)) => self.ui.input.wheel = Vec2::new(0.0, y),
            Some(crate::new_ui_add::WinInput::Lost) => self.ui.input.down = false,
            None if add.is_none() && tool.is_none() && report.is_none() => {
                self.ui.input.down = false
            }
            None => {}
        }
        self.ui.begin(size, scale, dt);
        self.ui.input.mouse = area.local(mouse) / scale;
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
            if let (Some(d), None) = (map_picture, self.map_tex) {
                let tex = gpu.add_image(device, queue, d.w, d.h, &d.rgba);
                self.map_tex = Some(omsi_ui::loading::Slide {
                    tex,
                    aspect: d.w as f32 / d.h.max(1) as f32,
                });
            }
            self.show.set_count(self.slide_tex.len());
            self.show.update(dt);
            fn card<'a>(
                c: &'a Option<crate::new_ui_loading::CardText>,
                icon: &'a str,
                thumb: Option<omsi_ui::loading::Slide>,
            ) -> Option<omsi_ui::loading::Card<'a>> {
                c.as_ref().map(|c| omsi_ui::loading::Card {
                    title: &c.title,
                    sub: &c.sub,
                    detail: &c.detail,
                    icon,
                    thumb: thumb.map(|t| (t.tex, t.aspect)),
                })
            }
            let view = omsi_ui::loading::View {
                version: &l.version,
                heading: &l.heading,
                stage: &l.stage,
                activity: &l.activity,
                progress: l.progress,
                logo: self.logo,
                bus: card(&l.bus, "directions_bus", None),
                map: card(&l.map, "map", self.map_tex),
                slides: &self.slide_tex,
            };
            omsi_ui::loading::draw(&mut self.ui, &view, &self.show);
        } else if let Some(page) = page {
            // (a page behind a line of the menu: the column beside it, the world dimmed)
            page.draw(&mut self.ui, size, self.logo);
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
        } else if let Some(report) = report {
            // (the simulation stands still behind the report, as behind the menu)
            self.ui.p().rect(
                omsi_ui::Rect::new(0.0, 0.0, size.x, size.y),
                omsi_ui::Color::hex(0x000000).alpha(0.35),
            );
            if report.draw(&mut self.ui, size) {
                if let Ok(mut s) = self.shared.lock() {
                    s.report_done = true;
                }
            }
        } else {
            // (the world is not dimmed: the game goes on behind the bar and the window)
            if let Some(alt) = alt {
                alt.draw(&mut self.ui);
            }
            if let Some(add) = add {
                let preview = add.preview(self.add_picture);
                let e = add.draw(&mut self.ui, size, preview);
                let modal = self.ui.popup_open();
                if let Ok(mut s) = self.shared.lock() {
                    s.add_events.extend(e);
                    s.add_modal = modal;
                }
            }
            if let Some(tool) = tool {
                if let Some(e) = tool.draw(&mut self.ui, size) {
                    if let Ok(mut s) = self.shared.lock() {
                        s.tool_events.push(e);
                    }
                }
            }
        }
        let (mut layers, verts, ranges) = self.ui.finish();
        place_layers(&mut layers, area, scale, (w, h));
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
    map_picture_started: bool,
    pub(crate) alt: crate::new_ui_alt::AltState,
    /// The "Add vehicle" window, while it is open.
    pub(crate) add: Option<crate::new_ui_add::AddWindow>,
    /// Keys the "Add vehicle" window's registration field took: their release is its too.
    pub(crate) add_taken: std::collections::HashSet<winit::keyboard::KeyCode>,
    /// The vehicle and paint whose picture the "Add vehicle" window asked for last.
    add_picture: Option<u64>,
    /// The manufacturer and the vehicle file chosen last in that window: it opens on them.
    pub(crate) add_last: Option<(String, String)>,
    /// The page open behind a line of the Esc menu, as the HUD's frame of this frame made it.
    pub(crate) page: Option<crate::new_ui_pages::PageFrame>,
    /// The headset's picture of the menu and the page (a render texture of the window's size,
    /// and its size), and the plate that dims the view behind them.
    #[cfg_attr(not(windows), allow(dead_code))]
    headset: Option<(omsi_render::TextureId, u32, u32)>,
    #[cfg_attr(not(windows), allow(dead_code))]
    headset_dim: Option<omsi_render::TextureId>,
    /// The Alt bar's other windows.
    pub(crate) tool: crate::new_ui_windows::Tools,
    /// The end-of-trip report, while it is open.
    pub(crate) report: Option<crate::new_ui_report::Report>,
    /// The trip whose report was shown last (line, tour, trip in the tour): it opens once.
    pub(crate) report_shown: Option<(String, String, usize)>,
    /// A trip at its last stop whose report waits for the doors to open there: its key, its
    /// place in the duty and where that stop is.
    pub(crate) report_watch: Option<crate::new_ui_report::Watch>,
}

impl NewUi {
    pub(crate) fn installed(&self) -> bool {
        self.installed
    }

    /// Hand the renderer the overlay (once), with the mirror its draws also go to.
    pub(crate) fn install(
        &mut self,
        renderer: &mut omsi_render::Renderer,
        mirror: Option<omsi_ui::UiMirror>,
    ) {
        if self.installed {
            return;
        }
        let mut overlay = Overlay::new(self.shared.clone(), renderer.format(), mirror);
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

    /// The loading screen is over, or a new drive's is about to start: it is taken down, and
    /// the next one lists its own map's screenshots and reads its own bus. What a thread of
    /// the last one still hands over is dropped (it belongs to another loading).
    pub(crate) fn reset_loading(&mut self) {
        self.slides_started = false;
        self.bus_started = false;
        self.map_picture_started = false;
        if let Ok(mut s) = self.shared.lock() {
            s.loading = None;
            s.slides.clear();
            s.map_picture = None;
            s.bus_card = None;
            s.loading_gen = s.loading_gen.wrapping_add(1);
        }
    }

    /// The loading the threads started now belong to.
    fn loading_gen(&self) -> u64 {
        self.shared.lock().map(|s| s.loading_gen).unwrap_or(0)
    }

    /// Once per loading: list the map's screenshots and decode the latest on a thread of
    /// their own, handing them to the overlay one by one.
    pub(crate) fn start_slides(&mut self, map_folder: &str) {
        if self.slides_started {
            return;
        }
        self.slides_started = true;
        let shared = self.shared.clone();
        let gen = self.loading_gen();
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
                            Ok(mut s) if s.loading_gen == gen => s.slides.push(d),
                            _ => return,
                        }
                    }
                }
            });
    }

    /// Once per loading: read the map's own picture on a thread of its own, for the thumbnail
    /// of the map's block of the card.
    pub(crate) fn start_map_picture(&mut self, root: &std::path::Path, map: &str) {
        if self.map_picture_started {
            return;
        }
        self.map_picture_started = true;
        let shared = self.shared.clone();
        let gen = self.loading_gen();
        let (root, map) = (root.to_path_buf(), map.to_string());
        let _ = std::thread::Builder::new()
            .name("loading-map-picture".into())
            .spawn(move || {
                if let Some(d) = crate::new_ui_loading::map_picture(&root, &map) {
                    match shared.lock() {
                        Ok(mut s) if s.loading_gen == gen => s.map_picture = Some(d),
                        _ => {}
                    }
                }
            });
    }

    /// Once per loading: read the bus's file (`bus`, under `root`) on a thread of its own for
    /// the loading card's name, maker, paint and description.
    pub(crate) fn start_bus(&mut self, root: &std::path::Path, bus: &str, paint: Option<&str>) {
        if self.bus_started {
            return;
        }
        self.bus_started = true;
        let shared = self.shared.clone();
        let gen = self.loading_gen();
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
                match shared.lock() {
                    Ok(mut s) if s.loading_gen == gen => s.bus_card = Some(card),
                    _ => {}
                }
            });
    }

    /// The bus's block of the loading card, when its file has been read.
    pub(crate) fn bus_card(&self) -> Option<crate::new_ui_loading::CardText> {
        self.shared.lock().ok().and_then(|s| s.bus_card.clone())
    }

    fn set(
        &self,
        menu: Option<MenuFrame>,
        alt: Option<crate::new_ui_alt::AltFrame>,
        add: Option<crate::new_ui_add::AddFrame>,
        tool: Option<crate::new_ui_windows::ToolFrame>,
        report: Option<crate::new_ui_report::ReportFrame>,
        mouse: Vec2,
        area: Option<Area>,
    ) {
        if let Ok(mut s) = self.shared.lock() {
            s.menu = menu;
            s.page = self.page.clone();
            s.alt = alt;
            s.add = add;
            s.tool = tool;
            s.report = report;
            s.mouse = mouse;
            s.area = area;
        }
    }

    /// The headset's picture of the Esc menu or the page open behind it: the overlay drawn
    /// into a texture of the window's `size`, laid in `scene`'s overlays after a plate that
    /// dims the view, for the menu plane (see [`headset_rects`]). The cursor works it as it
    /// works the window: the menu's and the page's rectangles are the window's. Returns where
    /// the two are in `scene.overlays`, for the menu plane; none while neither is open.
    #[cfg_attr(not(windows), allow(dead_code))]
    pub(crate) fn headset_surface(
        &mut self,
        r: &mut omsi_render::Renderer,
        scene: &mut omsi_render::Scene,
        size: (u32, u32),
    ) -> Option<std::ops::Range<usize>> {
        let open = self
            .shared
            .lock()
            .is_ok_and(|s| s.menu.is_some() || s.page.is_some());
        if !open || size.0 == 0 || size.1 == 0 {
            return None;
        }
        if self.headset.is_none_or(|t| (t.1, t.2) != size) {
            if let Some((t, _, _)) = self.headset.take() {
                r.free_texture(scene, t);
                scene.premultiplied.remove(&t);
            }
            let t = r.add_render_texture(scene, size.0, size.1);
            scene.premultiplied.insert(t);
            self.headset = Some((t, size.0, size.1));
        }
        let dim = *self.headset_dim.get_or_insert_with(|| {
            let black = omsi_texture::Image {
                width: 1,
                height: 1,
                rgba: vec![0, 0, 0, 90],
                has_alpha: true,
            };
            r.add_texture(scene, &black, false)
        });
        let (t, w, h) = self.headset?;
        r.draw_overlay_into(scene, t);
        let [back, plane] = headset_rects((w, h));
        let at = scene.overlays.len();
        scene.overlays.push((dim, back));
        scene.overlays.push((t, plane));
        Some(at..at + 2)
    }

    /// Whether "Continue" was clicked in the report since the last call.
    pub(crate) fn take_report_done(&self) -> bool {
        match self.shared.lock() {
            Ok(mut s) => std::mem::take(&mut s.report_done),
            Err(_) => false,
        }
    }

    /// Forget the clicks still on their way to a window that closed.
    pub(crate) fn clear_win_input(&self) {
        if let Ok(mut s) = self.shared.lock() {
            s.win_input.clear();
            s.report_done = false;
        }
    }

    /// Hand the window showing a click or a turn of the wheel over it.
    pub(crate) fn push_input(&self, input: crate::new_ui_add::WinInput) {
        if let Ok(mut s) = self.shared.lock() {
            s.win_input.push_back(input);
        }
    }

    /// What clicks in the "Add vehicle" window chose since the last call.
    pub(crate) fn take_add_events(&self) -> Vec<omsi_ui::ingame::AddVehicleEvent> {
        match self.shared.lock() {
            Ok(mut s) => std::mem::take(&mut s.add_events),
            Err(_) => Vec::new(),
        }
    }

    /// What clicks in the Alt bar's window asked for since the last call.
    pub(crate) fn take_tool_events(&self) -> Vec<omsi_ui::alt_windows::Event> {
        match self.shared.lock() {
            Ok(mut s) => std::mem::take(&mut s.tool_events),
            Err(_) => Vec::new(),
        }
    }

    /// Close the Alt bar's window, forgetting what was still on its way to it.
    pub(crate) fn close_tool(&mut self) {
        self.tool.close();
        if let Ok(mut s) = self.shared.lock() {
            s.tool = None;
            s.win_input.clear();
            s.tool_events.clear();
        }
    }

    /// Close the "Add vehicle" window, forgetting what was still on its way to it.
    pub(crate) fn close_add(&mut self) {
        self.add = None;
        self.add_picture = None;
        if let Ok(mut s) = self.shared.lock() {
            s.add = None;
            s.win_input.clear();
            s.add_events.clear();
            s.add_modal = false;
            s.add_picture = None;
            s.add_picture_want = None;
            s.add_open = false;
        }
    }

    /// Tell the overlay whether the "Add vehicle" window is open (each frame).
    pub(crate) fn set_add_open(&self, open: bool) {
        if let Ok(mut s) = self.shared.lock() {
            s.add_open = open;
        }
    }

    /// Whether a dropdown of the "Add vehicle" window has its list open (it takes every
    /// click, wherever it is, until it closes).
    pub(crate) fn add_modal(&self) -> bool {
        self.shared.lock().is_ok_and(|s| s.add_modal)
    }

    /// Read the picture drawn ahead of time of `bus` in `paint` (the launcher's, under the
    /// installation `root`) on a thread of its own for the "Add vehicle" window, once per
    /// vehicle and paint chosen. Returns the key the window asks the overlay for it by.
    pub(crate) fn want_add_picture(
        &mut self,
        root: &std::path::Path,
        bus: &str,
        paint: &str,
    ) -> u64 {
        let key = crate::new_ui_add::picture_key(root, bus, paint);
        if self.add_picture == Some(key) {
            return key;
        }
        self.add_picture = Some(key);
        if let Ok(mut s) = self.shared.lock() {
            s.add_picture_want = Some(key);
        }
        let shared = self.shared.clone();
        let (root, bus, paint) = (root.to_path_buf(), bus.to_string(), paint.to_string());
        let _ = std::thread::Builder::new()
            .name("add-vehicle-picture".into())
            .spawn(move || {
                let pic = crate::new_ui_add::read_picture(&root, &bus, &paint);
                // (a picture read for a vehicle or paint chosen before this one is dropped: it
                // came after the one wanted, or would take its place)
                if let Ok(mut s) = shared.lock() {
                    if s.add_picture_want == Some(key) {
                        s.add_picture = Some((key, pic));
                    }
                }
            });
        key
    }
}

impl App {
    /// Whether the new Esc menu is the one showing: the game menu is open at its top level (no
    /// list opened from it).
    pub(crate) fn new_menu_active(&self) -> bool {
        crate::new_ui_pages::showing(self.game_menu.is_some(), self.chooser.is_some())
            == crate::new_ui_pages::Showing::Top
    }

    /// Where the menus are drawn in the window now (see [`menu_area`]); none before the
    /// window has its surface.
    pub(crate) fn new_ui_area(&self) -> Option<Area> {
        let s = self.surface.as_ref()?;
        Some(menu_area(
            self.settings.triple.enabled,
            self.settings.vr_requested(),
            (s.config.width, s.config.height),
        ))
    }

    /// The id of the Esc menu's line under the cursor, where the column is drawn.
    pub(crate) fn new_menu_hit(&self) -> Option<String> {
        let area = self.new_ui_area()?;
        let at = area.local(Vec2::new(self.cursor.0, self.cursor.1));
        self.new_menu_frame().hit(at, area.size)
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
        let mirror = (!self.new_ui.installed())
            .then(|| self.d3d9_ui_mirror())
            .flatten();
        if let Some(r) = self.renderer.as_mut() {
            self.new_ui.install(r, mirror);
        }
        let menu = self.new_menu_active().then(|| self.new_menu_frame());
        // (the Esc menu hides the bar and takes the keys back)
        if self.game_menu.is_some() {
            self.new_ui.alt.focus = None;
        }
        // (the page open behind a line: made with the HUD's frame, `new_ui_pages`)
        if !self.new_pages_active() {
            self.new_ui.page = None;
        }
        self.add_events();
        self.tool_events();
        self.report_events();
        let alt = self.alt_bar_active().then(|| self.alt_frame());
        let report = self.report_frame();
        // (the report stands over everything else of the game, and pauses it)
        let (alt, add, tool) = if report.is_some() {
            (None, None, None)
        } else {
            (alt, self.add_frame(), self.tool_frame())
        };
        let area = self.new_ui_area();
        self.new_ui.set_add_open(self.new_ui.add.is_some());
        self.new_ui.set(
            menu,
            alt,
            add,
            tool,
            report,
            Vec2::new(self.cursor.0, self.cursor.1),
            area,
        );
    }

    /// A left click while the game menu is open: when the new menu is showing, a click on one of
    /// its lines does what the HUD's menu does for that line. Returns whether the new menu took
    /// the click (it takes every click while it is showing, so that none reaches the HUD's rows
    /// that are not drawn).
    pub(crate) fn new_menu_click(&mut self, event_loop: &ActiveEventLoop) -> bool {
        // a page open beside the column: a line of the column closes the page and does what it
        // does; the rest of the window is the page's, hit-tested by the HUD's code as before
        // (and an open drop-down closes on a click outside it, the column too)
        if self.new_pages_active() && self.game_menu.is_some() && self.dropdown.is_none() {
            let Some(id) = self.new_menu_hit() else {
                return false;
            };
            if let Some(k) = self.game_menu_items().iter().position(|m| m.0 == id) {
                self.close_list();
                self.menu_kbd = false;
                self.game_menu = Some(k);
                self.menu_choose(event_loop, k);
            }
            return true;
        }
        if !self.new_menu_active() || self.surface.is_none() {
            return false;
        }
        if let Some(id) = self.new_menu_hit() {
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
    fn a_new_loading_reads_its_own_bus_and_screenshots_again() {
        let mut ui = NewUi::default();
        let card = crate::new_ui_loading::CardText {
            title: "First bus".into(),
            ..Default::default()
        };
        // a first drive's loading: started, its bus read, a screenshot waiting
        ui.slides_started = true;
        ui.bus_started = true;
        {
            let mut s = ui.shared.lock().unwrap();
            s.bus_card = Some(card.clone());
            s.slides.push(crate::new_ui_loading::Decoded {
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
        crate::input::menus::GAME_MENU.to_vec()
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
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
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
        shared.lock().unwrap().loading = Some(crate::new_ui_loading::LoadingFrame {
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

    /// The pages behind the menu's lines, with placeholder data, drawn by the overlay over a
    /// plain picture into PNGs (`DEPOT_PAGES_PNG`: the folder; else they are only drawn), for
    /// looking at them next to the design. Also checks a page is drawn where `write_hits`
    /// says its rows are.
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn the_pages_behind_the_menu_draw_in_the_new_look() {
        use crate::game_lists::{row, ListKind, HEADING};
        use crate::new_ui_pages::{page_frame, test_column, test_frame, PageFrame};
        use crate::ui::{DropdownView, MenuKind, Preview};
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
        let Ok(adapter) = pollster::block_on(instance.request_adapter(&Default::default())) else {
            eprintln!("no graphics adapter: skipped");
            return;
        };
        let (device, queue) =
            pollster::block_on(adapter.request_device(&Default::default())).unwrap();
        let format = wgpu::TextureFormat::Rgba8UnormSrgb;
        let out = std::env::var("DEPOT_PAGES_PNG").ok();
        let shoot = |name: &str, (w, h): (u32, u32), page: PageFrame, mouse: Vec2| {
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
            let mut ui = crate::ui::Ui::new().unwrap();
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
        };
        let size = (1672, 941);
        let away = Vec2::new(-100.0, -100.0);
        let k = |s: &str| s.to_string();
        // (the HUD's list: (what it does, what it says))
        let refs = |v: &[(String, String)]| -> Vec<(String, String)> {
            v.iter().map(|(l, id)| (id.clone(), l.clone())).collect()
        };

        // Options: a long page of switches, a slider and a select, the keyboard on a row
        let general = refs(&[
            (row("Navigator", 's', "on", "Enables/Disables the Minimap", None), k("navigator")),
            (row("AI vehicles on the map", 's', "on", "Shows/hides the other (AI) vehicles on the Minimap and the city map", None), k("nav_ai")),
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
        let tabs = vec![
            k("General"),
            k("Graphics"),
            k("Display"),
            k("Sound"),
            k("Camera"),
            k("Driving"),
            k("Advanced"),
        ];
        let items: Vec<(&str, &str)> = general
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        let mut f = test_frame(&items, MenuKind::Options);
        f.menu = Some((4, &items));
        f.menu_tabs = Some((tabs.clone(), 0));
        let p = page_frame(&f, Some(&ListKind::Options(0)), test_column(true)).unwrap();
        shoot("options-general", size, p.clone(), away);
        shoot("options-general-1280x720", (1280, 720), p.clone(), away);
        shoot(
            "options-general-1920x1080-hover",
            (1920, 1080),
            p,
            Vec2::new(900.0, 330.0),
        );

        // Graphics: sliders, a stepper, a select with its drop-down open
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
        f.menu_tabs = Some((tabs.clone(), 1));
        let names = ["Vanilla", "Vanilla+", "Enhanced", "Ultra", "Custom"];
        f.dropdown = Some(DropdownView {
            row: 0,
            items: names.to_vec(),
            sel: 2,
            top: 0,
            current: Some(1),
        });
        let p = page_frame(&f, Some(&ListKind::Options(1)), test_column(true)).unwrap();
        shoot("options-graphics-dropdown", size, p, away);
        f.dropdown = None;
        f.menu = Some((3, &items));
        let p = page_frame(&f, Some(&ListKind::Options(1)), test_column(true)).unwrap();
        shoot("options-graphics", size, p, away);

        // Controls, and the keyboard's bindings (one being changed)
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
        shoot(
            "controls",
            size,
            page_frame(&f, Some(&ListKind::Controls), test_column(true)).unwrap(),
            away,
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
        shoot(
            "controls-keyboard",
            size,
            page_frame(&f, Some(&ListKind::Keyboard(0)), test_column(true)).unwrap(),
            away,
        );

        // a game controller: headings, information, switches, sliders
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
        shoot(
            "controls-controllers",
            size,
            page_frame(&f, Some(&ListKind::ControllerDevices(0)), test_column(true)).unwrap(),
            away,
        );

        // Vehicle options, World options (a time being typed)
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
        shoot(
            "vehicle-options",
            size,
            page_frame(&f, Some(&ListKind::Vehicle(0)), test_column(true)).unwrap(),
            away,
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
        shoot(
            "world-options",
            size,
            page_frame(&f, Some(&ListKind::World(0)), test_column(true)).unwrap(),
            away,
        );

        // Line and tour: the lines with the tours of the chosen one beside them
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
        shoot(
            "line-and-tour",
            size,
            page_frame(&f, Some(&ListKind::Lines), test_column(true)).unwrap(),
            away,
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
        shoot(
            "line-and-tour-tours",
            size,
            page_frame(&f, Some(&ListKind::Tours(k("50"), None)), test_column(true)).unwrap(),
            away,
        );

        // a long list of codes and names (the destinations), scrolled, the way back under it
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
        shoot(
            "vehicle-destinations",
            size,
            page_frame(&f, Some(&ListKind::Destinations), test_column(true)).unwrap(),
            away,
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
        shoot(
            "vehicle-drivers-hover",
            size,
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
        shoot(
            "vehicle-place",
            size,
            page_frame(&f, Some(&ListKind::PlaceMaker), test_column(true)).unwrap(),
            away,
        );
    }

    /// The Esc menu and a page on the centre screen of a triple screen (the renderer's own
    /// three panels, the overlay drawn once they are in the window), and drawn into the
    /// headset's picture of them (`NewUi::headset_surface`, the texture laid over the window
    /// here as the menu plane shows it), into PNGs (`DEPOT_PAGES_PNG`: the folder; else they
    /// are only drawn). Also checks they are where the clicks are taken.
    #[test]
    #[ignore = "requires a graphics adapter; run with --ignored on a GPU host"]
    fn the_menus_draw_on_the_centre_screen_and_into_the_headset_s_picture() {
        use crate::game_lists::{row, ListKind};
        use crate::new_ui_pages::{page_frame, test_column, test_frame};
        use crate::ui::MenuKind;
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
        let report = crate::new_ui_report::ReportFrame::for_test();
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
        let mut hud = crate::ui::Ui::new().unwrap();
        page.write_hits(&mut hud, area);
        for rect in hud.menu_rects.iter().filter(|r| r[0] > 0.0) {
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
        let mut hud = crate::ui::Ui::new().unwrap();
        page.write_hits(&mut hud, area);
        for rect in hud.menu_rects.iter().filter(|r| r[0] > 0.0) {
            let (x0, x1) = (rect[0] as u32, rect[2] as u32);
            assert!(apart(&vr_page, &plain, size.0, x0, x1) > 0, "{rect:?}");
        }
    }
}
