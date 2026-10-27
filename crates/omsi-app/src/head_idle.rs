//! The part of the driver's head that moves when nothing else does.
//!
//! A person standing still does not hold their head still: it breathes up and down, shifts
//! its weight slowly from one side to the other and leans a little, and it wanders about the
//! middle while doing it. That is what this is - the sway of a head that has nothing to react
//! to, so the view is never perfectly frozen while the bus stands at a stop and the player
//! touches nothing.
//!
//! Each of those movements has its own slow rate, and none of the rates is a multiple of
//! another, so the pattern never lands on itself and the head does not look like a metronome.
//! The sideways sway and the lean have to be periodic: a movement that only drifts one way and
//! stops reads as nothing at all, however far it goes - the eye gives up on a slow uniform
//! slide (the same reason a bus climbing onto a bridge looks level). The long drift is still
//! there underneath each of them, in a smaller share, so that nothing repeats exactly.
//!
//! The vertical movement is the largest - and the one that is really noticed, because it is
//! the breath. The fore and aft movement is the smallest: from the driver's seat it runs along
//! the line of sight, so moving by it changes almost nothing on the screen; it is here for the
//! near field and for the mirrors, which the eye is asked for.
//!
//! It is *added* to what the bus's own motion does (OMSI's head movement, `Player::move_head`):
//! where that is thrown by accelerations and is therefore nothing at a standstill, this goes on
//! of its own. Two settings ask for it - how much of it there is (`Settings::head_idle`: zero is
//! exactly nothing, and that is the default) and how fast it moves (`Settings::head_idle_pace`:
//! one is the periods written here). A step with the first at zero is nothing at all and puts
//! the head back to the middle, so no other code has to ask whether the effect is on.
//!
//! While the player is reaching for a switch the sway waits where it is (`Hold` below): a view
//! that goes on sliding under the pointer is a view that misses what it was reaching for.
use glam::Vec3;

/// How far the head wanders at full strength: metres in the bus's frame (across, along, up).
pub const SIDE_M: f32 = 0.010;
pub const FORE_M: f32 = 0.003;
pub const UP_M: f32 = 0.014;

/// The turn it adds to the view at full strength, in degrees.
pub const YAW_DEG: f32 = 0.30;
pub const PITCH_DEG: f32 = 0.18;
pub const ROLL_DEG: f32 = 0.30;

/// The periods of the movements, in seconds: a breath (about fifteen a minute), the weight
/// going from one foot to the other, the body's lean, and the long wander of a standing body.
const BREATH_S: f32 = 4.1;
const WEIGHT_S: f32 = 6.4;
const LEAN_S: f32 = 5.7;
const WANDER_S: f32 = 13.1;

/// How much faster or slower than the periods above the pace setting may ask for (1 = as they
/// stand here).
const MIN_PACE: f32 = 0.5;
const MAX_PACE: f32 = 2.0;

/// Where the head is and how it is turned on its own at one instant.
///
/// `offset` is in the bus's frame, so it is turned with the bus like the head that OMSI's own
/// head movement moves; the angles are what the view is turned by, in the same degrees and the
/// same sense as a head tracker's pose (a positive yaw turns left, as `finish` in `app_events`
/// adds it to the camera's yaw).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct HeadIdle {
    pub offset: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    /// Seconds of sway so far: it is what makes the movement smooth and a picture at a given
    /// time the same whatever the frame rate was.
    time: f32,
}

