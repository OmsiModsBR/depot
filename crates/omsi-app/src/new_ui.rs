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
pub const LOGO_WIDTH: u32 = 640;

/// The wordmark ready for the GPU: shrunk, and its black field made see-through, so that over
/// the loading screen's photos and the dimmed world it stands with no black box round it.
pub fn logo_rgba() -> Option<image::RgbaImage> {
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
pub fn scale_for(w: u32, h: u32) -> f32 {
    (w as f32 / DESIGN.0)
        .min(h as f32 / DESIGN.1)
        .max(MIN_SCALE)
}

/// Where the new interface is drawn in the window, in pixels: the whole window, or the
/// centre screen of a triple screen.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Area {
    /// Its top-left corner in the window.
    pub origin: Vec2,
    pub size: (u32, u32),
}

impl Area {
    /// The whole of a window of `size` pixels.
    pub fn whole(size: (u32, u32)) -> Area {
        Area {
            origin: Vec2::ZERO,
            size,
        }
    }

    /// Window pixels `at` as pixels of the area.
    pub fn local(&self, at: Vec2) -> Vec2 {
        at - self.origin
    }

    /// The area where it lies inside a picture of `size` pixels; else (another picture) the
    /// whole of it.
    pub fn within(self, size: (u32, u32)) -> Area {
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
pub fn menu_area(triple: bool, vr: bool, size: (u32, u32)) -> Area {
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
pub fn place_layers(layers: &mut [omsi_ui::Layer], area: Area, scale: f32, target: (u32, u32)) {
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
pub fn headset_rects(size: (u32, u32)) -> [[f32; 4]; 2] {
    let all = [0.0, 0.0, size.0 as f32, size.1 as f32];
    [all, all]
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedItem {
    pub id: String,
    icon: &'static str,
    label: String,
    key: &'static str,
    pub kind: Kind,
    disabled: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OwnedSection {
    title: &'static str,
    pub items: Vec<OwnedItem>,
}

/// What one frame of the menu shows, owned so that it can cross to the renderer.
#[derive(Clone, Debug, PartialEq)]
pub struct MenuFrame {
    pub sections: Vec<OwnedSection>,
    pub title: String,
    pub subtitle: String,
    clock: String,
    pub state: &'static str,
    pub selected: Option<String>,
}

/// Where a game menu line goes in the column, its icon, the key that does the same, and how it
/// is drawn. A line not listed here is shown under "Game" with a plain icon, so that a line
/// added to the game's menu is never lost.
pub fn place(id: &str) -> (usize, &'static str, &'static str, Kind) {
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
pub fn menu_frame(
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
pub fn display_order(items: &[(&str, &str)]) -> Vec<usize> {
    let mut order: Vec<usize> = (0..items.len()).collect();
    order.sort_by_key(|&k| place(items[k].0).0);
    order
}

/// The line after (or before) `from` in the displayed order, skipping greyed-out ones.
pub fn step_displayed(
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
    pub fn with_view<R>(&self, f: impl FnOnce(&View) -> R, logo: Option<(usize, f32)>) -> R {
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

    /// Pixels per design point for this menu in a window of `size` pixels: the usual one, or
    /// less when the lines would run off the bottom of a short window.
    pub fn scale(&self, size: (u32, u32)) -> f32 {
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
    pub fn hit(&self, px: Vec2, size: (u32, u32)) -> Option<String> {
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
pub struct State {
    pub menu: Option<MenuFrame>,
    /// A page behind one of the menu's lines, while one is open.
    pub page: Option<crate::new_ui_pages::PageFrame>,
    alt: Option<crate::ui::AltFrame>,
    /// The "Add vehicle" window.
    add: Option<crate::ui::AddFrame>,
    /// The end-of-trip report.
    report: Option<crate::ui::ReportFrame>,
    /// "Continue" was clicked in the report.
    report_done: bool,
    /// Clicks and the wheel over the window showing, not yet handed to it (one a frame).
    win_input: std::collections::VecDeque<crate::ui::WinInput>,
    /// What clicks in that window chose, for the game to apply.
    add_events: Vec<omsi_ui::ingame::AddVehicleEvent>,
    /// A dropdown of that window has its list open: every click goes to the window then.
    add_modal: bool,
    /// The picture of the vehicle and paint chosen there, read on its thread and not yet in
    /// the GPU (none inside: no picture was drawn of them).
    add_picture: Option<(u64, Option<crate::ui::Picture>)>,
    /// The picture the window wants now: a picture read for one chosen before is dropped.
    add_picture_want: Option<u64>,
    /// The window is open (also while the Esc menu or the report hides it: its picture stays
    /// in the GPU then).
    add_open: bool,
    /// The live 3D view's latest frame, not yet in the GPU: which vehicle and paint it shows
    /// (`preview::tag`), its size and RGBA.
    add_live: Option<(u64, u32, u32, Vec<u8>)>,
    /// The Alt bar's window open (reposition, date and time, weather...).
    tool: Option<crate::ui::ToolFrame>,
    /// What clicks in it asked for, for the game to apply.
    tool_events: Vec<omsi_ui::alt_windows::Event>,
    /// The loading screen, while the first area of the map loads.
    pub loading: Option<crate::ui::LoadingFrame>,
    /// Screenshots decoded on their thread and not yet in the GPU.
    pub slides: Vec<crate::ui::Decoded>,
    /// The bus's block of the loading card, once its file has been read.
    pub bus_card: Option<crate::ui::CardText>,
    /// The map's own picture for its block of the card, decoded and not yet in the GPU.
    map_picture: Option<crate::ui::Decoded>,
    /// Which loading the screenshots and the bus card being read belong to (one more each
    /// time a loading is over).
    pub loading_gen: u64,
    /// The cursor, in window pixels.
    pub mouse: Vec2,
    /// Where in the window the interface is drawn (none: the whole picture).
    area: Option<Area>,
}

pub type Shared = Arc<Mutex<State>>;

/// The overlay's own state, owned by the closure the renderer calls.
pub struct Overlay {
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
    pub slide_tex: Vec<omsi_ui::loading::Slide>,
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
    /// The live 3D view's frame in the GPU: which vehicle and paint it shows, and its texture.
    add_live: Option<(u64, usize)>,
}

impl Overlay {
    pub fn new(
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
            add_live: None,
        }
    }

    pub fn draw(
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
            (add_picture, add_open, add_live),
        ) = match self.shared.lock() {
            Ok(mut s) => {
                let arrived = std::mem::take(&mut s.slides);
                let add_picture = s.add_picture.take();
                let add_live = s.add_live.take();
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
                    (add_picture, s.add_open, add_live),
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
        // the live 3D view's frame, into the texture it had (out of the GPU once the view is
        // off or the window closed; not while the window is only hidden)
        if !add_open || add.as_ref().is_some_and(|a| !a.live_on()) {
            if let Some((_, tex)) = self.add_live.take() {
                gpu.free(tex);
            }
        }
        if let Some((tag, fw, fh, px)) = add_live {
            let tex = match self.add_live {
                Some((_, tex)) => {
                    gpu.update_image(device, queue, tex, fw, fh, &px);
                    tex
                }
                None => gpu.add_image(device, queue, fw, fh, &px),
            };
            self.add_live = Some((tag, tex));
        }
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
            Some(crate::ui::WinInput::Press) => {
                self.ui.input.pressed = true;
                self.ui.input.down = true;
            }
            Some(crate::ui::WinInput::Release) => {
                self.ui.input.released = true;
                self.ui.input.down = false;
            }
            Some(crate::ui::WinInput::Wheel(y)) => self.ui.input.wheel = Vec2::new(0.0, y),
            Some(crate::ui::WinInput::Lost) => self.ui.input.down = false,
            None if add.is_none() && tool.is_none() && report.is_none() => {
                self.ui.input.down = false
            }
            None => {}
        }
        self.ui.begin(size, scale, dt);
        self.ui.input.mouse = area.local(mouse) / scale;
        if let Some(l) = loading {
            for d in arrived.into_iter().take(crate::ui::MAX_SLIDES) {
                if self.slide_tex.len() >= crate::ui::MAX_SLIDES {
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
                c: &'a Option<crate::ui::CardText>,
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
                let preview = add.preview(self.add_picture, self.add_live);
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
pub struct NewUi {
    pub shared: Shared,
    installed: bool,
    pub slides_started: bool,
    pub bus_started: bool,
    map_picture_started: bool,
    pub(crate) alt: crate::ui::AltState,
    /// The "Add vehicle" window, while it is open.
    pub(crate) add: Option<crate::ui::AddWindow>,
    /// Keys the "Add vehicle" window's registration field took: their release is its too.
    pub(crate) add_taken: std::collections::HashSet<winit::keyboard::KeyCode>,
    /// The vehicle and paint whose picture the "Add vehicle" window asked for last.
    add_picture: Option<u64>,
    /// The manufacturer and the vehicle file chosen last in that window: it opens on them.
    pub(crate) add_last: Option<(String, String)>,
    /// That window's live 3D view, while the player has it on.
    pub(crate) add_live: Option<crate::ui::LiveView>,
    /// The page open behind a line of the Esc menu, as the HUD's frame of this frame made it.
    pub page: Option<crate::new_ui_pages::PageFrame>,
    /// The headset's picture of the menu and the page (a render texture of the window's size,
    /// and its size), and the plate that dims the view behind them.
    #[cfg_attr(not(windows), allow(dead_code))]
    headset: Option<(omsi_render::TextureId, u32, u32)>,
    #[cfg_attr(not(windows), allow(dead_code))]
    headset_dim: Option<omsi_render::TextureId>,
    /// The Alt bar's other windows.
    pub(crate) tool: crate::ui::Tools,
    /// The end-of-trip report, while it is open.
    pub(crate) report: Option<crate::ui::Report>,
    /// The trip whose report was shown last (line, tour, trip in the tour): it opens once.
    pub(crate) report_shown: Option<(String, String, usize)>,
    /// A trip at its last stop whose report waits for the doors to open there: its key, its
    /// place in the duty and where that stop is.
    pub(crate) report_watch: Option<crate::ui::Watch>,
}

impl NewUi {
    pub(crate) fn installed(&self) -> bool {
        self.installed
    }

    /// Hand the renderer the overlay (once), with the mirror its draws also go to.
    pub fn install(
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
    pub(crate) fn set_loading(&self, frame: Option<crate::ui::LoadingFrame>) {
        if let Ok(mut s) = self.shared.lock() {
            s.loading = frame;
        }
    }

    /// The loading screen is over, or a new drive's is about to start: it is taken down, and
    /// the next one lists its own map's screenshots and reads its own bus. What a thread of
    /// the last one still hands over is dropped (it belongs to another loading).
    pub fn reset_loading(&mut self) {
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
    pub fn loading_gen(&self) -> u64 {
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
                let paths = crate::ui::recent(
                    omsi_launcher_lib::screenshots::for_map(&map),
                    crate::ui::MAX_SLIDES,
                );
                for p in paths {
                    if let Some(d) = crate::ui::decode(&p) {
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
                if let Some(d) = crate::ui::map_picture(&root, &map) {
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
                let card = crate::ui::bus_card(&v, &folder, paint.as_deref());
                match shared.lock() {
                    Ok(mut s) if s.loading_gen == gen => s.bus_card = Some(card),
                    _ => {}
                }
            });
    }

    /// The bus's block of the loading card, when its file has been read.
    pub fn bus_card(&self) -> Option<crate::ui::CardText> {
        self.shared.lock().ok().and_then(|s| s.bus_card.clone())
    }

    pub fn set(
        &self,
        menu: Option<MenuFrame>,
        alt: Option<crate::ui::AltFrame>,
        add: Option<crate::ui::AddFrame>,
        tool: Option<crate::ui::ToolFrame>,
        report: Option<crate::ui::ReportFrame>,
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
    pub fn headset_surface(
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
    pub(crate) fn push_input(&self, input: crate::ui::WinInput) {
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
        self.stop_add_live();
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

    /// Turn the "Add vehicle" window's live 3D view off: its helper ends (by itself, its
    /// input closed) and its last frame is not shown any more.
    pub(crate) fn stop_add_live(&mut self) {
        self.add_live = None;
        if let Ok(mut s) = self.shared.lock() {
            s.add_live = None;
        }
    }

    /// The live 3D view's latest frame, for the overlay.
    pub(crate) fn offer_add_live(&self, frame: (u64, u32, u32, Vec<u8>)) {
        if let Ok(mut s) = self.shared.lock() {
            s.add_live = Some(frame);
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
        let key = crate::ui::picture_key(root, bus, paint);
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
                let pic = crate::ui::read_picture(&root, &bus, &paint);
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
        self.add_live_update();
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
