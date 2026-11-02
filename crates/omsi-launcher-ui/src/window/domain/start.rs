//! What the Maps page's start panel keeps between frames (`StartView`), the windows it opens
//! over the page (`Popup`), and how a typed date, time and start point search are read.

use omsi_ui::widgets::matches;

/// A window over the page.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Popup {
    StartPoint,
    Weather,
    Explore,
}

#[derive(Default)]
pub struct StartView {
    pub popup: Option<Popup>,
    /// What the window changes, as it was when it opened (Cancel puts it back): the start
    /// point and the weather.
    pub(crate) undo: Option<(i32, String)>,
    /// The start points' search.
    pub(crate) search: String,
    /// The weather window shows the choice (a weather chosen, or "Choose" clicked).
    pub(crate) choose: bool,
    /// The date and the time as typed (put back to the choice's when it changes elsewhere).
    pub(crate) date_text: String,
    pub(crate) time_text: String,
}

/// A typed date (`14/06/2024`, `14.6.2024`, or `2024-06-14`) as `2024-06-14`; None when it is
/// no date.
pub fn date_typed(text: &str) -> Option<String> {
    let t = text.trim();
    let parts: Vec<&str> = t.split(['/', '.', '-']).map(str::trim).collect();
    if parts.len() != 3 {
        return None;
    }
    let n: Vec<i32> = parts
        .iter()
        .map(|p| p.parse().ok())
        .collect::<Option<_>>()?;
    let (y, m, d) = if parts[0].len() == 4 {
        (n[0], n[1], n[2])
    } else {
        (n[2], n[1], n[0])
    };
    let days = match m {
        2 if (y % 4 == 0 && y % 100 != 0) || y % 400 == 0 => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        1..=12 => 31,
        _ => return None,
    };
    ((1000..=9999).contains(&y) && (1..=days).contains(&d)).then(|| format!("{y:04}-{m:02}-{d:02}"))
}

/// A typed time (`08:30`, `8.30`, `830`, `8`) as minutes of the day; None when it is no time.
pub fn time_typed(text: &str) -> Option<i32> {
    let t = text.trim();
    let (h, m) = match t.split_once([':', '.', 'h']) {
        Some((h, m)) => (
            h.trim().parse::<i32>().ok()?,
            if m.trim().is_empty() {
                0
            } else {
                m.trim().parse::<i32>().ok()?
            },
        ),
        None if t.len() >= 3 && t.chars().all(|c| c.is_ascii_digit()) => (
            t[..t.len() - 2].parse().ok()?,
            t[t.len() - 2..].parse().ok()?,
        ),
        None => (t.parse().ok()?, 0),
    };
    ((0..24).contains(&h) && (0..60).contains(&m)).then_some(h * 60 + m)
}

/// Whether a start point's name shows for the typed search (case does not matter).
pub fn entry_matches(name: &str, search: &str) -> bool {
    let q = search.trim().to_lowercase();
    q.is_empty() || matches(name, &q)
}
