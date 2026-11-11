//! The poles of the player's trolleybus, step by step: up from the hooks onto the wire,
//! along it, off it (dewired, or taken off by the retract control), and down again.

use glam::DVec3;

use super::walk::{self, Reached, Way};
use crate::domain::declaration::{PoleGeometry, PoleParams, TrolleyDeclaration};
use crate::domain::network::{SpanId, WireNetwork, WireObjectKind, WireSide};
use crate::domain::pole::{
    BodyPose, DewireReason, PoleEvent, PoleInputs, PoleOutput, PoleOutputs, PolePhase, PoleSim,
    PoleState, WireCursor,
};

/// How far around the modelled shoe a rising pole looks for wire, metres.
const SEARCH_RADIUS: f64 = 2.5;

/// How long a fork stays set for the second shoe (and a shoe that steps back across the
/// gap), seconds.
const FORK_MEMORY: f64 = 10.0;

/// The way on at a fork, and the kind of object it is: a switch sets itself by the
/// vehicle's demand - with demand a Y turns left and a branch switch takes its branch,
/// without demand a Y turns right and a branch switch runs straight; a crossing runs
/// straight; where the content has no object the shoe keeps to the closest, straightest
/// wire.
fn fork_way(ways: &[Way], travel: DVec3, demand: bool) -> (usize, Option<WireObjectKind>) {
    let rank = |k: &WireObjectKind| match k {
        WireObjectKind::SwitchY
        | WireObjectKind::SwitchBranchLeft
        | WireObjectKind::SwitchBranchRight => 2,
        WireObjectKind::Crossing => 1,
        WireObjectKind::SoundMarker => 0,
    };
    let kind = ways.iter().filter_map(|w| w.via).max_by_key(rank);
    // degrees each way turns off the travel, left positive
    let turn = |w: &Way| {
        let t = travel.truncate().normalize_or_zero();
        let h = w.heading.truncate().normalize_or_zero();
        t.perp_dot(h).atan2(t.dot(h)).to_degrees()
    };
    let by = |f: &dyn Fn(&Way) -> f64| {
        (0..ways.len())
            .min_by(|a, b| f(&ways[*a]).total_cmp(&f(&ways[*b])))
            .unwrap_or(0)
    };
    let k = match (kind, demand) {
        (Some(WireObjectKind::SwitchY), true) => by(&|w| -turn(w)),
        (Some(WireObjectKind::SwitchY), false) => by(&|w| turn(w)),
        (Some(WireObjectKind::SwitchBranchLeft | WireObjectKind::SwitchBranchRight), true) => {
            by(&|w| -turn(w).abs())
        }
        (Some(_), _) => by(&|w| turn(w).abs()),
        (None, _) => by(&|w| w.gap + turn(w).abs() / 20.0),
    };
    (k, kind)
}

/// How the shoe sits on the wire this frame, for the dewirement rules.
#[derive(Debug, Clone, Copy)]
struct Contact {
    /// Degrees the pole stands off straight back (body frame).
    side_angle: f64,
    /// Degrees the shoe is turned against the wire.
    turn: f64,
    /// The wire ended under the shoe.
    wire_ended: bool,
}

impl PoleSim {
    /// One frame of `dt` seconds for the vehicle declared by `decl`, whose pole-carrying part
    /// stands at `body`.
    pub fn step(
        &mut self,
        decl: &TrolleyDeclaration,
        net: &WireNetwork,
        body: &BodyPose,
        inputs: PoleInputs,
        dt: f64,
    ) -> PoleOutputs {
        self.events.clear();
        if inputs.reset || dt > decl.params.stall_reset {
            if dt > decl.params.stall_reset
                && self.poles.iter().any(|p| p.phase != PolePhase::Hooked)
            {
                self.events.push(PoleEvent::StallReset { seconds: dt });
            }
            self.poles = Default::default();
        }
        // a fork is set for the two shoes passing it; forget it a while after
        self.demand = inputs.switch_demand > decl.params.switch_threshold;
        for f in self.forks.iter_mut() {
            f.2 += dt;
        }
        self.forks.retain(|f| f.2 < FORK_MEMORY);
        let retract = inputs.retract && !self.retract_before;
        self.retract_before = inputs.retract;
        let mut out = PoleOutputs {
            spline_id: -1,
            ..Default::default()
        };
        for i in 0..2 {
            let pressed = inputs.grab[i] && !self.grab_before[i];
            self.grab_before[i] = inputs.grab[i];
            if retract {
                self.retract(i, decl, net, body);
            }
            out.poles[i] = self.step_pole(i, decl, net, body, pressed, dt);
        }
        out.wire_present = out.poles.iter().all(|p| p.on_wire);
        if let Some(c) = self.poles[0].cursor.filter(|_| out.poles[0].on_wire) {
            if let Some(span) = net.span(c.span) {
                out.spline_id = span.spline_id;
                out.progress = progress(span.wires[c.side.index()].as_slice(), &c);
            }
        }
        out
    }

