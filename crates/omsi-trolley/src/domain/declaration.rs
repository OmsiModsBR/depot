//! A vehicle declared as a trolleybus: which of its parts carries the poles, where the poles
//! stand, and the parameters they move with.
//!
//! Positions are in the carrying part's body frame (metres; x right, y forward, z up, the
//! model's own frame).

use glam::DVec3;

use super::sound::{SoundParams, SparkParams};

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
    /// Dewirement: the wire out of reach by more than `reach_release_error` metres for
    /// `reach_release_time` seconds; the contact force under `force_dewire` N for
    /// `force_dewire_time`; the sideways pull over `side_force_dewire` N for
    /// `side_force_time`; the pole turned more than `yaw_limit` degrees off straight back.
    pub reach_release_error: f64,
    pub reach_release_time: f64,
    pub force_dewire: f64,
    pub force_dewire_time: f64,
    pub side_force_dewire: f64,
    pub side_force_time: f64,
    pub yaw_limit: f64,
    /// After a dewirement: the pole springs up at `free_rise_rate` degrees a second (to
    /// `max_pitch` degrees above the horizontal), and after `catcher_delay` seconds the
    /// retriever pulls it down at `catcher_lower_rate` degrees a second.
    pub free_rise_rate: f64,
    pub max_pitch: f64,
    pub catcher_delay: f64,
    pub catcher_lower_rate: f64,
    /// The switch demand above which a switch turns.
    pub switch_threshold: f64,
    /// Through a switch or crossing faster than this (km/h) for longer than the grace time
    /// (seconds), the pole leaves the wire.
    pub complex_speed_limit: f64,
    pub complex_overspeed_grace: f64,
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
    /// The wire's sounds and sparks (None: the parameter file has no usable `[sound]` or
    /// `[sparks]` section, and the wire stays silent or does not spark).
    pub sound: Option<SoundParams>,
    pub sparks: Option<SparkParams>,
}
