//! Bodies coupled by joints: the sections of an articulated bus and a lorry's trailer, each
//! a [`RigidBody`] of its own `.bus` file on the same wheel physics, as Omsi.exe runs them.
//! A joint holds the two bodies together at the ball - and, at a pusher's joint, at the hinge
//! ahead of it the joint pitches about -, keeps an articulated bus's sections from rolling
//! against each other (`[coupling_front_character]` type other than 0), and stops at the
//! file's angles: alpha about the vertical axis, beta about the transverse one.
//!
//! The joint is solved with the tyres' hold across, as one more set of constraints on the
//! velocities each body would have at the end of the substep: whatever acts on one section -
//! its mass, its brakes, its drive, its tyres - reaches the other through the joint, with
//! the joint's load on the axles, the lever of its pull and the inertia of the part behind.
//! A body alone is a train of one, solved exactly as before.

use super::{body_damping, GroundProbe, Impact, Lateral, RigidBody, Substep, BUMP};
use glam::{DVec3, Mat3, Quat, Vec3};
use omsi_vehicle::Vehicle;
use std::borrow::Cow;

/// How much of a joint's error a substep takes away (Baumgarte), and the fastest it does so
/// (m/s, rad/s): a part put down a little off its joint is drawn to it, not thrown.
const BAUMGARTE: f32 = 0.2;
const MAX_BIAS: f32 = 1.0;
/// Passes over the constraints of a substep: the tyres' alone take 8 (Omsi.exe's hold across
/// converges within them); with joints the sections share their tyres through the joint, which
/// takes a few more.
const PASSES: usize = 8;
const JOINT_PASSES: usize = 16;
/// The joint's limits are looked at from this far (rad) inside them.
const LIMIT_WINDOW: f32 = 0.3;

/// What a joint lets turn besides the vertical and the transverse axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JointKind {
    /// A lorry's hitch (`[coupling_front_character]` type 0): free about every axis.
    Ball,
    /// An articulated bus's turntable (any other type): the sections do not roll against
    /// each other.
    Articulation,
}

/// The joint between body `lead` and body `follow` of a train.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Joint {
    pub lead: usize,
    pub follow: usize,
    /// The ball: the lead's `[coupling_back]` and the follower's `[coupling_front]`, each in its
    /// own model frame (`vehicle::coupling_points`).
    pub anchor_lead: Vec3,
    pub anchor_follow: Vec3,
    /// How far ahead of the ball, on the lead, a pusher's joint pitches (its
    /// `[coupling_pitch_offset]`; 0: a puller's, pitching at the ball). The ball then rides a
    /// link that turns with the lead and pitches with the follower (`TrailerPart::pitch_hinge`).
    pub pitch_offset: f32,
    /// `[coupling_front_character]`: the most the joint turns either way about the vertical
    /// axis, and its range about the transverse one (rad).
    pub alpha_max: f32,
    pub beta_min: f32,
    pub beta_max: f32,
    pub kind: JointKind,
}

/// What a body of a train is given for a step: its brake force per wheel (N), the steering
/// (−1..1) for the one that is steered - the others turn as their joint bends them -, and
/// whether the air acts on it (on the leading body alone: the sections behind are in its lee).
#[derive(Debug, Clone, Copy)]
pub struct BodyInputs<'a> {
    pub brake: &'a [f32],
    pub steer: Option<f32>,
    pub air: bool,
}

impl Joint {
    /// The joint of `follow` behind `lead`, `back` and `front` where they meet (see
    /// [`Joint::anchor_lead`]). Without a `[coupling_front_character]`: 55° either way, 15°
    /// up and down, an articulated bus's turntable.
    pub fn new(lead: usize, follow: usize, follow_def: &Vehicle, back: Vec3, front: Vec3) -> Joint {
        let [alpha, beta_min, beta_max, kind] = follow_def
            .coupling_front_character
            .filter(|c| c.iter().all(|x| x.is_finite()) && c[0] > 0.0 && c[1] < c[2])
            .unwrap_or([55.0, -15.0, 15.0, 1.0]);
        Joint {
            lead,
            follow,
            anchor_lead: back,
            anchor_follow: front,
            pitch_offset: follow_def.pitch_offset(),
            alpha_max: alpha.to_radians(),
            beta_min: beta_min.to_radians(),
            beta_max: beta_max.to_radians(),
            kind: if kind == 0.0 { JointKind::Ball } else { JointKind::Articulation },
        }
    }

    /// Where the joint pitches, in the lead's model frame (the ball for a puller).
    pub fn hinge(&self) -> Vec3 {
        let ahead = if self.anchor_lead.y < 0.0 { 1.0 } else { -1.0 };
        self.anchor_lead + Vec3::new(0.0, ahead * self.pitch_offset, 0.0)
    }

    /// Alpha - the lead's heading less the follower's - and beta - the lead's pitch less the
    /// follower's - as the bodies stand (rad), as `articulation_<n>_alpha` and `_beta` read.
    pub fn angles(&self, bodies: &[RigidBody]) -> (f32, f32) {
        let (hl, pl, _) = bodies[self.lead].heading_pitch_bank();
        let (hf, pf, _) = bodies[self.follow].heading_pitch_bank();
        let alpha = ((hl - hf + 540.0).rem_euclid(360.0) - 180.0) as f32;
        (alpha.to_radians(), (pl - pf).to_radians())
    }

    /// How far apart (m) the two bodies hold the ball.
    pub fn gap(&self, bodies: &[RigidBody]) -> f32 {
        self.ball(&bodies[self.lead], &bodies[self.follow]).2.length()
    }

    /// The pitch axis (world): the lead's transverse axis at a pusher's hinge, the follower's
    /// at a puller's ball; and the yaw axis, the other body's vertical.
    fn axes(&self, lead: &RigidBody, follow: &RigidBody) -> (Vec3, Vec3) {
        if self.pitch_offset > 0.0 {
            (lead.orientation.mul_vec3(Vec3::X), follow.orientation.mul_vec3(Vec3::Z))
        } else {
            (follow.orientation.mul_vec3(Vec3::X), lead.orientation.mul_vec3(Vec3::Z))
        }
    }

