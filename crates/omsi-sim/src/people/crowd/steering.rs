//! Walkers and what they keep clear of, and one step of the crowd: anticipatory avoidance
//! and the contact pass.

use glam::DVec2;
use hashbrown::HashMap;

use crate::people::crowd::corridor::ease_into_corridor;

/// One person on foot in a plane (the ground, or the floor of one bus).
#[derive(Debug, Clone, Copy)]
pub struct Walker {
    pub pos: DVec2,
    pub vel: DVec2,
    pub radius: f64,
    /// Preferred velocity this frame (where the person wants to go, how fast).
    pub want: DVec2,
    /// How far the person yields to others and to contact (0 = not at all, 1 = fully).
    /// People standing in a queue or at a stop give little; walkers give fully.
    pub give: f64,
    /// Walkers only see walkers of the same space (0 = the ground, else one bus floor).
    pub space: u64,
    /// Seen by the others but not moved (a pedestrian held on a pavement lane, somebody
    /// seated, a person carried through a door by the bus).
    pub fixed: bool,
    /// Passes through other walkers for a moment: the way out of a deadlock in a doorway
    /// or a narrow aisle (two people who have pushed against each other for seconds).
    pub ghost: bool,
    /// Keeps to within `.2` of the segment `.0`-`.1` (an aisle): avoidance may move a walker
    /// sideways, never into the seats.
    pub corridor: Option<(DVec2, DVec2, f64)>,
}

impl Walker {
    pub fn new(pos: DVec2, radius: f64, space: u64) -> Walker {
        Walker {
            pos,
            vel: DVec2::ZERO,
            radius,
            want: DVec2::ZERO,
            give: 1.0,
            space,
            fixed: false,
            ghost: false,
            corridor: None,
        }
    }
}

/// A box nobody walks through (a vehicle), on the ground.
#[derive(Debug, Clone, Copy)]
pub struct Block {
    pub center: DVec2,
    /// Half extents: x across, y along the heading.
    pub half: DVec2,
    /// Radians, clockwise from north (like vehicle headings).
    pub heading: f64,
    /// Velocity of the vehicle (m/s): people keep out of the way of a moving bus earlier.
    pub vel: DVec2,
}

impl Block {
    fn axes(&self) -> (DVec2, DVec2) {
        let (s, c) = (self.heading.sin(), self.heading.cos());
        (DVec2::new(c, -s), DVec2::new(s, c))
    }

    /// Closest point of the box's outline to `p` and whether `p` lies inside it.
    pub fn closest(&self, p: DVec2) -> (DVec2, bool) {
        let (r, f) = self.axes();
        let d = p - self.center;
        let (x, y) = (d.dot(r), d.dot(f));
        let inside = x.abs() < self.half.x && y.abs() < self.half.y;
        if inside {
            // out through the nearest side
            let (dx, dy) = (self.half.x - x.abs(), self.half.y - y.abs());
            let q = if dx < dy {
                DVec2::new(self.half.x * sign(x), y)
            } else {
                DVec2::new(x, self.half.y * sign(y))
            };
            return (self.center + r * q.x + f * q.y, true);
        }
        let q = DVec2::new(
            x.clamp(-self.half.x, self.half.x),
            y.clamp(-self.half.y, self.half.y),
        );
        (self.center + r * q.x + f * q.y, false)
    }

    /// Whether `p` lies within `margin` of the box.
    pub fn near(&self, p: DVec2, margin: f64) -> bool {
        let (r, f) = self.axes();
        let d = p - self.center;
        d.dot(r).abs() < self.half.x + margin && d.dot(f).abs() < self.half.y + margin
    }
}

fn sign(v: f64) -> f64 {
    if v < 0.0 {
        -1.0
    } else {
        1.0
    }
}