    fn step_pole(
        &mut self,
        i: usize,
        decl: &TrolleyDeclaration,
        net: &WireNetwork,
        body: &BodyPose,
        pressed: bool,
        dt: f64,
    ) -> PoleOutput {
        let geo = decl.poles[i];
        let p = decl.params;
        let foot_to_wire = foot_to_wire(decl, i, body);
        self.poles[i].timer += dt;
        match self.poles[i].phase {
            PolePhase::Hooked => {
                if !pressed {
                    return hooked(&geo);
                }
                let found = capture(
                    net,
                    body,
                    &geo,
                    i == 0,
                    foot_to_wire,
                    geo.length(),
                    p.capture_distance,
                );
                let Some((cursor, sideways)) = found else {
                    self.events.push(PoleEvent::NoWire { pole: i });
                    return hooked(&geo);
                };
                let spline_id = net.span(cursor.span).map_or(-1, |s| s.spline_id);
                self.events.push(PoleEvent::Captured {
                    pole: i,
                    spline_id,
                    sideways,
                });
                self.poles[i] = PoleState {
                    phase: PolePhase::Rising,
                    cursor: Some(cursor),
                    ..Default::default()
                };
            }
            PolePhase::OnWire if pressed => {
                let from = self
                    .track(i, decl, net, body, foot_to_wire)
                    .map_or((0.0, 0.0), |(o, _)| (o.yaw, o.pitch));
                self.poles[i] = PoleState {
                    phase: PolePhase::Lowering,
                    lowered_from: from,
                    ..Default::default()
                };
                return lowering(&mut self.poles[i], &p, &geo, &mut self.events, i);
            }
            PolePhase::Lowering => {
                return lowering(&mut self.poles[i], &p, &geo, &mut self.events, i);
            }
            PolePhase::Dewired => {
                return dewired(&mut self.poles[i], &p, &geo, &mut self.events, i);
            }
            PolePhase::Retracting => {
                return retracting(&mut self.poles[i], &p, &geo, &mut self.events, i);
            }
            PolePhase::Rising | PolePhase::OnWire => {}
        }
        // rising or on the wire: the shoe follows the wire
        let Some((mut o, contact)) = self.track(i, decl, net, body, foot_to_wire) else {
            self.poles[i] = PoleState::default();
            return hooked(&geo);
        };
        let state = &mut self.poles[i];
        match state.phase {
            PolePhase::OnWire => {
                if let Some(reason) = dewirement(state, &p, &o, &contact, dt) {
                    self.events.push(PoleEvent::Dewired { pole: i, reason });
                    *state = PoleState {
                        phase: PolePhase::Dewired,
                        lowered_from: (o.yaw, o.pitch),
                        dewire: Some(reason),
                        ..Default::default()
                    };
                    return dewired(state, &p, &geo, &mut self.events, i);
                }
            }
            PolePhase::Rising => {
                let k = (state.timer / p.raise_duration.max(1e-3)).clamp(0.0, 1.0);
                if k >= 1.0 {
                    state.phase = PolePhase::OnWire;
                    state.timer = 0.0;
                } else {
                    o.yaw *= k;
                    o.pitch *= k;
                    o.shoe_rot *= k;
                    o.force = 0.0;
                    o.on_wire = false;
                }
            }
            _ => {}
        }
        o
    }