    /// The hinge (world), the link from it to the ball as the lead holds it (world; zero at a
    /// puller's ball) and how far the follower's ball stands off that.
    fn ball(&self, lead: &RigidBody, follow: &RigidBody) -> (Vec3, Vec3, Vec3) {
        let at = |b: &RigidBody, p: Vec3| b.position + b.orientation.mul_vec3(p - b.cog).as_dvec3();
        let hinge = at(lead, self.hinge());
        let ball_f = at(follow, self.anchor_follow);
        let link = if self.pitch_offset > 0.0 {
            let a = lead.orientation.mul_vec3(Vec3::X);
            let to = (ball_f - hinge).as_vec3();
            let back = to - a * to.dot(a);
            let back = if back.length() > 1e-3 { back.normalize() } else { -lead.orientation.mul_vec3(Vec3::Y) };
            back * self.pitch_offset
        } else {
            Vec3::ZERO
        };
        let r_hinge = (hinge - lead.position).as_vec3();
        (r_hinge, link, ((hinge - ball_f).as_vec3() + link))
    }

    /// The curvature (1/m, positive to the right) the joint, bent by `alpha`, makes the
    /// follower turn on about its `[rot_pnt_long]` line: sin alpha over the distance from the
    /// ball to that line. An axle steered towards that line (the AG300's rear one, #322)
    /// follows it.
    fn kappa(&self, bodies: &[RigidBody]) -> f32 {
        let (alpha, _) = self.angles(bodies);
        let f = &bodies[self.follow];
        alpha.sin() / (self.anchor_follow.y - f.rot_pnt_long).abs().max(0.5)
    }
}

/// One velocity constraint between the lead (`a`) and the follower (`b`): world directions
/// for their linear and angular velocities.
#[derive(Debug, Clone, Copy, Default)]
struct Row {
    a_lin: Vec3,
    a_ang: Vec3,
    b_lin: Vec3,
    b_ang: Vec3,
}

/// A body's side of the constraints over a substep: its rotation, heading, and how it
/// answers an impulse.
#[derive(Clone, Copy)]
struct Side {
    rot: Quat,
    fwd: Vec3,
    m: f32,
    mt: f32,
    inertia: Vec3,
}

impl Side {
    fn new(b: &RigidBody, s: &Substep, l: &Lateral) -> Side {
        Side { rot: s.rot, fwd: s.body_fwd, m: l.m, mt: l.mt, inertia: b.inertia }
    }

    /// The velocity of a linear impulse `p` and the world angular velocity of an angular one.
    fn lin(&self, p: Vec3) -> Vec3 {
        super::lin(p, self.fwd, self.m, self.mt)
    }

    fn ang(&self, l: Vec3) -> Vec3 {
        self.rot.mul_vec3(self.rot.inverse().mul_vec3(l) / self.inertia)
    }

    fn apply(&self, l: &mut Lateral, p: Vec3, ang: Vec3) {
        let ang_b = self.rot.inverse().mul_vec3(ang);
        l.v += self.lin(p);
        l.w += ang_b / self.inertia;
        l.joint_p += p;
        l.joint_l += ang_b;
        l.jointed = true;
    }
}

impl Row {
    fn velocity(&self, sa: &Side, la: &Lateral, sb: &Side, lb: &Lateral) -> f32 {
        self.a_lin.dot(la.v) + self.a_ang.dot(sa.rot.mul_vec3(la.w)) + self.b_lin.dot(lb.v) + self.b_ang.dot(sb.rot.mul_vec3(lb.w))
    }

    /// What one unit of impulse along `other` does to this row's velocity.
    fn coupling(&self, other: &Row, sa: &Side, sb: &Side) -> f32 {
        self.a_lin.dot(sa.lin(other.a_lin)) + self.a_ang.dot(sa.ang(other.a_ang)) + self.b_lin.dot(sb.lin(other.b_lin)) + self.b_ang.dot(sb.ang(other.b_ang))
    }

    fn apply(&self, sa: &Side, la: &mut Lateral, sb: &Side, lb: &mut Lateral, lambda: f32) {
        sa.apply(la, self.a_lin * lambda, self.a_ang * lambda);
        sb.apply(lb, self.b_lin * lambda, self.b_ang * lambda);
    }
}

/// A constraint of one row: the velocity it asks for, its effective mass, and whether it only
/// pushes (a limit) - with the impulse it has given so far.
struct Scalar {
    row: Row,
    target: f32,
    inv_k: f32,
    unilateral: bool,
    acc: f32,
}

/// A joint's constraints over a substep.
struct JointRows {
    sides: [Side; 2],
    point: [Row; 3],
    point_k_inv: Mat3,
    point_target: Vec3,
    scalars: Vec<Scalar>,
}

/// The two entries `i` and `j` (different) of a slice.
fn pair<T>(v: &mut [T], i: usize, j: usize) -> (&mut T, &mut T) {
    assert_ne!(i, j);
    if i < j {
        let (x, y) = v.split_at_mut(j);
        (&mut x[i], &mut y[0])
    } else {
        let (x, y) = v.split_at_mut(i);
        (&mut y[0], &mut x[j])
    }
}

/// The bias velocity that takes `BAUMGARTE` of an error away in a substep, capped.
fn bias(err: f32, h: f32) -> f32 {
    (-BAUMGARTE * err / h).clamp(-MAX_BIAS, MAX_BIAS)
}

