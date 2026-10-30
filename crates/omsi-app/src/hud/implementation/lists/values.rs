//! The settings' values as the windows read and change them.

use super::chooser::LIST_DIRTY;
use super::rows::{step_move, steps_of};
use super::select::sync_live;
use super::store::{remember_bus_seat, remember_setting};
use crate::core::app::App;
use crate::hud::domain::lists::{Move, CLOUD_TYPES, PRECIP_KINDS};

/// The index of the cloud type `kind` (a weather file's) in `CLOUD_TYPES`.
pub(in crate::hud) fn cloud_index(kind: &str) -> Option<usize> {
    let k = kind.trim();
    CLOUD_TYPES.iter().position(|(id, _)| {
        id.eq_ignore_ascii_case(k) || (*id == "-1" && (k.is_empty() || k.starts_with("-1")))
    })
}

/// Set the kind of precipitation (an index of `PRECIP_KINDS`) of the weather set by hand.
pub(in crate::hud) fn custom_state(app: &App) -> crate::weather_setup::CustomWeather {
    if let Some(mut c) = crate::weather_setup::custom_weather(app.args.weather.as_deref()) {
        // Wetness keeps evolving while driving; never restore an old serialized value just
        // because another custom field (brightness, humidity, etc.) was edited.
        c.road_wetness = app.wetness;
        return c;
    }
    match app.weather.as_ref() {
        Some(w) => crate::weather_setup::CustomWeather::from_weather(w, 1.0, app.wetness),
        None => crate::weather_setup::CustomWeather::default(),
    }
}

pub(crate) fn set_precip(app: &mut App, to: usize) {
    let to = to.min(PRECIP_KINDS.len() - 1);
    app.edit_weather(|w| {
        w.precip[0] = to as f32;
        w.snow = to == 2;
        // (rain or snow with no strength would be nothing: a moderate one)
        if to != 0 && w.precip[1] < 1.0 {
            w.precip[1] = 100.0;
        }
    });
}

/// Keep the physical triple-screen calibration separate from the single-screen FOV.
pub fn set_camera_fov(
    settings: &mut crate::settings::Settings,
    value: f32,
) -> (&'static str, String) {
    let value = if value < 20.0 {
        0.0
    } else {
        value.round().min(120.0)
    };
    if settings.triple.enabled && !settings.vr_requested() {
        settings.triple.fov_deg = value;
        ("triple_fov_deg", value.to_string())
    } else {
        settings.fov = value;
        ("fov", value.to_string())
    }
}

