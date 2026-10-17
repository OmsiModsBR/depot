//! Runtime for a sound configuration attached to an object with script variables.
//!
//! Everything here follows the behaviour OMSI 2 shows when it plays each entry through its
//! own DirectSound buffer:
//!
//! * volume = the entry's volume x every `[volcurve]` (linear between `[pnt]`s, held at the
//!   ends), x `min(range / distance, 1)` for a `[3d]` sound, x `0.2 + Snd_OutsideVol` for an
//!   AI vehicle while the camera is in a cab, x `Snd_OutsideVol` for the own bus's outside
//!   sounds heard in the cab - as a linear amplitude, turned into hundredths of a dB
//!   (`2000 * log10`) for `SetVolume`: over 0 dB or under -100 dB the call is refused and the
//!   buffer keeps the volume it had, at -100 dB or below the sound is not started;
//! * pitch = a `[loopsound]`'s `rate * |variable| / reference` for `SetFrequency` (refused
//!   outside DirectSound's 100 to 200 000 Hz; not started below 100 Hz); a `[sound]` plays
//!   at its file's rate;
//! * a `[3d]` sound is panned by at most `50 * [sound_stereo]` hundredths of a dB (see
//!   `mixer::pan_gains`), nothing else is spatial: a sound without `[3d]` is heard at the
//!   same level wherever the listener is - on an AI vehicle too.

use crate::mixer::{AudioEngine, Clip, VoiceId, VoiceParams};
use glam::{Mat4, Vec3};
use omsi_vehicle::{SoundCfg, SoundEntry};
use std::path::Path;
use std::sync::Arc;

struct RuntimeSound {
    def: SoundEntry,
    clip: Option<Arc<Clip>>,
    voice: Option<VoiceId>,
    /// The conditions held last frame (a `[noloop]` entry without a trigger plays once
    /// when they start to hold).
    held: bool,
    /// Since when the conditions hold - what a `[volcurve] -1` reads, see
    /// [`SoundSet::curve_input`].
    active_since: Option<std::time::Instant>,
    /// The buffer's volume (linear) as DirectSound last took it: a `SetVolume` over 0 dB
    /// or under -100 dB is refused and the buffer keeps this one. A new buffer is at 0 dB.
    last_gain: f32,
    /// The buffer's playback rate relative to the clip, as last taken by `SetFrequency`.
    last_pitch: f32,
    /// The loudest the curves have made a triggered entry since its trigger fired: OMSI 2
    /// never lets such a sound get quieter while it plays (`TSound` +0x2c, reset when the
    /// trigger fires, peak hold @0x7507bc), so a door sound whose curve follows the door keeps
    /// its tail when the door stops (#131).
    peak: f32,
    /// When a loop of this entry was last stopped for its volume or pitch, and what the
    /// variables its curves and pitch read stood at then (`OMSI_DEBUG_AUDIO` counts the
    /// loops started again soon after, see [`note_loop_restart`]).
    stopped_at: Option<(std::time::Instant, String)>,
}

impl RuntimeSound {
    fn new(def: SoundEntry, clip: Option<Arc<Clip>>) -> RuntimeSound {
        RuntimeSound {
            def,
            clip,
            voice: None,
            held: false,
            active_since: None,
            last_gain: 1.0,
            last_pitch: 1.0,
            peak: 0.0,
            stopped_at: None,
        }
    }
}

/// A loop started again this soon after it was stopped is counted by `OMSI_DEBUG_AUDIO`:
/// each new start is from a random place of the clip (as OMSI 2 does), so a loop stopped
/// and started over and over is heard as short fragments (#35).
const QUICK_RESTART: f32 = 1.0;

/// A loop that was started again within [`QUICK_RESTART`] of being stopped.
#[derive(Debug, Clone)]
pub struct LoopRestart {
    pub file: String,
    /// How many times since the last [`take_loop_restarts`].
    pub count: u32,
    /// The shortest pause between a stop and the start after it (s).
    pub shortest: f32,
    /// What the variables its curves and pitch read stood at when it was last stopped.
    pub stopped_with: String,
}

static LOOP_RESTARTS: parking_lot::Mutex<Vec<LoopRestart>> = parking_lot::Mutex::new(Vec::new());

/// The variables entry `def`'s curves, conditions and pitch read, as they stand now
/// (`engine_n=1118 engine_throttle_injection=0.000`).
fn inputs_now(def: &SoundEntry, var: &dyn Fn(&str) -> Option<f32>) -> String {
    let mut names: Vec<&str> = def.vol_curves.iter().map(|vc| vc.variable.trim()).collect();
    names.extend(def.conditions.iter().map(|c| c.variable.trim()));
    if def.is_loop {
        names.push(def.pitch_variable.trim());
    }
    let mut out: Vec<String> = Vec::new();
    for n in names {
        // (`-1`, `-2`: the entry's clock and its facing, not variables)
        if n.is_empty() || n.starts_with('-') || out.iter().any(|o| o.starts_with(&format!("{n}=")))
        {
            continue;
        }
        match var(n) {
            Some(x) => out.push(format!("{n}={x:.3}")),
            None => out.push(format!("{n}=?")),
        }
    }
    out.join(" ")
}

fn note_loop_restart(s: &RuntimeSound) {
    if !crate::mixer::debug_audio() {
        return;
    }
    let Some((at, with)) = &s.stopped_at else {
        return;
    };
    let gap = at.elapsed().as_secs_f32();
    if gap >= QUICK_RESTART {
        return;
    }
    let file = s.def.file.trim();
    let mut all = LOOP_RESTARTS.lock();
    match all.iter_mut().find(|r| r.file == file) {
        Some(r) => {
            r.count += 1;
            r.shortest = r.shortest.min(gap);
            r.stopped_with.clone_from(with);
        }
        None => all.push(LoopRestart {
            file: file.to_string(),
            count: 1,
            shortest: gap,
            stopped_with: with.clone(),
        }),
    }
}

/// The loops started again soon after being stopped since the last call (see
/// [`note_loop_restart`]), most often first.
pub fn take_loop_restarts() -> Vec<LoopRestart> {
    let mut all = std::mem::take(&mut *LOOP_RESTARTS.lock());
    all.sort_by_key(|r| std::cmp::Reverse(r.count));
    all
}

/// A triggered entry's volume this frame (the entry's volume times its curves): never below
/// what it has been since its trigger fired (`fired`: this frame, which forgets the old peak).
fn peak_hold(peak: &mut f32, vol: f32, fired: bool) -> f32 {
    if fired {
        *peak = 0.0;
    }
    let v = vol.max(*peak);
    *peak = v;
    v
}

pub struct SoundSet {
    sounds: Vec<RuntimeSound>,
    /// The `[sound_ai]`/`[sound_scenery]` share of this set (the settings' sliders).
    pub master: f32,
    /// Folder of the sound config: files of `(T.F.)` triggers resolve against it.
    dir: std::path::PathBuf,
    /// The listener sits in this vehicle's interior (see [`SoundSet::set_inside`]).
    inside: bool,
    /// This set belongs to an AI vehicle or another player's (view 4).
    ai: bool,
    /// The player's own vehicle moves with its listener; its 3D sounds still pan and fade,
    /// but frame timing must not turn their fixed cabin positions into Doppler pitch shifts.
    listener_vehicle: bool,
    /// The camera is in a cab view (driver or passenger: camera modes 0 and 1),
    /// set every frame on every sound set (see [`SoundSet::set_muffled`]).
    muffled: bool,
    /// The sound sets of the coupled parts (with the part's index among the vehicle's
    /// trailers): the rear section of an articulated bus has a `[sound]` of its own - on a
    /// pusher like the MB C2 G that is where the engine is - and plays it on the triggers
    /// and variables of the scripts it shares with the front (see [`SoundSet::update_parts`]).
    pub parts: Vec<(usize, SoundSet)>,
}