impl JointRows {
    fn new(j: &Joint, bodies: &[RigidBody], subs: &[Substep], lats: &[Lateral], h: f32) -> JointRows {
        let (lead, follow) = (&bodies[j.lead], &bodies[j.follow]);
        let sides = [Side::new(lead, &subs[j.lead], &lats[j.lead]), Side::new(follow, &subs[j.follow], &lats[j.follow])];
        let (sa, sb) = (&sides[0], &sides[1]);
        // The ball as the lead's link holds it moves with the lead, and pitches about the
        // hinge (axis `a`) as the follower pitches against the lead.
        let (r_hinge, link, err) = j.ball(lead, follow);
        let r_lead = r_hinge + link;
        let r_follow = (lead.position - follow.position).as_vec3() + r_lead - err;
        let a = lead.orientation.mul_vec3(Vec3::X);
        let a_link = a.cross(link);
        let point = [Vec3::X, Vec3::Y, Vec3::Z].map(|e| {
            let s = e.dot(a_link);
            Row { a_lin: e, a_ang: r_lead.cross(e) - a * s, b_lin: -e, b_ang: a * s - r_follow.cross(e) }
        });
        let mut k = Mat3::ZERO;
        for c in 0..3 {
            for r in 0..3 {
                k.col_mut(c)[r] = point[r].coupling(&point[c], sa, sb);
            }
        }
        let point_k_inv = if k.determinant().abs() > 1e-12 { k.inverse() } else { Mat3::ZERO };
        let point_target = Vec3::new(bias(err.x, h), bias(err.y, h), bias(err.z, h));
        let mut scalars = Vec::new();
        let mut push = |row: Row, target: f32, unilateral: bool| {
            let k = row.coupling(&row, sa, sb);
            if k > 1e-9 {
                scalars.push(Scalar { row, target, inv_k: 1.0 / k, unilateral, acc: 0.0 });
            }
        };
        let (pitch_axis, yaw_axis) = j.axes(lead, follow);
        if j.kind == JointKind::Articulation {
            // no roll between the sections: the yaw axis stays square to the pitch axis
            let (p, q) = if j.pitch_offset > 0.0 { (pitch_axis, yaw_axis) } else { (yaw_axis, pitch_axis) };
            let n = p.cross(q);
            push(Row { a_ang: n, b_ang: -n, ..Default::default() }, bias(p.dot(q), h), false);
        }
        // the limits: alpha grows as the follower turns left of the lead, beta as it pitches
        // down against it (d alpha / dt = (w_f - w_l) . yaw, d beta / dt = (w_l - w_f) . pitch)
        let (alpha, beta) = j.angles(bodies);
        let mut limit = |gap: f32, a_ang: Vec3| {
            if gap < LIMIT_WINDOW {
                let target = if gap > 0.0 { -gap / h } else { bias(gap, h).abs() };
                push(Row { a_ang, b_ang: -a_ang, ..Default::default() }, target, true);
            }
        };
        if j.kind == JointKind::Articulation {
            limit(j.alpha_max - alpha, yaw_axis);
            limit(j.alpha_max + alpha, -yaw_axis);
        }
        limit(j.beta_max - beta, -pitch_axis);
        limit(beta - j.beta_min, pitch_axis);
        JointRows { sides, point, point_k_inv, point_target, scalars }
    }

    fn solve(&mut self, j: &Joint, lats: &mut [Lateral]) {
        let (la, lb) = pair(lats, j.lead, j.follow);
        let [sa, sb] = self.sides;
        let v = Vec3::from_array(self.point.map(|r| r.velocity(&sa, la, &sb, lb)));
        let lambda = self.point_k_inv.mul_vec3(self.point_target - v);
        for (r, l) in self.point.iter().zip(lambda.to_array()) {
            r.apply(&sa, la, &sb, lb, l);
        }
        for s in self.scalars.iter_mut() {
            let v = s.row.velocity(&sa, la, &sb, lb);
            let before = s.acc;
            s.acc += (s.target - v) * s.inv_k;
            if s.unilateral {
                s.acc = s.acc.max(0.0);
            }
            s.row.apply(&sa, la, &sb, lb, s.acc - before);
        }
    }
}

