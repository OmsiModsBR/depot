//! The AI rows of the conformance matrix (spec 002, phase 8): each engine's AI body on the five
//! conformance vehicles. `physics_model_classic.rs` runs the OMSI Classic rows (`KA-*`),
//! `physics_model_depot.rs` the Project Depot Physics rows (`DA-*`).
//!
//! (Not in `support/`: the trace harnesses copy that into older commits, which lack these
//! bodies.)

#![allow(dead_code)]

use glam::{DVec2, DVec3};
use omsi_cfg::PhysicsMode;
use omsi_model::Model;
use omsi_script::Program;
use omsi_sim::ai::{AiClassic, AiMotion, MotionKind};
use omsi_sim::physics::{kinematic_wheel_rest_loads, Engine};
use omsi_sim::vehicle::AiFrame;
use omsi_sim::{SimClock, VehicleHost, VehicleInstance, VehicleType};
use omsi_vehicle::Vehicle;
use std::path::PathBuf;
use std::sync::Arc;

pub const DT: f32 = 1.0 / 60.0;

/// Straight north for 30 m, a right turn of radius `r` through 90°, then east.
pub fn bend(r: f64, s: f64) -> DVec3 {
    let arc = r * std::f64::consts::FRAC_PI_2;
    let p = if s < 30.0 {
        DVec2::new(0.0, s)
    } else if s < 30.0 + arc {
        let a = (s - 30.0) / r;
        DVec2::new(r - r * a.cos(), 30.0 + r * a.sin())
    } else {
        DVec2::new(r + (s - 30.0 - arc), 30.0 + r)
    };
    p.extend(0.0)
}

/// How far `p` lies from the bend (plan view, m).
fn off_bend(r: f64, p: DVec2) -> f64 {
    let a = DVec2::new(0.0, -1000.0);
    let b = DVec2::new(0.0, 30.0);
    let seg = |a: DVec2, b: DVec2| {
        let t = ((p - a).dot(b - a) / (b - a).length_squared()).clamp(0.0, 1.0);
        (a + (b - a) * t - p).length()
    };
    let centre = DVec2::new(r, 30.0);
    let d = p - centre;
    let on_arc = d.x <= 0.0 + 1e-9 && d.y >= -1e-9 || (d.x >= 0.0 && d.y >= 0.0);
    let arc = if on_arc && d.x <= r + 1e-9 {
        (d.length() - r).abs()
    } else {
        f64::MAX
    };
    seg(a, b).min(arc).min(seg(
        DVec2::new(r, 30.0 + r),
        DVec2::new(r + 1000.0, 30.0 + r),
    ))
}

pub fn vehicle_type(def: Vehicle) -> Arc<VehicleType> {
    let mut program = Program::default();
    for n in ["M_Wheel", "Brakeforce", "Throttle", "Brake"] {
        program.declare_var(n);
    }
    Arc::new(VehicleType {
        def,
        model: Model::default(),
        model_dir: PathBuf::new(),
        program: Arc::new(program),
        meshes: Vec::new(),
        paint_schemes: Vec::new(),
        texchanges: Vec::new(),
        wheel_meshes: Vec::new(),
        suspension_axles: Vec::new(),
        missing_packs: Vec::new(),
        mesh_bounds: Vec::new(),
        mesh_boxes: Vec::new(),
    })
}

/// The vehicle of `defs` (its sections coupled behind the first) run by the AI of `mode`.
pub fn instance(defs: &[Vehicle], mode: PhysicsMode) -> VehicleInstance {
    let mut host = VehicleHost::new(SimClock::default());
    host.temperature = 15.0;
    let mut v = VehicleInstance::new(vehicle_type(defs[0].clone()), host);
    v.set_physics_mode(mode);
    for d in &defs[1..] {
        v.attach_trailer(vehicle_type(d.clone()));
    }
    v
}

/// One frame of the AI: the body along the bend at `speed`, the vehicle and its sections after
/// it.
fn frame(body: &mut AiMotion, v: &mut VehicleInstance, r: f64, s: f64, speed: f32) {
    body.step(DT, speed, &|d| bend(r, s + d as f64), None, None);
    body.apply(v);
    v.update_ai(
        DT,
        &AiFrame {
            speed,
            odometer: s as f32,
            steer_deg: body.steer(),
            ..Default::default()
        },
    );
}