/// The file a `(T.F.)` trigger names, as OMSI 2 opens it: the
/// characters a Windows file name cannot have are `%` in it, the folders' `\` and a drive's
/// `:` stay. The stock IBIS announces a stop by its name, and Spandau's "Falkenseer
/// Ch/Stadtrandstr" is `Falkenseer Ch%Stadtrandstr.wav`: read as a folder, it was not found
/// and the stop was not announced (#1096).
fn file_trigger_name(file: &str) -> String {
    file.chars()
        .map(|c| {
            if matches!(c, '/' | '?' | '*' | '"' | '<' | '>' | '|') {
                '%'
            } else {
                c
            }
        })
        .collect()
}

/// Say once per file that a `(T.F.)` sound cannot be found: every AI bus of a type asks for
/// the same missing announcement at every stop.
fn warn_missing_once(trigger: &str, path: &Path) {
    static SEEN: std::sync::Mutex<Option<std::collections::HashSet<std::path::PathBuf>>> =
        std::sync::Mutex::new(None);
    let mut seen = SEEN.lock().unwrap_or_else(|e| e.into_inner());
    if seen
        .get_or_insert_with(Default::default)
        .insert(path.to_path_buf())
    {
        log::warn!(
            "sound {} for (T.F.{trigger}) not found (further requests for it are not logged)",
            path.display()
        );
    }
}

/// A `[volcurve]` at `x`, as OMSI 2 reads it: the first point's
/// value left of it, the last one's from the last point on, and in between the straight
/// line between the last point at or left of `x` and the first one right of it
/// (two points at the same place give their mean).
fn curve(points: &[(f32, f32)], x: f32) -> f32 {
    let Some(&(x0, y0)) = points.first() else {
        return 1.0;
    };
    if x < x0 {
        return y0;
    }
    let last = points[points.len() - 1];
    if x >= last.0 {
        return last.1;
    }
    let i = (1..points.len())
        .find(|&i| x < points[i].0)
        .unwrap_or(points.len() - 1);
    let (xa, ya) = points[i - 1];
    let (xb, yb) = points[i];
    if xb == xa {
        (ya + yb) / 2.0
    } else {
        (x - xa) * ((yb - ya) / (xb - xa)) + ya
    }
}

/// What DirectSound makes of a linear volume (`SetVolume(Round(2000 * log10 v))`, the
/// buffer at `last`): the volume it plays at, and whether it is loud enough to be started
/// (over -100 dB). A request over 0 dB or under -100 dB is refused - the buffer keeps
/// `last`; the MB 412D's `[sound] start2.wav`, which carries a loop sound's lines and reads
/// "44100" as its volume, plays at full volume, not 44 100 times too loud.
fn direct_sound_volume(v: f32, last: f32) -> (f32, bool) {
    if !(v > 1.0e-10) {
        // (-10000 is a valid request: silence)
        return (0.0, false);
    }
    let hundredths = (2000.0 * v.log10()).round();
    if hundredths > 0.0 {
        (last, true)
    } else if hundredths < -10000.0 {
        (last, false)
    } else {
        (10f32.powf(hundredths / 2000.0), hundredths > -10000.0)
    }
}

/// DirectSound's frequency range (`DSBFREQUENCY_MIN`/`MAX`).
const FREQ_MIN: f32 = 100.0;
const FREQ_MAX: f32 = 200_000.0;

/// What a `[volcurve] -1` reads on a triggered entry: OMSI 2 never starts its clock (it
/// is reset only while the conditions of an untriggered entry fail), so it reads
/// `GetTickCount / 1000`, the seconds since Windows started - always past the last point.
const TRIGGERED_ACTIVE: f32 = 1.0e6;

/// One entry's state this frame, as OMSI 2 shows it.
struct Eval {
    /// What the voice is given (the distance attenuation is left to the mixer).
    gain: f32,
    audible: bool,
    pitch: f32,
    /// `[viewpoint]` (or the global stop) silences it: a playing voice is stopped.
    wrong_view: bool,
    /// Cut-off (Hz) of the low-pass the voice is played through, 0 for none: an outside
    /// sound heard in a cab with the `muffle_outside` setting on (see [`MUFFLE_OUTSIDE`]).
    lowpass_hz: f32,
}

impl SoundSet {
    /// The files of a sound config's fixed clips (`dir` as for [`SoundSet::new`]), for
    /// [`AudioEngine::clips_ready`].
    pub fn clip_paths(cfg: &SoundCfg, dir: &Path) -> Vec<std::path::PathBuf> {
        cfg.sounds
            .iter()
            .filter(|def| def.file.trim().parse::<i32>().is_err())
            .map(|def| omsi_cfg::resolve_path(dir, &def.file))
            .collect()
    }

    /// Load the clips of a sound config; `dir` is the directory of the config file.
    pub fn new(engine: &AudioEngine, cfg: &SoundCfg, dir: &Path) -> SoundSet {
        let sounds = cfg
            .sounds
            .iter()
            .map(|def| {
                // `[sound] N`: no fixed file, the script names one with `(T.F.trigger)`
                let dynamic = def.file.trim().parse::<i32>().is_ok();
                let path = omsi_cfg::resolve_path(dir, &def.file);
                let clip = if engine.enabled && !dynamic {
                    engine.load_clip(&path)
                } else {
                    None
                };
                RuntimeSound::new(def.clone(), clip)
            })
            .collect();
        SoundSet {
            sounds,
            master: 1.0,
            dir: dir.to_path_buf(),
            inside: false,
            ai: false,
            listener_vehicle: false,
            muffled: false,
            parts: Vec::new(),
        }
    }

    /// Where the listener is, for the `[viewpoint]` of an entry: `true` while the camera is
    /// one of this vehicle's interior views. The player's bus is told every frame; a scenery
    /// object or another vehicle is always listened to from outside.
    pub fn set_inside(&mut self, inside: bool) {
        self.inside = inside;
        for (_, p) in &mut self.parts {
            p.set_inside(inside);
        }
    }

    /// Track whether the listener travels with the player's vehicle and its coupled parts.
    pub fn set_listener_vehicle(&mut self, follows: bool) {
        self.listener_vehicle = follows;
        for (_, p) in &mut self.parts {
            p.set_listener_vehicle(follows);
        }
    }

    /// The camera is in a cab view (driver or passenger, of any vehicle): called every frame
    /// on every sound set. An AI vehicle is then heard at `0.2 + Snd_OutsideVol` of the
    /// player's bus - no other change, and no filter unless the `muffle_outside` setting is
    /// on ([`MUFFLE_OUTSIDE`]).
    pub fn set_muffled(&mut self, muffled: bool) {
        self.muffled = muffled;
        for (_, p) in &mut self.parts {
            p.set_muffled(muffled);
        }
    }

    /// Attach the sound set of coupled part `index` (built like this one: [`SoundSet::new`]
    /// for the player's bus, [`SoundSet::new_exterior`] for the others).
    pub fn add_part(&mut self, index: usize, mut part: SoundSet) {
        part.inside = self.inside;
        part.muffled = self.muffled;
        part.ai = self.ai;
        part.listener_vehicle = self.listener_vehicle;
        self.parts.push((index, part));
    }

    /// Per-frame update of the coupled parts' sound sets: the same variables and triggers
    /// as the leading vehicle's, each at its part's place (`part_to_world`; a part that is
    /// gone is silenced).
    pub fn update_parts(
        &mut self,
        engine: &AudioEngine,
        var: &dyn Fn(&str) -> Option<f32>,
        part_to_world: &dyn Fn(usize) -> Option<Mat4>,
        triggers: &[String],
    ) {
        for (i, p) in &mut self.parts {
            match part_to_world(*i) {
                Some(xf) => p.update(engine, var, &xf, triggers),
                None => p.stop_all(engine),
            }
        }
    }

