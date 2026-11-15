//! Where a person's own centre of mass lies, standing and seated ([`BodyCom`]): what the people
//! aboard a vehicle weigh on it is centred there, not at the point of the cabin they stand
//! or sit on.
//!
//! The body is split into segments, each with its share of the mass and the point along it
//! where its own centre lies, from the standard anthropometric tables (D. A. Winter,
//! *Biomechanics and Motor Control of Human Movement*, table 4.1). The segments are laid on
//! the joints of the type's [`Rig`] in two still postures.

use super::Rig;
use glam::Vec3;

/// A segment's share of the body's mass and how far along it, from its upper joint, its own
/// centre lies.
struct Segment {
    share: f32,
    along: f32,
}

const HEAD: Segment = Segment {
    share: 0.081,
    along: 0.5,
};
const TRUNK: Segment = Segment {
    share: 0.497,
    along: 0.5,
};
const UPPER_ARM: Segment = Segment {
    share: 0.028,
    along: 0.436,
};
const FOREARM: Segment = Segment {
    share: 0.016,
    along: 0.430,
};
const HAND: Segment = Segment {
    share: 0.006,
    along: 0.506,
};
const THIGH: Segment = Segment {
    share: 0.100,
    along: 0.433,
};
const SHANK: Segment = Segment {
    share: 0.0465,
    along: 0.433,
};
const FOOT: Segment = Segment {
    share: 0.0145,
    along: 0.5,
};

/// A hand's length from the wrist to the finger tips, for a body 1.75 m tall (m).
const HAND_LENGTH: f32 = 0.19;

/// A person's centre of mass in their own frame (x right, y forward - the way they face -, z
/// up; m), from the point the cabin keeps for them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BodyCom {
    /// Standing upright, arms down: from the floor point the person stands on (the model's
    /// origin).
    pub standing: Vec3,
    /// Seated upright, thighs level ahead, shins down, hands on the lap: from the point
    /// `[seatheight]` under the seat's `[passpos]` (where the seated body is put).
    pub seated: Vec3,
}

impl BodyCom {
    /// The two postures laid on `rig`; `seat_height` is the type's `[seatheight]`. A rig
    /// measured from broken `[links]` gives the centre of the body as broken as it is drawn.
    pub fn of(rig: &Rig, seat_height: f32) -> BodyCom {
        BodyCom {
            standing: standing(rig),
            seated: seated(rig, seat_height),
        }
    }

    /// The centre of mass of a person kept at `place` (a vehicle's frame), facing
    /// `heading` (degrees, OMSI's: 0 = +y, clockwise), seated or not.
    pub fn at(&self, place: Vec3, heading: f64, seated: bool) -> Vec3 {
        let own = if seated { self.seated } else { self.standing };
        let (sin, cos) = heading.to_radians().sin_cos();
        let (right, forward) = (
            Vec3::new(cos as f32, -sin as f32, 0.0),
            Vec3::new(sin as f32, cos as f32, 0.0),
        );
        place + right * own.x + forward * own.y + Vec3::Z * own.z
    }
}

/// The mass-weighted mean of the segments' centres.
fn mean(parts: &[(&Segment, Vec3)]) -> Vec3 {
    let (sum, mass) = parts.iter().fold((Vec3::ZERO, 0.0), |(s, m), (seg, at)| {
        (s + *at * seg.share, m + seg.share)
    });
    sum / mass
}

/// A point `along` of the way from `a` to `b`.
fn on(a: Vec3, b: Vec3, along: f32) -> Vec3 {
    a + (b - a) * along
}

/// The head's centre: over the head pivot, halfway between the neck and the top of the head.
fn head(rig: &Rig) -> Vec3 {
    Vec3::new(
        rig.head_pivot.x,
        rig.head_pivot.y,
        rig.neck.z + (rig.head_top - rig.neck.z) * HEAD.along,
    )
}

/// The trunk's centre: halfway from the middle of the hips to the middle of the shoulders.
fn trunk(rig: &Rig) -> Vec3 {
    let shoulders = (rig.shoulder[0] + rig.shoulder[1]) / 2.0;
    on(rig.pelvis, shoulders, TRUNK.along)
}

/// A foot's centre: halfway from the heel to the toe tip, half the ankle's height up.
fn foot(rig: &Rig, ankle: Vec3) -> Vec3 {
    ankle + Vec3::new(0.0, (rig.heel + rig.toe) / 2.0, -rig.ankle_h / 2.0)
}

fn standing(rig: &Rig) -> Vec3 {
    let mut parts = vec![(&HEAD, head(rig)), (&TRUNK, trunk(rig))];
    let hand = HAND_LENGTH * rig.scale;
    for side in 0..2 {
        let (hip, knee, ankle) = (rig.hip[side], rig.knee[side], rig.ankle[side]);
        let shoulder = rig.shoulder[side];
        // the arm hanging straight down from the shoulder
        let elbow = shoulder - Vec3::Z * rig.upper_arm;
        let wrist = elbow - Vec3::Z * rig.forearm;
        parts.extend([
            (&THIGH, on(hip, knee, THIGH.along)),
            (&SHANK, on(knee, ankle, SHANK.along)),
            (&FOOT, foot(rig, ankle)),
            (&UPPER_ARM, on(shoulder, elbow, UPPER_ARM.along)),
            (&FOREARM, on(elbow, wrist, FOREARM.along)),
            (&HAND, wrist - Vec3::Z * hand * HAND.along),
        ]);
    }
    // from the model's origin, which is put on the floor where the person stands
    mean(&parts)
}

fn seated(rig: &Rig, seat_height: f32) -> Vec3 {
    // the upper body as it stands, moved so that the middle of the hips is the origin
    let up = |p: Vec3| p - rig.pelvis;
    let mut parts = vec![(&HEAD, up(head(rig))), (&TRUNK, up(trunk(rig)))];
    let hand = HAND_LENGTH * rig.scale;
    for side in 0..2 {
        let hip = up(rig.hip[side]);
        // thigh level ahead, shin straight down, the foot flat ahead of the ankle
        let knee = hip + Vec3::Y * rig.thigh;
        let ankle = knee - Vec3::Z * rig.shin;
        // upper arm down, forearm level ahead, the hand on the thigh
        let shoulder = up(rig.shoulder[side]);
        let elbow = shoulder - Vec3::Z * rig.upper_arm;
        let wrist = elbow + Vec3::Y * rig.forearm;
        parts.extend([
            (&THIGH, on(hip, knee, THIGH.along)),
            (&SHANK, on(knee, ankle, SHANK.along)),
            (&FOOT, foot(rig, ankle)),
            (&UPPER_ARM, on(shoulder, elbow, UPPER_ARM.along)),
            (&FOREARM, on(elbow, wrist, FOREARM.along)),
            (&HAND, wrist + Vec3::Y * hand * HAND.along),
        ]);
    }
    // the hips sit `seat_lift` over the seat, which lies `[seatheight]` over the point
    // the seated body is put at
    mean(&parts) + Vec3::Z * (seat_height + rig.seat_lift)
}
