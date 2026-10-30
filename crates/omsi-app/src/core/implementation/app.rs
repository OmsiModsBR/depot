//! The window's game at work: `App`'s per-frame and loading procedures, the streamed map's
//! start, the missing content's notice, the cockpit camera's glide and the stand-in picture.

use crate::ambience;
use crate::career;
use crate::core::cli::{parse_time, Args};
use crate::core::domain::app::{wrap_deg, App};
use crate::core::memory::texture_budget;
use crate::core::startup::{fatal_message, window_renderer};
use crate::hud;
use crate::hud::nav;
use crate::humans;
use crate::menu;
use crate::money;
use crate::output::Backend;
use crate::scene::World;
use crate::schedule;
use crate::settings;
use crate::situation::{start_clock, ticket_key_name};
use crate::spawn::spawn_player;
use crate::tiles;
use crate::traffic;
use crate::weather_setup::{initial_wetness, load_weather, setup_sky};
use crate::world_load::{load_world, open_world, spawn_point};
use glam::DVec3;
use omsi_render::{Camera, Renderer, Scene, SurfaceState};
use std::sync::Arc;
use std::time::Instant;
use winit::event_loop::ActiveEventLoop;
use winit::window::Window;

const SLOW_UPLOAD_MB_S: f64 = 300.0;

impl App {
    #[cfg(windows)]
    pub(crate) fn vr_active(&self) -> bool {
        self.vr.is_some()
    }

    #[cfg(not(windows))]
    pub(crate) fn vr_active(&self) -> bool {
        false
    }

    pub(crate) fn resumed_impl(&mut self, event_loop: &ActiveEventLoop) {
        if let Some(window) = self.window.clone() {
            // back from the background (a phone): the window's surface is made again
            if self.surface.is_none() {
                if let Some(r) = self.renderer.as_ref() {
                    let size = window.inner_size();
                    let vsync = self.settings.vsync && !self.vr_active();
                    self.surface = if self.output.backend() == Backend::D3d9 {
                        SurfaceState::shown_elsewhere(
                            &self.instance,
                            window.clone(),
                            r,
                            size.width.max(1),
                            size.height.max(1),
                            vsync,
                        )
                        .ok()
                    } else {
                        SurfaceState::new_with(
                            &self.instance,
                            window.clone(),
                            r,
                            size.width.max(1),
                            size.height.max(1),
                            vsync,
                        )
                        .ok()
                    };
                    self.last = Instant::now();
                }
            }
            return;
        }
        self.create_window(event_loop, None);
    }

