//! Script host for road vehicles: system variables, callbacks, sound triggers.

mod callbacks;

use crate::textures::scripttex::{FontTable, ScriptTexture};
use crate::textures::texttex::FontLibrary;
use crate::SimClock;
use omsi_script::{Host, NameId, Stacks, State, SysVar};
use omsi_vehicle::hof::Hof;
use parking_lot::Mutex;
use std::sync::Arc;

/// How many stops a page's departures are kept for at the same time (`omsi.getDepartures`).
pub const MAX_HTML_DEPARTURE_STOPS: usize = 8;

/// `wearlifespan` of a vehicle that does not wear (OMSI: every AI vehicle, and the
/// player's with the maintenance option "infinite").
pub const AI_WEAR_LIFESPAN: f32 = 1.5e6;

#[derive(Default)]
pub struct VehicleHost {
    /// The paint scheme the vehicle is made with (`Some(None)`: the model's own textures),
    /// set before it is made: `Colorscheme` and the scheme's `[setvar]`s are there for its
    /// `{init}`, as OMSI sets them when it makes the vehicle, before the
    /// scripts start. None: not known yet (`apply_paint_vars` later).
    pub paint_scheme: Option<Option<usize>>,
    /// Fleet number and registration chosen by the vehicle dialog. They are copied to the
    /// script's `number` / `ident` strings before `{init}`, like OMSI does.
    pub initial_number: Option<String>,
    pub initial_ident: Option<String>,
    /// A time of day a script wrote to `(S.S.Time)` this frame: the game's clock takes it.
    pub time_written: Option<f64>,
    pub clock: SimClock,
    pub mouse: (f32, f32),
    pub precip_type: f32,
    pub precip_rate: f32,
    /// `StreetCond`: how the road under the vehicle is - 0 dry, 1 wet, 2 covered in snow,
    /// and everything in between. The engine feeds it like `Dirt_Norm` (no varlist declares
    /// it); the stock sound configurations fade `Sounds\WetLane_1.wav` in over 0 … 1 and
    /// `WetLane_2.wav` over 1 … 2, and `spray.osc` raises the wheel spray while it is
    /// between 1 and 2.
    pub street_cond: f32,
    /// People aboard (`humans_count`), set by whoever carries them.
    pub humans_count: f32,
    /// 1 while the vehicle drives a timetable duty (`schedule_active`).
    pub schedule_active: f32,
    /// The odometer's reading when the session began (km).
    pub km_base: f64,
    pub temperature: f32,
    pub abs_humidity: f32,
    pub sun_alt: f32,
    /// Last collision: position (vehicle frame, m) and energy (kJ), for the coll_* sysvars.
    /// The energy is this frame's crash: the vehicle clears it once its `collision` block
    /// has run.
    pub coll_pos: [f32; 3],
    pub coll_energy: f32,
    /// `wearlifespan`: how long wearing parts last, 1 = normal. It scales every random
    /// lifetime the scripts draw (rear-door automatic, bulbs, the matrix display). At 0 the
    /// SD200's rear door was "worn out" on its first closing and reopened by itself with the
    /// opening sound from then on.
    pub wear_lifespan: f32,
    /// Ground height (m) under a point given in the vehicle frame, minus the vehicle's own
    /// height: filled every frame by `VehicleInstance::update` for `GetHeightAbovePoint`.
    pub ground_probe: Option<Arc<dyn Fn(f32, f32, f32) -> f32 + Send + Sync>>,
    /// People standing on each `paths.cfg` link inside the cabin (`GetHumanCountOnPathLink`).
    pub humans_on_path_link: Vec<u32>,
    /// People sitting on each `[passpos]` (`GetHumanCountOnSeat`).
    pub humans_on_seat: Vec<u32>,
    /// Callbacks a script asked for that this host does not provide (reported once each).
    unknown_callbacks: Vec<String>,
    pub auto_clutch: f32,
    pub no_sound: f32,
    pub fired_triggers: Vec<String>,
    /// Triggers (lower case) whose sounds read variables in their volume curves: when one
    /// fires, the variables of that moment are kept in `fired_trigger_vars` (the door's
    /// hit sound reads `doorSpeed_<n>`, which the script turns round right after it).
    pub snapshot_triggers: hashbrown::HashSet<String>,
    pub fired_trigger_vars: Vec<(String, Vec<f32>)>,
    /// `(T.F.name)` triggers of this frame: (trigger, sound file relative to the sound folder).
    pub fired_file_triggers: Vec<(String, String)>,
    pub messages: Vec<String>,
    /// Fonts registered by `GetFontIndex` (index = position), loaded through `font_lib`.
    pub fonts: FontTable,
    pub font_lib: Option<Arc<Mutex<FontLibrary>>>,
    /// `[scripttexture]` images drawn by the `ST*` callbacks.
    pub script_textures: Vec<ScriptTexture>,
    /// Folder for `STLoadTex` paths (the vehicle directory).
    pub content_dir: std::path::PathBuf,
    /// Depot file (termini, bus stop strings, IBIS trips) behind the `Get*` callbacks.
    pub hof: Option<Arc<Hof>>,
    /// The map's ticket pack for `GetTicketName/Value`.
    pub tickets: Option<Arc<omsi_content::tickets::TicketPack>>,
    /// Coins the driver handed out with `GiveChangeCoin` (currency coin indices), taken by
    /// the passenger system.
    pub change_coins: Vec<usize>,
    /// The script's `number` string variable (the fleet number): `NrSpecRandom` is
    /// specific to it.
    pub number_var: Option<u32>,
    /// Timetable information for the `GetTT*` callbacks (line, delay in s, stops).
    pub tt_line: String,
    pub tt_delay: f32,
    pub tt_stops: Vec<(String, f32, f32)>,
    /// The map objects of `tt_stops` (0: not known).
    pub tt_stop_ids: Vec<i64>,
    pub tt_terminus_index: i32,
    pub tt_busstop_index: i32,
    /// The next buses due at the bus stop a scenery object belongs to (its `[varparent]`),
    /// soonest first, for the `GetArrBus*` callbacks of the stop's departure displays.
    pub arrivals: Vec<Arrival>,
    /// Route, line and destination requests of the vehicle's HTML pages, taken by the game
    /// (`VehicleInstance::take_html_requests`).
    pub html_requests: Vec<crate::htmltex::HtmlRequest>,
    /// The departures the game made for the stops the pages asked for (`omsi.getDepartures`),
    /// by key (trimmed, lower case): (line, destination, timestamp), soonest first.
    pub html_departures: std::collections::HashMap<String, Vec<(String, String, f64)>>,
    /// Which board generation of the game `html_departures` is from.
    pub html_departures_gen: u64,
    /// The stops the pages asked departures for, taken by the game: the `MAX_HTML_DEPARTURE_STOPS`
    /// asked most recently, the oldest first (see [`VehicleHost::want_departures`]).
    pub html_departure_wants: Vec<String>,
}

