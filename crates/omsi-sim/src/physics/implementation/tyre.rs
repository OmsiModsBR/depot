//! The tyres' forces over a substep: along the tyre the drive, the brakes and the hold of the
//! standing tyres (`RigidBody::prepare`); across it OMSI's holding or sliding, solved as an
//! impulse on each tyre (`RigidBody::lateral_start`, `lateral_pass`, `lateral_end`).

use super::ground::{track_walls, tyre_line};
use super::wheel::{sign, TyreFrame};
use crate::physics::domain::params::{
    AIR_DENSITY, CLIMB, DRAG_COEFFICIENT, FRONT_AREA_MAX, FRONT_AREA_MIN, GRAVITY, GRIPPED_ACROSS,
    GRIPPED_ALONG, HUB_REACH, MASS_PER_FRONT_AREA, REHOLD_SPEED, STANDING, WALL_HEIGHT,
};
use crate::physics::domain::{GroundProbe, Impact, RigidBody};
use glam::{Quat, Vec2, Vec3};

/// A wheel on the ground in one substep, before its tyre forces are settled.
pub(super) struct Contact {
    /// Contact point relative to the centre of gravity, tyre forward and right (world).
    pub(super) r: Vec3,
    pub(super) fwd: Vec3,
    pub(super) right: Vec3,
    /// Suspension, step and wall forces (world).
    pub(super) base: Vec3,
    pub(super) drive: f32,
    /// What the brake (and the rolling resistance) can hold.
    pub(super) brake: f32,
    pub(super) grip: f32,
    pub(super) v_long: f32,
    /// Takes side forces (a coupled part's joint does not).
    pub(super) lateral: bool,
    /// The wheel it is (None: a coupled part's joint).
    pub(super) wheel: Option<usize>,
}

/// What acts on a body in one substep (see `RigidBody::prepare`): its rotation and
/// heading at the start of it, the mass towed along it, and the forces (world) and torques
/// (body frame) but for the tyres' hold across, which `RigidBody::lateral_start` sets up.
#[derive(Default)]
pub struct Substep {
    pub(super) rot: Quat,
    pub(super) body_fwd: Vec3,
    pub(super) towed: f32,
    pub(super) force: Vec3,
    pub(super) torque: Vec3,
    pub(super) contacts: Vec<Contact>,
}

/// The tyres' hold across over a substep, solved as a constraint on each tyre: the body's
/// velocities as they would be at the end of it (`w` in the body frame), the impulse of
/// each tyre so far, and what a joint passed on to the body (`joint_p` world, `joint_l` body
/// frame; see [`Joint`](crate::physics::Joint)).
#[derive(Default)]
pub struct Lateral {
    pub(super) lat: Vec<usize>,
    pub(super) arms: Vec<(Vec3, f32)>,
    pub(super) j: Vec<f32>,
    pub(super) v: Vec3,
    pub(super) w: Vec3,
    pub(super) reach: f32,
    pub(super) gripped: bool,
    pub(super) slipping: bool,
    pub(super) m: f32,
    pub(super) mt: f32,
    pub(super) joint_p: Vec3,
    pub(super) joint_l: Vec3,
    pub(super) jointed: bool,
}

/// The buffers a body's substep works in besides its [`Substep`] and [`Lateral`], kept from
/// one substep to the next (see `SolverScratch`).
#[derive(Default)]
pub struct Work {
    found: Vec<Option<(f32, f32)>>,
    rolling_long: Vec<f32>,
    standing: Vec<usize>,
    hold_long: Vec<f32>,
}

/// The velocity an impulse `p` gives a body of mass `m` that tows `mt - m` along `fwd`.
pub fn lin(p: Vec3, fwd: Vec3, m: f32, mt: f32) -> Vec3 {
    (p - fwd * p.dot(fwd)) / m + fwd * (p.dot(fwd) / mt)
}