    /// The game's window (or the launcher's, handed over on a phone), its surface and the
    /// renderer; then the menu or, when the session is given, the world.
    pub(crate) fn create_window(
        &mut self,
        event_loop: &ActiveEventLoop,
        given: Option<Arc<Window>>,
    ) {
        // Steam's rich presence starts before the game window (see `steam.rs`)
        #[cfg(steam)]
        if self.steam.is_none() {
            self.steam = crate::steam::Steam::start();
        }

        // --size sets the window's size in points as well (1600x900 unless given)
        let (lw, lh) = self
            .args
            .size
            .split_once('x')
            .map(|(a, b)| {
                (
                    a.parse::<u32>().unwrap_or(1600),
                    b.parse::<u32>().unwrap_or(900),
                )
            })
            .unwrap_or((1600, 900));
        let monitor_at = self
            .args
            .monitor_at
            .as_deref()
            .and_then(crate::core::startup::parse_point);
        let (fit, at) =
            crate::core::startup::fit_window(event_loop, lw as f64, lh as f64, monitor_at);
        let mut attrs = Window::default_attributes()
            .with_title("Project Depot")
            .with_inner_size(fit)
            .with_window_icon(crate::core::startup::window_icon());
        if let Some(at) = at {
            attrs = attrs.with_position(at);
        }
        // the window size of the settings (pixels, #904) unless --size names one
        let resolution = crate::settings::Settings::resolution()
            .filter(|_| self.args.size == crate::core::cli::DEFAULT_SIZE);
        if let Some((w, h)) = resolution {
            attrs = attrs.with_inner_size(winit::dpi::PhysicalSize::new(w, h));
            if let Some(m) = crate::core::startup::window_monitor(event_loop, monitor_at) {
                let (sw, sh) = (m.size().width as i32, m.size().height as i32);
                attrs = attrs.with_position(winit::dpi::PhysicalPosition::new(
                    m.position().x + ((sw - w as i32) / 2).max(0),
                    m.position().y + ((sh - h as i32) * 2 / 5).max(0),
                ));
            }
        }
        // (a Steam Deck's Gaming Mode: gamescope shows one window over the whole screen and
        // scales whatever size it has to it - a window fitted to 90 % of the screen came out
        // blurred and letterboxed. There the window is the screen's, unless a size is set)
        let gamescope = resolution.is_none() && crate::core::startup::under_gamescope();
        if gamescope {
            log::info!("gamescope (Steam Deck Gaming Mode): the window fills the screen");
        }
        if self.settings.fullscreen || gamescope {
            attrs = attrs.with_fullscreen(Some(winit::window::Fullscreen::Borderless(
                monitor_at
                    .and_then(|_| crate::core::startup::window_monitor(event_loop, monitor_at)),
            )));
        }
        if self.settings.triple.enabled
            && self.settings.triple_span
            && !self.settings.vr_requested()
        {
            let mut monitors: Vec<_> = event_loop.available_monitors().collect();
            monitors.sort_by_key(|m| m.position().x);
            let row = monitors.windows(3).find(|row| {
                let size = row[0].size();
                row.iter()
                    .all(|m| m.size() == size && m.position().y == row[0].position().y)
                    && row[1].position().x == row[0].position().x + size.width as i32
                    && row[2].position().x == row[1].position().x + size.width as i32
            });
            if let Some(row) = row {
                let size = row[0].size();
                if self.settings.fullscreen
                    || gamescope
                    || resolution.is_some()
                    || self.args.size != crate::core::cli::DEFAULT_SIZE
                {
                    log::info!("triple screen: spanning three monitors instead of the fullscreen / window size settings");
                }
                self.spanned = true;
                attrs = attrs
                    .with_fullscreen(None)
                    .with_decorations(false)
                    .with_position(row[0].position())
                    .with_inner_size(winit::dpi::PhysicalSize::new(size.width * 3, size.height));
                log::info!("triple screen: spanning {}x{}", size.width * 3, size.height);
            } else {
                log::warn!("triple screen: no equal horizontal monitor row found; using configured window size (Surround/Eyefinity can expose one wide display)");
            }
        }
        // OMSI_BACKGROUND=1: a test window that does not take the keyboard from whoever is
        // working at the screen (OMSI_INPUT drives the handlers directly, it needs no focus)
        if omsi_cfg::flags::BACKGROUND.is_set() {
            attrs = attrs.with_active(false);
        }
        let window = match given {
            Some(w) => w,
            // (no display to open it on, a compositor that refuses it: said so, not a panic
            // report about "window")
            None => match event_loop.create_window(attrs) {
                Ok(w) => Arc::new(w),
                Err(e) => {
                    fatal_message(&format!("The game cannot open its window: {e}"));
                    crate::core::platform::exit(event_loop);
                    return;
                }
            },
        };
        // the backend is chosen here, once: under `Backend::D3d9` the renderer keeps the
        // books on wgpu's noop device and the output's device draws the window
        let backend = Backend::wanted();
        let opened = match backend {
            Backend::D3d9 => {
                crate::core::startup::books_renderer(&self.instance, self.settings.render_options())
            }
            Backend::Wgpu => {
                window_renderer(&mut self.instance, &window, self.settings.render_options())
            }
        };
        let mut renderer = match opened {
            Ok(r) => r,
            Err(e) => {
                fatal_message(&format!("The game cannot draw on this computer: {e:#}"));
                crate::core::platform::exit(event_loop);
                return;
            }
        };
        if backend == Backend::D3d9 {
            if let Err(e) = self.output.attach(
                &mut renderer,
                &window,
                self.settings.vsync,
                (self.settings.msaa, u32::from(self.settings.anisotropy)),
            ) {
                fatal_message(&format!(
                    "{e:#}. Choose another graphics API in the options."
                ));
                crate::core::platform::exit(event_loop);
                return;
            }
        }
        #[cfg(windows)]
        if self.settings.vr_requested() {
            match crate::output::Vr::new(
                &renderer,
                self.settings.vr_scale,
                self.settings.vr_desktop_mirror,
            ) {
                Ok(vr) => self.vr = Some(vr),
                Err(e) => log::error!("OpenXR could not start: {e:#}"),
            }
        }
        let upload = renderer.upload_speed_mb_s();
        log::info!("graphics: {upload:.0} MB/s copied towards the card");
        if upload < SLOW_UPLOAD_MB_S {
            log::error!("graphics: the driver copies only {upload:.0} MB/s towards the card (thousands are usual); every texture and buffer the game sends waits on it, down to a few frames a second - restarting the computer usually brings it back");
            self.service_msg = Some((format!("Graphics driver is slow ({upload:.0} MB/s): the game will stutter. Restarting the computer usually fixes it."), 30.0));
        }
        crate::lights::load_smoke_texture(&mut renderer, &self.args.root);
        crate::lights::set_corona_root(&self.args.root);
        let size = window.inner_size();
        let surface = if backend == Backend::D3d9 {
            SurfaceState::shown_elsewhere(
                &self.instance,
                window.clone(),
                &renderer,
                size.width,
                size.height,
                self.settings.vsync,
            )
        } else {
            SurfaceState::new_with(
                &self.instance,
                window.clone(),
                &renderer,
                size.width,
                size.height,
                self.settings.vsync && !self.vr_active(),
            )
        };
        let surface = match surface {
            Ok(s) => s,
            Err(e) => {
                fatal_message(&format!("The game's window cannot be drawn into: {e:#}"));
                crate::core::platform::exit(event_loop);
                return;
            }
        };
        let (sw, sh) = renderer.scene_size(size.width, size.height);
        log::info!(
            "window: {}x{} pixels (scale factor {:.2}), 3D picture {sw}x{sh}, present mode {:?}",
            size.width,
            size.height,
            window.scale_factor(),
            surface.config.present_mode
        );
        let scene = renderer.new_scene();
        self.window = Some(window);
        self.surface = Some(surface);
        self.renderer = Some(renderer);
        self.scene = Some(scene);
        // fully specified runs skip the menu
        if self.args.bus.is_some() || self.args.cam.is_some() || self.args.no_menu {
            self.load_world_now(event_loop);
        } else {
            let mut fonts = omsi_sim::texttex::FontLibrary::new(&self.args.root);
            self.hud = Some(hud::Hud::new(&mut fonts));
            self.menu = Some(menu::Menu::new(&self.args.root, &self.args.map));
        }
    }