/// Tuning of the crowd model.
#[derive(Debug, Clone, Copy)]
pub struct CrowdParams {
    /// Scale of the anticipatory force.
    pub k: f64,
    /// Time horizon (s): collisions further away than about this matter little.
    pub tau0: f64,
    /// Relaxation time towards the preferred velocity (s).
    pub relax: f64,
    /// Largest acceleration (m/s²): nobody starts, stops or swerves like a robot.
    pub max_accel: f64,
    /// How far a walker looks for neighbours (m).
    pub sense: f64,
}

impl Default for CrowdParams {
    fn default() -> Self {
        CrowdParams {
            k: 1.5,
            tau0: 3.0,
            relax: 0.45,
            max_accel: 2.2,
            sense: 4.0,
        }
    }
}

impl CrowdParams {
    /// Inside a bus people are close by necessity: they look less far ahead and brush past
    /// each other instead of stopping a metre early.
    pub fn cabin() -> CrowdParams {
        CrowdParams {
            k: 0.6,
            tau0: 1.2,
            relax: 0.35,
            max_accel: 2.0,
            sense: 2.0,
        }
    }
}

const CELL: f64 = 2.0;

fn cell_of(space: u64, p: DVec2) -> (u64, i32, i32) {
    (
        space,
        (p.x / CELL).floor() as i32,
        (p.y / CELL).floor() as i32,
    )
}

/// Anticipatory avoidance force on a walker at `x` with velocity `v` from a body at
/// `xo` moving with `vo` (combined radius `r`). Zero when they would never touch.
fn ttc_force(p: &CrowdParams, x: DVec2, v: DVec2, xo: DVec2, vo: DVec2, r: f64) -> DVec2 {
    let w = xo - x;
    let dist = w.length();
    // already touching: count the contact at the current distance, so that the force
    // stays finite and still pushes apart
    let r = if dist < r { dist.max(1e-3) * 0.99 } else { r };
    let rv = v - vo;
    let a = rv.dot(rv);
    let b = w.dot(rv);
    let c = w.dot(w) - r * r;
    let discr = b * b - a * c;
    if discr <= 0.0 || a.abs() < 1e-6 {
        return DVec2::ZERO;
    }
    let sq = discr.sqrt();
    let t = (b - sq) / a;
    if !(t > 0.0) || t > p.tau0 * 3.0 {
        return DVec2::ZERO;
    }
    let m = 2.0;
    let f = -p.k * (-t / p.tau0).exp() * (rv - (rv * b - w * a) / sq) / (a * t.powf(m))
        * (m / t + 1.0 / p.tau0);
    let len = f.length();
    if len > 12.0 {
        f * (12.0 / len)
    } else {
        f
    }
}

