//! The body against the obstacles: an impulse at each point of contact (`RigidBody::collide`)
//! and what a hit tells the scripts ([`Impact`]).

use crate::physics::domain::params::{
    BOUNCE_SPEED, CLEARANCE, CRASH_SPEED, INSIDE_DEPTH, INSIDE_FRAMES, INSIDE_REACH, RESTITUTION,
    SCRAPE_FRICTION,
};
use crate::physics::domain::{Impact, RigidBody};
use crate::collision::Obb;
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
    pub fn collide(&mut self, bb: [f32; 6], obstacles: &[Obb], skip: &dyn Fn(usize) -> bool, dt: f32) -> Vec<Impact> {
        let mut impacts: Vec<Impact> = Vec::new();
        let mut answered: Vec<usize> = Vec::new();
        {
            let me = self.body_box(bb);
            let touching: Vec<i64> = obstacles.iter().enumerate().filter(|(i, o)| o.id >= 0 && o.mass <= 0.0 && !skip(*i) && me.contact(o).is_some()).map(|(_, o)| o.id).collect();
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
                let rel = (glam::DVec2::new(self.velocity.x as f64, self.velocity.y as f64) - o.velocity).length() + self.omega.z.abs() as f64 * INSIDE_REACH;
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
            let seen = DVec3::new(c.point.x, c.point.y, crate::collision::impact_height(c.z0, c.z1));
            let point = self.orientation.inverse().mul_vec3((seen - self.position).as_vec3()) + self.cog;
            if let Some((mass_t, load_kn)) = o.pole.filter(|_| vn < 0.0) {
                // A post breaks at its foot when stopping the vehicle would take more than it
                // can bear; the vehicle then only loses what it takes to throw the post.
                let stop = -vn / k_n / dt.max(1e-3);
                if -vn > CRASH_SPEED && stop > load_kn.max(0.1) * 1000.0 {
                    let m_pole = (mass_t * 1000.0).max(5.0);
                    let j = -(1.0 + RESTITUTION) * vn / (k_n + 1.0 / m_pole);
                    self.apply_impulse(r, n * j);
                    impacts.push(Impact { point, speed: -vn, energy: (before - self.kinetic_energy()).max(0.0), obstacle: i, broke: true, push: -n });
                    continue;
                }
            }
            // out of the obstacle along the shortest way, a millimetre clear - out of a moving
            // one only as far as the body itself ran into it this frame: a standing car is not
            // ploughed through, and one that drives into the bus does not drag it along
            let out = if o.mass > 0.0 { (c.depth as f32).min((-vn).max(0.0) * dt + CLEARANCE) } else { c.depth as f32 + CLEARANCE };
            self.position += (n * out).as_dvec3();
            if vn_full >= 0.0 {
                continue;
            }
            // a gentle push is dead, a real blow gives a little back
            let e = if -vn_full > BOUNCE_SPEED { RESTITUTION } else { 0.0 };
            let j = -(1.0 + e) * vn.min(0.0) / k_n;
            let mut p = n * j;
            let vt = v_c - n * vn;
            let vt = Vec3::new(vt.x, vt.y, 0.0);
            if vt.length() > 1e-3 {
                let t = vt.normalize();
                let jt = (vt.length() / (self.inv_mass_at(r, t) + inv_other)).min(SCRAPE_FRICTION * j);
                p -= t * jt;
            }
            self.apply_impulse(r, p);
            // what the crash destroyed: the closing motion of the two, less what bounced back
            // (the bus alone may even gain energy when the other one does the hitting)
            let energy = (before - self.kinetic_energy()).max(0.5 * vn_full * vn_full * (1.0 - e * e) / k_n);
            impacts.push(Impact { point, speed: -vn_full, energy, obstacle: i, broke: false, push: -n });
        }
        // a blow the tyres have to catch: they slide until the body is back on its wheels
        if impacts.iter().any(|m| m.speed >= CRASH_SPEED) {
            self.holding = false;
        }
        impacts
    }
}

#[cfg(test)]
mod tests {
    use crate::physics::tests::{bus, road, run};
    use super::*;

