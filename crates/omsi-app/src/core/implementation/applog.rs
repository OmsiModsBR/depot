//! The game's log as a record of the whole session (`game.log`, which the launcher keeps per
//! session): the machine and the settings at the start, and while playing what was said on
//! the screen, the views, the pause, and a status line every minute - what a bug report
//! needs to be understood without asking.

use crate::core::app::App;

#[derive(Default)]
pub(crate) struct LogState {
    last_msg: Option<String>,
    last_view: String,
    last_paused: bool,
    t: f32,
    frames: u32,
    worst_dt: f32,
    /// OMSI_PROFILE: the stage times (`App::profile`) at the start of the window that
    /// [`App::log_profile_window`] reports on, its length so far (s) and its frames.
    prof_last: std::collections::BTreeMap<&'static str, f64>,
    prof_t: f32,
    prof_frames: u32,
}

/// How often (s) OMSI_PROFILE writes the stages' cost over the time since the last report.
const PROFILE_WINDOW_S: f32 = 30.0;

/// The stages that took at least this long (ms a frame) in a window are listed.
const PROFILE_WINDOW_MIN_MS: f64 = 0.05;

/// The stages of a profile window, the most costly first: (name, ms a frame) from the
/// stage totals (s) at its start and at its end over `frames` frames. A stage that was not
/// there at the start counts from zero; one that cost less than
/// [`PROFILE_WINDOW_MIN_MS`] is left out.
pub fn profile_window(
    before: &std::collections::BTreeMap<&'static str, f64>,
    now: &std::collections::BTreeMap<&'static str, f64>,
    frames: u32,
) -> Vec<(&'static str, f64)> {
    let n = frames.max(1) as f64;
    let mut out: Vec<(&'static str, f64)> = now
        .iter()
        .map(|(k, v)| (*k, (v - before.get(k).copied().unwrap_or(0.0)) / n * 1000.0))
        .filter(|(_, ms)| *ms >= PROFILE_WINDOW_MIN_MS)
        .collect();
    out.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(b.0)));
    out
}

/// The machine, the program and its settings, once at the start.
pub(crate) fn log_system(settings: &crate::settings::Settings) {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(0);
    let ram = crate::core::memory::physical_memory()
        .map(|b| b / 1_000_000)
        .unwrap_or(0);
    log::info!(
        "system: {} {} ({}), {cpus} threads, {ram} MB memory",
        std::env::consts::OS,
        std::env::consts::ARCH,
        os_version()
    );
    log::info!(
        "command line: {}",
        std::env::args().collect::<Vec<_>>().join(" ")
    );
    if let Ok(d) = std::env::current_dir() {
        log::info!("working folder: {}", d.display());
    }
    let env: Vec<String> = std::env::vars()
        .filter(|(k, _)| k.starts_with("OMSI_") || k == "RUST_LOG" || k == "WGPU_BACKEND")
        .map(|(k, v)| format!("{k}={v}"))
        .collect();
    if !env.is_empty() {
        log::info!("environment: {}", env.join(" "));
    }
    log::info!("all settings: {settings:?}");
}

fn os_version() -> String {
    #[cfg(target_os = "macos")]
    {
        if let Ok(o) = std::process::Command::new("sw_vers")
            .arg("-productVersion")
            .output()
        {
            return format!("macOS {}", String::from_utf8_lossy(&o.stdout).trim());
        }
    }
    #[cfg(target_os = "linux")]
    {
        if let Ok(s) = std::fs::read_to_string("/etc/os-release") {
            if let Some(l) = s.lines().find(|l| l.starts_with("PRETTY_NAME=")) {
                return l
                    .trim_start_matches("PRETTY_NAME=")
                    .trim_matches('"')
                    .to_string();
            }
        }
    }
    #[cfg(windows)]
    {
        if let Ok(o) = std::process::Command::new("cmd")
            .args(["/C", "ver"])
            .output()
        {
            return String::from_utf8_lossy(&o.stdout).trim().to_string();
        }
    }
    "version unknown".to_string()
}

