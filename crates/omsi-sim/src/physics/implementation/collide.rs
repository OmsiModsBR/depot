//! The body against the obstacles: an impulse at each point of contact (`RigidBody::collide`)
//! and what a hit tells the scripts ([`Impact`]).

use crate::collision::Obb;
use crate::physics::domain::params::{
    BOUNCE_SPEED, CLEARANCE, CRASH_SPEED, INSIDE_DEPTH, INSIDE_FRAMES, INSIDE_REACH, RESTITUTION,
    SCRAPE_FRICTION,
};
use crate::physics::domain::{Impact, RigidBody};
use glam::{DVec3, Vec3};

impl RigidBody {
    /// Keep the body's `[boundingbox]` out of the obstacles and answer each hit with an
    /// impulse at the point of contact: the closing speed goes (a fifth of it comes back),
    /// sliding along the obstacle is braked by friction, and an off-centre blow turns the
    /// body. Nothing is frozen - the vehicle can drive off or reverse at once.
    ///
    /// `skip(i)`: obstacles already broken off. `dt` is the frame, to tell a hit from a box
    /// the vehicle has been standing in all along (spawned in a depot hall's box): that one
    /// is deeper than any frame of motion could have taken it, and never pushes.
    ///
    /// Each obstacle is answered once per call (up to four of them, deepest first). A car
    /// that drives into the bus is still there after its answer - the bus only steps a
    /// millimetre out of its way and takes none of its speed - and was taken as the same
    /// crash four times over.
    pub fn collide(
        &mut self,
        bb: [f32; 6],
        obstacles: &[Obb],
        skip: &dyn Fn(usize) -> bool,
        dt: f32,
    ) -> Vec<Impact> {
        let mut impacts: Vec<Impact> = Vec::new();
        let mut answered: Vec<usize> = Vec::new();
        {
            let me = self.body_box(bb);
            let touching: Vec<i64> = obstacles
                .iter()
                .enumerate()
                .filter(|(i, o)| o.id >= 0 && o.mass <= 0.0 && !skip(*i) && me.contact(o).is_some())
                .map(|(_, o)| o.id)
                .collect();
            match self.spawned_inside.as_mut() {
                None => self.spawned_inside = Some(touching),
                // (once out of one, it counts again)
                Some(list) => list.retain(|id| touching.contains(id)),
            }
        }
        let inside = self.spawned_inside.clone().unwrap_or_default();
        for _ in 0..4 {
            let me = self.body_box(bb);
            let mut deepest: Option<(usize, crate::collision::Contact)> = None;
            for (i, o) in obstacles.iter().enumerate() {
                if skip(i) || answered.contains(&i) || (o.id >= 0 && inside.contains(&o.id)) {
                    continue;
                }
                let Some(c) = me.contact(o) else { continue };
                let rel = (glam::DVec2::new(self.velocity.x as f64, self.velocity.y as f64)
                    - o.velocity)
                    .length()
                    + self.omega.z.abs() as f64 * INSIDE_REACH;
                if c.depth > INSIDE_DEPTH + rel * dt as f64 * INSIDE_FRAMES {
                    continue;
                }
                if deepest.map(|d| c.depth > d.1.depth).unwrap_or(true) {
                    deepest = Some((i, c));
                }
            }
            let Some((i, c)) = deepest else { break };
            answered.push(i);
            let o = &obstacles[i];
            let n = Vec3::new(c.normal.x as f32, c.normal.y as f32, 0.0);
            // A wall as tall as the vehicle stops it at the height of its centre of gravity (a
            // real front is stiffest down at the frame; the middle of a double-decker's face
            // tipped it onto its rear wheels); a low bollard or a high beam acts where it is.
            let z = self.position.z.clamp(c.z0, c.z1.max(c.z0));
            let r = (DVec3::new(c.point.x, c.point.y, z) - self.position).as_vec3();
            // A moving obstacle (an AI car) has a mass of its own, which shares the impulse and
            // the way out of the overlap. But the AI moves it by itself and never gives up its
            // speed: answered with its full approach, it came back every frame and shoved the
            // bus across the road at its own pace until the bus nearly rolled over. So it blocks
            // the bus's motion into it; its own run into the bus counts as the crash it is, and
            // pushes nothing.
            let v_other = Vec3::new(o.velocity.x as f32, o.velocity.y as f32, 0.0);
            let v_body = self.velocity + self.orientation.mul_vec3(self.omega).cross(r);
            let vn_full = (v_body - v_other).dot(n);
            let v_c = v_body - (v_other - n * v_other.dot(n).max(0.0));
            let vn = v_c.dot(n);
            let inv_other = if o.mass > 0.0 { 1.0 / o.mass } else { 0.0 };
            let k_body = self.inv_mass_at(r, n);
            let k_n = k_body + inv_other;
            let before = self.kinetic_energy();
            // The scripts are told where the bodies meet, low down where the bumpers are
            // (the stock buses damage their rear engine only below 1.10 m), not where the
            // blow is taken: at the centre of gravity every wall and car read 1.2-1.3 m.
            let seen = DVec3::new(
                c.point.x,
                c.point.y,
                crate::collision::impact_height(c.z0, c.z1),
            );
            let point = self
                .orientation
                .inverse()
                .mul_vec3((seen - self.position).as_vec3())
                + self.cog;
            if let Some((mass_t, load_kn)) = o.pole.filter(|_| vn < 0.0) {
                // A post breaks at its foot when stopping the vehicle would take more than it
                // can bear; the vehicle then only loses what it takes to throw the post.
                let stop = -vn / k_n / dt.max(1e-3);
                if -vn > CRASH_SPEED && stop > load_kn.max(0.1) * 1000.0 {
                    let m_pole = (mass_t * 1000.0).max(5.0);
                    let j = -(1.0 + RESTITUTION) * vn / (k_n + 1.0 / m_pole);
                    self.apply_impulse(r, n * j);
                    impacts.push(Impact {
                        point,
                        speed: -vn,
                        energy: (before - self.kinetic_energy()).max(0.0),
                        obstacle: i,
                        broke: true,
                        push: -n,
                    });
                    continue;
                }
            }
            // out of the obstacle along the shortest way, a millimetre clear - out of a moving
            // one only as far as the body itself ran into it this frame: a standing car is not
            // ploughed through, and one that drives into the bus does not drag it along
            let out = if o.mass > 0.0 {
                (c.depth as f32).min((-vn).max(0.0) * dt + CLEARANCE)
            } else {
                c.depth as f32 + CLEARANCE
            };
            self.position += (n * out).as_dvec3();
            if vn_full >= 0.0 {
                continue;
            }
            // a gentle push is dead, a real blow gives a little back
            let e = if -vn_full > BOUNCE_SPEED {
                RESTITUTION
            } else {
                0.0
            };
            let j = -(1.0 + e) * vn.min(0.0) / k_n;
            let mut p = n * j;
            let vt = v_c - n * vn;
            let vt = Vec3::new(vt.x, vt.y, 0.0);
            if vt.length() > 1e-3 {
                let t = vt.normalize();
                let jt =
                    (vt.length() / (self.inv_mass_at(r, t) + inv_other)).min(SCRAPE_FRICTION * j);
                p -= t * jt;
            }
            self.apply_impulse(r, p);
            // what the crash destroyed: the closing motion of the two, less what bounced back
            // (the bus alone may even gain energy when the other one does the hitting)
            let energy =
                (before - self.kinetic_energy()).max(0.5 * vn_full * vn_full * (1.0 - e * e) / k_n);
            impacts.push(Impact {
                point,
                speed: -vn_full,
                energy,
                obstacle: i,
                broke: false,
                push: -n,
            });
        }
        // a blow the tyres have to catch: they slide until the body is back on its wheels
        if impacts.iter().any(|m| m.speed >= CRASH_SPEED) {
            self.holding = false;
        }
        impacts
    }
}
