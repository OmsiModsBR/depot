//! The rows of a settings window: plain rows, switches, sliders and the values a slider runs through.

use super::values::{option_now, toggle_now};
use crate::core::app::App;
use crate::hud::domain::lists::{Move, PAX, PEDAL, SPEEDS, TRAFFIC, VOLUME};

/// A row of a settings window (see `ui::MenuKind::Options` for the format).
pub fn row(name: &str, kind: char, value: &str, desc: &str, frac: Option<f32>) -> String {
    format!(
        "{name}\u{1f}{kind}\u{1f}{value}\u{1f}{desc}\u{1f}{}",
        frac.map(|f| format!("{f:.3}")).unwrap_or_default()
    )
}

/// A row that opens another list.
pub(crate) fn opens(name: &str, desc: &str, id: &str) -> (String, String) {
    (row(name, 'o', "", desc, None), id.to_string())
}

/// A row with a button that does something.
pub(crate) fn button(name: &str, text: &str, desc: &str, id: &str) -> (String, String) {
    (row(name, 'a', text, desc, None), id.to_string())
}

pub(crate) fn switch_row(app: &App, id: &str, name: &str, desc: &str) -> Option<(String, String)> {
    let on = toggle_now(app, id)?;
    Some((
        row(name, 's', if on { "on" } else { "off" }, desc, None),
        id.to_string(),
    ))
}

/// The row of an option, unless "DirectX9 (D3D9Ex)" limits it (`api_limits`): then a locked one is
/// an info line with its fixed value and the reason, and a hidden one has no row.
pub(in crate::hud) fn limit_or(
    app: &App,
    key: &str,
    name: &str,
    normal: impl FnOnce() -> Option<(String, String)>,
) -> Option<(String, String)> {
    limit_row(
        app.output.backend() == crate::output::Backend::D3d9,
        key,
        name,
        normal,
    )
}

/// `limit_or` with the API given (so a test can ask without a running game).
pub fn limit_row(
    d3d9: bool,
    key: &str,
    name: &str,
    normal: impl FnOnce() -> Option<(String, String)>,
) -> Option<(String, String)> {
    if d3d9 {
        if let Some(l) = crate::api_limits::d3d9(key).filter(|l| !l.is_row()) {
            return l.shown().map(|v| {
                (
                    row(name, 'i', &omsi_ui::tr(v), &omsi_ui::tr(l.reason), None),
                    "noop".to_string(),
                )
            });
        }
    }
    normal()
}

/// A slider row for the setting `id` ("verb" or "verb arg"); `fmt` writes its value.
pub(crate) fn slider_row(
    app: &App,
    id: &str,
    name: &str,
    desc: &str,
    fmt: &dyn Fn(f32) -> String,
) -> Option<(String, String)> {
    let (verb, arg) = id.split_once(' ').unwrap_or((id, ""));
    let steps = steps_of(verb)?;
    let now = option_now(app, verb, arg)?;
    let i = nearest(&steps, now);
    let frac = if steps.len() > 1 {
        i as f32 / (steps.len() - 1) as f32
    } else {
        0.0
    };
    Some((row(name, 'v', &fmt(now), desc, Some(frac)), id.to_string()))
}

