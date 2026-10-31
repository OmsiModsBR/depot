//! The Tutorials page: OMSI 2's lessons, one card each, started from the card.

use super::super::domain::frame::Frame;
use super::super::interface::output::Page;
use super::heading::page_title;
use glam::Vec2;
use omsi_ui::paint::Align;
use omsi_ui::theme::{GAP, TEXT, TEXT_DIM, TEXT_FAINT};
use omsi_ui::widgets::ButtonKind;
use omsi_ui::{Rect, Weight};

/// OMSI's four tutorials: what each teaches, and a button to start it.
pub fn tutorials(f: &mut Frame, area: Rect) {
    let body = page_title(f.ui, area, Page::Tutorials);
    static LIST: std::sync::OnceLock<Vec<(usize, String, String)>> = std::sync::OnceLock::new();
    let list = LIST.get_or_init(omsi_launcher_lib::tutorials);
    if list.is_empty() {
        // (a card, as Sessions says it has none)
        let r = Rect::new(body.x, body.y, body.w.min(720.0), 120.0);
        f.ui.panel(r);
        f.ui.icon(
            "help",
            Vec2::new(r.x + 40.0, r.center().y),
            36.0,
            TEXT_FAINT,
        );
        f.ui.paragraph(
            "No tutorials were found in the OMSI 2 folder (Tutorials).",
            Vec2::new(r.x + 76.0, r.y + 52.0),
            r.w - 100.0,
            13.5,
            Weight::Regular,
            TEXT_DIM,
        );
        return;
    }
    let cols = 2;
    let cw = (body.w - GAP * 2.0) / cols as f32;
    let ch = ((body.h - GAP * 2.0) / 2.0).min(330.0);
    let mut start = None;
    for (k, (n, title, text)) in list.iter().enumerate() {
        let r = Rect::new(
            body.x + (k % cols) as f32 * (cw + GAP * 2.0),
            body.y + (k / cols) as f32 * (ch + GAP * 2.0),
            cw,
            ch,
        );
        f.ui.panel(r);
        f.ui.text_in(
            title,
            Rect::new(r.x + 18.0, r.y + 14.0, r.w - 36.0, 26.0),
            17.0,
            Weight::Bold,
            TEXT,
            Align::Left,
        );
        // (scrolled: a long lesson text ran past the card and was cut off)
        f.ui.scroll_area(
            &format!("tut-text-{n}"),
            Rect::new(r.x + 18.0, r.y + 46.0, r.w - 36.0, r.h - 110.0),
            &mut |ui, v| {
                ui.paragraph(
                    text,
                    Vec2::new(v.x, v.y),
                    v.w - 12.0,
                    12.5,
                    Weight::Regular,
                    TEXT_DIM,
                )
            },
        );
        if f.ui.button(
            &format!("tut-{n}"),
            Rect::new(r.x + 18.0, r.bottom() - 58.0, 200.0, 40.0),
            "Start the lesson",
            Some("play_arrow"),
            ButtonKind::Primary,
        ) {
            start = Some(*n);
        }
    }
    if let Some(n) = start {
        f.state.launch_tutorial(n);
    }
}