impl RigidBody {
    /// What acts on the body in one substep of `h` seconds, but for the tyres' hold across
    /// (see [`Self::lateral_start`]), into `s`: `driven`, the driven wheels of the whole train
    /// (each takes its share of `drive_torque`); `air`, whether the air acts on it (the leading
    /// body).
    #[allow(clippy::too_many_arguments)]
    pub(super) fn prepare(
        &mut self,
        h: f32,
        drive_torque: f32,
        driven: f32,
        brake: &[f32],
        air: bool,
        probe: &dyn Fn(f64, f64, f64) -> GroundProbe,
        s: &mut Substep,
        work: &mut Work,
    ) {
        let rot = self.orientation;
        let mut force = Vec3::new(0.0, 0.0, -GRAVITY * self.mass);
        // the air: ½ ρ cw A v² (cw 0.6, the front a bus's or a car's by its own mass, the
        // people aboard aside). Without it a bus rolling downhill without its brakes ran on
        // to any speed
        if air {
            let v = self.velocity;
            let area =
                (self.empty_mass / MASS_PER_FRONT_AREA).clamp(FRONT_AREA_MIN, FRONT_AREA_MAX);
            force -= v * v.length() * (0.5 * AIR_DENSITY * DRAG_COEFFICIENT * area);
        }
        let mut torque = Vec3::ZERO; // body frame
                                     // the towed mass moves with the body along its length
        let body_fwd = rot.mul_vec3(Vec3::Y);
        let towed: f32 = self.coupled.iter().map(|d| d.mass).sum();
        let (position, cog) = (self.position, self.cog);
        // What each tyre finds under and around it: how steeply the ground rises as the wheel
        // rolls on, where the tyre touches it, and the faces it cannot climb. Steps up to
        // CLIMB radii over the ground it stood on are ground; the first step after
        // `place` (which knows no ground) takes anything up to the hub, and so does a
        // tyre that finds nothing lower, to lift a body set down a little too low.
        let found = &mut work.found;
        found.clear();
        for i in 0..self.wheels.len() {
            let w = &self.wheels[i];
            let TyreFrame { fwd_h, hub, .. } = TyreFrame::of(w, rot, position, cog);
            let r = w.radius;
            let z_top = if w.ground_seen {
                w.ground_z + (CLIMB * r) as f64
            } else {
                hub.z + HUB_REACH
            };
            let wall_top = hub.z + WALL_HEIGHT;
            // faces are looked for as far ahead as the hub moves in a substep, so that
            // the first touch is already answered before the tyre is in the face
            let v_hub = self.velocity + rot.mul_vec3(self.omega).cross((hub - position).as_vec3());
            let look = Vec2::new(
                v_hub.dot(fwd_h).abs(),
                v_hub.dot(Vec3::new(fwd_h.y, -fwd_h.x, 0.0)).abs(),
            ) * h;
            let line = tyre_line(probe, hub, fwd_h, r, z_top, wall_top, look.x);
            let ground = match line.ground {
                None if w.ground_seen => {
                    tyre_line(probe, hub, fwd_h, r, hub.z + HUB_REACH, wall_top, look.x).ground
                }
                g => g,
            };
            let walls = if self.wheel_walls {
                track_walls(
                    probe, &w.walls, &line, hub, fwd_h, r, z_top, wall_top, look.y,
                )
            } else {
                Vec::new()
            };
            found.push(ground.map(|(_, slope, x)| (slope, x)));
            let w = &mut self.wheels[i];
            w.walls = walls;
            w.step_force = 0.0;
        }
        // A face stops the hub at once: whatever would close on it within the substep is
        // taken away by an impulse at the hub, as the body's own collisions do (and it is
        // a crash like theirs). A spring alone gave way - a front wheel met at 30 km/h
        // went through a platform's face, and once the hub was over the face nothing
        // pushed at all.
        for _ in 0..2 {
            for i in 0..self.wheels.len() {
                let TyreFrame { fwd_h, hub, .. } =
                    TyreFrame::of(&self.wheels[i], rot, position, cog);
                let r_hub = (hub - self.position).as_vec3();
                for f in 0..self.wheels[i].walls.len() {
                    let face = self.wheels[i].walls[f];
                    let n = face.normal.extend(0.0);
                    let reach = face.reach(fwd_h, self.wheels[i].radius);
                    let gap = face.distance(hub) - reach;
                    let v_hub = self.velocity + rot.mul_vec3(self.omega).cross(r_hub);
                    let closing = -v_hub.dot(n);
                    let excess = closing - gap.max(0.0) / h;
                    if excess <= 0.0 {
                        continue;
                    }
                    let j = excess / self.inv_mass_at(r_hub, n);
                    let before = self.kinetic_energy();
                    self.apply_impulse(r_hub, n * j);
                    // (a wheel struck: the tyres let go, as in OMSI 2)
                    self.holding = false;
                    let lost = (before - self.kinetic_energy()).max(0.0);
                    self.wheels[i].step_force += j / h * n.dot(fwd_h);
                    match self.wheel_impacts.iter_mut().find(|m| m.obstacle == i) {
                        Some(m) => {
                            m.speed = m.speed.max(closing);
                            m.energy += lost;
                        }
                        None => {
                            let point = rot.inverse().mul_vec3(r_hub - n * reach) + self.cog;
                            self.wheel_impacts.push(Impact {
                                point,
                                speed: closing,
                                energy: lost,
                                obstacle: i,
                                broke: false,
                                push: -n,
                            });
                        }
                    }
                }
            }
        }
        let contacts = &mut s.contacts;
        contacts.clear();
        self.wheel_contacts(
            h,
            drive_torque,
            driven,
            brake,
            &work.found,
            &mut force,
            &mut torque,
            probe,
            contacts,
        );
        let omega_world = rot.mul_vec3(self.omega);
        // The coupled parts, each a tyre at the joint that pushes and brakes along the
        // body's axis and takes no side force.
        let fwd_flat = Vec3::new(body_fwd.x, body_fwd.y, 0.0).normalize_or(Vec3::Y);
        for d in &self.coupled {
            let share = Vec3::new(d.dir.x, d.dir.y, 0.0)
                .normalize_or(fwd_flat)
                .dot(fwd_flat)
                .max(0.0);
            let r_world = rot.mul_vec3(d.point - self.cog);
            let cap = self.friction * d.driven_load;
            let drive = if d.driven_wheels > 0 {
                (drive_torque / d.radius.max(0.1) / driven * d.driven_wheels as f32)
                    .clamp(-cap, cap)
            } else {
                0.0
            };
            contacts.push(Contact {
                r: r_world,
                fwd: body_fwd,
                right: Vec3::ZERO,
                base: body_fwd * (-GRAVITY * d.mass * body_fwd.z),
                drive: drive * share,
                brake: d.brake.max(0.0) * share,
                grip: d.load,
                v_long: (self.velocity + omega_world.cross(r_world)).dot(body_fwd),
                lateral: false,
                wheel: None,
            });
        }
        // Tyre forces as OMSI has them.
        // Along the tyre: a rolling one passes on the drive and the brake, a slipping one
        // the sliding friction. Tyres that stand hold like static friction - but
        // together: they take up everything else that pushes the body (gravity on a
        // slope, the drive, the rolling ones) and what is left of its speed, shared by
        // what each brake can bear. One wheel at a time, each cancelling only its quarter
        // of the bus, a parked bus crept down an 8 % slope at 2 mm/s.
        let mut outer = force + contacts.iter().map(|c| c.base).sum::<Vec3>();
        let rolling_long = &mut work.rolling_long;
        rolling_long.clear();
        rolling_long.resize(contacts.len(), 0.0);
        for (k, c) in contacts.iter().enumerate() {
            // a slipping wheel passes on the sliding friction, whichever way the tyre
            // slides over the road (the wheel's own speed against the ground's)
            if let Some(w) = c.wheel.map(|i| &self.wheels[i]).filter(|w| w.slipping) {
                let mu_n = self.friction * c.grip;
                let slide = w.spin * w.radius - c.v_long;
                rolling_long[k] = mu_n * sign(slide);
                outer += c.fwd * rolling_long[k];
                continue;
            }
            if c.v_long.abs() > STANDING {
                rolling_long[k] = c.drive - c.brake * c.v_long.signum();
                outer += c.fwd * rolling_long[k];
            } else {
                outer += c.fwd * c.drive;
            }
        }
        let standing = &mut work.standing;
        standing.clear();
        standing.extend((0..contacts.len()).filter(|&k| {
            contacts[k].v_long.abs() <= STANDING
                && !contacts[k].wheel.is_some_and(|i| self.wheels[i].slipping)
        }));
        // (the standing tyres hold along the body, each by what its brake bears)
        let hold_long = &mut work.hold_long;
        {
            let (axis, mass, cap) = (body_fwd, self.mass + towed, |c: &Contact| c.brake);
            let need = -outer.dot(axis) - self.velocity.dot(axis) * mass / h;
            let total: f32 = standing.iter().map(|&k| cap(&contacts[k])).sum();
            hold_long.clear();
            hold_long.extend(standing.iter().map(|&k| {
                if total > 1e-3 {
                    (need * cap(&contacts[k]) / total).clamp(-cap(&contacts[k]), cap(&contacts[k]))
                } else {
                    0.0
                }
            }));
        }
        // (the tyres' pull along the body, for the pitch lever below)
        let mut long_sum = 0.0f32;
        for (k, c) in contacts.iter().enumerate() {
            let mut f_long = match standing.iter().position(|&j| j == k) {
                Some(si) => c.drive + hold_long[si],
                None => rolling_long[k],
            };
            let max_f = self.friction * c.grip;
            f_long = f_long.clamp(-max_f, max_f);
            let f_world = c.base + c.fwd * f_long;
            long_sum += f_long * c.fwd.dot(body_fwd);
            if let Some(i) = c.wheel {
                self.wheels[i].advance_spin(c, f_long, self.friction, h);
            }
            force += f_world;
            torque += rot
                .inverse()
                .mul_vec3(c.r)
                .cross(rot.inverse().mul_vec3(f_world));
        }
        // Driving and braking pitch the body about a lever longer than the centre of
        // gravity's height over the road: OMSI 2 takes the tyres' forces at the hubs, the
        // radius of the first axle's wheels more ((drive - brake) x d/2 besides
        // [schwerpunkt] x the pull), and only while the bus moves or its drive beats the
        // brakes: not while it stands with its brakes holding. (Standing is a speed of
        // exactly zero in OMSI; here it is every tyre standing, see `standing`.)
        let held = standing.len() == contacts.len()
            && contacts.iter().map(|c| c.drive).sum::<f32>().abs()
                <= contacts.iter().map(|c| c.brake).sum::<f32>();
        if !held {
            let r = self.wheels.first().map_or(0.0, |w| w.radius);
            torque.x += long_sum * r;
        }
        (s.rot, s.body_fwd, s.towed, s.force, s.torque) = (rot, body_fwd, towed, force, torque);
    }

