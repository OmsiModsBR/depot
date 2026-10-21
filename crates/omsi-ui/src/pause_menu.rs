//! The pause menu: a column of sections on the left and, to its right, the city map under a
//! bar that says what is paused. The caller supplies the items (what the menu offers is the
//! game's business) and draws the map into the rectangle `draw` returns; nothing here knows
//! a game state.

use crate::paint::Align;
use crate::theme::v5::{BAR, DANGER, FG, LINE, MUTED, SIDE, TS};
use crate::theme::{accent, on_accent};
use crate::widgets::{id_of, Ui};
use crate::{Color, Rect, Weight};

pub const COLUMN_W: f32 = 340.0;
pub const BAR_H: f32 = 45.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Normal,
    /// The way back into the game: the accent row.
    Primary,
    /// Leaves the session: red.
    Danger,
    /// The item of the page being shown (the city map while it is open).
    Current,
}

pub struct Item<'a> {
    /// What `draw` returns when the item is clicked.
    pub id: &'a str,
    pub icon: &'a str,
    pub label: &'a str,
    /// The key that does the same ("Esc", "Shift+M"), or empty.
    pub key: &'a str,
    pub kind: Kind,
    /// Why the item cannot be used now; it is drawn dim and the reason shows on hover.
    pub disabled: Option<&'a str>,
}

pub struct Section<'a> {
    pub title: &'a str,
    pub items: Vec<Item<'a>>,
}

pub struct View<'a> {
    pub logo: Option<(usize, f32)>,
    pub sections: &'a [Section<'a>],
    /// "City map", "Line 50 · Example route", "08:42"
    pub title: &'a str,
    pub subtitle: &'a str,
    pub clock: &'a str,
    /// "PAUSED"
    pub state: &'a str,
    /// The item the keyboard is on: lit as under a mouse (the caller moves it with the keys).
    pub selected: Option<&'a str>,
}

pub struct Output<'a> {
    /// The item clicked this frame.
    pub clicked: Option<&'a str>,
    /// Where the map goes (under the bar, right of the column).
    pub map: Rect,
}

fn text(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) {
    let px = px * TS;
    let w = ui.width(s, px, weight);
    ui.text_in(
        s,
        Rect::new(x, cy - px, w + 2.0, px * 2.0),
        px,
        weight,
        c,
        Align::Left,
    );
}

fn right(ui: &mut Ui, s: &str, x: f32, cy: f32, px: f32, weight: Weight, c: Color) {
    let px = px * TS;
    let w = ui.width(s, px, weight);
    ui.text_in(
        s,
        Rect::new(x - w - 2.0, cy - px, w + 2.0, px * 2.0),
        px,
        weight,
        c,
        Align::Right,
    );
}

/// Row height for a window of `h` points: the column's items shrink to fit a small window.
fn row_height(view: &View, h: f32) -> f32 {
    let items: usize = view.sections.iter().map(|s| s.items.len()).sum();
    let heads = view.sections.len() as f32 * 31.0;
    ((h - 142.0 - heads - 24.0) / items.max(1) as f32).clamp(26.0, 37.0)
}

/// Where one item sits (in the points `draw` works in).
pub struct Row<'a> {
    pub id: &'a str,
    pub rect: Rect,
    /// Whether it can be clicked.
    pub enabled: bool,
}

/// The items' rectangles for a window of `size` points, in the order `draw` lays them out.
pub fn rows<'a>(view: &'a View<'a>, size: glam::Vec2) -> Vec<Row<'a>> {
    let step = row_height(view, size.y);
    let mut head = 142.0;
    let mut out = Vec::new();
    for section in view.sections {
        let mut top = head + 13.0;
        for item in &section.items {
            out.push(Row {
                id: item.id,
                rect: Rect::new(10.0, top, COLUMN_W - 21.0, step),
                enabled: item.disabled.is_none(),
            });
            top += step;
        }
        head = top + 18.0;
    }
    out
}

/// The item under `point` (in points) that can be clicked, if any.
pub fn hit<'a>(view: &'a View<'a>, size: glam::Vec2, point: glam::Vec2) -> Option<&'a str> {
    rows(view, size)
        .into_iter()
        .find(|r| r.enabled && r.rect.contains(point))
        .map(|r| r.id)
}