/// The value of the slider setting `verb` (`arg`: the seat's axis).
pub(in crate::hud) fn option_now(app: &App, verb: &str, arg: &str) -> Option<f32> {
    if let Some(field) = verb.strip_prefix("vr_nav_") {
        return if app.vr_active() && app.player.is_some() {
            app.vr_nav_profile().value(field)
        } else {
            None
        };
    }
    let s = &app.settings;
    Some(match verb {
        "speed" => s.time_speed as f32,
        "traffic" => app.traffic.as_ref()?.target as f32,
        "pax" => s.pax_density,
        "volume" => s.volume,
        "led_glow" => s.led_glow as f32,
        "night_brightness" => s.night_brightness,
        "led_mips" => s.led_mips,
        "ctrl_deadzone" => s.ctrl_deadzone,
        "pedal_t" => s.pedal_throttle,
        "pedal_b" => s.pedal_brake,
        "mouse_sens" => s.mouse_sens,
        "look_sens" => s.look_sens,
        "look_smoothing_ms" => s.look_smoothing_ms,
        "pad_steer_smooth" => s.pad_steer_smooth,
        "ui_scale" => s.ui_scale,
        "chat_size" => s.chat_size,
        "ui_opacity" => s.ui_opacity,
        "vol_ai" => s.vol_ai,
        "vol_scenery" => s.vol_scenery,
        "wheel_range" => s.wheel_range,
        "wheel_lock" => s.wheel_lock,
        "triple_width_mm" => s.triple.width_mm,
        "triple_distance_mm" => s.triple.distance_mm,
        "triple_bezel_mm" => s.triple.bezel_mm,
        "triple_left_angle_deg" => s.triple.left_angle_deg,
        "triple_right_angle_deg" => s.triple.right_angle_deg,
        "triple_eye_height_mm" => s.triple.eye_height_mm,
        "fov" => {
            if s.triple.enabled && !s.vr_requested() {
                s.triple.fov_deg
            } else {
                s.fov
            }
        }
        "outside_fov" => s.outside_fov,
        "steer_look_angle" => s.steer_look_angle,
        "steer_look_response" => s.steer_look_response,
        "head_idle" => s.head_idle,
        "head_idle_pace" => s.head_idle_pace,
        "head_tracking_yaw_sens" => s.head_tracking_yaw_sens,
        "head_tracking_pitch_sens" => s.head_tracking_pitch_sens,
        "head_tracking_roll_sens" => s.head_tracking_roll_sens,
        "head_tracking_x_sens" => s.head_tracking_x_sens,
        "head_tracking_y_sens" => s.head_tracking_y_sens,
        "head_tracking_z_sens" => s.head_tracking_z_sens,
        "seat" => s.seat[arg.trim().parse::<usize>().unwrap_or(0).min(2)],
        "seat_pitch" => s.seat_pitch_deg,
        "hour" => ((app.clock.time / 3600.0) as i64).rem_euclid(24) as f32,
        "minute" => (((app.clock.time / 60.0) as i64) % 60) as f32,
        "visibility" => app.weather.as_ref()?.fog.0,
        "rain_amt" => {
            let w = app.weather.as_ref()?;
            if w.precip.first().copied().unwrap_or(0.0) < 0.5 {
                0.0
            } else {
                (w.precip.get(1).copied().unwrap_or(0.0) / 255.0).clamp(0.0, 1.0)
            }
        }
        "wet" => app.wetness,
        "brightness" => custom_state(app).brightness,
        "humidity" => custom_state(app).humidity,
        "temp" => app.weather.as_ref()?.temp.0,
        "wind_speed" => app.weather.as_ref()?.wind.1,
        "wind_dir" => app.weather.as_ref()?.wind.0.rem_euclid(360.0),
        _ => return None,
    })
}