    /// Load the map, vehicle and traffic according to `args`.
    pub(crate) fn load_world_now(&mut self, event_loop: &ActiveEventLoop) {
        let renderer = self.renderer.take().expect("renderer");
        self.output.new_scene();
        let mut scene = renderer.new_scene();
        self.envir = omsi_content::Envir::load(&self.args.root.join("envir.cfg")).ok();
        // the weather cycle: a first weather that suits the month, the others after it
        if crate::weather_cycle::is_cycle(self.args.weather.as_deref()) {
            let seed = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos() as u64)
                .unwrap_or(7);
            let mut c = crate::weather_cycle::Cycle::new(seed);
            let month = start_clock(&self.args).day_month().1;
            let all = crate::weather_cycle::installed();
            let clear = omsi_content::weather::Weather {
                fog: (50000.0, 1.0),
                ..Default::default()
            };
            let r = c.rand();
            self.args.weather = crate::weather_cycle::pick(&all, &clear, "", month, r);
            log::info!("weather cycle: starting with {:?}", self.args.weather);
            self.weather_cycle = Some(c);
        }
        self.weather = Some(load_weather(&self.args));
        // the roads start in the state this weather has already left them in, as they do
        // offscreen: a session begun in the rain used to open on a bone-dry street
        self.wetness = self.weather.as_ref().map(initial_wetness).unwrap_or(0.0);
        self.clock = start_clock(&self.args);
        setup_sky(
            &self.args,
            &renderer,
            &mut scene,
            self.envir.as_ref(),
            self.weather.as_ref(),
        );
        // the window streams the tiles around the camera unless a fixed area was asked for
        if !self.args.all && self.args.radius.is_none() {
            match open_world(&self.args) {
                Ok((w, cam, _)) => {
                    let w = Arc::new(w);
                    let distance = self
                        .args
                        .view_distance
                        .or_else(settings::view_distance)
                        .unwrap_or(1200.0)
                        .max(omsi_map::tile_size());
                    w.set_fast_texture_loads(true);
                    w.set_texture_budget(texture_budget(&self.settings));
                    log::info!(
                        "texture budget: {:.0} MB",
                        texture_budget(&self.settings) as f64 / 1e6
                    );
                    self.streamer = Some(tiles::Streamer::new(
                        w.clone(),
                        &start_centers(&self.args, &cam, Some(&w)),
                        distance,
                        700.0,
                    ));
                    self.world = Some(w);
                    // (a new drive: its loading screen shows its own map's screenshots and bus)
                    self.new_ui.reset_loading();
                    self.starting = Some(cam);
                }
                Err(e) => {
                    log::error!("{e:#}");
                    crate::core::platform::exit(event_loop);
                }
            }
            self.renderer = Some(renderer);
            self.scene = Some(scene);
            self.last = Instant::now();
            return;
        }
        match load_world(&self.args, &renderer, &mut scene) {
            Ok((w, cam)) => self.start_world(Arc::new(w), cam, &renderer, &mut scene),
            Err(e) => {
                log::error!("{e:#}");
                crate::core::platform::exit(event_loop);
            }
        }
        self.renderer = Some(renderer);
        self.scene = Some(scene);
        self.last = Instant::now();
    }

    /// Everything that needs the map to stand: the player's bus, the passengers, the traffic
    /// and the timetable.
    pub(crate) fn start_world(
        &mut self,
        w: Arc<World>,
        cam: Camera,
        renderer: &Renderer,
        mut scene: &mut Scene,
    ) {
        report_missing_content(&w, &mut self.service_msg);
        {
            {
                // (once more when it fails: a file read while the start was still reading
                // others; a failure is said on the screen - the game went on without a bus
                // and the player found himself on foot, with no word why)
                let first = spawn_player(&self.args, &w, &renderer, &mut scene);
                let spawned = match first {
                    Err(e) if self.args.bus.is_some() => {
                        log::warn!("the bus could not be put down ({e:#}); trying again");
                        spawn_player(&self.args, &w, &renderer, &mut scene).map_err(|e2| {
                            self.service_msg = Some((
                                format!(
                                    "The bus could not be loaded: {}",
                                    format!("{e2:#}").lines().next().unwrap_or_default()
                                ),
                                15.0,
                            ));
                            e2
                        })
                    }
                    other => other,
                };
                match spawned {
                    Ok(mut p) => {
                        let audio = omsi_audio::AudioEngine::new();
                        if let Some(p) = p.as_mut() {
                            p.vehicle.host.auto_clutch =
                                if self.settings.auto_clutch { 1.0 } else { 0.0 };
                            p.load_sounds(&audio);
                            p.ibis_background = true;
                            // --autostart applies in the window too, not only offscreen
                            if self.args.autostart && !self.args.is_resuming() {
                                let msg = p.start_up();
                                self.service_msg = Some((msg, 6.0));
                            }
                            // a pack this bus borrows parts from is not installed: say so
                            // once, it explains dark displays and missing devices
                            if !p.vehicle.ty.missing_packs.is_empty() {
                                let packs: Vec<String> = p
                                    .vehicle
                                    .ty
                                    .missing_packs
                                    .iter()
                                    .map(|(n, _)| n.clone())
                                    .collect();
                                let msg = omsi_ui::tr("This bus takes parts from vehicle pack(s) that are not installed: %{packs} (install them for its displays and devices)")
                                    .replace("%{packs}", &packs.join(", "));
                                self.service_msg = Some(match self.service_msg.take() {
                                    Some((m, _)) => (format!("{m}   |   {msg}"), 12.0),
                                    None => (msg, 12.0),
                                });
                            }
                        }
                        self.ambience = Some(ambience::Ambience::load(&audio, &self.args.root));
                        self.audio = Some(audio);
                        if let Some(p) = &p {
                            if self.args.cam.is_none() && self.args.view != "free" {
                                self.camera = Some(p.camera(&self.args.view, &cam));
                            }
                        }
                        self.player = p;
                    }
                    Err(e) => log::error!("{e:#}"),
                }
                // a situation's further vehicles, each as it was saved
                for o in self.args.situation_others.clone() {
                    // saved under the world (it fell through an unloaded tile, #33) with its place
                    // not loaded: its ground is unknown, and put at sea level it would fall on
                    // through elevated terrain; left out rather than placed wrongly
                    if let Some((x, y)) = crate::situation::spawn_without_height(&o.spawn) {
                        if w.ground_height(x, y).is_none() {
                            log::warn!("situation vehicle {}: saved at an implausible height and its place is not loaded; left out", o.bus);
                            continue;
                        }
                    }
                    let one = Args {
                        bus: Some(o.bus.clone()),
                        spawn: Some(o.spawn.clone()),
                        hof: o.hof.clone(),
                        paint: o.paint.clone(),
                        situation_vars: o.vars.clone(),
                        situation_strvars: o.strvars.clone(),
                        situation_odometer_km: o.odometer_km,
                        situation_script_textures: o.script_textures.clone(),
                        situation_others: Vec::new(),
                        line: None,
                        tour: None,
                        trip: None,
                        autostart: false,
                        situation_next_stop: None,
                        ..self.args.clone()
                    };
                    match spawn_player(&one, &w, &renderer, &mut scene) {
                        Ok(Some(q)) => {
                            log::info!("situation: {} placed at {}", o.bus, o.spawn);
                            self.placed.push(q);
                        }
                        Ok(None) => {}
                        Err(e) => log::warn!("situation vehicle {}: {e:#}", o.bus),
                    }
                }
                if self.camera.is_none() {
                    self.camera = Some(cam);
                }
                self.navigator = Some(nav::Navigator::new(
                    self.settings.navigator,
                    self.settings.ui_opacity,
                    &self.settings.navigator_corner,
                ));
                // (under `Backend::D3d9` the maps are drawn on the output's device too)
                let ui_mirror = self.output.ui_mirror();
                if let Some(n) = self.navigator.as_mut() {
                    n.ui_mirror = ui_mirror;
                    n.arrows = self.settings.nav_arrows;
                    n.show_ai = self.settings.nav_ai;
                    n.show_surface = self.settings.nav_surface;
                    n.light = self.settings.nav_light;
                    n.show_contours = self.settings.nav_contours;
                    n.view_top = self.settings.nav_view == "top";
                    n.orient = match self.settings.nav_orientation.as_str() {
                        "north" => 1,
                        "heading" => 2,
                        _ => 0,
                    };
                }
                if let Some(d) = self.args.driver.as_deref() {
                    self.career = career::Career::load(&self.args.root, d);
                }
                // (and a player who joins another's game sees the host's people)
                if self.args.passengers || self.args.lan_join.is_some() {
                    let mut h = humans::Humans::new(&self.args.root);
                    h.exact_fare = self.settings.exact_fare;
                    h.boarding = self.settings.boarding.clone();
                    h.voices = match self.settings.pax_voices.as_str() {
                        "off" => 2,
                        "tickets" => 1,
                        _ => 0,
                    };
                    if let Some(p) = self.player.as_mut() {
                        h.set_cabin(&mut p.vehicle);
                        h.ticket_key = ticket_key_name(&self.args.root, &p.bindings);
                        h.tickets = p.vehicle.host.tickets.clone();
                        if !w.global.money_system.trim().is_empty() {
                            h.money =
                                Some(money::Money::new(&self.args.root, &w.global.money_system));
                        }
                    }
                    if let Some(p) = self.player.as_ref() {
                        if self.args.riders > 0 {
                            let centre = p.vehicle.position;
                            h.populate(&w, &renderer, &mut scene, centre);
                            h.seed_riders(self.args.riders, &p.vehicle, &w, &renderer, &mut scene);
                        }
                    }
                    self.humans = Some(h);
                }
                // (a player who joins draws the host's traffic in it, whatever their own count
                // says: the host's cars had nowhere to go without it)
                let populated = self.args.traffic > 0
                    || self.args.schedule
                    || crate::rail_drive::args_rail(&self.args)
                    || self.args.lan_join.is_some();
                // Without traffic it still runs the light programs and switches the lamps:
                // they stood frozen with red, yellow and green all lit (#727).
                {
                    match traffic::Traffic::new(&self.args.root, &w, self.args.traffic) {
                        Ok(mut t) => {
                            t.lights_only = !populated;
                            t.no_timetable_buses = self.args.no_timetable_buses;
                            if self.args.traffic > 0 {
                                t.precache_random(&w, &renderer, &mut scene);
                            }
                            t.day_time = parse_time(&self.args.time);
                            self.traffic = Some(t);
                        }
                        Err(e) => log::error!("traffic: {e:#}"),
                    }
                    if self.args.schedule {
                        let mut sch =
                            schedule::Schedule::new(&self.args.root, &w, &start_clock(&self.args));
                        sch.precache(
                            &w,
                            &renderer,
                            &mut scene,
                            self.traffic.as_mut(),
                            parse_time(&self.args.time),
                        );
                        if let (Some(line), Some(p)) = (&self.args.line, self.player.as_mut()) {
                            self.duty = match sch.player_duty(
                                &w,
                                line,
                                self.args.tour.as_deref().unwrap_or(""),
                                parse_time(&self.args.time),
                                self.args.trip.as_deref(),
                                self.args.whole_tour,
                            ) {
                                Ok(mut d) => {
                                    if let Some(k) = self.args.duty_trip {
                                        d.start_at(k, self.args.duty_first_stop);
                                    }
                                    if self.args.is_resuming() {
                                        d.resume(
                                            &mut p.vehicle,
                                            parse_time(&self.args.time),
                                            self.args.situation_next_stop,
                                        );
                                        // the IBIS keeps its saved trip; with --autostart
                                        // the duty's next trips are typed into it again
                                        p.duty_typed = self.args.autostart;
                                    }
                                    Some(d)
                                }
                                Err(e) => {
                                    log::warn!("no player duty: {e}");
                                    self.service_msg = Some((
                                        omsi_ui::tr("No duty: %{error}")
                                            .replace("%{error}", &e.to_string()),
                                        20.0,
                                    ));
                                    None
                                }
                            };
                            // --autostart in the window puts the duty on the IBIS as well
                            // (it only ever did offscreen: the duty did not exist yet when
                            // the start-up began, and the displays stayed dark)
                            if let (true, Some(d)) = (
                                self.args.autostart && !self.args.is_resuming(),
                                self.duty.as_mut(),
                            ) {
                                d.update(&mut p.vehicle, parse_time(&self.args.time));
                                let (trip, stop) = d.trip_for_ibis();
                                p.set_duty_destination(trip, stop);
                            }
                            if let Some(d) = self.duty.as_ref() {
                                let mut fonts = w.fonts.lock();
                                if let Err(e) = crate::schedule_paper::update_vehicle(
                                    &mut p.vehicle,
                                    d,
                                    &mut fonts,
                                ) {
                                    log::warn!("driver timetable paper: {e:#}");
                                }
                            }
                        }
                        self.schedule = Some(sch);
                    }
                }
                if self.traffic.is_none() {
                    if let Some(n) = self.navigator.as_mut() {
                        n.add_lanes(std::mem::take(&mut *w.lanes.lock()));
                    }
                }
                self.world = Some(w);
            }
        }
        self.last = Instant::now();
    }

    /// The first area of a streamed map is loading: show how far it got, and start the rest
    /// of the world once it is there. Returns false while the loading screen is up.
    pub(crate) fn drive_start(&mut self, event_loop: &ActiveEventLoop) -> bool {
        let Some(cam) = self.starting.take() else {
            return true;
        };
        let (Some(mut renderer), Some(mut scene)) = (self.renderer.take(), self.scene.take())
        else {
            self.starting = Some(cam);
            return true;
        };
        let centers = start_centers(&self.args, &cam, self.world.as_deref());
        let progress = match self.streamer.as_mut() {
            Some(streamer) => {
                streamer.update(
                    &renderer,
                    &mut scene,
                    &centers,
                    std::time::Duration::from_millis(30),
                    None,
                );
                streamer.initial_progress()
            }
            None => None,
        };
        let Some((done, total)) = progress else {
            // (the loading screen of the new interface is over)
            self.new_ui.reset_loading();
            let w = self.world.clone().expect("world");
            log::info!(
                "map ready after {:.2} s: {} tiles loaded",
                self.started.elapsed().as_secs_f64(),
                w.loaded_tiles().len()
            );
            self.start_world(w, cam, &renderer, &mut scene);
            self.renderer = Some(renderer);
            self.scene = Some(scene);
            return true;
        };
        // the loading screen of the new interface, drawn by the renderer's hook over the blank picture
        self.new_loading_update(&mut renderer, done, total);
        let mut reconfigure = false;
        if let (Some(s), Some(win)) = (self.surface.as_ref(), self.window.as_ref()) {
            scene.overlays.clear();
            let acquired = s.acquire();
            // a swapchain that no longer fits the window (Vulkan says so after the switch
            // to full screen, without a resize event) is made again, as the game's own
            // frames do: left as it was, every later frame of the loading screen failed
            // the same way and its picture stood still until the map was there (#776)
            reconfigure = matches!(
                acquired,
                wgpu::CurrentSurfaceTexture::Outdated | wgpu::CurrentSurfaceTexture::Lost
            );
            let (frame, view) = match acquired {
                wgpu::CurrentSurfaceTexture::Success(frame)
                | wgpu::CurrentSurfaceTexture::Suboptimal(frame) => {
                    let view = frame.texture.create_view(&Default::default());
                    (Some(frame), Some(view))
                }
                // ("DirectX9 (D3D9Ex)": drawn into a stand-in, shown by its device)
                wgpu::CurrentSurfaceTexture::Occluded if s.is_shown_elsewhere() => (
                    None,
                    Some(stand_in_view(
                        &mut self.stand_in,
                        &renderer,
                        s.config.width,
                        s.config.height,
                    )),
                ),
                _ => (None, None),
            };
            if let Some(view) = view {
                // the tiles loaded so far stay out of the picture: the camera looks at nothing
                let blank = Camera {
                    position: DVec3::new(0.0, 0.0, -1.0e6),
                    yaw: 0.0,
                    pitch: -89.0,
                    roll: 0.0,
                    fov_deg: 60.0,
                    near: 0.5,
                    far: 10.0,
                };
                let lighting = omsi_render::Lighting {
                    sky_color: glam::Vec3::new(0.08, 0.10, 0.14),
                    ..Default::default()
                };
                let mut renderer = renderer;
                renderer.render(
                    &mut scene,
                    &view,
                    s.config.width,
                    s.config.height,
                    &blank,
                    &lighting,
                );
                if let Some(frame) = frame {
                    win.pre_present_notify();
                    frame.present();
                }
                self.renderer = Some(renderer);
                self.output.present();
            } else {
                self.renderer = Some(renderer);
            }
            win.request_redraw();
        } else {
            self.renderer = Some(renderer);
        }
        if reconfigure {
            if let (Some(s), Some(r), Some(win)) = (
                self.surface.as_mut(),
                self.renderer.as_ref(),
                self.window.as_ref(),
            ) {
                let size = win.inner_size();
                s.resize(r, size.width, size.height);
            }
        }
        self.scene = Some(scene);
        self.starting = Some(cam);
        if let Some(limit) = self.args.exit_after {
            if self.started.elapsed().as_secs_f32() > limit {
                log::info!("exit after {limit} s while loading: {done} of {total} tiles");
                crate::core::platform::exit(event_loop);
            }
        }
        false
    }

    /// Stream the tiles around the camera and the player's bus (whose ground must stay when
    /// the free camera flies off); when the loaded set changed, hand the new data to whatever
    /// keeps its own copy (the bus's obstacles, the traffic network, the navigator, the
    /// street lamps).
    pub(crate) fn drive_streaming(&mut self) {
        let mut centers: Vec<DVec3> = self.camera.iter().map(|c| c.position).collect();
        centers.extend(self.player.iter().map(|p| p.vehicle.position));
        let (Some(streamer), Some(w), Some(r), Some(scene)) = (
            self.streamer.as_mut(),
            self.world.as_ref(),
            self.renderer.as_ref(),
            self.scene.as_mut(),
        ) else {
            return;
        };
        // textures uploaded as RGBA to spare a frame, compressed on the workers since, and the
        // texture budget
        w.apply_texture_upgrades(
            r,
            scene,
            Some(Instant::now() + std::time::Duration::from_millis(2)),
        );
        w.update_texture_budget(r, scene, &centers, false);
        if centers.is_empty() {
            return;
        }
        let changed = streamer.update(
            r,
            scene,
            &centers,
            std::time::Duration::from_millis(6),
            self.audio.as_ref(),
        );
        // Uploads can temporarily exceed the texture budget before the next frame's
        // housekeeping pass. Recheck immediately after streaming so far textures are
        // reduced before the renderer allocates more frame resources.
        if !changed {
            return;
        }
        w.update_texture_budget(r, scene, &centers, true);
        if let Some(p) = self.player.as_mut() {
            // (OMSI's [no_collision]: no solid object stops the bus)
            p.vehicle.collision = self
                .settings
                .collision_objects
                .then(|| w.collision.lock().clone());
            p.vehicle.wheel_walls = self.settings.collision_objects;
        }
        match self.traffic.as_mut() {
            Some(t) => {
                t.add_tiles(w);
            }
            None => {
                // no traffic system: the navigator keeps the roads for its map
                let lanes = std::mem::take(&mut *w.lanes.lock());
                if let Some(n) = self.navigator.as_mut() {
                    n.add_lanes(lanes);
                }
            }
        }
        if let Some(on) = self.lamps_on {
            w.set_lamps(r, scene, on);
        }
    }
}

