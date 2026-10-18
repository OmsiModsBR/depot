//! Changing lanes: the move over to a neighbour, pulling out round something standing, a
//! route's lane change, and the short sequence of lanes the way runs along.

use glam::{DVec2, DVec3};
use omsi_geometry::util::smoothstep;

use super::{AiState, JOINT_BLEND, PLAN_LANES, SIGNAL_BEFORE_CHANGE};
use crate::traffic::network::Network;

/// A lane change: the car moves over from its lane to `to` along `length` metres of road
/// (by distance, not by time: a car that has to stop halfway stands still, and so does its
/// way).
#[derive(Debug, Clone, Copy)]
pub struct LaneChange {
    pub to: usize,
    /// Progress 0..1.
    pub t: f32,
    pub length: f32,
    /// Position along `to`.
    pub s_to: f32,
    /// 1 left, 2 right.
    pub dir: i32,
    /// Seconds of indicating left before the car starts to move over.
    pub wait: f32,
    /// Pulling out from behind something standing: the car moves off sideways from a
    /// standstill, and what it leaves behind in its lane no longer holds it.
    pub bypass: bool,
}

/// A short sequence of lanes the way runs along, with the lane the car is on at `cur`.
pub(super) struct LaneSeq {
    pub(super) lanes: [usize; PLAN_LANES + 2],
    pub(super) n: usize,
    pub(super) cur: usize,
}

impl LaneSeq {
    /// Gap from the end of `lanes[i]` to the start of `lanes[i + 1]`, if they are linked
    /// (a gap of a few metres means they are not one road, and nothing is bent).
    pub(super) fn gap(&self, net: &Network, i: usize) -> Option<DVec3> {
        let (a, b) = (&net.lanes[self.lanes[i]], &net.lanes[self.lanes[i + 1]]);
        let g = b.start() - a.end();
        (g.length() < 3.0).then_some(g)
    }

    /// Point and heading `u` metres into `lanes[i]` (beyond the ends of the first and last
    /// lane: straight on), with the joints on both sides smoothed.
    pub(super) fn at(&self, net: &Network, i: usize, u: f32) -> (DVec3, f32) {
        let l = &net.lanes[self.lanes[i]];
        let (mut p, h) = l.at_ext(u);
        let w = |x: f32| 1.0 - smoothstep(0.0, 1.0, x / JOINT_BLEND);
        if i > 0 {
            if let Some(g) = self.gap(net, i - 1) {
                p -= g * (0.5 * w(u.max(0.0))) as f64;
            }
        }
        if i + 1 < self.n {
            if let Some(g) = self.gap(net, i) {
                p += g * (0.5 * w((l.length() - u).max(0.0))) as f64;
            }
        }
        (p, h)
    }

    /// Lane index in `lanes` and distance along it, `d` metres from distance `s` of the
    /// current lane.
    pub(super) fn locate(&self, net: &Network, s: f32, d: f32) -> (usize, f32) {
        let mut i = self.cur;
        let mut u = s + d;
        while u < 0.0 && i > 0 {
            i -= 1;
            u += net.lanes[self.lanes[i]].length();
        }
        while i + 1 < self.n && u > net.lanes[self.lanes[i]].length() {
            u -= net.lanes[self.lanes[i]].length();
            i += 1;
        }
        (i, u)
    }

    /// Point and heading `d` metres from distance `s` of the current lane.
    pub(super) fn point(&self, net: &Network, s: f32, d: f32) -> (DVec3, f32) {
        let (i, u) = self.locate(net, s, d);
        self.at(net, i, u)
    }
}