/// A bus due at a stop (`GetArrBusLine`, `GetArrBusTerminus`, `GetArrBusTimeDiff`).
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Arrival {
    pub line: String,
    pub terminus: String,
    /// Seconds until it arrives: its timetable time plus its delay, less the time of day.
    /// Negative once it is due and not there yet (the stock display shows 0 and blinks).
    pub due: f32,
}

/// `STLoadTex`: a destination display loads its bitmap every time it changes, in every bus
/// of the fleet - read from the disk (or an archive) in the frame each time. The pictures
/// are kept (up to 64 MB of them), the misses too.
fn st_load(path: &std::path::Path) -> Result<Arc<omsi_texture::Image>, String> {
    type Cache = Mutex<(
        std::collections::HashMap<std::path::PathBuf, Result<Arc<omsi_texture::Image>, String>>,
        usize,
    )>;
    static CACHE: std::sync::OnceLock<Cache> = std::sync::OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);
    if let Some(hit) = cache.lock().0.get(path) {
        return hit.clone();
    }
    let got = omsi_texture::decode_file(path)
        .map(Arc::new)
        .map_err(|e| e.to_string());
    let mut c = cache.lock();
    let bytes = got.as_ref().map(|i| i.rgba.len()).unwrap_or(0);
    if c.1 + bytes > 64 << 20 {
        c.0.clear();
        c.1 = 0;
    }
    c.1 += bytes;
    c.0.insert(path.to_path_buf(), got.clone());
    got
}

/// The weather's air temperature and humidity (bits of two f32), known before any vehicle
/// is made: the `{init}` of a bus reads `Weather_Temperature` for its engine and cabin (the
/// PAZ and LAZ carburettor engines, every heater script). Set by the game whenever the
/// weather is loaded or changes; NaN until then.
static AMBIENT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(u64::MAX);

/// Tell vehicles made from now on what the weather is (see [`VehicleHost::new`]).
pub fn set_ambient_weather(temperature: f32, abs_humidity: f32) {
    let bits = ((temperature.to_bits() as u64) << 32) | abs_humidity.to_bits() as u64;
    AMBIENT.store(bits, std::sync::atomic::Ordering::Relaxed);
}

