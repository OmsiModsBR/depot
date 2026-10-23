//! The pose of one person: what drives it ([`PoseInput`]), the state that carries over from
//! frame to frame ([`Pose`]) and the bone transforms it gives ([`Posed`]).

use glam::{Affine3A, DVec2, DVec3, Vec2, Vec3};
use std::f32::consts::TAU;

use crate::people::rig::SLOTS;

mod advance;
mod bones;
mod gait;
mod math;

pub use gait::run_factor;

/// What a human is doing; drives the pose.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Activity {
    Stand,
    Walk,
    Sit,
    /// Standing at the cash desk with the right hand held out.
    Pay,
}

/// What the animation needs to know about a person this frame. Points and directions are
/// in the person's model frame (x right, y forward, z up, origin at the feet) unless they
/// say otherwise.
#[derive(Clone, Copy)]
pub struct PoseInput<'a> {
    pub activity: Activity,
    /// Where the person stands and which way they face, in the frame of the floor they are
    /// on (the world, or a bus): position of the feet, heading in degrees (OMSI's, 0 = +y,
    /// clockwise).
    pub origin: DVec3,
    pub heading: f64,
    /// Which floor frame `origin` is in (0 the ground, else a bus); a change keeps the feet
    /// where they are and re-expresses them in the new frame.
    pub frame: u64,
    /// Ground velocity in that frame (m/s).
    pub velocity: DVec2,
    /// While sitting: the seat point (`[passpos]`, the hip) in the model frame.
    pub seat: Option<Vec3>,
    /// Something to look at.
    pub look: Option<Vec3>,
    /// Where the right hand reaches (the cash desk).
    pub reach: Option<Vec3>,
    /// Where both hands hold on (left, right): the driver's hands on the steering wheel.
    /// Followed as given every frame, no easing (the caller moves them).
    pub grips: Option<[Vec3; 2]>,
    /// How each gripping hand lies (left, right): the direction from the wrist to the
    /// knuckles and the direction the palm faces, model frame. Without it the hand goes on
    /// straight from the forearm.
    pub grip_frames: Option<[(Vec3, Vec3); 2]>,
    /// Extra forward lean towards the grips (degrees): a driver whose wheel is far off leans
    /// to it from the seat instead of leaving it.
    pub grip_lean: f32,
    /// Hold on to a handrail (0 … 1).
    pub hold: f32,
    /// Acceleration of the floor under the person (a moving bus), model frame, m/s².
    pub sway: Vec2,
    /// Floor height at a point of the floor frame, when the floor is not flat.
    pub floor: Option<&'a dyn Fn(DVec2) -> Option<f64>>,
}

impl Default for PoseInput<'_> {
    fn default() -> Self {
        PoseInput {
            activity: Activity::Stand,
            origin: DVec3::ZERO,
            heading: 0.0,
            frame: 0,
            velocity: DVec2::ZERO,
            seat: None,
            look: None,
            reach: None,
            grips: None,
            grip_frames: None,
            grip_lean: 0.0,
            hold: 0.0,
            sway: Vec2::ZERO,
            floor: None,
        }
    }
}

/// One foot: planted on the floor (frame coordinates of the floor point under the ankle
/// and the foot's heading), or in the air on its way to the next place.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Foot {
    pub(crate) planted: bool,
    /// Floor point under the ankle, and heading (degrees), in the floor frame.
    pub(crate) pos: DVec3,
    pub(crate) yaw: f64,
    /// Swing: where it left (ankle position and pitch at lift-off) and where it goes.
    from_ankle: DVec3,
    from_yaw: f64,
    from_pitch: f32,
    from_floor: f64,
    to: DVec3,
    to_yaw: f64,
    /// Floor height sampled at `to` (and where that was).
    to_floor: f64,
    to_sampled: DVec2,
    /// The landing height the swing aims at: follows `to_floor` quickly but never in a
    /// jump (the target crossing the edge of a step changed it by 30-40 cm mid-swing and
    /// the foot snapped up with it), and how long a landing has waited for it.
    land_z: f64,
    land_wait: f32,
    /// Swing progress 0 … 1 and length (s) of a step not driven by the gait.
    t: f32,
    /// A walking step: the gait's swing progress when the foot left (the swing's own
    /// progress runs from there to the landing).
    t0: f32,
    dur: f32,
    /// This step belongs to the walk (heel strike and push-off) rather than a shuffle.
    walk: bool,
    lift: f32,
}

