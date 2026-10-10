//! The A/B switches of the vehicle physics: environment variables that bring an older model
//! back for comparison. Each is read once a process (the scenarios of
//! `tests/physics_traces.rs` therefore run once with each of them on, in a process of its own).

use std::sync::OnceLock;

/// Whether `var` is set (to anything), read the first time and kept in `cell`.
fn set(cell: &OnceLock<bool>, var: &str) -> bool {
    *cell.get_or_init(|| omsi_cfg::env::var_os(var).is_some())
}

/// The suspension as OMSI has it (see `RigidBody::wheel_contacts`); `OMSI_TYRE_SUSPENSION=1`
/// gives the old one with a wheel mass, a tyre and bump stops.
pub(super) fn omsi_suspension() -> bool {
    static TYRE_SUSPENSION: OnceLock<bool> = OnceLock::new();
    !set(&TYRE_SUSPENSION, "OMSI_TYRE_SUSPENSION")
}

/// `OMSI_NO_WHEEL_SLIP=1`: every wheel grips, as before the wheels had a turning of their own.
pub(super) fn no_wheel_slip() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    set(&ON, "OMSI_NO_WHEEL_SLIP")
}

/// `OMSI_KINEMATIC_TRAILERS=1`: every coupled part follows kinematically, as before the
/// multibody physics (see `vehicle::articulation`).
pub(crate) fn kinematic_trailers() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    set(&ON, "OMSI_KINEMATIC_TRAILERS")
}