/// Set the slider setting `verb` to `v`; the key and value to keep for the next game.
pub(in crate::hud) fn option_set(
    app: &mut App,
    verb: &str,
    arg: &str,
    v: f32,
) -> Option<(&'static str, String)> {
    if let Some(field) = verb.strip_prefix("vr_nav_") {
        app.vr_nav_set(field, v);
        return None; // Stored per bus, never in the desktop settings file.
    }
    match verb {
        "speed" => {
            app.settings.time_speed = v as f64;
            Some(("time_speed", app.settings.time_speed.to_string()))
        }
        "traffic" => {
            if let Some(t) = app.traffic.as_mut() {
                t.target = v.round() as usize;
                app.args.traffic = t.target;
            }
            None
        }
        "pax" => {
            app.settings.pax_density = v;
            Some(("pax_density", v.to_string()))
        }
        "volume" => {
            app.settings.volume = v;
            Some(("volume", v.to_string()))
        }
        "led_glow" => {
            app.settings.led_glow = v.round() as _;
            Some(("led_glow", app.settings.led_glow.to_string()))
        }
        "night_brightness" => {
            app.settings.night_brightness = v.clamp(0.0, 3.0);
            Some((
                "night_brightness",
                app.settings.night_brightness.to_string(),
            ))
        }
        "led_mips" => {
            app.settings.led_mips = v.clamp(0.0, 4.0);
            Some(("led_mips", app.settings.led_mips.to_string()))
        }
        "ctrl_deadzone" => {
            app.settings.ctrl_deadzone = v.clamp(0.0, 0.3);
            Some(("ctrl_deadzone", app.settings.ctrl_deadzone.to_string()))
        }
        "pedal_t" => {
            app.settings.pedal_throttle = v;
            Some(("pedal_throttle", v.to_string()))
        }
        "pedal_b" => {
            app.settings.pedal_brake = v;
            Some(("pedal_brake", v.to_string()))
        }
        "look_sens" => {
            app.settings.look_sens = (v * 100.0).round() / 100.0;
            Some(("look_sens", app.settings.look_sens.to_string()))
        }
        "pad_steer_smooth" => {
            app.settings.pad_steer_smooth = v.clamp(0.0, 300.0).round();
            Some((
                "pad_steer_smooth",
                app.settings.pad_steer_smooth.to_string(),
            ))
        }
        "look_smoothing_ms" => {
            app.settings.look_smoothing_ms = v.round();
            Some((
                "look_smoothing_ms",
                app.settings.look_smoothing_ms.to_string(),
            ))
        }
        "mouse_sens" => {
            app.settings.mouse_sens = (v * 100.0).round() / 100.0;
            Some(("mouse_sens", app.settings.mouse_sens.to_string()))
        }
        "ui_scale" => {
            app.settings.ui_scale = (v * 100.0).round() / 100.0;
            Some(("ui_scale", app.settings.ui_scale.to_string()))
        }
        "chat_size" => {
            app.settings.chat_size = ((v * 10.0).round() / 10.0).clamp(0.5, 3.0);
            Some(("chat_size", app.settings.chat_size.to_string()))
        }
        "ui_opacity" => {
            app.settings.ui_opacity = (v * 100.0).round() / 100.0;
            Some(("ui_opacity", app.settings.ui_opacity.to_string()))
        }
        "vol_ai" => {
            app.settings.vol_ai = (v * 100.0).round() / 100.0;
            Some(("vol_ai", app.settings.vol_ai.to_string()))
        }
        "vol_scenery" => {
            app.settings.vol_scenery = (v * 100.0).round() / 100.0;
            Some(("vol_scenery", app.settings.vol_scenery.to_string()))
        }
        "wheel_range" => {
            app.settings.wheel_range = v.round();
            Some(("wheel_range", app.settings.wheel_range.to_string()))
        }
        "wheel_lock" => {
            app.settings.wheel_lock = if v < 45.0 { 0.0 } else { v.round() };
            Some(("wheel_lock", app.settings.wheel_lock.to_string()))
        }
        "triple_width_mm" => {
            app.settings.triple.width_mm = v.clamp(200.0, 2000.0);
            Some(("triple_width_mm", app.settings.triple.width_mm.to_string()))
        }
        "triple_distance_mm" => {
            app.settings.triple.fov_deg = 0.0;
            remember_setting("triple_fov_deg", "0");
            app.settings.triple.distance_mm = v.clamp(200.0, 3000.0);
            Some((
                "triple_distance_mm",
                app.settings.triple.distance_mm.to_string(),
            ))
        }
        "triple_bezel_mm" => {
            app.settings.triple.bezel_mm = v.clamp(0.0, 100.0);
            Some(("triple_bezel_mm", app.settings.triple.bezel_mm.to_string()))
        }
        "triple_left_angle_deg" => {
            app.settings.triple.left_angle_deg = v.clamp(0.0, 90.0);
            Some((
                "triple_left_angle_deg",
                app.settings.triple.left_angle_deg.to_string(),
            ))
        }
        "triple_right_angle_deg" => {
            app.settings.triple.right_angle_deg = v.clamp(0.0, 90.0);
            Some((
                "triple_right_angle_deg",
                app.settings.triple.right_angle_deg.to_string(),
            ))
        }
        "triple_eye_height_mm" => {
            app.settings.triple.eye_height_mm = v.clamp(-500.0, 500.0);
            Some((
                "triple_eye_height_mm",
                app.settings.triple.eye_height_mm.to_string(),
            ))
        }
        "fov" => Some(set_camera_fov(&mut app.settings, v)),
        "outside_fov" => {
            app.settings.outside_fov = if v < 20.0 { 0.0 } else { v.round().min(120.0) };
            Some(("outside_fov", app.settings.outside_fov.to_string()))
        }
        "steer_look_angle" => {
            app.settings.steer_look_angle = v.round();
            Some((
                "steer_look_angle",
                app.settings.steer_look_angle.to_string(),
            ))
        }
        "steer_look_response" => {
            app.settings.steer_look_response = (v * 100.0).round() / 100.0;
            Some((
                "steer_look_response",
                app.settings.steer_look_response.to_string(),
            ))
        }
        "head_idle" => {
            app.settings.head_idle = (v * 100.0).round() / 100.0;
            Some(("head_idle", app.settings.head_idle.to_string()))
        }
        "head_idle_pace" => {
            app.settings.head_idle_pace = (v * 100.0).round() / 100.0;
            Some(("head_idle_pace", app.settings.head_idle_pace.to_string()))
        }
        // the seat: kept for this bus (`bus_seats`), not for every bus (#1355)
        "head_tracking_yaw_sens" => {
            app.settings.head_tracking_yaw_sens = v.round().clamp(0.0, 100.0);
            Some((
                "head_tracking_yaw_sens",
                app.settings.head_tracking_yaw_sens.to_string(),
            ))
        }
        "head_tracking_pitch_sens" => {
            app.settings.head_tracking_pitch_sens = v.round().clamp(0.0, 100.0);
            Some((
                "head_tracking_pitch_sens",
                app.settings.head_tracking_pitch_sens.to_string(),
            ))
        }
        "head_tracking_roll_sens" => {
            app.settings.head_tracking_roll_sens = v.round().clamp(0.0, 100.0);
            Some((
                "head_tracking_roll_sens",
                app.settings.head_tracking_roll_sens.to_string(),
            ))
        }
        "head_tracking_x_sens" => {
            app.settings.head_tracking_x_sens = v.round().clamp(0.0, 100.0);
            Some((
                "head_tracking_x_sens",
                app.settings.head_tracking_x_sens.to_string(),
            ))
        }
        "head_tracking_y_sens" => {
            app.settings.head_tracking_y_sens = v.round().clamp(0.0, 100.0);
            Some((
                "head_tracking_y_sens",
                app.settings.head_tracking_y_sens.to_string(),
            ))
        }
        "head_tracking_z_sens" => {
            app.settings.head_tracking_z_sens = v.round().clamp(0.0, 100.0);
            Some((
                "head_tracking_z_sens",
                app.settings.head_tracking_z_sens.to_string(),
            ))
        }
        "seat" => {
            let k: usize = arg.trim().parse().unwrap_or(0).min(2);
            app.settings.seat[k] = (v * 100.0).round() / 100.0;
            remember_bus_seat(app);
            None
        }
        "seat_pitch" => {
            app.settings.seat_pitch_deg = v.clamp(-45.0, 45.0).round();
            remember_bus_seat(app);
            None
        }
        // the clock set directly: the hour or the minute (the seconds stay)
        "hour" | "minute" => {
            if app.is_lan_client() {
                app.service_msg = Some(("In a LAN session the host sets the clock".into(), 3.0));
                return None;
            }
            let t = app.clock.time;
            let (h, m) = (
                ((t / 3600.0) as i64).rem_euclid(24),
                ((t / 60.0) as i64) % 60,
            );
            let (h, m) = if verb == "hour" {
                (v.round() as i64, m)
            } else {
                (h, v.round() as i64)
            };
            let target = (h * 3600 + m * 60) as f64 + t % 60.0;
            app.shift_clock(target - t);
            None
        }
        // the weather, made by hand (what the preset was stays as it was but for this)
        "visibility" => {
            app.edit_weather(|w| w.fog.0 = v);
            None
        }
        "rain_amt" => {
            app.edit_weather(|w| {
                w.precip[1] = (v * 255.0).round();
                if v > 0.0 && w.precip[0] < 0.5 {
                    w.precip[0] = 1.0;
                }
            });
            None
        }
        "wet" => {
            let mut c = custom_state(app);
            c.road_wetness = v;
            app.set_custom_weather(c);
            None
        }
        "brightness" => {
            let mut c = custom_state(app);
            c.brightness = v;
            app.set_custom_weather(c);
            None
        }
        "humidity" => {
            let mut c = custom_state(app);
            c.humidity = v;
            app.set_custom_weather(c);
            None
        }
        "temp" => {
            app.edit_weather(|w| w.temp.0 = v);
            None
        }
        "wind_speed" => {
            app.edit_weather(|w| w.wind.1 = v);
            None
        }
        "wind_dir" => {
            app.edit_weather(|w| w.wind.0 = v);
            None
        }
        _ => None,
    }
}