/// Where a streamed map loads around while it starts: the vehicle's spawn point (a bus must
/// stand on ground when it appears, whatever `--cam` says), and the camera unless the view
/// follows the bus (every view but the free camera does, `--cam` or not).
pub fn start_centers(args: &Args, cam: &Camera, world: Option<&World>) -> Vec<DVec3> {
    let mut out: Vec<DVec3> = spawn_point(args, world).into_iter().collect();
    let view_in_bus = args.bus.is_some() && args.view != "free";
    if out.is_empty() || !view_in_bus {
        out.push(cam.position);
    }
    out
}

/// What the map needs and this installation lacks, said on the screen and written to
/// `~/.depot/missing_content.txt` by add-on folder: a map short of an add-on showed
/// holes, bare roads and white objects, and nobody could tell that from a fault of the game.
pub(crate) fn report_missing_content(w: &World, msg: &mut Option<(String, f32)>) {
    let (files, textures) = w.missing_content();
    if files.is_empty() && textures.is_empty() {
        return;
    }
    // the add-on a file comes with: the folder under Sceneryobjects / Splines / Vehicles
    let addon = |f: &str| f.split('/').take(2).collect::<Vec<_>>().join("/");
    let mut by_addon: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for (f, what) in &files {
        by_addon
            .entry(addon(f))
            .or_default()
            .push(format!("{what}: {f}"));
    }
    let mut text = format!(
        "Project Depot: content this map uses that is not installed\nmap: {}\n\n",
        w.map_dir.display()
    );
    for (a, list) in &by_addon {
        text.push_str(&format!("{a} ({} files)\n", list.len()));
        for l in list {
            text.push_str(&format!("  {l}\n"));
        }
    }
    if !textures.is_empty() {
        text.push_str(&format!("\ntextures not found ({}):\n", textures.len()));
        for t in &textures {
            text.push_str(&format!("  {t}\n"));
        }
    }
    let Some(dir) = crate::settings::data_dir() else {
        return;
    };
    let path = dir.join("missing_content.txt");
    let _ = std::fs::write(&path, text);
    let objects = files.iter().filter(|(_, w)| *w != "spline").count();
    let splines = files.len() - objects;
    let addons: Vec<&String> = by_addon.keys().take(4).collect();
    let more = by_addon.len().saturating_sub(4);
    log::warn!(
        "missing content: {objects} objects, {splines} splines, {} textures (list: {})",
        textures.len(),
        path.display()
    );
    if !files.is_empty() {
        let text = if more > 0 {
            "This map uses %{objects} objects and %{splines} splines that are not installed (add-ons: %{addons} and %{more} more). The list is in %{path}"
        } else {
            "This map uses %{objects} objects and %{splines} splines that are not installed (add-ons: %{addons}). The list is in %{path}"
        };
        *msg = Some((
            omsi_ui::tr(text)
                .replace("%{objects}", &objects.to_string())
                .replace("%{splines}", &splines.to_string())
                .replace(
                    "%{addons}",
                    &addons
                        .iter()
                        .map(|a| a.as_str())
                        .collect::<Vec<_>>()
                        .join(", "),
                )
                .replace("%{more}", &more.to_string())
                .replace("%{path}", &path.display().to_string()),
            15.0,
        ));
    }
}

