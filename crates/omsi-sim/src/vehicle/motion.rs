//! A vehicle's motion: the driver's controls, the physics step and the rigid body.

use super::physics_bridge::{body_wheel_shown, driven_rpm, script_speed, DriveInputs};
use super::trailer::{body_heading, TrailerPart};
use super::weather::road_grip;
use super::VehicleInstance;
use crate::physics::{Controls, Dynamics, KinematicModel, PhysicsState, VehicleMotion};
use glam::{DVec3, Vec3};

impl VehicleInstance {
    pub fn set_controls(&mut self, c: Controls) {
        self.controls = c;
    }

    /// The driver's inputs, as last set.
    pub fn controls(&self) -> Controls {
        self.controls
    }

    /// The vehicle's motion this frame, whichever model moves it (see [`VehicleMotion`]).
    pub fn motion(&self) -> &VehicleMotion {
        &self.motion
    }

    /// The vehicle's mass (kg) as the model that moves it has it: the rigid body's
    /// (`physics::mass_kg`), the kinematic model's (`physics::kinematic_mass_kg`).
    pub fn mass_kg(&self) -> f32 {
        match &self.dynamics {
            Dynamics::Rigid(rb) => rb.mass(),
            Dynamics::Kinematic(k) => k.mass_kg,
        }
    }

    /// The vehicle's rigid body, when it runs on one (the player's vehicle).
    pub fn rigid(&self) -> Option<&crate::physics::RigidBody> {
        match &self.dynamics {
            Dynamics::Rigid(rb) => Some(rb),
            Dynamics::Kinematic(_) => None,
        }
    }

    pub fn rigid_mut(&mut self) -> Option<&mut crate::physics::RigidBody> {
        match &mut self.dynamics {
            Dynamics::Rigid(rb) => Some(rb),
            Dynamics::Kinematic(_) => None,
        }
    }

    /// What each section carries (kg, the front one first, then each coupled part: the
    /// people aboard it), on the rigid bodies of the sections that have one (see
    /// [`crate::physics::RigidBody::set_payload`]). A section left out carries nothing.
    pub fn set_payload(&mut self, kg: &[f32]) {
        let at = |i: usize| kg.get(i).copied().unwrap_or(0.0);
        if let Some(rb) = self.rigid_mut() {
            rb.set_payload(at(0));
        }
        for (i, t) in self.trailers.iter_mut().enumerate() {
            t.set_payload(at(i + 1));
        }
    }

    /// The vehicle put at `at` facing `heading` (degrees), standing still: its rigid body set
    /// down there and every rear section lined up straight behind it.
    pub fn teleport(&mut self, at: glam::DVec3, heading: f64) {
        self.position = at;
        self.heading = heading;
        if let Some(rb) = self.rigid_mut() {
            rb.place(at, heading);
        }
        for t in self.trailers.iter_mut() {
            t.realign();
        }
    }

    /// The rigid body taken out for its step (`step_rigid` puts it back), if the vehicle has one.
    fn take_rigid(&mut self) -> Option<crate::physics::RigidBody> {
        match std::mem::replace(
            &mut self.dynamics,
            Dynamics::Kinematic(KinematicModel::default()),
        ) {
            Dynamics::Rigid(rb) => Some(rb),
            other => {
                self.dynamics = other;
                None
            }
        }
    }

    /// The vehicle's physics as it stands, to be put back with `set_physics_state`.
    pub fn physics_state(&self) -> PhysicsState {
        PhysicsState {
            dynamics: self.dynamics.clone(),
            motion: self.motion.clone(),
            controls: self.controls,
        }
    }

    pub fn set_physics_state(&mut self, state: PhysicsState) {
        self.dynamics = state.dynamics;
        self.motion = state.motion;
        self.controls = state.controls;
    }