/// Whether the switch `id` is on (None: `id` is no switch).
pub(in crate::hud) fn toggle_now(app: &App, id: &str) -> Option<bool> {
    let s = &app.settings;
    Some(match id {
        "navigator" => {
            if app.vr_active() {
                app.vr_nav_profile().enabled
            } else {
                app.navigator.as_ref().is_some_and(|n| n.enabled)
            }
        }
        "nav_ai" => app.navigator.as_ref().map_or(s.nav_ai, |n| n.show_ai),
        "nav_follow_view" => s.nav_follow_view,
        "nav_surface" => app
            .navigator
            .as_ref()
            .map_or(s.nav_surface, |n| n.show_surface),
        "nav_light" => app.navigator.as_ref().map_or(s.nav_light, |n| n.light),
        "nav_contours" => app
            .navigator
            .as_ref()
            .map_or(s.nav_contours, |n| n.show_contours),
        "shadows" => s.shadows,
        "head" => s.head_movement,
        "cam_smooth" => s.driverview_smooth,
        "coll_objects" => s.collision_objects,
        "coll_vehicles" => s.collision_vehicles,
        "mouse" => app.mouse_drive,
        "mouse_right" => s.mouse_right_off,
        "mouse_smooth" => s.mouse_smooth,
        "blinker_cancel" => s.blinker_cancel,
        "fps" => s.show_fps,
        "get_up" => s.get_up,
        "time_sync" => s.time_sync,
        "metar_sync" => s.metar_sync,
        "snow_cover" => app.weather.as_ref().is_some_and(|w| w.snow),
        "snow_road" => app.weather.as_ref().is_some_and(|w| w.snow_on_road),
        "camcoll" => s.camera_collision,
        "steer_look" => s.steer_look,
        "hands_in_cab" => s.hands_in_cab,
        "ff" => s.ff_enabled,
        "brake_hold" => s.brake_hold,
        "auto_clutch" => s.auto_clutch,
        "headtrack" => s.head_tracking,
        "head_tracking_invert_yaw" => s.head_tracking_invert_yaw,
        "head_tracking_invert_pitch" => s.head_tracking_invert_pitch,
        "head_tracking_invert_roll" => s.head_tracking_invert_roll,
        "head_tracking_invert_x" => s.head_tracking_invert_x,
        "head_tracking_invert_y" => s.head_tracking_invert_y,
        "head_tracking_invert_z" => s.head_tracking_invert_z,
        "timetable_win" => app.timetable,
        "info_bar" => app.info_bar,
        "info_bar_fps" => s.info_bar_fps,
        k if crate::settings::info_item_index(k).is_some() => {
            s.info_items[crate::settings::info_item_index(k)?]
        }
        "nav_arrows" => app.navigator.as_ref().map_or(s.nav_arrows, |n| n.arrows),
        "exact_fare" => s.exact_fare,
        "collision_pedestrians" => s.collision_pedestrians,
        "ssao" => s.ssao,
        "detail_textures" => s.detail_textures,
        "reflections" => s.reflections,
        "clouds" => s.clouds,
        "windy_trees" => s.windy_trees,
        "dx9_snow" => s.dx9_snow,
        "fullscreen" => s.fullscreen,
        "vsync" => s.vsync,
        "texture_compression" => s.texture_compression,
        "driver" => s.driver,
        "alt_view" => s.alt_view,
        "precision_zoom" => s.precision_zoom,
        "triple_screen" => s.triple.enabled,
        "triple_hud_center" => s.triple_hud_center,
        "triple_span" => s.triple_span,
        "vr" => s.vr,
        "vr_desktop_mirror" => s.vr_desktop_mirror,
        "doppler" => s.doppler,
        "muffle_outside" => s.muffle_outside,
        "steering_linear" => s.steering_linear,
        "pad_steer_linear" => s.pad_steer_linear,
        "arrows_switch_cams" => s.arrows_switch_cams,
        "old_steering" => s.old_steering,
        "red_steer_spd" => s.red_steer_spd,
        "momentary_gears" => s.momentary_gears,
        "auto_shift" => s.auto_shift,
        "ff_invert" => s.ff_invert,
        "ui_scale_window" => s.ui_scale_window,
        "tooltips" => s.tooltips,
        "notes" => s.notes,
        "chat" => s.chat,
        "name_tags" => s.name_tags,
        _ => return None,
    })
}