/// How long the glide between two cockpit cameras takes (seconds). The eye, the turn of the
/// view and the field of view all follow the same curve over this time. 0 = hard cut.
pub(crate) const CAM_BLEND_SECS: f32 = 0.54;

/// The longest step of time one frame adds to the glide (seconds): a frame that hitches at
/// the start of a switch does not skip ahead in it.
pub(crate) const CAM_BLEND_MAX_DT: f32 = 1.0 / 30.0;

/// `a` (k = 0) to `b` (k = 1), both cameras fixed in the bus's frame: the eye, the turn of the
/// view and the field of view on a straight way. The bus's own motion (its pitch, bank, the
/// head) is put on the result afterwards, so the glide is the same standing and driving.
pub(crate) fn blend_local(
    a: &omsi_vehicle::Camera,
    b: &omsi_vehicle::Camera,
    k: f32,
) -> omsi_vehicle::Camera {
    // (measured from `b`: at k = 1 every value is exactly `b`'s - no 360 degree residue of a
    // yaw that went the short way round, no rounding left over for the hand-over to the
    // plain camera to show)
    let k = k.clamp(0.0, 1.0);
    if k >= 1.0 {
        return b.clone();
    }
    let rest = 1.0 - k;
    let l = |x: f32, y: f32| y + (x - y) * rest;
    // The view direction turns along the great circle between the two (a slerp of the
    // directions), not yaw and pitch each on their own straight line: that swept the view
    // out in a bow - up and across at once - while the eye went straight, which looked like
    // a zigzag in the glide.
    let dir = |c: &omsi_vehicle::Camera| {
        let (sy, cy) = c.yaw.to_radians().sin_cos();
        let (sp, cp) = c.pitch.to_radians().sin_cos();
        glam::Vec3::new(sy * cp, cy * cp, sp)
    };
    let (fa, fb) = (dir(a), dir(b));
    let dot = fa.dot(fb).clamp(-1.0, 1.0);
    let (yaw, pitch) = if dot < -0.9995 {
        // (turned right round: no one great circle, so the plain way)
        (b.yaw - wrap_deg(b.yaw - a.yaw) * rest, l(a.pitch, b.pitch))
    } else {
        let f = if dot > 0.9995 {
            (fa * rest + fb * k).normalize_or(fb)
        } else {
            let theta = dot.acos();
            let s = theta.sin();
            ((fa * ((rest * theta).sin() / s)) + (fb * ((k * theta).sin() / s))).normalize_or(fb)
        };
        (
            f.x.atan2(f.y).to_degrees(),
            f.z.clamp(-1.0, 1.0).asin().to_degrees(),
        )
    };
    omsi_vehicle::Camera {
        pos: [
            l(a.pos[0], b.pos[0]),
            l(a.pos[1], b.pos[1]),
            l(a.pos[2], b.pos[2]),
        ],
        dist: l(a.dist, b.dist),
        fov: l(a.fov, b.fov),
        yaw,
        pitch,
        extra: b.extra,
        fixed: b.fixed,
    }
}

/// The texture a picture is drawn into when the window gives none (OMSI_RENDER_OCCLUDED, or
/// "DirectX9 (D3D9Ex)", whose device shows the picture), made anew at a new size.
pub(crate) fn stand_in_view(
    slot: &mut Option<wgpu::Texture>,
    r: &omsi_render::Renderer,
    w: u32,
    h: u32,
) -> wgpu::TextureView {
    if slot
        .as_ref()
        .map(|t| (t.width(), t.height()) != (w, h))
        .unwrap_or(true)
    {
        *slot = Some(r.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("hidden window"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: r.format(),
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        }));
    }
    slot.as_ref()
        .expect("made above")
        .create_view(&Default::default())
}