impl AiState {
    /// Begin a lane change onto the neighbour `to` (`dir` 1 left, 2 right): the indicator
    /// goes on at once, the car moves over after a moment.
    pub fn start_change(&mut self, net: &Network, to: usize, dir: i32) {
        let Some(l) = net.lanes.get(self.lane) else {
            return;
        };
        let Some(t) = net.lanes.get(to) else { return };
        // two to three and a half seconds at the speed the car has now, at least 12 m
        let duration = (3.5 - self.speed * 0.05).clamp(2.0, 3.5);
        let length = (self.speed * duration).max(12.0);
        // (the same share of a lane that starts beside this one, else the point beside the car)
        let s_to = net.beside_s(self.lane, to, self.s.min(l.length()));
        let rest = t.length() - s_to;
        self.change = Some(LaneChange {
            to,
            t: 0.0,
            length,
            s_to,
            dir,
            wait: SIGNAL_BEFORE_CHANGE,
            bypass: false,
        });
        self.blinker = dir;
        let mut plan = Vec::new();
        if self.route.is_empty() {
            self.extend_plan(net, to, rest, &mut plan);
        } else {
            plan.extend(
                self.route
                    .iter()
                    .skip(self.route_index + 2)
                    .take(PLAN_LANES)
                    .copied(),
            );
        }
        self.change_plan = plan;
    }

    /// A timetable route that moves over to the lane beside the current one next: that lane
    /// and the side it lies on (1 left, 2 right). The driver starts the move as soon as the
    /// lane is free (`start_route_change`).
    pub fn route_change_due(&self, net: &Network) -> Option<(usize, i32)> {
        if self.route.is_empty() || self.change.is_some() {
            return None;
        }
        let &b = self.route.get(self.route_index + 1)?;
        if !net.parallel(self.lane, b) {
            return None;
        }
        // a lane beside that begins further on: not before the car is level with its start
        let delta = net.beside_delta(self.lane, b);
        if delta < 0.0 && self.s < -delta + 1.0 {
            return None;
        }
        let (la, lb) = (&net.lanes[self.lane], &net.lanes[b]);
        // (compared where the car is: a lane beside may start well before or after this one)
        let (pa, ha) = la.at(self.s.min(la.length()));
        let pb = lb.at(net.beside_s(self.lane, b, self.s)).0;
        let h = (ha as f64).to_radians();
        let side = (pb - pa).truncate().dot(DVec2::new(h.cos(), -h.sin()));
        Some((
            b,
            if side > 0.0 || (side.abs() < 0.5 && lb.offset > la.offset) {
                2
            } else {
                1
            },
        ))
    }

    /// Pull out into `to` from behind a standing obstacle: a short, steep move.
    pub fn start_bypass(&mut self, net: &Network, to: usize, dir: i32) {
        self.start_change(net, to, dir);
        if let Some(c) = self.change.as_mut() {
            c.length = 8.0;
            c.bypass = true;
        }
    }

    /// Start the lane change a route asks for: at once (the indicator has been on while the
    /// driver waited for a gap) and quick enough to be done well before a fork's branches part.
    pub fn start_route_change(&mut self, net: &Network, to: usize, dir: i32) {
        self.start_change(net, to, dir);
        let left = (net.lanes[self.lane].length() - self.s).max(0.0);
        if let Some(c) = self.change.as_mut() {
            c.wait = 0.0;
            c.length = c.length.min(left * 0.7).max(6.0);
        }
    }

    /// The lane sequence the car is driving along: where it came from, where it is, the plan.
    pub(super) fn seq(&self) -> LaneSeq {
        let mut q = LaneSeq {
            lanes: [0; PLAN_LANES + 2],
            n: 0,
            cur: 0,
        };
        if let Some(p) = self.prev_lane {
            q.lanes[0] = p;
            q.n = 1;
        }
        q.cur = q.n;
        q.lanes[q.n] = self.lane;
        q.n += 1;
        for l in self.upcoming() {
            if q.n == q.lanes.len() {
                break;
            }
            q.lanes[q.n] = l;
            q.n += 1;
        }
        q
    }

    /// The lane sequence of the lane the car is changing to, with the way on it chose.
    pub(super) fn change_seq(&self, to: usize) -> LaneSeq {
        let mut q = LaneSeq {
            lanes: [0; PLAN_LANES + 2],
            n: 1,
            cur: 0,
        };
        q.lanes[0] = to;
        for &l in &self.change_plan {
            if q.n == q.lanes.len() {
                break;
            }
            q.lanes[q.n] = l;
            q.n += 1;
        }
        q
    }
}
