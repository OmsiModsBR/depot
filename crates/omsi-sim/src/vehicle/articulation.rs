//! The parts coupled behind a vehicle: the rear sections of an articulated bus and a lorry's
//! trailers. Behind the player's vehicle (the one with a [`RigidBody`]) every road part is a
//! body of its own on the same wheel physics, with its own `.bus` file's mass, inertia,
//! centre of gravity, axles and springs, held to the part in front by its joint
//! (`crate::physics::Joint`, `[coupling_front_character]`), as OMSI 2 runs each section: what
//! the rear section weighs, brakes, drives and leans comes back to the front. The parts of the
//! AI's vehicles, of other players' over the LAN and of a train on rails follow kinematically,
//! hung on their coupling and pitched towards the ground under their axle.

use super::physics_bridge::PartWheelVars;
use super::trailer::{body_reversed, joint_angles, TrailerPart, ARTICULATION_SIGN};
use super::VehicleInstance;
use crate::physics::{BodyInputs, Joint, RigidBody};
use glam::{DVec3, Mat4, Vec3};
use omsi_script::Program;
use omsi_vehicle::Vehicle;

/// A part's own body and the variables its wheels are written to.
pub struct PartBody {
    pub(super) rb: RigidBody,
    /// Hung on its joint since the train was last put down (see [`TrailerPart::realign`]).
    pub(super) placed: bool,
    vars: Vec<PartWheelVars>,
}

impl PartBody {
    fn new(t: &TrailerPart, program: &Program) -> PartBody {
        // (its hubs from the `.bus` alone, as the lead body's - see `enable_rigid_body`)
        // (it turns about its own `[rot_pnt_long]` as the file gives it, 0 included, as
        // OMSI's sections do; the kinematic part's line, `TrailerPart::axle_long`, takes the
        // farthest axle for a 0)
        let rb = RigidBody::from_definition(&t.ty.def, &[]);
        let vars = (0..rb.wheels.len())
            .map(|i| {
                let (axle, side) = (t.first_axle + i / 2, ["L", "R"][i % 2]);
                [
                    "Wheel_Rotation",
                    "Wheel_RotationSpeed",
                    "Axle_Steering",
                    "Axle_Suspension",
                    "Axle_Springfactor",
                ]
                .map(|v| program.var(&format!("{v}_{axle}_{side}")))
            })
            .collect();
        PartBody {
            rb,
            placed: false,
            vars,
        }
    }
}

impl TrailerPart {
    /// The part's own body, when it has one (see the module's notes).
    pub fn rigid(&self) -> Option<&RigidBody> {
        self.body.as_ref().map(|b| &b.rb)
    }

    /// The part's body is to be hung on its joint again at the next step.
    pub(super) fn unplace(&mut self) {
        if let Some(b) = self.body.as_mut() {
            b.placed = false;
        }
    }

    /// The joint this part hangs on, as body `index` of the train behind body `index - 1`.
    fn joint(&self, index: usize) -> Joint {
        Joint::new(
            index - 1,
            index,
            &self.ty.def,
            self.coupling_back,
            self.coupling_front,
        )
    }
}

impl VehicleInstance {
    /// Whether the coupled parts run as bodies of their own (see the module's notes): road
    /// parts the right way round behind a road vehicle, all of them or none.
    fn part_bodies_wanted(&self) -> bool {
        !self.trailers.is_empty()
            && !self.ty.def.is_rail()
            && self.trailers.iter().all(|t| {
                !t.ty.def.is_rail()
                    && !body_reversed(&t.ty.def, t.reversed)
                    && !t.ty.def.axles.is_empty()
            })
    }

