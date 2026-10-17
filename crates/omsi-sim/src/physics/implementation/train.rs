//! Bodies coupled by joints: the sections of an articulated bus and a lorry's trailer, each
//! a [`RigidBody`] of its own `.bus` file on the same wheel physics, as OMSI 2 runs them.
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

use super::spec::{split, wheel_rest_loads_supported};
use super::tyre::{lin, Lateral, Substep, Work};
use crate::physics::domain::params::{BAUMGARTE, BUMP, FRAME_DT_MAX, JOINT_PASSES, LIMIT_WINDOW, MAX_BIAS, PASSES, SLICE_DT, SUBSTEP_DT};
use crate::physics::domain::{BodyInputs, GroundProbe, Impact, Joint, JointKind, RigidBody};
use glam::{DVec3, Mat3, Quat, Vec3};
use omsi_vehicle::Vehicle;
use std::borrow::Cow;

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
        lin(p, self.fwd, self.m, self.mt)
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
/// the same substeps, its own pitch and roll damping, and should any body come out of the
/// step at no number, the whole train stays where it stood, at rest.
pub fn step_train(bodies: &mut [RigidBody], joints: &[Joint], dt: f32, drive_torque: f32, inputs: &[BodyInputs], probe: &dyn Fn(f64, f64, f64) -> GroundProbe) {
    step_train_with(bodies, joints, dt, drive_torque, inputs, probe, &mut |_, _| {});
}

