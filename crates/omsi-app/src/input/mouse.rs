//! The mouse and the VR pointer: clicks, the cursor, looking around and zooming, and what
//! is under the cursor.

use crate::app::App;
use crate::camera_util::{cursor_ray, pixel_angle, ORBIT_MAX, ORBIT_MIN};
use omsi_render::Camera;
use winit::keyboard::KeyCode;
// (its look tests also check two of the keyboard's helpers)
#[cfg(test)]
use super::keyboard::{key_left_free, route_char};

/// How far (m) a click reaches a page (`[htmltexture]`) on a scenery object.
pub(crate) const HTML_OBJECT_REACH: f32 = 4.0;

/// Degrees the view turns per (logical) pixel of the cursor's way while looking round:
/// OMSI's fov / 78.75.
pub(crate) fn look_deg_per_px(fov_deg: f32) -> f32 {
    fov_deg / 78.75
}

/// F3 chase orbit step from raw drag pixels: full turn in yaw at 0.35
/// deg/px (faster than the head's 0.15), pitch between -60 (near top-down)
/// and +25 (just below eye level) around the -15 rest pose, so the camera
/// never swings under the bus. Pure (tested below).
pub(crate) fn chase_orbit_step(yaw: f32, pitch: f32, dx_px: f32, dy_px: f32) -> (f32, f32) {
    const GAIN: f32 = 0.35;
    (
        (yaw + dx_px * GAIN).rem_euclid(360.0),
        (pitch - dy_px * GAIN).clamp(-60.0, 25.0),
    )
}

/// Precision zoom step from a vertical drag: the zoom state `z` (0 wide ..
/// 1 full zoom) travels at `intent` per 364 px, and the FOV multiplier is
/// `1/(1+5.5*z)` — full zoom ~6.5x in. Drag down (`dy > 0`) zooms in.
/// Never past 1.0 (never wider than the bus's own field of view); the floor
/// is the caller's clamp. Pure (tested below).
pub(crate) fn precision_zoom_step(mult: f32, dy_px: f32, intent: f32) -> f32 {
    const RANGE: f32 = 5.5;
    const FULL_DRAG_PX: f32 = 364.0;
    let z = ((1.0 / mult.max(0.154) - 1.0) / RANGE).clamp(0.0, 1.0);
    let z2 = (z + dy_px * intent / FULL_DRAG_PX).clamp(0.0, 1.0);
    1.0 / (1.0 + RANGE * z2)
}

/// F1 zoom intent: the head zoom runs 20% slower than outside/free.
pub(crate) const ZOOM_INTENT_F1: f32 = 0.56;

/// Outside/free zoom intent: a full 364 px drag takes `z` 0 to 0.70.
pub(crate) const ZOOM_INTENT: f32 = 0.70;

/// Eased Space return for the F1 head: look and zoom glide home on the same
/// ease-out as the viewpoint switch instead of teleporting. `t` seconds in;
/// returns the current look, zoom and done. Pure (tested below).
pub(crate) fn reset_blend(
    look_from: (f32, f32),
    zoom_from: f32,
    t: f32,
) -> ((f32, f32), f32, bool) {
    let x = (t / crate::app::CAM_BLEND_SECS).clamp(0.0, 1.0);
    let u = 1.0 - x;
    let s = 1.0 - u * u * u;
    (
        (look_from.0 * (1.0 - s), look_from.1 * (1.0 - s)),
        zoom_from + (1.0 - zoom_from) * s,
        x >= 1.0,
    )
}

#[cfg(test)]
pub(crate) mod look_tests {
    /// F1 given to a door and the driver's view moved to 1 (#701): F1 is not the view any more.
    #[test]
    fn a_built_in_view_key_steps_aside_for_the_players_own() {
        use omsi_content::KeyBinding;
        let kb = |a: &str, k: i32| KeyBinding {
            action: a.into(),
            scan_code: k,
            modifier: 0,
        };
        let none = std::collections::HashSet::new();
        assert!(super::key_left_free(
            Some(59),
            "view_set_driver",
            &none,
            &[]
        ));
        let own: std::collections::HashSet<i32> = [59].into();
        assert!(!super::key_left_free(
            Some(59),
            "view_set_driver",
            &own,
            &[]
        ));
        assert!(!super::key_left_free(
            Some(59),
            "view_set_driver",
            &none,
            &[kb("view_set_driver", 2)]
        ));
        assert!(super::key_left_free(
            Some(59),
            "view_set_driver",
            &none,
            &[kb("view_set_passenger", 60)]
        ));
    }

    #[test]
    fn a_route_number_takes_digits_letters_and_symbols() {
        use winit::keyboard::KeyCode;
        assert_eq!(super::route_char(KeyCode::Digit5), Some('5'));
        assert_eq!(super::route_char(KeyCode::Numpad0), Some('0'));
        assert_eq!(super::route_char(KeyCode::KeyE), Some('E'));
        assert_eq!(super::route_char(KeyCode::Minus), Some('-'));
        assert_eq!(super::route_char(KeyCode::NumpadSubtract), Some('-'));
        assert_eq!(super::route_char(KeyCode::NumpadAdd), Some('+'));
        assert_eq!(super::route_char(KeyCode::Slash), Some('/'));
        assert_eq!(super::route_char(KeyCode::Space), None);
    }

    #[test]
    fn a_cursor_way_of_78_75_px_turns_by_the_field_of_view() {
        assert!((78.75 * super::look_deg_per_px(60.0) - 60.0).abs() < 1e-4);
    }

    #[test]
    fn chase_orbits_at_035_deg_px_with_stops_above_and_below() {
        // 100 px drag down-right: +35 yaw, -35 pitch.
        let (y, p) = super::chase_orbit_step(0.0, 0.0, 100.0, 100.0);
        assert!(
            (y - 35.0).abs() < 1e-4 && (p + 35.0).abs() < 1e-4,
            "{y} {p}"
        );
        // yaw wraps the full circle.
        assert!((super::chase_orbit_step(350.0, 0.0, 100.0, 0.0).0 - 25.0).abs() < 1e-3);
        // pitch never leaves the stops, whichever way it is dragged.
        assert_eq!(super::chase_orbit_step(0.0, 0.0, 0.0, -1000.0).1, 25.0);
        assert_eq!(super::chase_orbit_step(0.0, 0.0, 0.0, 1000.0).1, -60.0);
    }

    #[test]
    fn space_return_eases_home_like_the_viewpoint_switch() {
        // start: untouched; partway: well on the way (ease-out); end: exact and done.
        let (look, zoom, done) = super::reset_blend((30.0, -10.0), 0.5, 0.0);
        assert_eq!((look, zoom, done), ((30.0, -10.0), 0.5, false));
        let (look, zoom, done) = super::reset_blend((30.0, -10.0), 0.5, 0.27);
        assert!(look.0 > 3.0 && look.0 < 27.0 && zoom > 0.5 && zoom < 1.0 && !done);
        let (look, zoom, done) = super::reset_blend((30.0, -10.0), 0.5, 0.54);
        assert_eq!((look, zoom, done), ((0.0, 0.0), 1.0, true));
        assert!(super::reset_blend((30.0, -10.0), 0.5, 5.0).2);
    }