    /// One step of the vehicle's body (`lead`, taken out of its `Dynamics`) and the bodies of the
    /// parts coupled behind it - or of the body alone, the parts handing it what they hold
    /// along the joint (`physics::CoupledPart`) as they follow.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn step_bodies(
        &mut self,
        mut lead: RigidBody,
        dt: f32,
        m_wheel: f32,
        brakes: &[f32],
        steer: f32,
        probe: &dyn Fn(f64, f64, f64) -> crate::physics::GroundProbe,
        after_slice: &mut dyn FnMut(&mut [RigidBody], f32),
    ) -> RigidBody {
        lead.wheel_walls = self.wheel_walls;
        if !self.part_bodies_wanted() {
            lead.coupled = self.coupled_parts();
            if self.rest_loads_for != 0 {
                // the parts that were carried by the bodies are gone: the wheels carry their own again
                let loads = crate::physics::wheel_rest_loads(&self.ty.def);
                for (w, a) in lead.wheels.iter_mut().zip(&lead.wheel_axle) {
                    w.rest_load = loads.get(*a).copied().unwrap_or(w.rest_load);
                }
                self.rest_loads_for = 0;
            }
            crate::physics::step_train_with(
                std::slice::from_mut(&mut lead),
                &[],
                dt,
                m_wheel,
                &[BodyInputs {
                    brake: brakes,
                    steer: Some(steer),
                    air: true,
                }],
                probe,
                after_slice,
            );
            return lead;
        }
        lead.coupled.clear();
        let created = self.trailers.iter().any(|t| t.body.is_none());
        let ty = self.ty.clone();
        for t in self.trailers.iter_mut().filter(|t| t.body.is_none()) {
            t.body = Some(PartBody::new(t, &ty.program));
        }
        let joints: Vec<Joint> = self
            .trailers
            .iter()
            .enumerate()
            .map(|(i, t)| t.joint(i + 1))
            .collect();
        let parts: Vec<PartBody> = self
            .trailers
            .iter_mut()
            .map(|t| t.body.take().unwrap())
            .collect();
        let mut bodies = vec![lead];
        let mut meta = Vec::with_capacity(parts.len());
        for p in parts {
            bodies.push(p.rb);
            meta.push((p.placed, p.vars));
        }
        if created || self.rest_loads_for != self.trailers.len() {
            self.rest_loads_for = self.trailers.len();
            // the joints carry part of each part behind (from the front: each load the part in
            // front of it takes on its axles)
            let loads = crate::physics::wheel_rest_loads(&self.ty.def);
            let lead = &mut bodies[0];
            for (w, a) in lead.wheels.iter_mut().zip(&lead.wheel_axle) {
                w.rest_load = loads.get(*a).copied().unwrap_or(w.rest_load);
            }
            let defs: Vec<&Vehicle> = std::iter::once(&self.ty.def)
                .chain(self.trailers.iter().map(|t| &t.ty.def))
                .collect();
            crate::physics::support_rest_loads(&mut bodies, &joints, &defs);
        }
        // put down where the part was put (a bus placed on a bend has its rear section follow
        // the bend: `TrailerPart::place_pivot`) or straight behind - and so is one the vehicle
        // was taken away from without a word (a test run put back to its start)
        for (k, j) in joints.iter().enumerate() {
            if meta[k].0 && j.gap(&bodies) < 0.5 {
                continue;
            }
            let lead = &bodies[j.lead];
            let ball = lead.position
                + lead
                    .orientation
                    .mul_vec3(j.anchor_lead - lead.cog)
                    .as_dvec3();
            let (mut heading, _, _) = lead.heading_pitch_bank();
            if let Some(p) = self.trailers[k].pivot {
                let d = (ball - p).truncate();
                if d.length() > 1e-3 {
                    heading = d.x.atan2(d.y).to_degrees().rem_euclid(360.0);
                }
            }
            let mut headings = vec![0.0; bodies.len()];
            headings[j.follow] = heading;
            crate::physics::hang_followers(&mut bodies, std::slice::from_ref(j), &headings);
            meta[k].0 = true;
        }
        // each part: its own brakes and air springs, the road's grip
        let part_brakes: Vec<Vec<f32>> = self
            .trailers
            .iter()
            .map(|t| self.part_brakes(t).collect())
            .collect();
        let friction = bodies[0].friction;
        for (b, (_, vars)) in bodies[1..].iter_mut().zip(&meta) {
            for (w, v) in b.wheels.iter_mut().zip(vars) {
                w.spring_factor = self.part_spring_factor(v);
            }
            b.friction = friction;
            b.wheel_walls = self.wheel_walls;
        }
        let mut inputs = vec![BodyInputs {
            brake: brakes,
            steer: Some(steer),
            air: true,
        }];
        inputs.extend(part_brakes.iter().map(|b| BodyInputs {
            brake: b,
            steer: None,
            air: false,
        }));
        crate::physics::step_train_with(
            &mut bodies,
            &joints,
            dt,
            m_wheel,
            &inputs,
            probe,
            after_slice,
        );
        // a part's wheel stopped by a face is a crash of the vehicle (in its frame)
        let (front, parts) = bodies.split_at_mut(1);
        let front = &mut front[0];
        for b in parts.iter() {
            for hit in &b.wheel_impacts {
                let world = b.position + b.orientation.mul_vec3(hit.point - b.cog).as_dvec3();
                let point = front
                    .orientation
                    .inverse()
                    .mul_vec3((world - front.position).as_vec3())
                    + front.cog;
                front.wheel_impacts.push(crate::physics::Impact {
                    point,
                    obstacle: usize::MAX,
                    ..*hit
                });
            }
        }
        if omsi_cfg::flags::DEBUG_TRAILER.is_set() {
            for j in &joints {
                let (alpha, beta) = j.angles(&bodies);
                log::info!(
                    "joint {}: gap {:.3} m, alpha {:.1}, beta {:.1}",
                    j.follow - 1,
                    j.gap(&bodies),
                    alpha.to_degrees(),
                    beta.to_degrees()
                );
            }
        }
        let mut bodies = bodies.into_iter();
        let lead = bodies.next().unwrap();
        for (k, (rb, (placed, vars))) in bodies.zip(meta).enumerate() {
            let t = &mut self.trailers[k];
            t.position = rb.shown_origin();
            let (heading, pitch, bank) = rb.shown_heading_pitch_bank();
            (t.heading, t.pitch, t.bank) = (heading, pitch, bank);
            self.show_part_wheels(&rb, &vars);
            self.trailers[k].body = Some(PartBody { rb, placed, vars });
        }
        lead
    }
}

