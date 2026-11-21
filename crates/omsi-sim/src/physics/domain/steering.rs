//! A steerable axle's state (spec 004, `D-6` of `docs/physics/depot-model.md`): the mode the
//! scripts pick and where the axle stands.

/// What a steerable axle does, as the script picks it (`Axle_Steer_Mode_<n>`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SteerMode {
    /// 0: turns with the driver's steering (behind a joint, the joint's bend) in proportion to
    /// its lock.
    #[default]
    Basic,
    /// 1: locked straight.
    Locked,
    /// 2: the script's angle (`Axle_Steer_Angle_<n>`), clamped to its lock.
    ScriptAngle,
}

impl SteerMode {
    /// The mode a script's value asks for; any value but 1 and 2 is the basic mode (0, the
    /// value of a variable the script does not declare).
    pub fn from_value(v: f32) -> SteerMode {
        if v == 1.0 {
            SteerMode::Locked
        } else if v == 2.0 {
            SteerMode::ScriptAngle
        } else {
            SteerMode::Basic
        }
    }
}

/// Where a steerable axle stands.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SteerState {
    /// The axle's angle (rad, positive to the right).
    pub angle: f32,
    /// How much the axle steers, 0..1: 1 steering (the basic mode reached), 0 held (locked or
    /// held by the script). It weighs the axle in the turning line (`D-5`).
    pub engagement: f32,
    /// The mode the scripts picked this frame.
    pub mode: SteerMode,
    /// The script's angle (rad), used in [`SteerMode::ScriptAngle`].
    pub script_angle: f32,
}

impl Default for SteerState {
    /// Straight and steering, as a bus is put down.
    fn default() -> Self {
        SteerState {
            angle: 0.0,
            engagement: 1.0,
            mode: SteerMode::Basic,
            script_angle: 0.0,
        }
    }
}
