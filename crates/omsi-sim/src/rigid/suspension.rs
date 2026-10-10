//! What carries the body: the static load on each wheel ([`wheel_rest_loads`]) and each wheel
//! on its strut - OMSI's spring on the ground under the wheel, or the A/B model with a wheel
//! mass, a tyre and bump stops -, the tyres on the ground handed on as contacts.

use super::body::RigidBody;
use super::ground::GroundProbe;
use super::params::{
    BUMP, BUMP_LOAD_MAX, BUMP_STIFFNESS, BUMP_TRAVEL, CLIMB, DAMPER_RATE_MAX, DROOP, DROOP_REACH,
    ENVELOPE_SLOPE, GRAVITY, MIN_LOAD_SHARE, MIN_WHEEL_MASS, OMSI_PROBE_HEIGHT, REBOUND_LOAD_MAX,
    RISE_MIN_SPEED, RISE_SPARE, STEP_LOAD_MAX, TYRE_CLOSING_MAX, TYRE_LOAD_MAX, UNSPRUNG, WALL_C,
    WALL_K,
};
use super::toggles::{no_wheel_slip, omsi_suspension};
use super::tyre::Contact;
use super::wheel::{sign, TyreFrame};
use glam::{DVec3, Vec3};
use omsi_vehicle::Vehicle;

/// The static load on each wheel of each axle (N): the weight shared so that the axle loads
/// balance the centre of gravity (a least-squares split when there are more than two axles).
pub fn wheel_rest_loads(def: &Vehicle) -> Vec<f32> {
    let mass = if def.mass < 100.0 { def.mass * 1000.0 } else { def.mass }.max(500.0);
    let cog_y = def.cog.map(|c| c[1]).unwrap_or(0.0);
    let g = GRAVITY;
    let n = (def.axles.len() * 2).max(1) as f32;
    let sy: f32 = def.axles.iter().map(|a| 2.0 * a.long).sum();
    let syy: f32 = def.axles.iter().map(|a| 2.0 * a.long * a.long).sum();
    let det = n * syy - sy * sy;
    let (a, b) = if det.abs() > 1e-6 {
        let rhs1 = mass * g;
        let rhs2 = mass * g * cog_y;
        ((rhs1 * syy - sy * rhs2) / det, (n * rhs2 - sy * rhs1) / det)
    } else {
        (mass * g / n, 0.0)
    };
    def.axles.iter().map(|x| (a + b * x.long).max(mass * g / n * MIN_LOAD_SHARE)).collect()
}