fn ambient_weather() -> Option<(f32, f32)> {
    let bits = AMBIENT.load(std::sync::atomic::Ordering::Relaxed);
    (bits != u64::MAX).then(|| {
        (
            f32::from_bits((bits >> 32) as u32),
            f32::from_bits(bits as u32),
        )
    })
}

impl VehicleHost {
    /// A page asked for the departures of stop `key` (trimmed, lower case). The game keeps the
    /// stops asked most recently: one asked again moves to the back, a new one past the limit
    /// pushes out the one asked longest ago. (A fixed first-come list stayed full for good, and
    /// every later stop - the bus drives on to new ones - got no departures at all.)
    pub fn want_departures(&mut self, key: String) {
        if let Some(i) = self.html_departure_wants.iter().position(|k| *k == key) {
            let k = self.html_departure_wants.remove(i);
            self.html_departure_wants.push(k);
            return;
        }
        if self.html_departure_wants.len() >= MAX_HTML_DEPARTURE_STOPS {
            self.html_departure_wants.remove(0);
        }
        self.html_departure_wants.push(key);
    }

    /// Forget the sound triggers fired so far, and the variables kept with them. A bus is
    /// silent as it is put down: what its `{init}` and first steps fired (the stock scripts
    /// fire `ev_engineshutdown` while setting the engine up) happened before anyone could
    /// listen, and OMSI 2's sound set does not exist yet to hear it. The `(T.F.)` file
    /// triggers, the announcements, are left as they are.
    pub fn forget_fired_sound_triggers(&mut self) {
        self.fired_triggers.clear();
        self.fired_trigger_vars.clear();
    }

    pub fn new(clock: SimClock) -> Self {
        // the weather is there before {init} runs: made at 0 °C (the value before the first
        // weather update) every engine was cold, and the PAZ's carburettor engine, which
        // wants its choke below 0 °C, caught and died again on the key
        let (temperature, abs_humidity) = ambient_weather().unwrap_or((0.0, 0.0));
        // `AutoClutch` is 1 unless OMSI's options say `[no_automaticClutch]` (OMSI sets
        // 1.0 before reading them): the manual gearboxes of the Sprinters, the PAZ and the
        // LAZ work their clutch themselves then, and at 0 every gear a driver without a
        // clutch pedal put in was thrown out again at once
        Self {
            clock,
            wear_lifespan: AI_WEAR_LIFESPAN,
            auto_clutch: 1.0,
            temperature,
            abs_humidity,
            ..Default::default()
        }
    }

    /// A host for trying the scripts out on a copy of the vehicle's state (the IBIS typist
    /// plans its key presses that way): the clock, weather, depot, tickets and timetable,
    /// but no fonts, script textures or sounds.
    pub fn scratch(&self) -> VehicleHost {
        VehicleHost {
            clock: self.clock.clone(),
            precip_type: self.precip_type,
            precip_rate: self.precip_rate,
            street_cond: self.street_cond,
            temperature: self.temperature,
            abs_humidity: self.abs_humidity,
            sun_alt: self.sun_alt,
            wear_lifespan: self.wear_lifespan,
            humans_on_path_link: self.humans_on_path_link.clone(),
            humans_on_seat: self.humans_on_seat.clone(),
            unknown_callbacks: self.unknown_callbacks.clone(),
            auto_clutch: self.auto_clutch,
            no_sound: self.no_sound,
            content_dir: self.content_dir.clone(),
            hof: self.hof.clone(),
            tickets: self.tickets.clone(),
            tt_line: self.tt_line.clone(),
            tt_delay: self.tt_delay,
            tt_stops: self.tt_stops.clone(),
            tt_stop_ids: self.tt_stop_ids.clone(),
            tt_terminus_index: self.tt_terminus_index,
            tt_busstop_index: self.tt_busstop_index,
            ..Default::default()
        }
    }

    /// Callbacks the scripts called that this host does not provide (lower case).
    pub fn unknown_callbacks(&self) -> &[String] {
        &self.unknown_callbacks
    }

    /// The font behind a `GetFontIndex` result; -1 (a missing font) draws nothing.
    fn font_atlas(&self, index: i32) -> Option<Arc<omsi_content::font::FontAtlas>> {
        usize::try_from(index)
            .ok()
            .and_then(|i| self.fonts.entries.get(i))
            .and_then(|f| f.1.clone())
    }

    fn terminus(&self, index: i32) -> Option<&omsi_vehicle::hof::Terminus> {
        self.hof
            .as_ref()
            .and_then(|h| h.termini.get(usize::try_from(index).ok()?))
    }

