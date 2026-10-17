//! Driving a vehicle the AI moves: the motion it is handed each frame.

use super::physics_bridge::driven_rpm;
use super::VehicleInstance;
use crate::physics::{AI_AXLE_STEER_MAX, AI_STEER_ARM_MIN};

/// Motion state handed to `VehicleInstance::update_ai`.
#[derive(Debug, Clone, Copy, Default)]
pub struct AiFrame {
    /// m/s
    pub speed: f32,
    /// Distance travelled so far (m), drives the wheel rotation.
    pub odometer: f32,
    /// Front wheel angle of the bicycle model through the `[rot_pnt_long]` line (deg,
    /// positive = right); each wheel's own angle follows from it.
    pub steer_deg: f32,
    /// 0 none, 1 left, 2 right, 3 both (hazard warning).
    pub blinker: i32,
    pub brake: bool,
    pub lights: bool,
    /// `AI_Scheduled_AtStation` as OMSI hands it to the script: 1 = standing at a stop, open
    /// the doors; -1 = time to go, close them (the script answers with 0 once they are
    /// shut); 0 = not at a stop (a script still closing its doors is told -1 until it
    /// answers, see `VehicleInstance::station_released`).
    pub at_station: i32,
    /// `AI_Scheduled_AtStation_Side`: which side's doors a bus standing at its stop opens -
    /// 0 = the side the map lays its road on, 1 = the other, 2 = both. AiList vehicles whose
    /// model has doors on both sides read it (Urumqi61's `[AI]YoungMan*`: the BRT platforms
    /// lie left, the ordinary stops right) and a script without the variable opens the right
    /// side, which is OMSI's default too. 0 when the vehicle is not at a stop.
    pub at_station_side: f32,
    /// `TrafficPriorityWarningNeeded`: a vehicle with right of way (`TrafficPriority`) has
    /// something in its way that is to be warned - the stock ambulance's script sounds its
    /// siren for the next 30 m on it.
    pub priority_warning: bool,
    /// `AI_Engine` -1: a timetable bus standing at a stop for its departure with time to
    /// spare switches its engine off (the stock AI scripts: engine off, parking brake on),
    /// and on again 20 s before it leaves.
    pub engine_off: bool,
}

impl VehicleInstance {
    /// One frame of an AI-controlled vehicle: the pose is set by the traffic system
    /// (`ai::AiBody`, which also leaves the accelerations and each wheel's travel in
    /// `physics`), the scripts see the motion through `Velocity` and the `AI_*` variables and
    /// run their `{frame_ai}` blocks (falling back to `{frame}` when a script has none).
    pub fn update_ai(&mut self, dt: f32, ai: &AiFrame) {
        self.update_ai_with(dt, ai, &[], &[]);
    }

