use serde::{Deserialize, Serialize};

/// Bumped on every change of a message. Launcher and game of different versions refuse each
/// other in the handshake instead of misreading bytes.
pub const PROTOCOL_VERSION: u32 = 1;

/// First message of each side after the connection opens.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hello {
    pub protocol: u32,
    /// Free text for the tool's status line, such as the build's version.
    pub build: String,
}

/// A script variable's value. Strings and floats are the two kinds scripts have.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum VarValue {
    Float(f32),
    Text(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EventKind {
    Macro,
    Trigger,
    Toggle,
}

/// A macro, trigger or toggle that fired. Sent as an event with its frame number, not as a
/// sampled value, so a pulse shorter than a frame is not lost between snapshots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Event {
    pub frame: u64,
    pub kind: EventKind,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WheelSnapshot {
    pub load_n: f32,
    pub slip: f32,
    pub suspension_travel_m: f32,
}

/// Simulated state after the last physics substep of the frame (not the interpolated pose
/// that is drawn).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PhysicsSnapshot {
    pub position: [f32; 3],
    pub velocity: [f32; 3],
    pub wheels: Vec<WheelSnapshot>,
}

/// One frame of the watched vehicle. Variables are only the ones the tool subscribed to and
/// only those that changed since the previous snapshot.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Snapshot {
    pub frame: u64,
    pub sim_time_s: f64,
    pub vars: Vec<(String, VarValue)>,
    pub events: Vec<Event>,
    pub physics: Option<PhysicsSnapshot>,
}

/// What the tool may ask the game to do. The game applies them at the start of a frame,
/// before input and scripts, and validates each one (see `validate_command`). None carries a
/// file path: a file is named by the game's own list of the vehicle's files.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Command {
    /// Choose which variables are sent in snapshots.
    Subscribe {
        names: Vec<String>,
    },
    SetVariable {
        name: String,
        value: VarValue,
    },
    /// Reload one script or sound of the watched vehicle, by index in the game's file list.
    Reload {
        file_index: u32,
    },
}

/// Game to launcher.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToLauncher {
    Hello(Hello),
    Snapshot(Snapshot),
    /// A command was applied at this frame; the run record marks it.
    Applied {
        frame: u64,
        command: Command,
    },
    Rejected {
        reason: String,
    },
}

/// Launcher to game.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ToGame {
    Hello(Hello),
    Command(Command),
}