impl TrailerPart {
    /// The part this frame: where it stands - its body's pose, or hung on the part in front
    /// (`lead`: position, rotation and heading of the part in front; the vehicle itself for
    /// the first part) -, its joint's angles, and its animations from the leading vehicle's
    /// variables.
    pub(crate) fn update(
        &mut self,
        main: &mut VehicleInstance,
        dt: f32,
        lead: Option<(DVec3, Mat4, f64)>,
    ) {
        // coupling point in the world: on the leading part (the vehicle or the previous trailer)
        let (lead_pos, lead_rot, lead_heading) =
            lead.unwrap_or((main.position, main.body_rotation(), main.heading));
        let sprung = main.rigid().is_some() && self.body.as_ref().is_some_and(|b| b.placed);
        if sprung {
            self.follow_body();
        } else {
            self.follow(main, dt, lead_pos, lead_rot, lead_heading);
        }
        // The joint's angles (degrees) for its plates and bellows and for the scripts: alpha
        // about the vertical axis - the stock articulation.osc's jackknife protection brakes
        // at |alpha| > 47° - and beta about the transverse axis. (The horizontal angle went
        // to beta: the protection never engaged, the bellows turned in the wrong plane.)
        // Both are read off this part, not off the world: see `joint_angles`.
        let (alpha, beta) =
            joint_angles(lead_rot, lead_heading, self.body_rotation(), self.heading);
        if let Some(id) = self.v_alpha {
            main.state.vars[id as usize] = (alpha * ARTICULATION_SIGN) as f32;
        }
        if let Some(id) = self.v_beta {
            main.state.vars[id as usize] = beta as f32;
        }
        // (a body of its own has written its wheels' variables with its pose)
        if !sprung {
            // wheels of this part
            let rpm = main.motion().velocity_kmh()
                / 3.6
                / (2.0 * std::f32::consts::PI * self.wheel_radius)
                * 60.0;
            // radians, like every `Wheel_Rotation_*` (the degrees written here before spun the
            // rear section's wheels 57 times too fast: a flicker instead of a rolling wheel)
            let rot = (self.odometer / self.wheel_radius).rem_euclid(std::f32::consts::TAU);
            main.show_rolling_wheels(self.first_axle, self.axle_count, rot, rpm);
        }
        for (i, a) in self.animators.iter_mut().enumerate() {
            self.mesh_transforms[i] = a.update(dt, &main.state.vars);
        }
        crate::anim::apply_parents(&self.animators, &mut self.mesh_transforms);
        if omsi_cfg::flags::DEBUG_TRAILER.is_set() {
            for (i, m) in self.ty.meshes.iter().enumerate() {
                let file = &self.ty.model.meshes[m.def_index].file;
                if file.to_ascii_lowercase().contains("rad") {
                    log::info!(
                        "  {file}: pivot w {:?} transform w {:?}",
                        m.pivot.w_axis.truncate(),
                        self.mesh_transforms[i].w_axis.truncate()
                    );
                }
            }
        }
        self.props_plan.refresh(&self.ty, &main.var_index);
        self.props_plan
            .apply(&main.state.vars, &mut self.mesh_props);
    }