    /// The view an entry's `[viewpoint]` is tested against (set per vehicle): 2 for the player's bus while the camera is in a cab view, 1
    /// for it from outside, 4 for every other vehicle - an AI bus never plays its
    /// `[viewpoint] 1` sounds, which are the player's bus heard from the street.
    fn view_mask(&self) -> i32 {
        if self.ai {
            4
        } else if self.inside {
            2
        } else {
            1
        }
    }

    /// A sound set heard from outside: an AI vehicle's or another player's (view 4). Its
    /// sounds are placed exactly as the player's bus's: a `[3d]` one at its position, the
    /// others nowhere in particular.
    pub fn new_exterior(engine: &AudioEngine, cfg: &SoundCfg, dir: &Path) -> SoundSet {
        let mut s = Self::new(engine, cfg, dir);
        s.ai = true;
        s
    }

    /// Play the sound of a `(T.F.trigger)` event: the entry listening to `trigger` loads
    /// `file` (relative to the sound folder) into a new buffer and plays it as if its
    /// trigger had fired.
    pub fn play_file_trigger(
        &mut self,
        engine: &AudioEngine,
        trigger: &str,
        file: &str,
        var: &dyn Fn(&str) -> Option<f32>,
        object_to_world: &Mat4,
    ) {
        if !engine.enabled || file.trim().is_empty() {
            return;
        }
        let path = omsi_cfg::resolve_path(&self.dir, &file_trigger_name(file));
        let Some(clip) = engine.load_clip(&path) else {
            warn_missing_once(trigger, &path);
            return;
        };
        let ctx = self.ctx(engine);
        for s in self.sounds.iter_mut() {
            if !s
                .def
                .triggers
                .iter()
                .any(|d| d.trim().eq_ignore_ascii_case(trigger))
            {
                continue;
            }
            if let Some(id) = s.voice.take() {
                engine.stop(id);
            }
            s.clip = Some(clip.clone());
            s.last_gain = 1.0;
            s.last_pitch = 1.0;
            let e = ctx.eval(s, var, object_to_world, true);
            if e.audible && !e.wrong_view {
                s.voice =
                    Some(engine.play(clip.clone(), ctx.params(s, &e, false, object_to_world)));
            }
        }
    }

    fn ctx(&self, engine: &AudioEngine) -> Ctx {
        Ctx {
            listener: engine.listener_position(),
            ..self.ctx_without_engine()
        }
    }

    /// What a `[volcurve]` reads. The name is looked up in the vehicle's variables and,
    /// when it is none of them, read as a whole number; a negative number then reads one of
    /// the sound's own values: -1 = seconds since the sound became active (its conditions started to hold,
    /// GetTickCount/1000), -2 = how much a `[3d]` sound with a direction faces the listener
    /// (1 for a `[3d]` one without, 0 for one without `[3d]`), anything below is skipped
    /// with "Volume Variable not valid!". The LiAZ 5292's engine loops fade in with
    /// `[volcurve] -1` (0 at 1 s, 1 at 1.3 s).
    fn curve_input(
        vc: &omsi_vehicle::VolCurve,
        var: &dyn Fn(&str) -> Option<f32>,
        active: f32,
        facing: f32,
    ) -> Option<f32> {
        match vc.variable.trim().parse::<i32>() {
            Ok(-1) => Some(active),
            Ok(-2) => Some(facing),
            Ok(n) if n < 0 => None,
            _ => Some(var(&vc.variable).unwrap_or(0.0)),
        }
    }

    fn conditions_hold(def: &SoundEntry, var: &dyn Fn(&str) -> Option<f32>) -> bool {
        def.conditions
            .iter()
            .all(|c| c.holds(var(&c.variable).unwrap_or(0.0)))
    }

    /// A `[loopsound]`'s frequency request in Hz (`rate * |variable| / reference`), `None`
    /// for a `[sound]`, which plays at its file's own rate.
    fn frequency(def: &SoundEntry, var: &dyn Fn(&str) -> Option<f32>) -> Option<f32> {
        def.is_loop.then(|| {
            def.sample_rate.trunc() * var(&def.pitch_variable).unwrap_or(0.0).abs() / def.pitch_ref
        })
    }

