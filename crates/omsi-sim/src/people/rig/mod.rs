//! The skeleton of a human: the engine's bone ids and slots, the `[links]` joints, the
//! mesh influences and the rig measured from the joints and the mesh.

use glam::{Vec2, Vec3};
use omsi_content::Human;
use omsi_geometry::MeshData;

mod load;

pub use load::HumanType;

/// Engine bone ids of `[setbone]`.
pub const BONE_OS_L: i32 = -2;

pub const BONE_OS_R: i32 = -3;

pub const BONE_US_L: i32 = -4;

pub const BONE_US_R: i32 = -5;

pub const BONE_OA_L: i32 = -6;

pub const BONE_OA_R: i32 = -7;

pub const BONE_UA_L: i32 = -8;

pub const BONE_UA_R: i32 = -9;

pub const BONE_HIP: i32 = -10;

pub const BONE_MAIN: i32 = -11;

pub const BONE_HEAD: i32 = -12;

pub const BONE_HAND_L: i32 = -13;

pub const BONE_HAND_R: i32 = -14;

/// Bone transform slots: the thirteen engine bones (id -2 … -14 → slot 0 … 12), the two
/// feet split off the shins and the toes split off the feet.
pub const SLOTS: usize = 17;

pub(crate) const THIGH: [usize; 2] = [0, 1];

pub(crate) const SHIN: [usize; 2] = [2, 3];

pub(crate) const UPPER: [usize; 2] = [4, 5];

pub(crate) const FORE: [usize; 2] = [6, 7];

pub(crate) const HIP: usize = 8;

pub(crate) const MAIN: usize = 9;

pub(crate) const HEAD: usize = 10;

pub(crate) const HAND: [usize; 2] = [11, 12];

pub(crate) const FOOT: [usize; 2] = [13, 14];

pub(crate) const TOE: [usize; 2] = [15, 16];

/// Index 0 is the left side, 1 the right; this is the sign of x on that side.
pub(crate) const SIDE: [f32; 2] = [-1.0, 1.0];

/// The transform slot of an engine bone id.
pub fn slot_of(id: i32) -> Option<usize> {
    (BONE_HAND_R..=BONE_OS_L)
        .contains(&id)
        .then(|| (-id - 2) as usize)
}

pub(crate) fn bone_id_by_name(name: &str) -> Option<i32> {
    match name.trim().to_ascii_lowercase().replace('_', "").as_str() {
        "osl" => Some(BONE_OS_L),
        "osr" => Some(BONE_OS_R),
        "usl" => Some(BONE_US_L),
        "usr" => Some(BONE_US_R),
        "oal" => Some(BONE_OA_L),
        "oar" => Some(BONE_OA_R),
        "ual" => Some(BONE_UA_L),
        "uar" => Some(BONE_UA_R),
        "hip" => Some(BONE_HIP),
        "main" => Some(BONE_MAIN),
        "head" => Some(BONE_HEAD),
        "handl" => Some(BONE_HAND_L),
        "handr" => Some(BONE_HAND_R),
        _ => None,
    }
}

/// Joint positions in the model frame (x right, y forward, z up) of the right side.
#[derive(Debug, Clone, Copy)]
pub struct Joints {
    pub hip: Vec3,
    pub knee: Vec3,
    pub waist: Vec3,
    pub shoulder: Vec3,
    pub elbow: Vec3,
    pub neck: Vec3,
    pub hand: Vec3,
    pub finger: Vec3,
}

impl Joints {
    pub fn from_links(l: &[f32]) -> Joints {
        let g = |i: usize| l.get(i).copied().filter(|v| v.is_finite()).unwrap_or(0.0);
        Joints {
            hip: Vec3::new(g(0), g(1), g(2)),
            knee: Vec3::new(g(3), g(4), g(5)),
            waist: Vec3::new(0.0, g(6), g(7)),
            shoulder: Vec3::new(g(8), g(9), g(10)),
            elbow: Vec3::new(g(11), g(12), g(13)),
            neck: Vec3::new(0.0, g(14), g(15)),
            hand: Vec3::new(g(16), g(17), g(18)),
            finger: Vec3::new(g(19), g(20), g(21)),
        }
    }
}

/// One vertex's bone influences: up to four (slot, weight) pairs, normalised.
#[derive(Debug, Clone, Copy, Default)]
pub struct Influence {
    pub n: u8,
    pub slot: [u8; 4],
    pub weight: [f32; 4],
}

/// A skinned mesh of a human with its bone → engine id table.
pub struct HumanMesh {
    pub data: MeshData,
    pub materials: Vec<omsi_o3d::Material>,
    /// Per bone of the file: engine id and (vertex, weight) list, as stored.
    pub bones: Vec<(i32, Vec<(u32, f32)>)>,
    /// Per vertex: the influences the skinning uses. The files list a vertex once per
    /// face corner (up to 24 times, always with the same weight), so the weights are taken
    /// once per bone and normalised; vertices near the sole also follow the foot.
    pub skin: Vec<Influence>,
    /// Per material: the model's `[matl_alpha]` for it (0 opaque, 1 alpha test, 2 blend) -
    /// the hair of the stock women and of man02 is an alpha-tested texture.
    pub alpha: Vec<i32>,
}