    /// The part hung on its coupling (`lead_*`: the part in front) and pitched towards the
    /// ground under its axle, on springs of its own when it shows them (see `update`).
    fn follow(
        &mut self,
        main: &mut VehicleInstance,
        dt: f32,
        lead_pos: DVec3,
        lead_rot: Mat4,
        lead_heading: f64,
    ) {
        let c = lead_pos + lead_rot.transform_point3(self.coupling_back).as_dvec3();
        // the height its axle had (the new one eases from it; nothing after a move)
        let prev_z = self.pivot.and(self.axle_z);
        let pivot = match self.pivot {
            Some(p) => p,
            None => {
                // start straight behind the leading vehicle
                let h = main.heading.to_radians();
                c - DVec3::new(h.sin(), h.cos(), 0.0) * self.length as f64
            }
        };
        let mut dir = (c - pivot).truncate();
        if dir.length() < 1e-3 {
            let h = main.heading.to_radians();
            dir = glam::DVec2::new(h.sin(), h.cos());
        }
        let mut dir = dir.normalize();
        let mut heading = dir.x.atan2(dir.y).to_degrees();
        // `[coupling_front_character]`: a bus joint (type != 0) stops hard at its max
        // alpha; the rear section's axle is dragged sideways there
        // instead of jackknifing through the part in front.
        if let Some([amax, _, _, kind]) = self.ty.def.coupling_front_character {
            if kind != 0.0 && amax > 0.0 {
                let a = amax as f64;
                let rel = ((lead_heading - heading + 540.0) % 360.0) - 180.0;
                if rel.abs() > a {
                    heading = lead_heading - rel.clamp(-a, a);
                    let h = heading.to_radians();
                    dir = glam::DVec2::new(h.sin(), h.cos());
                }
            }
        }
        self.heading = heading;
        let new_pivot = c - DVec3::new(dir.x, dir.y, 0.0) * self.length as f64;
        let ds = (new_pivot - pivot).truncate().length() as f32;
        self.odometer +=
            ds * (main.motion().velocity_kmh().signum().max(0.0) * 2.0 - 1.0).max(-1.0);
        self.pivot = Some(new_pivot);
        // Its springs: an AI copy sits `[ai_deltaheight]` low like its tractor; a driven one
        // as far down as the scripts' `Axle_Springfactor` lets the load press it.
        let ai = main.var("AI").map(|v| v > 0.5).unwrap_or(false);
        let shows = !self.ty.suspension_axles.is_empty();
        let mut sag = Vec::with_capacity(self.rest.len());
        for (a, (offset, load, k)) in self.rest.iter().enumerate() {
            let axle = self.first_axle + a;
            let compression = if ai {
                if shows {
                    -self.ty.def.ai_delta_height
                } else {
                    0.0
                }
            } else if self.ty.suspension_axles.contains(&axle) {
                let factor = main
                    .var(&format!("Axle_Springfactor_{axle}_L"))
                    .filter(|_| !main.springs_full)
                    .unwrap_or(1.0)
                    .max(0.05);
                (load / (k * factor)).min(crate::physics::BUMP)
            } else {
                0.0
            };
            for side in ["L", "R"] {
                if let Some(id) = main
                    .ty
                    .program
                    .var(&format!("Axle_Suspension_{axle}_{side}"))
                {
                    main.state.vars[id as usize] = -compression;
                }
            }
            sag.push(offset - compression);
        }
        let lift = sag.iter().sum::<f32>() as f64 / sag.len().max(1) as f64;
        self.ground_lift = lift as f32;
        // On rails: the track's height where it was put on it (the ground probe found the
        // platform edge or the embankment beside a bend, and the car jumped up and down).
        let on_track = self
            .track
            .filter(|t| (t.truncate() - new_pivot.truncate()).length() < 1.0)
            .map(|t| t.z);
        // the height of the part's origin over its axle (where the ground has none: level
        // with the coupling, as before)
        let level = c.z - self.coupling_front.z as f64;
        // the ground under its axle: what the wheels stand on where the world says, else the
        // plain height sampler
        let ground_z = match (on_track, &main.contact, &main.ground) {
            (Some(_), _, _) => None,
            (None, Some(g), _) => {
                // Looked for from above the coupling's level as well as from the part's own
                // height: from its own height alone, a rear section that had once dropped
                // under a viaduct's deck (a frame's step at the ramp, a gap at a joint) only
                // ever found the ground beneath and hung there under the bridge while the
                // front section drove on above (#135).
                let top = self.position.z.max(level) + 1.5;
                g.probe(new_pivot.x, new_pivot.y, top).below
            }
            (None, None, Some(g)) => g(new_pivot.x, new_pivot.y),
            _ => None,
        };
        // A height far from where the coupling holds the part is another level's: the AI's
        // ground lookup knows only x and y and gives the highest road there, which under a
        // bridge is the deck (or, on the deck, a road that runs on beneath it) - the trailer
        // of a lorry and the rear of an articulated bus stood up on the bridge or down under
        // it (#140). Level with the coupling instead. With the world's faces the part may
        // stand lower than the coupling on a grade, but never metres under it: that is the
        // road under a bridge seen through a gap in the deck (#135).
        let ground_z = ground_z.filter(|z| {
            if main.contact.is_some() {
                z + lift - level > -3.0
            } else {
                (z + lift - level).abs() < 1.5
            }
        });
        let axle_z = match on_track.or(ground_z.map(|z| z + lift)) {
            Some(z) if on_track.is_some() => z,
            Some(z) if main.contact.is_some() && dt > 0.0 => {
                // On the road the part stands on its springs as the part in front does in
                // OMSI 2 (each section is a body of its own on the same wheel springs): a
                // bump under its axle is a jolt that swings out, not a height eased into over
                // a sixth of a second, which smoothed every bump away under the rear of an
                // articulated bus. (Sprung at about 1.6 Hz, a little damped, as a bus body.)
                let from = prev_z.unwrap_or(z);
                if (z - from).abs() > 0.5 {
                    self.axle_vz = 0.0;
                    z
                } else {
                    let (w, zeta) = (2.0 * std::f64::consts::PI * 1.6, 0.35);
                    let h = (dt as f64).min(0.05);
                    let acc = w * w * (z - from) - 2.0 * zeta * w * self.axle_vz;
                    self.axle_vz = (self.axle_vz + acc * h).clamp(-3.0, 3.0);
                    from + self.axle_vz * h
                }
            }
            Some(z) => {
                // The sampled surface is not perfectly smooth (a centimetre of wobble along
                // the railway ballast every metre or two), and a car that follows every
                // sample shivers up and down. Ease towards it instead: a slope still comes
                // through within a fraction of a second, the wobble does not.
                let from = prev_z.unwrap_or(z);
                let dz = z - from;
                if dz.abs() > 0.5 {
                    z
                } else {
                    from + dz * (dt as f64 * 6.0).min(1.0)
                }
            }
            None => level,
        };
        self.axle_z = Some(axle_z);
        // The part hangs at the coupling in front and stands on its axle behind: its pitch is
        // the slope between them (it was drawn level at the axle's height, and on a grade or
        // a crest the joint came apart by a hand's breadth or more). It leans as the vehicle
        // it is coupled to does.
        // A pusher's joint (`[coupling_pitch_offset]`) pitches about a hinge on the part in
        // front, ahead of the ball it turns about: the slope runs from that hinge, which
        // stands the hinge's offset further from the axle (less as the joint turns).
        let d = self.pitch_offset as f64;
        let hinge = self.pitch_hinge(lead_pos, lead_rot);
        let run = if d > 0.0 {
            let alpha = ((lead_heading - heading + 540.0) % 360.0) - 180.0;
            self.length as f64 + d * alpha.to_radians().cos()
        } else {
            self.length as f64
        };
        let rise = hinge.z - (axle_z + self.coupling_front.z as f64);
        self.pitch = (rise.atan2(run.max(0.5)).to_degrees() as f32).clamp(-15.0, 15.0);
        self.bank = main.bank;
        // The ball rides the link between the hinge and this part, which pitches with this
        // part and turns with the part in front: `d` behind the hinge at this part's pitch.
        // At the pitch of the part in front that is the coupling point itself; anywhere else
        // the ball has swung up or down about the hinge, by about d·sin(beta).
        let ball = if d > 0.0 {
            let back = (c - hinge).truncate();
            let back = if back.length() > 1e-6 {
                back.normalize()
            } else {
                -dir
            };
            let p = (self.pitch as f64).to_radians();
            hinge + DVec3::new(back.x * p.cos() * d, back.y * p.cos() * d, -p.sin() * d)
        } else {
            c
        };
        // the trailer origin: coupling_front sits at the ball
        let rot = self.body_rotation();
        self.position = ball - rot.transform_point3(self.coupling_front).as_dvec3();
        // The wheels stand on the road under them, wherever the body above swings: the
        // travel of each wheel is the gap between its hub on the body and the ground under
        // it (OMSI 2 runs the section as a body on its own springs, each wheel's travel
        // its own). Held at the static sag, the rear axle of an articulated bus was a rigid
        // one - its wheels bounced and leant with the body over every bump and in every
        // bend (#901).
        if !ai && on_track.is_none() && dt > 0.0 && shows {
            self.spring_wheels(main, rot);
        }
        if omsi_cfg::flags::DEBUG_TRAILER.is_set() {
            log::info!(
                "trailer: rest {:?} sag {:?} lift {lift:.3} ground {ground_z:?} z {:.3}",
                self.rest,
                sag,
                self.position.z
            );
        }
    }