    /// The triggers (lower case) whose entries read script variables in a volume curve:
    /// what [`SoundSet::update_fired`] wants the values of at the moment they fire.
    pub fn curve_triggers(&self) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        for s in &self.sounds {
            if !s
                .def
                .vol_curves
                .iter()
                .any(|vc| vc.variable.trim().parse::<i32>().is_err())
            {
                continue;
            }
            for t in &s.def.triggers {
                let t = t.trim().to_ascii_lowercase();
                if !out.contains(&t) {
                    out.push(t);
                }
            }
        }
        out
    }

    /// Per-frame update. `triggers` are the sound triggers fired by the scripts this frame.
    pub fn update(
        &mut self,
        engine: &AudioEngine,
        var: &dyn Fn(&str) -> Option<f32>,
        object_to_world: &Mat4,
        triggers: &[String],
    ) {
        self.update_fired(engine, var, object_to_world, triggers, &|_, _| None);
    }

    /// [`SoundSet::update`] with `at_fire(trigger, variable)`: a variable as it stood when
    /// the trigger fired (None: as it is now). OMSI 2 starts a trigger's sounds in the
    /// middle of the script, so the volume they start with is that of the moment:
    /// the SD200's door hits read `doorSpeed_<n>`, which the door script reverses right
    /// after firing them - read at the frame's end they were silent (#676). From the next
    /// frame on the volume follows the curves as they stand, but never below the loudest
    /// they have made it since the trigger fired (see [`peak_hold`], #131).
    pub fn update_fired(
        &mut self,
        engine: &AudioEngine,
        var: &dyn Fn(&str) -> Option<f32>,
        object_to_world: &Mat4,
        triggers: &[String],
        at_fire: &dyn Fn(&str, &str) -> Option<f32>,
    ) {
        if !engine.enabled {
            return;
        }
        let ctx = self.ctx(engine);
        for s in self.sounds.iter_mut() {
            // How an entry is played:
            // * with a `[trigger]`: once each time the trigger fires, from the start, never
            //   looped (a `[loopsound]` too) and without looking at its conditions;
            // * without one: looped (from a random place in the clip) for as long as its
            //   conditions hold and it can be heard - `[sound]` and `[loopsound]` both loop;
            // * without one but with `[noloop]`: once, when its conditions start to hold.
            let triggered = !s.def.triggers.is_empty();
            let holds = triggered || Self::conditions_hold(&s.def, var);
            let rising = !triggered && holds && !s.held;
            s.held = holds;
            if !triggered {
                if !holds {
                    s.active_since = None;
                } else if s.active_since.is_none() {
                    s.active_since = Some(std::time::Instant::now());
                }
            }
            let Some(clip) = s.clip.clone() else { continue };
            let fired_by = if triggered {
                triggers.iter().find(|t| {
                    s.def
                        .triggers
                        .iter()
                        .any(|d| d.trim().eq_ignore_ascii_case(t))
                })
            } else {
                None
            };
            let fired = fired_by.is_some();
            // An entry that is about to start playing starts at full volume. DirectSound refuses
            // a request over 0 dB and keeps the buffer's last volume, so a `[volume]` over 1
            // that a curve or a condition held at 0 while the entry was silent would otherwise
            // start "audible" at that 0 (#1851, #1418).
            let looping = !triggered && !s.def.no_loop;
            let idle = s.voice.is_none_or(|id| !engine.is_playing(id));
            let before = s.last_gain;
            let reset = fired || rising || (looping && idle);
            if reset {
                s.last_gain = 1.0;
            }
            let eval_now = |ctx: &Ctx, s: &mut RuntimeSound| match fired_by {
                Some(t) => ctx.eval(
                    s,
                    &|n| at_fire(t, n).or_else(|| var(n)),
                    object_to_world,
                    true,
                ),
                None => ctx.eval(s, var, object_to_world, false),
            };
            let mut e = eval_now(&ctx, s);
            // (a trigger that fires while an `[onlyone]` voice still plays, or an entry that is
            // too quiet to start, keeps the buffer's gain: nothing is restarted)
            if reset
                && triggered
                && (!e.audible
                    || s.voice
                        .is_some_and(|id| s.def.only_one && engine.is_playing(id)))
            {
                s.last_gain = before;
                e = eval_now(&ctx, s);
            }
            if e.wrong_view {
                // "Stopped Sound ... globalstop/wrongview"
                if let Some(id) = s.voice.take() {
                    engine.stop(id);
                }
                continue;
            }
            if !triggered && !s.def.no_loop {
                let params = ctx.params(s, &e, true, object_to_world);
                match (s.voice, e.audible) {
                    (Some(id), true) => {
                        if engine.is_playing(id) {
                            engine.set_params(id, params);
                        } else {
                            s.voice = Some(engine.play_from_random_place(clip, params));
                        }
                    }
                    (Some(id), false) => {
                        // "Stopped Sound ... due to volume or pitch"
                        engine.stop(id);
                        s.voice = None;
                        if crate::mixer::debug_audio() {
                            let with = inputs_now(&s.def, var);
                            s.stopped_at = Some((std::time::Instant::now(), with));
                        }
                    }
                    (None, true) => {
                        note_loop_restart(s);
                        s.voice = Some(engine.play_from_random_place(clip, params));
                    }
                    (None, false) => {}
                }
                continue;
            }
            // one-shot: started by its trigger or by its conditions starting to hold; the
            // volume follows the curves while it plays
            let params = ctx.params(s, &e, false, object_to_world);
            if (fired || rising) && e.audible {
                if let Some(id) = s.voice {
                    if s.def.only_one && engine.is_playing(id) {
                        engine.set_params(id, params);
                        continue;
                    }
                    engine.stop(id);
                }
                s.voice = Some(engine.play(clip, params));
            } else if let Some(id) = s.voice {
                if engine.is_playing(id) {
                    engine.set_params(id, params);
                } else {
                    s.voice = None;
                }
            }
        }
    }

    /// How many `[sound]`/`[loopsound]` entries the configuration has.
    pub fn len(&self) -> usize {
        self.sounds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sounds.is_empty()
    }

    /// The sounds playing right now: (file, gain at the listener, pitch) - for the logs.
    pub fn playing(&self, engine: &AudioEngine) -> Vec<(String, f32, f32)> {
        self.sounds
            .iter()
            .filter_map(|s| {
                s.voice
                    .and_then(|id| engine.voice_state(id))
                    .map(|(p, heard)| (s.def.file.clone(), heard, p.pitch))
            })
            .collect()
    }

    /// One line per entry of the configuration, saying whether it is heard and why not
    /// (`OMSI_DEBUG_SOUND`): the only way to see which of a bus's hundred sounds the
    /// rewrite never reaches - a missing clip, a condition on a variable nobody feeds, a
    /// volume curve that stays at zero or a `[viewpoint]` the camera is not in. A heard one
    /// gives its gain at the listener (after the distance) and its playback rate.
    pub fn report(&self, engine: &AudioEngine, var: &dyn Fn(&str) -> Option<f32>) -> Vec<String> {
        let ctx = self.ctx(engine);
        let mut out = Vec::new();
        for s in &self.sounds {
            let file = s.def.file.trim();
            let mut why = String::new();
            if s.clip.is_none() {
                why = if file.parse::<i32>().is_ok() {
                    "waits for a (T.F.) file".into()
                } else {
                    "no clip".into()
                };
            } else if !ctx.view_lets_through(s.def.viewpoint) {
                why = format!(
                    "viewpoint {} (listener {}, Snd_OutsideVol {:.2})",
                    s.def.viewpoint,
                    ctx.view,
                    outside_vol()
                );
            } else if s.def.pos.is_some() && s.def.range <= 0.0 {
                // (`min(range / distance, 1)` is 0 at any distance but 0)
                why = "[3d] range 0: silent anywhere but at its position".into();
            } else if let Some(c) =
                s.def.conditions.iter().find(|c| {
                    s.def.triggers.is_empty() && !c.holds(var(&c.variable).unwrap_or(0.0))
                })
            {
                why = format!("condition {} = {:?}", c.variable, var(&c.variable));
            } else if let Some(vc) = s.def.vol_curves.iter().find(|vc| {
                let active = s.active_since.map_or(0.0, |t| t.elapsed().as_secs_f32());
                Self::curve_input(vc, var, active, 1.0)
                    .is_some_and(|x| curve(&vc.points, x) <= 1.0e-5)
            }) {
                why = format!("volcurve {} = {:?}", vc.variable, var(&vc.variable));
            } else if let Some(f) = Self::frequency(&s.def, var).filter(|f| !(*f >= FREQ_MIN)) {
                why = format!(
                    "frequency {f:.0} Hz ({} = {:?})",
                    s.def.pitch_variable,
                    var(&s.def.pitch_variable)
                );
            }
            let playing = s.voice.and_then(|id| engine.voice_state(id));
            match (playing, why.is_empty()) {
                (Some((p, heard)), _) => out.push(format!(
                    "{file}: {heard:.3} heard, pitch {:.2}, viewpoint {}",
                    p.pitch, s.def.viewpoint
                )),
                (None, true) => out.push(format!("{file}: ready, silent")),
                (None, false) => out.push(format!("{file}: off - {why}")),
            }
        }
        out
    }

    /// Every entry's level for one state, without playing anything (for tools comparing
    /// sound configurations): (file, gain at a listener at `listener` - distance included,
    /// pan left out -, playback rate, whether it would be started). `view_inside`/`cab` as
    /// for [`SoundSet::set_inside`]/[`SoundSet::set_muffled`]; triggered entries as if
    /// their trigger fired now.
    pub fn levels(
        &mut self,
        var: &dyn Fn(&str) -> Option<f32>,
        object_to_world: &Mat4,
        listener: Vec3,
    ) -> Vec<(String, f32, f32, bool)> {
        let ctx = Ctx {
            listener,
            ..self.ctx_without_engine()
        };
        self.sounds
            .iter_mut()
            .map(|s| {
                if !s.def.triggers.is_empty() || Self::conditions_hold(&s.def, var) {
                    s.active_since.get_or_insert_with(|| {
                        std::time::Instant::now() - std::time::Duration::from_secs(5)
                    });
                }
                let e = ctx.eval(s, var, object_to_world, true);
                let dist = s
                    .def
                    .pos
                    .map(|p| {
                        crate::mixer::distance_gain(
                            s.def.range,
                            (object_to_world.transform_point3(Vec3::from_array(p)) - listener)
                                .length(),
                        )
                    })
                    .unwrap_or(1.0);
                let heard = if e.wrong_view || !e.audible {
                    0.0
                } else {
                    e.gain * dist
                };
                (
                    s.def.file.trim().to_string(),
                    heard,
                    e.pitch,
                    e.audible && !e.wrong_view,
                )
            })
            .collect()
    }

    fn ctx_without_engine(&self) -> Ctx {
        Ctx {
            view: self.view_mask(),
            cab: self.muffled,
            master: self.master,
            doppler: !self.listener_vehicle,
            listener: Vec3::ZERO,
        }
    }

    pub fn stop_all(&mut self, engine: &AudioEngine) {
        for s in self.sounds.iter_mut() {
            if let Some(id) = s.voice.take() {
                engine.stop(id);
            }
        }
        for (_, p) in &mut self.parts {
            p.stop_all(engine);
        }
    }
}

/// The per-set inputs of an entry's evaluation.
struct Ctx {
    view: i32,
    cab: bool,
    master: f32,
    doppler: bool,
    listener: Vec3,
}

impl Ctx {
    /// The `[viewpoint]` test: the entry is heard when it names the view, names
    /// none, or is an outside sound of the player's bus heard from its cab while
    /// `Snd_OutsideVol` is over 0.01 (doors or a window open).
    fn view_lets_through(&self, vp: i32) -> bool {
        vp == 0 || vp & self.view != 0 || (vp & 2 == 0 && self.view == 2 && outside_vol() > 0.01)
    }

