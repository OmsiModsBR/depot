//! Driving on: the point the steering aims at, the bay offset, the indicator, and each
//! step along the lanes.

use glam::DVec3;

use super::{AiState, Lead};
use crate::traffic::geometry::wrap_deg;
use crate::traffic::lane::LaneKind;
use crate::traffic::network::Network;
use omsi_geometry::util::smoothstep;

impl AiState {
    /// The point of the car's way `d` metres ahead of it (behind for negative `d`): along
    /// its lanes with the joints smoothed, over to the new lane during a lane change, and
    /// out to the side where it pulls into a bay or round a parked car. The steering
    /// follows this curve; it has no steps, so neither does the car.
    pub fn way_point(&self, net: &Network, d: f32) -> DVec3 {
        let (mut p, mut h) = self.seq().point(net, self.s, d);
        if let Some(c) = self.change {
            if c.to < net.lanes.len() {
                let (pt, ht) = self.change_seq(c.to).point(net, c.s_to, d);
                // how far the change will have got by the time the car is `d` metres on
                let tau = (d - c.wait * self.speed.max(2.0)) / c.length;
                let k = smoothstep(0.0, 1.0, c.t + tau);
                p = p.lerp(pt, k as f64);
                h += wrap_deg(ht - h) * k;
            }
        }
        let lat = self.lateral_at(self.odometer + d);
        if lat.abs() > 1e-3 {
            let hr = (h as f64).to_radians();
            p += DVec3::new(hr.cos(), -hr.sin(), 0.0) * lat as f64;
        }
        p
    }

    /// The sideways offset of the way `d` metres ahead of the car (m, + = right).
    pub fn lateral_ahead(&self, d: f32) -> f32 {
        self.lateral_at(self.odometer + d)
    }

    /// The sideways offset of the way when the odometer reads `x`.
    fn lateral_at(&self, x: f32) -> f32 {
        let (from, to, x0, len) = self.lateral_ramp;
        from + (to - from) * smoothstep(0.0, 1.0, (x - x0) / len.max(0.1))
    }

    /// The first turn (1 left, 2 right) on the way within `within` metres, with the
    /// distance to the start of the turning lane (0 while on it).
    pub fn turn_ahead(&self, net: &Network, within: f32) -> Option<(i32, f32)> {
        let here = &net.lanes[self.lane];
        if here.turn != 0 {
            return Some((here.turn, 0.0));
        }
        let mut dist = here.length() - self.s;
        for l in self.upcoming() {
            if dist > within {
                break;
            }
            let lane = &net.lanes[l];
            if lane.turn != 0 {
                return Some((lane.turn, dist));
            }
            dist += lane.length();
        }
        None
    }

    /// Set the indicator for what the car is doing or about to do: a lane change, its own
    /// manoeuvre (pulling away from a stop), moving sideways into a bay or out round a
    /// parked car and back, and a turn at the junction ahead - from about four seconds
    /// before it (at least 25 m, at most 60 m) until the turn is done.
    pub fn update_blinker(&mut self, net: &Network) {
        self.blinker = if let Some(c) = self.change {
            c.dir
        } else if self.signal != 0 && self.signal_time > 0.0 {
            self.signal
        } else if (self.lateral_target - self.lateral).abs() > 0.3 {
            if self.lateral_target > self.lateral {
                2
            } else {
                1
            }
        } else {
            let within = (self.speed * 4.0).clamp(25.0, 60.0);
            self.turn_ahead(net, within).map(|t| t.0).unwrap_or(0)
        };
    }

    /// Advance along the network. `obstacle` = distance from the car's origin to the rear of
    /// a standing vehicle ahead (m), `stop_at` = distance from its origin to a stop line
    /// (m). False at a dead end (the car is taken off the road).
    pub fn advance(
        &mut self,
        net: &Network,
        dt: f32,
        obstacle: Option<f32>,
        stop_at: Option<f32>,
    ) -> bool {
        let lead = obstacle.map(|d| Lead {
            gap: d - self.front,
            speed: 0.0,
            acc: 0.0,
        });
        self.drive(net, dt, lead, stop_at)
    }