    /// The part as its body stands (the step wrote its pose): the plane its wheels touch,
    /// and its turning axle for the kinematic part it becomes when it is put down again.
    fn follow_body(&mut self) {
        let Some(b) = &self.body else { return };
        let rb = &b.rb;
        let standing = rb.wheels.iter().any(|w| w.on_ground);
        let touching: Vec<f32> = rb
            .wheels
            .iter()
            .filter(|w| w.on_ground || !standing)
            .map(|w| w.radius - w.attach.z - w.compression.max(-crate::physics::DROOP))
            .collect();
        self.ground_lift = touching.iter().sum::<f32>() / touching.len().max(1) as f32;
        self.pivot = Some(
            self.position
                + self
                    .body_rotation()
                    .transform_point3(Vec3::new(0.0, self.axle_long, 0.0))
                    .as_dvec3(),
        );
        self.axle_z = None;
    }

    /// `Axle_Suspension_*` of the part's sprung axles from the ground under each wheel, the
    /// body standing at `self.position` turned by `rot` (see `update`).
    fn spring_wheels(&self, main: &mut VehicleInstance, rot: Mat4) {
        let probe = |x: f64, y: f64, top: f64| -> Option<f64> {
            match (&main.contact, &main.ground) {
                (Some(g), _) => g.probe(x, y, top).below,
                (None, Some(g)) => g(x, y),
                _ => None,
            }
        };
        let mut travel: Vec<(usize, [Option<f32>; 2])> = Vec::new();
        for (a, (offset, _, _)) in self.rest.iter().enumerate() {
            let axle = self.first_axle + a;
            if !self.ty.suspension_axles.contains(&axle) {
                continue;
            }
            let Some(def) = self.ty.def.axles.get(a) else {
                continue;
            };
            let def = crate::physics::AxleSpec::of(def);
            let r = def.kinematic_radius;
            let hub = r - offset;
            let across = def.kinematic_track;
            let mut sides = [None, None];
            for (si, x) in [-across, across].into_iter().enumerate() {
                let p =
                    self.position + rot.transform_point3(Vec3::new(x, def.long, hub)).as_dvec3();
                let Some(g) = probe(p.x, p.y, p.z + 1.0) else {
                    continue;
                };
                // how far the wheel is pushed up into its arch (never below where it hangs
                // unloaded, never past the bump stop)
                sides[si] = Some(((g + r as f64 - p.z) as f32).clamp(0.0, crate::physics::BUMP));
            }
            travel.push((axle, sides));
        }
        for (axle, sides) in travel {
            for (side, c) in ["L", "R"].into_iter().zip(sides) {
                if let (Some(c), Some(id)) = (
                    c,
                    main.ty
                        .program
                        .var(&format!("Axle_Suspension_{axle}_{side}")),
                ) {
                    main.state.vars[id as usize] = -c;
                }
            }
        }
    }
}
