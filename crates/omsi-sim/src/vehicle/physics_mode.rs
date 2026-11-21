//! Which physics engine a vehicle runs, decided once when it appears (spec 002, D4 and D6).

use omsi_cfg::{Driver, PhysicsMode, Reason};
use omsi_vehicle::Vehicle;

/// The mode of a vehicle and of its coupled sections: the front section's file decides for all
/// of them (`front`); the `[depot_physics]` key of a rear section's file is noted and left out.
/// The mode never changes after this. The result is logged as `<vehicle>: <mode> physics
/// (<reason>)`.
pub fn resolve_physics_mode(
    name: &str,
    global: PhysicsMode,
    front: &Vehicle,
    rear: &[&Vehicle],
    driver: Driver,
) -> PhysicsMode {
    let (mode, reason) = PhysicsMode::for_vehicle(global, front.depot_physics, driver);
    let why = match reason {
        Reason::Setting => "setting",
        Reason::BusKey => "bus key",
    };
    log::info!("{name}: {} physics ({why})", mode.as_str());
    if rear.iter().any(|r| r.depot_physics) {
        log::info!("{name}: [depot_physics] in a rear section's file is left out; the front section's file decides");
    }
    note_steer_axles(name, mode, front, rear);
    mode
}

/// What becomes of the sections' steerable axles (`achse_steer_*`, spec 004), logged once
/// when the vehicle appears: left out under OMSI Classic, and on the front section's front
/// axle, which the driver steers (`RigidBody::lead` leaves it out).
fn note_steer_axles(name: &str, mode: PhysicsMode, front: &Vehicle, rear: &[&Vehicle]) {
    let declared = std::iter::once(front)
        .chain(rear.iter().copied())
        .any(|v| v.axles.iter().any(|a| a.steer.is_some()));
    if !declared {
        return;
    }
    if mode == PhysicsMode::Classic {
        log::info!("{name}: steerable axles (achse_steer_*) are read and left out under OMSI Classic physics");
        return;
    }
    let front_axle = front.axles.iter().max_by(|a, b| a.long.total_cmp(&b.long));
    if front_axle.is_some_and(|a| a.steer.is_some()) {
        log::info!("{name}: achse_steer_* on the front axle is left out; the driver steers it");
    }
}
