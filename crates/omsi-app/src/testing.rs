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
#[cfg(feature = "lan")]
pub mod admin {
    pub use crate::admin::*;
}
pub mod api_limits {
    pub use crate::api_limits::*;
}
pub mod bus_service {
    pub use crate::bus_service::*;
}
pub mod camera_arm {
    pub use crate::camera_arm::*;
}
pub mod camera_util {
    pub use crate::camera_util::*;
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
pub mod head_idle {
    pub use crate::head_idle::*;
}
pub mod headtrack {
    pub use crate::headtrack::*;
}
pub mod hpattern {
    pub use crate::hpattern::*;
}
pub mod hud {
    pub use crate::hud::*;
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
#[cfg(feature = "lan")]
pub mod lan {
    pub use crate::lan::*;
}
#[cfg(feature = "lan")]
pub mod lan_mods {
    pub use crate::lan_mods::*;
}
#[cfg(feature = "lan")]
pub mod lan_world {
    pub use crate::lan_world::*;
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
pub mod mirror_hud {
    pub use crate::mirror_hud::*;
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
pub mod new_ui {
    pub use crate::new_ui::*;
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
pub mod new_ui_add {
    pub use crate::new_ui_add::*;
}
pub mod new_ui_alt {
    pub use crate::new_ui_alt::*;
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
pub mod new_ui_loading {
    pub use crate::new_ui_loading::*;
}
pub mod new_ui_pages {
    use crate::new_ui::MenuFrame;
    pub use crate::new_ui_pages::*;
    use crate::ui::{Frame, MenuKind};

    /// The Esc menu's column of a session with every line (tests).
    pub fn test_column(paused: bool) -> MenuFrame {
        crate::new_ui::menu_frame(
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
            notices: &[],
            notice_anchor: None,
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
pub mod new_ui_report {
    pub use crate::new_ui_report::*;
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
pub mod new_ui_windows {
    pub use crate::new_ui_windows::*;
}
pub mod offscreen {
    pub use crate::offscreen::*;
}
pub mod on_foot {
    pub use crate::on_foot::*;
}
#[cfg(windows)]
pub mod openxr {
    pub use crate::openxr::*;
}
pub mod player {
    pub use crate::player::*;
}
#[cfg(feature = "plugins")]
pub mod plugin_ui {
    pub use crate::plugin_ui::*;
}
#[cfg(feature = "plugins")]
pub mod plugins {
    pub use crate::plugins::*;
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
#[cfg(feature = "server")]
pub mod server {
    pub use crate::server::*;
}
pub mod services {
    pub use crate::services::*;
}
pub mod settings {
    pub use crate::settings::*;
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
    pub use crate::ui::*;
}
#[cfg(feature = "lan")]
pub mod voice {
    pub use crate::voice::*;
}
pub mod vr_navigator {
    pub use crate::vr_navigator::*;
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
