//! The `OMSI_*` environment variables, each declared once: its name, the kind of value it
//! takes, who it is for, the module that reads it, whether a phone's `env.txt` may set it and
//! what it does.
//!
//! Code reads a variable through its declaration (`flags::MUTE.is_set()`,
//! `flags::IBIS_BUDGET.number::<f64>()`) instead of spelling the name at the read. The values
//! come from [`crate::env`], so a variable is looked up once per thread and the program sees
//! the environment it started with (a variable the program sets for itself is set at start-up,
//! before anything reads it).
//!
//! The declarations are filled crate by crate (macro-refactor plan, D2); [`ALL`] lists the
//! ones made so far.

use std::ffi::OsString;
use std::path::PathBuf;
use std::str::FromStr;

/// What a variable holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    /// On when it is set at all (`=1` by convention).
    Switch,
    /// A number.
    Number,
    /// Words: a name, a list, a mode.
    Text,
    /// A file or a folder, or a list of them separated like `PATH`.
    Path,
}

/// Who a variable is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// A setting people may use, documented for them.
    User,
    /// Where the program reads from or what it runs. Never set from `env.txt`: a file
    /// another app can write must not change what the game reads or runs.
    Path,
    /// A debugging aid: logs, traces, a switch to compare with the old behaviour. The owner
    /// says where it is read, so that it can go when it is no longer needed.
    Diagnostic,
    /// Only for tests and the example programs.
    Test,
    /// Set by the program itself for a process it starts.
    Internal,
}

/// One `OMSI_*` variable.
#[derive(Clone, Copy, Debug)]
pub struct Flag {
    /// The variable's name, `OMSI_…`.
    pub name: &'static str,
    pub value: Value,
    pub kind: Kind,
    /// The crate (and module) that reads it.
    pub owner: &'static str,
    /// Whether a phone's `env.txt` may set it.
    pub env_txt: bool,
    /// What it does (its declaration's doc comment).
    pub about: &'static str,
}

impl Flag {
    /// Whether the variable is set (to anything).
    pub fn is_set(&self) -> bool {
        crate::env::var_os(self.name).is_some()
    }

    /// The raw value.
    pub fn os(&self) -> Option<OsString> {
        crate::env::var_os(self.name)
    }

    /// The value as text (None when it is not set or not Unicode).
    pub fn text(&self) -> Option<String> {
        crate::env::var(self.name).ok()
    }

    /// The value read as a number (None when it is not set or does not parse).
    pub fn number<T: FromStr>(&self) -> Option<T> {
        self.text().and_then(|v| v.parse().ok())
    }

    /// The value as a path.
    pub fn path(&self) -> Option<PathBuf> {
        self.os().map(PathBuf::from)
    }
}

