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
    WireWeather,
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
    /// The left shoe was right of the right one the last frame (logged once as it happens).
    crossed: bool,
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
            crossed: false,
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
                retract: flag(self, "shtanga_recolher_cmd"),
                switch_demand: self.var("shtanga_switch_pedal").unwrap_or(0.0) as f64,
                speed_kmh: self.motion().velocity_kmh() as f64,
                weather: self.wire_weather(),
            };
            let (out, wires) = {
                let net = t.network.lock();
                let out = t.sim.step(&t.declaration, &net, &body, inputs, dt as f64);
                // (which spline and which of its wires each shoe is on)
                let wires = t.sim.poles.map(|p| {
                    p.cursor
                        .map(|c| (net.span(c.span).map_or(-1, |s| s.spline_id), c.side))
                });
                (out, wires)
            };
            for e in &t.sim.events {
                log_event(e);
            }
            let crossed = out.wire_present && out.poles[0].shoe_pos.x >= out.poles[1].shoe_pos.x;
            if crossed && !t.crossed {
                log::warn!(
                    "trolleybus: the shoes crossed: left on {:?}, right on {:?} (spline, wire)",
                    wires[0],
                    wires[1]
                );
            }
            t.crossed = crossed;
            t.heartbeat += 1.0;
            self.show_poles(&out, t.heartbeat);
        }
        self.trolley = Some(t);
    }

    /// The weather the wire is in: snowing, freezing on a wet street (ice), raining, or dry.
    fn wire_weather(&self) -> WireWeather {
        let h = &self.host;
        if h.precip_rate > 0.0 && h.precip_type == 2.0 {
            WireWeather::Snow
        } else if h.temperature <= 0.0 && h.street_cond > 0.0 {
            WireWeather::Ice
        } else if h.precip_rate > 0.0 && h.precip_type == 1.0 {
            WireWeather::Rain
        } else {
            WireWeather::Dry
        }
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
            self.set_engine_var(
                &format!("shtanga_dewired_{en}"),
                f32::from(u8::from(p.dewired)),
            );
            self.set_engine_var(&format!("shtanga_spark_{en}"), f32::from(u8::from(p.spark)));
        }
        self.set_engine_var(
            "shtanga_wire_present",
            f32::from(u8::from(out.wire_present)),
        );
        self.set_engine_var("shtanga_wire_segment", out.spline_id as f32);
        self.set_engine_var("shtanga_wire_progress", out.progress as f32);
        self.set_engine_var("shtanga_plugin_heartbeat", heartbeat);
        self.set_engine_var(
            "shtanga_speed_limit_active",
            f32::from(u8::from(out.speed_limit_active)),
        );
        self.set_engine_var(
            "shtanga_complex_overspeed",
            f32::from(u8::from(out.overspeed)),
        );
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
        PoleEvent::NoWire { pole, nearest } => match nearest {
            Some(d) => log::info!(
                "trolleybus: {} pole found no wire within reach: the nearest is {:.2} m {}, {:.2} m {}, {:.2} m {} of its resting shoe",
                side(pole),
                d.x.abs(),
                if d.x < 0.0 { "left" } else { "right" },
                d.y.abs(),
                if d.y < 0.0 { "behind" } else { "ahead" },
                d.z.abs(),
                if d.z < 0.0 { "below" } else { "above" },
            ),
            None => log::info!(
                "trolleybus: {} pole found no wire within reach (none within 30 m)",
                side(pole)
            ),
        },
        PoleEvent::Joint {
            pole,
            from,
            to,
            gap,
            via,
            ..
        } => {
            let line = format!(
                "trolleybus: {} pole from spline {from} to {to} (gap {gap:.2} m{})",
                side(pole),
                via.map_or(String::new(), |k| format!(", {k:?}"))
            );
            // (through a switch or crossing: worth a line in the ordinary log)
            if via.is_some() {
                log::info!("{line}");
            } else {
                log::debug!("{line}");
            }
        }
        PoleEvent::WireEnds { pole, spline_id } => log::debug!(
            "trolleybus: {} pole at the end of the wire of spline {spline_id}",
            side(pole)
        ),
        PoleEvent::Dewired { pole, reason } => {
            log::info!("trolleybus: {} pole left the wire ({reason:?})", side(pole))
        }
        PoleEvent::Fork {
            pole,
            kind,
            spline_id,
            demand,
            ..
        } => log::info!(
            "trolleybus: {} pole at a {} took spline {spline_id}{}",
            side(pole),
            kind.map_or("fork".to_string(), |k| format!("{k:?}")),
            if *demand { " (demand)" } else { "" }
        ),
        PoleEvent::Retracted { pole } => {
            log::info!("trolleybus: {} pole taken off the wire", side(pole))
        }
        PoleEvent::Lowered { pole } => {
            log::info!("trolleybus: {} pole down on its hook", side(pole))
        }
        PoleEvent::WireLost {
            pole,
            phase,
            spline_id,
            span_loaded,
            joints_to_nothing,
            cursor,
        } => log::warn!(
            "trolleybus: {} pole lost its wire and went back on its hook ({phase:?} on spline {spline_id}, tile loaded {span_loaded}, {joints_to_nothing} joints to unloaded wire, at {cursor:?})",
            side(pole)
        ),
        PoleEvent::StallReset { seconds } => log::info!(
            "trolleybus: poles put back on their hooks after a frame of {seconds:.1} s"
        ),
    }
}