    #[test]
    fn precision_zoom_follows_the_fov_curve_and_never_widens() {
        // a full 364 px drag down takes z 0 to 0.70: m = 1/(1+5.5*0.70).
        let m = super::precision_zoom_step(1.0, 364.0, super::ZOOM_INTENT);
        assert!((m - 1.0 / (1.0 + 5.5 * 0.70)).abs() < 1e-4, "{m}");
        // drag down zooms in, drag up undoes it, never past 1.0.
        let mid = super::precision_zoom_step(1.0, 100.0, super::ZOOM_INTENT);
        assert!(mid < 1.0 && mid > 0.45, "{mid}");
        assert!((super::precision_zoom_step(mid, -100.0, super::ZOOM_INTENT) - 1.0).abs() < 1e-4);
        assert_eq!(
            super::precision_zoom_step(1.0, -50.0, super::ZOOM_INTENT),
            1.0
        );
        // F1 runs the same curve 20% slower.
        let slow = super::precision_zoom_step(1.0, 100.0, super::ZOOM_INTENT_F1);
        assert!(slow > mid && slow < 1.0, "{slow} vs {mid}");
    }
}

/// `App::sync_view_look` for where `self` is borrowed in parts.
/// See `App::look_key`.
pub(crate) fn look_key_of(view: &str, cam: Option<(usize, usize)>) -> String {
    match (view, cam) {
        ("driver", Some((d, _))) => format!("driver#{d}"),
        ("pax", Some((_, x))) => format!("pax#{x}"),
        _ => view.to_string(),
    }
}

/// How far the head turns inside the bus: all the way round, in the driver's seat as in a
/// passenger's - OMSI's mouse look adds the cursor's way to the camera's
/// yaw with no stop. Capped at 140 degrees each way, a quarter of the coach stayed out of
/// sight (#909). The turn is kept within -180..180 so that letting go of a glance still
/// swings the short way back.
pub(crate) fn cab_look_yaw(yaw: f32) -> f32 {
    (yaw + 180.0).rem_euclid(360.0) - 180.0
}

/// Put the direction of the view left away and take up the one of the view entered; `true`
/// when that was another view (the caller then knows the head did not just move: it is where
/// the view entered last left it).
pub(crate) fn swap_view_look(
    look: &mut (f32, f32),
    looks: &mut std::collections::HashMap<String, (f32, f32)>,
    look_view: &mut String,
    view: &str,
) -> bool {
    if look_view == view {
        return false;
    }
    let old = std::mem::replace(look_view, view.to_string());
    if !old.is_empty() {
        looks.insert(old, *look);
    }
    *look = looks.get(view).copied().unwrap_or((0.0, 0.0));
    true
}

/// Ease the angle the view is drawn at out of the head's own angle, once a frame, and give it
/// back: `shown` keeps the angle of the frame before, `ms` is the time constant
/// (`look_smoothing_ms`, 0 = the view is drawn where the head points at once).
///
/// The step is `1 - e^(-dt/tau)`, so 30 and 300 frames a second smooth alike (a plain
/// fraction of the way would smooth the faster machine harder) and the yaw goes the short way
/// round. Off, the angle is taken over at once, which is also what puts the view back in step
/// after a change of view or a `look` a script set whole.
///
/// A free function: it is called where `self` is borrowed in parts, and needs nothing of the
/// app but the two angles.
pub(crate) fn ease_look(
    shown: &mut (f32, f32),
    wanted: (f32, f32),
    dt: f32,
    ms: f32,
) -> (f32, f32) {
    let tau = ms * 0.001;
    if !(tau > 0.0) || !dt.is_finite() || dt <= 0.0 {
        *shown = wanted;
        return wanted;
    }
    let f = 1.0 - (-dt / tau).exp();
    let dyaw = (wanted.0 - shown.0 + 180.0).rem_euclid(360.0) - 180.0;
    // (in the range the head's yaw is kept in: -180..180 in the cab, `cab_look_yaw`, else
    // 0..360 - a glide across the back would otherwise hand the cab a yaw of 350)
    let yaw = shown.0 + dyaw * f;
    shown.0 = if (-180.0..=180.0).contains(&wanted.0) {
        (yaw + 180.0).rem_euclid(360.0) - 180.0
    } else {
        yaw.rem_euclid(360.0)
    };
    shown.1 += (wanted.1 - shown.1) * f;
    *shown
}

#[cfg(test)]
pub(crate) mod look_smoothing_tests {
    use super::ease_look;

    /// A cab's yaw (-180..180) eased across the back stays in its range: from 170 to -170 it
    /// goes through 180, not out to 190 or round to 350.
    #[test]
    fn a_cab_yaw_stays_within_half_a_turn() {
        let mut shown = (170.0, 0.0);
        for _ in 0..30 {
            let (yaw, _) = ease_look(&mut shown, (-170.0, 0.0), 1.0 / 60.0, 100.0);
            assert!((-180.0..=180.0).contains(&yaw), "{yaw}");
            assert!(yaw >= 170.0 || yaw <= -170.0, "the short way round: {yaw}");
        }
    }

    /// The step is `1 - e^(-dt/tau)`: a machine drawing 600 frames a second glides exactly
    /// as far in a second as one drawing 60 (a plain fraction of the way would smooth the
    /// faster of the two harder, and the same setting would feel different on a faster PC).
    #[test]
    fn the_glide_does_not_depend_on_the_frame_rate() {
        let (from, to) = ((0.0, 0.0), (30.0, 10.0));
        let mut slow = from;
        for _ in 0..60 {
            ease_look(&mut slow, to, 1.0 / 60.0, 100.0);
        }
        let mut fast = from;
        for _ in 0..600 {
            ease_look(&mut fast, to, 1.0 / 600.0, 100.0);
        }
        assert!(
            (slow.0 - fast.0).abs() < 0.01 && (slow.1 - fast.1).abs() < 0.01,
            "{slow:?} against {fast:?}"
        );
        // a whole second at a time constant of 100 ms is very nearly there
        assert!(slow.0 > 29.9 && slow.1 > 9.9, "{slow:?}");
    }

    /// A head turned a degree past north eases a degree forward, not 359 back.
    #[test]
    fn the_glide_goes_the_short_way_round() {
        let mut shown = (359.0, 0.0);
        let there = ease_look(&mut shown, (1.0, 0.0), 0.05, 100.0);
        assert!((there.0 - 359.0).rem_euclid(360.0) < 1.0, "{there:?}");
    }

    /// Off, the view is drawn exactly where the head points, at once: what every machine
    /// had before this was a setting.
    #[test]
    fn without_the_setting_the_view_is_where_the_head_points() {
        let mut shown = (0.0, 0.0);
        assert_eq!(
            ease_look(&mut shown, (30.0, 10.0), 1.0 / 60.0, 0.0),
            (30.0, 10.0)
        );
        assert_eq!(shown, (30.0, 10.0));
    }
}

/// A part of a vehicle (its origin, heading in degrees and `[boundingbox]`) is in reach of
/// a person standing at `eye`: within 3 m of the box's half length round its centre.
pub(crate) fn part_in_reach(
    eye: glam::DVec3,
    at: glam::DVec3,
    heading: f64,
    bb: Option<[f32; 6]>,
) -> bool {
    let bb = bb.unwrap_or([2.5, 12.0, 3.0, 0.0, 0.0, 1.5]);
    let h = heading.to_radians();
    let (fwd, right) = (
        glam::DVec2::new(h.sin(), h.cos()),
        glam::DVec2::new(h.cos(), -h.sin()),
    );
    let centre = at + (right * bb[3] as f64 + fwd * bb[4] as f64).extend(bb[5] as f64);
    let reach = (bb[0].max(bb[1]) as f64) * 0.5 + 3.0;
    (eye - centre).length() < reach
}