impl App {
    /// OMSI_PROFILE: every [`PROFILE_WINDOW_S`] seconds, what each stage cost a frame since
    /// the last report, with the traffic on the map then. The totals of the whole session
    /// are written only with `--exit-after`, and a session started from the launcher has
    /// no way to pass it; an average over the whole session also hides a cost that grows
    /// as the session goes on (with the number of cars, for one).
    pub(crate) fn log_profile_window(&mut self, dt: f32) {
        if !omsi_cfg::flags::PROFILE.is_set() {
            return;
        }
        let s = &mut self.log_state;
        s.prof_t += dt;
        s.prof_frames += 1;
        if s.prof_t < PROFILE_WINDOW_S {
            return;
        }
        let stages = profile_window(&s.prof_last, &self.profile, s.prof_frames);
        let (secs, frames) = (s.prof_t, s.prof_frames);
        s.prof_last = self.profile.clone();
        s.prof_t = 0.0;
        s.prof_frames = 0;
        let (cars, dormant) = self
            .session
            .traffic
            .as_ref()
            .map(|t| (t.cars.len(), t.dormant.len()))
            .unwrap_or((0, 0));
        let people = self
            .session
            .humans
            .as_ref()
            .map(|h| h.people.len())
            .unwrap_or(0);
        let list: Vec<String> = stages
            .iter()
            .map(|(k, ms)| format!("{k} {ms:.2}"))
            .collect();
        log::info!(
            "profile window: {:.1} fps over {secs:.0} s, {:.1} ms a frame; {cars} AI vehicles (+{dormant} out of range), {people} people; ms a frame: {}",
            frames as f32 / secs.max(1e-3),
            secs / frames.max(1) as f32 * 1000.0,
            list.join(", ")
        );
    }

    /// Once a frame: what changed on the screen, and every minute where things stand.
    pub(crate) fn log_frame(&mut self, dt: f32) {
        let msg = self.service_msg.as_ref().map(|m| m.0.clone());
        if msg.is_some() && msg != self.log_state.last_msg {
            log::info!("on screen: {}", msg.as_deref().unwrap_or_default());
        }
        self.log_state.last_msg = msg;
        if self.view != self.log_state.last_view {
            log::info!("view: {}", self.view);
            self.log_state.last_view = self.view.clone();
        }
        if self.paused != self.log_state.last_paused {
            log::info!("{}", if self.paused { "paused" } else { "resumed" });
            self.log_state.last_paused = self.paused;
        }
        let s = &mut self.log_state;
        s.t += dt;
        s.frames += 1;
        s.worst_dt = s.worst_dt.max(dt);
        if s.t < 60.0 {
            return;
        }
        let (fps, worst) = (s.frames as f32 / s.t, 1.0 / s.worst_dt.max(1e-3));
        s.t = 0.0;
        s.frames = 0;
        s.worst_dt = 0.0;
        let bus = self.player.as_ref().map(|p| {
            let v = &p.vehicle;
            format!(
                "bus at ({:.1}, {:.1}, {:.1}) heading {:.0}, {:.0} km/h",
                v.position.x,
                v.position.y,
                v.position.z,
                v.heading,
                v.motion().velocity_kmh()
            )
        });
        let cam = self.camera.as_ref().map(|c| {
            format!(
                "camera at ({:.0}, {:.0}, {:.0})",
                c.position.x, c.position.y, c.position.z
            )
        });
        let traffic = self
            .session
            .traffic
            .as_ref()
            .map(|t| t.cars.len())
            .unwrap_or(0);
        let time = self.clock.time;
        let gpu = match (self.renderer.as_ref(), self.scene.as_ref()) {
            (Some(r), Some(sc)) => format!(
                ", GPU memory: textures {:.0} MB, meshes {:.0} MB",
                r.texture_bytes(sc) as f64 / 1e6,
                r.mesh_bytes(sc) as f64 / 1e6
            ),
            _ => String::new(),
        };
        log::info!(
            "status: {fps:.0} fps (worst frame {worst:.0} fps), {}, view {}, game time {:02}:{:02}, {traffic} AI vehicles{}{gpu}",
            bus.or(cam).unwrap_or_default(),
            self.view,
            (time / 3600.0) as i64 % 24,
            (time / 60.0) as i64 % 60,
            if self.paused { ", paused" } else { "" }
        );
    }
}