macro_rules! flags {
    ($(
        $(#[doc = $doc:literal])+
        $id:ident = $name:literal, $value:ident, $kind:ident, $owner:literal $(, $txt:ident)?;
    )*) => {
        $(
            $(#[doc = $doc])+
            pub const $id: Flag = Flag {
                name: $name,
                value: Value::$value,
                kind: Kind::$kind,
                owner: $owner,
                env_txt: flags!(@txt $($txt)?),
                about: concat!($($doc, "\n"),+),
            };
        )*

        /// Every variable declared so far.
        pub const ALL: &[Flag] = &[$($id),*];
    };
    (@txt env_txt) => { true };
    (@txt) => { false };
}

flags! {
    // --- where the content is

    /// The original OMSI 2 installation, read first when the content is looked for (also
    /// where the tests find the installed content).
    ROOT = "OMSI_ROOT", Path, Path, "omsi-launcher-core";
    /// The folder the content is read from, in place of the one beside the game.
    CONTENT = "OMSI_CONTENT", Path, Path, "omsi-launcher-core";
    /// Archives mounted as content roots, separated like `PATH`.
    CONTENT_ZIP = "OMSI_CONTENT_ZIP", Path, Path, "omsi-cfg::vfs";

    // --- programs the game runs

    /// The `cloudflared` the LAN tunnel runs, in place of the one found or downloaded.
    CLOUDFLARED = "OMSI_CLOUDFLARED", Path, Path, "omsi-net::tunnel";
    /// The 32-bit plugin host (`omsi-plugin-host32.exe`), in place of the one beside the game.
    PLUGIN_HOST32 = "OMSI_PLUGIN_HOST32", Path, Path, "omsi-plugin";
    /// The `wine` that runs the 32-bit plugin host outside Windows.
    WINE = "OMSI_WINE", Path, Path, "omsi-plugin";

    // --- settings

    /// Silence: the mixer plays nothing.
    MUTE = "OMSI_MUTE", Switch, User, "omsi-audio::mixer", env_txt;
    /// `1`, `true` or `yes`: radio stations on plain `http` are allowed.
    RADIO_ALLOW_HTTP = "OMSI_RADIO_ALLOW_HTTP", Text, User, "omsi-audio::url_guard";
    /// How long (seconds, 10 by default) the IBIS may look for a way to type a duty in.
    IBIS_BUDGET = "OMSI_IBIS_BUDGET", Number, User, "omsi-sim::ibis";
    /// The LAN address(es) to offer, separated by commas, spaces or semicolons.
    LAN_IP = "OMSI_LAN_IP", Text, User, "omsi-net::addrs";
    /// How long (seconds) a joining game waits for the host.
    LAN_JOIN_TIMEOUT = "OMSI_LAN_JOIN_TIMEOUT", Number, User, "omsi-net";

    // --- diagnostics

    /// Log each step of a vehicle's start-up and the candidates it weighs.
    DEBUG_STARTUP = "OMSI_DEBUG_STARTUP", Switch, Diagnostic, "omsi-sim::startup", env_txt;
    /// Log a lane end that found no lane to go on with although one starts near it.
    DEBUG_UNLINKED = "OMSI_DEBUG_UNLINKED", Switch, Diagnostic, "omsi-sim::traffic";
    /// Log what the scripts write into the text textures.
    DEBUG_TEXT = "OMSI_DEBUG_TEXT", Switch, Diagnostic, "omsi-sim::host";
    /// Log the bus's impacts and the obstacles it hits.
    DEBUG_PHYSICS = "OMSI_DEBUG_PHYSICS", Switch, Diagnostic, "omsi-sim::vehicle";
    /// Log the animation of the meshes whose name contains this text.
    DEBUG_ANIM = "OMSI_DEBUG_ANIM", Text, Diagnostic, "omsi-sim::vehicle";
    /// Log the IBIS's search for a way to type a duty in.
    DEBUG_IBIS = "OMSI_DEBUG_IBIS", Switch, Diagnostic, "omsi-sim::ibis";
    /// Log, every tenth of a second, an AI car standing over 1.5 m beside its way.
    DEBUG_AI_WIDE = "OMSI_DEBUG_AI_WIDE", Switch, Diagnostic, "omsi-sim::ai_motion";
    /// Log the gap and the angles of the joints of articulated buses and trailers.
    DEBUG_TRAILER = "OMSI_DEBUG_TRAILER", Switch, Diagnostic, "omsi-sim::vehicle::articulation";
    /// Warn at every write of a NaN or an infinity into a script variable.
    DEBUG_NAN = "OMSI_DEBUG_NAN", Switch, Diagnostic, "omsi-script::vm";
    /// Every mesh on its own, without `[animparent]`, to compare.
    NO_ANIMPARENT = "OMSI_NO_ANIMPARENT", Switch, Diagnostic, "omsi-sim::anim";
    /// AI cars steer no further than their model's lock, to compare.
    AI_MODEL_LOCK = "OMSI_AI_MODEL_LOCK", Switch, Diagnostic, "omsi-sim::ai_motion";
    /// The one tile size of 371.9 m for maps on world coordinates, to compare.
    OLD_WORLD_GRID = "OMSI_OLD_WORLD_GRID", Switch, Diagnostic, "omsi-map";
    /// The ground cut away under the roads, to compare.
    ROAD_CUT = "OMSI_ROAD_CUT", Switch, Diagnostic, "omsi-geometry";
    /// The LAN session leaves the internet alone (no rendezvous bridge).
    NO_BRIDGE = "OMSI_NO_BRIDGE", Switch, Diagnostic, "omsi-net::bridge";
    /// A joining game tries only what the rendezvous tells, not the code's own addresses.
    BRIDGE_ONLY = "OMSI_BRIDGE_ONLY", Switch, Diagnostic, "omsi-net";

    // --- omsi-app: settings of the world's picture and memory

    /// The texture budget in MB, in place of the setting (an eighth of the machine's memory
    /// when neither says).
    TEXTURE_MEMORY = "OMSI_TEXTURE_MEMORY", Number, User, "omsi-app::memory", env_txt;
    /// No environment maps (reflections) on vehicles and objects.
    NO_ENVMAP = "OMSI_NO_ENVMAP", Switch, User, "omsi-app::scene", env_txt;
    /// No bump maps on vehicles and objects.
    NO_BUMP = "OMSI_NO_BUMP", Switch, User, "omsi-app::scene", env_txt;
    /// Roads without the bumps of their textures' `.surf` maps.
    NO_SURF = "OMSI_NO_SURF", Switch, User, "omsi-app::scene", env_txt;
    /// How wet the roads are (0 to 1), in place of what the weather made of them.
    WETNESS = "OMSI_WETNESS", Number, Diagnostic, "omsi-app::puddles";

    // --- omsi-app: diagnostics of the world (scene, tiles, lights, weather)

    /// Log where the time of a frame, a tile load and the memory go.
    PROFILE = "OMSI_PROFILE", Switch, Diagnostic, "omsi-app", env_txt;
    /// Log where a texture, an object or an attachment that was not found was looked for.
    DEBUG_MISSING = "OMSI_DEBUG_MISSING", Switch, Diagnostic, "omsi-app::scene", env_txt;
    /// Prepare the tiles of a whole-map load this many at a time, as the window's streaming
    /// does.
    BATCH = "OMSI_BATCH", Number, Diagnostic, "omsi-app::scene";
    /// Write every loaded tile's terrain and how much of it the roads cut away to this file.
    DUMP_GROUND = "OMSI_DUMP_GROUND", Path, Diagnostic, "omsi-app::scene";
    /// Write the terrain cut of every tile as a picture into this folder.
    DUMP_CUT = "OMSI_DUMP_CUT", Path, Diagnostic, "omsi-app::scene";
    /// Write the scenery objects' text textures, as drawn, into this folder.
    DUMP_SCENERY_TEXT = "OMSI_DUMP_SCENERY_TEXT", Path, Diagnostic, "omsi-app::scene";
    /// List the object and spline files the map names that are not installed.
    CHECK_TYPES = "OMSI_CHECK_TYPES", Switch, Diagnostic, "omsi-app::scene";
    /// Log the surface objects standing far over or under the ground.
    CHECK_OBJECTS = "OMSI_CHECK_OBJECTS", Switch, Diagnostic, "omsi-app::scene";
    /// Walk the driving lanes and report where no road surface is drawn under them.
    CHECK_ROADS = "OMSI_CHECK_ROADS", Switch, Diagnostic, "omsi-app::offscreen";
    /// Report chained splines whose ends do not meet in height.
    CHECK_SPLINES = "OMSI_CHECK_SPLINES", Switch, Diagnostic, "omsi-app::offscreen";
    /// Log the spline faces standing taller than the profile, gradient and cant allow.
    CHECK_SPIKES = "OMSI_CHECK_SPIKES", Switch, Diagnostic, "omsi-app::scene";
    /// Log the splines that align the terrain, with their mode.
    LIST_ALIGNED = "OMSI_LIST_ALIGNED", Switch, Diagnostic, "omsi-app::scene";
    /// Draw only the scenery objects whose file name contains this text.
    ONLY_OBJECT = "OMSI_ONLY_OBJECT", Text, Diagnostic, "omsi-app::scene";
    /// Leave out the scenery objects whose file name contains this text.
    SKIP_OBJECT = "OMSI_SKIP_OBJECT", Text, Diagnostic, "omsi-app::scene";
    /// Draw only the vehicle meshes whose file names contain one of these parts (`a|b`),
    /// and log their materials.
    ONLY_MESH = "OMSI_ONLY_MESH", Text, Diagnostic, "omsi-app::scene";
    /// Leave out the vehicle meshes whose file names contain one of these parts (`a|b`).
    HIDE_MESH = "OMSI_HIDE_MESH", Text, Diagnostic, "omsi-app::scene";
    /// Log the scenery objects placed and the ones outside their tile.
    DEBUG_OBJECTS = "OMSI_DEBUG_OBJECTS", Switch, Diagnostic, "omsi-app::scene";
    /// Log how high the objects standing on the ground are placed.
    DEBUG_FLOAT = "OMSI_DEBUG_FLOAT", Switch, Diagnostic, "omsi-app::scene";
    /// Log how far a crossing's mesh was moved to lie on the ground.
    DEBUG_WARP = "OMSI_DEBUG_WARP", Switch, Diagnostic, "omsi-app::scene";
    /// Log how the material slots of the objects whose file name contains this text are
    /// made.
    DEBUG_OBJMAT = "OMSI_DEBUG_OBJMAT", Text, Diagnostic, "omsi-app::scene";
    /// Log why the AI cars, the parked cars and the timetabled buses do what they do.
    DEBUG_TRAFFIC = "OMSI_DEBUG_TRAFFIC", Switch, Diagnostic, "omsi-app::traffic";
    /// Log the obstacles' collision boxes and meshes, and the ones a road runs through.
    DEBUG_COLLISION = "OMSI_DEBUG_COLLISION", Switch, Diagnostic, "omsi-app::scene";
    /// Log the traffic lights and the crossings they belong to.
    DEBUG_LAMPS = "OMSI_DEBUG_LAMPS", Switch, Diagnostic, "omsi-app::scene";
    /// Log what the splines of a tile make: lanes, meshes, ground cut.
    DEBUG_SPLINES = "OMSI_DEBUG_SPLINES", Switch, Diagnostic, "omsi-app::scene";
    /// Log the ground's raster around this point (`x,y`).
    DEBUG_RASTER = "OMSI_DEBUG_RASTER", Text, Diagnostic, "omsi-app::scene";
    /// Log the painted ground layers and the water of each tile.
    DEBUG_SURFACES = "OMSI_DEBUG_SURFACES", Switch, Diagnostic, "omsi-app::scene";
    /// Log the uploads to the GPU that take longer than 8 ms.
    DEBUG_UPLOAD = "OMSI_DEBUG_UPLOAD", Switch, Diagnostic, "omsi-app::scene";
    /// Log the textures, their sizes and formats.
    DEBUG_TEXTURES = "OMSI_DEBUG_TEXTURES", Switch, Diagnostic, "omsi-app::scene";
    /// Log what the signals show.
    DEBUG_SIGNALS = "OMSI_DEBUG_SIGNALS", Switch, Diagnostic, "omsi-app::scene";
    /// Log the switches thrown for a train.
    DEBUG_SWITCHES = "OMSI_DEBUG_SWITCHES", Switch, Diagnostic, "omsi-app::scene";
    /// Log the particle emitters near the camera and the smoke they make.
    DEBUG_PARTICLES = "OMSI_DEBUG_PARTICLES", Switch, Diagnostic, "omsi-app::scene";
    /// Log the vehicles' interior lamps and the meshes they light.
    DEBUG_INTERIOR = "OMSI_DEBUG_INTERIOR", Switch, Diagnostic, "omsi-app::scene";
    /// Log the material of the variant meshes whose variable contains this text.
    DEBUG_VARIANTS = "OMSI_DEBUG_VARIANTS", Text, Diagnostic, "omsi-app::scene";
    /// Log the spline object rows whose start the map and the spline chain disagree about.
    DEBUG_REPEATERS = "OMSI_DEBUG_REPEATERS", Switch, Diagnostic, "omsi-app::tiles";
    /// Log the lamps' fog cones.
    DEBUG_CONES = "OMSI_DEBUG_CONES", Switch, Diagnostic, "omsi-app::lights";
    /// A test lamp over the camera, and a log of the static lights.
    DEBUG_LIGHT = "OMSI_DEBUG_LIGHT", Switch, Diagnostic, "omsi-app::lights";
    /// Switch on this spotlight (as `Spot_Select` would), to check the headlights in a picture.
    SPOT_SELECT = "OMSI_SPOT_SELECT", Number, Diagnostic, "omsi-app::lights";
    /// Log the rain's particles and the tyres throwing water.
    DEBUG_RAIN = "OMSI_DEBUG_RAIN", Switch, Diagnostic, "omsi-app::rain";
    /// Log, every this many seconds (5 by default), what the player's bus and the
    /// environment sound.
    DEBUG_SOUND = "OMSI_DEBUG_SOUND", Number, Diagnostic, "omsi-app::weather_setup";
    /// Load the tiles around this far point (`x,y`), then the start area again, so that the
    /// picture is drawn from recycled GPU slots.
    CHURN = "OMSI_CHURN", Text, Diagnostic, "omsi-app::world_load";
    /// Write how far the drawn ground lies from the wheels to this file (`-`: the log only).
    GROUND_GAP = "OMSI_GROUND_GAP", Path, Diagnostic, "omsi-app::ground_gap";
    /// How far round the camera (m, 150 by default) `OMSI_GROUND_GAP` looks.
    GROUND_GAP_RADIUS = "OMSI_GROUND_GAP_RADIUS", Number, Diagnostic, "omsi-app::ground_gap";
    /// Log, along every street lane, how far the drawn ground lies from the lane.
    GROUND_LANES = "OMSI_GROUND_LANES", Switch, Diagnostic, "omsi-app::ground_gap";
    /// How far (m, 0.12 by default) the ground may stand over a flush surface before it is
    /// cut away.
    SURFACE_FLUSH = "OMSI_SURFACE_FLUSH", Number, Diagnostic, "omsi-app::scene";

    // --- omsi-app: the old behaviour of the world's picture, to compare

    /// The splines drawn one by one.
    NO_SPLINE_BATCHING = "OMSI_NO_SPLINE_BATCHING", Switch, Diagnostic, "omsi-app::scene";
    /// The splines batched by cell only, not by material.
    NO_MATERIAL_SPLINE_BATCHING = "OMSI_NO_MATERIAL_SPLINE_BATCHING", Switch, Diagnostic, "omsi-app::scene";
    /// The ground splines drawn one by one.
    NO_GROUND_SPLINE_BATCHING = "OMSI_NO_GROUND_SPLINE_BATCHING", Switch, Diagnostic, "omsi-app::scene";
    /// No holes cut into the terrain by the splines.
    NO_SPLINE_HOLES = "OMSI_NO_SPLINE_HOLES", Switch, Diagnostic, "omsi-app::scene";
    /// No painted ground layers on the tiles.
    NO_GROUND_PAINT = "OMSI_NO_GROUND_PAINT", Switch, Diagnostic, "omsi-app::scene";
    /// The tiles' night light maps left out.
    NO_LIGHT_MAP = "OMSI_NO_LIGHT_MAP", Switch, Diagnostic, "omsi-app::scene";
    /// No PBR maps on the textures.
    NO_PBR = "OMSI_NO_PBR", Switch, Diagnostic, "omsi-app::scene";
    /// Attachments with an index the parent does not have dropped, not placed on it.
    NO_ATTACH_FALLBACK = "OMSI_NO_ATTACH_FALLBACK", Switch, Diagnostic, "omsi-app::scene";
    /// The opaque parts of a vehicle drawn first, not in model order.
    NO_MODEL_ORDER = "OMSI_NO_MODEL_ORDER", Switch, Diagnostic, "omsi-app::scene";
    /// Wheels run on the splines' height profiles, not on the drawn road.
    HEIGHTPROFILE_GROUND = "OMSI_HEIGHTPROFILE_GROUND", Switch, Diagnostic, "omsi-app::scene";
    /// The terrain moved to the roads' height under them.
    TERRAIN_ALIGN = "OMSI_TERRAIN_ALIGN", Switch, Diagnostic, "omsi-app::scene";
    /// The ground under crossings deformed to them.
    CROSSING_DEFORM = "OMSI_CROSSING_DEFORM", Switch, Diagnostic, "omsi-app::scene";
    /// The terrain cut as a plain texture, without mipmaps made on the GPU.
    CUT_PLAIN = "OMSI_CUT_PLAIN", Switch, Diagnostic, "omsi-app::scene";
    /// `[matl_noZcheck]` read as "no depth test".
    NOZCHECK_BIAS = "OMSI_NOZCHECK_BIAS", Switch, Diagnostic, "omsi-app::scene";
    /// A blended vehicle body guessed opaque.
    REPAIR_BODY_DEPTH = "OMSI_REPAIR_BODY_DEPTH", Switch, Diagnostic, "omsi-app::scene";
    /// Every vehicle material opaque, except the dirt overlays.
    FORCE_OPAQUE = "OMSI_FORCE_OPAQUE", Switch, Diagnostic, "omsi-app::scene";
    /// The windows' rain drawn with OMSI 2's texture, not as a film of drops.
    TEXTURE_RAIN = "OMSI_TEXTURE_RAIN", Switch, Diagnostic, "omsi-app::scene";

    // --- tests and example programs

    /// The installed content for the tests, when `OMSI_ROOT` is not set.
    TEST_CONTENT = "OMSI_TEST_CONTENT", Path, Test, "omsi-o3d";
    /// The folder with Wine for the plugin host's test.
    TEST_WINE_DIR = "OMSI_TEST_WINE_DIR", Path, Test, "omsi-plugin";
    /// Where the physics traces test writes its traces.
    PHYSICS_TRACE_DIR = "OMSI_PHYSICS_TRACE_DIR", Path, Test, "omsi-sim";
    /// Only the game's own `cloudflared`, to test its download.
    CLOUDFLARED_OWN = "OMSI_CLOUDFLARED_OWN", Switch, Test, "omsi-net::tunnel";
    /// The original installation for the example programs of `omsi-sim`.
    ORIGINAL = "OMSI_ORIGINAL", Path, Test, "omsi-sim (examples)";
    /// The line the `bus_audit` example types in (`5E`), in place of the one from the code.
    AUDIT_LINE = "OMSI_AUDIT_LINE", Text, Test, "omsi-sim (examples)";

    // --- set by the program itself

    /// The launcher's id for a game it started (the game's LAN files are named after it).
    INSTANCE = "OMSI_INSTANCE", Text, Internal, "omsi-launcher-core::instances";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_name_once_and_spelt_as_declared() {
        let mut seen = std::collections::HashSet::new();
        for f in ALL {
            assert!(f.name.starts_with("OMSI_"), "{}", f.name);
            assert!(seen.insert(f.name), "{} declared twice", f.name);
            assert!(!f.about.trim().is_empty(), "{} says nothing", f.name);
            assert!(!f.owner.is_empty(), "{} has no owner", f.name);
        }
    }

    #[test]
    fn env_txt_never_sets_a_path_or_a_program() {
        for f in ALL.iter().filter(|f| f.env_txt) {
            assert!(
                f.kind != Kind::Path && f.value != Value::Path,
                "{} may not come from env.txt",
                f.name
            );
            assert!(
                matches!(f.kind, Kind::User | Kind::Diagnostic),
                "{} may not come from env.txt",
                f.name
            );
        }
    }

    #[test]
    fn a_number_is_read_as_one() {
        // (a name nobody sets)
        let f = Flag {
            name: "OMSI_FLAGS_TEST_UNSET",
            value: Value::Number,
            kind: Kind::Test,
            owner: "omsi-cfg",
            env_txt: false,
            about: "",
        };
        assert!(!f.is_set());
        assert_eq!(f.number::<f64>(), None);
        assert_eq!(f.path(), None);
    }
}
