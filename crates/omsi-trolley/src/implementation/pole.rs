//! The poles of the player's trolleybus, step by step: up from the hooks onto the wire,
//! along it, and down again.

use glam::DVec3;

use super::walk::{self, Reached};
use crate::domain::declaration::{PoleGeometry, TrolleyDeclaration};
use crate::domain::network::WireNetwork;
use crate::domain::pole::{
    BodyPose, PoleEvent, PoleInputs, PoleOutput, PoleOutputs, PolePhase, PoleSim, PoleState,
    WireCursor,
};

/// How far around the modelled shoe a rising pole looks for wire, metres.
const SEARCH_RADIUS: f64 = 2.5;

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
        let mut out = PoleOutputs {
            spline_id: -1,
            ..Default::default()
        };
        for i in 0..2 {
            let pressed = inputs.grab[i] && !self.grab_before[i];
            self.grab_before[i] = inputs.grab[i];
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
        let up = body.rotation * DVec3::Z;
        let foot = body.position + body.rotation * geo.base;
        // (the pole's end stands `shoe_height_offset` off the wire it presses on)
        let foot_to_wire = foot - up * p.shoe_height_offset;
        let len = geo.length();
        let state = &mut self.poles[i];
        state.timer += dt;
        match state.phase {
            PolePhase::Hooked => {
                if pressed {
                    match capture(
                        net,
                        body,
                        &geo,
                        i == 0,
                        foot_to_wire,
                        len,
                        p.capture_distance,
                    ) {
                        Some((cursor, sideways)) => {
                            let id = net.span(cursor.span).map_or(-1, |s| s.spline_id);
                            self.events.push(PoleEvent::Captured {
                                pole: i,
                                spline_id: id,
                                sideways,
                            });
                            *state = PoleState {
                                phase: PolePhase::Rising,
                                cursor: Some(cursor),
                                ..Default::default()
                            };
                        }
                        None => self.events.push(PoleEvent::NoWire { pole: i }),
                    }
                }
                if state.phase == PolePhase::Hooked {
                    return hooked(geo, body);
                }
            }
            PolePhase::OnWire if pressed => {
                let now = self.track(i, decl, net, body, foot_to_wire);
                let state = &mut self.poles[i];
                *state = PoleState {
                    phase: PolePhase::Lowering,
                    lowered_from: now.map_or((0.0, 0.0), |o| (o.yaw, o.pitch)),
                    ..Default::default()
                };
                return lowering(state, decl, &geo, body, &mut self.events, i);
            }
            PolePhase::Lowering => {
                return lowering(&mut self.poles[i], decl, &geo, body, &mut self.events, i);
            }
            _ => {}
        }
        // rising or on the wire: the shoe follows the wire
        let Some(mut o) = self.track(i, decl, net, body, foot_to_wire) else {
            self.poles[i] = PoleState::default();
            return hooked(geo, body);
        };
        let state = &mut self.poles[i];
        if state.phase == PolePhase::Rising {
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
    ) -> Option<PoleOutput> {
        let geo = decl.poles[i];
        let cursor = self.poles[i].cursor?;
        let Reached {
            cursor,
            point,
            error,
            joints,
            wire_ended,
        } = walk::reach(net, cursor, foot_to_wire, geo.length())?;
        for (from, to, gap) in joints {
            self.events.push(PoleEvent::Joint {
                pole: i,
                from,
                to,
                gap,
            });
        }
        if wire_ended {
            let id = net.span(cursor.span).map_or(-1, |s| s.spline_id);
            self.events.push(PoleEvent::WireEnds {
                pole: i,
                spline_id: id,
            });
        }
        let state = &mut self.poles[i];
        state.cursor = Some(cursor);
        state.reach_error = error;
        let up = body.rotation * DVec3::Z;
        let end = point + up * decl.params.shoe_height_offset;
        let wire_dir = walk::tangent(net, &cursor).unwrap_or(DVec3::Y);
        Some(on_wire(decl, i, body, end, wire_dir))
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

fn other_side(s: crate::domain::network::WireSide) -> crate::domain::network::WireSide {
    use crate::domain::network::WireSide::*;
    match s {
        Left => Right,
        Right => Left,
    }
}

/// A pole on its hook: the modelled pose.
fn hooked(geo: PoleGeometry, _body: &BodyPose) -> PoleOutput {
    PoleOutput {
        shoe_pos: geo.rest_shoe,
        ..Default::default()
    }
}

fn lowering(
    state: &mut PoleState,
    decl: &TrolleyDeclaration,
    geo: &PoleGeometry,
    body: &BodyPose,
    events: &mut Vec<PoleEvent>,
    i: usize,
) -> PoleOutput {
    let k = (state.timer / decl.params.lower_duration.max(1e-3)).clamp(0.0, 1.0);
    if k >= 1.0 {
        *state = PoleState::default();
        events.push(PoleEvent::Lowered { pole: i });
        return hooked(*geo, body);
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
) -> PoleOutput {
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
    PoleOutput {
        yaw: -(az_v - az_r) * p.yaw_factor[i],
        pitch: (el_v - el_r) * p.pitch_factor,
        shoe_rot: -turn * p.shoe_factor[i],
        shoe_pos: shoe,
        force,
        on_wire: true,
    }
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