#[cfg(test)]
pub(crate) mod reach_tests {
    use super::part_in_reach;
    use glam::DVec3;

    /// Standing by the rear section of an articulated bus, 17 m behind the front part's
    /// origin: out of the front part's reach, in the rear one's.
    #[test]
    fn the_rear_section_is_reached_by_its_own_box() {
        let front = [2.5, 11.0, 3.0, 0.0, -3.0, 1.5];
        let rear = [2.5, 7.0, 3.0, 0.0, -3.5, 1.5];
        let eye = DVec3::new(2.0, -17.0, 1.7);
        assert!(!part_in_reach(eye, DVec3::ZERO, 0.0, Some(front)));
        assert!(part_in_reach(
            eye,
            DVec3::new(0.0, -12.0, 0.0),
            0.0,
            Some(rear)
        ));
        // (the box's centre turns with the part: heading 180, the rear is ahead)
        assert!(part_in_reach(
            DVec3::new(-2.0, 17.0, 1.7),
            DVec3::new(0.0, 12.0, 0.0),
            180.0,
            Some(rear)
        ));
    }
}

#[cfg(test)]
pub(crate) mod cab_look_tests {
    use super::cab_look_yaw;

    /// A passenger and the driver turn all the way round, as in OMSI (#909).
    #[test]
    fn the_head_turns_all_the_way_round() {
        let mut yaw = 0.0;
        for _ in 0..40 {
            yaw = cab_look_yaw(yaw + 10.0);
        }
        // 400 degrees turned: 40 past straight ahead, the short way
        assert!((yaw - 40.0).abs() < 1e-3, "{yaw}");
        assert!((cab_look_yaw(170.0 + 20.0) + 170.0).abs() < 1e-3);
        // (the driver looks back down the saloon: no stop at 140 degrees)
        assert!((cab_look_yaw(175.0) - 175.0).abs() < 1e-3);
        assert!((cab_look_yaw(-160.0) + 160.0).abs() < 1e-3);
    }
}

/// The first entry a drop-down (or the tours' timetable) of `n` entries showing `rows`
/// shows with its thumb (`len` high) at the top `thumb_top` in `track`.
pub(crate) fn dropdown_top_at(
    thumb_top: f32,
    track: [f32; 4],
    len: f32,
    n: usize,
    rows: usize,
) -> usize {
    let max = n.saturating_sub(rows);
    let travel = (track[3] - track[1] - len).max(1.0);
    let f = ((thumb_top - track[1]) / travel).clamp(0.0, 1.0);
    ((f * max as f32).round() as usize).min(max)
}

#[cfg(test)]
pub(crate) mod dropdown_tests {
    /// A drop-down's thumb dragged down its track scrolls the list to its end (#794).
    #[test]
    fn a_dropdowns_thumb_dragged_scrolls_it() {
        // 40 entries, 8 shown, a 300 px track with a 60 px thumb
        let track = [0.0, 100.0, 4.0, 400.0];
        assert_eq!(super::dropdown_top_at(100.0, track, 60.0, 40, 8), 0);
        assert_eq!(super::dropdown_top_at(340.0, track, 60.0, 40, 8), 32);
        assert_eq!(super::dropdown_top_at(220.0, track, 60.0, 40, 8), 16);
        // past the ends it stays at them
        assert_eq!(super::dropdown_top_at(-50.0, track, 60.0, 40, 8), 0);
        assert_eq!(super::dropdown_top_at(900.0, track, 60.0, 40, 8), 32);
        // a list that fits never scrolls
        assert_eq!(super::dropdown_top_at(300.0, track, 60.0, 5, 8), 0);
    }
}

impl App {
    /// Turn the view by (dx, dy) degrees, as dragging with the right button does: the free
    /// camera turns, inside the bus the head turns, outside the camera swings around it.
    /// Keeps `look` with the view it belongs to: on a change of view the direction of the
    /// view left is put away and the one of the view entered comes back (straight ahead
    /// the first time).
    pub(crate) fn sync_view_look(&mut self) {
        let key = self.look_key();
        // A change of view is not a turn of the head: the direction of the view entered is
        // where the head already is, so the angle the camera is drawn at starts there as
        // well (a glide belongs between angles of one and the same view).
        if swap_view_look(
            &mut self.look,
            &mut self.view_looks,
            &mut self.look_view,
            &key,
        ) {
            self.look_smooth = self.look;
        }
    }

    /// Which camera the look belongs to: the view, and for the driver's and the passengers'
    /// view the camera chosen in it. Each of OMSI's cameras keeps where it was turned
    /// (a camera has its own yaw and pitch besides the file's, which a reset clears): the
    /// look went back to straight ahead whenever the viewpoint changed.
    pub(crate) fn look_key(&self) -> String {
        look_key_of(&self.view, self.player.as_ref().map(|p| p.cam_choice))
    }

    /// Zoom the view inside the bus by `notches` of the mouse wheel (in: positive).
    pub(crate) fn zoom_by(&mut self, notches: f32) {
        // a hand on the zoom cancels an eased Space return.
        self.f1_reset = None;
        let z = self.view_zoom.entry(self.view.clone()).or_insert(1.0);
        *z = (*z * (1.0 - 0.08 * notches.clamp(-5.0, 5.0))).clamp(0.2, 1.6);
    }

    pub(crate) fn look_by(&mut self, dx: f32, dy: f32) {
        self.sync_view_look();
        // a hand on the view cancels an eased Space return.
        self.f1_reset = None;
        if self.view == "foot" {
            self.foot_look(dx, dy);
            return;
        }
        if self.view == "free" || self.player.is_none() {
            if let Some(cam) = self.camera.as_mut() {
                cam.yaw = (cam.yaw + dx).rem_euclid(360.0);
                cam.pitch = (cam.pitch - dy).clamp(-89.0, 89.0);
            }
        } else if self.view == "outside" {
            // F3 chase orbit: full turn in yaw; pitch stops between near
            // top-down and just below eye level so the camera never swings
            // under the bus (see `chase_orbit_step` for the mouse gain).
            self.look.0 = (self.look.0 + dx).rem_euclid(360.0);
            self.look.1 = (self.look.1 - dy).clamp(-60.0, 25.0);
        } else {
            self.look.0 = cab_look_yaw(self.look.0 + dx);
            self.look.1 = (self.look.1 - dy).clamp(-85.0, 85.0);
        }
    }

    /// The cursor moved to (x, y) in physical pixels - from the window or an `OMSI_INPUT` script.
    /// The cursor moved, and the switch under it is named at once (touch input and
    /// `OMSI_INPUT` scripts read `hover` right after).
    pub(crate) fn on_cursor(&mut self, x: f32, y: f32) {
        if self.move_cursor(x, y) {
            self.update_hover();
        }
    }

    /// The right mouse button (or both) held in a view of the bus: start OMSI's mouse zoom
    /// (false when there is nothing to zoom - a menu, the city map, on foot).
    pub(crate) fn start_both_drag(&mut self) -> bool {
        if self.game_menu.is_some()
            || self.player.is_none()
            || self.navigator.as_ref().is_some_and(|n| n.map_open())
        {
            return false;
        }
        let value = match self.view.as_str() {
            "outside" => self.orbit,
            "driver" | "pax" => *self.view_zoom.get(&self.view).unwrap_or(&1.0),
            _ => return false,
        };
        self.both_drag = Some((self.cursor.1, value));
        self.mouse_look = false;
        self.update_hover();
        true
    }