impl RigidBody {
    /// Each wheel on its strut - the strut's, the step's and the walls' forces added to `force`
    /// and `torque`, a wheel off the ground turning by its drive and brake - and the tyres on
    /// the ground (`found`: what each stands on) as contacts for the tyre forces.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn wheel_contacts(&mut self, h: f32, drive_torque: f32, driven: f32, brake: &[f32], found: &[Option<(f64, f32, f32)>], force: &mut Vec3, torque: &mut Vec3, probe: &dyn Fn(f64, f64, f64) -> GroundProbe) -> Vec<Contact> {
        let rot = self.orientation;
        let up = rot.mul_vec3(Vec3::Z);
        let n_wheels = self.wheels.len().max(1) as f32;
        let (position, cog) = (self.position, self.cog);
        let omega_world = rot.mul_vec3(self.omega);
        let mut contacts: Vec<Contact> = Vec::with_capacity(self.wheels.len());
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
            // The tyre pushes the wheel up, the strut pushes it down and the body up, and
            // the wheel's own mass moves between the two. The tyre carries the static
            // load already where it just touches, so that the ride height stays the
            // springs' alone (see the module's notes).
            let (tyre_f, tyre_static, n) = if omsi_suspension() {
                // OMSI 2's suspension: the body hangs straight on
                // the ground point under each wheel. The spring pushes by how far the
                // ground lies over the wheel's unloaded place, times `Axle_Springfactor`
                // and `achse_feder`; below it the wheel is in the air, pushes nothing and
                // the tyres stop holding. Pressed, the damper takes
                // `achse_daempfer` times the body's upward speed where the strut acts
                // (the roll rate at `achse_maxwidth` / 2), and the whole never
                // passes `achse_maxforce` - there is no wheel of its own mass,
                // no tyre and no bump stop between the road and the body. (Those, with a
                // tyre envelope smoothing the road, had the bus float over what it drove
                // on: "like a boat on the sea".)
                let k = w.spring * w.spring_factor.max(0.0);
                let top = hub0.z + (CLIMB * r) as f64;
                // (nothing under it: OMSI 2's ground query looks from 3 m over the model
                // origin's plane - a wheel that sank through a face at a joint of
                // two surfaces, or into a bridge deck, found nothing within reach, the
                // spring let go and the bus fell through the world)
                // The spring's point is where OMSI 2 puts it: on the model's
                // origin plane (z = 0) under the wheel, and its height over the ground is
                // measured straight up. Measured from the hub less the `.bus` file's tyre
                // radius instead, a mod whose tyre mesh is larger than that radius stood
                // with its wheels drawn sunk a few centimetres into the road.
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
                (if on { n.max(0.0) } else { 0.0 }, if on { n.max(0.0) } else { 0.0 }, n)
            } else {
                let m_w = (w.rest_load / GRAVITY * UNSPRUNG).max(MIN_WHEEL_MASS);
                let k = w.spring * w.spring_factor.max(0.05);
                // How far the ground under the tyre can rise within a substep: a tyre rolls up
                // a step along its envelope, never steeper than 2.4 (see CLIMB). A reading
                // that leaps higher (a kerb's edge or a surface object's rim caught for one
                // substep: +25 cm under the Urbino's front wheel at a Spandau kerb) is
                // climbed at that pace - taken at once it struck the wheel with 260 kN and
                // threw it into its arch.
                let v_along = (self.velocity + omega_world.cross((hub - self.position).as_vec3())).dot(fwd_h).abs();
                let max_rise = (v_along.max(RISE_MIN_SPEED) * ENVELOPE_SLOPE + RISE_SPARE) * h;
                let need = found[i].map(|(need, _, _)| if w.ground_seen { need.min(w.ground_z + r as f64 + max_rise as f64) } else { need });
                let touch = need.map(|need| ((need - hub0.z) / up.z.max(0.3) as f64) as f32);
                if let Some(need) = need {
                    w.ground_z = need - r as f64;
                    w.ground_seen = true;
                }
                let (tyre_f, tyre_static) = match touch {
                    // (a wheel that cannot reach the ground at full droop hangs in the air)
                    Some(t) if t >= -DROOP - DROOP_REACH => {
                        let pen = t - w.compression;
                        let closing = w.touch.map(|p| ((t - p) / h).clamp(-TYRE_CLOSING_MAX, TYRE_CLOSING_MAX)).unwrap_or(0.0) - w.compression_rate;
                        let s = w.tyre_k * pen + w.rest_load;
                        if s > 0.0 {
                            ((s + w.tyre_c * closing).clamp(0.0, w.max_force * TYRE_LOAD_MAX), s)
                        } else {
                            (0.0, 0.0)
                        }
                    }
                    _ => (0.0, 0.0),
                };
                w.touch = touch;
                // the strut: spring, a digressive damper (a kerb struck at speed must not fire
                // the body into the air) and the bump stop, a stiff rubber block; as a whole it
                // takes no more than three times the load it is built for (`achse_maxforce`)
                // (the spring only pushes: below its unloaded length the wheel hangs on the
                // damper, which also works on the rebound)
                // (OMSI: spring and damper together never pass more than `achse_maxforce`;
                // beyond it the bump stop, a rubber block of its own, takes over)
                let c = w.compression;
                let rate_c = w.compression_rate.clamp(-DAMPER_RATE_MAX, DAMPER_RATE_MAX);
                let bump = if c > BUMP { (w.spring * BUMP_STIFFNESS * (c - BUMP)).min(w.max_force * BUMP_LOAD_MAX) } else { 0.0 };
                let n = (k * c.max(0.0) + w.damper * rate_c).clamp(-w.max_force * REBOUND_LOAD_MAX, w.max_force) + bump;
                // the wheel moves against the body, which is itself accelerated by what holds
                // it up (`accel_body.z`, 9.81 m/s² standing, 0 in the air: a wheel off a kerb
                // drops, a wheel of a body in the air does not) - semi-implicit Euler, stable
                // with the stiff tyre at these steps
                w.compression_rate += ((tyre_f - n) / m_w - self.accel_body.z) * h;
                w.compression += w.compression_rate * h;
                if w.compression < -DROOP {
                    w.compression = -DROOP;
                    w.compression_rate = w.compression_rate.max(0.0);
                } else if w.compression > BUMP + BUMP_TRAVEL {
                    w.compression = BUMP + BUMP_TRAVEL;
                    w.compression_rate = w.compression_rate.min(0.0);
                }
                w.on_ground = tyre_f > 0.0;
                w.load = tyre_f;
                (tyre_f, tyre_static, n)
            };
            // the strut's force turns the body about its length from the outer width, not
            // from where the tyre stands (see `RigidWheel::lever`): body x cross body z
            *torque += Vec3::new(0.0, -w.lever * n, 0.0);
            // and OMSI's damper reads the body's speed out there too (roll rate
            // x maxwidth / 2): the part of it the hub does not see, while the tyre carries
            if tyre_f > 0.0 && !omsi_suspension() {
                torque.y -= (w.attach.x + w.lever) * w.damper * self.omega.y * w.lever;
            }
            let Some((_, slope, contact_dx)) = found[i].filter(|_| tyre_f > 0.0) else {
                // off the ground the wheel keeps its turning (a brake stops it)
                let (drive_w, brake_w) = (if w.driven { drive_torque / r / driven } else { 0.0 }, brake.get(i).copied().unwrap_or(0.0).max(0.0));
                let before = w.spin;
                w.spin += h * w.inertia_inv * (drive_w - sign(w.spin) * brake_w);
                if before != 0.0 && sign(before) != sign(w.spin) && brake_w > 0.0 {
                    w.spin = 0.0;
                }
                w.slipping = !no_wheel_slip();
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
            // already leans with it and adds nothing; the tyre's static load does the
            // pushing - the damping kick as the tyre meets the edge is taken by the
            // wheel's own mass.
            let normal = (Vec3::Z - fwd_h * slope).normalize();
            let along_up = normal.dot(up).max(0.3);
            let carried = tyre_static.min(w.max_force * STEP_LOAD_MAX);
            let step = (normal - up * normal.dot(up)) * (carried / along_up);
            w.step_force += step.dot(fwd_h);
            let contact = hub + (fwd_h * contact_dx.clamp(-r, r)).as_dvec3() - DVec3::Z * r as f64;
            let r_world = (contact - self.position).as_vec3();
            let v_point = self.velocity + omega_world.cross(r_world);
            let v_long = v_point.dot(fwd);
            // the tyre works with at most its rated load: the spike of a wheel landing on
            // its bump stop is not grip, and fed into the side force it spun a bus round
            // on a verge
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
        contacts
    }
}