/// Switch `id` on or off; the key and value to keep for the next game.
pub(in crate::hud) fn toggle_set(
    app: &mut App,
    id: &str,
    on: bool,
) -> Option<(&'static str, String)> {
    let bit = (on as u8).to_string();
    match id {
        "navigator" => {
            if app.vr_active() {
                if app.vr_nav_profile().enabled != on {
                    app.vr_nav_adjust("enabled", 1.0);
                }
                return None;
            }
            if let Some(n) = app.navigator.as_mut() {
                n.enabled = on;
            }
            app.settings.navigator = on;
            Some(("navigator", bit))
        }
        "nav_ai" => {
            if let Some(n) = app.navigator.as_mut() {
                n.show_ai = on;
            }
            app.settings.nav_ai = on;
            Some(("nav_ai", bit))
        }
        "nav_follow_view" => {
            app.settings.nav_follow_view = on;
            Some(("nav_follow_view", bit))
        }
        "nav_surface" => {
            if let Some(n) = app.navigator.as_mut() {
                n.show_surface = on;
            }
            app.settings.nav_surface = on;
            Some(("nav_surface", bit))
        }
        "nav_contours" => {
            if let Some(n) = app.navigator.as_mut() {
                n.show_contours = on;
            }
            app.settings.nav_contours = on;
            Some(("nav_contours", bit))
        }
        "nav_light" => {
            if let Some(n) = app.navigator.as_mut() {
                n.light = on;
            }
            app.settings.nav_light = on;
            Some(("nav_light", bit))
        }
        "shadows" => {
            app.settings.shadows = on;
            Some(("shadows", bit))
        }
        "head" => {
            app.settings.head_movement = on;
            Some(("head_movement", bit))
        }
        "cam_smooth" => {
            app.settings.driverview_smooth = on;
            Some(("driverview_smooth", bit))
        }
        // (at once: stuck under a bridge a map made too low, the bus drives on)
        "coll_objects" => {
            app.settings.collision_objects = on;
            let cw = app.world.as_ref().map(|w| w.collision.lock().clone());
            if let Some(p) = app.player.as_mut() {
                p.vehicle.collision = cw.filter(|_| on);
            }
            Some(("collision_objects", bit))
        }
        "coll_vehicles" => {
            app.settings.collision_vehicles = on;
            Some(("collision_vehicles", bit))
        }
        "mouse" => {
            // (as the O key does it: switched off from the menu, the brake the mouse held
            // stayed behind and the bus rolled away - #517, #760)
            app.set_mouse_drive(on);
            None
        }
        "blinker_cancel" => {
            app.settings.blinker_cancel = on;
            if let Some(p) = app.player.as_mut() {
                p.blinker_cancel = on;
            }
            Some(("blinker_cancel", bit))
        }
        "mouse_right" => {
            app.settings.mouse_right_off = on;
            Some(("mouse_right_off", bit))
        }
        "mouse_smooth" => {
            app.settings.mouse_smooth = on;
            Some(("mouse_smooth", bit))
        }
        "get_up" => {
            app.settings.get_up = on;
            Some(("get_up", bit))
        }
        // the real-time sync: the clock takes the device's date and time at once (a host's
        // clock runs at real time while it is on, at its time speed again after)
        "time_sync" => {
            app.settings.time_sync = on;
            app.sync_real_time();
            Some(("time_sync", bit))
        }
        // the METAR sync: the weather goes over to the report of the nearest airport and
        // cannot be changed while it is on (the cycle and a hand-made weather end with it)
        "metar_sync" => {
            app.settings.metar_sync = on;
            app.metar_rx = None;
            app.metar_once = false;
            app.metar_next = 0.0;
            if on {
                app.weather_cycle = None;
                app.weather_blend = None;
            }
            Some(("metar_sync", bit))
        }
        "snow_cover" => {
            app.edit_weather(|w| w.snow = on);
            None
        }
        "snow_road" => {
            app.edit_weather(|w| w.snow_on_road = on);
            None
        }
        "fps" => {
            app.settings.show_fps = on;
            Some(("show_fps", bit))
        }
        "headtrack" => {
            app.settings.head_tracking = on;
            Some(("head_tracking", bit))
        }
        "head_tracking_invert_yaw" => {
            app.settings.head_tracking_invert_yaw = on;
            Some(("head_tracking_invert_yaw", bit))
        }
        "head_tracking_invert_pitch" => {
            app.settings.head_tracking_invert_pitch = on;
            Some(("head_tracking_invert_pitch", bit))
        }
        "head_tracking_invert_roll" => {
            app.settings.head_tracking_invert_roll = on;
            Some(("head_tracking_invert_roll", bit))
        }
        "head_tracking_invert_x" => {
            app.settings.head_tracking_invert_x = on;
            Some(("head_tracking_invert_x", bit))
        }
        "head_tracking_invert_y" => {
            app.settings.head_tracking_invert_y = on;
            Some(("head_tracking_invert_y", bit))
        }
        "head_tracking_invert_z" => {
            app.settings.head_tracking_invert_z = on;
            Some(("head_tracking_invert_z", bit))
        }
        "camcoll" => {
            app.settings.camera_collision = on;
            Some(("camera_collision", bit))
        }
        "steer_look" => {
            app.settings.steer_look = on;
            Some(("steer_look", bit))
        }
        "hands_in_cab" => {
            app.settings.hands_in_cab = on;
            Some(("hands_in_cab", bit))
        }
        "brake_hold" => {
            app.settings.brake_hold = on;
            Some(("brake_hold", bit))
        }
        "auto_clutch" => {
            app.settings.auto_clutch = on;
            if let Some(p) = app.player.as_mut() {
                p.vehicle.host.auto_clutch = if on { 1.0 } else { 0.0 };
            }
            Some(("auto_clutch", bit))
        }
        "ff" => {
            app.settings.ff_enabled = on;
            Some(("ff_enabled", bit))
        }
        "timetable_win" => {
            app.timetable = on;
            None
        }
        "info_bar" => {
            app.set_info_bar(on);
            None
        }
        "info_bar_fps" => {
            app.settings.info_bar_fps = on;
            Some(("info_bar_fps", bit))
        }
        k if crate::settings::info_item_index(k).is_some() => {
            let i = crate::settings::info_item_index(k)?;
            app.settings.info_items[i] = on;
            Some((crate::settings::INFO_ITEM_KEYS[i], bit))
        }
        "nav_arrows" => {
            app.settings.nav_arrows = on;
            Some(("nav_arrows", bit))
        }
        "exact_fare" => {
            app.settings.exact_fare = on;
            Some(("exact_fare", bit))
        }
        "collision_pedestrians" => {
            app.settings.collision_pedestrians = on;
            Some(("collision_pedestrians", bit))
        }
        "ssao" => {
            app.settings.ssao = on;
            Some(("ssao", bit))
        }
        "detail_textures" => {
            app.settings.detail_textures = on;
            Some(("detail_textures", bit))
        }
        "reflections" => {
            app.settings.reflections = on;
            Some(("reflections", bit))
        }
        "clouds" => {
            app.settings.clouds = on;
            Some(("clouds", bit))
        }
        "windy_trees" => {
            app.settings.windy_trees = on;
            Some(("windy_trees", bit))
        }
        "dx9_snow" => {
            app.settings.dx9_snow = on;
            Some(("dx9_snow", bit))
        }
        "fullscreen" => {
            app.settings.fullscreen = on;
            if app.spanned {
                log::info!(
                    "triple screen: the window spans three monitors, fullscreen is left alone"
                );
            } else if let Some(w) = app.window.as_ref() {
                w.set_fullscreen(on.then_some(winit::window::Fullscreen::Borderless(None)));
            }
            Some(("fullscreen", bit))
        }
        "vsync" => {
            app.settings.vsync = on;
            Some(("vsync", bit))
        }
        "texture_compression" => {
            app.settings.texture_compression = on;
            Some(("texture_compression", bit))
        }
        "driver" => {
            app.settings.driver = on;
            Some(("driver", bit))
        }
        "alt_view" => {
            app.settings.alt_view = on;
            Some(("alt_view", bit))
        }
        "precision_zoom" => {
            app.settings.precision_zoom = on;
            Some(("precision_zoom", bit))
        }
        "triple_screen" => {
            app.settings.triple.enabled = on;
            Some(("triple_screen", bit))
        }
        "triple_hud_center" => {
            app.settings.triple_hud_center = on;
            Some(("triple_hud_center", bit))
        }
        "triple_span" => {
            app.settings.triple_span = on;
            Some(("triple_span", bit))
        }
        "vr" => {
            app.settings.vr = on;
            Some(("vr", bit))
        }
        "vr_desktop_mirror" => {
            app.settings.vr_desktop_mirror = on;
            Some(("vr_desktop_mirror", bit))
        }
        "doppler" => {
            app.settings.doppler = on;
            Some(("doppler", bit))
        }
        "muffle_outside" => {
            app.settings.muffle_outside = on;
            Some(("muffle_outside", bit))
        }
        "steering_linear" => {
            app.settings.steering_linear = on;
            Some(("steering_linear", bit))
        }
        "pad_steer_linear" => {
            app.settings.pad_steer_linear = on;
            Some(("pad_steer_linear", bit))
        }
        "arrows_switch_cams" => {
            app.settings.arrows_switch_cams = on;
            Some(("arrows_switch_cams", bit))
        }
        "old_steering" => {
            app.settings.old_steering = on;
            Some(("old_steering", bit))
        }
        "red_steer_spd" => {
            app.settings.red_steer_spd = on;
            Some(("red_steer_spd", bit))
        }
        "momentary_gears" => {
            app.settings.momentary_gears = on;
            // (the bus being driven read it when it was taken over)
            if let Some(p) = app.player.as_mut() {
                p.momentary_gears = on;
            }
            Some(("momentary_gears", bit))
        }
        "auto_shift" => {
            app.settings.auto_shift = on;
            if let Some(p) = app.player.as_mut() {
                p.auto_shift = on;
            }
            Some(("auto_shift", bit))
        }
        "ff_invert" => {
            app.settings.ff_invert = on;
            Some(("ff_invert", bit))
        }
        "ui_scale_window" => {
            app.settings.ui_scale_window = on;
            Some(("ui_scale_window", bit))
        }
        "tooltips" => {
            app.settings.tooltips = on;
            Some(("tooltips", bit))
        }
        "notes" => {
            app.settings.notes = on;
            Some(("notes", bit))
        }
        "chat" => {
            app.settings.chat = on;
            Some(("chat", bit))
        }
        "name_tags" => {
            app.settings.name_tags = on;
            Some(("name_tags", bit))
        }
        _ => None,
    }
}