    /// One entry this frame: its volume and pitch as DirectSound
    /// takes them, and whether it can be started.
    ///
    /// `fired`: the entry's trigger fired this frame. A triggered entry's volume (after its
    /// curves, before distance and the cab's share of outside sound) never falls below its
    /// peak since then ([`peak_hold`]).
    fn eval(
        &self,
        s: &mut RuntimeSound,
        var: &dyn Fn(&str) -> Option<f32>,
        object_to_world: &Mat4,
        fired: bool,
    ) -> Eval {
        let def = &s.def;
        if !self.view_lets_through(def.viewpoint) {
            return Eval {
                gain: 0.0,
                audible: false,
                pitch: s.last_pitch,
                wrong_view: true,
                lowpass_hz: 0.0,
            };
        }
        let triggered = !def.triggers.is_empty();
        let pos = def
            .pos
            .map(|p| object_to_world.transform_point3(Vec3::from_array(p)));
        let active = if triggered {
            TRIGGERED_ACTIVE
        } else {
            s.active_since.map_or(0.0, |t| t.elapsed().as_secs_f32())
        };
        let facing = match (pos, def.dir) {
            (Some(at), Some(d)) => {
                let dir = object_to_world
                    .transform_vector3(Vec3::from_array(d))
                    .normalize_or_zero();
                dir.dot((self.listener - at).normalize_or_zero())
            }
            (Some(_), None) => 1.0,
            (None, _) => 0.0,
        };
        // (an untriggered entry whose conditions fail is at volume 0)
        let mut vol = if triggered || SoundSet::conditions_hold(def, var) {
            def.volume * self.master
        } else {
            0.0
        };
        for vc in &def.vol_curves {
            if let Some(x) = SoundSet::curve_input(vc, var, active, facing) {
                vol *= curve(&vc.points, x);
            }
        }
        if triggered {
            vol = peak_hold(&mut s.peak, vol, fired);
        }
        // `[3d]`: full up to the range, then range / distance
        let dist_gain =
            pos.map(|p| crate::mixer::distance_gain(def.range, (p - self.listener).length()));
        if let Some(g) = dist_gain {
            vol *= g;
        }
        // an AI vehicle (view 4) heard from a cab: through the player's bus's bodywork
        let mut lowpass_hz = 0.0;
        if self.view & 4 != 0 && self.cab {
            vol *= 0.2 + outside_vol();
            lowpass_hz = muffled_cutoff();
        }
        // the player's bus's outside sound heard in its cab: through what is open
        if def.viewpoint & 2 == 0 && def.viewpoint != 0 && self.view == 2 {
            vol *= outside_vol();
            lowpass_hz = muffled_cutoff();
        }
        let (gain, audible) = direct_sound_volume(vol, s.last_gain);
        s.last_gain = gain;
        let mut audible = audible;
        let mut pitch = s.last_pitch;
        if let (Some(f), Some(clip)) = (SoundSet::frequency(def, var), s.clip.as_ref()) {
            // (not started below DirectSound's minimum; a request outside its range is
            // refused and the buffer keeps its rate)
            audible &= f >= FREQ_MIN;
            if (FREQ_MIN..=FREQ_MAX).contains(&f.trunc()) {
                pitch = f.trunc() / clip.sample_rate.max(1) as f32;
                s.last_pitch = pitch;
            }
        }
        // the mixer applies the distance itself, as the listener moves between frames
        let gain = match dist_gain {
            Some(g) if g > 0.0 => gain / g,
            Some(_) => 0.0,
            None => gain,
        };
        Eval {
            gain,
            audible,
            pitch,
            wrong_view: false,
            lowpass_hz,
        }
    }

    fn params(
        &self,
        s: &RuntimeSound,
        e: &Eval,
        looping: bool,
        object_to_world: &Mat4,
    ) -> VoiceParams {
        let position = s
            .def
            .pos
            .map(|p| object_to_world.transform_point3(Vec3::from_array(p)));
        VoiceParams {
            gain: e.gain,
            pitch: e.pitch.max(0.001),
            looping,
            position,
            // (OMSI 2 shifts the frequency of a `[3d]` loop sound only: a `[sound]`
            // keeps its file's rate)
            doppler: self.doppler && s.def.is_loop,
            range: s.def.range,
            lowpass_hz: e.lowpass_hz,
            important: s.def.important,
        }
    }
}

/// `Snd_OutsideVol` of the player's bus (bits of an f32): 1 while the
/// player has no vehicle.
static OUTSIDE_VOL: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0x3f80_0000);

/// Set the player's bus's `Snd_OutsideVol` (0 for a bus whose scripts never write it), or
/// `None` without a bus of one's own.
pub fn set_outside_open(v: Option<f32>) {
    let v = match v {
        Some(x) if x.is_finite() => x,
        Some(_) => 0.0,
        None => 1.0,
    };
    OUTSIDE_VOL.store(v.to_bits(), std::sync::atomic::Ordering::Relaxed);
}

fn outside_vol() -> f32 {
    f32::from_bits(OUTSIDE_VOL.load(std::sync::atomic::Ordering::Relaxed))
}