    /// Straight into a wall at 40 km/h: the bus stops at the wall, bounces off a little, and
    /// drives away backwards straight after.
    #[test]
    fn crash_bounces_and_releases() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        rb.velocity = Vec3::new(0.0, 40.0 / 3.6, 0.0);
        let wall = [Obb::from_box([20.0, 1.0, 3.0, 0.0, 0.0, 1.5], DVec3::new(0.0, 12.0, 0.0), 0.0)];
        let mut impacts = Vec::new();
        for _ in 0..90 {
            rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
            impacts.extend(rb.collide(bb, &wall, &|_| false, 1.0 / 30.0));
        }
        assert_eq!(impacts.iter().filter(|i| i.speed > CRASH_SPEED).count(), 1, "{impacts:?}");
        let hit = impacts[0];
        assert!(hit.speed > 10.0 && hit.energy > 400_000.0, "{hit:?}");
        assert!(hit.point.y > 5.5, "{hit:?}");
        assert!(rb.forward_speed() < 0.0, "bounced: {}", rb.forward_speed());
        let front = rb.origin().y + 5.83;
        assert!(front < 11.5 + 0.01, "{front}");
        // reverse away at once
        let y0 = rb.origin().y;
        for _ in 0..60 {
            rb.step(1.0 / 30.0, -8000.0, &[0.0; 4], 0.0, &g);
            rb.collide(bb, &wall, &|_| false, 1.0 / 30.0);
        }
        assert!(rb.origin().y < y0 - 1.0, "{} -> {}", y0, rb.origin().y);
    }

    /// At 4 frames a second the bus answers the wall after each slice of the frame, with the
    /// time the slice covered: it stops at the wall, as it does at 30, and the crash counts
    /// once. (Answered once a frame, it ran 2.6 m into the wall before the wall pushed it out.)
    #[test]
    fn a_long_frame_meets_the_wall_slice_by_slice() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        rb.velocity = Vec3::new(0.0, 40.0 / 3.6, 0.0);
        let wall = [Obb::from_box([20.0, 1.0, 3.0, 0.0, 0.0, 1.5], DVec3::new(0.0, 12.0, 0.0), 0.0)];
        let mut impacts = Vec::new();
        let mut deepest = f64::MIN;
        for _ in 0..12 {
            let inputs = [crate::physics::BodyInputs { brake: &[0.0; 4], steer: Some(0.0), air: true }];
            crate::physics::step_train_with(std::slice::from_mut(&mut rb), &[], 0.25, 0.0, &inputs, &g, &mut |bodies, slice| {
                deepest = deepest.max(bodies[0].origin().y + 5.83 - 11.5);
                impacts.extend(bodies[0].collide(bb, &wall, &|_| false, slice));
            });
        }
        assert_eq!(impacts.iter().filter(|i| i.speed > CRASH_SPEED).count(), 1, "{impacts:?}");
        assert!(deepest < 0.6, "the front went {deepest} m into the wall");
        assert!(rb.origin().y + 5.83 < 11.5 + 0.01);
    }

    /// A glancing blow on the front corner turns the bus away from the wall and lets it slide.
    #[test]
    fn glancing_blow_turns_the_bus() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        // heading 20° to the right towards a wall along x = 4
        rb.place(DVec3::ZERO, 20.0);
        run(&mut rb, 0.5, 0.0, 0.0, &g);
        rb.velocity = rb.orientation.mul_vec3(Vec3::Y) * 30.0 / 3.6;
        let wall = [Obb::from_box([1.0, 60.0, 3.0, 0.0, 0.0, 1.5], DVec3::new(4.5, 10.0, 0.0), 0.0)];
        let mut hits = 0;
        for _ in 0..60 {
            rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
            hits += rb.collide(bb, &wall, &|_| false, 1.0 / 30.0).len();
        }
        let (heading, _, _) = rb.heading_pitch_bank();
        assert!(hits > 0);
        assert!(heading < 15.0 || heading > 300.0, "turned away from the wall: {heading}");
        assert!(rb.velocity.y > 1.0, "still sliding along: {:?}", rb.velocity);
    }

    /// A car (1 t, kinematic: it keeps its speed) drives into the side of the standing bus
    /// at 50 km/h: the bus gets a nudge and a crash, not a shove across the road.
    #[test]
    fn a_car_does_not_bulldoze_the_bus() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        let (x0, mut hits, mut energy) = (rb.origin().x, 0, 0.0f32);
        for k in 0..30 {
            // from the left, heading east
            let car = Obb::from_box([1.7, 4.2, 1.4, 0.0, 0.0, 0.7], DVec3::new(-4.0 + 50.0 / 3.6 * k as f64 / 30.0, 0.0, 0.0), 90.0).moving(glam::DVec2::new(50.0 / 3.6, 0.0), 1000.0, 7);
            rb.step(1.0 / 30.0, 0.0, &[5000.0; 4], 0.0, &g);
            for hit in rb.collide(bb, &[car], &|_| false, 1.0 / 30.0) {
                hits += 1;
                energy += hit.energy;
            }
        }
        run(&mut rb, 2.0, 0.0, 5000.0, &g);
        let (_, _, bank) = rb.heading_pitch_bank();
        assert!(hits > 0 && energy > 20_000.0, "{hits} hits, {energy} J");
        assert!((rb.origin().x - x0).abs() < 1.0, "shoved {} m", rb.origin().x - x0);
        assert!(bank.abs() < 3.0, "banked {bank}");
    }

    /// A car driving into the standing bus is one crash per call, however long it stays
    /// inside the bus: 0.5 v² (1 - e²) over the pair's inverse mass, not four times that.
    #[test]
    fn a_car_strike_counts_once() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        let v = 50.0f32 / 3.6;
        let car = Obb::from_box([1.7, 4.2, 1.4, 0.0, 0.0, 0.7], DVec3::new(-2.0, 0.0, 0.0), 90.0).moving(glam::DVec2::new(v as f64, 0.0), 1000.0, 7);
        let hits = rb.collide(bb, &[car], &|_| false, 1.0 / 30.0);
        assert_eq!(hits.len(), 1, "{hits:?}");
        let n = Vec3::X;
        let r = Vec3::new(-1.236, 0.0, 0.0);
        let k = rb.inv_mass_at(r, n) + 1.0 / 1000.0;
        let want = 0.5 * v * v * (1.0 - RESTITUTION * RESTITUTION) / k;
        assert!((hits[0].energy - want).abs() < 0.15 * want, "{} J, want about {want}", hits[0].energy);
        assert!(hits[0].energy < 100_000.0, "{hits:?}");
    }

    /// The scripts hear where the bodies meet - low down at the bumpers - not the height of
    /// the centre of gravity: reversing into a wall or a car is below 1.10 m and behind
    /// -4.70 m, where the stock buses damage their engine; a high beam is hit high.
    #[test]
    fn impacts_are_reported_at_bumper_height() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let hit = |obstacle: Obb, v: Vec3| {
            let mut rb = RigidBody::from_definition(&def, &[]);
            rb.place(DVec3::ZERO, 0.0);
            let g = road(1e9, 0.0);
            run(&mut rb, 1.0, 0.0, 5000.0, &g);
            rb.velocity = v;
            for _ in 0..60 {
                rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
                if let Some(h) = rb.collide(bb, &[obstacle], &|_| false, 1.0 / 30.0).into_iter().find(|h| h.speed > CRASH_SPEED) {
                    return h;
                }
            }
            panic!("no hit on {obstacle:?}");
        };
        let back = Vec3::new(0.0, -15.0 / 3.6, 0.0);
        let wall = hit(Obb::from_box([20.0, 1.0, 5.0, 0.0, 0.0, 2.5], DVec3::new(0.0, -8.0, 0.0), 0.0), back);
        assert!(wall.point.z < 1.1 && wall.point.z > 0.5 && wall.point.y < -4.7, "{wall:?}");
        let car = hit(Obb::from_box([1.7, 4.2, 1.4, 0.0, 0.0, 0.7], DVec3::new(0.0, -9.0, 0.0), 0.0).moving(glam::DVec2::ZERO, 1000.0, 3), back);
        assert!(car.point.z < 1.1 && car.point.y < -4.7, "{car:?}");
        let front = hit(Obb::from_box([20.0, 1.0, 5.0, 0.0, 0.0, 2.5], DVec3::new(0.0, 8.0, 0.0), 0.0), -back);
        assert!(front.point.z < 1.1 && front.point.y > 4.7, "{front:?}");
        let beam = hit(Obb::from_box([20.0, 1.0, 1.0, 0.0, 0.0, 2.5], DVec3::new(0.0, 8.0, 0.0), 0.0), -back);
        assert!(beam.point.z > 2.0, "{beam:?}");
    }

    /// Into the back of a standing car at 30 km/h: the car (1 t, which the AI holds where it
    /// is) stops the bus within a few metres instead of being driven through.
    #[test]
    fn rear_ends_a_standing_car() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        rb.velocity = Vec3::new(0.0, 30.0 / 3.6, 0.0);
        let car = [Obb::from_box([1.7, 4.2, 1.4, 0.0, 0.0, 0.7], DVec3::new(0.0, 12.0, 0.0), 0.0).moving(glam::DVec2::ZERO, 1000.0, 3)];
        let mut first = None;
        for k in 0..90 {
            rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
            if !rb.collide(bb, &car, &|_| false, 1.0 / 30.0).is_empty() && first.is_none() {
                first = Some((k, rb.origin().y));
            }
        }
        let (_, y_hit) = first.expect("hit the car");
        let front = rb.origin().y + 5.83;
        assert!(rb.forward_speed().abs() < 0.3, "still at {} km/h", rb.forward_speed() * 3.6);
        assert!(front < 12.0 - 2.1 + 0.3, "drove into the car: front at {front}");
        assert!(rb.origin().y - y_hit < 3.0, "pushed on {} m", rb.origin().y - y_hit);
    }

    /// A lamp post breaks off: the bus hardly slows down.
    #[test]
    fn a_post_breaks_off() {
        let def = bus();
        let bb = def.bounding_box.unwrap();
        let mut rb = RigidBody::from_definition(&def, &[]);
        rb.place(DVec3::ZERO, 0.0);
        let g = road(1e9, 0.0);
        run(&mut rb, 1.0, 0.0, 5000.0, &g);
        rb.velocity = Vec3::new(0.0, 30.0 / 3.6, 0.0);
        let mut post = Obb::from_box([0.2, 0.2, 6.0, 0.0, 0.0, 3.0], DVec3::new(0.5, 10.0, 0.0), 0.0);
        post.pole = Some((0.05, 0.5));
        let mut broke = false;
        for _ in 0..30 {
            rb.step(1.0 / 30.0, 0.0, &[0.0; 4], 0.0, &g);
            let hits = rb.collide(bb, &[post], &|_| broke, 1.0 / 30.0);
            broke |= hits.iter().any(|h| h.broke);
        }
        assert!(broke);
        assert!(rb.forward_speed() > 25.0 / 3.6, "{}", rb.forward_speed() * 3.6);
    }
}