/// The values a slider's setting runs through.
pub(in crate::hud) fn steps_of(verb: &str) -> Option<Vec<f32>> {
    Some(match verb {
        "vr_nav_x" | "vr_nav_y" | "vr_nav_z" => (-100..=100).map(|v| v as f32 * 0.02).collect(),
        "triple_width_mm" => (20..=200).map(|v| v as f32 * 10.0).collect(),
        "triple_distance_mm" => (20..=300).map(|v| v as f32 * 10.0).collect(),
        "triple_bezel_mm" => (0..=100).map(|v| v as f32).collect(),
        "triple_left_angle_deg" | "triple_right_angle_deg" => (0..=90).map(|v| v as f32).collect(),
        "triple_eye_height_mm" => (-100..=100).map(|v| v as f32 * 5.0).collect(),
        "vr_nav_width" => (12..=65).map(|v| v as f32 * 0.01).collect(),
        "vr_nav_yaw" | "vr_nav_roll" => (-90..=90).map(|v| v as f32 * 2.0).collect(),
        "vr_nav_tilt" => (-40..=40).map(|v| v as f32 * 2.0).collect(),
        "vr_nav_opacity" => (6..=20).map(|v| v as f32 * 0.05).collect(),
        "speed" => SPEEDS.iter().map(|&v| v as f32).collect(),
        "traffic" => TRAFFIC.iter().map(|&v| v as f32).collect(),
        "pax" => PAX.to_vec(),
        "volume" => VOLUME.to_vec(),
        "led_glow" => (0..16).map(|v| v as f32).collect(),
        "night_brightness" => (0..=12).map(|v| v as f32 * 0.25).collect(),
        "led_mips" => (0..=80).map(|v| v as f32 * 0.05).collect(),
        "ui_scale" => (10..=40).map(|v| v as f32 * 0.05).collect(),
        "chat_size" => (5..=30).map(|v| v as f32 * 0.1).collect(),
        "ui_opacity" => (4..=20).map(|v| v as f32 * 0.05).collect(),
        "vol_ai" | "vol_scenery" => (0..=20).map(|v| v as f32 * 0.05).collect(),
        "wheel_range" => (6..=60).map(|v| v as f32 * 30.0).collect(),
        "wheel_lock" => std::iter::once(0.0)
            .chain((2..=60).map(|v| v as f32 * 30.0))
            .collect(),
        "outside_fov" | "fov" => std::iter::once(0.0)
            .chain((20..=120).map(|v| v as f32))
            .collect(),
        "steer_look_angle" => (0..=60).map(|v| v as f32).collect(),
        "steer_look_response" => (1..=20).map(|v| v as f32 * 0.05).collect(),
        "head_idle" => (0..=20).map(|v| v as f32 * 0.05).collect(),
        "head_idle_pace" => (10..=40).map(|v| v as f32 * 0.05).collect(),
        "head_tracking_yaw_sens"
        | "head_tracking_pitch_sens"
        | "head_tracking_roll_sens"
        | "head_tracking_x_sens"
        | "head_tracking_y_sens"
        | "head_tracking_z_sens" => (0..=100).map(|v| v as f32).collect(),
        "pedal_t" | "pedal_b" => PEDAL.to_vec(),
        "ctrl_deadzone" => (0..=30).map(|v| v as f32 * 0.01).collect(),
        "mouse_sens" => (10..=300).map(|v| v as f32 / 100.0).collect(),
        "look_sens" => (2..=40).map(|v| v as f32 * 0.05).collect(),
        "look_smoothing_ms" => (0..=20).map(|v| v as f32 * 10.0).collect(),
        "pad_steer_smooth" => (0..=30).map(|v| v as f32 * 10.0).collect(),
        "seat" => (-50..=50).map(|v| v as f32 / 100.0).collect(),
        "seat_pitch" => (-45..=45).map(|v| v as f32).collect(),
        "hour" => (0..24).map(|v| v as f32).collect(),
        "minute" => (0..60).map(|v| v as f32).collect(),
        // the weather, made by hand
        // (visibility goes by a few percent at a time from 100 m to 50 km, on whole tens of metres)
        "visibility" => {
            let mut v: Vec<f32> = (0..=120)
                .map(|i| {
                    let x = 100.0 * 500f32.powf(i as f32 / 120.0);
                    let step = if x < 1000.0 {
                        10.0
                    } else if x < 10000.0 {
                        100.0
                    } else {
                        500.0
                    };
                    (x / step).round() * step
                })
                .collect();
            v.dedup();
            v
        }
        "rain_amt" | "wet" => (0..=100).map(|v| v as f32 / 100.0).collect(),
        "brightness" => (0..=30).map(|v| v as f32 * 0.05).collect(),
        "humidity" => (0..=100).map(|v| v as f32).collect(),
        "temp" => (-20..=45).map(|v| v as f32).collect(),
        "wind_speed" => (0..=25).map(|v| v as f32).collect(),
        "wind_dir" => (0..360).map(|v| v as f32).collect(),
        _ => return None,
    })
}

/// Whether `verb` is a slider (a click on its track sets the value there).
pub(crate) fn is_slider(verb: &str) -> bool {
    steps_of(verb).is_some()
}

/// The index of the value in `steps` nearest to `now`.
pub(in crate::hud) fn nearest(steps: &[f32], now: f32) -> usize {
    steps
        .iter()
        .enumerate()
        .min_by(|a, b| (a.1 - now).abs().total_cmp(&(b.1 - now).abs()))
        .map(|x| x.0)
        .unwrap_or(0)
}

/// The value of `steps` after moving from `now`.
pub(in crate::hud) fn step_move(steps: &[f32], now: f32, mv: Move) -> f32 {
    let n = steps.len().max(1);
    let i = nearest(steps, now);
    let to = match mv {
        Move::Next => (i + 1) % n,
        Move::Inc => (i + 1).min(n - 1),
        Move::Dec => i.saturating_sub(1),
        Move::To(f) => (f.clamp(0.0, 1.0) * (n - 1) as f32).round() as usize,
    };
    steps.get(to).copied().unwrap_or(now)
}