/// The skeleton of one human type, measured from its `[links]` and its mesh. Index 0 of the
/// pairs is the left side, 1 the right.
#[derive(Debug, Clone)]
pub struct Rig {
    pub hip: [Vec3; 2],
    pub knee: [Vec3; 2],
    pub ankle: [Vec3; 2],
    pub shoulder: [Vec3; 2],
    pub elbow: [Vec3; 2],
    pub wrist: [Vec3; 2],
    pub waist: Vec3,
    pub neck: Vec3,
    /// What the head turns about: at the neck's height, under the middle of the head. The
    /// `[links]` neck point lies at the back of the neck (13 cm behind the middle of the
    /// head of aXYZ man02), and the head turned about it swung off the collar - a broken
    /// neck whenever the passenger looked to the side.
    pub head_pivot: Vec3,
    /// Between the hip joints.
    pub pelvis: Vec3,
    pub thigh: f32,
    pub shin: f32,
    pub upper_arm: f32,
    pub forearm: f32,
    /// Lowest point of the soles.
    pub sole: f32,
    /// Ankle joint above the sole.
    pub ankle_h: f32,
    /// Heel, ball and toe tip along the foot from the ankle (m, heel negative), and the
    /// height of the toe joint above the sole.
    pub heel: f32,
    pub ball: f32,
    pub toe: f32,
    pub ball_h: f32,
    pub head_top: f32,
    /// Hip joint above the seat surface when sitting (`[seatheight]` names the rest).
    pub seat_lift: f32,
    /// Size relative to a 1.75 m adult.
    pub scale: f32,
    /// From `[walk_param]`: the step length at full stride (m) - half the file's first
    /// line, the stride ({schrittweite}, 1.4 by default) -, the arm swing and the hip sway.
    pub walk_step: f32,
    pub arm_swing: f32,
    pub hip_sway: f32,
}