    /// The pole `i` on its wire this frame (None: its wire is gone, the tile unloaded).
    fn track(
        &mut self,
        i: usize,
        decl: &TrolleyDeclaration,
        net: &WireNetwork,
        body: &BodyPose,
        foot_to_wire: DVec3,
    ) -> Option<(PoleOutput, Contact)> {
        let geo = decl.poles[i];
        let cursor = self.poles[i].cursor?;
        let Reached {
            cursor,
            point,
            error,
            joints,
            wire_ended,
        } = {
            let (forks, events, demand) = (&mut self.forks, &mut self.events, self.demand);
            let mut choose = |at: (SpanId, usize), ways: &[Way], travel: DVec3| -> usize {
                if let Some(k) = forks
                    .iter()
                    .find(|f| f.0 == at)
                    .and_then(|f| ways.iter().position(|w| w.cursor.span == f.1))
                {
                    return k;
                }
                let (k, kind) = fork_way(ways, travel, demand);
                forks.push((at, ways[k].cursor.span, 0.0));
                events.push(PoleEvent::Fork {
                    pole: i,
                    kind,
                    spline_id: net.span(ways[k].cursor.span).map_or(-1, |s| s.spline_id),
                    demand,
                });
                k
            };
            walk::reach(
                net,
                cursor,
                foot_to_wire,
                geo.length(),
                body.rotation * DVec3::NEG_Y,
                &mut choose,
            )?
        };
        for (from, to, gap) in joints {
            self.events.push(PoleEvent::Joint {
                pole: i,
                from,
                to,
                gap,
            });
        }
        if wire_ended {
            let spline_id = net.span(cursor.span).map_or(-1, |s| s.spline_id);
            self.events.push(PoleEvent::WireEnds { pole: i, spline_id });
        }
        let state = &mut self.poles[i];
        state.cursor = Some(cursor);
        state.reach_error = error;
        let up = body.rotation * DVec3::Z;
        let end = point + up * decl.params.shoe_height_offset;
        let wire_dir = walk::tangent(net, &cursor).unwrap_or(DVec3::Y);
        let (o, mut contact) = on_wire(decl, i, body, end, wire_dir);
        contact.wire_ended = wire_ended;
        Some((o, contact))
    }

    /// The retract control for pole `i`: off the wire, or off its way up, to the hook.
    fn retract(&mut self, i: usize, decl: &TrolleyDeclaration, net: &WireNetwork, body: &BodyPose) {
        let phase = self.poles[i].phase;
        if !matches!(phase, PolePhase::OnWire | PolePhase::Rising) {
            return;
        }
        let k = match phase {
            PolePhase::Rising => {
                (self.poles[i].timer / decl.params.raise_duration.max(1e-3)).clamp(0.0, 1.0)
            }
            _ => 1.0,
        };
        let from = self
            .track(i, decl, net, body, foot_to_wire(decl, i, body))
            .map_or((0.0, 0.0), |(o, _)| (o.yaw * k, o.pitch * k));
        self.poles[i] = PoleState {
            phase: PolePhase::Retracting,
            lowered_from: from,
            ..Default::default()
        };
        self.events.push(PoleEvent::Retracted { pole: i });
    }
}

/// The point the wire must lie at the pole's length from: its foot, moved by the shoe's
/// height off the wire (the pole's end stands `shoe_height_offset` off the wire).
fn foot_to_wire(decl: &TrolleyDeclaration, i: usize, body: &BodyPose) -> DVec3 {
    let foot = body.position + body.rotation * decl.poles[i].base;
    foot - body.rotation * DVec3::Z * decl.params.shoe_height_offset
}