impl HeadIdle {
    /// Move the head on by `dt` seconds with `strength` of the sway (0 = off, 1 = the whole of
    /// it) at `pace` times its designed speed (1 = the periods above; the setting's range is
    /// `MIN_PACE` to `MAX_PACE`).
    ///
    /// The pace is the time it is moved on by, so every movement - the breath, the weight going
    /// from one foot to the other, the lean and the long wander - grows slower or faster
    /// together, and asking for a different one mid sway does not make the head jump.
    ///
    /// A step longer than a tenth of a second is held back (a load hitch must not throw the
    /// head), the pace or no pace, and the whole movement is a function of the elapsed time
    /// alone, so it does not depend on how often it is asked.
    pub fn step(&mut self, dt: f32, strength: f32, pace: f32) {
        let strength = strength.clamp(0.0, 1.0);
        if strength <= 0.0 {
            *self = Self::default();
            return;
        }
        self.time += (dt * pace.clamp(MIN_PACE, MAX_PACE)).clamp(0.0, 0.1);
        let t = self.time;
        self.offset = Vec3::new(
            // the weight from one foot to the other: the sideways sway, slow enough not to be
            // a step, and with a drift under it so that it never comes back to the same place
            SIDE_M * strength * (wave(t, WEIGHT_S, 0.35) * 0.75 + wander(t, WANDER_S, 12) * 0.25),
            // fore and aft: the smallest of the three (see the note at the top of the file)
            FORE_M * strength * wander(t, WANDER_S, 13),
            // the breath, the one movement that is really periodic, and the largest
            UP_M * strength * (wave(t, BREATH_S, 0.0) * 0.6 + wander(t, BREATH_S, 14) * 0.4),
        );
        self.yaw = YAW_DEG * strength * wander(t, WANDER_S, 15);
        self.pitch = PITCH_DEG * strength * wander(t, WEIGHT_S, 16);
        // the lean: a horizon only just tilting, at its own rate
        self.roll =
            ROLL_DEG * strength * (wave(t, LEAN_S, 0.6) * 0.7 + wander(t, WANDER_S, 17) * 0.3);
    }

    /// The head is where it began: nothing is added to the view at all.
    pub fn is_still(&self) -> bool {
        *self == Self::default()
    }
}

/// How much of the sway a bus moving at `kmh` keeps: all of it standing, none from 5 km/h,
/// eased in between so that it neither stops with a jerk as the bus pulls away nor jumps
/// back when it stops (#1325: it swayed on the road too).
pub fn standstill(kmh: f32) -> f32 {
    let x = ((kmh.abs() - 0.5) / 4.5).clamp(0.0, 1.0);
    1.0 - x * x * (3.0 - 2.0 * x)
}

/// The sway held back while the player is reaching for something.
///
/// OMSI's switches are small and the cursor has to find them: while a control is under it the
/// sway waits where it is - the camera then stands as it does without this effect at all - and
/// it goes on once the hand has left. It is only ever the sway of a head at rest that waits:
/// the bus's own head movement is not held back by it.
#[derive(Clone, Copy, Debug, Default)]
pub struct Hold {
    /// Seconds of it left.
    left: f32,
}

impl Hold {
    /// Tell it whether a control is under the cursor (`reaching`) and move it on by `dt`
    /// seconds; the answer is whether the sway is to wait where it is.
    pub fn step(&mut self, dt: f32, reaching: bool) -> bool {
        self.left = if reaching {
            HOLD_S
        } else {
            (self.left - dt.max(0.0)).max(0.0)
        };
        self.left > 0.0
    }
}

/// How long the sway waits where it is after the cursor leaves a control again: long enough
/// that the click itself, and the moment after it, are steady, and that the edge of a control's
/// box cannot flicker the sway in and out.
pub const HOLD_S: f32 = 0.6;

/// A slow, even wave in -1..1 that depends on the time alone. `phase` is a share of the
/// period, so that several waves do not all start from the same point.
fn wave(t: f32, period: f32, phase: f32) -> f32 {
    (std::f32::consts::TAU * (t / period + phase)).sin()
}

/// A smooth wander in -1..1 that depends on the time alone: one value every `period` seconds,
/// drawn from the cell's number and the seed, the two sides joined with a cubic so there are no
/// corners and no jitter. No state is kept, so asking for an earlier moment gives it back.
fn wander(t: f32, period: f32, seed: u32) -> f32 {
    let k = (t / period).floor();
    let f = t / period - k;
    let a = cell(k as i32, seed);
    let b = cell(k as i32 + 1, seed);
    let s = f * f * (3.0 - 2.0 * f);
    (a + (b - a) * s) * 2.0 - 1.0
}

/// A stable value in 0..1 for one cell of a wander: the same cell always gives the same value,
/// on every machine and in every run.
fn cell(index: i32, seed: u32) -> f32 {
    let mut h = (index as u32).wrapping_mul(0x9E37_79B9) ^ seed.wrapping_mul(0x85EB_CA6B);
    h ^= h >> 15;
    h = h.wrapping_mul(0x2545_F491);
    h ^= h >> 13;
    (h >> 8) as f32 / 16_777_216.0
}
