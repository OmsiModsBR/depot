//! A vehicle declared as a trolleybus: which of its parts carries the poles, where the poles
//! stand, and the parameters they move with.
//!
//! Positions are in the carrying part's body frame (metres; x right, y forward, z up, the
//! model's own frame).

use glam::DVec3;

/// One pole: its pivot and its shoe as the model draws it at rest.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleGeometry {
    /// The pivot at the pole's foot.
    pub base: DVec3,
    /// The shoe at the end of the pole in the modelled pose (the pose the pole's meshes are
    /// drawn in when its variables are 0).
    pub rest_shoe: DVec3,
}

impl PoleGeometry {
    /// Pivot to shoe.
    pub fn length(&self) -> f64 {
        self.base.distance(self.rest_shoe)
    }
}

/// What the poles move with, from the parameter file and the entry's own keys.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PoleParams {
    /// How far sideways (body frame) from where the model's shoe stands at rest the wire may
    /// be for a rising pole to take it, metres.
    pub capture_distance: f64,
    /// Seconds to raise a pole from its hook onto the wire, and to lower it back.
    pub raise_duration: f64,
    pub lower_duration: f64,
    /// Output scales: yaw per side (left, right), pitch, shoe turn per side.
    pub yaw_factor: [f64; 2],
    pub pitch_factor: f64,
    pub shoe_factor: [f64; 2],
    /// Height of the pole's end below (negative) or above the wire it presses on, metres.
    pub shoe_height_offset: f64,
    /// Contact force: at the reference elevation, its change per degree, and its limit (N).
    pub spring_nominal_force: f64,
    pub spring_reference_elevation: f64,
    pub spring_gain_per_deg: f64,
    pub contact_force_max: f64,
    /// A frame longer than this (seconds) puts the poles back on their hooks.
    pub stall_reset: f64,
}

/// One registry entry that applies to the player's vehicle.
#[derive(Debug, Clone, PartialEq)]
pub struct TrolleyDeclaration {
    /// The registry section's name (after `vehicle:`).
    pub entry: String,
    /// `bus_file` and `pole_carrier_bus_file` as the registry writes them.
    pub bus_file: String,
    pub carrier_bus_file: Option<String>,
    /// Left (index 0) and right pole.
    pub poles: [PoleGeometry; 2],
    pub params: PoleParams,
}