/// Does the pole leave the wire this frame, and why? Each condition with a time must hold
/// that long without a break (so the frame rate does not change the outcome); a pole turned
/// past its limit, or a wire that ends under the shoe, leaves at once.
fn dewirement(
    state: &mut PoleState,
    p: &PoleParams,
    o: &PoleOutput,
    c: &Contact,
    dt: f64,
) -> Option<DewireReason> {
    if c.wire_ended {
        return Some(DewireReason::Gap);
    }
    // (out of reach the shoe stands at the nearest point of the wire, beside the foot: its
    // angle says nothing, the reach rule decides)
    if state.reach_error <= 0.0 && c.side_angle.abs() > p.yaw_limit {
        return Some(DewireReason::SideAngle);
    }
    let side_force = o.force * c.turn.abs().min(89.0).to_radians().tan();
    let conditions = [
        (
            state.reach_error > p.reach_release_error,
            p.reach_release_time,
            DewireReason::Reach,
        ),
        (
            o.force < p.force_dewire,
            p.force_dewire_time,
            DewireReason::Force,
        ),
        (
            side_force > p.side_force_dewire,
            p.side_force_time,
            DewireReason::SideForce,
        ),
    ];
    for (k, (holds, time, reason)) in conditions.into_iter().enumerate() {
        state.held[k] = if holds { state.held[k] + dt } else { 0.0 };
        if holds && state.held[k] >= time {
            return Some(reason);
        }
    }
    None
}

/// A dewired pole: up freely, a pause, then the retriever pulls it down to its hook.
fn dewired(
    state: &mut PoleState,
    p: &PoleParams,
    geo: &PoleGeometry,
    events: &mut Vec<PoleEvent>,
    i: usize,
) -> PoleOutput {
    let (yaw0, pitch0) = state.lowered_from;
    // (the highest the pole goes, in output degrees from the modelled pose)
    let top = ((p.max_pitch - elevation(geo.rest_shoe - geo.base)) * p.pitch_factor).max(pitch0);
    let t = state.timer;
    let risen = (pitch0 + p.free_rise_rate * t.min(p.catcher_delay)).min(top);
    let pitch = if t <= p.catcher_delay {
        risen
    } else {
        risen - p.catcher_lower_rate * (t - p.catcher_delay)
    };
    if pitch <= 0.0 {
        *state = PoleState::default();
        events.push(PoleEvent::Lowered { pole: i });
        return hooked(geo);
    }
    // (the pole swings back in line as the retriever brings it down)
    let yaw = if t <= p.catcher_delay || risen <= 0.0 {
        yaw0
    } else {
        yaw0 * pitch / risen
    };
    PoleOutput {
        yaw,
        pitch,
        shoe_pos: geo.rest_shoe,
        dewired: true,
        ..Default::default()
    }
}

/// A pole the retract control takes down: halfway in half the lowering time, a pause as
/// long as the retriever's, then the rest of the way.
fn retracting(
    state: &mut PoleState,
    p: &PoleParams,
    geo: &PoleGeometry,
    events: &mut Vec<PoleEvent>,
    i: usize,
) -> PoleOutput {
    let half = p.lower_duration.max(1e-3) / 2.0;
    let t = state.timer;
    let k = if t <= half {
        0.5 * t / half
    } else if t <= half + p.catcher_delay {
        0.5
    } else {
        (0.5 + 0.5 * (t - half - p.catcher_delay) / half).min(1.0)
    };
    if k >= 1.0 {
        *state = PoleState::default();
        events.push(PoleEvent::Lowered { pole: i });
        return hooked(geo);
    }
    PoleOutput {
        yaw: state.lowered_from.0 * (1.0 - k),
        pitch: state.lowered_from.1 * (1.0 - k),
        shoe_pos: geo.rest_shoe,
        ..Default::default()
    }
}

/// A rising pole's wire: of the wires near the modelled shoe that the pole can reach, the
/// one that lies on the pole's side of its pair, nearest sideways (body frame) to where the
/// model draws the shoe; None when none is within `capture` metres of it.
fn capture(
    net: &WireNetwork,
    body: &BodyPose,
    geo: &PoleGeometry,
    left_pole: bool,
    foot_to_wire: DVec3,
    len: f64,
    capture: f64,
) -> Option<(WireCursor, f64)> {
    let to_body = body.rotation.transpose();
    let in_body = |w: DVec3| to_body * (w - body.position);
    let shoe = body.position + body.rotation * geo.rest_shoe;
    walk::crossings_near(net, shoe, SEARCH_RADIUS, foot_to_wire, len)
        .into_iter()
        .filter(|(c, p)| {
            // behind the foot, as a trailing pole stands
            let b = in_body(*p);
            if b.y > geo.base.y {
                return false;
            }
            // the other wire of the same span at the same piece: the left pole takes the
            // left one of the pair, the right pole the right one
            let other = WireCursor {
                side: other_side(c.side),
                ..*c
            };
            match walk::point(net, &other) {
                Some(o) => (in_body(o).x > b.x) == left_pole,
                None => true,
            }
        })
        .map(|(c, p)| (c, (in_body(p).x - geo.rest_shoe.x).abs()))
        .filter(|(_, sideways)| *sideways <= capture)
        .min_by(|a, b| a.1.total_cmp(&b.1))
}

