//! The physics' side of the scripts' interface, for every way a vehicle moves (its rigid
//! body, the kinematic model, the AI, the parts coupled behind it).
//!
//! What the scripts hand the physics each frame: `M_Wheel` (the drive torque at the driven
//! wheels, N m), the brake forces (`Brakeforce` for the whole vehicle,
//! `Axle_Brakeforce_<axle>_<side>` for each wheel, cleared once read) and the air springs'
//! `Axle_Springfactor_<axle>_<side>`. What the physics hands back, for the scripts' next
//! frame: `Velocity` and `Velocity_Ground` (km/h), `n_Wheel` (rpm), `A_Trans_*` (m/s²) and,
//! per wheel, `Wheel_Rotation` (rad), `Wheel_RotationSpeed` (rpm), `Axle_Steering` (rad) and
//! `Axle_Suspension` (m). The driver's controls go to `Throttle`, `Brake` and `Clutch`.

use super::*;

/// What the scripts left for the physics this frame.
pub(super) struct DriveInputs {
    /// `M_Wheel`, N m.
    pub(super) m_wheel: f32,
    /// Each wheel's brake force (N), in the order of the wheels (axle by axle, left first).
    pub(super) brakes: Vec<f32>,
}

/// One wheel as its variables show it: `Wheel_Rotation` (rad), `Wheel_RotationSpeed` (rpm),
/// `Axle_Steering` (rad) and `Axle_Suspension` (m).
pub(super) type WheelShown = [f32; 4];

/// `Wheel_Rotation`, `Wheel_RotationSpeed`, `Axle_Steering`, `Axle_Suspension` and
/// `Axle_Springfactor` of one wheel of a coupled part (`<axle>_L`, `<axle>_R`).
pub(super) type PartWheelVars = [Option<omsi_script::VarId>; 5];

impl VehicleInstance {
    /// The driver's controls into `Throttle`, `Brake` and `Clutch`, then `M_Wheel` and each
    /// wheel's brake force: its own and its share of the vehicle's (the rolling resistance's
    /// share is the physics' own).
    pub(super) fn take_drive_inputs(&mut self) -> DriveInputs {
        let c = self.physics.controls;
        self.put(self.v_throttle, c.throttle);
        self.put(self.v_brake, c.brake);
        self.put(self.v_clutch, c.clutch);
        let m_wheel = self.get(self.v_m_wheel);
        let n = (self.v_wheels.len() * 2).max(1) as f32;
        let shared = self.get(self.v_brakeforce).max(0.0) / n;
        let brakes: Vec<f32> = self
            .v_wheels
            .iter()
            .flat_map(|a| a.iter().map(|w| self.get(w[4]).max(0.0) + shared))
            .collect();
        // Read, the brake forces go back to 0, as OMSI clears them every frame before
        // the scripts run: a script sets them each frame it brakes. Kept,
        // a mod that brakes only inside an {if} (a retarder, a stop brake, its own physics)
        // stayed braked for good, and one that adds to its own value kept on growing.
        self.put(self.v_brakeforce, 0.0);
        for a in self.v_wheels.clone() {
            for w in a {
                self.put(w[4], 0.0);
            }
        }
        DriveInputs { m_wheel, brakes }
    }

    /// `Axle_Springfactor_<axle>_<side>` as the scripts left it.
    pub(super) fn spring_factor(&self, axle: usize, side: usize) -> f32 {
        if self.springs_full {
            return 1.0;
        }
        self.v_springfactor
            .get(axle)
            .and_then(|a| a[side])
            .map(|i| self.state.vars[i as usize])
            .unwrap_or(1.0)
    }

    /// The brake forces of a coupled part's wheels (its `v_brakes`), as its scripts left them.
    pub(super) fn part_brakes<'a>(&'a self, t: &'a TrailerPart) -> impl Iterator<Item = f32> + 'a {
        t.v_brakes.iter().map(|&id| self.get(id).max(0.0))
    }

    /// The spring factor of a coupled part's wheel (`vars`: its variables).
    pub(super) fn part_spring_factor(&self, vars: &PartWheelVars) -> f32 {
        vars[4].filter(|_| !self.springs_full).map(|i| self.state.vars[i as usize]).unwrap_or(1.0)
    }

    /// `Velocity` and `Velocity_Ground` (km/h).
    pub(super) fn show_velocity(&mut self, kmh: f32) {
        self.put(self.v_velocity, kmh);
        self.put(self.v_velocity_ground, kmh);
    }

    /// `n_Wheel` (rpm).
    pub(super) fn show_n_wheel(&mut self, rpm: f32) {
        self.put(self.v_n_wheel, rpm);
    }

    /// `A_Trans_*` (m/s²), which the rest of the game reads as `physics.a_trans` too.
    pub(super) fn show_a_trans(&mut self, a: Vec3) {
        self.physics.a_trans = a;
        self.put(self.v_accel[0], a.x);
        self.put(self.v_accel[1], a.y);
        self.put(self.v_accel[2], a.z);
    }

    /// `A_Trans_*` as OMSI has them for a body: the change of its velocity over the frame,
    /// turned into the body frame - its acceleration, not what an accelerometer reads, so 0
    /// standing or cruising. `accel_body` carries gravity's 9.81 m/s² (the wheels' springs
    /// need it), which as `A_Trans_Z` kept checks such as the NEOMAN ECAS's "|A_Trans_Z| < 3
    /// while driving" from ever passing. (Over OMSI's frames: the rattle scripts take its
    /// change from one frame to the next.)
    pub(super) fn body_a_trans(&mut self, rb: &crate::rigid::RigidBody, dt: f32) -> Vec3 {
        self.a_trans.push(scripts_acceleration(rb.accel_body, rb.orientation), dt)
    }

