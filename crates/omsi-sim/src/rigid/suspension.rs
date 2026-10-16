//! What carries the body: each wheel
//! on its strut - OMSI's spring on the ground under the wheel -, the tyres on the ground handed
//! on as contacts.

use super::body::RigidBody;
use super::ground::GroundProbe;
use super::params::{BUMP, CLIMB, DROOP, OMSI_PROBE_HEIGHT, STEP_LOAD_MAX, WALL_C, WALL_K};
use super::tyre::Contact;
use super::wheel::{sign, TyreFrame};
use glam::{DVec3, Vec3};

impl RigidBody {
    /// Each wheel on its strut - the strut's, the step's and the walls' forces added to `force`
    /// and `torque`, a wheel off the ground turning by its drive and brake - and the tyres on
    /// the ground (`found`: the slope each stands on and where it touches) as contacts for the
    /// tyre forces, onto `contacts`.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wheel_contacts(&mut self, h: f32, drive_torque: f32, driven: f32, brake: &[f32], found: &[Option<(f32, f32)>], force: &mut Vec3, torque: &mut Vec3, probe: &dyn Fn(f64, f64, f64) -> GroundProbe, contacts: &mut Vec<Contact>) {
        let rot = self.orientation;
        let up = rot.mul_vec3(Vec3::Z);
        let n_wheels = self.wheels.len().max(1) as f32;
        let (position, cog) = (self.position, self.cog);
        let omega_world = rot.mul_vec3(self.omega);
        for (i, w) in self.wheels.iter_mut().enumerate() {
            let TyreFrame { fwd, right, fwd_h, hub0, hub } = TyreFrame::of(w, rot, position, cog);
            let r = w.radius;
            // pressed into a face: pushed back out
            let mut f_world = Vec3::ZERO;
            for face in &w.walls {
                let n = face.normal.extend(0.0);
                let pen = face.reach(fwd_h, r) - face.distance(hub);
                if pen > 0.0 {
                    let v_hub = self.velocity + omega_world.cross((hub - self.position).as_vec3());
                    f_world += n * (WALL_K * pen - WALL_C * v_hub.dot(n)).max(0.0);
                }
            }
            w.step_force += f_world.dot(fwd_h);
            // OMSI 2's suspension: the body hangs straight on the ground point under each
            // wheel. The spring pushes by how far the ground lies over the wheel's unloaded
            // place, times `Axle_Springfactor` and `achse_feder`; below it the wheel is in the
            // air, pushes nothing and the tyres stop holding. Pressed, the damper takes
            // `achse_daempfer` times the body's upward speed where the strut acts (the roll
            // rate at `achse_maxwidth` / 2), and the whole never passes `achse_maxforce` -
            // there is no wheel of its own mass, no tyre and no bump stop between the road and
            // the body. (Those, with a tyre envelope smoothing the road, had the bus float
            // over what it drove on: "like a boat on the sea".)
            let k = w.spring * w.spring_factor.max(0.0);
            let top = hub0.z + (CLIMB * r) as f64;
            // (nothing under it: OMSI 2's ground query looks from 3 m over the model origin's
            // plane - a wheel that sank through a face at a joint of two surfaces, or into a
            // bridge deck, found nothing within reach, the spring let go and the bus fell
            // through the world)
            // The spring's point is where OMSI 2 puts it: on the model's origin plane (z = 0)
            // under the wheel, and its height over the ground is measured straight up.
            // Measured from the hub less the `.bus` file's tyre radius instead, a mod whose
            // tyre mesh is larger than that radius stood with its wheels drawn sunk a few
            // centimetres into the road.
            let plane0 = position + rot.mul_vec3(Vec3::new(w.attach.x, w.attach.y, 0.0) - cog).as_dvec3();
            let under = probe(plane0.x, plane0.y, top).below.or_else(|| probe(plane0.x, plane0.y, plane0.z + OMSI_PROBE_HEIGHT).below);
            let t = under.map(|g| (g - plane0.z) as f32);
            if let Some(g) = under {
                w.ground_z = g;
                w.ground_seen = true;
            }
            let r_out = rot.mul_vec3(w.attach - self.cog + Vec3::new(w.lever, 0.0, 0.0));
            let v_up = (self.velocity + omega_world.cross(r_out)).dot(up);
            let mut n = 0.0f32;
            if let Some(t) = t {
                let spring = k * t;
                if spring >= 0.0 {
                    n = (spring - w.damper * v_up).min(w.max_force);
                }
            }
            let travel = t.unwrap_or(-DROOP);
            let full = if k > 1.0 { w.max_force / k } else { BUMP };
            w.compression_rate = if w.touch.is_some() { (travel.clamp(-DROOP, full) - w.compression) / h } else { 0.0 };
            w.compression = travel.clamp(-DROOP, full);
            w.touch = t;
            let on = t.is_some_and(|t| k * t >= 0.0);
            w.on_ground = on;
            w.load = n.max(0.0);
            let tyre_f = if on { n.max(0.0) } else { 0.0 };
            // the strut's force turns the body about its length from the outer width, not
            // from where the tyre stands (see `RigidWheel::lever`): body x cross body z
            *torque += Vec3::new(0.0, -w.lever * n, 0.0);
            let Some((slope, contact_dx)) = found[i].filter(|_| tyre_f > 0.0) else {
                // off the ground the wheel keeps its turning (a brake stops it)
                let (drive_w, brake_w) = (if w.driven { drive_torque / r / driven } else { 0.0 }, brake.get(i).copied().unwrap_or(0.0).max(0.0));
                let before = w.spin;
                w.spin += h * w.inertia_inv * (drive_w - sign(w.spin) * brake_w);
                if before != 0.0 && sign(before) != sign(w.spin) && brake_w > 0.0 {
                    w.spin = 0.0;
                }
                w.slipping = true;
                w.rpm = w.spin * 60.0 / std::f32::consts::TAU;
                w.rotation_deg = (w.rotation_deg + w.spin.to_degrees() * h).rem_euclid(360.0);
                // the tyre off the ground (or nothing under it: the edge of the loaded
                // world): the strut still pushes on the body
                let f = f_world + up * n;
                *force += f;
                *torque += rot.inverse().mul_vec3((hub - self.position).as_vec3()).cross(rot.inverse().mul_vec3(f));
                continue;
            };
            // The ground pushes along its own normal; the strut carries the part along
            // the body's up axis, and the rest pushes the wheel back up a kerb (or on
            // down an edge). Measured against the body, a slope the bus stands on
            // already leans with it and adds nothing.
            let normal = (Vec3::Z - fwd_h * slope).normalize();
            let along_up = normal.dot(up).max(0.3);
            let carried = tyre_f.min(w.max_force * STEP_LOAD_MAX);
            let step = (normal - up * normal.dot(up)) * (carried / along_up);
            w.step_force += step.dot(fwd_h);
            let contact = hub + (fwd_h * contact_dx.clamp(-r, r)).as_dvec3() - DVec3::Z * r as f64;
            let r_world = (contact - self.position).as_vec3();
            let v_point = self.velocity + omega_world.cross(r_world);
            let v_long = v_point.dot(fwd);
            // the tyre works with at most its rated load
            let grip = tyre_f.min(w.max_force);
            contacts.push(Contact {
                r: r_world,
                fwd,
                right,
                base: f_world + up * n + step,
                drive: if w.driven { drive_torque / r / driven } else { 0.0 },
                brake: brake.get(i).copied().unwrap_or(0.0).max(0.0) + self.rolling_resistance / n_wheels,
                grip,
                v_long,
                lateral: true,
                wheel: Some(i),
            });
        }
    }
}