/// The `muffle_outside` setting: the sounds of outside the bus heard from a cab - other
/// vehicles' and the player's bus's own outside sounds - are played through a low-pass, as
/// the game did before it computed sound as OMSI 2 does (which has no filter, so it is off by
/// default).
pub static MUFFLE_OUTSIDE: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Cut-off (Hz) of that low-pass, 0 with the setting off: 450 Hz with the bus shut, up to 7.2
/// kHz with doors or the driver's window open (`Snd_OutsideVol` 0.5), where the outside comes
/// in unfiltered.
fn muffled_cutoff() -> f32 {
    if MUFFLE_OUTSIDE.load(std::sync::atomic::Ordering::Relaxed) {
        450.0 * (1.0 + 30.0 * outside_vol().clamp(0.0, 0.5))
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use omsi_vehicle::VolCurve;

    fn ctx(view: i32, cab: bool) -> Ctx {
        Ctx {
            view,
            cab,
            master: 1.0,
            doppler: false,
            listener: Vec3::ZERO,
        }
    }

    fn eval(c: &Ctx, def: SoundEntry, var: &dyn Fn(&str) -> Option<f32>) -> Eval {
        let mut s = RuntimeSound::new(def, None);
        c.eval(&mut s, var, &Mat4::IDENTITY, false)
    }

    #[test]
    fn curves_are_linear_and_held_at_the_ends() {
        let p = [(0.0, 0.0), (1.0, 1.0), (1.0, 0.5), (3.0, 0.0)];
        assert_eq!(curve(&p, -1.0), 0.0);
        assert_eq!(curve(&p, 0.5), 0.5);
        // at x = 1 the segment that starts there: (1, 0.5) .. (3, 0)
        assert_eq!(curve(&p, 1.0), 0.5);
        assert_eq!(curve(&p, 2.0), 0.25);
        assert_eq!(curve(&p, 9.0), 0.0);
        assert_eq!(curve(&[], 9.0), 1.0);
    }

    #[test]
    fn direct_sound_refuses_over_0_db() {
        assert_eq!(direct_sound_volume(1.0, 0.3), (1.0, true));
        // 44100 "as a volume": the buffer keeps what it had
        assert_eq!(direct_sound_volume(44100.0, 0.3), (0.3, true));
        assert_eq!(direct_sound_volume(1.3, 1.0), (1.0, true));
        let (g, a) = direct_sound_volume(0.5, 1.0);
        assert!((g - 0.5).abs() < 1e-3 && a);
        assert_eq!(direct_sound_volume(0.0, 0.7), (0.0, false));
        // under -100 dB: refused and not started
        assert_eq!(direct_sound_volume(1.0e-6, 0.7), (0.7, false));
    }

    #[test]
    fn the_cab_hears_outside_through_what_is_open() {
        let none = |_: &str| None;
        // the own bus's outside-only entry (`[viewpoint] 5`) in the cab: at Snd_OutsideVol,
        // not at all with everything shut
        let engine_out = SoundEntry {
            volume: 0.8,
            viewpoint: 5,
            ..Default::default()
        };
        set_outside_open(Some(0.0));
        assert!(eval(&ctx(2, true), engine_out.clone(), &none).wrong_view);
        set_outside_open(Some(0.5));
        let e = eval(&ctx(2, true), engine_out.clone(), &none);
        assert!((e.gain - 0.4).abs() < 1e-3, "{}", e.gain);
        assert!(
            (eval(&ctx(1, false), engine_out.clone(), &none).gain - 0.8).abs() < 1e-3,
            "outside: as it is"
        );
        // an AI bus (view 4) heard from a cab: 0.2 + Snd_OutsideVol, no filter
        let ai = SoundEntry {
            volume: 0.5,
            viewpoint: 4,
            ..Default::default()
        };
        assert!((eval(&ctx(4, true), ai.clone(), &none).gain - 0.35).abs() < 1e-3);
        assert!(
            (eval(&ctx(4, false), ai, &none).gain - 0.5).abs() < 1e-3,
            "from the street: as it is"
        );
        // the player's bus heard from the street on an AI bus: never
        assert!(
            eval(
                &ctx(4, false),
                SoundEntry {
                    volume: 1.0,
                    viewpoint: 1,
                    ..Default::default()
                },
                &none
            )
            .wrong_view
        );
        let cab = SoundEntry {
            volume: 0.8,
            viewpoint: 2,
            ..Default::default()
        };
        assert!(
            eval(&ctx(1, false), cab.clone(), &none).wrong_view,
            "a cab sound stays in"
        );
        // the low-pass of the `muffle_outside` setting: off, nothing is filtered
        let ai = |volume: f32| SoundEntry {
            volume,
            viewpoint: 4,
            ..Default::default()
        };
        assert_eq!(eval(&ctx(4, true), ai(0.5), &none).lowpass_hz, 0.0);
        assert_eq!(
            eval(&ctx(2, true), engine_out.clone(), &none).lowpass_hz,
            0.0
        );
        MUFFLE_OUTSIDE.store(true, std::sync::atomic::Ordering::Relaxed);
        // on: outside sounds heard in the cab - 450 Hz shut, 7.2 kHz with the doors open
        set_outside_open(Some(0.5));
        assert!((eval(&ctx(2, true), engine_out.clone(), &none).lowpass_hz - 7200.0).abs() < 1.0);
        assert!((eval(&ctx(4, true), ai(0.5), &none).lowpass_hz - 7200.0).abs() < 1.0);
        set_outside_open(Some(0.01));
        assert!((eval(&ctx(4, true), ai(0.5), &none).lowpass_hz - 585.0).abs() < 1.0);
        // never what belongs to the cab or what plays everywhere (the indicator relay), nor
        // anything heard from outside
        let everywhere = |viewpoint: i32| SoundEntry {
            volume: 0.8,
            viewpoint,
            ..Default::default()
        };
        assert_eq!(eval(&ctx(2, true), cab, &none).lowpass_hz, 0.0);
        assert_eq!(eval(&ctx(2, true), everywhere(0), &none).lowpass_hz, 0.0);
        assert_eq!(eval(&ctx(2, true), everywhere(3), &none).lowpass_hz, 0.0);
        assert_eq!(
            eval(&ctx(1, false), engine_out.clone(), &none).lowpass_hz,
            0.0
        );
        assert_eq!(eval(&ctx(4, false), ai(0.5), &none).lowpass_hz, 0.0);
        MUFFLE_OUTSIDE.store(false, std::sync::atomic::Ordering::Relaxed);
        viewpoint_matrix(&none);
        set_outside_open(None);
    }

    /// What `[viewpoint]` lets through, for every value of a stock `sound.cfg` (none or 0, 1,
    /// 2, 3, 5) and the 4 the bit model also has, from every view (1 the player's bus seen
    /// from outside, 2 from its cab, 4 another vehicle) and with the bus shut
    /// (`Snd_OutsideVol` 0) or open (0.5): the entry's volume is 1, `None` is silent.
    ///
    /// `2` is the cab alone, `3` the player's two views, `5` the outside and other vehicles,
    /// as the `omsi-sound-cfg` reference has them. Two things go beyond it, both meant:
    /// * an entry without `[viewpoint]` (or `0`, which 46 stock entries say) is heard in
    ///   every view, other vehicles included, not only as a `3`;
    /// * an outside entry (`1`, `5`, and `4`, which no stock vehicle uses) is heard in the
    ///   cab at `Snd_OutsideVol` while the bus is open - the SD200's exterior engine - and a
    ///   twin pair, `2` for the cab and `5` for outside, then plays twice there.
    ///
    /// Called from `the_cab_hears_outside_through_what_is_open`: `Snd_OutsideVol` is global.
    fn viewpoint_matrix(none: &dyn Fn(&str) -> Option<f32>) {
        // (view, listener's camera in a cab, vp, expected factor at Snd_OutsideVol 0 and 0.5)
        type Row = (i32, bool, i32, Option<f32>, Option<f32>);
        let table: [Row; 24] = [
            (1, false, 0, Some(1.0), Some(1.0)),
            (1, false, 1, Some(1.0), Some(1.0)),
            (1, false, 2, None, None),
            (1, false, 3, Some(1.0), Some(1.0)),
            (1, false, 4, None, None),
            (1, false, 5, Some(1.0), Some(1.0)),
            (2, true, 0, Some(1.0), Some(1.0)),
            (2, true, 1, None, Some(0.5)),
            (2, true, 2, Some(1.0), Some(1.0)),
            (2, true, 3, Some(1.0), Some(1.0)),
            (2, true, 4, None, Some(0.5)),
            (2, true, 5, None, Some(0.5)),
            (4, false, 0, Some(1.0), Some(1.0)),
            (4, false, 1, None, None),
            (4, false, 2, None, None),
            (4, false, 3, None, None),
            (4, false, 4, Some(1.0), Some(1.0)),
            (4, false, 5, Some(1.0), Some(1.0)),
            (4, true, 0, Some(0.2), Some(0.7)),
            (4, true, 1, None, None),
            (4, true, 2, None, None),
            (4, true, 3, None, None),
            (4, true, 4, Some(0.2), Some(0.7)),
            (4, true, 5, Some(0.2), Some(0.7)),
        ];
        for (open, pick) in [(0.0, 0usize), (0.5, 1)] {
            set_outside_open(Some(open));
            for &(view, cab, vp, shut, opened) in &table {
                let want = if pick == 0 { shut } else { opened };
                let def = SoundEntry {
                    volume: 1.0,
                    viewpoint: vp,
                    ..Default::default()
                };
                let e = eval(&ctx(view, cab), def, none);
                let at = format!("view {view} cab {cab} viewpoint {vp} at {open}");
                match want {
                    None => assert!(e.wrong_view, "{at}: heard"),
                    Some(f) => {
                        assert!(!e.wrong_view, "{at}: silenced");
                        assert!((e.gain - f).abs() < 1e-3, "{at}: {}", e.gain);
                    }
                }
            }
        }
    }

    /// An engine, its fan and its telemetry read the same variable (`engine_n`): each entry
    /// takes its own volume from it, none gets in another's way.
    #[test]
    fn sounds_that_read_the_same_variable_all_play() {
        let entry = |points: Vec<(f32, f32)>| SoundEntry {
            volume: 1.0,
            vol_curves: vec![VolCurve {
                variable: "engine_n".into(),
                points,
            }],
            ..Default::default()
        };
        let engine_n = |n: &str| (n == "engine_n").then_some(1000.0);
        let c = ctx(1, false);
        let engine = eval(&c, entry(vec![(0.0, 0.0), (1000.0, 1.0)]), &engine_n);
        let fan = eval(&c, entry(vec![(0.0, 0.0), (2000.0, 1.0)]), &engine_n);
        let telemetry = eval(&c, entry(vec![(0.0, 1.0), (3000.0, 1.0)]), &engine_n);
        assert!(engine.audible && fan.audible && telemetry.audible);
        assert!((engine.gain - 1.0).abs() < 1e-3);
        assert!((fan.gain - 0.5).abs() < 1e-3);
        assert!((telemetry.gain - 1.0).abs() < 1e-3);
    }

    /// A `[3d]` sound is full up to its range and falls as range / distance beyond it: with
    /// a range of 0 nothing is left at any distance.
    #[test]
    fn a_3d_sound_with_no_range_is_silent_away_from_its_position() {
        let at = |range: f32| SoundEntry {
            volume: 1.0,
            pos: Some([10.0, 0.0, 0.0]),
            range,
            ..Default::default()
        };
        let none = |_: &str| None;
        assert!(!eval(&ctx(1, false), at(0.0), &none).audible);
        assert!(eval(&ctx(1, false), at(5.0), &none).audible);
        let flat = SoundEntry {
            volume: 1.0,
            ..Default::default()
        };
        assert!(
            eval(&ctx(1, false), flat, &none).audible,
            "without [3d]: everywhere"
        );
    }

    #[test]
    fn volume_curves_multiply_and_a_3d_sound_fades_with_distance() {
        let def = SoundEntry {
            volume: 1.0,
            pos: Some([0.0, 8.0, 0.0]),
            range: 2.0,
            vol_curves: vec![
                VolCurve {
                    variable: "a".into(),
                    points: vec![(0.0, 0.0), (1.0, 1.0)],
                },
                VolCurve {
                    variable: "b".into(),
                    points: vec![(0.0, 1.0), (1.0, 0.0)],
                },
            ],
            ..Default::default()
        };
        let var = |n: &str| Some(if n == "a" { 0.5 } else { 0.5 });
        let e = eval(&ctx(1, false), def, &var);
        // 0.5 * 0.5 * (2 / 8), the distance left to the mixer: 0.25 given to the voice
        assert!((e.gain - 0.25).abs() < 1e-3, "{}", e.gain);
        assert!(e.audible);
    }

    #[test]
    fn a_loop_sound_is_not_started_below_100_hz_and_keeps_its_rate_past_200_khz() {
        let def = SoundEntry {
            volume: 1.0,
            is_loop: true,
            sample_rate: 44100.0,
            pitch_variable: "n".into(),
            pitch_ref: 1000.0,
            ..Default::default()
        };
        let mut s = RuntimeSound::new(
            def,
            Some(Arc::new(Clip {
                sample_rate: 44100,
                channels: 1,
                samples: vec![0; 4],
            })),
        );
        let c = ctx(1, false);
        let e = c.eval(&mut s, &|_| Some(1.0), &Mat4::IDENTITY, false);
        assert!(!e.audible, "44 Hz");
        let e = c.eval(&mut s, &|_| Some(2000.0), &Mat4::IDENTITY, false);
        assert!(e.audible && (e.pitch - 2.0).abs() < 1e-4);
        // 6 x 44100 = 264 600 Hz: refused, the rate stays at 2x
        let e = c.eval(&mut s, &|_| Some(6000.0), &Mat4::IDENTITY, false);
        assert!(e.audible && (e.pitch - 2.0).abs() < 1e-4, "{}", e.pitch);
    }

    /// The SD200's door hit: `[volcurve] doorSpeed_0` from 1 at -1 down to 0 at -0.5,
    /// fired while the door still closes at -1 m/s; by the frame's end the script has turned
    /// the speed round. The volume is the one of the moment it fired (#676).
    #[test]
    fn a_triggered_sound_reads_its_curve_when_it_fires() {
        let hit = SoundEntry {
            volume: 1.0,
            triggers: vec!["ev_doorhitclose_0".into()],
            vol_curves: vec![VolCurve {
                variable: "doorSpeed_0".into(),
                points: vec![(-1.0, 1.0), (-0.5, 0.0)],
            }],
            ..Default::default()
        };
        let now = |n: &str| (n == "doorSpeed_0").then_some(0.8);
        let at_fire =
            |t: &str, n: &str| (t == "ev_doorhitclose_0" && n == "doorSpeed_0").then_some(-1.0);
        assert_eq!(
            eval(&ctx(2, true), hit.clone(), &now).gain,
            0.0,
            "read at the frame's end: silent"
        );
        let fired = |n: &str| at_fire("ev_doorhitclose_0", n).or_else(|| now(n));
        assert_eq!(eval(&ctx(2, true), hit.clone(), &fired).gain, 1.0);
        let set = SoundSet {
            sounds: vec![RuntimeSound::new(hit, None)],
            master: 1.0,
            dir: Default::default(),
            inside: true,
            ai: false,
            listener_vehicle: true,
            muffled: false,
            parts: Vec::new(),
        };
        assert_eq!(set.curve_triggers(), vec!["ev_doorhitclose_0".to_string()]);
    }

    /// A door sound whose curve follows the door: fired with the door at 0.7, the door closed
    /// the next frame - OMSI 2 holds the peak (0x7507bc), so the sound keeps its tail instead
    /// of falling silent the frame after it starts (#131).
    #[test]
    fn a_triggered_sound_keeps_its_peak_while_it_plays() {
        let door = |at: f32| move |n: &str| (n == "door").then_some(at);
        let entry = |triggers: Vec<String>| SoundEntry {
            volume: 1.0,
            triggers,
            vol_curves: vec![VolCurve {
                variable: "door".into(),
                points: vec![(0.0, 0.0), (1.0, 1.0)],
            }],
            ..Default::default()
        };
        let id = Mat4::IDENTITY;
        let c = ctx(2, true);
        let mut s = RuntimeSound::new(entry(vec!["ev_door".into()]), None);
        let e = c.eval(&mut s, &door(0.7), &id, true);
        assert!((e.gain - 0.7).abs() < 1e-3, "{}", e.gain);
        // the door stops: the curve reads 0, the sound does not get quieter
        let e = c.eval(&mut s, &door(0.0), &id, false);
        assert!((e.gain - 0.7).abs() < 1e-3 && e.audible, "{}", e.gain);
        // a louder moment raises the peak
        let e = c.eval(&mut s, &door(1.0), &id, false);
        assert!((e.gain - 1.0).abs() < 1e-3, "{}", e.gain);
        // fired again quieter: the old peak is forgotten
        let e = c.eval(&mut s, &door(0.3), &id, true);
        assert!((e.gain - 0.3).abs() < 1e-3, "{}", e.gain);
        // heard from the wrong view it stays silent and keeps no peak
        let mut cab = RuntimeSound::new(
            SoundEntry {
                viewpoint: 2,
                ..entry(vec!["ev_door".into()])
            },
            None,
        );
        assert!(
            ctx(1, false)
                .eval(&mut cab, &door(1.0), &id, true)
                .wrong_view
        );
        assert_eq!(cab.peak, 0.0);
        // an entry without a trigger follows its curve, up and down (the engine's loops)
        let mut lp = RuntimeSound::new(entry(Vec::new()), None);
        assert!((c.eval(&mut lp, &door(0.7), &id, false).gain - 0.7).abs() < 1e-3);
        assert_eq!(c.eval(&mut lp, &door(0.0), &id, false).gain, 0.0);
    }

    /// The same through a whole sound set: a triggered door sound started at the curve's
    /// peak keeps playing at it on the frames after, when the script has moved the
    /// variable back to where the curve reads 0 (#131).
    #[test]
    fn a_triggered_voice_is_not_turned_down_by_its_curve_after_it_starts() {
        let engine = AudioEngine::silent();
        let mut set = set_of(SoundEntry {
            volume: 1.0,
            triggers: vec!["ev_door".into()],
            vol_curves: vec![VolCurve {
                variable: "v".into(),
                points: vec![(0.0, 0.0), (1.0, 1.0)],
            }],
            ..Default::default()
        });
        step(&mut set, &engine, 1.0, &["ev_door".to_string()]);
        assert_eq!(voice_gain(&set, &engine), Some(1.0));
        // (the running voice is updated, not restarted: the remembered gain is what it is given)
        step(&mut set, &engine, 0.0, &[]);
        assert!(set.sounds[0].voice.is_some_and(|id| engine.is_playing(id)));
        assert_eq!(set.sounds[0].last_gain, 1.0);
    }

    /// The stock IBIS announces Spandau's "Falkenseer Ch/Stadtrandstr" with the file
    /// `Falkenseer Ch%Stadtrandstr.wav`, as OMSI 2's file triggers write `/` (#1096).
    #[test]
    fn a_file_trigger_writes_what_a_file_name_cannot_have_as_percent() {
        let file = r"..\..\Announcements\Spandau\Falkenseer Ch/Stadtrandstr.wav";
        assert_eq!(
            file_trigger_name(file),
            r"..\..\Announcements\Spandau\Falkenseer Ch%Stadtrandstr.wav"
        );
        assert_eq!(
            file_trigger_name(r"C:\a?b*c\d<e>f|g.wav"),
            r"C:\a%b%c\d%e%f%g.wav"
        );
        // found where the announcement lies
        let root = std::env::temp_dir().join(format!("depot-file-trigger-{}", std::process::id()));
        let sound = root.join("Vehicles").join("MAN_NL_NG").join("Sound");
        let spandau = root.join("Vehicles").join("Announcements").join("Spandau");
        std::fs::create_dir_all(&sound).unwrap();
        std::fs::create_dir_all(&spandau).unwrap();
        std::fs::write(spandau.join("Falkenseer Ch%Stadtrandstr.wav"), b"RIFF").unwrap();
        let path = omsi_cfg::resolve_path(&sound, &file_trigger_name(file));
        assert!(path.is_file(), "{}", path.display());
        // (the name as the script wrote it: a folder "Falkenseer Ch" that is not there)
        assert!(!omsi_cfg::resolve_path(&sound, file).is_file());
        let _ = std::fs::remove_dir_all(&root);
    }

    fn tone() -> std::sync::Arc<crate::mixer::Clip> {
        std::sync::Arc::new(crate::mixer::Clip {
            sample_rate: 8000,
            channels: 1,
            samples: vec![1000; 8000],
        })
    }

    /// The gain a running entry's voice has been given.
    fn voice_gain(set: &SoundSet, engine: &AudioEngine) -> Option<f32> {
        set.sounds[0]
            .voice
            .and_then(|id| engine.voice_state(id))
            .map(|(p, _)| p.gain)
    }

    fn set_of(def: SoundEntry) -> SoundSet {
        SoundSet {
            sounds: vec![RuntimeSound::new(def, Some(tone()))],
            master: 1.0,
            dir: Default::default(),
            inside: true,
            ai: false,
            listener_vehicle: true,
            muffled: false,
            parts: Vec::new(),
        }
    }

    fn step(set: &mut SoundSet, engine: &AudioEngine, v: f32, fired: &[String]) {
        let var = move |n: &str| (n == "v").then_some(v);
        set.update_fired(engine, &var, &Mat4::IDENTITY, fired, &|_, _| None);
    }

    /// A `[loopsound]` with `[volume] 3` whose curve holds it at 0 starts, when the curve
    /// lets go, at full gain - not at the 0 it was left at while silent (#1851, #1418).
    #[test]
    fn a_loop_over_full_volume_starts_at_full_gain_after_silence() {
        let engine = AudioEngine::silent();
        let mut set = set_of(SoundEntry {
            volume: 3.0,
            vol_curves: vec![VolCurve {
                variable: "v".into(),
                points: vec![(0.0, 0.0), (1.0, 1.0)],
            }],
            ..Default::default()
        });
        for _ in 0..3 {
            step(&mut set, &engine, 0.0, &[]);
        }
        assert!(
            set.sounds[0].voice.is_none(),
            "silent while the curve is at 0"
        );
        step(&mut set, &engine, 1.0, &[]);
        assert_eq!(voice_gain(&set, &engine), Some(1.0));
    }

    /// A triggered one-shot with `[volume] 2` and a curve that was low at rest plays at full
    /// gain when its trigger fires.
    #[test]
    fn a_triggered_one_shot_with_volume_over_one_starts_at_full_gain() {
        let engine = AudioEngine::silent();
        let mut set = set_of(SoundEntry {
            volume: 2.0,
            triggers: vec!["ev_brake".into()],
            vol_curves: vec![VolCurve {
                variable: "v".into(),
                points: vec![(0.0, 0.0), (1.0, 1.0)],
            }],
            ..Default::default()
        });
        // (a silent frame leaves the remembered gain at 0)
        set.sounds[0].last_gain = 0.0;
        step(&mut set, &engine, 1.0, &["ev_brake".to_string()]);
        assert_eq!(voice_gain(&set, &engine), Some(1.0));
    }

    /// An entry that keeps playing keeps following its curve: the remembered gain is only
    /// reset when a voice is about to start.
    #[test]
    fn an_entry_already_playing_keeps_following_its_curve() {
        let engine = AudioEngine::silent();
        let mut set = set_of(SoundEntry {
            volume: 1.0,
            vol_curves: vec![VolCurve {
                variable: "v".into(),
                points: vec![(0.0, 0.0), (1.0, 1.0)],
            }],
            ..Default::default()
        });
        step(&mut set, &engine, 1.0, &[]);
        assert_eq!(voice_gain(&set, &engine), Some(1.0));
        step(&mut set, &engine, 0.5, &[]);
        // (the running voice is updated, not restarted: the remembered gain follows the curve)
        let g = set.sounds[0].last_gain;
        assert!((g - 0.5).abs() < 1.0e-3, "{g}");
    }

    /// A trigger that fires while an `[onlyone]` voice keeps playing does not restart it, so
    /// the gain the buffer has is not put back to full.
    #[test]
    fn a_retrigger_of_a_playing_only_one_voice_keeps_its_gain() {
        let engine = AudioEngine::silent();
        let mut set = set_of(SoundEntry {
            volume: 2.0,
            only_one: true,
            triggers: vec!["ev".into()],
            vol_curves: vec![VolCurve {
                variable: "v".into(),
                points: vec![(0.0, 0.0), (1.0, 1.0)],
            }],
            ..Default::default()
        });
        step(&mut set, &engine, 0.4, &["ev".to_string()]);
        assert!(
            (set.sounds[0].last_gain - 0.8).abs() < 1.0e-3,
            "{}",
            set.sounds[0].last_gain
        );
        // volume 2 now: over 0 dB, refused - the buffer keeps the 0.8 it plays at
        step(&mut set, &engine, 1.0, &["ev".to_string()]);
        assert!(
            (set.sounds[0].last_gain - 0.8).abs() < 1.0e-3,
            "{}",
            set.sounds[0].last_gain
        );
    }

    /// `OMSI_DEBUG_AUDIO` names what stopped a loop: the variables its curves and pitch
    /// read, each once, the entry's clock (`-1`) left out.
    #[test]
    fn a_stopped_loop_is_logged_with_what_its_curves_read() {
        let curve = |v: &str| VolCurve {
            variable: v.into(),
            points: vec![(0.0, 0.0), (1.0, 1.0)],
        };
        let def = SoundEntry {
            is_loop: true,
            pitch_variable: "engine_n".into(),
            vol_curves: vec![
                curve("engine_throttle_injection"),
                curve("engine_n"),
                curve("-1"),
            ],
            ..Default::default()
        };
        let var = |n: &str| match n {
            "engine_n" => Some(1118.0),
            "engine_throttle_injection" => Some(0.0),
            _ => None,
        };
        assert_eq!(
            inputs_now(&def, &var),
            "engine_throttle_injection=0.000 engine_n=1118.000"
        );
    }
}