    /// Each wheel's `Axle_Suspension` travel (m) for a copy another game drives, axle by axle
    /// and left first; wheels past the end of `travel` hang at 0.
    pub fn set_wheel_travel(&mut self, travel: impl IntoIterator<Item = f32>) {
        let mut travel = travel.into_iter();
        for axle in self.motion.wheels.iter_mut() {
            for w in axle.iter_mut() {
                w.suspension = travel.next().unwrap_or(0.0);
            }
        }
    }

    pub(super) fn get(&self, id: Option<omsi_script::VarId>) -> f32 {
        id.map(|i| self.state.vars[i as usize]).unwrap_or(0.0)
    }

    pub(super) fn put(&mut self, id: Option<omsi_script::VarId>, v: f32) {
        if let Some(i) = id {
            self.state.vars[i as usize] = v;
        }
    }

    /// Physics step: read the script's torque/brake outputs, integrate, write the motion
    /// variables back for the next script frame.
    /// Switch the player vehicle to rigid-body dynamics at its current pose.
    pub fn enable_rigid_body(&mut self) {
        // (its hubs from the `.bus` alone, a tyre radius over the model origin's plane where
        // OMSI's springs stand: neither the model's wheels nor their animation move them)
        let mut rb = crate::physics::RigidBody::from_definition(&self.ty.def, &[]);
        for (i, w) in rb.wheels.iter_mut().enumerate() {
            w.spring_factor = self.spring_factor(i / 2, i % 2);
        }
        rb.place(self.position, self.heading);
        self.dynamics = Dynamics::Rigid(rb);
    }