impl Foot {
    fn new() -> Foot {
        Foot {
            planted: true,
            pos: DVec3::ZERO,
            yaw: 0.0,
            from_ankle: DVec3::ZERO,
            t0: 0.0,
            from_yaw: 0.0,
            from_pitch: 0.0,
            from_floor: 0.0,
            to: DVec3::ZERO,
            to_yaw: 0.0,
            to_floor: 0.0,
            to_sampled: DVec2::splat(f64::MAX),
            land_z: 0.0,
            land_wait: 0.0,
            t: 0.0,
            dur: 0.4,
            walk: false,
            lift: 0.05,
        }
    }
}

/// A person's animation state.
#[derive(Debug, Clone)]
pub struct Pose {
    rng: u32,
    init: bool,
    frame: u64,
    origin: DVec3,
    heading: f64,
    /// Gait cycle (0 … 1, the left heel strikes at 0) and whether the gait drives the feet.
    phase: f32,
    gait: bool,
    /// Walking weight for the upper body (0 … 1), smoothed ground speed (m/s), forward
    /// acceleration (m/s²) and turning rate (deg/s).
    walk: f32,
    speed: f32,
    accel: f32,
    turn: f32,
    vel: DVec2,
    pub(crate) feet: [Foot; 2],
    /// Standing offsets of the feet (a little different each time somebody settles).
    fidget: [Vec2; 2],
    fidget_t: f32,
    /// 0 standing … 1 seated, and the seat.
    sit: f32,
    seat: Vec3,
    reach: f32,
    reach_at: Vec3,
    /// Both hands on a steering wheel: how far (0 … 1) and where.
    grip: f32,
    grip_at: [Vec3; 2],
    /// How the gripping hands lie (see `PoseInput::grip_frames`).
    grip_frame: Option<[(Vec3, Vec3); 2]>,
    /// Extra lean towards the wheel (degrees, eased).
    grip_extra: f32,
    hold: f32,
    /// Head direction (degrees: right, up) and where a glance goes and for how long.
    pub(crate) head: Vec2,
    glance: Vec2,
    glance_t: f32,
    /// Weight shift: where the pelvis leans (m) and until when.
    shift: f32,
    shift_to: f32,
    shift_t: f32,
    breath: f32,
    /// Balance against the floor's acceleration: lean (m/s² equivalent) and its rate.
    lean: Vec2,
    lean_v: Vec2,
    /// Pelvis height offset kept from the last frame (rises are smoothed).
    pub(crate) drop: f32,
    /// Floor under the body relative to the origin (smoothed), from the planted feet.
    body_floor: f32,
    /// Per-person style: arm hang, head tilt, stance width.
    style: [f32; 4],
    /// Steps taken to catch up with a foot left behind, and frames a foot had to be pulled
    /// in (for the tests).
    pub(crate) catch_ups: u32,
    pub(crate) shuffles: u32,
    /// A foot was put down in the last [`Pose::advance`] - one footstep sound.
    landed: bool,
}

/// Bone transforms and the joint positions they put the limbs at (model frame).
#[derive(Debug, Clone)]
pub struct Posed {
    pub bones: [Affine3A; SLOTS],
    pub hip: [Vec3; 2],
    pub knee: [Vec3; 2],
    pub ankle: [Vec3; 2],
    pub elbow: [Vec3; 2],
    pub wrist: [Vec3; 2],
    /// Lowest point of each sole (heel, ball or toe).
    pub sole: [f32; 2],
    /// Knee flexion and ankle flexion (degrees, toes up positive) per side.
    pub knee_flex: [f32; 2],
    pub ankle_flex: [f32; 2],
    /// How far each ankle stayed from where it should be (unreachable targets).
    pub leg_miss: [f32; 2],
    pub neck: Vec3,
    /// All transforms are finite (else `bones` is the rest pose and the caller should keep
    /// the last good mesh).
    pub ok: bool,
}