/// Change the setting `verb` (a switch or a slider) as `mv` says; false when it is neither.
pub(crate) fn option_do(app: &mut App, verb: &str, arg: &str, mv: Move) -> bool {
    // (the weather is the METAR report's while the sync is on)
    if app.metar_locked()
        && matches!(
            verb,
            "visibility"
                | "rain_amt"
                | "wet"
                | "brightness"
                | "humidity"
                | "temp"
                | "wind_speed"
                | "wind_dir"
                | "snow_cover"
                | "snow_road"
        )
    {
        app.service_msg = Some((
            "The weather cannot be changed while the METAR sync is on".into(),
            3.0,
        ));
        return true;
    }
    if let Some(cur) = toggle_now(app, verb) {
        let on = match mv {
            Move::Next => !cur,
            Move::Inc => true,
            Move::Dec => false,
            Move::To(f) => f >= 0.5,
        };
        if on != cur {
            if let Some((k, v)) = toggle_set(app, verb, on) {
                remember_setting(k, &v);
            }
            sync_live(app);
            LIST_DIRTY.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        return true;
    }
    if let Some(steps) = steps_of(verb) {
        if let Some(now) = option_now(app, verb, arg) {
            let to = step_move(&steps, now, mv);
            // (a slider dragged sends the same value many times over)
            if (to - now).abs() > 1e-6 {
                if let Some((k, v)) = option_set(app, verb, arg, to) {
                    remember_setting(k, &v);
                }
                sync_live(app);
                LIST_DIRTY.store(true, std::sync::atomic::Ordering::Relaxed);
            }
        }
        return true;
    }
    false
}