/// One step of a train (see the module's notes): `drive_torque` (N m, from `M_Wheel`) shared
/// by every driven wheel of every body - a pusher's front section drives none and is pushed -,
/// one [`BodyInputs`] per body. Each body is stepped as [`RigidBody::step`] steps one alone:
/// the same slices and substeps, its own pitch and roll damping, and should any body come out
/// of the step at no number, the whole train stays where it stood, at rest.
pub fn step_train(bodies: &mut [RigidBody], joints: &[Joint], dt: f32, drive_torque: f32, inputs: &[BodyInputs], probe: &dyn Fn(f64, f64, f64) -> GroundProbe) {
    assert_eq!(bodies.len(), inputs.len());
    // a long frame (below 20 fps) is stepped in slices of at most 50 ms, so that the
    // body covers the whole frame the clock, the odometer and the scripts count (it was
    // cut to 50 ms: at 10 fps the bus went half as far as the time went on)
    let dt = if dt.is_finite() { dt.clamp(0.0, 0.25) } else { 0.0 };
    // A script's value that is no number - a bus's `M_Wheel` or a brake force of 0/0 or
    // 1/0 - put the whole body at NaN within a step: the bus vanished and the steering's
    // clamp stopped the game a frame later ("min > max, or either was NaN", #1045).
    // Such an input counts as none; and should the body still come out of the step at NaN
    // (a value of the file's), it stays where it stood before, at rest.
    let finite = |x: f32| if x.is_finite() { x } else { 0.0 };
    let drive_torque = finite(drive_torque);
    let brakes: Vec<Cow<[f32]>> = inputs.iter().map(|i| if i.brake.iter().all(|b| b.is_finite()) { Cow::Borrowed(i.brake) } else { Cow::Owned(i.brake.iter().map(|&b| finite(b)).collect()) }).collect();
    let steers: Vec<Option<f32>> = inputs.iter().map(|i| i.steer.map(finite)).collect();
    let air: Vec<bool> = inputs.iter().map(|i| i.air).collect();
    let before: Vec<_> = bodies.iter().map(|b| (b.position, b.orientation, b.steer_deg, b.wheels.iter().map(|w| (w.compression, w.touch)).collect::<Vec<_>>())).collect();
    // Every driven wheel of the train takes its share of `M_Wheel`. A body without
    // driven wheels of its own (a pusher's front section) was pushed by nothing: the
    // count was clamped to one and the torque went to wheels that do not drive.
    let driven = bodies.iter().map(|b| b.wheels.iter().filter(|w| w.driven).count() + b.coupled.iter().map(|d| d.driven_wheels).sum::<usize>()).sum::<usize>().max(1) as f32;
    let slices = (dt / 0.05).ceil().max(1.0) as usize;
    let mut impacts: Vec<Vec<Impact>> = vec![Vec::new(); bodies.len()];
    for _ in 0..slices {
        slice(bodies, joints, dt / slices as f32, drive_torque, driven, &brakes, &steers, &air, probe);
        for (b, impacts) in bodies.iter_mut().zip(impacts.iter_mut()) {
            // OMSI damps the body's pitch and roll - never its yaw - by sin(x)/x each frame,
            // x = 1.5 x sqrt(springs / mass) x frame (Omsi.exe 0x7e4f55..0x7e50b0). Its
            // frames are OMSI's (its options.cfg caps them at 30 a second): taken per frame
            // of this game, a machine drawing 150 frames a second damped the body a fifth as
            // much, and the bus rocked on its springs like a boat. The same damping a second
            // as OMSI's at 30 frames, however many are drawn:
            let k = body_damping(b.body_freq, dt / slices as f32);
            b.omega.x *= k;
            b.omega.y *= k;
            // (one impact per obstacle a frame, as a single slice gives)
            for m in b.wheel_impacts.drain(..) {
                match impacts.iter_mut().find(|x: &&mut Impact| x.obstacle == m.obstacle) {
                    Some(x) if x.energy >= m.energy => {}
                    Some(x) => *x = m,
                    None => impacts.push(m),
                }
            }
        }
    }
    for (b, impacts) in bodies.iter_mut().zip(impacts) {
        b.wheel_impacts = impacts;
    }
    if !bodies.iter().all(|b| b.position.is_finite() && b.orientation.is_finite() && b.velocity.is_finite() && b.omega.is_finite()) {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| log::warn!("vehicle physics: the body's state became no number; it stays where it was, at rest"));
        for (b, (position, orientation, steer_deg, wheels)) in bodies.iter_mut().zip(before) {
            (b.position, b.orientation, b.steer_deg) = (position, orientation, steer_deg);
            b.velocity = Vec3::ZERO;
            b.omega = Vec3::ZERO;
            b.accel_body = Vec3::ZERO;
            for (w, (c, t)) in b.wheels.iter_mut().zip(wheels) {
                (w.compression, w.touch) = (c, t);
                w.compression_rate = 0.0;
                w.spin = 0.0;
            }
            b.wheel_impacts.clear();
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn slice(bodies: &mut [RigidBody], joints: &[Joint], dt: f32, drive_torque: f32, driven: f32, brakes: &[Cow<[f32]>], steers: &[Option<f32>], air: &[bool], probe: &dyn Fn(f64, f64, f64) -> GroundProbe) {
    let n = bodies.len();
    // The steering sets the curvature the bus turns on (`steer` 1 = the full lock of
    // `[inv_min_turnradius]`); a body behind turns as its joint bends it.
    let kappas: Vec<f32> = (0..n)
        .map(|i| match steers[i] {
            Some(s) => s.clamp(-1.0, 1.0) * bodies[i].inv_min_turn_radius,
            None => joints.iter().find(|j| j.follow == i).map_or(0.0, |j| j.kappa(bodies)),
        })
        .collect();
    for (b, k) in bodies.iter_mut().zip(kappas) {
        b.steer_slice(k);
    }
    // (substeps of at most ~4 ms whatever the frame: a frame held up by loading - 50 ms -
    // made 12 ms substeps, too long for the stiff tyres, and the body hopped on its
    // springs for no reason the driver could see)
    let substeps = ((dt / 0.0042).ceil() as usize).clamp(4, 16);
    let h = dt / substeps as f32;
    let mut accel = vec![Vec3::ZERO; n];
    let passes = if joints.is_empty() { PASSES } else { JOINT_PASSES };
    for _ in 0..substeps {
        let mut subs: Vec<Substep> = (0..n).map(|i| bodies[i].prepare(h, drive_torque, driven, &brakes[i], air[i], probe)).collect();
        let mut lats: Vec<Lateral> = (0..n).map(|i| bodies[i].lateral_start(&subs[i], h)).collect();
        let mut rows: Vec<JointRows> = joints.iter().map(|j| JointRows::new(j, bodies, &subs, &lats, h)).collect();
        for _ in 0..passes {
            for i in 0..n {
                bodies[i].lateral_pass(&subs[i], &mut lats[i], h);
            }
            for (j, r) in joints.iter().zip(rows.iter_mut()) {
                r.solve(j, &mut lats);
            }
        }
        for i in 0..n {
            bodies[i].lateral_end(&mut subs[i], &lats[i], h);
            bodies[i].integrate(&subs[i], h, &mut accel[i]);
        }
    }
    // Asleep: with the brakes holding and no drive, the last few millimetres per
    // second of horizontal drift are noise from the tyre model, not motion. Take them
    // away so the bus stands truly still (the vertical velocity stays: the suspension
    // may still be settling). (The whole train or none of it: a section stopped on its
    // own was pulled along by the other through the joint.)
    let braked: f32 = brakes.iter().map(|b| b.iter().sum::<f32>()).sum();
    let mass: f32 = bodies.iter().map(|b| b.mass).sum();
    if bodies.iter().all(|b| Vec3::new(b.velocity.x, b.velocity.y, 0.0).length() < 0.03) && drive_torque.abs() < 1.0 && braked > 0.02 * mass * 9.81 {
        for b in bodies.iter_mut() {
            b.velocity.x = 0.0;
            b.velocity.y = 0.0;
            b.omega.z = 0.0;
        }
    }
    for (b, a) in bodies.iter_mut().zip(accel) {
        b.end_slice(substeps, a);
    }
}

/// The least-squares split of a load `total` with its centre at `centre` over points at
/// `(long, count)`: the load per point grows linearly along the vehicle, so that the shares
/// balance the load and its moment (exactly, with two places to rest on).
fn split(points: &[(f32, f32)], total: f32, centre: f32) -> (f32, f32) {
    let n: f32 = points.iter().map(|p| p.1).sum::<f32>().max(1.0);
    let sy: f32 = points.iter().map(|p| p.1 * p.0).sum();
    let syy: f32 = points.iter().map(|p| p.1 * p.0 * p.0).sum();
    let det = n * syy - sy * sy;
    if det.abs() > 1e-6 {
        ((total * syy - sy * total * centre) / det, (n * total * centre - sy * total) / det)
    } else {
        (total / n, 0.0)
    }
}

/// [`super::wheel_rest_loads`] for a part that also rests on its front joint at `support_long`
/// (an articulated bus's rear section, a semitrailer): the static load on each wheel of each
/// axle (N), and what the joint carries (N), which the part in front takes on its axles (see
/// [`point_load_shares`]).
pub fn wheel_rest_loads_supported(def: &Vehicle, support_long: f32) -> (Vec<f32>, f32) {
    let mass = if def.mass < 100.0 { def.mass * 1000.0 } else { def.mass }.max(500.0);
    let weight = mass * 9.81;
    let cog_y = def.cog.map(|c| c[1]).unwrap_or(0.0);
    let mut points: Vec<(f32, f32)> = def.axles.iter().map(|a| (a.long, 2.0)).collect();
    points.push((support_long, 1.0));
    let (a, b) = split(&points, weight, cog_y);
    let n = points.iter().map(|p| p.1).sum::<f32>();
    let joint = (a + b * support_long).clamp(0.0, weight);
    (def.axles.iter().map(|x| (a + b * x.long).max(weight / n * 0.2)).collect(), joint)
}

/// The share (N) of each wheel of each axle of `def` in a load `load` put on it at `long` (a
/// joint carrying part of the part behind).
pub fn point_load_shares(def: &Vehicle, long: f32, load: f32) -> Vec<f32> {
    let points: Vec<(f32, f32)> = def.axles.iter().map(|a| (a.long, 2.0)).collect();
    let (a, b) = split(&points, load, long);
    def.axles.iter().map(|x| a + b * x.long).collect()
}

/// The static loads of the train's wheels with the joints carrying their share
/// (`defs[i]`: body `i`'s file): each follower rests on its joint as well as on its axles,
/// and the lead takes what the joint carries on its own (joints in order from the front).
pub fn support_rest_loads(bodies: &mut [RigidBody], joints: &[Joint], defs: &[&Vehicle]) {
    for j in joints {
        let (loads, joint) = wheel_rest_loads_supported(defs[j.follow], j.anchor_follow.y);
        let f = &mut bodies[j.follow];
        for (w, a) in f.wheels.iter_mut().zip(f.wheel_axle.iter()) {
            w.rest_load = loads.get(*a).copied().unwrap_or(w.rest_load);
        }
        let shares = point_load_shares(defs[j.lead], j.anchor_lead.y, joint);
        let l = &mut bodies[j.lead];
        for (w, a) in l.wheels.iter_mut().zip(l.wheel_axle.iter()) {
            w.rest_load = (w.rest_load + shares.get(*a).copied().unwrap_or(0.0)).max(0.0);
        }
    }
}

/// Put the train down at rest: the first body as [`RigidBody::place`] does, at `origin` and
/// `headings[0]`, the others hung on their joints (see [`hang_followers`]).
pub fn place_train(bodies: &mut [RigidBody], joints: &[Joint], origin: DVec3, headings: &[f64]) {
    if let Some(first) = bodies.first_mut() {
        first.place(origin, headings.first().copied().unwrap_or(0.0));
    }
    hang_followers(bodies, joints, headings);
}

/// Put every follower down at rest on its joint, turned to `headings[follow]` and its ball at
/// the lead's (the lead is placed already: joints in order from the front).
pub fn hang_followers(bodies: &mut [RigidBody], joints: &[Joint], headings: &[f64]) {
    for j in joints {
        let lead = &bodies[j.lead];
        let ball = lead.position + lead.orientation.mul_vec3(j.anchor_lead - lead.cog).as_dvec3();
        let f = &mut bodies[j.follow];
        let heading = headings.get(j.follow).copied().unwrap_or(0.0);
        let rot = Quat::from_rotation_z((-heading).to_radians() as f32);
        let origin = ball - rot.mul_vec3(j.anchor_follow).as_dvec3() - DVec3::Z * f.rest_sag() as f64;
        f.place(origin, heading);
    }
}

impl RigidBody {
    /// How far the model origin stands above the ground at rest, on average over the wheels
    /// (m): where [`RigidBody::place`] puts it.
    pub(super) fn rest_sag(&self) -> f32 {
        let n = self.wheels.len().max(1) as f32;
        self.wheels.iter().map(|w| w.radius - w.attach.z - w.rest_compression().min(BUMP)).sum::<f32>() / n
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{bus, road};
    use super::*;
    use omsi_vehicle::vehicle::{Axle, Coupling};

    /// The stock GN92's two sections (`MAN_GN92_main.bus`, `MAN_GN92_trail.bus`): a pusher,
    /// whose front section drives no wheel and whose rear one stands on one axle and the joint.
    fn gn92() -> (Vehicle, Vehicle) {
        let axle = |long: f32, max_width: f32, min_width: f32, driven: bool| Axle { long, max_width, min_width, wheel_diameter: 0.94, spring: 280.0, max_force: 116.0, damper: 20.0, driven, inertia_inv: 0.0 };
        let front = Vehicle {
            mass: 8.0,
            moment_of_inertia: [300.0, 80.0, 300.0],
            cog_height: 1.2,
            rolling_resistance: 900.0,
            rot_pnt_long: -2.7,
            inv_min_turn_radius: 0.13,
            axles: vec![Axle { spring: 240.0, max_force: 90.0, ..axle(3.238, 2.25, 1.76, false) }, axle(-2.637, 2.4, 1.2, false)],
            coupling_back: Some(Coupling { pos: [0.0, -4.331, 0.315] }),
            ..Default::default()
        };
        let rear = Vehicle {
            mass: 7.2,
            moment_of_inertia: [150.0, 40.0, 150.0],
            cog_height: 1.2,
            rolling_resistance: 500.0,
            rot_pnt_long: -0.387,
            axles: vec![axle(-0.387, 2.4, 1.2, true)],
            coupling_front: Some(Coupling { pos: [0.0, 4.169, 0.315] }),
            coupling_front_character: Some([52.5, -20.0, 20.0, 1.0]),
            ..Default::default()
        };
        (front, rear)
    }

    struct Train {
        bodies: Vec<RigidBody>,
        joints: Vec<Joint>,
        /// The largest gap at the joint so far, and the angles at the end of the last step.
        gap: f32,
        alpha: f32,
        beta: f32,
    }

    impl Train {
        fn new(front: &Vehicle, rear: &Vehicle) -> Train {
            let mut bodies = vec![RigidBody::from_definition(front, &[]), RigidBody::from_definition(rear, &[])];
            let back = Vec3::from(front.coupling_back.as_ref().unwrap().pos);
            let at = Vec3::from(rear.coupling_front.as_ref().unwrap().pos);
            let joints = vec![Joint::new(0, 1, rear, back, at)];
            support_rest_loads(&mut bodies, &joints, &[front, rear]);
            place_train(&mut bodies, &joints, DVec3::ZERO, &[0.0, 0.0]);
            Train { bodies, joints, gap: 0.0, alpha: 0.0, beta: 0.0 }
        }

        /// `secs` at 60 frames a second: `torque` from the speed it is at, brakes per wheel of
        /// the front and the rear section, steering from the time.
        fn run(&mut self, secs: f32, torque: &dyn Fn(f32) -> f32, brake: [f32; 2], steer: &dyn Fn(f32) -> f32, g: &dyn Fn(f64, f64, f64) -> GroundProbe) {
            let (b0, b1) = (vec![brake[0]; self.bodies[0].wheels.len()], vec![brake[1]; self.bodies[1].wheels.len()]);
            for k in 0..(secs * 60.0).round() as usize {
                let t = k as f32 / 60.0;
                let m = torque(self.bodies[0].forward_speed());
                let inputs = [BodyInputs { brake: &b0, steer: Some(steer(t)), air: true }, BodyInputs { brake: &b1, steer: None, air: false }];
                step_train(&mut self.bodies, &self.joints, 1.0 / 60.0, m, &inputs, g);
                assert!(self.bodies.iter().all(|b| b.position.is_finite() && b.velocity.is_finite()));
                self.gap = self.gap.max(self.joints[0].gap(&self.bodies));
                (self.alpha, self.beta) = self.joints[0].angles(&self.bodies);
            }
        }

        fn front(&self) -> &RigidBody {
            &self.bodies[0]
        }
    }

    /// The torque (N m at a 0.47 m wheel) that keeps a train of `mass` kg near `v` m/s.
    fn cruise(v: f32, mass: f32) -> impl Fn(f32) -> f32 {
        move |at: f32| ((v - at) * mass * 1.5 + 0.012 * mass * 9.81 * v.signum()) * 0.47
    }

    fn none(_: f32) -> f32 {
        0.0
    }

    /// Humps 6 cm high and 0.6 m long every 7 m.
    fn humps(_x: f64, y: f64, top: f64) -> GroundProbe {
        let z = if y > 10.0 && (y % 7.0) < 0.6 { 0.06 } else { 0.0 };
        if z <= top { GroundProbe { below: Some(z), above: None } } else { GroundProbe { below: Some(0.0), above: Some(z) } }
    }

    /// A grade of `s` from y = 20 on.
    fn ramp(s: f64) -> impl Fn(f64, f64, f64) -> GroundProbe {
        move |_x, y, top| {
            let z = (y - 20.0).max(0.0) * s;
            if z <= top { GroundProbe { below: Some(z), above: None } } else { GroundProbe { below: None, above: Some(z) } }
        }
    }

    /// Two bodies of a train without a joint, both steered, both in the air's way, step exactly
    /// as each would alone: the train takes nothing from the calculation of a body but the
    /// source of its curvature and of the air (the drive is shared by the wheels of both).
    #[test]
    fn a_follower_without_a_joint_is_the_leader_again() {
        let mut lone = RigidBody::from_definition(&bus(), &[]);
        lone.place(DVec3::ZERO, 0.0);
        let mut pair = vec![lone.clone(), lone.clone()];
        let g = road(30.0, 0.05);
        for k in 0..900 {
            let t = k as f32 / 60.0;
            let torque = if t < 6.0 { 6000.0 } else { 0.0 };
            let brake = [if t > 10.0 { 15_000.0 } else { 0.0 }; 4];
            let steer = (t * 0.8).sin() * 0.7;
            lone.step(1.0 / 60.0, torque, &brake, steer, &g);
            let inputs = [BodyInputs { brake: &brake, steer: Some(steer), air: true }; 2];
            step_train(&mut pair, &[], 1.0 / 60.0, 2.0 * torque, &inputs, &g);
            for b in &pair {
                assert_eq!((b.position, b.orientation, b.velocity, b.omega), (lone.position, lone.orientation, lone.velocity, lone.omega), "at {t} s");
                assert!(b.wheels.iter().zip(&lone.wheels).all(|(a, b)| a.compression == b.compression && a.spin == b.spin));
            }
        }
        assert!(lone.origin().y > 30.0, "{:?}", lone.origin());
    }

    /// The joint holds: weaving at 40 km/h, braking hard from there and over a run of humps
    /// at 30 km/h, the two balls stay within 2 cm of each other.
    #[test]
    fn the_joint_stays_closed() {
        let (front, rear) = gn92();
        let mass = 15_200.0;
        let flat = road(1e9, 0.0);
        let mut t = Train::new(&front, &rear);
        t.run(1.0, &none, [5000.0; 2], &none, &flat);
        t.gap = 0.0;
        t.run(12.0, &cruise(40.0 / 3.6, mass), [0.0; 2], &none, &flat);
        t.run(20.0, &cruise(40.0 / 3.6, mass), [0.0; 2], &|s| (s * 0.7).sin() * 0.5, &flat);
        assert!(t.front().forward_speed() > 9.0, "{}", t.front().forward_speed());
        assert!(t.gap < 0.02, "weaving: the joint opened {} m", t.gap);
        t.run(5.0, &none, [20_000.0; 2], &none, &flat);
        assert!(t.front().forward_speed().abs() < 0.1, "{}", t.front().forward_speed());
        assert!(t.gap < 0.02, "braking: the joint opened {} m", t.gap);
        let mut t = Train::new(&front, &rear);
        t.run(1.0, &none, [5000.0; 2], &none, &humps);
        t.gap = 0.0;
        t.run(15.0, &cruise(30.0 / 3.6, mass), [0.0; 2], &none, &humps);
        assert!(t.front().origin().y > 60.0, "{:?}", t.front().origin());
        assert!(t.gap < 0.02, "humps: the joint opened {} m", t.gap);
    }

    /// Standing, the front section's rear axle carries the share of the rear section the
    /// joint holds (some 6 kN of the GN92's 71 kN, the rest on its one axle), which lifts the
    /// front axle a little and pitches the front section nose up.
    #[test]
    fn the_joint_weighs_on_the_front_sections_rear_axle() {
        let (front, rear) = gn92();
        let flat = road(1e9, 0.0);
        let mut t = Train::new(&front, &rear);
        t.run(4.0, &none, [5000.0; 2], &none, &flat);
        let mut lone = RigidBody::from_definition(&front, &[]);
        lone.place(DVec3::ZERO, 0.0);
        super::super::tests::run(&mut lone, 4.0, 0.0, 5000.0, &flat);
        let axle = |b: &RigidBody, a: usize| b.wheels.iter().zip(&b.wheel_axle).filter(|(_, x)| **x == a).map(|(w, _)| w.load).sum::<f32>();
        let (gain_rear, gain_front) = (axle(t.front(), 1) - axle(&lone, 1), axle(t.front(), 0) - axle(&lone, 0));
        eprintln!("the joint on the front section: rear axle {gain_rear:+.0} N, front axle {gain_front:+.0} N");
        assert!(gain_rear > 5000.0 && gain_rear < 11_000.0, "rear axle took {gain_rear} N more");
        assert!(gain_front < 0.0, "front axle took {gain_front} N more");
        let carried = axle(&t.bodies[1], 0);
        let weight = 7200.0 * 9.81;
        assert!((weight - carried - (gain_rear + gain_front)).abs() < 1000.0, "rear axle {carried} N, front section {} N", gain_rear + gain_front);
        let (_, pitch, _) = t.front().heading_pitch_bank();
        let (_, lone_pitch, _) = lone.heading_pitch_bank();
        assert!(pitch > lone_pitch + 0.03, "pitch {pitch} alone {lone_pitch}");
        assert!(t.gap < 0.02, "{}", t.gap);
    }

    /// The rear section's mass comes back to the front through the joint: braking on the
    /// front section's wheels alone, the train slows much less than the front section alone
    /// and dips differently; a heavier rear section with more inertia turns and rolls the
    /// front differently in the same bend, and so does a joint that lets it roll on its own.
    #[test]
    fn the_rear_sections_mass_and_inertia_reach_the_front() {
        let (front, rear) = gn92();
        let flat = road(1e9, 0.0);
        let start = |rear: &Vehicle| {
            let mut t = Train::new(&front, rear);
            t.run(1.0, &none, [5000.0; 2], &none, &flat);
            let m = t.bodies.iter().map(|b| b.mass).sum::<f32>();
            t.run(10.0, &cruise(40.0 / 3.6, m), [0.0; 2], &none, &flat);
            t
        };
        // braking
        let mut t = start(&rear);
        let v0 = t.front().forward_speed();
        let mut dip = 0.0f32;
        for _ in 0..30 {
            t.run(1.0 / 60.0, &none, [12_000.0, 0.0], &none, &flat);
            dip = dip.min(t.front().heading_pitch_bank().1);
        }
        let train_decel = (v0 - t.front().forward_speed()) * 2.0;
        let mut lone = RigidBody::from_definition(&front, &[]);
        lone.place(DVec3::ZERO, 0.0);
        super::super::tests::run(&mut lone, 1.0, 0.0, 5000.0, &flat);
        lone.velocity = Vec3::Y * v0;
        lone.wheels.iter_mut().for_each(|w| w.spin = v0 / w.radius);
        let mut lone_dip = 0.0f32;
        for _ in 0..30 {
            lone.step(1.0 / 60.0, 0.0, &[12_000.0; 4], 0.0, &flat);
            lone_dip = lone_dip.min(lone.heading_pitch_bank().1);
        }
        let lone_decel = (v0 - lone.forward_speed()) * 2.0;
        eprintln!("braking on the front: train {train_decel:.2} m/s², front alone {lone_decel:.2}; dip {dip:.2}° / {lone_dip:.2}°");
        assert!(train_decel < 0.7 * lone_decel, "train {train_decel}, alone {lone_decel}");
        assert!((dip - lone_dip).abs() > 0.05, "dip {dip} alone {lone_dip}");
        assert!(t.gap < 0.02, "{}", t.gap);
        // a step of the steering at 40 km/h, rear sections of different mass and inertia
        let bend = |rear: &Vehicle, friction: f32| {
            let mut t = start(rear);
            t.bodies.iter_mut().for_each(|b| b.friction = friction);
            let (mut yaw, mut roll) = (0.0f32, 0.0f32);
            let m = t.bodies.iter().map(|b| b.mass).sum::<f32>();
            for _ in 0..90 {
                t.run(1.0 / 60.0, &cruise(40.0 / 3.6, m), [0.0; 2], &|_| 0.4, &flat);
                yaw = yaw.max(t.front().omega.z.abs());
                roll = roll.max(t.front().heading_pitch_bank().2.abs());
            }
            assert!(t.gap < 0.02, "{}", t.gap);
            (yaw, roll)
        };
        let light = Vehicle { mass: 3.0, moment_of_inertia: [60.0, 15.0, 60.0], ..rear.clone() };
        let heavy = Vehicle { mass: 12.0, moment_of_inertia: [250.0, 80.0, 250.0], ..rear.clone() };
        let ball = Vehicle { coupling_front_character: Some([52.5, -20.0, 20.0, 0.0]), ..rear.clone() };
        // (on a dry road the front follows its wheels, as Omsi.exe's holds it: the yaw is the
        // steering's; on a wet one its tyres slide and the rear section's inertia shows)
        let ((_, rl), (_, rh)) = (bend(&light, 0.85), bend(&heavy, 0.85));
        let ((yl, _), (yh, _)) = (bend(&light, 0.25), bend(&heavy, 0.25));
        let ((_, ra), (_, rb)) = (bend(&rear, 0.85), bend(&ball, 0.85));
        eprintln!("step steer: roll {rl:.2}° / {rh:.2}°, sliding yaw rate {yl:.3} / {yh:.3} rad/s (light / heavy rear); roll with the turntable {ra:.2}°, with a ball {rb:.2}°");
        assert!(rh > 1.2 * rl, "roll {rl} {rh}");
        assert!((yl - yh).abs() > 0.05 * yl.max(yh), "yaw rate {yl} {yh}");
        assert!(ra > rb + 0.2, "roll with the turntable {ra}, with a ball {rb}");
    }

    /// A pusher: the front section has no driven wheel and is pushed through the joint, the
    /// train running straight; the brakes of the rear section alone stop it.
    #[test]
    fn a_pusher_pushes_its_front_section() {
        let (front, rear) = gn92();
        let flat = road(1e9, 0.0);
        let mut t = Train::new(&front, &rear);
        t.run(1.0, &none, [5000.0; 2], &none, &flat);
        t.run(8.0, &|_| 9000.0, [0.0; 2], &none, &flat);
        let v = t.front().forward_speed();
        let (heading, _, _) = t.front().heading_pitch_bank();
        assert!(v > 3.0, "pushed to {v} m/s");
        assert!(heading.min(360.0 - heading) < 0.5 && t.alpha.abs() < 0.01, "heading {heading}, alpha {}", t.alpha);
        t.run(4.0, &none, [0.0, 30_000.0], &none, &flat);
        assert!(t.front().forward_speed().abs() < 0.1, "{}", t.front().forward_speed());
        assert!(t.gap < 0.02, "{}", t.gap);
    }

    /// Reversing on full lock folds a pusher up until its joint stops at
    /// `[coupling_front_character]`'s 52.5 degrees; it goes no further, and the joint holds.
    #[test]
    fn the_joint_stops_at_its_max_alpha() {
        let (front, rear) = gn92();
        let flat = road(1e9, 0.0);
        let mut t = Train::new(&front, &rear);
        t.run(1.0, &none, [5000.0; 2], &none, &flat);
        let mut most = 0.0f32;
        for _ in 0..(25 * 4) {
            t.run(0.25, &cruise(-1.5, 15_200.0), [0.0; 2], &|_| 1.0, &flat);
            most = most.max(t.alpha.abs());
        }
        eprintln!("reversing on full lock: alpha up to {:.1}°", most.to_degrees());
        assert!(most.to_degrees() > 45.0, "alpha only {}", most.to_degrees());
        assert!(most.to_degrees() < 52.5 + 1.5, "alpha {}", most.to_degrees());
        assert!(t.gap < 0.03, "{}", t.gap);
    }

    /// Beta stays within the file's range: with a joint that bends 2 degrees up and down the
    /// rear section is held in line with the front as it drives onto a 10 % grade, its wheel
    /// lifted rather than bending the joint.
    #[test]
    fn beta_stays_within_its_range() {
        let (front, rear) = gn92();
        let stiff = Vehicle { coupling_front_character: Some([52.5, -2.0, 2.0, 1.0]), ..rear.clone() };
        for (rear, limit) in [(&rear, 20.0f32), (&stiff, 2.0)] {
            let g = ramp(0.10);
            let mut t = Train::new(&front, rear);
            t.run(1.0, &none, [5000.0; 2], &none, &g);
            let mut most = 0.0f32;
            for _ in 0..(16 * 4) {
                t.run(0.25, &cruise(3.0, 15_200.0), [0.0; 2], &none, &g);
                most = most.max(t.beta.abs());
            }
            eprintln!("onto a 10 % grade with ±{limit}°: beta up to {:.2}°", most.to_degrees());
            assert!(t.front().origin().y > 35.0, "{:?}", t.front().origin());
            assert!(most.to_degrees() < limit + 0.5, "beta {}", most.to_degrees());
            assert!(t.gap < 0.02, "{}", t.gap);
        }
    }

    /// A pusher's joint that pitches about a hinge 0.8 m ahead of the ball holds over humps.
    #[test]
    fn a_hinge_ahead_of_the_ball_holds() {
        let (front, rear) = gn92();
        let rear = Vehicle { coupling_pitch_offset: Some(0.8), ..rear };
        let mut t = Train::new(&front, &rear);
        t.run(1.0, &none, [5000.0; 2], &none, &humps);
        t.gap = 0.0;
        t.run(15.0, &cruise(30.0 / 3.6, 15_200.0), [0.0; 2], &|s| (s * 0.5).sin() * 0.3, &humps);
        assert!(t.front().origin().y > 50.0, "{:?}", t.front().origin());
        assert!(t.gap < 0.02, "{}", t.gap);
    }
}