    /// The variables of wheel `side` (0 left, 1 right) of the vehicle's axle `axle`.
    pub(super) fn show_wheel(&mut self, axle: usize, side: usize, shown: WheelShown) {
        let Some(ids) = self.v_wheels.get(axle).map(|a| a[side]) else { return };
        for (id, x) in ids.into_iter().zip(shown) {
            self.put(id, x);
        }
    }

    /// The wheels' variables of a coupled part's own body (`vars`: each wheel's).
    pub(super) fn show_part_wheels(&mut self, rb: &crate::rigid::RigidBody, vars: &[PartWheelVars]) {
        for (i, (w, v)) in rb.wheels.iter().zip(vars).enumerate() {
            let shown = body_wheel_shown(rb, i, w);
            for (id, x) in v.iter().zip(shown) {
                if let Some(id) = id {
                    self.state.vars[*id as usize] = x;
                }
            }
        }
    }

    /// `Wheel_Rotation` and `Wheel_RotationSpeed` of the `count` axles from `first` on, for a
    /// coupled part that follows without a body of its own.
    pub(super) fn show_rolling_wheels(&mut self, first: usize, count: usize, rotation: f32, rpm: f32) {
        for a in 0..count {
            for side in ["L", "R"] {
                let k = first + a;
                if let Some(id) = self.ty.program.var(&format!("Wheel_Rotation_{k}_{side}")) {
                    self.state.vars[id as usize] = rotation;
                }
                if let Some(id) = self.ty.program.var(&format!("Wheel_RotationSpeed_{k}_{side}")) {
                    self.state.vars[id as usize] = rpm;
                }
            }
        }
    }
}

/// Wheel `i` (`w`) of a body as its variables show it.
pub(super) fn body_wheel_shown(rb: &crate::rigid::RigidBody, i: usize, w: &crate::rigid::RigidWheel) -> WheelShown {
    // `Axle_Suspension_*` is the wheel's travel *relative to the body*, and the stock
    // model.cfg moves the wheel down for a positive value (`origin_rot_y -90` +
    // `anim_trans`, checked with OMSI_DEBUG_ANIM on the SD200's front wheel). A compressed
    // spring means the body has come down towards the wheel, i.e. the wheel sits *up* in
    // its arch: the value is minus the compression. Handing over the compression itself
    // pushed every wheel down by twice its travel - into the road under braking, with the
    // body riding high above the arches. (Never below where the spring is unloaded: OMSI
    // hands over -clamp(travel, 0, maxforce / k) - a wheel in the air stays where it hangs
    // at rest.) The steering is each axle's own angle: OMSI turns every axle towards the
    // centre of the bend on the `[rot_pnt_long]` line - one angle for both sides.
    [w.rotation_deg.to_radians(), w.rpm, rb.axle_steer(i), -w.compression.max(0.0)]
}

/// `n_Wheel`: the rpm of the first driven wheel, or of the first wheel when none drives.
pub(super) fn driven_rpm<W>(wheels: &[W], driven: impl Fn(&W) -> bool, rpm: impl Fn(&W) -> f32) -> f32 {
    wheels.iter().find(|w| driven(w)).or(wheels.first()).map(rpm).unwrap_or(0.0)
}

/// Scripts often test a stopped bus with `!Velocity_Ground`, so do not expose tiny solver drift.
pub(super) fn script_speed(speed_kmh: f32) -> f32 {
    if speed_kmh.abs() < 0.01 { 0.0 } else { speed_kmh }
}

/// OMSI's frames, a thirtieth of a second (`[maxFPS]` 30 in its options.cfg and every option
/// preset but one): `A_Trans_*` is the body's velocity change over one of them,
/// and the stock rattle scripts (`klappern.osc`: `Klappern_Vol` follows how much |A_Trans|
/// changes from one frame to the next) were tuned on that. Taken over this game's frames -
/// 60 to 150 a second - the change from frame to frame was a half to a fifth of OMSI's for
/// the same jolt, and the buses kept quiet on rough roads (#772, #886). The value is the
/// mean over each thirtieth, held until the next one is complete.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct OmsiFrames {
    sum: Vec3,
    t: f32,
    out: Vec3,
}

impl OmsiFrames {
    const FRAME: f32 = 1.0 / 30.0;

    pub(super) fn push(&mut self, a: Vec3, dt: f32) -> Vec3 {
        if !(dt > 0.0) || !a.is_finite() {
            return self.out;
        }
        // (a frame as long as OMSI's or longer is one of OMSI's)
        if dt >= Self::FRAME * 0.99 {
            *self = OmsiFrames { out: a, ..Default::default() };
            return a;
        }
        self.sum += a * dt;
        self.t += dt;
        if self.t >= Self::FRAME * 0.99 {
            self.out = self.sum / self.t;
            self.sum = Vec3::ZERO;
            self.t = 0.0;
        }
        self.out
    }
}

/// The body-frame acceleration the scripts see as `A_Trans_*` (as OMSI computes it: the
/// velocity's change over the frame, rotated into the body): `accel_body`, the specific force
/// an accelerometer would read, less gravity's share in the body frame.
pub(super) fn scripts_acceleration(accel_body: Vec3, orientation: Quat) -> Vec3 {
    accel_body - orientation.inverse().mul_vec3(Vec3::new(0.0, 0.0, 9.81))
}