    /// `update_ai` for the copy of a vehicle another game drives (LAN play): `inputs` are
    /// written after the `AI_*` variables (so they may override `AI_Engine` or `AI_Light`)
    /// and before the scripts run; `pinned` are written before and once more after the
    /// scripts, so the animations, lamps and sounds show the other game's values whatever
    /// this copy's AI scripts made of them. The odometer may run backwards (reversing).
    pub fn update_ai_with(
        &mut self,
        dt: f32,
        ai: &AiFrame,
        inputs: &[(omsi_script::VarId, f32)],
        pinned: &[(omsi_script::VarId, f32)],
    ) {
        self.host.clock.advance(dt);
        self.set_engine_var("schedule_active", self.host.schedule_active);
        self.motion.speed = ai.speed;
        self.motion.steer_deg = ai.steer_deg;
        self.show_velocity(ai.speed * 3.6);
        // how far the vehicle rolled since the last frame (a reset or a jump counts as none;
        // backwards only for a vehicle that reverses)
        let ds = ai.odometer - self.ai_odometer;
        self.ai_odometer = ai.odometer;
        let ds = if (0.0..50.0).contains(&ds) || (ai.speed < 0.0 && ds > -50.0 && ds < 0.0) {
            ds
        } else {
            0.0
        };
        // `steer_deg` is the front wheel angle of a bicycle model turning about the
        // `[rot_pnt_long]` line: every axle ahead of that line points at the common turning
        // centre, and every wheel rolls as far as its own track through the bend is long
        let (rot, wheelbase) = crate::ai::rotation_point(&self.ty.def);
        let k = ai.steer_deg.to_radians().tan() / wheelbase;
        for ai_idx in 0..self.v_wheels.len() {
            for si in 0..2 {
                let Some(ws) = self.motion.wheels.get_mut(ai_idx).map(|a| &mut a[si]) else {
                    continue;
                };
                let arm = ws.long - rot;
                let across = 1.0 - ws.lat * k;
                // (the axle's angle, the same for both sides, as OMSI hands it to the
                // scripts: see `RigidBody::axle_steer`)
                let steer = if arm > AI_STEER_ARM_MIN {
                    (arm * k).atan().clamp(-AI_AXLE_STEER_MAX, AI_AXLE_STEER_MAX)
                } else {
                    0.0
                };
                let travel = (across * across + (arm * k) * (arm * k)).sqrt();
                let r = ws.radius.max(0.05);
                ws.rotation_deg =
                    (ws.rotation_deg + (ds * travel / r).to_degrees()).rem_euclid(360.0);
                ws.rpm = ai.speed * travel / (std::f32::consts::TAU * r) * 60.0;
                // (the suspension: the travel `AiBody` works out on its springs, about the
                // static sag of an AI copy - `ai_rest_offset`, already in it)
                let shown = [ws.rotation_deg.to_radians(), ws.rpm, steer, ws.suspension];
                self.show_wheel(ai_idx, si, shown);
            }
        }
        self.show_n_wheel(driven_rpm(&self.motion.wheels, |w| w[0].driven, |w| w[0].rpm));
        self.show_a_trans(self.motion.accel);
        // `AI` is the engine's "this vehicle is driven by the AI" flag; the police and
        // ambulance script sounds the siren whenever `AI` is 0 and the car rolls, which is
        // why every spawn came with an ambulance nobody could see
        // `AI_Scheduled_AtStation`: 1 while the bus serves a stop (the door scripts open the
        // doors), then -1 when it wants to leave - the stock `door_X10_AI.osc` closes the
        // doors on -1, releases the stop brake and only then writes 0, which is the signal
        // that the bus may move (`VehicleInstance::station_released`). Writing 0 straight
        // away left every timetable bus driving off with its doors open. A frame that says
        // nothing (0) keeps telling a script that has not answered yet to close.
        let station = match ai.at_station {
            1 => 1.0,
            -1 => -1.0,
            _ => match self.var("AI_Scheduled_AtStation") {
                Some(v) if v.abs() > 0.5 => -1.0,
                _ => 0.0,
            },
        };
        for (name, v) in [
            ("AI", 1.0),
            ("AI_Blinker_L", matches!(ai.blinker, 1 | 3) as i32 as f32),
            ("AI_Blinker_R", matches!(ai.blinker, 2 | 3) as i32 as f32),
            ("AI_Brakelight", ai.brake as i32 as f32),
            ("AI_Light", ai.lights as i32 as f32),
            // (an AI bus lights its saloon when it drives with
            // its lights on - the LiAZ's `lights_AI` switches both saloon circuits on it)
            ("AI_Interiorlight", ai.lights as i32 as f32),
            ("AI_Engine", if ai.engine_off { -1.0 } else { 1.0 }),
            ("AI_Scheduled_AtStation", station),
            // Which side's doors: OMSI hands the stop's side to the script, and a vehicle
            // with doors on both sides opens only the platform's (the BRT stops in
            // Urumqi61 lie left, the ordinary ones right). Off a stop it is 0 (OMSI's
            // default), so a script that reads it there does the same as ever.
            ("AI_Scheduled_AtStation_Side", ai.at_station_side),
            ("TrafficPriorityWarningNeeded", ai.priority_warning as i32 as f32),
        ] {
            self.set_var(name, v);
        }
        for &(id, v) in inputs.iter().chain(pinned) {
            self.put(Some(id), v);
        }
        let p = self.ty.program.clone();
        if p.frame_ai.is_empty() {
            self.vm.run_frame(&p, &mut self.state, &mut self.host);
        } else {
            self.vm.run_frame_ai(&p, &mut self.state, &mut self.host);
        }
        self.clear_pax_requests();
        for &(id, v) in pinned {
            self.put(Some(id), v);
        }
        if self.ai_visuals {
            let dt = dt + std::mem::take(&mut self.ai_visuals_missed);
            self.update_visuals(dt);
        } else {
            // the coupled parts follow all the same (other cars keep clear of them)
            self.update_trailers(dt);
            self.ai_visuals_missed = (self.ai_visuals_missed + dt).min(10.0);
        }
    }
}