    /// The axle that steers: the frontmost, as the rigid body takes it (the simple model
    /// took the first listed, which is not always the front one).
    fn steered_axle(&self) -> usize {
        self.ty
            .def
            .axles
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.long.total_cmp(&b.1.long))
            .map(|(i, _)| i)
            .unwrap_or(0)
    }

    /// Set the forward speed (m/s), for tests that need a run-up without the road.
    pub fn set_speed(&mut self, v: f32) {
        self.motion.speed = v;
        let lead = match &mut self.dynamics {
            Dynamics::Rigid(rb) => Some(rb),
            Dynamics::Kinematic(_) => None,
        };
        let parts = self
            .trailers
            .iter_mut()
            .filter_map(|t| t.body.as_mut())
            .map(|b| &mut b.rb);
        for rb in lead.into_iter().chain(parts) {
            rb.velocity = rb.orientation.mul_vec3(Vec3::Y) * v;
        }
    }

    pub(crate) fn step_physics(&mut self, dt: f32) {
        let c = self.controls;
        let DriveInputs { m_wheel, brakes } = self.take_drive_inputs();
        let k = match self.dynamics {
            Dynamics::Kinematic(k) => k,
            Dynamics::Rigid(_) => {
                self.step_rigid(dt, m_wheel, &brakes, c.steering);
                return;
            }
        };
        let (ds, dheading) = k.step(&mut self.motion, c.steering, dt, m_wheel, &brakes);
        // move along the heading
        let prev = (self.position, self.heading);
        let h = self.heading.to_radians();
        self.position.x += h.sin() * ds as f64;
        self.position.y += h.cos() * ds as f64;
        self.heading = (self.heading + dheading as f64).rem_euclid(360.0);
        self.back_out_of_obstacles(k, prev);
        let (pitch_terrain, bank_terrain) = self.ground_angles(k, h);
        self.settle_body(k, dt, pitch_terrain, bank_terrain);
        self.show_drive_state();
    }
    /// Back out of the obstacles the body has just driven into, and stop there.
    fn back_out_of_obstacles(&mut self, k: KinematicModel, prev: (DVec3, f64)) {
        // collisions: back out of obstacles and stop
        if let (Some(cw), Some(bb)) = (&self.collision, self.ty.def.bounding_box) {
            let obb = crate::collision::Obb::from_box(bb, self.position, self.body_heading());
            // an obstacle we were already inside before this step (spawned on it, pushed into
            // it) never blocks: only entering an obstacle does
            let prev_obb = crate::collision::Obb::from_box(
                bb,
                prev.0,
                body_heading(&self.ty.def, prev.1, false),
            );
            let hit = cw
                .obstacles_near(&obb)
                .into_iter()
                .chain(self.dynamic_boxes.iter().copied())
                .find(|b| {
                    b.overlaps(&obb)
                        && !b.overlaps(&prev_obb)
                        && !(b.id >= 0 && self.knocked.contains(&b.id))
                });
            if let Some(hit) = hit {
                let v = self.motion.speed;
                if v.abs() > crate::physics::CRASH_SPEED {
                    let e = 0.5 * k.mass_kg * v * v;
                    let rel = hit.center - self.position.truncate();
                    let body_h = body_heading(&self.ty.def, prev.1, false).to_radians();
                    let (sh, ch) = (body_h.sin(), body_h.cos());
                    self.host.coll_pos = [
                        (rel.x * ch - rel.y * sh) as f32,
                        (rel.x * sh + rel.y * ch) as f32,
                        (crate::collision::impact_height(hit.z0.max(obb.z0), hit.z1.min(obb.z1))
                            - self.position.z) as f32,
                    ];
                    // kJ, like the rigid model reports it
                    self.host.coll_energy += e / 1000.0;
                    self.last_crash += e;
                    self.crashes += 1;
                    self.last_impact = e;
                    self.collided = true;
                }
                self.position = prev.0;
                self.heading = prev.1;
                self.motion.speed = 0.0;
            }
        }
    }

    /// The height, terrain pitch and bank the wheels on the ground give the body.
    fn ground_angles(&mut self, k: KinematicModel, h: f64) -> (f64, f64) {
        // ground under each wheel: height, terrain pitch and bank of the body
        let (mut z_sum, mut n_z) = (0.0, 0);
        let (mut zf, mut zr, mut zl, mut zrr, mut nf, mut nr, mut nl, mut nrr) =
            (0.0, 0.0, 0.0, 0.0, 0, 0, 0, 0);
        let (sh, ch) = (h.sin(), h.cos());
        let wheelbase = k.wheelbase as f64;
        let mut track = 2.0f64;
        if let Some(g) = &self.ground {
            for axle in &self.motion.wheels {
                for w in axle {
                    let (lat, long) = (w.lat as f64, w.long as f64);
                    track = track.max(w.lat.abs() as f64 * 2.0);
                    let wx = self.position.x + lat * ch + long * sh;
                    let wy = self.position.y - lat * sh + long * ch;
                    if let Some(z) = g(wx, wy) {
                        z_sum += z;
                        n_z += 1;
                        if long > 0.0 {
                            zf += z;
                            nf += 1;
                        } else {
                            zr += z;
                            nr += 1;
                        }
                        if lat < 0.0 {
                            zl += z;
                            nl += 1;
                        } else {
                            zrr += z;
                            nrr += 1;
                        }
                    }
                }
            }
            // the body sits down on its springs like the rigid one does
            let n_axles = self.rest_sag.len().max(1) as f32;
            let lift = self.rest_sag.iter().map(|(c, off)| off - c).sum::<f32>() / n_axles;
            if n_z > 0 {
                self.position.z = z_sum / n_z as f64 + lift as f64;
            } else if let Some(z) = g(self.position.x, self.position.y) {
                self.position.z = z + lift as f64;
            }
        }
        let pitch_terrain = if nf > 0 && nr > 0 {
            ((zf / nf as f64 - zr / nr as f64) / wheelbase)
                .atan()
                .to_degrees()
        } else {
            0.0
        };
        let bank_terrain = if nl > 0 && nrr > 0 {
            ((zl / nl as f64 - zrr / nrr as f64) / track)
                .atan()
                .to_degrees()
        } else {
            0.0
        };
        (pitch_terrain, bank_terrain)
    }

    /// The body follows the terrain and its own motion: the nose dives, the body leans and the
    /// springs take the weight transfer.
    fn settle_body(&mut self, k: KinematicModel, dt: f32, pitch_terrain: f64, bank_terrain: f64) {
        // dynamic body motion: nose dives when braking, leans out of curves
        let a_long = self.motion.accel.y;
        let curvature = self.motion.steer_deg.to_radians().tan() / k.wheelbase;
        let a_lat = self.motion.speed * self.motion.speed * curvature;
        let pitch_target = pitch_terrain as f32 - a_long * 0.45;
        let bank_target = bank_terrain as f32 - a_lat * 0.6;
        let k = (dt / 0.35).min(1.0);
        self.pitch += (pitch_target - self.pitch) * k;
        self.bank += (bank_target - self.bank) * k;
        // suspension travel per wheel (m): weight transfer from the same accelerations
        for (ai, axle) in self.motion.wheels.iter_mut().enumerate() {
            let rest = self.rest_sag.get(ai).map(|r| r.0).unwrap_or(0.0);
            for w in axle.iter_mut() {
                let front = if w.long > 0.0 { 1.0 } else { -1.0 };
                let right = if w.lat > 0.0 { 1.0 } else { -1.0 };
                // positive = the wheel drops below the body (see step_rigid): braking
                // compresses the front, so its wheels go up (negative)
                let target = -rest + a_long * 0.008 * front - a_lat * 0.01 * right;
                w.suspension += (target - w.suspension) * k;
            }
        }
    }

    /// The wheels, the drive and the body as the scripts see them.
    fn show_drive_state(&mut self) {
        self.show_velocity(script_speed(self.motion.velocity_kmh()));
        self.show_n_wheel(driven_rpm(
            &self.motion.wheels,
            |w| w[0].driven,
            |w| w[0].rpm,
        ));
        self.show_a_trans(self.motion.accel);
        // Wheel_Rotation_* and Axle_Steering_* are radians in the original: the stock
        // model.cfg turns a wheel with anim_rot ... 57.29577951308232 (180/pi) and the
        // steering wheel with 1450 (SD200) or 1680 (SD202), roughly two and a half turns
        // from lock to lock. Positive is to the right, as here - checked by looking
        // straight down on the front wheel alone (OMSI_ONLY_MESH=SD_Rad_VL) and at the
        // steering wheel in the cab, which turn together with this sign.
        let steer = self.motion.steer_deg.to_radians();
        for ai in 0..self.v_wheels.len() {
            for si in 0..2 {
                let ws = self.motion.wheels[ai][si].clone();
                let shown = [
                    ws.rotation_deg.to_radians(),
                    ws.rpm,
                    if ai == self.steered_axle() {
                        steer
                    } else {
                        0.0
                    },
                    ws.suspension,
                ];
                self.show_wheel(ai, si, shown);
            }
        }
    }

    /// The coupled parts as the leading body feels them at its rear coupling (each along
    /// the direction of the part hung on that coupling; the parts follow kinematically and
    /// pass on what acts on them): drive, brakes, rolling resistance and mass.
    pub(super) fn coupled_parts(&self) -> Vec<crate::physics::CoupledPart> {
        let Some(first) = self.trailers.first() else {
            return Vec::new();
        };
        // everything behind comes through the first coupling (the chain passes it on), but
        // each part pulls or holds back along its own heading: the third section of a
        // double articulated bus in a bend does not point where the second does
        let dir_of = |t: &TrailerPart| {
            let h = if t.pivot.is_some() {
                t.heading
            } else {
                self.heading
            }
            .to_radians();
            Vec3::new(h.sin() as f32, h.cos() as f32, 0.0)
        };
        self.trailers
            .iter()
            .map(|t| {
                let dir = dir_of(t);
                let def = &t.ty.def;
                let driven: Vec<usize> = def
                    .axles
                    .iter()
                    .enumerate()
                    .filter(|(_, a)| a.driven)
                    .map(|(i, _)| i)
                    .collect();
                let loads = crate::physics::wheel_rest_loads(def);
                let wheel_load = |a: usize| loads.get(a).copied().unwrap_or(0.0) * 2.0;
                crate::physics::CoupledPart {
                    point: first.coupling_back,
                    dir,
                    mass: crate::physics::mass_kg(def),
                    driven_wheels: driven.len() * 2,
                    radius: driven
                        .first()
                        .map(|&a| crate::physics::AxleSpec::of(&def.axles[a]).radius)
                        .unwrap_or(0.5),
                    driven_load: driven.iter().map(|&a| wheel_load(a)).sum(),
                    brake: self.part_brakes(t).sum::<f32>()
                        + crate::physics::coupled_rolling_resistance(def),
                    load: (0..def.axles.len()).map(wheel_load).sum(),
                }
            })
            .collect()
    }

    fn step_rigid(&mut self, dt: f32, m_wheel: f32, brakes: &[f32], steer: f32) {
        let mut rb = self.take_rigid().unwrap();
        // what the body ran into this frame, and what it was
        let mut hits: Vec<(crate::physics::Impact, crate::collision::Obb)> = Vec::new();
        // the tyres' grip on the road as it is this frame (set after the step, it took hold a
        // frame late: the first frame on a frozen road still had the dry road's grip)
        rb.friction = road_grip(self.host.street_cond, self.host.temperature);
        for (i, w) in rb.wheels.iter_mut().enumerate() {
            w.spring_factor = self.spring_factor(i / 2, i % 2);
        }
        rb = self.collide_and_step(rb, dt, m_wheel, brakes, steer, &mut hits);
        let (mut energy, mut worst) = Self::wheel_crashes(&rb);
        self.record_hits(&hits, &mut energy, &mut worst);
        self.apply_crash(energy, worst);
        self.show_rigid_state(&rb, dt);
        self.dynamics = Dynamics::Rigid(rb);
    }

    /// The body driven through the step, the obstacles it runs into answered in slices.
    fn collide_and_step(
        &mut self,
        mut rb: crate::physics::RigidBody,
        dt: f32,
        m_wheel: f32,
        brakes: &[f32],
        steer: f32,
        hits: &mut Vec<(crate::physics::Impact, crate::collision::Obb)>,
    ) -> crate::physics::RigidBody {
        let ground = self.ground.clone();
        let flat = |_: f64, _: f64, _: f64| crate::physics::GroundProbe {
            below: Some(0.0),
            above: None,
        };
        let from_ground = |x: f64, y: f64, top: f64| {
            let z = ground.as_ref().and_then(|g| g(x, y));
            match z {
                Some(z) if z > top => crate::physics::GroundProbe {
                    below: None,
                    above: Some(z),
                },
                z => crate::physics::GroundProbe {
                    below: z,
                    above: None,
                },
            }
        };
        // one session for the whole step: the tyres ask for a few hundred points close
        // together, and it looks the tiles under them up once
        let contact = self.contact.clone();
        let session = contact.as_ref().map(|c| c.session());
        let probe: &dyn Fn(f64, f64, f64) -> crate::physics::GroundProbe = match (&session, &ground)
        {
            (Some(s), _) => s.as_ref(),
            (None, Some(_)) => &from_ground,
            (None, None) => &flat,
        };
        // Obstacles: the static boxes around the vehicle and the moving ones, answered
        // with impulses after each slice of the step, with the time it covered (once a
        // frame, with the frame's time however long, a long frame took a box the bus had
        // only just reached for one it had stood in all along). The old answer - put the
        // bus back where it was and take its speed - held it against whatever it touched:
        // it could not slide along a wall or turn away, and every frame of pushing counted
        // as another crash.
        let bb = self.ty.def.bounding_box;
        let collision = self.collision.clone();
        let dynamic = std::mem::take(&mut self.dynamic_boxes);
        let mut knocked = self.knocked.clone();
        let mut collide = |bodies: &mut [crate::physics::RigidBody], slice: f32| {
            let Some(bb) = bb else { return };
            let rb = &mut bodies[0];
            let me = rb.body_box(bb);
            let mut obstacles: Vec<crate::collision::Obb> = Vec::new();
            if let Some(cw) = &collision {
                let mut probe_box = me;
                probe_box.half += glam::DVec2::splat(1.0);
                // the faces of collision meshes only where they are in the body as it
                // stands: pitched on a ramp, the plan-view box reaches down to its lowest
                // corner all along its length and ran into the ramp it drove on
                let solid = rb.body_solid(bb, 0.05);
                obstacles.extend(cw.obstacles_near_solid(&probe_box, Some(&solid)));
            }
            obstacles.extend(dynamic.iter().copied());
            let impacts = rb.collide(
                bb,
                &obstacles,
                &|i| obstacles[i].id >= 0 && knocked.contains(&obstacles[i].id),
                slice,
            );
            for hit in impacts {
                let o = obstacles[hit.obstacle];
                if hit.broke && o.id >= 0 && !knocked.contains(&o.id) {
                    knocked.push(o.id);
                }
                // (one hit per obstacle a frame, the hardest, as a single slice gives)
                match hits.iter_mut().find(
                    |(_, p): &&mut (crate::physics::Impact, crate::collision::Obb)| {
                        o.id != -1 && p.id == o.id
                    },
                ) {
                    Some((h, _)) if h.energy >= hit.energy => {}
                    Some(x) => *x = (hit, o),
                    None => hits.push((hit, o)),
                }
            }
        };
        // (with the parts coupled behind it: see `articulation`)
        rb = self.step_bodies(rb, dt, m_wheel, brakes, steer, probe, &mut collide);
        self.dynamic_boxes = dynamic;
        rb
    }

    /// The energy and the worst impact of the wheels stopped by a face they cannot climb.
    fn wheel_crashes(rb: &crate::physics::RigidBody) -> (f32, Option<crate::physics::Impact>) {
        let mut energy = 0.0f32;
        let mut worst: Option<crate::physics::Impact> = None;
        // a wheel stopped by a face it cannot climb (a platform, a step in the road)
        for hit in &rb.wheel_impacts {
            if hit.speed < crate::physics::CRASH_SPEED || hit.energy < 1000.0 {
                continue;
            }
            energy += hit.energy;
            if worst.map(|w| hit.energy > w.energy).unwrap_or(true) {
                worst = Some(*hit);
            }
        }
        (energy, worst)
    }

    /// Record what the body ran into this step: the knocked-down obstacles, the crash energy
    /// and the worst impact.
    fn record_hits(
        &mut self,
        hits: &[(crate::physics::Impact, crate::collision::Obb)],
        energy: &mut f32,
        worst: &mut Option<crate::physics::Impact>,
    ) {
        let touching: Vec<i64> = hits
            .iter()
            .map(|(_, o)| o)
            .filter(|o| o.mass > 0.0)
            .map(|o| o.id)
            .collect();
        for (hit, o) in hits {
            if hit.broke && o.id >= 0 && !self.knocked.contains(&o.id) {
                self.knocked.push(o.id);
                self.knocked_now.push((o.id, hit.push.as_dvec3()));
            }
            // a brush that takes less than a kilojoule is no accident (a broken post is), and
            // a car still inside the bus from the frame before is the same accident
            if hit.speed >= crate::physics::CRASH_SPEED && omsi_cfg::flags::DEBUG_PHYSICS.is_set() {
                log::info!("  hit obstacle {} at ({:.1}, {:.1}) z {:.2}..{:.2} half {:.2}x{:.2} heading {:.0}, {:.1} km/h", o.id, o.center.x, o.center.y, o.z0, o.z1, o.half.x, o.half.y, o.heading.to_degrees(), hit.speed * 3.6);
            }
            if hit.speed < crate::physics::CRASH_SPEED
                || (hit.energy < 1000.0 && !hit.broke)
                || (o.mass > 0.0 && self.touching.contains(&o.id))
            {
                continue;
            }
            *energy += hit.energy;
            if worst.map(|w| hit.energy > w.energy).unwrap_or(true) {
                *worst = Some(*hit);
            }
        }
        if self.ty.def.bounding_box.is_some() {
            self.touching = touching;
        }
    }

    /// The crash of this step, when it was a hard one: the host sees its point and energy.
    fn apply_crash(&mut self, energy: f32, worst: Option<crate::physics::Impact>) {
        if let Some(hit) = worst {
            if omsi_cfg::flags::DEBUG_PHYSICS.is_set() {
                log::info!(
                    "impact at {:.1} km/h, {:.1} kJ, body point ({:.2}, {:.2}, {:.2}){}",
                    hit.speed * 3.6,
                    energy / 1000.0,
                    hit.point.x,
                    hit.point.y,
                    hit.point.z,
                    if hit.broke { ", a post broke off" } else { "" }
                );
            }
            // `coll_energy` is in kJ: the stock and mod collision scripts compare it with
            // 100-1000 for the electrics and 1-10 for a lamp - in joules a bump at
            // walking pace took the electrics out
            self.host.coll_pos = hit.point.to_array();
            self.host.coll_energy += energy / 1000.0;
            self.last_crash += energy;
            self.last_impact = energy;
            self.crashes += 1;
            self.collided = true;
        }
    }

    /// The rigid body as the scripts and the picture see it.
    fn show_rigid_state(&mut self, rb: &crate::physics::RigidBody, dt: f32) {
        // (shown between its last two substeps, as far as the time not yet stepped reaches:
        // smooth at any frame rate, a substep behind the physics at most)
        self.position = rb.shown_origin();
        let (heading, pitch, bank) = rb.shown_heading_pitch_bank();
        self.heading = heading;
        self.pitch = pitch;
        self.bank = bank;
        // script interface
        let speed = rb.forward_speed();
        self.motion.speed = speed;
        self.motion.steer_deg = rb.steer_deg;
        self.motion.accel = rb.accel_body;
        self.show_velocity(script_speed(speed * 3.6));
        self.show_n_wheel(driven_rpm(&rb.wheels, |w| w.driven, |w| w.rpm));
        let a = self.body_a_trans(&rb, dt);
        self.show_a_trans(a);
        for ai in 0..self.v_wheels.len() {
            for si in 0..2 {
                if let Some(rw) = rb.wheels.get(ai * 2 + si) {
                    let shown = body_wheel_shown(&rb, ai * 2 + si, rw);
                    self.show_wheel(ai, si, shown);
                    if let Some(ws) = self.motion.wheels.get_mut(ai).and_then(|a| a.get_mut(si)) {
                        ws.rotation_deg = rw.rotation_deg;
                        ws.rpm = rw.rpm;
                        ws.suspension = shown[3];
                    }
                }
            }
        }
    }

    /// The bus's total mileage, as OMSI writes it into a situation:
    /// the starting reading plus the kilometres driven since it was loaded.
    pub fn odometer_km(&self) -> f64 {
        self.host.km_base + self.driven_km
    }

    /// Continue a saved reading without giving the bus another random service history.
    /// OMSI clears driven km and restores the total into the starting
    /// reading; subtract our already driven distance to keep the same total
    /// even when the caller has ticked the vehicle before restoring its situation.
    pub fn set_odometer_km(&mut self, km: f64) {
        self.host.km_base = km - self.driven_km;
        self.km_started = true;
    }
}
