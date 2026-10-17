//! A road vehicle: the bicycle model that follows the way, and the body on its springs over
//! the ground under its wheels.

use super::{dir, AiBody};
use glam::{DVec2, DVec3};
use std::f32::consts::FRAC_PI_2;

/// How far over an AI car's wheel its ground is looked for (m): OMSI casts the ray of its
/// ground query from 3 m over the point and takes the highest face under that.
pub const AI_RAY_UP: f64 = 3.0;
/// A face this far under the wheel's lane is not this wheel's ground (m): the road under the
/// bridge the lane crosses, or the ground beyond the edge of the road, which the wheel hangs
/// over on its spring rather than drops onto (the lane stays its height there).
const AI_DROP: f64 = 0.3;

impl AiBody {
    /// Bicycle model: the rotation point follows the way, the front wheels steer towards a
    /// point further along it (pure pursuit, looking further ahead the faster the car goes).
    pub(super) fn drive(&mut self, dt: f32, speed: f32, way: &dyn Fn(f32) -> DVec3) {
        let target = way(self.rot_long).truncate();
        if !self.started || (target - self.rear).length() > 8.0 {
            // spawned, or put somewhere else: stand on the way, straight
            let ahead = way(self.rot_long + 2.0).truncate();
            let d = ahead - target;
            self.heading = if d.length() > 1e-3 { d.x.atan2(d.y).to_degrees() } else { self.heading };
            self.rear = target;
            self.steer = 0.0;
            self.steer_cmd = 0.0;
            self.started = true;
        }
        let fwd = dir(self.heading);
        let right = DVec2::new(fwd.y, -fwd.x);
        // the planner's position along the way leads; the body keeps up with it
        let along = (target - self.rear).dot(fwd) as f32;
        let v = if speed < 0.02 && along.abs() < 0.05 { 0.0 } else { (speed + 1.5 * along).clamp(0.0, speed + 3.0) };
        let look = (1.2 * self.wheelbase).max(3.5) + 0.6 * speed.min(20.0);
        let g = way(self.rot_long + look).truncate() - self.rear;
        let alpha = (g.dot(right) as f32).atan2(g.dot(fwd) as f32).clamp(-FRAC_PI_2, FRAC_PI_2);
        let reach = (g.length() as f32).max(1.0);
        // The way itself says how tight it bends here: a junction's turn laid tighter than
        // the model's `[inv_min_turnradius]` still has to be followed - capped at the
        // model's lock the car ran wide of it, through the kerb, the corner house and the
        // people on the pavement (#249; OMSI's AI keeps to its path). The lock the bend
        // needs, and a little more, is allowed, and the wheel may turn that much faster.
        let (a, b, c) = (way(self.rot_long).truncate(), way(self.rot_long + 0.5 * look).truncate(), way(self.rot_long + look).truncate());
        let (u, w) = (b - a, c - b);
        let turn = (u.x * w.y - u.y * w.x).atan2(u.dot(w)).abs() as f32;
        let bend_k = if look > 0.1 { 2.0 * turn / look } else { 0.0 };
        let need = (bend_k * self.wheelbase).atan().to_degrees() * 1.15;
        let limit = if omsi_cfg::flags::AI_MODEL_LOCK.is_set() { self.max_steer } else { self.max_steer.max(need.min(60.0)) };
        // OMSI_DEBUG_AI_WIDE: every tenth of a second a car stands over 1.5 m beside its way
        if omsi_cfg::flags::DEBUG_AI_WIDE.is_set() {
            let off = ((target - self.rear).dot(right)).abs();
            if off > 1.5 {
                static N: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let n = N.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                if n % 50 == 0 {
                    log::info!("ai wide: {off:.1} m beside its way at ({:.0}, {:.0}), bend needs {need:.0} deg, lock {:.0} ({} so far)", self.rear.x, self.rear.y, self.max_steer, n + 1);
                }
            }
        }
        let want = (2.0 * self.wheelbase * alpha.sin() / reach).atan().to_degrees().clamp(-limit, limit);
        if dt > 0.0 {
            let rate = self.steer_rate * dt * if limit > self.max_steer { 1.5 } else { 1.0 };
            self.steer_cmd += (want - self.steer_cmd).clamp(-rate, rate);
            self.steer += (self.steer_cmd - self.steer) * (dt / 0.08).min(1.0);
        }
        let k = self.steer.to_radians().tan() / self.wheelbase;
        let dpsi = (v * dt * k) as f64;
        let mid = dir(self.heading + dpsi.to_degrees() * 0.5);
        self.rear += mid * (v * dt) as f64;
        self.heading = (self.heading + dpsi.to_degrees()).rem_euclid(360.0);
        self.yaw_rate = if dt > 0.0 { (dpsi / dt as f64) as f32 } else { 0.0 };
        self.a_lat = v * v * k;
        let o = self.rear - dir(self.heading) * self.rot_long as f64;
        self.position.x = o.x;
        self.position.y = o.y;
    }

