//! Which way the driver goes: the choice at the end of a lane, the plan of lanes ahead and a
//! timetable vehicle's route.

use super::{AiState, PLAN_AHEAD, PLAN_LANES};
use crate::traffic::lane::LaneKind;
use crate::traffic::network::{Network, DEAD_END};

impl AiState {
    /// A random way on from the end of `lane`. Lanes the map closes to this vehicle ([rule]
    /// no_cars, bus, trucks: `Lane::allows`) and lanes whose traffic density is zero are not driven
    /// into - filtering them only at spawn still let cars turn into a pedestrian street or a
    /// depot yard from next door. A car that has taken a turn lane takes the turn.
    pub(crate) fn choose_after(&mut self, net: &Network, lane: usize) -> Option<usize> {
        let l = &net.lanes[lane];
        // (a car of a traffic pool - the trucks of a map that keeps them to its port roads -
        // takes only the ways its pool may go, as it was put on one. A forbidden exit
        // remains forbidden even if every exit is closed; normal end-of-route handling
        // must deal with that, rather than driving into a pedestrian or restricted lane.)
        let open_to = |pooled: bool| -> Vec<usize> {
            l.next
                .iter()
                .copied()
                .filter(|&n| {
                    let nl = &net.lanes[n];
                    let d = match self.traffic_pool.as_ref().filter(|_| pooled) {
                        Some((p, defaults)) => nl.pool_density(defaults, *p),
                        None => nl.density,
                    };
                    nl.allows(self.veh_type) && d > 0.0
                })
                .collect()
        };
        let pooled = self.traffic_pool.is_some().then(|| open_to(true));
        let weighted = pooled.is_some();
        let open = pooled.unwrap_or_else(|| open_to(false));
        let mut choices = open;
        // and a way that goes on rather than into the end of the network, where there is
        // the choice (the map's edge is where OMSI takes its cars away; a village like
        // Grundorf had a queue of twenty growing at the end of its one outbound road)
        if l.kind == LaneKind::Street && net.reach.len() == net.lanes.len() {
            let through: Vec<usize> = choices
                .iter()
                .copied()
                .filter(|&n| net.reach[n] >= DEAD_END)
                .collect();
            if !through.is_empty() {
                choices = through;
            }
        }
        if self.turn_wish != 0 {
            let wished: Vec<usize> = choices
                .iter()
                .copied()
                .filter(|&n| net.lanes[n].turn == self.turn_wish)
                .collect();
            if !wished.is_empty() {
                choices = wished;
                self.turn_wish = 0;
            }
        }
        if choices.is_empty() {
            None
        } else {
            if let Some((pool, defaults)) = self.traffic_pool.clone().filter(|_| weighted) {
                let total: f32 = choices
                    .iter()
                    .map(|&n| net.lanes[n].pool_density(&defaults, pool))
                    .sum();
                let mut pick = (self.rand() >> 32) as f32 / (u32::MAX as f32 + 1.0) * total;
                for &n in &choices {
                    let weight = net.lanes[n].pool_density(&defaults, pool);
                    if pick < weight {
                        return Some(n);
                    }
                    pick -= weight;
                }
                choices.last().copied()
            } else {
                Some(choices[(self.rand() % choices.len() as u64) as usize])
            }
        }
    }

    /// Decide the way on: the lane after the current one and enough lanes after it to
    /// cover `PLAN_AHEAD` metres (a timetable route simply is that plan).
    pub fn plan_next(&mut self, net: &Network) {
        if !self.route.is_empty() {
            // a route step onto a lane beside this one is a lane change, not the way on
            let mut k = self.route_index + 1;
            if self
                .route
                .get(k)
                .map(|&b| net.parallel(self.lane, b))
                .unwrap_or(false)
            {
                k += 1;
            }
            self.planned_next = self.route.get(k).copied();
            self.ahead = self
                .route
                .iter()
                .skip(k + 1)
                .take(PLAN_LANES)
                .copied()
                .collect();
            return;
        }
        if self.planned_next.is_none() {
            self.ahead.clear();
        }
        let planned: f32 = self.upcoming().map(|l| net.lanes[l].length()).sum();
        let rest = net.lanes[self.lane].length() - self.s;
        if self.planned_next.is_some()
            && (rest + planned >= PLAN_AHEAD || self.ahead.len() >= PLAN_LANES)
        {
            return;
        }
        let mut plan: Vec<usize> = self.upcoming().collect();
        self.extend_plan(net, self.lane, rest, &mut plan);
        self.planned_next = plan.first().copied();
        self.ahead = plan.into_iter().skip(1).collect();
    }

    /// Choose lanes after `from` (which has `rest` metres left) until `plan` covers
    /// `PLAN_AHEAD` metres.
    pub(super) fn extend_plan(
        &mut self,
        net: &Network,
        from: usize,
        rest: f32,
        plan: &mut Vec<usize>,
    ) {
        let mut dist = rest + plan.iter().map(|&l| net.lanes[l].length()).sum::<f32>();
        while dist < PLAN_AHEAD && plan.len() <= PLAN_LANES {
            let last = plan.last().copied().unwrap_or(from);
            match self.choose_after(net, last) {
                Some(n) => {
                    dist += net.lanes[n].length();
                    plan.push(n);
                }
                None => break,
            }
        }
    }

    /// Put the car on a fixed route starting at its first lane.
    pub fn set_route(&mut self, net: &Network, route: Vec<usize>, s: f32) {
        self.route = route;
        self.route_index = 0;
        if let Some(&l) = self.route.first() {
            self.lane = l;
            self.s = s;
        }
        self.prev_lane = None;
        self.plan_next(net);
    }

    /// How far the car still has to drive to distance `ss` of route lane `ri` (a lane it
    /// changes over from counts as the lane beside it).
    pub fn route_distance(&self, net: &Network, ri: usize, ss: f32) -> f32 {
        let (mut k, mut d) = (self.route_index, -self.s);
        if let Some(c) = self.change {
            if self.route.get(k + 1) == Some(&c.to) {
                k += 1;
                d = -c.s_to;
            }
        }
        while k < ri && k + 1 < self.route.len() {
            if !net.parallel(self.route[k], self.route[k + 1]) {
                d += net.lanes[self.route[k]].length();
            } else {
                // a lane beside that starts earlier is further along at the same place
                d -= net.beside_delta(self.route[k], self.route[k + 1]);
            }
            k += 1;
        }
        d + ss
    }
}