impl Rig {
    pub(crate) fn measure(def: &Human, j: &Joints, meshes: &[HumanMesh]) -> Rig {
        let scale = if def.height > 0.5 {
            (def.height / 1.75).clamp(0.6, 1.3)
        } else {
            (j.neck.z / 1.55).clamp(0.6, 1.3)
        };
        let mirror = |v: Vec3| Vec3::new(-v.x, v.y, v.z);
        // the right shin's vertices (the foot is part of it)
        let mut shin: Vec<Vec3> = Vec::new();
        let mut top = f32::MIN;
        let (mut head_sum, mut head_n) = (Vec3::ZERO, 0u32);
        for m in meshes {
            for p in &m.data.positions {
                top = top.max(p.z);
            }
            for (i, inf) in m.skin.iter().enumerate() {
                if (0..inf.n as usize).any(|k| inf.slot[k] as usize == HEAD && inf.weight[k] > 0.5)
                {
                    head_sum += m.data.positions[i];
                    head_n += 1;
                }
            }
            for (i, inf) in m.skin.iter().enumerate() {
                if (0..inf.n as usize)
                    .any(|k| inf.slot[k] as usize == SHIN[1] && inf.weight[k] > 0.5)
                {
                    shin.push(m.data.positions[i]);
                }
            }
        }
        let knee = j.knee;
        let sole = shin.iter().map(|p| p.z).fold(f32::MAX, f32::min);
        let sole = if sole.is_finite() && sole < knee.z - 0.1 {
            sole
        } else {
            0.0
        };
        let ankle_h = (0.075 * scale).clamp(0.05, 0.1);
        let foot: Vec<&Vec3> = shin.iter().filter(|p| p.z < sole + 0.035 * scale).collect();
        let (mut heel_y, mut toe_y) = (knee.y - 0.07 * scale, knee.y + 0.19 * scale);
        if foot.len() >= 4 {
            let lo = foot.iter().map(|p| p.y).fold(f32::MAX, f32::min);
            let hi = foot.iter().map(|p| p.y).fold(f32::MIN, f32::max);
            if hi - lo > 0.12 * scale && hi - lo < 0.45 {
                heel_y = lo;
                toe_y = hi;
            }
        }
        // the ankle: the middle of the leg just above the foot
        let ring: Vec<&Vec3> = shin
            .iter()
            .filter(|p| (p.z - (sole + ankle_h + 0.02)).abs() < 0.025)
            .collect();
        let (mut ax, mut ay) = (knee.x, heel_y + 0.28 * (toe_y - heel_y));
        if ring.len() >= 4 {
            let n = ring.len() as f32;
            ax = ring.iter().map(|p| p.x).sum::<f32>() / n;
            ay = ring.iter().map(|p| p.y).sum::<f32>() / n;
        }
        // (a foot shorter than 13 cm - a child, a small model - has the ankle as far back as
        // it goes: the bounds crossed and the game stopped on the clamp, #138)
        let ay = ay.clamp(heel_y + 0.03, (toe_y - 0.1).max(heel_y + 0.03));
        let ax = if (ax - knee.x).abs() < 0.1 {
            ax
        } else {
            knee.x
        };
        let ankle = Vec3::new(ax, ay, sole + ankle_h);
        let foot_len = toe_y - heel_y;
        let hip = j.hip;
        let shoulder = j.shoulder;
        let elbow = if (j.elbow - j.shoulder).length() > 0.1 {
            j.elbow
        } else {
            j.shoulder + Vec3::new(0.26 * scale, 0.0, 0.0)
        };
        let wrist = if (j.hand - elbow).length() > 0.1 {
            j.hand
        } else {
            elbow + (elbow - shoulder).normalize() * 0.25 * scale
        };
        let head_top = if top.is_finite() && top > j.neck.z {
            top
        } else {
            j.neck.z + 0.2 * scale
        };
        let seat_lift = if def.seat_height > 0.2 {
            (hip.z - def.seat_height).clamp(0.05, 0.2)
        } else {
            0.1 * scale
        };
        let wp = def.walk_param;
        Rig {
            hip: [mirror(hip), hip],
            knee: [mirror(knee), knee],
            ankle: [mirror(ankle), ankle],
            shoulder: [mirror(shoulder), shoulder],
            elbow: [mirror(elbow), elbow],
            wrist: [mirror(wrist), wrist],
            waist: j.waist,
            neck: j.neck,
            head_pivot: if head_n > 20 {
                // the middle of the neck itself at the linked height: the vertices of the
                // mesh there (the neck's cross-section, under the head, not the collar)
                let c = head_sum / head_n as f32;
                let (mut sum, mut n) = (Vec3::ZERO, 0u32);
                for m in meshes {
                    for p in &m.data.positions {
                        if (p.z - j.neck.z).abs() < 0.03
                            && Vec2::new(p.x - c.x, p.y - c.y).length() < 0.09
                        {
                            sum += *p;
                            n += 1;
                        }
                    }
                }
                let at = if n >= 8 {
                    sum / n as f32
                } else {
                    Vec3::new(c.x, j.neck.y + (c.y - j.neck.y) * 0.75, j.neck.z)
                };
                // (never further than 12 cm from the linked point)
                Vec3::new(
                    j.neck.x,
                    j.neck.y + (at.y - j.neck.y).clamp(-0.12, 0.12),
                    j.neck.z,
                )
            } else {
                j.neck
            },
            pelvis: Vec3::new(0.0, hip.y, hip.z),
            thigh: (knee - hip).length().max(0.2),
            shin: (ankle - knee).length().max(0.2),
            upper_arm: (elbow - shoulder).length(),
            forearm: (wrist - elbow).length(),
            sole,
            ankle_h,
            heel: heel_y - ay,
            ball: heel_y + 0.72 * foot_len - ay,
            toe: toe_y - ay,
            ball_h: (0.022 * scale).clamp(0.015, 0.03),
            head_top,
            seat_lift,
            scale,
            // (line 1 is the stride: OMSI's walk phase runs 2.0 per
            // two strides and sets a foot down at 0.2, 0.7, 1.2 and 1.7, one step per half a
            // stride; line 2, {upper_arm_beta}, is an angle of the arm)
            walk_step: 0.5 * if wp[0] > 0.3 { wp[0] } else { 1.4 },
            arm_swing: if wp[2] > 0.0 {
                wp[2].clamp(0.2, 1.6)
            } else {
                1.0
            },
            hip_sway: if wp[3] > 0.0 {
                wp[3].clamp(0.4, 2.2)
            } else {
                1.0
            },
        }
    }

    /// The toe joint of a foot in the rest pose.
    pub(crate) fn ball_joint(&self, side: usize) -> Vec3 {
        self.ankle[side] + Vec3::new(0.0, self.ball, self.ball_h - self.ankle_h)
    }

    /// Hip joint height above the sole with the legs straight.
    pub fn leg(&self) -> f32 {
        self.hip[1].z - self.sole
    }

    /// Where a standing foot rests (the floor point under its ankle), in the model frame.
    pub(crate) fn rest_foot(&self, side: usize) -> Vec3 {
        Vec3::new(self.ankle[side].x, self.ankle[side].y, self.sole)
    }

    /// How far in front of a seat's `[passpos]` (the hip) a person stands before sitting
    /// down, which is also where the feet stay while seated.
    pub fn seat_front(&self) -> f32 {
        (self.thigh * 0.85).clamp(0.25, 0.4)
    }

    /// Steps per second at `speed`, as OMSI times the walk: the stride is
    /// the full one from 1.2 m/s on and shortens with the speed below that, so the steps
    /// keep one pace when walking slowly; never with a step longer than the legs allow.
    pub fn cadence(&self, speed: f32) -> f32 {
        let v = speed.max(0.05);
        let f = v / (self.walk_step * (v / 1.2).min(1.0));
        f.max(v / (0.8 * self.leg())).min(3.4)
    }
}