    fn arrival(&self, index: i32) -> Option<&Arrival> {
        self.arrivals.get(usize::try_from(index).ok()?)
    }

    fn bus_stop(&self, index: i32) -> Option<&omsi_vehicle::hof::BusStop> {
        self.hof
            .as_ref()
            .and_then(|h| h.bus_stops.get(usize::try_from(index).ok()?))
    }
}

/// The `(M.V.…)` callbacks this host answers (lower case); anything else reads 0.
pub const PROVIDED_CALLBACKS: &[&str] = &[
    "getfontindex",
    "textlength",
    "stnewtex",
    "stlock",
    "stunlock",
    "stfilter",
    "stsetcolor",
    "stdrawpixel",
    "stdrawrect",
    "sttextout",
    "streadpixel",
    "stgetr",
    "stgetg",
    "stgetb",
    "stgeta",
    "stcopycolor",
    "stloadtex",
    "getrouteindex",
    "getrouteterminusindex",
    "getterminuscode",
    "getterminusindex",
    "getterminusstring",
    "getbusstopcount",
    "getroutebusstopident",
    "getbusstopindex",
    "getbusstopstring",
    "getdepotstringglobal",
    "getttlinestring",
    "getttdelay",
    "getttbusstopcount",
    "getttbusstopindex",
    "gettterminusindex",
    "getttterminusindex",
    "getttbusstopname",
    "getttbusstopdep",
    "getttbusstoparr",
    "getheightabovepoint",
    "gethumancountonpathlink",
    "gethumancountonseat",
    "givechangecoin",
    "nrspecrandom",
    "getticketname",
    "gettticketname",
    "getticketvalue",
    "getarrbusline",
    "getarrbusterminus",
    "getarrbustimediff",
];

/// A number a script hands a callback, as OMSI takes it: rounded to the nearest
/// integer, a half to the even one (`fistp` under Delphi's control word; vehicles and scenery objects
/// convert every argument that way), not cut
/// off - 1100.9999 out of float arithmetic is 1101.
pub(crate) fn arg_i32(v: f32) -> i32 {
    v.round_ties_even() as i32
}

/// The same as an index (a negative one names nothing).
pub(crate) fn arg_idx(v: f32) -> usize {
    usize::try_from(arg_i32(v)).unwrap_or(usize::MAX)
}

impl Host for VehicleHost {
    fn sys_var(&mut self, v: SysVar) -> f32 {
        match v {
            SysVar::Timegap => self.clock.timegap,
            SysVar::GetTime => self.clock.run_time as f32,
            SysVar::NoSound => self.no_sound,
            SysVar::Pause => {
                if self.clock.paused {
                    1.0
                } else {
                    0.0
                }
            }
            // seconds since midnight ("Time in seconds", says the stock clock object): every
            // script divides it by 3600 for the hour (IBIS and ticket printer clocks) or
            // compares it with 28800/72000 (the Citaro's door chime by day). Handing out hours
            // made every clock read 00:00 and the day-time checks think it was night.
            SysVar::Time => self.clock.time as f32,
            SysVar::Day => self.clock.day_month().0 as f32,
            SysVar::Month => self.clock.day_month().1 as f32,
            SysVar::Year => self.clock.year as f32,
            SysVar::DayOfYear => self.clock.day_of_year as f32,
            SysVar::MouseX => self.mouse.0,
            SysVar::MouseY => self.mouse.1,
            SysVar::PrecipType => self.precip_type,
            SysVar::PrecipRate => self.precip_rate,
            SysVar::CollPosX => self.coll_pos[0],
            SysVar::CollPosY => self.coll_pos[1],
            SysVar::CollPosZ => self.coll_pos[2],
            // Read as often as the block likes: the stock collision block adds it to the
            // general damage and then again to the engine's, and a value gone after the
            // first read left the engine and the drivetrain whole after any crash.
            SysVar::CollEnergy => self.coll_energy,
            SysVar::WeatherTemperature => self.temperature,
            SysVar::WeatherAbsHum => self.abs_humidity,
            SysVar::WearLifespan => self.wear_lifespan,
            SysVar::AutoClutch => self.auto_clutch,
            SysVar::SunAlt => self.sun_alt,
        }
    }