    /// The tyres' hold across (see [`Self::lateral_pass`]) set up over the substep: the
    /// velocities the body would have without it, and whether the tyres hold or slide, into
    /// `l`. `steered`: the steering sets the bend the body turns on (the front section), not
    /// its joint (a section behind).
    pub(super) fn lateral_start(&mut self, s: &Substep, h: f32, l: &mut Lateral, steered: bool) {
        let (rot, body_fwd, towed, force, torque, contacts) =
            (s.rot, s.body_fwd, s.towed, s.force, s.torque, &s.contacts);
        let (m, mt) = (self.mass, self.mass + towed);
        let lin = |p: Vec3| lin(p, body_fwd, m, mt);
        let along = force.dot(body_fwd);
        let v = self.velocity + ((force - body_fwd * along) / m + body_fwd * (along / mt)) * h;
        let w =
            self.omega + (torque - self.omega.cross(self.inertia * self.omega)) / self.inertia * h;
        let lat = &mut l.lat;
        lat.clear();
        lat.extend((0..contacts.len()).filter(|&k| contacts[k].lateral && contacts[k].grip > 0.0));
        // (following its wheels: hardly any speed across the body)
        let gripped = self.velocity.dot(rot.mul_vec3(Vec3::X)).abs() < GRIPPED_ACROSS
            && self.velocity.dot(body_fwd).abs() > GRIPPED_ALONG;
        let arms = &mut l.arms;
        arms.clear();
        arms.extend(lat.iter().map(|&k| {
            let c = &contacts[k];
            let mut rn = rot
                .inverse()
                .mul_vec3(c.r)
                .cross(rot.inverse().mul_vec3(c.right));
            // (holding, no roll from the tyres' hold across: OMSI 2 then sets the
            // sideways motion outright and rolls the body by the bend's pull alone,
            // below - taken at the ground, every turn of the wheel kicked the body
            // over; sliding, the tyres trip it as before)
            if self.holding && gripped {
                rn.y = 0.0;
            }
            let inv = c.right.dot(lin(c.right)) + (rn / self.inertia).dot(rn);
            (rn, inv.max(1e-9))
        }));
        // holding: the bend's pull against the grip of all tyres, and
        // no wheel spinning or locked (the bend the body turns on this slice: the steering's
        // for the front section, its joint's for a section behind)
        let grip_all: f32 = lat.iter().map(|&k| self.friction * contacts[k].grip).sum();
        let v_fwd = self.velocity.dot(body_fwd);
        let kappa = self.kappa.abs();
        let slipping = self.wheels.iter().any(|w| w.slipping && w.on_ground);
        // (and a wheel in the air: OMSI 2 drops the holding state there)
        let airborne = self.wheels.iter().any(|w| !w.on_ground);
        // (and the body knocked sideways: holding, its point on the `[rot_pnt_long]` line
        // moves straight along it whatever the steering does, so speed across there is a
        // push from outside, which the tyres do not take outright)
        let on_line = rot.mul_vec3(Vec3::new(-self.cog.x, self.rot_pnt_long - self.cog.y, 0.0));
        let knocked = (v + rot.mul_vec3(w).cross(on_line))
            .dot(rot.mul_vec3(Vec3::X))
            .abs()
            > GRIPPED_ACROSS;
        // (what the tyres take across: the bend's pull, towards the side the body turns to,
        // less what a joint pushes it with - the rear section of an articulated bus pushes
        // the front one sideways at its back, and the tyres let go when they cannot take
        // that too)
        let right = rot.mul_vec3(Vec3::X);
        let inward = rot
            .mul_vec3(self.omega)
            .cross(self.velocity)
            .dot(right)
            .signum();
        let across = m * v_fwd * v_fwd * kappa * inward - self.joint_across;
        if self.holding && (across.abs() > grip_all || slipping || airborne || knocked) {
            self.holding = false;
        }
        // (holding, the tyres take whatever keeps them rolling where they point: the body
        // turns about the `[rot_pnt_long]` line at once, as in OMSI 2, which sets the yaw
        // rate v κ and the direction of the velocity outright; the bend's pull above lets
        // them go. A section behind turns as its joint bends it, and its tyres hold by what
        // the road takes: taking any force, the driven axle of a pusher's rear section held
        // its line whatever pushed it sideways and folded the bus in two on uneven ground)
        let reach = if self.holding && steered {
            f32::INFINITY
        } else {
            1.0
        };
        l.j.clear();
        l.j.resize(lat.len(), 0.0);
        (l.v, l.w, l.reach, l.gripped, l.slipping, l.m, l.mt) =
            (v, w, reach, gripped, slipping, m, mt);
        (l.joint_p, l.joint_l, l.jointed) = (Vec3::ZERO, Vec3::ZERO, false);
    }