/// [`step_train`], with `after_slice(bodies, slice)` called after each slice of the step
/// (`slice` its length, s): where the bodies answer the obstacles around them
/// ([`RigidBody::collide`]), with the time that slice covered.
///
/// The frame runs as many substeps of [`SUBSTEP_DT`] as fit in its `dt` and in what the frames
/// before it left over (the first body keeps that, `RigidBody::unstepped`), in slices of at
/// most [`SLICE_DT`]; whatever the frame rate, the same drive makes the same motion.
#[allow(clippy::too_many_arguments)]
pub fn step_train_with(bodies: &mut [RigidBody], joints: &[Joint], dt: f32, drive_torque: f32, inputs: &[BodyInputs], probe: &dyn Fn(f64, f64, f64) -> GroundProbe, after_slice: &mut dyn FnMut(&mut [RigidBody], f32)) {
    assert_eq!(bodies.len(), inputs.len());
    if bodies.is_empty() {
        return;
    }
    // a long frame (below 20 fps) is stepped in slices, so that the body covers the whole
    // frame the clock, the odometer and the scripts count (it was cut to 50 ms: at 10 fps the
    // bus went half as far as the time went on)
    let dt = if dt.is_finite() { dt.clamp(0.0, FRAME_DT_MAX) } else { 0.0 };
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
    // the substeps this frame: what fits in the time it brings and what was left over (a
    // hair's breadth short of a whole one counts as one: 1/60 s is four 1/240 s substeps,
    // not three and a remainder)
    let time = (bodies[0].unstepped + dt).max(0.0);
    let substeps = (time / SUBSTEP_DT + 1e-3).floor() as usize;
    let unstepped = (time - substeps as f32 * SUBSTEP_DT).max(0.0);
    for b in bodies.iter_mut() {
        b.unstepped = unstepped;
        b.wheel_impacts.clear();
    }
    let per_slice = ((SLICE_DT / SUBSTEP_DT).round() as usize).max(1);
    let mut impacts: Vec<Vec<Impact>> = vec![Vec::new(); bodies.len()];
    let mut accel = vec![Vec3::ZERO; bodies.len()];
    let mut scratch = SolverScratch::new(bodies.len());
    let mut left = substeps;
    while left > 0 {
        let n = left.min(per_slice);
        left -= n;
        slice(bodies, joints, n, drive_torque, driven, &brakes, &steers, &air, probe, &mut scratch, &mut accel);
        after_slice(bodies, n as f32 * SUBSTEP_DT);
        for (b, impacts) in bodies.iter_mut().zip(impacts.iter_mut()) {
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
    for ((b, impacts), a) in bodies.iter_mut().zip(impacts).zip(accel) {
        b.wheel_impacts = impacts;
        // the acceleration the body felt over the frame (a frame that ran no substep keeps
        // the last one's)
        if substeps > 0 {
            b.accel_body = a / substeps as f32;
        }
    }
    if !bodies.iter().all(|b| b.position.is_finite() && b.orientation.is_finite() && b.velocity.is_finite() && b.omega.is_finite()) {
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| log::warn!("vehicle physics: the body's state became no number; it stays where it was, at rest"));
        for (b, (position, orientation, steer_deg, wheels)) in bodies.iter_mut().zip(before) {
            (b.position, b.orientation, b.steer_deg) = (position, orientation, steer_deg);
            (b.last_position, b.last_orientation) = (position, orientation);
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

/// What the solver works in, a body each (and a joint each for `rows`): filled anew every
/// substep, kept across the substeps and slices of a step rather than allocated for each.
struct SolverScratch {
    subs: Vec<Substep>,
    lats: Vec<Lateral>,
    works: Vec<Work>,
    rows: Vec<JointRows>,
}

impl SolverScratch {
    fn new(n: usize) -> SolverScratch {
        fn each<T: Default>(n: usize) -> Vec<T> {
            (0..n).map(|_| T::default()).collect()
        }
        SolverScratch { subs: each(n), lats: each(n), works: each(n), rows: Vec::new() }
    }
}

/// `substeps` substeps of [`SUBSTEP_DT`] of the whole train, the bodies' accelerations added
/// to `accel`.
#[allow(clippy::too_many_arguments)]
fn slice(bodies: &mut [RigidBody], joints: &[Joint], substeps: usize, drive_torque: f32, driven: f32, brakes: &[Cow<[f32]>], steers: &[Option<f32>], air: &[bool], probe: &dyn Fn(f64, f64, f64) -> GroundProbe, scratch: &mut SolverScratch, accel: &mut [Vec3]) {
    let n = bodies.len();
    let h = SUBSTEP_DT;
    let passes = if joints.is_empty() { PASSES } else { JOINT_PASSES };
    let braked: f32 = brakes.iter().map(|b| b.iter().sum::<f32>()).sum();
    let mass: f32 = bodies.iter().map(|b| b.mass).sum();
    for _ in 0..substeps {
        // The steering sets the curvature the bus turns on (`steer` 1 = the full lock of
        // `[inv_min_turnradius]`); a body behind turns as its joint bends it.
        let kappas: Vec<f32> = (0..n)
            .map(|i| match steers[i] {
                Some(s) => s.clamp(-1.0, 1.0) * bodies[i].inv_min_turn_radius,
                None => joints.iter().find(|j| j.follow == i).map_or(0.0, |j| j.kappa(bodies)),
            })
            .collect();
        for (b, k) in bodies.iter_mut().zip(kappas) {
            b.steer_to(k);
        }
        let SolverScratch { subs, lats, works, rows } = &mut *scratch;
        for i in 0..n {
            bodies[i].prepare(h, drive_torque, driven, &brakes[i], air[i], probe, &mut subs[i], &mut works[i]);
        }
        for i in 0..n {
            bodies[i].lateral_start(&subs[i], h, &mut lats[i]);
        }
        rows.clear();
        rows.extend(joints.iter().map(|j| JointRows::new(j, bodies, subs, lats, h)));
        for _ in 0..passes {
            for i in 0..n {
                bodies[i].lateral_pass(&subs[i], &mut lats[i], h);
            }
            for (j, r) in joints.iter().zip(rows.iter_mut()) {
                r.solve(j, lats);
            }
        }
        for i in 0..n {
            bodies[i].lateral_end(&mut subs[i], &lats[i], h);
            bodies[i].integrate(&subs[i], h, &mut accel[i]);
            bodies[i].end_substep(h);
        }
        // Asleep: with the brakes holding and no drive, the last few millimetres per
        // second of horizontal drift are noise from the tyre model, not motion. Take them
        // away so the bus stands truly still (the vertical velocity stays: the suspension
        // may still be settling). (The whole train or none of it: a section stopped on its
        // own was pulled along by the other through the joint.)
        if bodies.iter().all(|b| Vec3::new(b.velocity.x, b.velocity.y, 0.0).length() < 0.03) && drive_torque.abs() < 1.0 && braked > 0.02 * mass * 9.81 {
            for b in bodies.iter_mut() {
                b.velocity.x = 0.0;
                b.velocity.y = 0.0;
                b.omega.z = 0.0;
            }
        }
    }
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
mod tests;
