//! The Esc menu's column drawn with `omsi_ui::pause_menu`, its clicks, and the game's side
//! of it (the `impl App` part).

use super::super::domain::menu::{
    menu_area, menu_frame, step_displayed, Area, MenuFrame, DESIGN, MIN_SCALE,
};
use super::super::interface::output::App;
use glam::Vec2;
use omsi_ui::pause_menu::{self, Item, Kind, Section, View};
use winit::event_loop::ActiveEventLoop;

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

impl App {
    /// Whether the new Esc menu is the one showing: the game menu is open at its top level (no
    /// list opened from it).
    pub(crate) fn new_menu_active(&self) -> bool {
        crate::ui::showing(self.game_menu.is_some(), self.chooser.is_some())
            == crate::ui::Showing::Top
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