    /// The body on its springs over the ground under its wheels: the plane through the
    /// contact points is where the body wants to be, braking and cornering lean it off
    /// that plane, and the difference is how far each wheel moves in its arch.
    pub(super) fn settle(&mut self, dt: f32, way: &dyn Fn(f32) -> DVec3, ground: Option<&dyn Fn(f64, f64) -> Option<f64>>, contact: Option<&dyn crate::physics::Ground>) {
        let fwd = dir(self.heading);
        let right = DVec2::new(fwd.y, -fwd.x);
        let origin = self.position.truncate();
        // the way's own height at each axle: the road the map says is there (where nothing
        // is drawn under a wheel, and what tells another level from this one)
        let mut axle_z = vec![f64::NAN; self.axle_count];
        if self.contact_z.len() != self.wheels.len() {
            self.contact_z = vec![f64::NAN; self.wheels.len()];
        }
        let mut contacts: Vec<(f32, f32, f64)> = Vec::with_capacity(self.wheels.len());
        // per wheel: no face near its lane's height, and the face found deeper down
        let mut hung: Vec<bool> = Vec::with_capacity(self.wheels.len());
        let mut low: Vec<Option<f64>> = Vec::with_capacity(self.wheels.len());
        for (k, w) in self.wheels.iter().enumerate() {
            if axle_z[w.axle].is_nan() {
                axle_z[w.axle] = way(w.long).z;
            }
            let path_z = axle_z[w.axle];
            let p = origin + right * w.lat as f64 + fwd * w.long as f64;
            // What the wheel stands on, as OMSI stands an AI car's wheels: its AI cars
            // are bodies on the same wheel physics as the player's bus,
            // asking the ground under each wheel: a ray
            // from 3 m over the wheel down into the terrain, the
            // splines and the first mesh of the `[surface]` objects, and the highest face it
            // meets is the ground. Each wheel on its own face: a car on a cambered road, a
            // kerb or a junction plate tilted on a hill leans with it. (Kept to the lane's
            // height, the least of an axle's wheels and only a few centimetres off that,
            // every car stood up to 5 cm in the asphalt wherever the road was drawn higher
            // than its lane or one wheel stood lower than the other, and one whose lane ran
            // under the drawn road drove through it with only its roof showing.) The ray
            // starts over the wheel's last contact, or over the lane where it has none yet.
            // Without the world's faces, the plain height sampler.
            let last = self.contact_z[k];
            let from = if last.is_finite() && (last - path_z).abs() < AI_RAY_UP { last.max(path_z) } else { path_z };
            let (drawn, deeper) = match contact {
                Some(c) => {
                    let below = c.probe(p.x, p.y, from + AI_RAY_UP).below;
                    (below.filter(|g| *g >= path_z - AI_DROP), below.filter(|g| *g < path_z - AI_DROP && *g > path_z - AI_RAY_UP))
                }
                None => (ground.and_then(|g| g(p.x, p.y)).filter(|g| (*g - path_z).abs() < AI_RAY_UP), None),
            };
            low.push(deeper);
            contacts.push((w.lat, w.long, drawn.unwrap_or(path_z)));
            hung.push(drawn.is_none());
        }
        // A wheel hangs over a drop on its spring while the others carry the car; with no
        // wheel near its lane's height the car has nothing to hang from and comes down onto
        // what is drawn under it, as OMSI's wheel physics drops it. (Kept at the lane's
        // height, a car whose junction's paths ran half a metre over the drawn road drove
        // through the junction in the air, #876.)
        if hung.iter().all(|h| *h) && low.iter().any(|l| l.is_some()) {
            for (c, l) in contacts.iter_mut().zip(&low) {
                if let Some(z) = l {
                    c.2 = *z;
                }
            }
        }
        for (k, c) in contacts.iter().enumerate() {
            self.contact_z[k] = c.2;
        }
        // least-squares plane h = a + b·long + c·lat (the wheels sit symmetrically, so the
        // two slopes separate)
        let n = contacts.len().max(1) as f64;
        let (ml, mt, mh) = contacts.iter().fold((0.0, 0.0, 0.0), |acc, c| (acc.0 + c.1 as f64 / n, acc.1 + c.0 as f64 / n, acc.2 + c.2 / n));
        let (mut sll, mut slh, mut stt, mut sth) = (0.0, 0.0, 0.0, 0.0);
        for c in &contacts {
            let (dl, dt_, dh) = (c.1 as f64 - ml, c.0 as f64 - mt, c.2 - mh);
            sll += dl * dl;
            slh += dl * dh;
            stt += dt_ * dt_;
            sth += dt_ * dh;
        }
        let b = if sll > 1e-6 { slh / sll } else { 0.0 };
        let c = if stt > 1e-6 { sth / stt } else { 0.0 };
        let zg = mh - b * ml - c * mt;
        // pitch up for a road rising ahead; bank positive = right side down (c < 0)
        let pg = b.atan() as f32;
        let rg = (-c).atan() as f32;
        match self.ground {
            Some((z0, p0, r0)) if (zg - self.z).abs() < 1.0 && dt > 0.0 => {
                let k = (dt / 0.1).min(1.0);
                let rates = [((zg - z0) / dt as f64) as f32, (pg - p0) / dt, (rg - r0) / dt];
                for (r, new) in self.ground_rate.iter_mut().zip(rates) {
                    *r += (new.clamp(-5.0, 5.0) - *r) * k;
                }
                let steps = 4;
                let h = dt / steps as f32;
                for _ in 0..steps {
                    let az = (self.spring[0] * (zg - self.z) as f32 + self.damper[0] * (self.ground_rate[0] - self.vz)) / self.mass;
                    let ap = (self.spring[1] * (pg - self.pitch) + self.damper[1] * (self.ground_rate[1] - self.vpitch) + self.mass * self.a_long * self.cog) / self.inertia[0];
                    let ar = (self.spring[2] * (rg - self.roll) + self.damper[2] * (self.ground_rate[2] - self.vroll) - self.mass * self.a_lat * self.cog) / self.inertia[1];
                    self.vz += az * h;
                    self.vpitch += ap * h;
                    self.vroll += ar * h;
                    self.z += (self.vz * h) as f64;
                    self.pitch += self.vpitch * h;
                    self.roll += self.vroll * h;
                }
                self.z = self.z.clamp(zg - 0.15, zg + 0.15);
                self.pitch = self.pitch.clamp(pg - 0.07, pg + 0.07);
                self.roll = self.roll.clamp(rg - 0.09, rg + 0.09);
            }
            _ => {
                // first frame, or the ground under the car changed completely
                self.z = zg;
                self.pitch = pg;
                self.roll = rg;
                self.vz = 0.0;
                self.vpitch = 0.0;
                self.vroll = 0.0;
                self.ground_rate = [0.0; 3];
            }
        }
        self.ground = Some((zg, pg, rg));
        self.position.z = self.z;
        self.pitch_deg = self.pitch.to_degrees();
        self.bank_deg = self.roll.to_degrees();
        let (tp, tr) = (self.pitch.tan(), self.roll.tan());
        for (w, c) in self.wheels.iter().zip(&contacts) {
            let body = self.z + (w.long * tp - w.lat * tr) as f64;
            if let Some(s) = self.suspension.get_mut(w.axle) {
                s[w.side] = ((body - c.2) as f32).clamp(-0.15, 0.15);
            }
        }
    }
}