/// Advance all walkers by `dt`: velocities follow the wishes with avoidance, positions
/// integrate, contacts are resolved. `blocks` only concern the ground (space 0).
pub fn step(walkers: &mut [Walker], blocks: &[Block], params: &CrowdParams, dt: f64) {
    if dt <= 0.0 || walkers.is_empty() {
        return;
    }
    let mut grid: HashMap<(u64, i32, i32), Vec<usize>> = HashMap::new();
    for (i, w) in walkers.iter().enumerate() {
        grid.entry(cell_of(w.space, w.pos)).or_default().push(i);
    }
    let reach = (params.sense / CELL).ceil() as i32;
    let mut new_vel = vec![DVec2::ZERO; walkers.len()];
    for (i, me) in walkers.iter().enumerate() {
        if me.fixed {
            continue;
        }
        let mut force = (me.want - me.vel) / params.relax;
        let mut avoid = DVec2::ZERO;
        let (sp, cx, cy) = cell_of(me.space, me.pos);
        if !me.ghost {
            for gy in cy - reach..=cy + reach {
                for gx in cx - reach..=cx + reach {
                    let Some(list) = grid.get(&(sp, gx, gy)) else {
                        continue;
                    };
                    for &j in list {
                        if j == i {
                            continue;
                        }
                        let o = &walkers[j];
                        let d = o.pos - me.pos;
                        if d.length_squared() > params.sense * params.sense {
                            continue;
                        }
                        // nobody minds the people behind them
                        if d.dot(me.want) < 0.0 && d.length() > me.radius + o.radius + 0.1 {
                            continue;
                        }
                        avoid +=
                            ttc_force(params, me.pos, me.vel, o.pos, o.vel, me.radius + o.radius);
                    }
                }
            }
        }
        if me.space == 0 {
            for b in blocks {
                if !b.near(me.pos, params.sense) {
                    continue;
                }
                let (q, inside) = b.closest(me.pos);
                if inside {
                    continue;
                }
                avoid += ttc_force(params, me.pos, me.vel, q, b.vel, me.radius + 0.05);
            }
        }
        // Straight at somebody the force only brakes, and two people stop nose to nose:
        // a walker then steps to the right, as people do.
        let wl = me.want.length();
        let al = avoid.length();
        if wl > 0.1 && al > 1e-3 && avoid.dot(me.want) < -0.9 * wl * al {
            avoid += DVec2::new(me.want.y, -me.want.x) / wl * al * 0.5;
        }
        // somebody who stands still only makes a little room
        force += avoid * me.give.clamp(0.0, 1.0);
        let len = force.length();
        if len > params.max_accel {
            force *= params.max_accel / len;
        }
        let mut v = me.vel + force * dt;
        // a walker never goes much faster than it wants to; a standing person only shuffles
        let cap = (me.want.length() * 1.2).max(0.3);
        let vl = v.length();
        if vl > cap {
            v *= cap / vl;
        }
        new_vel[i] = v;
    }
    for (i, w) in walkers.iter_mut().enumerate() {
        if w.fixed {
            continue;
        }
        w.vel = new_vel[i];
        w.pos += w.vel * dt;
        if let Some((a, b, dev)) = w.corridor {
            w.pos = ease_into_corridor(w.pos, a, b, dev, dt);
        }
    }
    // contacts: nobody inside anybody else or inside a vehicle
    for _ in 0..2 {
        for i in 0..walkers.len() {
            let (sp, cx, cy) = cell_of(walkers[i].space, walkers[i].pos);
            for gy in cy - 1..=cy + 1 {
                for gx in cx - 1..=cx + 1 {
                    let Some(list) = grid.get(&(sp, gx, gy)) else {
                        continue;
                    };
                    for &j in list {
                        if j <= i {
                            continue;
                        }
                        let (a, b) = (walkers[i], walkers[j]);
                        if a.ghost || b.ghost || a.space != b.space {
                            continue;
                        }
                        let d = b.pos - a.pos;
                        let dist = d.length();
                        let min = a.radius + b.radius;
                        if dist >= min {
                            continue;
                        }
                        let ga = if a.fixed { 0.0 } else { a.give.max(0.05) };
                        let gb = if b.fixed { 0.0 } else { b.give.max(0.05) };
                        if ga + gb <= 0.0 {
                            continue;
                        }
                        let n = if dist > 1e-6 {
                            d / dist
                        } else {
                            DVec2::new(((i * 7 + j * 3) % 5) as f64 - 2.0, 1.0).normalize()
                        };
                        let push = min - dist;
                        walkers[i].pos -= n * push * ga / (ga + gb);
                        walkers[j].pos += n * push * gb / (ga + gb);
                    }
                }
            }
        }
        for w in walkers.iter_mut() {
            if w.fixed || w.space != 0 {
                continue;
            }
            for b in blocks {
                if !b.near(w.pos, w.radius + 0.1) {
                    continue;
                }
                let (q, inside) = b.closest(w.pos);
                let d = w.pos - q;
                let dist = d.length();
                if inside {
                    let n = if dist > 1e-6 { -d / dist } else { DVec2::X };
                    w.pos = q + n * w.radius;
                } else if dist < w.radius {
                    let n = if dist > 1e-6 { d / dist } else { DVec2::X };
                    w.pos = q + n * w.radius;
                }
            }
        }
    }
    for w in walkers.iter_mut() {
        if let (false, Some((a, b, dev))) = (w.fixed, w.corridor) {
            w.pos = ease_into_corridor(w.pos, a, b, dev, dt);
        }
    }
}
