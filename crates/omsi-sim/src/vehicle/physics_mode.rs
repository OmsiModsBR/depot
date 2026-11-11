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
    mode
}
