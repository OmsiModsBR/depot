//! How fast the driver wants to go: the speed profile through bends, the Intelligent Driver
//! Model towards the car ahead and a stop line, and the ramp out of a bay.

use super::lane_change::LaneSeq;
use super::{AiState, Lead};
use crate::traffic::geometry::wrap_deg;
use crate::traffic::lane::Lane;
use crate::traffic::network::Network;
use omsi_geometry::util::smoothstep;

/// Seconds a vehicle needs to cover `dist` metres from speed `v`, speeding up at `a` to at
/// most `v_max` (the earliest it can be there when nothing holds it back).
pub fn arrival_time(dist: f32, v: f32, a: f32, v_max: f32) -> f32 {
    if dist <= 0.0 {
        return 0.0;
    }
    let (v, a) = (v.max(0.0), a.max(0.05));
    let v_max = v_max.max(v).max(0.1);
    // speeding up to v_max takes (v_max - v) / a seconds and that much road
    let t_up = (v_max - v) / a;
    let d_up = (v + v_max) * 0.5 * t_up;
    if dist <= d_up {
        (-v + (v * v + 2.0 * a * dist).sqrt()) / a
    } else {
        t_up + (dist - d_up) / v_max
    }
}

/// How far along its S-curve (smoothstep, 0..1) a car moving back from the lane `side`
/// metres to the left has to be before its middle is `clear` metres from that lane's
/// middle (out of the way of the traffic there).
pub fn ramp_progress_for(side: f32, clear: f32) -> f32 {
    if clear <= 0.0 {
        return 0.0;
    }
    if side <= clear {
        return 1.0;
    }
    // the offset left is side × (1 − smoothstep(t)); it is clear once smoothstep(t) ≥ clear / side
    let want = clear / side;
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..20 {
        let mid = 0.5 * (lo + hi);
        if smoothstep(0.0, 1.0, mid) < want {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    hi
}

/// Hardest braking of an AI driver (m/s²): an emergency stop.
pub const MAX_BRAKE: f32 = 8.0;

/// Gap a car leaves before a stop line or a stop point (m).
const STOP_LINE_GAP: f32 = 0.6;
impl AiState {
    /// How fast the car may be going now so that it can take every bend of the next stretch
    /// of its way at `lat_accel`, braking gently (2 m/s²) into the tight ones. Without this
    /// the cars swept round a junction at 40 km/h - well over a g. The bend is the lane's
    /// own curvature or the turn of the way over 6 m, whichever is sharper: lanes are linked
    /// with up to 40° between them, and such a kink is a bend too.
    pub fn curve_speed(&self, net: &Network) -> f32 {
        let mut best = self.curve_speed_on(net, &self.seq(), self.s);
        if let Some(c) = self.change {
            if c.to < net.lanes.len() {
                best = best.min(self.curve_speed_on(net, &self.change_seq(c.to), c.s_to));
            }
        }
        best
    }

    fn curve_speed_on(&self, net: &Network, q: &LaneSeq, s: f32) -> f32 {
        let reach = (self.speed * self.speed / 4.0 + 12.0).min(90.0);
        let mut best = f32::MAX;
        // The samples lie at fixed places of the road (every 2.5 m of the odometer), not at
        // fixed distances ahead of the car: moving with the car, the sample that caught a
        // bend jumped 2.5 m nearer every 2.5 m, the allowed speed fell in steps of 4 m²/s²
        // and the car braked hard, let go and braked hard again with nothing in front of it.
        let first = 2.5 - self.odometer.rem_euclid(2.5);
        let mut d = 0.0f32;
        let mut next = first;
        while d <= reach {
            let (i, u) = q.locate(net, s, d);
            let turn = wrap_deg(q.point(net, s, d + 3.0).1 - q.point(net, s, d - 3.0).1)
                .abs()
                .to_radians()
                / 6.0;
            let k = net.lanes[q.lanes[i]].curvature_at(u).abs().max(turn);
            if k > 1e-4 {
                let v = (self.lat_accel / k).sqrt().max(2.5);
                // (a bend may begin anywhere up to a sample's spacing before the sample
                // that finds it: the speed is taken from there, or the car met the start of
                // a tight turn half a metre after its profile had allowed 1 m/s more)
                best = best.min((v * v + 2.0 * 2.0 * (d - 2.5).max(0.0)).sqrt());
            }
            d = next;
            next += 2.5;
        }
        best
    }

    /// The acceleration the driver wants (the Intelligent Driver Model): towards the
    /// desired speed on a free road, and a braking term for what is ahead that is gentle
    /// (`decel`) while there is room and only as hard as it must be when there is not.
    /// `lead` is the vehicle ahead, `stop` the distance from the car's origin to where it
    /// has to stop (a light, a junction it gives way at, a bus stop).
    pub fn desired_accel(&self, net: &Network, lead: Option<Lead>, stop: Option<f32>) -> f32 {
        let Some(lane) = net.lanes.get(self.lane) else {
            return 0.0;
        };
        let (a, b) = (self.accel.max(0.1), self.decel.max(0.5));
        let v = self.speed;
        let limit = |l: &Lane| {
            (l.speed_limit_kmh * self.desire)
                .min(self.max_speed_kmh)
                .max(3.0)
                / 3.6
        };
        let mut v0 = limit(lane);
        // a lower limit on the lanes ahead (a junction's turning lanes, a 30 zone) is
        // reached at that speed, slowing gently (1.2 m/s²) from where it has to: taken only
        // on entering the lane, the new limit threw the model into a -3 m/s² stop at every
        // junction, with nothing in front of the car
        {
            let reach = (v * v / 2.4 + 10.0).min(120.0);
            let mut d = lane.length() - self.s;
            for l in self.upcoming() {
                if d > reach {
                    break;
                }
                if let Some(nl) = net.lanes.get(l) {
                    let vl = limit(nl);
                    if vl < v0 {
                        v0 = v0.min((vl * vl + 2.0 * 1.2 * d.max(0.0)).sqrt());
                    }
                    d += nl.length();
                }
            }
        }
        let mut acc = a * (1.0 - (v / v0).powi(4)).max(-1.5 * b / a);
        // bends: follow the speed profile `curve_speed` lays out (it assumes 2 m/s² of
        // braking), blending in over the last metre per second above it
        let bend = self.curve_speed(net);
        if bend < v0 && v > bend - 1.0 {
            let track = -2.0 + (bend - v) / 0.6;
            let k = ((v - (bend - 1.0)) / 1.0).clamp(0.0, 1.0);
            acc = acc.min(acc + (track - acc) * k);
        }
        let interaction = |gap: f32, lead_speed: f32, s0: f32, headway: f32| -> f32 {
            let s_star =
                s0 + (v * headway + v * (v - lead_speed) / (2.0 * (a * b).sqrt())).max(0.0);
            -a * (s_star / gap.max(0.05)).powi(2)
        };
        let mut out = acc;
        if let Some(l) = lead {
            let idm = acc + interaction(l.gap, l.speed.max(0.0), self.min_gap, self.headway);
            // The constant-acceleration heuristic (the "ACC" variant of the model): how hard
            // the driver has to brake if the car ahead keeps doing what it does. A car that
            // has just cut in close ahead but drives almost as fast calls for a firm brake
            // and a gap that opens again over the next seconds, not an emergency stop.
            let (vl, s, al) = (l.speed.max(0.0), l.gap.max(0.05), l.acc.min(a));
            let den = vl * vl - 2.0 * s * al;
            let cah = if vl * (v - vl) <= -2.0 * s * al && den > 1e-3 {
                v * v * al / den
            } else {
                al - (v - vl).max(0.0).powi(2) / (2.0 * s)
            };
            // (a car beside or just ahead that drives away faster gives the model a gap of
            // nothing: its braking term is bounded so that the heuristic decides)
            let idm = idm.max(-2.0 * MAX_BRAKE);
            let acc_lead = if idm >= cah {
                idm
            } else {
                0.01 * idm + 0.99 * (cah + b * ((idm - cah) / b).tanh())
            };
            out = out.min(acc_lead);
        }
        if let Some(d) = stop {
            // A stop line: far away the model's braking term (without a time gap, the line
            // does not move off), gently; once the constant deceleration that stops the car
            // at the line reaches half the driver's comfortable braking, that deceleration.
            // The model alone braked twice as hard as needed when a light turned yellow 25 m
            // ahead, and then eased off.
            let room = (d - self.front - STOP_LINE_GAP).max(0.02);
            let need = v * v / (2.0 * room);
            let idm = (acc + interaction(d - self.front, 0.0, STOP_LINE_GAP, 0.2)).max(-0.5 * b);
            let constant = -need * 1.05;
            let k = ((need - 0.35 * b) / (0.15 * b)).clamp(0.0, 1.0);
            let a_stop = idm + (constant - idm) * k;
            out = out.min(a_stop);
        }
        out.clamp(-MAX_BRAKE, a)
    }
}