fn other_side(s: WireSide) -> WireSide {
    match s {
        WireSide::Left => WireSide::Right,
        WireSide::Right => WireSide::Left,
    }
}

/// A pole on its hook: the modelled pose.
fn hooked(geo: &PoleGeometry) -> PoleOutput {
    PoleOutput {
        shoe_pos: geo.rest_shoe,
        ..Default::default()
    }
}

fn lowering(
    state: &mut PoleState,
    p: &PoleParams,
    geo: &PoleGeometry,
    events: &mut Vec<PoleEvent>,
    i: usize,
) -> PoleOutput {
    let k = (state.timer / p.lower_duration.max(1e-3)).clamp(0.0, 1.0);
    if k >= 1.0 {
        *state = PoleState::default();
        events.push(PoleEvent::Lowered { pole: i });
        return hooked(geo);
    }
    PoleOutput {
        yaw: state.lowered_from.0 * (1.0 - k),
        pitch: state.lowered_from.1 * (1.0 - k),
        shoe_pos: geo.rest_shoe,
        ..Default::default()
    }
}

/// The outputs of pole `i` whose end is at `end` (world) on a wire running along `wire_dir`.
fn on_wire(
    decl: &TrolleyDeclaration,
    i: usize,
    body: &BodyPose,
    end: DVec3,
    wire_dir: DVec3,
) -> (PoleOutput, Contact) {
    let geo = decl.poles[i];
    let p = decl.params;
    let to_body = body.rotation.transpose();
    let shoe = to_body * (end - body.position);
    let v = shoe - geo.base;
    let r = geo.rest_shoe - geo.base;
    let (az_v, el_v) = (azimuth(v), elevation(v));
    let (az_r, el_r) = (azimuth(r), elevation(r));
    let w = to_body * wire_dir;
    // (a wire has no front: the shoe turns the shorter way)
    let mut turn = azimuth(w) - az_v;
    while turn > 90.0 {
        turn -= 180.0;
    }
    while turn < -90.0 {
        turn += 180.0;
    }
    let force = (p.spring_nominal_force
        + p.spring_gain_per_deg * (el_v - p.spring_reference_elevation))
        .clamp(0.0, p.contact_force_max);
    (
        PoleOutput {
            yaw: -(az_v - az_r) * p.yaw_factor[i],
            pitch: (el_v - el_r) * p.pitch_factor,
            shoe_rot: -turn * p.shoe_factor[i],
            shoe_pos: shoe,
            force,
            on_wire: true,
            dewired: false,
        },
        Contact {
            side_angle: az_v,
            turn,
            wire_ended: false,
        },
    )
}

/// Degrees about the vertical from straight back (0) towards the right (+).
fn azimuth(v: DVec3) -> f64 {
    v.x.atan2(-v.y).to_degrees()
}

/// Degrees above the horizontal.
fn elevation(v: DVec3) -> f64 {
    v.z.atan2(v.x.hypot(v.y)).to_degrees()
}

/// Metres along a wire to the cursor.
fn progress(pts: &[DVec3], c: &WireCursor) -> f64 {
    let before: f64 = pts
        .windows(2)
        .take(c.seg)
        .map(|w| w[0].distance(w[1]))
        .sum();
    before
        + pts
            .get(c.seg)
            .zip(pts.get(c.seg + 1))
            .map_or(0.0, |(a, b)| a.distance(*b) * c.t)
}