    /// Whether the mouse steering (when on) steers in the view shown: every view of the
    /// player's bus, the map camera (F4) included, but not walking.
    pub(crate) fn mouse_steers_in_view(&self) -> bool {
        self.player.is_some()
            && (matches!(self.view.as_str(), "driver" | "outside" | "pax")
                || (self.view == "free" && !self.ego))
    }

    /// Looking round with the mouse goes by the cursor's way in the window (a view of the
    /// bus); on foot and with the free camera it keeps the raw mouse movement.
    pub(crate) fn cursor_looks(&self) -> bool {
        self.mouse_look && self.player.is_some() && !matches!(self.view.as_str(), "foot" | "free")
    }

    /// The right button alone zooms, as in OMSI (the right button
    /// without `[altView]`, or Shift+right with it); otherwise it turns the view.
    pub(crate) fn right_zooms(&self) -> bool {
        !self.settings.alt_view
            || self.keys.contains(&KeyCode::ShiftLeft)
            || self.keys.contains(&KeyCode::ShiftRight)
    }

    /// The right mouse button on the desktop: held, it zooms (the outside camera's distance,
    /// the view in the bus), or with OMSI's `[altView]` turns the view; the middle button
    /// turns it in any case. Where there is nothing to zoom it turns the view.
    pub(crate) fn on_right(&mut self, pressed: bool) {
        self.buttons_held.1 = pressed;
        // the left button already down on nothing it works: both held zoom
        if pressed && self.buttons_held.0 && !self.dragging && self.start_both_drag() {
            return;
        }
        // (a switch held with the left button keeps the mouse: looking round
        // took the cursor's movement away from it, and the drag stopped)
        if pressed && self.dragging {
            return;
        }
        if !pressed {
            self.both_drag = None;
        }
        // a right click lets go of the mouse steering as in OMSI (#162) when the player
        // wants it so; otherwise the right button looks round and the wheel and pedals stay
        // where the mouse left them (it went off with every look round, and with every
        // look round in the pause)
        if pressed
            && self.mouse_drive
            && self.game_menu.is_none()
            && self.settings.mouse_right_off
            && !self.paused
        {
            self.set_mouse_drive(false);
            self.service_msg = Some(("Mouse steering off".into(), 3.0));
        }
        if pressed && self.right_zooms() && self.start_both_drag() {
            return;
        }
        if self.mouse_drive && self.game_menu.is_none() {
            if pressed {
                self.steer_cursor = Some(self.cursor);
            } else if let Some((x, y)) = self.steer_cursor.take() {
                self.cursor = (x, y);
                if let Some(win) = self.window.as_ref() {
                    let _ = win
                        .set_cursor_position(winit::dpi::PhysicalPosition::new(x as f64, y as f64));
                }
            }
        }
        self.mouse_look = pressed;
        // (the cursor shows it at once, not with the next look at what is under it)
        self.update_hover();
    }

    pub(crate) fn on_mouse_moved(&mut self, x: f32, y: f32) {
        // a mirror panel being dragged follows the cursor (nothing else of the cursor's
        // work is done meanwhile, and outside a drag none of it is touched)
        if self.mirror_hud.dragging() {
            if let Some(size) = self.surface.as_ref().map(|_| self.hud_size()) {
                let origin_x = self.cursor.0 - self.hud_cursor().0;
                if self.mirror_hud.moved((x - origin_x, y), size) {
                    self.cursor = (x, y);
                    return;
                }
            }
        }
        if self.move_cursor(x, y) {
            self.html_move();
        }
    }

    pub(crate) fn html_move(&mut self) {
        if let Some((id, page, ..)) = self.html_object_pressed {
            let Some((o, d, _)) = self.cursor_ray_now() else {
                return;
            };
            let Some(w) = self.world.clone() else { return };
            if let Some(h) = w
                .html_object_hit(o, d, HTML_OBJECT_REACH)
                .filter(|h| h.map_id == id && h.page == page)
            {
                w.html_object_pointer(id, page, h.u, h.v, omsi_sim::htmltex::PointerKind::Move);
                self.html_object_pressed = Some((id, page, h.u, h.v));
            }
            return;
        }
        let Some((page, ..)) = self.html_pressed else {
            return;
        };
        let Some((o, d, _)) = self.cursor_ray_now() else {
            return;
        };
        let Some(p) = self.player.as_mut() else {
            return;
        };
        if let Some((pg, u, v)) = p.html_hit(o, d).filter(|h| h.0 == page) {
            p.html_pointer(pg, u, v, omsi_sim::htmltex::PointerKind::Move);
            self.html_pressed = Some((pg, u, v));
        }
    }

    #[cfg(windows)]
    pub(crate) fn reset_vr_pointer(&mut self) {
        if let Some(vr) = self.vr.as_mut() {
            vr.recenter_pointer();
        }
        self.vr_cursor_physical = None;
        self.vr_cursor_warp_pending = None;
    }

    #[cfg(windows)]
    pub(crate) fn on_vr_cursor_moved(&mut self, x: f32, y: f32) {
        if let Some(target) = self.vr_cursor_warp_pending.take() {
            // CursorMoved from set_cursor_position is not hand movement.
            self.vr_cursor_physical = Some((x, y));
            if (x - target.0).abs() < 3.0 && (y - target.1).abs() < 3.0 {
                return;
            }
            // A real move arrived first; use the next event as the new baseline.
            return;
        }
        if let Some(previous) = self.vr_cursor_physical {
            self.cursor.0 += x - previous.0;
            self.cursor.1 += y - previous.1;
        }
        self.vr_cursor_physical = Some((x, y));
        self.html_move();
        let Some((width, height)) = self
            .surface
            .as_ref()
            .map(|s| (s.config.width as f32, s.config.height as f32))
        else {
            return;
        };
        if self.window_focused
            && !self.mouse_look
            && (x < 12.0 || x > width - 12.0 || y < 12.0 || y > height - 12.0)
        {
            let center = (width * 0.5, height * 0.5);
            if self.window.as_ref().is_some_and(|window| {
                window
                    .set_cursor_position(winit::dpi::PhysicalPosition::new(
                        center.0 as f64,
                        center.1 as f64,
                    ))
                    .is_ok()
            }) {
                self.vr_cursor_physical = Some(center);
                self.vr_cursor_warp_pending = Some(center);
            }
        }
    }