impl Pose {
    pub fn new(seed: u32) -> Pose {
        let mut p = Pose {
            rng: seed.wrapping_mul(0x9E37_79B9) | 1,
            init: false,
            frame: 0,
            origin: DVec3::ZERO,
            heading: 0.0,
            phase: 0.0,
            gait: false,
            walk: 0.0,
            speed: 0.0,
            accel: 0.0,
            turn: 0.0,
            vel: DVec2::ZERO,
            feet: [Foot::new(), Foot::new()],
            fidget: [Vec2::ZERO; 2],
            fidget_t: 0.0,
            sit: 0.0,
            seat: Vec3::ZERO,
            reach: 0.0,
            reach_at: Vec3::new(0.3, 0.5, 1.1),
            grip: 0.0,
            grip_at: [Vec3::new(-0.2, 0.4, 1.0), Vec3::new(0.2, 0.4, 1.0)],
            grip_frame: None,
            grip_extra: 0.0,
            hold: 0.0,
            head: Vec2::ZERO,
            glance: Vec2::ZERO,
            glance_t: 0.0,
            shift: 0.0,
            shift_to: 0.0,
            shift_t: 0.0,
            breath: 0.0,
            lean: Vec2::ZERO,
            lean_v: Vec2::ZERO,
            drop: 0.0,
            body_floor: 0.0,
            style: [0.0; 4],
            catch_ups: 0,
            shuffles: 0,
            landed: false,
        };
        for k in 0..4 {
            p.style[k] = p.rand() * 2.0 - 1.0;
        }
        p.breath = p.rand() * TAU;
        p.glance_t = p.rand() * 3.0;
        p.shift_t = p.rand() * 5.0;
        p.fidget_t = 8.0 + p.rand() * 10.0;
        p
    }

    fn rand(&mut self) -> f32 {
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x >> 8) as f32 / (1u32 << 24) as f32
    }

    /// 0 standing … 1 seated.
    pub fn sit_amount(&self) -> f32 {
        self.sit
    }

    /// Sitting down or getting up is under way: the person should not walk off yet.
    pub fn settling(&self) -> bool {
        self.sit > 0.02 && self.sit < 0.98
    }

    /// The state in a line, for `OMSI_DEBUG_POSE`.
    pub fn describe(&self) -> String {
        let foot = |f: &Foot| {
            if f.planted {
                format!("down at ({:.2}, {:.2}, {:.2})", f.pos.x, f.pos.y, f.pos.z)
            } else {
                format!(
                    "in the air {:.0}%{}",
                    f.t * 100.0,
                    if f.walk { " (walk)" } else { "" }
                )
            }
        };
        format!(
            "origin ({:.2}, {:.2}, {:.2}) heading {:.0} turning {:.0}/s frame {} speed {:.2} gait {} walk {:.2} phase {:.2} sit {:.2} reach {:.2} hold {:.2} head ({:.0}, {:.0}) floor {:.2} drop {:.3} catch-ups {} shuffles {}; left {}; right {}",
            self.origin.x, self.origin.y, self.origin.z, self.heading, self.turn, self.frame, self.speed, self.gait, self.walk, self.phase, self.sit, self.reach, self.hold, self.head.x, self.head.y, self.body_floor, self.drop, self.catch_ups, self.shuffles, foot(&self.feet[0]), foot(&self.feet[1])
        )
    }

    /// Whether foot `side` (0 left, 1 right) is on the floor.
    pub fn planted(&self, side: usize) -> bool {
        self.feet[side.min(1)].planted
    }

    /// A foot was put down during the last [`Pose::advance`]: one footfall, for the step
    /// sound. (The gait plants a foot only while walking, so standing still is silent.)
    pub fn landed(&self) -> bool {
        self.landed
    }

    /// Nothing quick is going on (standing or sitting still: breathing, shifting, glancing),
    /// so a lower update rate will not show.
    pub fn calm(&self) -> bool {
        !self.gait
            && self.walk < 0.05
            && self.feet.iter().all(|f| f.planted)
            && (self.sit == 0.0 || self.sit == 1.0)
            && (self.reach == 0.0 || self.reach == 1.0)
            && (self.hold == 0.0 || self.hold == 1.0)
    }

    /// Quick steps taken so far to catch up with a foot left behind.
    pub fn catch_ups(&self) -> u32 {
        self.catch_ups
    }
}