    /// One pass of the tyres' hold across, as OMSI 2 has it (see `lateral_start`).
    pub(super) fn lateral_pass(&self, s: &Substep, l: &mut Lateral, h: f32) {
        let (rot, body_fwd, contacts) = (s.rot, s.body_fwd, &s.contacts);
        let Lateral {
            lat,
            arms,
            j,
            v,
            w,
            reach,
            m,
            mt,
            ..
        } = l;
        let (reach, m, mt) = (*reach, *m, *mt);
        let lin = |p: Vec3| lin(p, body_fwd, m, mt);
        for (q, &k) in lat.iter().enumerate() {
            let c = &contacts[k];
            let v_lat = (*v + rot.mul_vec3(*w).cross(c.r)).dot(c.right);
            let cap = self.friction * c.grip * h * reach;
            let before = j[q];
            j[q] = (before - v_lat / arms[q].1).clamp(-cap, cap);
            let dj = j[q] - before;
            *v += lin(c.right) * dj;
            *w += arms[q].0 / self.inertia * dj;
        }
    }

    /// The hold across (and a joint's pull, see [`Joint`](crate::physics::Joint)) as forces over the substep.
    pub(super) fn lateral_end(&mut self, s: &mut Substep, l: &Lateral, h: f32) {
        let (rot, contacts, v_fwd) = (s.rot, &s.contacts, self.velocity.dot(s.body_fwd));
        let Lateral {
            lat,
            arms,
            j,
            v,
            w,
            gripped,
            slipping,
            m,
            ..
        } = l;
        let (v, w, gripped, slipping, m) = (*v, *w, *gripped, *slipping, *m);
        let (force, torque) = (&mut s.force, &mut s.torque);
        for (q, &k) in lat.iter().enumerate() {
            *force += contacts[k].right * (j[q] / h);
            *torque += arms[q].0 * (j[q] / h);
        }
        if !self.holding
            && !slipping
            && lat.iter().all(|&k| {
                (v + rot.mul_vec3(w).cross(contacts[k].r))
                    .dot(contacts[k].right)
                    .abs()
                    < REHOLD_SPEED
            })
        {
            self.holding = true;
        }
        // the bend's pull rolls the body out of it ([schwerpunkt] x mass x
        // the centripetal acceleration)
        if self.holding && gripped && !lat.is_empty() {
            torque.y += m * self.cog.z * v_fwd * self.omega.z;
        }
        if l.jointed {
            s.force += l.joint_p / h;
            s.torque += l.joint_l / h;
        }
        self.joint_across = if l.jointed {
            (l.joint_p / h).dot(rot.mul_vec3(Vec3::X))
        } else {
            0.0
        };
    }
}