    /// Advance along the network with the car ahead (`lead`) and a stop point (`stop`,
    /// distance from the car's origin). False at a dead end.
    pub fn drive(&mut self, net: &Network, dt: f32, lead: Option<Lead>, stop: Option<f32>) -> bool {
        self.change_cooldown = (self.change_cooldown - dt).max(0.0);
        self.signal_time = (self.signal_time - dt).max(0.0);
        if self.signal_time <= 0.0 {
            self.signal = 0;
        }
        if net.lanes.get(self.lane).is_none() {
            return false;
        }
        let mut acc = self.desired_accel(net, lead, stop);
        if let Some(cap) = self.accel_cap {
            acc = acc.min(cap);
        }
        // standing: a driver who is held there moves off only after a moment when the way
        // clears (the wave that runs down a queue at a green light)
        if self.speed < 0.05 {
            if acc <= 0.05 {
                // (a hold of a frame or two - a junction that is free and not free by turns
                // as the cars on the ring come and go - winds the reaction back only a
                // little: set back whole every frame, it never ran out, and the car stood at
                // an empty roundabout for minutes, "about to go")
                self.start_timer = if self.held {
                    (self.start_timer + 3.0 * dt).min(self.reaction)
                } else {
                    self.reaction
                };
                self.held = true;
                acc = acc.min(0.0);
            } else if self.held {
                self.start_timer -= dt;
                if self.start_timer > 0.0 {
                    acc = 0.0;
                } else {
                    self.held = false;
                }
            }
        } else if self.speed > 0.5 {
            self.held = false;
        }
        let v0 = self.speed;
        let v1 = (v0 + acc * dt).max(0.0);
        self.speed = v1;
        self.acc = if dt > 0.0 { (v1 - v0) / dt } else { 0.0 };
        // brake lights: braking, or holding the car on the brake
        self.braking = acc < -0.6 || (v1 < 0.3 && self.held);
        let ds = (v0 + v1) * 0.5 * dt;
        self.s += ds;
        self.odometer += ds;
        // lane change: signal, glide over to the neighbour, then continue there
        if let Some(mut c) = self.change {
            c.s_to += ds;
            if c.wait > 0.0 {
                c.wait = (c.wait - dt).max(0.0);
            } else {
                c.t += ds / c.length;
            }
            let from_len = net.lanes[self.lane].length();
            let mut to_len = net.lanes.get(c.to).map(|l| l.length()).unwrap_or(0.0);
            // A move that is not over where a lane ends carries on across the joint: the
            // lane beside goes on into the way chosen on it, and this lane into its plan
            // (below). Finished at the joint instead, the car jumped sideways by what was
            // left of the move - so a lane change was only started 20 m and more before a
            // lane's end, and on a road of short spline pieces a car stopped behind a parked
            // car just past a joint never got round it. (A timetable vehicle's route moves
            // over well before the end, as before.)
            let carry_on = self.route.is_empty();
            while carry_on && c.t < 1.0 && c.s_to >= to_len && !self.change_plan.is_empty() {
                c.s_to -= to_len;
                c.to = self.change_plan.remove(0);
                to_len = net.lanes.get(c.to).map(|l| l.length()).unwrap_or(0.0);
            }
            let from_ends = self.s >= from_len && (self.planned_next.is_none() || !carry_on);
            if c.t >= 1.0 || from_ends || c.s_to >= to_len {
                // arrived on the new lane: it has no joint behind the car, and the way on
                // is planned afresh from there
                if self.route.get(self.route_index + 1) == Some(&c.to) {
                    self.route_index += 1;
                }
                self.lane = c.to;
                self.s = c.s_to.min(to_len);
                self.change = None;
                self.change_cooldown = 6.0;
                self.prev_lane = None;
                let plan = std::mem::take(&mut self.change_plan);
                self.planned_next = plan.first().copied();
                self.ahead = plan.into_iter().skip(1).collect();
            } else {
                self.change = Some(c);
            }
        }
        // bay offset: a new target starts an S-curve from where the car is now, eight metres
        // long for every metre sideways (at least 8, at most 30)
        if (self.lateral_target - self.lateral_ramp.1).abs() > 1e-3 {
            let from = self.lateral_at(self.odometer - ds);
            let len = ((self.lateral_target - from).abs() * 8.0).clamp(8.0, 30.0);
            self.lateral_ramp = (from, self.lateral_target, self.odometer - ds, len);
        }
        self.lateral = self.lateral_at(self.odometer);
        if self.planned_next.is_none() {
            self.plan_next(net);
        }
        while (self.change.is_none() || self.route.is_empty())
            && self.s >= net.lanes[self.lane].length()
        {
            let l = &net.lanes[self.lane];
            let Some(next) = self.planned_next else {
                // the end of a flight path: the aircraft flies on straight (the way runs on
                // along the end tangent) until the traffic takes it away out of sight
                if l.kind == LaneKind::Air {
                    break;
                }
                return false; // dead end: despawn
            };
            self.s -= l.length();
            self.prev_lane = Some(self.lane);
            self.lane = next;
            if self.route.is_empty() {
                self.planned_next = if self.ahead.is_empty() {
                    None
                } else {
                    Some(self.ahead.remove(0))
                };
            } else {
                // the plan may have stepped over a lane-change entry of the route
                let from = self.route_index + 1;
                self.route_index = (from..(from + 3).min(self.route.len()))
                    .find(|&k| self.route[k] == next)
                    .unwrap_or(from);
            }
            self.plan_next(net);
        }
        // keep the plan PLAN_AHEAD metres long as the car eats into it (a route's plan
        // only moves when the car enters its next lane)
        if self.route.is_empty() {
            self.plan_next(net);
        }
        true
    }
}