    #[cfg(windows)]
    pub(crate) fn poll_vr_cursor_position(&mut self) {
        if self.vr_nav_edit.is_some() {
            return;
        }
        let cockpit = self.vr.is_some()
            && self.game_menu.is_none()
            && self.chooser.is_none()
            && !self.mouse_drive
            && matches!(self.view.as_str(), "driver" | "pax");
        if !cockpit {
            self.vr_cursor_physical = None;
            self.vr_cursor_warp_pending = None;
            return;
        }
        if !self.window_focused || self.mouse_look {
            return;
        }
        let Some(window) = self.window.as_ref() else {
            return;
        };
        let Ok(client_origin) = window.inner_position() else {
            return;
        };
        let mut point = windows::Win32::Foundation::POINT::default();
        if unsafe { windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut point) }.is_ok() {
            self.on_vr_cursor_moved(
                (point.x - client_origin.x) as f32,
                (point.y - client_origin.y) as f32,
            );
        }
    }

    /// Mouse steering beyond the window's edge: with the cursor pinned at the left or right
    /// edge, the mouse moving on outwards turns the wheel further (the whole width per full
    /// lock, as standing); moving back gives that back first, the cursor held at the edge
    /// until it is used up, so the wheel never jumps.
    pub(crate) fn mouse_past_edge(&mut self, dx: f32) {
        let Some(w) = self.surface.as_ref().map(|s| s.config.width as f32) else {
            return;
        };
        let per_px = 2.0 / w.max(1.0);
        let (at_left, at_right) = (self.cursor.0 <= 2.0, self.cursor.0 >= w - 3.0);
        let before = self.mouse_edge;
        if (at_right && dx > 0.0) || (at_left && dx < 0.0) {
            self.mouse_edge = (self.mouse_edge + dx * per_px).clamp(-2.0, 2.0);
        } else if (self.mouse_edge > 0.0 && dx < 0.0) || (self.mouse_edge < 0.0 && dx > 0.0) {
            let m = self.mouse_edge + dx * per_px;
            self.mouse_edge = if m.signum() != before.signum() {
                0.0
            } else {
                m
            };
            // (the cursor stays where it was: the move went into the wheel)
            if let Some(win) = self.window.as_ref() {
                let x = if before > 0.0 { w - 2.0 } else { 1.0 };
                let _ = win.set_cursor_position(winit::dpi::PhysicalPosition::new(
                    x as f64,
                    self.cursor.1 as f64,
                ));
                self.cursor.0 = x;
            }
        }
    }

    /// Take the cursor's new place; false when the move was someone else's (the object
    /// editor's drag, the city map) and no switch is to be named.
    pub(crate) fn move_cursor(&mut self, x: f32, y: f32) -> bool {
        let last = self.cursor;
        self.cursor = (x, y);
        // the navigator held by the mouse follows it
        if let Some(n) = self.navigator.as_mut() {
            if n.panel_move(x, y) {
                return true;
            }
        }
        // (the mouse has taken over from the keyboard: only what is under it is lit)
        if self.game_menu.is_some() && (x, y) != last {
            self.menu_kbd = false;
        }
        // a slider held with the mouse button follows the cursor (while the menu is open)
        if self.menu_drag.is_some() && self.game_menu.is_none() {
            self.menu_drag = None;
        }
        if let Some(k) = self.menu_drag {
            let c = self.ui.as_ref().and_then(|u| {
                k.checked_sub(u.menu_start)
                    .and_then(|i| u.menu_ctl.get(i).copied().flatten())
            });
            match c {
                Some(c) => {
                    let fx = ((x - c[0]) / (c[2] - c[0]).max(1.0)).clamp(0.0, 1.0);
                    self.list_click(k, fx);
                }
                None => self.menu_drag = None,
            }
            return false;
        }
        if let Some((y0, v0)) = self.both_drag {
            // a hand on the zoom cancels an eased Space return.
            self.f1_reset = None;
            // (outside, the distance at the press times 1 + the way up over 500
            // pixels; in the bus the field of view at the press plus the way up over 500
            // pixels times the camera's own, which is also its widest:
            // moving up widens the view as it backs the outside camera away)
            if self.view == "outside" {
                let k = (1.0 + (y0 - y) / 500.0).max(0.05);
                self.orbit = (v0 * k).clamp(ORBIT_MIN, ORBIT_MAX);
            } else if self.settings.precision_zoom {
                // precision zoom from the press anchor (drag down zooms in):
                // the FOV-multiplier curve instead of the linear way, same
                // floor. Past the authored field of view it stays linear.
                let intent = if self.view == "driver" {
                    ZOOM_INTENT_F1
                } else {
                    ZOOM_INTENT
                };
                let dy = y - y0;
                let m = if v0 > 1.0 {
                    (v0 - dy / 500.0).clamp(0.2, v0.max(1.0))
                } else {
                    precision_zoom_step(v0, dy, intent).clamp(0.2, 1.0)
                };
                self.view_zoom.insert(self.view.clone(), m);
            } else {
                self.view_zoom.insert(
                    self.view.clone(),
                    (v0 + (y0 - y) / 500.0).clamp(0.2, 1.0_f32.max(v0)),
                );
            }
            return false;
        }
        if self.pane_scroll_drag.is_some() {
            if self.chooser.is_none() || self.game_menu.is_none() {
                self.pane_scroll_drag = None;
            } else {
                self.drag_pane(y);
                return false;
            }
        }
        if self.dd_scroll_drag.is_some() {
            if self.dropdown.is_none() || self.game_menu.is_none() {
                self.dd_scroll_drag = None;
            } else {
                self.drag_dropdown(y);
                return false;
            }
        }
        if self.menu_scroll_drag {
            let Some(ui) = self.ui.as_ref() else {
                self.menu_scroll_drag = false;
                return true;
            };

            if let (Some(track), Some(thumb)) = (ui.menu_scroll_track, ui.menu_scroll_thumb) {
                let track_h = (track[3] - track[1]).max(1.0);
                let thumb_h = (thumb[3] - thumb[1]).max(1.0);
                let travel = (track_h - thumb_h).max(1.0);

                let max_top = (self.menu_len() as f32 - ui.menu_rows as f32).max(0.0);

                if max_top > 0.0 {
                    let delta = (y - last.1) / travel * max_top;

                    self.menu_top = Some(
                        (self.menu_top.unwrap_or(ui.menu_start as f32) + delta).clamp(0.0, max_top),
                    );
                }
            }

            return false;
        }
        // an object dragged in the object editor follows
        if self.editor_drag {
            self.editor_drag_frame();
            return false;
        }
        // while the city map is open the mouse is the map's
        if let Some(n) = self.navigator.as_mut().filter(|n| n.map_open()) {
            n.map_move(x, y);
            return false;
        }
        // looking round in a view of the bus follows the cursor, as OMSI turns it
        // (yaw and pitch at the press plus the cursor's way times fov / 78.75):
        // raw device deltas are no window pixels (a tablet, a remote desktop or a VM
        // reports positions there and spun the view) and did not follow the zoom
        if self.cursor_looks() {
            let scale = self
                .window
                .as_ref()
                .map(|w| w.scale_factor() as f32)
                .unwrap_or(1.0)
                .max(0.1);
            let fov = self.camera.as_ref().map(|c| c.fov_deg).unwrap_or(60.0);
            let k = look_deg_per_px(fov) * self.settings.look_sens;
            self.look_by((x - last.0) / scale * k, (y - last.1) / scale * k);
        }
        // Dragging a switch reads the movement in screen pixels - take it from the
        // cursor itself rather than from the raw device delta, which is not in the
        // window's pixels (and on this Mac is not always delivered at all): that is
        // why the parking brake could not be pulled with the mouse. The movement is
        // collected here and handed to the script once a frame (`App::drag_frame`).
        if self.dragging {
            let scale = self
                .window
                .as_ref()
                .map(|w| w.scale_factor() as f32)
                .unwrap_or(1.0)
                .max(0.1);
            self.drag_delta.0 += (self.cursor.0 - last.0) / scale;
            self.drag_delta.1 += (self.cursor.1 - last.1) / scale;
        }
        true
    }

    pub(crate) fn on_left(&mut self, pressed: bool) {
        if self.vr_nav_edit.is_some() {
            return;
        }
        // the object editor: the mouse picks and drags
        if self.game_menu.is_none() && self.editor_mouse(pressed) {
            return;
        }
        // the city map: a click on the navigator opens it; while it is open the mouse is
        // the map's (a click outside closes it)
        let (x, y) = self.cursor;
        let vr_active = self.vr_active();
        if let Some(n) = self.navigator.as_mut() {
            if n.map_open() {
                let ctrl = self.keys.contains(&KeyCode::ControlLeft)
                    || self.keys.contains(&KeyCode::ControlRight);
                if pressed && (ctrl || self.teleport_pick) {
                    // Ctrl+click (or a click after Esc → Move the bus): the bus to the street
                    // nearest that point, as OMSI's map window places vehicles
                    if let Some(at) = n.map_point(x, y) {
                        if std::mem::take(&mut self.teleport_pick) {
                            n.toggle_map();
                        }
                        self.place_bus_at(at);
                    }
                } else if pressed {
                    n.map_press(x, y);
                } else {
                    n.map_release();
                }
                return;
            }
            // (a click opens the city map, a drag moves the navigator: #940)
            if pressed && !vr_active && n.over_panel(x, y) {
                n.panel_press(x, y);
                return;
            }
            if !pressed {
                match n.panel_release() {
                    Some(false) => {
                        n.toggle_map();
                        return;
                    }
                    Some(true) => {
                        let at = n.placement();
                        self.settings.navigator_corner = at.clone();
                        crate::game_lists::remember_setting("navigator_corner", &at);
                        return;
                    }
                    None => {}
                }
            }
        }
        // the map camera (F4): Ctrl+click on the ground puts the bus on the street nearest
        // that point, as Ctrl+click on the city map does - OMSI's map view moves the vehicle
        // to a place clicked as well (#1039). A rail vehicle stays on its track.
        let ctrl =
            self.keys.contains(&KeyCode::ControlLeft) || self.keys.contains(&KeyCode::ControlRight);
        if pressed
            && ctrl
            && self.view == "free"
            && self.game_menu.is_none()
            && self.player.is_some()
        {
            if self
                .player
                .as_ref()
                .is_some_and(|p| crate::rail_drive::is_rail(&p.vehicle.ty.def))
            {
                self.service_msg =
                    Some(("A rail vehicle cannot be moved off its track".into(), 3.0));
                return;
            }
            let hit = self
                .cursor_ray_now()
                .zip(self.world.clone())
                .and_then(|((o, d, _), w)| crate::placing::ground_hit(&w, o, d.as_dvec3(), 2000.0));
            match hit {
                Some(at) => self.place_bus_at(at.truncate()),
                None => {
                    self.service_msg =
                        Some(("Ctrl+click on the ground to move the bus there".into(), 3.0))
                }
            }
            return;
        }
        // a click on the chat opens its input box (and is the chat's, not the cockpit's)
        #[cfg(feature = "lan")]
        if pressed && self.in_lan_session() && self.settings.chat {
            if self.ui.as_ref().map(|u| u.chat.hovered).unwrap_or(false) {
                self.online.remotes.chat.open();
                return;
            }
            // a click anywhere else leaves the line and goes on to the game
            self.online.remotes.chat.blur();
        }
        // in another player's bus: a passenger, whose clicks work nothing of it (they
        // went to the driver's game, which worked its switches for them)
        #[cfg(feature = "lan")]
        if self.view == "foot" && self.online.inside_remote.is_some() {
            return;
        }
        // a page (`[htmltexture]`) on a scenery object: pressed and released like the bus's own
        if self.html_object_click(pressed) {
            return;
        }
        // on foot: the own bus's switches, doors and flaps from inside it or standing by it
        if self.view == "foot" && !self.foot_reaches_bus() {
            return;
        }
        #[cfg(windows)]
        if self.vr.is_some()
            && self.mouse_drive
            && self.game_menu.is_none()
            && matches!(self.view.as_str(), "driver" | "pax")
        {
            if !pressed {
                if let Some(player) = self.player.as_mut() {
                    player.release();
                }
                self.dragging = false;
            }
            return;
        }
        let ray = self
            .camera
            .as_ref()
            .zip(self.surface.as_ref())
            .map(|(cam, s)| self.cockpit_cursor_ray(cam, (s.config.width, s.config.height)));
        if let (Some(p), Some((o, d, spread))) = (self.player.as_mut(), ray) {
            let (dx, dy) = std::mem::take(&mut self.drag_delta);
            if pressed {
                if let Some((page, u, v)) = p.html_hit(o, d) {
                    p.release();
                    p.html_pointer(page, u, v, omsi_sim::htmltex::PointerKind::Down);
                    self.html_pressed = Some((page, u, v));
                    self.dragging = false;
                    return;
                }
                // a tear-off ticket block: a ticket of its type torn off for the passenger
                if let Some(n) = self
                    .humans
                    .as_ref()
                    .and_then(|h| h.ticket_blocks.as_ref())
                    .and_then(|b| b.hit(o, d, &p.vehicle))
                {
                    log::info!("ticket block {n}: a ticket torn off");
                    p.vehicle.set_engine_var("GivenTicket", n as f32);
                    self.dragging = false;
                    return;
                }
                self.dragging = p.click(o, d, spread).is_some();
            } else {
                if let Some((page, u, v)) = self.html_pressed.take() {
                    let (u, v) = p
                        .html_hit(o, d)
                        .filter(|h| h.0 == page)
                        .map_or((u, v), |h| (h.1, h.2));
                    p.html_pointer(page, u, v, omsi_sim::htmltex::PointerKind::Up);
                    self.dragging = false;
                    return;
                }
                // CursorMoved and the release can arrive between redraws. Deliver the
                // last movement before `_off`, so a short adjustment is not lost or
                // mistaken for a stationary click on a drag-only control.
                if self.dragging && (dx != 0.0 || dy != 0.0) {
                    p.drag(dx, dy);
                }
                if self.dragging && self.buttons_held.1 {
                    p.release_keeping();
                } else {
                    p.release();
                }
                self.dragging = false;
            }
        }
    }

    /// A click on a page of a scenery object (`[htmltexture]` in its model). True when the
    /// click was the page's: the press lands on it, the release goes to it wherever the
    /// pointer is by then.
    pub(crate) fn html_object_click(&mut self, pressed: bool) -> bool {
        let Some(w) = self.world.clone() else {
            return false;
        };
        if !pressed {
            let Some((id, page, u, v)) = self.html_object_pressed.take() else {
                return false;
            };
            let (u, v) = self
                .cursor_ray_now()
                .and_then(|(o, d, _)| w.html_object_hit(o, d, HTML_OBJECT_REACH))
                .filter(|h| h.map_id == id && h.page == page)
                .map_or((u, v), |h| (h.u, h.v));
            w.html_object_pointer(id, page, u, v, omsi_sim::htmltex::PointerKind::Up);
            self.dragging = false;
            return true;
        }
        // (driving with the VR pointer: the clicks are the bus's)
        #[cfg(windows)]
        if self.vr.is_some()
            && self.mouse_drive
            && self.game_menu.is_none()
            && matches!(self.view.as_str(), "driver" | "pax")
        {
            return false;
        }
        let Some((o, d, _)) = self.cursor_ray_now() else {
            return false;
        };
        let Some(h) = w.html_object_hit(o, d, HTML_OBJECT_REACH) else {
            return false;
        };
        // the bus in front of the page (a switch, a window, its own page) takes the click
        if self
            .player
            .as_ref()
            .and_then(|p| p.body_hit(o, d))
            .is_some_and(|t| t < h.t)
        {
            return false;
        }
        if let Some(p) = self.player.as_mut() {
            p.release();
        }
        w.html_object_pointer(
            h.map_id,
            h.page,
            h.u,
            h.v,
            omsi_sim::htmltex::PointerKind::Down,
        );
        self.html_object_pressed = Some((h.map_id, h.page, h.u, h.v));
        self.dragging = false;
        true
    }

    /// The ray under the cursor now (see [`Self::cockpit_cursor_ray`]).
    pub(crate) fn cursor_ray_now(&self) -> Option<(glam::DVec3, glam::Vec3, f32)> {
        let (cam, s) = self.camera.as_ref().zip(self.surface.as_ref())?;
        Some(self.cockpit_cursor_ray(cam, (s.config.width, s.config.height)))
    }

    /// A switch held with the mouse: OMSI runs its `<event>_drag` trigger every frame the
    /// button is down, with this frame's movement in `mouse_x` / `mouse_y` - 0 while the
    /// hand keeps still. The scripts rely on that: the EN92 cash desk takes its swing speed
    /// from the last two positions, and fired only on movement it kept the speed of the last
    /// small move through a pause and swung shut when let go; the door scripts set their
    /// push once per trigger.
    ///
    /// The redraw path may inline this after `Player::tick` (field borrow of `player`); keep
    /// the helper for any call site that does not already hold `self.player`.
    #[allow(dead_code)] // inlined in `app_events` redraw while `player` is borrowed
    pub(crate) fn drag_frame(&mut self) {
        if !self.dragging {
            return;
        }
        let (dx, dy) = std::mem::take(&mut self.drag_delta);
        if let Some(p) = self.player.as_mut() {
            p.drag(dx, dy);
        }
    }

    /// The open drop-down's scroll bar held with the mouse at height `y`: the first entry
    /// shown follows the thumb.
    pub(crate) fn drag_dropdown(&mut self, y: f32) {
        let (Some(grab), Some(ui)) = (self.dd_scroll_drag, self.ui.as_ref()) else {
            return;
        };
        let Some((track, thumb)) = ui.dd_scroll else {
            return;
        };
        let rows = ui.dd_rows.max(1);
        let Some(d) = self.dropdown.as_mut() else {
            return;
        };
        d.top = dropdown_top_at(y - grab, track, thumb[3] - thumb[1], d.items.len(), rows);
    }

    /// The scroll bar of the timetable beside the tours held with the mouse at height `y`:
    /// the first stop shown follows the thumb.
    pub(crate) fn drag_pane(&mut self, y: f32) {
        let (Some(grab), Some(k)) = (self.pane_scroll_drag, self.chooser) else {
            return;
        };
        let Some((track, thumb, n, fit)) = self.ui.as_ref().and_then(|u| u.menu_pane_scroll) else {
            return;
        };
        self.pane_scroll = Some((
            k,
            dropdown_top_at(y - grab, track, thumb[3] - thumb[1], n, fit),
        ));
    }

    /// The mouse wheel over the game menu: the chosen line moves (the menu scrolls with it),
    /// in a list the same; no wrapping round.
    pub(crate) fn menu_wheel(&mut self, amount: f32) {
        // (an open drop-down scrolls, not the window under it)
        if self.dropdown.is_some() {
            self.wheel_acc += amount;
            let steps = self.wheel_acc.trunc() as i64;
            if steps == 0 {
                return;
            }
            self.wheel_acc -= steps as f32;
            let rows = self.ui.as_ref().map(|u| u.dd_rows).unwrap_or(8);
            if let Some(d) = self.dropdown.as_mut() {
                let max = d.items.len().saturating_sub(rows) as i64;
                d.top = (d.top as i64 - steps).clamp(0, max) as usize;
            }
            return;
        }
        // (the new menu has no list to scroll)
        if self.new_menu_active() {
            return;
        }
        self.wheel_acc += amount;
        let steps = self.wheel_acc.trunc() as i64;
        if steps == 0 {
            return;
        }
        self.wheel_acc -= steps as f32;
        // the wheel over the timetable beside the tours scrolls its stops
        if let (Some(u), Some(k)) = (self.ui.as_ref(), self.chooser) {
            let (x, y) = self.cursor;
            if u.menu_pane_box
                .is_some_and(|r| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3])
            {
                let first = (u.menu_pane_start as i64 - steps).max(0) as usize;
                self.pane_scroll = Some((k, first));
                return;
            }
        }
        // the list scrolls under the mouse; what is chosen stays chosen (the wheel used to
        // walk the highlight up and down the lines)
        let n = self.menu_len() as f32;
        let (start, rows) = self
            .ui
            .as_ref()
            .map(|u| (u.menu_start as f32, u.menu_rows as f32))
            .unwrap_or((0.0, n));
        let top = (self.menu_top.unwrap_or(start) - steps as f32).clamp(0.0, (n - rows).max(0.0));
        self.menu_top = Some(top);
    }

    /// Switch the mouse steering on or off, and remember it for the next game. Switched off,
    /// the wheel stays where the mouse left it; switched on, it eases from where it is to
    /// the cursor for the first second.
    pub(crate) fn set_mouse_drive(&mut self, on: bool) {
        self.mouse_drive = on;
        if on {
            // O can be pressed while the pointer is anywhere in the window. Start mouse
            // steering from the neutral cursor position instead of applying that offset
            // to the wheel on the first frame.
            self.center_cursor = true;
        }
        if !on {
            crate::player::keep_wheel(self.player.as_mut());
            // the brake the mouse held stays on, as the brake key leaves it (OMSI has one
            // brake for both): the bus rolled off when the mouse let go of it (#517, #760)
            if let Some(p) = self.player.as_mut() {
                p.axes.brake = p.axes.brake.max(self.mouse_pedals.1);
            }
            #[cfg(windows)]
            self.reset_vr_pointer();
        }
        self.mouse_steer = (
            self.player
                .as_ref()
                .map(|p| p.vehicle.controls().steering)
                .unwrap_or(0.0),
            1.0,
        );
        self.mouse_pedals = self
            .player
            .as_ref()
            .map(|p| (p.vehicle.controls().throttle, p.vehicle.controls().brake))
            .unwrap_or((0.0, 0.0));
        if self.settings.mouse_steering != on {
            self.settings.mouse_steering = on;
            crate::game_lists::remember_setting("mouse_steering", if on { "1" } else { "0" });
        }
    }

    pub(crate) fn hud_size(&self) -> (f32, f32) {
        let v = self
            .surface
            .as_ref()
            .map(|s| {
                self.settings
                    .hud_viewport((s.config.width, s.config.height))
            })
            .unwrap_or([0.0, 0.0, 1.0, 1.0]);
        (v[2], v[3])
    }

    pub(crate) fn hud_cursor(&self) -> (f32, f32) {
        let x = self
            .surface
            .as_ref()
            .map(|s| {
                self.settings
                    .hud_viewport((s.config.width, s.config.height))[0]
            })
            .unwrap_or(0.0);
        (self.cursor.0 - x, self.cursor.1)
    }

    pub(crate) fn cockpit_cursor_ray(
        &self,
        cam: &Camera,
        size: (u32, u32),
    ) -> (glam::DVec3, glam::Vec3, f32) {
        #[cfg(windows)]
        if let Some(ray) = self
            .vr
            .as_ref()
            .and_then(|vr| vr.cursor_ray(self.cursor.0, self.cursor.1, size))
        {
            return (ray.0, ray.1, ray.2 * 6.0);
        }
        if let Some(rig) = self.triple_rig(size) {
            let (o, d, spread) = rig.cursor_ray(cam, self.cursor, size);
            return (o, d, spread * 6.0);
        }
        let (o, d) = cursor_ray(
            cam,
            self.cursor.0,
            self.cursor.1,
            size.0 as f32,
            size.1 as f32,
        );
        (o, d, pixel_angle(cam, size.1 as f32) * 6.0)
    }

    /// The triple screen's rig as drawn this frame (the view's zoom applied), while it is on
    /// and no headset is asked for.
    pub(crate) fn triple_rig(&self, size: (u32, u32)) -> Option<omsi_render::TripleScreen> {
        (self.settings.triple.enabled && !self.settings.vr_requested()).then(|| {
            self.settings.triple.zoomed(
                size.0,
                size.1,
                self.view_zoom.get(&self.view).copied().unwrap_or(1.0),
            )
        })
    }

    /// The desktop ray under the cursor for placing and editing on the ground: the window's
    /// own projection, or the triple screen panel's under the cursor.
    pub(crate) fn world_cursor_ray(
        &self,
        cam: &Camera,
        size: (u32, u32),
    ) -> (glam::DVec3, glam::Vec3) {
        match self.triple_rig(size) {
            Some(rig) => {
                let (o, d, _) = rig.cursor_ray(cam, self.cursor, size);
                (o, d)
            }
            None => cursor_ray(
                cam,
                self.cursor.0,
                self.cursor.1,
                size.0 as f32,
                size.1 as f32,
            ),
        }
    }

    /// With a triple screen, the frustum around all three panels for "nothing appears or
    /// vanishes in sight": tangents of its half-angles, horizontal and vertical (None: the
    /// window's own view is the whole picture).
    pub(crate) fn sight_extent(&self, cam: &Camera, size: (u32, u32)) -> Option<(f64, f64)> {
        let rig = self.triple_rig(size)?;
        let (tx, ty) = rig.view_extent(cam, size.0, size.1);
        Some((tx as f64, ty as f64))
    }

    pub(crate) fn update_hover(&mut self) {
        if self.vr_nav_edit.is_some() || self.cursor_hidden.is_some() {
            self.hover = None;
            self.hover_part = None;
            self.hover_hand = false;
            return;
        }
        #[cfg(windows)]
        if !self.mouse_drive
            && self.vr.as_ref().is_some_and(|vr| {
                vr.needs_cursor_surface(
                    self.cursor,
                    self.game_menu.is_some() || self.chooser.is_some(),
                )
            })
        {
            let surface = self
                .player
                .as_ref()
                .zip(self.camera.as_ref())
                .zip(self.surface.as_ref())
                .filter(|_| matches!(self.view.as_str(), "driver" | "pax"))
                .map(|((player, camera), window)| {
                    let (origin, direction, _) = self
                        .cockpit_cursor_ray(camera, (window.config.width, window.config.height));
                    (
                        player.surface_hit(origin, direction),
                        (player.vehicle.position, player.vehicle.body_rotation()),
                    )
                });
            if let Some(vr) = self.vr.as_mut() {
                vr.set_cursor_surface(surface.as_ref().and_then(|s| s.0), surface.map(|s| s.1));
            }
        }
        let found = match (
            self.player.as_ref(),
            self.camera.as_ref(),
            self.surface.as_ref(),
        ) {
            (Some(p), Some(cam), Some(s))
                if self.view != "free"
                    && (self.view != "foot" || self.foot_reaches_bus())
                    && !(self.vr_active()
                        && self.mouse_drive
                        && matches!(self.view.as_str(), "driver" | "pax")) =>
            {
                let (o, d, spread) =
                    self.cockpit_cursor_ray(cam, (s.config.width, s.config.height));
                p.hovered_part(o, d, spread)
            }
            // (in another player's bus nothing is offered: its switches are the driver's)
            _ => (None, false),
        };
        let (found, hand) = found;
        self.hover_hand = hand;
        match found {
            Some((name, true)) => {
                self.hover = Some(name);
                self.hover_part = None;
            }
            Some((name, false)) => {
                self.hover = None;
                self.hover_part = Some(name);
            }
            None => {
                self.hover = None;
                self.hover_part = None;
            }
        }
        // the cursor itself says when it is over something that can be operated
        // (steering with the mouse: a cross, as OMSI shows it; turning the view with the
        // right button held: the four arrows OMSI shows then, #185)
        // (zooming with the mouse: the up-down arrows, Omsi's crSizeNS)
        // SIZENS only while the right button really zooms (with `alt_view`
        // it turns the view instead, and keeps the four arrows).
        let rmb_zoom = self.buttons_held.1
            && !self.mmb_held
            && !self.settings.alt_view
            && self.player.is_some()
            && self.both_drag.is_none()
            && matches!(self.view.as_str(), "driver" | "outside" | "pax" | "free");
        let kind: u8 = if self.both_drag.is_some() && self.game_menu.is_none() {
            4
        } else if rmb_zoom && self.game_menu.is_none() {
            4
        } else if self.mouse_look && self.game_menu.is_none() {
            3
        } else if self.mouse_drive && self.mouse_steers_in_view() && self.game_menu.is_none() {
            2
        } else if self.game_menu.is_some() {
            // (the game menu's own cursor: not overwritten here, or it flips back and forth)
            self.menu_cursor_kind()
        } else if self.hover.is_some() || self.hover_hand {
            1
        } else {
            0
        };
        self.set_cursor_kind(kind);
    }

    /// Show the mouse cursor `kind` (0 arrow, 1 pointing hand, 2 cross, 3 arrows, 4 closed hand).
    pub(crate) fn set_cursor_kind(&mut self, kind: u8) {
        if kind != self.cursor_kind {
            self.cursor_kind = kind;
            if let Some(w) = self.window.as_ref() {
                w.set_cursor(match kind {
                    4 => winit::window::CursorIcon::NsResize,
                    3 => winit::window::CursorIcon::Move,
                    2 => winit::window::CursorIcon::Crosshair,
                    1 => winit::window::CursorIcon::Pointer,
                    _ => winit::window::CursorIcon::Default,
                });
            }
        }
    }

    /// The cursor over the open game menu: a pointing hand over what can be clicked (a line,
    /// a page, a control, the scroll bar), the closed hand while a slider or the scroll bar
    /// is held.
    pub(crate) fn menu_cursor_kind(&self) -> u8 {
        if self.menu_drag.is_some()
            || self.menu_scroll_drag
            || self.dd_scroll_drag.is_some()
            || self.pane_scroll_drag.is_some()
        {
            return 4;
        }
        if self.new_menu_active() {
            // (the new menu is drawn by the renderer: its lines are not the HUD's rectangles)
            return u8::from(self.new_menu_hit().is_some());
        }
        // (a page open: the Esc menu's column is drawn beside it, and its lines can be clicked)
        if self.new_pages_active()
            && self.game_menu.is_some()
            && self.dropdown.is_none()
            && self.new_menu_hit().is_some()
        {
            return 1;
        }
        let Some(u) = self.ui.as_ref() else { return 0 };
        let (x, y) = self.cursor;
        let inside = |r: &[f32; 4]| x >= r[0] && x <= r[2] && y >= r[1] && y <= r[3];
        let clickable = u.menu_scroll_thumb.is_some_and(|r| inside(&r))
            || u.menu_side.iter().any(|r| inside(r))
            || u.menu_pane.iter().any(|r| inside(r))
            || u.menu_pane_go.as_ref().is_some_and(|r| inside(r))
            || u.menu_time.iter().any(|r| inside(r))
            || u.menu_ctl.iter().flatten().any(|r| inside(r))
            || u.menu_rects
                .iter()
                .enumerate()
                .any(|(i, r)| inside(r) && !self.menu_item_off(i + u.menu_start));
        if clickable {
            1
        } else {
            0
        }
    }
}