/// The farthest the rotation point (`[rot_pnt_long]` on the body's axis) strays from the lane
/// through bends of 12 and 20 m radius at 5 m/s, the worst roll (deg) and the worst gap at a
/// joint (m).
pub fn route_following(defs: &[Vehicle], mode: PhysicsMode) -> (f64, f32, f64) {
    let (rot, _) = omsi_sim::ai::rotation_point(&defs[0]);
    let (mut off, mut roll, mut gap) = (0.0f64, 0.0f32, 0.0f64);
    for r in [12.0, 20.0] {
        let mut v = instance(defs, mode);
        let mut body = AiMotion::new(&defs[0], MotionKind::Road, mode);
        body.place(&|d| bend(r, d as f64), None, None, 0.0);
        body.apply(&mut v);
        let speed = 5.0f32;
        let mut s = 0.0f64;
        for k in 0..(25.0 / DT) as usize {
            s += (speed * DT) as f64;
            frame(&mut body, &mut v, r, s, speed);
            if k < 30 {
                continue;
            }
            let h = v.heading.to_radians();
            let p = v.position.truncate() + DVec2::new(h.sin(), h.cos()) * rot as f64;
            off = off.max(off_bend(r, p));
            roll = roll.max(v.bank.abs());
            let mut lead = (v.position, v.body_rotation());
            for t in &v.trailers {
                let (_, front) = t.couplings();
                let ball = t.coupling_point(lead.0, lead.1);
                let held = t.position + t.body_rotation().transform_point3(front).as_dvec3();
                gap = gap.max((ball - held).truncate().length());
                lead = (t.position, t.body_rotation());
            }
        }
    }
    (off, roll, gap)
}

/// The static loads the scripts' sag comes from (FR-015, `KA-4` / `DA-6`): unchanged by
/// `[cog]` in OMSI Classic, shared about it in Project Depot Physics.
pub fn static_loads_follow_cog(defs: &[Vehicle], engine: Engine) -> bool {
    let def = &defs[0];
    let moved = Vehicle {
        cog: Some([
            0.0,
            def.cog.map(|c| c[1]).unwrap_or(0.0) + 1.0,
            def.cog_height,
        ]),
        ..def.clone()
    };
    kinematic_wheel_rest_loads(def, engine) != kinematic_wheel_rest_loads(&moved, engine)
}

/// `KA-6`, `KA-6b`: the Classic vehicle knocked off its route at 8 m/s runs on as the
/// player's rigid bodies (every section), braked by its scripts; cleared, it is on its route
/// again with no section left on a body. Returns the seconds to a stand.
pub fn release_and_return(defs: &[Vehicle]) -> f32 {
    let mut v = instance(defs, PhysicsMode::Classic);
    let mut ai = AiClassic::new(&defs[0]);
    let straight = |d: f32| DVec3::new(0.0, d as f64, 0.0);
    ai.place(&straight, None, None, 8.0);
    ai.step(DT, 8.0, &straight, None, None);
    ai.apply(&mut v);
    v.update_ai(
        DT,
        &AiFrame {
            speed: 8.0,
            ..Default::default()
        },
    );
    assert!(ai.release(&mut v));
    assert!(ai.is_loose() && v.rigid().is_some());
    assert!((v.motion().speed - 8.0).abs() < 0.5, "{}", v.motion().speed);
    let frame = AiFrame {
        blinker: 3,
        brake: true,
        ..Default::default()
    };
    // (what a brake script answers to the full brake: about 4 m/s² on the front section)
    let brake = 4.0 * v.mass_kg();
    let mut t = 0.0f32;
    while t < 20.0 {
        v.set_var("Brakeforce", brake);
        v.update_loose(DT, &frame);
        t += DT;
        if v.motion().speed.abs() < 0.05 {
            break;
        }
    }
    assert!(v.trailers.iter().all(|p| p.rigid().is_some()));
    ai.settle(&mut v);
    assert!(!ai.is_loose() && v.rigid().is_none());
    t
}
