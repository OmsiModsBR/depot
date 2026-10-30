//! A departure as the editor shows and reads it: "h:mm", or "h:mm:ss" when it has seconds.

/// "h:mm" (or "h:mm:ss") from minutes after midnight.
pub fn fmt_time(min: f32) -> String {
    let s = (min as f64 * 60.0).round() as i64;
    if s % 60 == 0 {
        format!("{}:{:02}", s / 3600, s / 60 % 60)
    } else {
        format!("{}:{:02}:{:02}", s / 3600, s / 60 % 60, s % 60)
    }
}

/// Minutes after midnight from "h:mm", "h:mm:ss" or "h.mm" (after midnight as 24:10 and on).
pub fn parse_time(t: &str) -> Option<f32> {
    let parts: Vec<&str> = t.trim().split([':', '.']).collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }
    let h: u32 = parts[0].trim().parse().ok()?;
    let m: u32 = parts[1].trim().parse().ok()?;
    let s: u32 = parts
        .get(2)
        .map(|x| x.trim().parse().ok())
        .unwrap_or(Some(0))?;
    (h < 48 && m < 60 && s < 60).then(|| h as f32 * 60.0 + m as f32 + s as f32 / 60.0)
}
