//! What the wire sounds like and where it sparks: the parameter file's `[sound]` and
//! `[sparks]` sections, and the sounds the poles ask the game to play.

use glam::DVec3;

/// The sound folder of the convention, relative to a content root: `<name>.wav` outside,
/// `<name>_int.wav` heard inside.
pub const SOUND_DIR: &str = "Sounds/AckerTroll";

/// The `[sound]` section.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SoundParams {
    /// How near (metres, along the ground) the middle of the two shoes passes a crossing (or
    /// a wire sound marker) to be heard. Every switch and crossing passed is heard: the
    /// parameter file's `*_chance` keys are not read.
    pub cross_trigger_radius: f64,
    /// Outside: full volume up to `min_distance` metres from the shoe, nothing beyond
    /// `max_distance`.
    pub min_distance: f64,
    pub max_distance: f64,
    /// dB of the switch and crossing sounds.
    pub volume_db: f64,
    /// Inside: where the sounds come from (the carrying part's body frame: x right, y
    /// forward, z up) and the dB they gain in the cab and in the saloon.
    pub interior_source: DVec3,
    pub interior_cab_gain_db: f64,
    pub interior_salon_gain_db: f64,
    /// dB of a spark, and of the spark of a pole leaving the wire.
    pub spark_volume_db: f64,
    pub dewire_volume_db: f64,
    /// The spark recording's own amplification (times).
    pub spark_sample_gain: f64,
}

/// The `[sparks]` section: when a shoe on the wire sparks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SparkParams {
    pub enabled: bool,
    /// Only while the vehicle is faster than this (km/h) and draws more than this
    /// (the throttle the convention feeds in, 0..1).
    pub min_speed_kmh: f64,
    pub min_throttle: f64,
    /// Seconds between two sparks of a shoe (drawn between the two), and how long one
    /// lasts.
    pub interval: [f64; 2],
    pub duration: [f64; 2],
    /// How many times more often it sparks in rain, in snow, on ice.
    pub rain_gain: f64,
    pub snow_gain: f64,
    pub ice_gain: f64,
}

/// The weather the wire is in, for the sparks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireWeather {
    #[default]
    Dry,
    Rain,
    Snow,
    Ice,
}

/// What a sound is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WireSoundKind {
    /// A switch, the shoes leaving its entry for its left (`left`) or right way (left and
    /// right as seen from the entry, looking along the ways).
    SwitchSeparate { left: bool },
    /// A switch, the shoes coming in from its left (`left`) or right way to its entry.
    SwitchMerge { left: bool },
    /// A crossing, or a wire sound marker.
    Cross,
    /// A spark under a shoe.
    Spark,
    /// The spark of a pole leaving the wire.
    DewireSpark,
}

impl WireSoundKind {
    /// The recording's name in [`SOUND_DIR`] (without `_int` and `.wav`).
    pub fn file_stem(self) -> &'static str {
        match self {
            WireSoundKind::SwitchSeparate { left: true } => "switch_left_sep",
            WireSoundKind::SwitchSeparate { left: false } => "switch_right_sep",
            WireSoundKind::SwitchMerge { left: true } => "switch_left_merg",
            WireSoundKind::SwitchMerge { left: false } => "switch_right_merg",
            WireSoundKind::Cross => "cross",
            WireSoundKind::Spark | WireSoundKind::DewireSpark => "iskra",
        }
    }

    /// Whether a recording heard inside (`<stem>_int.wav`) may exist for it.
    pub fn has_interior(self) -> bool {
        !matches!(self, WireSoundKind::Spark | WireSoundKind::DewireSpark)
    }
}

/// A sound the poles ask the game to play once.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WireSound {
    pub kind: WireSoundKind,
    /// Where, in the world.
    pub position: DVec3,
}