    /// `(S.S.x)`: OMSI writes through the system variable's pointer into the engine's
    /// own value (`TXPC_calcblock_var`). What the engine sets anew every frame (the frame
    /// time, the mouse, the weather, the sun) takes the write for the rest of this frame
    /// only; the clock keeps it - `time_written` hands a new time of day to the game - and so
    /// do the collision values and the switches.
    fn set_sys_var(&mut self, v: SysVar, value: f32) {
        if !value.is_finite() {
            return;
        }
        match v {
            SysVar::Timegap => self.clock.timegap = value,
            SysVar::GetTime => self.clock.run_time = value as f64,
            SysVar::NoSound => self.no_sound = value,
            SysVar::Pause => self.clock.paused = value > 0.5,
            SysVar::Time => {
                self.clock.time = (value as f64).rem_euclid(86400.0);
                self.time_written = Some(self.clock.time);
            }
            SysVar::Day | SysVar::Month | SysVar::Year => {
                let (d, m) = self.clock.day_month();
                let (y, m, d) = match v {
                    SysVar::Day => (self.clock.year, m, value as i32),
                    SysVar::Month => (self.clock.year, value as i32, d),
                    _ => (value as i32, m, d),
                };
                self.clock.set_date(y, m, d);
            }
            SysVar::DayOfYear => {
                self.clock.day_of_year =
                    (value as i32).clamp(1, crate::clock::days_in_year(self.clock.year))
            }
            SysVar::MouseX => self.mouse.0 = value,
            SysVar::MouseY => self.mouse.1 = value,
            SysVar::PrecipType => self.precip_type = value,
            SysVar::PrecipRate => self.precip_rate = value,
            SysVar::CollPosX => self.coll_pos[0] = value,
            SysVar::CollPosY => self.coll_pos[1] = value,
            SysVar::CollPosZ => self.coll_pos[2] = value,
            SysVar::CollEnergy => self.coll_energy = value,
            SysVar::WeatherTemperature => self.temperature = value,
            SysVar::WeatherAbsHum => self.abs_humidity = value,
            SysVar::WearLifespan => self.wear_lifespan = value,
            SysVar::AutoClutch => self.auto_clutch = value,
            SysVar::SunAlt => self.sun_alt = value,
        }
    }

    fn callback(&mut self, name: &str, _id: NameId, stacks: &mut Stacks, state: &mut State) {
        let mut buf = [0u8; 64];
        let owned;
        let lname: &str = match buf.get_mut(..name.len()) {
            Some(b) => {
                b.copy_from_slice(name.as_bytes());
                b.make_ascii_lowercase();
                std::str::from_utf8(b).unwrap_or(name)
            }
            None => {
                owned = name.to_ascii_lowercase();
                &owned
            }
        };
        if lname.starts_with("st") && debug_text() {
            log::info!(
                "callback {name} stack {:?} strings {:?}",
                &stacks.st[..stacks.st.len().min(8)],
                stacks.sst.last()
            );
        }
        if self.cb_fonts(lname, stacks)
            || self.cb_script_tex(lname, stacks)
            || self.cb_depot(lname, stacks)
            || self.cb_timetable(lname, stacks)
            || self.cb_passengers(lname, stacks, state)
            || self.cb_scenery(lname, stacks)
        {
            return;
        }
        // numeric callbacks default to 0; say once which one the host lacks
        // numeric callbacks default to 0; say once which one the host lacks
        if !self.unknown_callbacks.iter().any(|n| n == lname) {
            log::warn!("script callback (M.V.{name}) is not provided; it reads 0");
            self.unknown_callbacks.push(lname.to_string());
        }
        stacks.push(0.0);
    }

    fn sound_trigger_file(&mut self, name: &str, file: &str) {
        // `" " (T.F.ev_nbs_on)` is how the MB_C2 gearbox script says "no sound" (every
        // frame in neutral, 80 times a minute per bus at Ahlheim): nothing to play
        if file.trim().is_empty() {
            return;
        }
        self.fired_file_triggers
            .push((name.to_string(), file.to_string()));
    }
    fn sound_trigger(&mut self, name: &str, _id: NameId) {
        self.fired_triggers.push(name.to_string());
    }
    fn sound_trigger_vars(&mut self, name: &str, id: NameId, vars: &[f32]) {
        self.sound_trigger(name, id);
        if !self.snapshot_triggers.is_empty() {
            let key = name.to_ascii_lowercase();
            if self.snapshot_triggers.contains(&key) {
                self.fired_trigger_vars.push((key, vars.to_vec()));
            }
        }
    }

    /// `$msg`: kept as the last few (OMSI shows the latest on its debug line; every
    /// AI bus says one per stop, and the list grew all session).
    fn message(&mut self, text: &str) {
        const KEEP: usize = 16;
        log::debug!("script message: {text}");
        if self.messages.len() >= KEEP {
            self.messages.remove(0);
        }
        self.messages.push(text.to_string());
    }
}

fn debug_text() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::flags::DEBUG_TEXT.is_set())
}
