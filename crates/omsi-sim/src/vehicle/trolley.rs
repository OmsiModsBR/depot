//! The poles of a vehicle declared as a trolleybus (see `omsi_trolley`): each frame, after
//! the vehicle and its coupled parts stand where they are drawn, the poles follow the wire
//! and their variables are written for the meshes and the scripts.
//!
//! The meshes of the pole-carrying part use them the next frame (a coupled part animates
//! inside its own placing), and so do the scripts (they ran before the bodies moved).

use std::sync::Arc;

use glam::Mat3;
use omsi_trolley::{
    BodyPose, PoleEvent, PoleInputs, PoleOutputs, PoleSim, TrolleyDeclaration, WireNetwork,
};

use super::VehicleInstance;

/// A declared trolleybus's poles and the wire they run on.
pub struct Trolley {
    pub declaration: TrolleyDeclaration,
    /// The part that carries the poles: 0 the vehicle itself, n its n-th coupled part.
    pub carrier: usize,
    /// The overhead wire of the loaded tiles (the world's).
    pub network: Arc<parking_lot::Mutex<WireNetwork>>,
    pub sim: PoleSim,
    heartbeat: f32,
}

impl Trolley {
    pub fn new(
        declaration: TrolleyDeclaration,
        carrier: usize,
        network: Arc<parking_lot::Mutex<WireNetwork>>,
    ) -> Trolley {
        Trolley {
            declaration,
            carrier,
            network,
            sim: PoleSim::default(),
            heartbeat: 0.0,
        }
    }
}

/// Left and right as the convention's variables name them.
const SIDES: [&str; 2] = ["lev", "prav"];
const SIDES_EN: [&str; 2] = ["left", "right"];

impl VehicleInstance {
    /// One frame of the poles (nothing for a vehicle that is no trolleybus).
    pub(crate) fn step_trolley(&mut self, dt: f32) {
        let Some(mut t) = self.trolley.take() else {
            return;
        };
        let pose = match t.carrier {
            0 => Some((self.position, self.body_rotation())),
            n => self
                .trailers
                .get(n - 1)
                .map(|p| (p.position, p.body_rotation())),
        };
        if let Some((position, rotation)) = pose {
            let body = BodyPose {
                position,
                rotation: Mat3::from_mat4(rotation).as_dmat3(),
            };
            let flag = |v: &VehicleInstance, name: &str| v.var(name).is_some_and(|x| x > 0.5);
            let inputs = PoleInputs {
                grab: [
                    flag(self, "shtanga_manual_grab_left"),
                    flag(self, "shtanga_manual_grab_right"),
                ],
                reset: flag(self, "shtanga_wire_reset"),
            };
            let out = {
                let net = t.network.lock();
                t.sim.step(&t.declaration, &net, &body, inputs, dt as f64)
            };
            for e in &t.sim.events {
                log_event(e);
            }
            t.heartbeat += 1.0;
            self.show_poles(&out, t.heartbeat);
        }
        self.trolley = Some(t);
    }

    fn show_poles(&mut self, out: &PoleOutputs, heartbeat: f32) {
        for (i, p) in out.poles.iter().enumerate() {
            let (s, en) = (SIDES[i], SIDES_EN[i]);
            // the pole is fitted: the convention's scripts show it with this
            self.set_engine_var(&format!("shtanga_manual_command_{en}"), 1.0);
            self.set_engine_var(&format!("shtanga_{s}_rot"), p.yaw as f32);
            self.set_engine_var(&format!("shtanga_{s}_pitch"), p.pitch as f32);
            self.set_engine_var(&format!("shtanga_{s}_shoe_rot"), p.shoe_rot as f32);
            self.set_engine_var(&format!("shtanga_{s}_shoe_pos_x"), p.shoe_pos.x as f32);
            self.set_engine_var(&format!("shtanga_{s}_shoe_pos_y"), p.shoe_pos.y as f32);
            self.set_engine_var(&format!("shtanga_{s}_shoe_pos_z"), p.shoe_pos.z as f32);
            self.set_engine_var(&format!("shtanga_contact_force_{en}"), p.force as f32);
        }
        self.set_engine_var(
            "shtanga_wire_present",
            f32::from(u8::from(out.wire_present)),
        );
        self.set_engine_var("shtanga_wire_segment", out.spline_id as f32);
        self.set_engine_var("shtanga_wire_progress", out.progress as f32);
        self.set_engine_var("shtanga_plugin_heartbeat", heartbeat);
    }
}

fn log_event(e: &PoleEvent) {
    let side = |p: &usize| SIDES_EN[*p];
    match e {
        PoleEvent::Captured {
            pole,
            spline_id,
            sideways,
        } => log::debug!(
            "trolleybus: {} pole took the wire of spline {spline_id} ({sideways:.2} m from its shoe)",
            side(pole)
        ),
        PoleEvent::NoWire { pole } => {
            log::info!("trolleybus: {} pole found no wire within reach", side(pole))
        }
        PoleEvent::Joint {
            pole,
            from,
            to,
            gap,
        } => log::debug!(
            "trolleybus: {} pole from spline {from} to {to} (gap {gap:.2} m)",
            side(pole)
        ),
        PoleEvent::WireEnds { pole, spline_id } => log::debug!(
            "trolleybus: {} pole at the end of the wire of spline {spline_id}",
            side(pole)
        ),
        PoleEvent::Lowered { pole } => {
            log::debug!("trolleybus: {} pole down on its hook", side(pole))
        }
        PoleEvent::StallReset { seconds } => log::info!(
            "trolleybus: poles put back on their hooks after a frame of {seconds:.1} s"
        ),
    }
}