/// Draw the menu column and the map's bar over the whole window.
pub fn draw<'a>(ui: &mut Ui, view: &'a View<'a>) -> Output<'a> {
    let (w, h) = (ui.size.x, ui.size.y);
    let col = Rect::new(0.0, 0.0, COLUMN_W, h);
    ui.solid(col);
    ui.p().rect(col, SIDE.alpha(0.96));
    ui.p().rect(Rect::new(COLUMN_W - 1.0, 0.0, 1.0, h), LINE);
    if let Some((tex, aspect)) = view.logo {
        ui.image(Rect::new(22.0, 22.0, 220.0, 220.0 * aspect), tex, 0.0);
    }
    text(
        ui,
        &crate::tr("Game menu").to_uppercase(),
        22.0,
        109.0,
        12.0,
        Weight::Regular,
        MUTED,
    );
    let step = row_height(view, h);
    let mut head = 142.0;
    let mut clicked = None;
    for section in view.sections {
        text(
            ui,
            &section.title.to_uppercase(),
            22.0,
            head,
            11.0,
            Weight::Regular,
            MUTED,
        );
        let mut top = head + 13.0;
        for item in &section.items {
            let r = Rect::new(10.0, top, COLUMN_W - 21.0, step);
            let id = id_of(&format!("pause:{}", item.id));
            let (hover, _, click) = ui.interact(id, r);
            let enabled = item.disabled.is_none();
            match item.kind {
                Kind::Primary => ui.p().rect(r, accent()),
                Kind::Current => {
                    ui.p().rect(r, Color::hex(0x2a2518));
                    ui.p().rect(Rect::new(r.x, r.y, 3.0, r.h), accent());
                }
                _ if (hover || view.selected == Some(item.id)) && enabled => {
                    ui.p().rect(r, Color::WHITE.alpha(0.06))
                }
                _ => {}
            }
            let (label_c, icon_c) = match item.kind {
                Kind::Primary => (on_accent(), on_accent()),
                Kind::Danger => (DANGER, DANGER),
                _ => (FG, MUTED),
            };
            let dim = if enabled { 1.0 } else { 0.4 };
            let cy = r.y + r.h * 0.5;
            ui.icon(
                item.icon,
                glam::Vec2::new(45.0, cy),
                14.0,
                icon_c.alpha(dim),
            );
            let weight = if item.kind == Kind::Primary {
                Weight::Bold
            } else {
                Weight::Regular
            };
            text(ui, item.label, 69.0, cy, 15.0, weight, label_c.alpha(dim));
            if !item.key.is_empty() {
                let key_c = if item.kind == Kind::Primary {
                    on_accent()
                } else {
                    MUTED
                };
                right(
                    ui,
                    item.key,
                    r.right() - 21.0,
                    cy,
                    11.0,
                    weight,
                    key_c.alpha(dim),
                );
            }
            if let Some(why) = item.disabled {
                ui.tooltip(r, why);
            } else if click {
                clicked = Some(item.id);
            }
            top += step;
        }
        head = top + 18.0;
    }
    // the bar over the map
    let bar = Rect::new(COLUMN_W, 0.0, w - COLUMN_W, BAR_H);
    ui.p().rect(bar, BAR);
    text(
        ui,
        view.title,
        COLUMN_W + 20.0,
        BAR_H * 0.5,
        14.0,
        Weight::Bold,
        FG,
    );
    let tw = ui.width(view.title, 14.0 * TS, Weight::Bold);
    let x = COLUMN_W + 20.0 + tw + 24.0;
    text(
        ui,
        view.subtitle,
        x,
        BAR_H * 0.5,
        14.0,
        Weight::Regular,
        MUTED,
    );
    let sw = ui.width(view.subtitle, 14.0 * TS, Weight::Regular);
    text(
        ui,
        view.clock,
        x + sw + 24.0,
        BAR_H * 0.5,
        14.0,
        Weight::Regular,
        MUTED,
    );
    right(
        ui,
        &crate::tr(view.state).to_uppercase(),
        w - 24.0,
        BAR_H * 0.5,
        13.0,
        Weight::Medium,
        accent(),
    );
    Output {
        clicked,
        map: Rect::new(COLUMN_W, BAR_H, w - COLUMN_W, h - BAR_H),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use glam::Vec2;

    fn sections() -> Vec<Section<'static>> {
        let item = |id, kind, disabled| Item {
            id,
            icon: "settings",
            label: id,
            key: "",
            kind,
            disabled,
        };
        vec![
            Section {
                title: "Session",
                items: vec![
                    item("resume", Kind::Primary, None),
                    item("skip", Kind::Normal, Some("no next stop")),
                ],
            },
            Section {
                title: "Game",
                items: vec![item("quit", Kind::Danger, None)],
            },
        ]
    }

    fn frame(ui: &mut Ui, secs: &[Section], click: Option<Vec2>) -> Option<String> {
        let view = View {
            logo: None,
            sections: secs,
            title: "City map",
            subtitle: "Line 50",
            clock: "08:42",
            state: "paused",
            selected: None,
        };
        let run = |ui: &mut Ui| {
            ui.begin(Vec2::new(1672.0, 941.0), 1.0, 0.016);
            let out = draw(ui, &view).clicked.map(String::from);
            ui.finish();
            out
        };
        if let Some(p) = click {
            ui.input.mouse = p;
            ui.input.pressed = true;
            ui.input.down = true;
            run(ui);
            ui.input.released = true;
            ui.input.down = false;
        }
        run(ui)
    }

    #[test]
    fn a_click_returns_the_item_and_a_disabled_item_returns_nothing() {
        let secs = sections();
        let mut ui = Ui::new();
        frame(&mut ui, &secs, None);
        let at = |ui: &Ui, id: &str| {
            let r = ui.drawn[&id_of(&format!("pause:{id}"))];
            Vec2::new(r.x + 20.0, r.y + r.h * 0.5)
        };
        let resume = at(&ui, "resume");
        assert_eq!(
            frame(&mut ui, &secs, Some(resume)).as_deref(),
            Some("resume")
        );
        let skip = at(&ui, "skip");
        assert_eq!(frame(&mut ui, &secs, Some(skip)), None);
        let quit = at(&ui, "quit");
        assert_eq!(frame(&mut ui, &secs, Some(quit)).as_deref(), Some("quit"));
    }

    #[test]
    fn the_rows_are_where_draw_puts_them_and_hit_finds_the_clickable_ones() {
        let secs = sections();
        let size = Vec2::new(1672.0, 941.0);
        let view = View {
            logo: None,
            sections: &secs,
            title: "",
            subtitle: "",
            clock: "",
            state: "",
            selected: Some("skip"),
        };
        let mut ui = Ui::new();
        ui.begin(size, 1.0, 0.016);
        draw(&mut ui, &view);
        ui.finish();
        let rows = rows(&view, size);
        assert_eq!(rows.len(), 3);
        for r in &rows {
            assert_eq!(
                ui.drawn[&id_of(&format!("pause:{}", r.id))],
                r.rect,
                "{}",
                r.id
            );
        }
        let mid = |r: &Row| Vec2::new(r.rect.x + 20.0, r.rect.y + r.rect.h * 0.5);
        assert_eq!(hit(&view, size, mid(&rows[0])), Some("resume"));
        // (a disabled item and the space between items are not hits)
        assert_eq!(hit(&view, size, mid(&rows[1])), None);
        assert_eq!(hit(&view, size, Vec2::new(500.0, 500.0)), None);
        assert_eq!(hit(&view, size, mid(&rows[2])), Some("quit"));
    }

    #[test]
    fn the_rows_shrink_to_keep_every_item_inside_a_small_window() {
        let secs = sections();
        let view = View {
            logo: None,
            sections: &secs,
            title: "",
            subtitle: "",
            clock: "",
            state: "",
            selected: None,
        };
        assert!(row_height(&view, 941.0) <= 37.0);
        assert!(row_height(&view, 300.0) >= 26.0);
    }
}
