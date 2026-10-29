//! The new interface in the game.
//!
//! Today this is the Esc menu: while the game menu is open at its top level, the column of
//! sections from `omsi_ui::pause_menu` is drawn over the window's picture through the
//! renderer's overlay hook (`Renderer::set_overlay_hook`), in place of the HUD's lines. The
//! menu stays the game's own: its lines come from `game_menu_items`, a click does what
//! `menu_choose` does for the same line, the keys are the HUD menu's, and the simulation
//! pauses as it did. What is behind a line ("Options...", "Controls...", the lists) is drawn
//! here too, beside the column: see `ui::domain::pages`.
//!
//! Every picture has them: on a triple screen they stand on the centre screen (the hook is
//! called once the three panels are in the window, and draws in that screen's [`Area`]), and
//! in the headset they are drawn into a texture of the window's size that the menu plane
//! shows where the HUD's menu was (`NewUi::headset_surface`).
//!
//! The overlay runs inside the renderer with no access to the game, so the game hands it the
//! frame to draw through [`Shared`] before each render.

use super::super::domain::menu::{scale_for, Area, MenuFrame};
use super::super::domain::overlay::{NewUi, Shared};
use super::super::interface::output::App;
use super::menu::{headset_rects, place_layers};
use glam::Vec2;
use omsi_ui::pause_menu;
use omsi_ui::widgets::Ui;
use omsi_ui::{Draw, Gpu};
use std::time::Instant;

const LOGO: &[u8] = include_bytes!("../../../../../assets/logos/wordmark-on-dark.png");

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
    /// Before each render: give the renderer the overlay and the frame it should draw.
    pub(crate) fn new_ui_update(&mut self) {
        let mirror = (!self.new_ui.installed())
            .then(|| self.output.ui_mirror())
            .flatten();
        if let Some(r) = self.renderer.as_mut() {
            self.new_ui.install(r, mirror);
        }
        let menu = self.new_menu_active().then(|| self.new_menu_frame());
        // (the Esc menu hides the bar and takes the keys back)
        if self.game_menu.is_some() {
            self.new_ui.alt.focus = None;
        }
        // (the page open behind a line: made with the HUD's frame, `ui::domain::pages`)
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
}
