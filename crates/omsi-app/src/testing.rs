//! What the tests in `tests/` reach of the crate's insides, built only with the
//! `test-support` feature (the crate's own dev-dependency turns it on; the game builds
//! without it).
//!
//! One module per module of the crate whose tests moved out, each re-exporting what that
//! module has public, plus the builders the tests share.

/// A vehicle of the script `osc` that declares the variables `varlist` and the string
/// variables `stringvarlist` (one a line).
pub fn script_test_vehicle(
    osc: &str,
    varlist: &str,
    stringvarlist: &str,
) -> omsi_sim::VehicleInstance {
    // (a folder of its own: tests run side by side)
    static MADE: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let n = MADE.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let dir = std::env::temp_dir().join(format!("omsi_ibis_dest_{}_{n}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let script = dir.join("ibis.osc");
    let vars = dir.join("vars.txt");
    let strings = dir.join("strings.txt");
    std::fs::write(&vars, varlist).unwrap();
    std::fs::write(&strings, stringvarlist).unwrap();
    std::fs::write(&script, osc).unwrap();
    let program = omsi_script::compile(&omsi_script::CompileInput {
        scripts: vec![script],
        varlists: vec![vars],
        stringvarlists: vec![strings],
        ..Default::default()
    });
    assert!(program.errors.is_empty(), "{:?}", program.errors);
    let ty = std::sync::Arc::new(omsi_sim::VehicleType {
        def: Default::default(),
        model: Default::default(),
        model_dir: dir.clone(),
        program: std::sync::Arc::new(program),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    });
    std::fs::remove_dir_all(dir).unwrap();
    omsi_sim::VehicleInstance::new(ty, omsi_sim::VehicleHost::new(Default::default()))
}

pub use crate::close_app;

/// The game's own text for `key` in the language `lang`, if it has one.
pub fn translate(lang: &str, key: &str) -> Option<String> {
    crate::_rust_i18n_try_translate(lang, key).map(|t| t.into_owned())
}

pub mod about {
    pub use crate::about::*;
}
pub mod actions {
    pub use crate::actions::*;
}
pub mod api_limits {
    pub use crate::api_limits::*;
}
pub mod app {
    pub use crate::app::*;
}
pub mod app_events {
    pub use crate::app_events::*;
}
pub mod bus_service {
    pub use crate::bus_service::*;
}
pub mod cli {
    pub use crate::cli::*;
}
pub mod condensation {
    pub use crate::condensation::*;

    /// The cabin's air as a test reads it.
    pub trait CabinAirInternals {
        /// The windscreen's glass where the defroster blows dries rather than mists.
        fn defrosted_glass_clear(&self, i: &CabinInputs) -> bool;
    }

    impl CabinAirInternals for CabinAir {
        fn defrosted_glass_clear(&self, i: &CabinInputs) -> bool {
            i.engine && self.defroster_drying(i) > 0.0
        }
    }
}
pub mod controllers {
    pub use crate::controllers::*;
}
pub mod describe {
    pub use crate::describe::*;

    /// The control names as the tests build them.
    pub trait ControlNamesInternals {
        /// The same names from a table.
        fn from_table(lang: &str, table: &[(&str, &str)]) -> Self;
    }

    impl ControlNamesInternals for ControlNames {
        fn from_table(lang: &str, table: &[(&str, &str)]) -> Self {
            ControlNames {
                lang: language_code(lang),
                texts: table
                    .iter()
                    .map(|(k, v)| (k.to_ascii_lowercase(), v.to_string()))
                    .collect(),
                spellings: table
                    .iter()
                    .map(|(k, _)| (k.to_ascii_lowercase(), k.to_string()))
                    .collect(),
            }
        }
    }
}
pub mod diagnostics {
    pub use crate::diagnostics::*;
}
#[cfg(windows)]
pub mod dinput {
    pub use crate::dinput::*;
}
pub mod editor {
    pub use crate::editor::*;
}
pub mod env_file {
    pub use crate::env_file::*;
}
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub mod evdev_buttons {
    pub use crate::evdev_buttons::*;
}
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub mod evdev_ff {
    pub use crate::evdev_ff::*;
}
pub mod ffb_calibration {
    pub use crate::ffb_calibration::*;
}
pub mod frame_stats {
    pub use crate::frame_stats::*;
}
pub mod game_controller_menu {
    pub use crate::game_controller_menu::*;
}
pub mod game_lists {
    pub use crate::game_lists::*;
}
pub mod headtrack {
    pub use crate::headtrack::*;
}
pub mod hpattern {
    pub use crate::hpattern::*;
}
pub mod hud {
    pub use crate::hud::domain::*;
    pub use crate::hud::implementation::legacy::oft::*;
    pub use crate::hud::implementation::legacy::*;
}
pub mod humans {
    pub use crate::humans::*;

    pub mod pax {
        pub use crate::humans::pax::*;
    }
}
pub mod input {
    pub mod keyboard {
        pub use crate::input::keyboard::*;
    }
    pub mod mouse {
        pub use crate::input::mouse::*;
    }
    pub mod menus {
        pub use crate::input::menus::*;
    }
}
pub mod journey {
    pub use crate::journey::*;
}
pub mod keys {
    pub use crate::keys::*;
}
pub mod launcher {
    pub use crate::launcher::*;

    pub mod ai_edit {
        pub use crate::launcher::ai_edit::*;
    }
    pub mod ai_traffic {
        pub use crate::launcher::ai_traffic::*;
    }
    pub mod backdrop {
        pub use crate::launcher::backdrop::*;
    }
    pub mod chrome {
        pub use crate::launcher::chrome::*;
    }
    pub mod diagnostics {
        pub use crate::launcher::diagnostics::*;
    }
    pub mod dialogs {
        pub use crate::launcher::dialogs::*;
    }
    pub mod drive {
        pub use crate::launcher::drive::*;
    }
    pub mod integrity {
        pub use crate::launcher::integrity::*;
    }
    pub mod keyboard {
        pub use crate::launcher::keyboard::*;
    }
    pub mod mapview {
        pub use crate::launcher::mapview::*;
    }
    pub mod original_ui {
        pub use crate::launcher::original_ui::*;
    }
    pub mod pages {
        pub use crate::launcher::pages::*;
    }
    pub mod prerender {
        pub use crate::launcher::prerender::*;
    }
    pub mod preview {
        pub use crate::launcher::preview::*;
    }
    pub mod settings {
        pub use crate::launcher::settings::*;
    }
    pub mod setup {
        pub use crate::launcher::setup::*;
    }
    pub mod showroom {
        pub use crate::launcher::showroom::*;
    }
    pub mod start {
        pub use crate::launcher::start::*;
    }
    pub mod state {
        pub use crate::launcher::state::*;
    }
    pub mod thumbs {
        pub use crate::launcher::thumbs::*;
    }
    pub mod timetable {
        pub use crate::launcher::timetable::*;
    }
    pub mod timetable_plan {
        pub use crate::launcher::timetable_plan::*;
    }
    pub mod tray {
        pub use crate::launcher::tray::*;
    }
    pub mod ui {
        pub use crate::launcher::ui::*;
    }
}
pub mod lights {
    pub use crate::lights::*;
}
pub mod line_times {
    pub use crate::line_times::*;
}
#[cfg(target_os = "macos")]
pub mod mac_game_controller {
    pub use crate::mac_game_controller::*;
}
pub mod map_surface {
    pub use crate::map_surface::*;
}
pub mod money {
    pub use crate::money::*;
}
pub mod nav {
    pub use crate::nav::*;

    pub mod draw {
        pub use crate::nav::draw::*;
    }

    pub mod city {
        pub use crate::nav::city::*;
    }
    pub mod geometry {
        pub use crate::nav::geometry::*;
    }
    pub mod lines {
        pub use crate::nav::lines::*;
    }
}
pub mod on_foot {
    pub use crate::on_foot::*;
}
pub mod output {
    pub mod camera {
        pub mod arm {
            pub use crate::output::implementation::camera::arm::*;
        }
        pub mod util {
            pub use crate::output::implementation::camera::util::*;
        }
    }
    pub mod head_idle {
        pub use crate::output::domain::head_idle::*;
    }
    pub mod mirror_hud {
        pub use crate::output::domain::mirror_hud::*;
    }
    pub mod xr {
        pub mod navigator {
            pub use crate::output::domain::vr_navigator::*;
        }
        #[cfg(windows)]
        pub mod session {
            pub use crate::output::implementation::xr::session::*;
        }
    }
}
pub mod player {
    pub use crate::player::*;
}
pub mod puddles {
    pub use crate::puddles::*;
}
pub mod quit {
    pub use crate::quit::*;

    #[cfg(unix)]
    pub mod sys {
        pub use crate::quit::sys::*;
    }
}
pub mod radio {
    pub use crate::radio::*;
}
pub mod rain {
    pub use crate::rain::*;
}
pub mod scene {
    pub use crate::scene::*;

    pub mod ground_query {
        pub use crate::scene::ground_query::*;
    }
}
pub mod schedule {
    pub use crate::schedule::*;
}
pub mod schedule_paper {
    pub use crate::schedule_paper::*;
}
pub mod screenshot_name {
    pub use crate::screenshot_name::*;
}
pub mod services {
    pub use crate::services::*;
}
pub mod settings {
    pub use crate::settings::*;

    /// The settings as a test writes them.
    pub trait SettingsInternals {
        /// The settings as the file holds them. The game only ever reads the file - the
        /// launcher's settings page writes it - so this is here for the round-trip test that
        /// every key read is written back.
        fn to_text(&self) -> String;
    }

    impl SettingsInternals for Settings {
        fn to_text(&self) -> String {
            let mut text = format!(
                "# Project Depot settings\nversion={}\nmsaa={}\nanisotropy={}\nssao={}\nshadows={}\nshadow_size={}\nshadow_blobs={}\nnavigator={}\nui_opacity={}\nnavigator_corner={}\nboarding={}\ndetail_textures={}\nexact_fare={}\nenhanced={}\ngraphics={}\nvr={}\nvr_scale={}\nfullscreen={}\nvsync={}\nvolume={}\ndrive_keys={}\npost_aa={}\nrender_scale={}\nlanguage={}\ntexture_compression={}\ntexture_memory={}\nauto_clutch={}\nmomentary_gears={}\nmin_obj_size={}\nmax_obj_dist={}\nmax_fps={}\nchat={}\ntooltips={}\nname_tags={}\nshow_fps={}\nclouds={}\npax_density={}\nvol_ai={}\nvol_scenery={}\nmirror_size={}\nmirror_hud={}\nmirror_refresh={}\ndoppler={}\ndriver={}\ndriverview_smooth={}\n",
                crate::settings::SETTINGS_VERSION, self.msaa, self.anisotropy, self.ssao as u8, self.shadows as u8, self.shadow_size, self.shadow_blobs as u8, self.navigator as u8, self.ui_opacity, self.navigator_corner, self.boarding, self.detail_textures as u8, self.exact_fare as u8, self.enhanced as u8, self.graphics, self.vr as u8, self.vr_scale, self.fullscreen as u8, self.vsync as u8, self.volume, self.drive_keys, self.post_aa, self.render_scale_text(), self.language, self.texture_compression as u8, self.texture_memory, self.auto_clutch as u8, self.momentary_gears as u8, self.min_obj_size, if self.max_obj_dist < 0.0 { "auto".to_string() } else { self.max_obj_dist.to_string() }, self.max_fps, self.chat as u8, self.tooltips as u8, self.name_tags as u8, self.show_fps as u8, self.clouds as u8, self.pax_density, self.vol_ai, self.vol_scenery, self.mirror_size, self.mirror_hud, self.mirror_refresh, self.doppler as u8, self.driver as u8, self.driverview_smooth as u8
            );
            text.push_str(&format!(
                "vr_head_smoothing_ms={}\nvr_mirror_rate={}\nvr_desktop_mirror={}\nled_glow={}\nled_mips={}\nnight_brightness={}\nui_scale={}\nui_scale_window={}\nchat_size={}\nnotes={}\nff_road_vib={}\nff_engine_vib={}\nff_fade={}\n",
                self.vr_head_smoothing_ms, self.vr_mirror_rate, self.vr_desktop_mirror as u8, self.led_glow, self.led_mips, self.night_brightness, self.ui_scale, self.ui_scale_window as u8, self.chat_size, self.notes as u8, self.ff_road_vib, self.ff_engine_vib, self.ff_fade,
            ));
            text.push_str(&format!(
                "triple_hud_center={}\ntriple_fov_deg={}\nfov={}\noutside_fov={}\n",
                self.triple_hud_center as u8, self.triple.fov_deg, self.fov, self.outside_fov
            ));
            text.push_str(&format!("triple_screen={}\ntriple_span={}\ntriple_width_mm={}\ntriple_distance_mm={}\ntriple_bezel_mm={}\ntriple_left_angle_deg={}\ntriple_right_angle_deg={}\ntriple_eye_height_mm={}\n", self.triple.enabled as u8, self.triple_span as u8, self.triple.width_mm, self.triple.distance_mm, self.triple.bezel_mm, self.triple.left_angle_deg, self.triple.right_angle_deg, self.triple.eye_height_mm));
            text.push_str(&format!("steer_look={}\nsteer_look_angle={}\nsteer_look_response={}\nhead_idle={}\nhead_idle_pace={}\nlook_sens={}\nlook_smoothing_ms={}\nblinker_cancel={}\n", self.steer_look as u8, self.steer_look_angle, self.steer_look_response, self.head_idle, self.head_idle_pace, self.look_sens, self.look_smoothing_ms, self.blinker_cancel as u8));
            text.push_str(&format!("seat_pitch_deg={}\n", self.seat_pitch_deg));
            text.push_str(&format!("head_tracking_yaw_sens={}\nhead_tracking_pitch_sens={}\nhead_tracking_roll_sens={}\nhead_tracking_x_sens={}\nhead_tracking_y_sens={}\nhead_tracking_z_sens={}\nhead_tracking_invert_yaw={}\nhead_tracking_invert_pitch={}\nhead_tracking_invert_roll={}\nhead_tracking_invert_x={}\nhead_tracking_invert_y={}\nhead_tracking_invert_z={}\n", { let p = self.head_tracking_yaw_sens; if p <= 2.0 { p / 100.0 } else { p } }, { let p = self.head_tracking_pitch_sens; if p <= 2.0 { p / 100.0 } else { p } }, { let p = self.head_tracking_roll_sens; if p <= 2.0 { p / 100.0 } else { p } }, { let p = self.head_tracking_x_sens; if p <= 2.0 { p / 100.0 } else { p } }, { let p = self.head_tracking_y_sens; if p <= 2.0 { p / 100.0 } else { p } }, { let p = self.head_tracking_z_sens; if p <= 2.0 { p / 100.0 } else { p } }, self.head_tracking_invert_yaw as u8, self.head_tracking_invert_pitch as u8, self.head_tracking_invert_roll as u8, self.head_tracking_invert_x as u8, self.head_tracking_invert_y as u8, self.head_tracking_invert_z as u8));
            text.push_str(&format!("auto_shift={}\n", self.auto_shift as u8));
            text.push_str(&format!(
                "right_stick_look={}\n",
                self.right_stick_look as u8
            ));
            text.push_str(&format!("pad_steer_smooth={}\n", self.pad_steer_smooth));
            text.push_str(&format!(
                "pad_steer_linear={}\n",
                self.pad_steer_linear as u8
            ));
            text.push_str(&format!(
                "arrows_switch_cams={}\n",
                self.arrows_switch_cams as u8
            ));
            text.push_str(&format!(
                "info_bar_mode={}\ninfo_bar_pos={}\ninfo_bar_fps={}\n",
                self.info_bar_mode, self.info_bar_pos, self.info_bar_fps as u8
            ));
            for (key, on) in INFO_ITEM_KEYS.iter().zip(self.info_items) {
                text.push_str(&format!("{key}={}\n", on as u8));
            }
            text.push_str(&format!("voice_chat={}\n", self.voice_chat as u8));
            text.push_str(&format!("windy_trees={}\n", self.windy_trees as u8));
            text.push_str(&format!("sun_glow={}\n", self.sun_glow as u8));
            text.push_str(&format!("muffle_outside={}\n", self.muffle_outside as u8));
            // OMSI's own options (`ai_unsched_factor` as a percentage, as the file has it)
            text.push_str(&format!("maintenance={}\nai_unsched_factor={}\nai_max_scheduled={}\nai_max_random={}\nai_priority_scheduled={}\nai_max_parked={}\nai_max_humans={}\ncollision_vehicles={}\ncollision_objects={}\ncollision_pedestrians={}\nhead_movement={}\nhands_in_cab={}\nalt_view={}\nprecision_zoom={}\n", self.maintenance, self.ai_unsched_factor * 100.0, self.ai_max_scheduled, self.ai_max_random, self.ai_priority_scheduled, self.ai_max_parked, self.ai_max_humans, self.collision_vehicles as u8, self.collision_objects as u8, self.collision_pedestrians as u8, self.head_movement as u8, self.hands_in_cab as u8, self.alt_view as u8, self.precision_zoom as u8));
            text.push_str(&format!("pax_voices={}\nnav_arrows={}\nnav_ai={}\nnav_follow_view={}\nnav_surface={}\nnav_light={}\nnav_contours={}\nnav_orientation={}\nnav_view={}\nget_up={}\ninfo_bar={}\ntime_speed={}\ntime_sync={}\nmetar_sync={}\nmetar_station={}\nshadow_casters={}\nreflections={}\n", self.pax_voices, self.nav_arrows as u8, self.nav_ai as u8, self.nav_follow_view as u8, self.nav_surface as u8, self.nav_light as u8, self.nav_contours as u8, self.nav_orientation, self.nav_view, self.get_up as u8, self.info_bar as u8, self.time_speed, self.time_sync as u8, self.metar_sync as u8, self.metar_station, self.shadow_casters, self.reflections as u8));
            text.push_str(&format!("ctrl_deadzone={}\nctrl_off={}\nsteering_linear={}\nold_steering={}\nred_steer_spd={}\nmouse_sens={}\ngraphics_api={}\nff_invert={}\nff_enabled={}\nbrake_hold={}\nmouse_steering={}\nmouse_right_off={}\nmouse_smooth={}\nwheel_range={}\nwheel_lock={}\ncamera_collision={}\npedal_throttle={}\npedal_brake={}\nseat_x={}\nseat_y={}\nseat_z={}\n", self.ctrl_deadzone, self.ctrl_off, self.steering_linear as u8, self.old_steering as u8, self.red_steer_spd as u8, self.mouse_sens, self.graphics_api, self.ff_invert as u8, self.ff_enabled as u8, self.brake_hold as u8, self.mouse_steering as u8, self.mouse_right_off as u8, self.mouse_smooth as u8, self.wheel_range, self.wheel_lock, self.camera_collision as u8, self.pedal_throttle, self.pedal_brake, self.seat[0], self.seat[1], self.seat[2]));
            text.push_str(&format!("head_tracking={}\nhead_tracking_port={}\nhead_tracking_bind={}\nhead_tracking_invert={}\n", self.head_tracking as u8, self.head_tracking_port, self.head_tracking_bind, self.head_tracking_invert));
            text.push_str(&format!("autosave_minutes={}\n", self.autosave_minutes));
            text.push_str(&format!("ui_accent={}\n", self.ui_accent));
            text.push_str(&format!("ui_corners={}\n", self.ui_corners));
            text
        }
    }

    pub mod bus_seats {
        pub use crate::settings::bus_seats::*;
    }
}
pub mod situation {
    pub use crate::situation::*;
}
pub mod startup {
    pub use crate::startup::*;
}
pub mod tiles {
    pub use crate::tiles::*;
}
pub mod touch {
    pub use crate::touch::*;
}
pub mod tr_fill {
    pub use crate::tr_fill::*;

    /// Enough words around the values to tell the template apart ("%{a}: %{b}" is not).
    pub fn telling(parts: &[String]) -> bool {
        parts.len() >= 2
            && parts
                .iter()
                .map(|s| s.trim().chars().count())
                .sum::<usize>()
                >= 4
    }
}
pub mod traffic {
    pub use crate::traffic::*;
}
pub mod tutorial {
    pub use crate::tutorial::*;
}
pub mod ui {
    pub mod add {
        pub use crate::ui::domain::add::*;
    }
    pub mod alt {
        pub use crate::ui::domain::alt::*;
        use glam::Vec2;
        use omsi_ui::alt_bar::Hit;

        /// The bar as a test reads it.
        pub trait AltFrameInternals {
            /// The index of the button under `px` (window pixels) in a window of `size` pixels.
            fn hit(&self, px: Vec2, size: (u32, u32)) -> Option<usize>;
        }

        impl AltFrameInternals for AltFrame {
            fn hit(&self, px: Vec2, size: (u32, u32)) -> Option<usize> {
                match self.hit_any(px, size) {
                    Some(Hit::Button(i)) => Some(i),
                    _ => None,
                }
            }
        }
    }
    pub mod loading {
        pub use crate::ui::domain::loading::*;
        pub use crate::ui::implementation::loading::*;
    }
    pub mod menu {
        pub use crate::ui::domain::menu::*;
        pub use crate::ui::implementation::menu::*;
        use omsi_ui::pause_menu::Kind;

        /// A menu's column as a test reads it.
        pub trait MenuFrameInternals {
            /// The line marked as the page shown, if any.
            fn current(&self) -> Option<&str>;
            /// What the bar over the page says: the title, the subtitle and the state ("paused").
            fn bar(&self) -> (&str, &str, &str);
        }

        impl MenuFrameInternals for MenuFrame {
            fn current(&self) -> Option<&str> {
                self.sections
                    .iter()
                    .flat_map(|s| s.items.iter())
                    .find(|i| i.kind == Kind::Current)
                    .map(|i| i.id.as_str())
            }
            fn bar(&self) -> (&str, &str, &str) {
                (&self.title, &self.subtitle, self.state)
            }
        }
    }
    pub mod overlay {
        pub use crate::ui::domain::overlay::*;
        pub use crate::ui::implementation::overlay::*;
    }
    pub mod pages {
        use crate::hud::{Frame, MenuKind};
        use crate::ui::domain::menu::MenuFrame;
        pub use crate::ui::domain::pages::*;
        pub use crate::ui::implementation::pages::*;

        /// The Esc menu's column of a session with every line (tests).
        pub fn test_column(paused: bool) -> MenuFrame {
            crate::ui::domain::menu::menu_frame(
                &crate::input::menus::GAME_MENU,
                Some(0),
                &|_| false,
                "08:42".into(),
                paused,
            )
        }

        /// The HUD's frame with the list `items` of `kind` open on its first line (tests).
        pub fn test_frame<'a>(items: &'a [(&'a str, &'a str)], kind: MenuKind) -> Frame<'a> {
            Frame {
                scale: 1.0,
                ui_scale: 1.0,
                opacity: 1.0,
                width: 1672.0,
                height: 941.0,
                cursor: (0.0, 0.0),
                vr: false,
                tooltip: None,
                chat: None,
                chat_size: 1.0,
                notes: &[],
                fps: None,
                paused: true,
                menu: Some((0, items)),
                menu_top: None,
                timetable: None,
                info: None,
                info_room: None,
                tutorial: None,
                tags: Vec::new(),
                menu_kind: kind,
                menu_head: Some(("Options".into(), String::new())),
                menu_preview: None,
                pane_first: None,
                menu_tabs: None,
                menu_kbd: true,
                dropdown: None,
            }
        }
    }
    pub mod report {
        pub use crate::ui::domain::report::*;
        use omsi_ui::ingame::{self, ReportRow};

        /// A report as the tests build it.
        pub trait ReportFrameInternals {
            /// A trip of three stops, for the overlay's tests.
            fn for_test() -> Self;
        }

        impl ReportFrameInternals for ReportFrame {
            fn for_test() -> Self {
                let row = |stop: &str, at: &str, state| ReportRow {
                    stop: stop.into(),
                    scheduled: at.into(),
                    actual: at.into(),
                    deviation: "0:00".into(),
                    state,
                };
                ReportFrame {
                    title: "50 · Central Station".into(),
                    note: "paused",
                    rows: vec![
                        row("Depot", "08:44", ingame::StopState::OnTime),
                        row("Market", "08:47", ingame::StopState::OnTime),
                        row("Central Station", "08:54", ingame::StopState::OnTime),
                    ],
                    totals: vec![("stops served", "3".into())],
                }
            }
        }
    }
    pub mod windows {
        pub use crate::ui::domain::windows::*;
    }
}
pub mod weather_cycle {
    pub use crate::weather_cycle::*;
}
pub mod weather_model {
    pub use crate::weather_model::*;
}
pub mod weather_setup {
    pub use crate::weather_setup::*;
}
pub mod world_load {
    pub use crate::world_load::*;
}
