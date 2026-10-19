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
    /// Log the bus's impacts and the obstacles it hits (a number: also the bus's pose every
    /// this many seconds).
    DEBUG_PHYSICS = "OMSI_DEBUG_PHYSICS", Number, Diagnostic, "omsi-sim::vehicle";
    /// Log the animation of the meshes whose name contains this text.
    DEBUG_ANIM = "OMSI_DEBUG_ANIM", Text, Diagnostic, "omsi-sim::vehicle";
    /// Log the IBIS's search for a way to type a duty in.
    DEBUG_IBIS = "OMSI_DEBUG_IBIS", Switch, Diagnostic, "omsi-sim::ibis";
    /// Log, every tenth of a second, an AI car standing over 1.5 m beside its way.
    DEBUG_AI_WIDE = "OMSI_DEBUG_AI_WIDE", Switch, Diagnostic, "omsi-sim::ai";
    /// Log the gap and the angles of the joints of articulated buses and trailers.
    DEBUG_TRAILER = "OMSI_DEBUG_TRAILER", Switch, Diagnostic, "omsi-sim::vehicle::articulation";
    /// Warn at every write of a NaN or an infinity into a script variable.
    DEBUG_NAN = "OMSI_DEBUG_NAN", Switch, Diagnostic, "omsi-script::vm";
    /// Every mesh on its own, without `[animparent]`, to compare.
    NO_ANIMPARENT = "OMSI_NO_ANIMPARENT", Switch, Diagnostic, "omsi-sim::anim";
    /// AI cars steer no further than their model's lock, to compare.
    AI_MODEL_LOCK = "OMSI_AI_MODEL_LOCK", Switch, Diagnostic, "omsi-sim::ai";
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
    /// Log, every this many seconds (10 by default), how the sound device's callbacks come
    /// (late ones, blocks mixed too slowly, waits for the voice list) and the loops started
    /// again soon after they stopped.
    DEBUG_AUDIO = "OMSI_DEBUG_AUDIO", Number, Diagnostic, "omsi-audio::mixer";
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

    // --- omsi-app: diagnostics of traffic, people, timetables and the driver

    /// Log the navigator's search for a way back to the route.
    DEBUG_NAV = "OMSI_DEBUG_NAV", Switch, Diagnostic, "omsi-app::navigator", env_txt;
    /// Log the lanes of the network that start or end near this point (`x,y[,r]`), and how
    /// they link.
    NAV_PROBE = "OMSI_NAV_PROBE", Text, Diagnostic, "omsi-app::navigator";
    /// The navigator's city map at this many metres per pixel (0.25 to 20), for pictures to
    /// compare.
    NAV_MAP_MPP = "OMSI_NAV_MAP_MPP", Number, Diagnostic, "omsi-app::navigator";
    /// Log these lanes (indices, or `at:x,y,r` for the street lanes near a point).
    DEBUG_LANES = "OMSI_DEBUG_LANES", Text, Diagnostic, "omsi-app::traffic";
    /// Log the traffic lights of these crossings (indices, or `near`).
    DEBUG_LIGHTS = "OMSI_DEBUG_LIGHTS", Text, Diagnostic, "omsi-app::traffic";
    /// Log the AI vehicle with this id, every frame.
    DEBUG_CAR = "OMSI_DEBUG_CAR", Number, Diagnostic, "omsi-app::traffic";
    /// Log the AI types' wheels as they stand at start.
    DEBUG_WHEELS = "OMSI_DEBUG_WHEELS", Switch, Diagnostic, "omsi-app::traffic";
    /// Log why a car at a junction waits.
    DEBUG_JUNCTION = "OMSI_DEBUG_JUNCTION", Switch, Diagnostic, "omsi-app::traffic";
    /// Log why a car standing behind something does not go round it.
    DEBUG_PASS = "OMSI_DEBUG_PASS", Switch, Diagnostic, "omsi-app::traffic";
    /// Log the cars standing behind an obstacle for over 20 s, and why they wait at junctions.
    DEBUG_STUCK = "OMSI_DEBUG_STUCK", Switch, Diagnostic, "omsi-app::traffic";
    /// Log the AI cars placed and taken away round the camera.
    DEBUG_POPULATION = "OMSI_DEBUG_POPULATION", Switch, Diagnostic, "omsi-app::traffic";
    /// Log the timetable buses' doors twice a second.
    DEBUG_DOORS = "OMSI_DEBUG_DOORS", Switch, Diagnostic, "omsi-app::traffic";
    /// Log the coupled parts of trains and articulated vehicles off the level of what pulls
    /// them.
    DEBUG_TRAILERS = "OMSI_DEBUG_TRAILERS", Switch, Diagnostic, "omsi-app::traffic";
    /// Log the AI vehicles that overlap the player's or one another.
    CHECK_OVERLAP = "OMSI_CHECK_OVERLAP", Switch, Diagnostic, "omsi-app::traffic";
    /// Write every AI vehicle's state, frame by frame, to this CSV file.
    TRACE_AI = "OMSI_TRACE_AI", Path, Diagnostic, "omsi-app::traffic";
    /// `OMSI_TRACE_AI` writes the timetable buses only.
    TRACE_AI_BUSES = "OMSI_TRACE_AI_BUSES", Switch, Diagnostic, "omsi-app::traffic";
    /// The chance per population pass (about 2 s, 0.035 by default) that a parked car
    /// drives off.
    PARKED_PULL_OUT = "OMSI_PARKED_PULL_OUT", Number, Diagnostic, "omsi-app::traffic";
    /// The chance per population pass (about 2 s, 0.04 by default) that a car parks.
    PARK_IN = "OMSI_PARK_IN", Number, Diagnostic, "omsi-app::traffic";
    /// The vehicle groups the map keeps to some roads drive everywhere.
    TRAFFIC_ALL_GROUPS = "OMSI_TRAFFIC_ALL_GROUPS", Switch, Diagnostic, "omsi-app::traffic";
    /// AI cars on their way and the plain ground sampler, as before (to compare).
    AI_WAY_ONLY = "OMSI_AI_WAY_ONLY", Switch, Diagnostic, "omsi-app::traffic";
    /// Log where the timetable's routes leave a gap between lanes.
    DEBUG_ROUTES = "OMSI_DEBUG_ROUTES", Switch, Diagnostic, "omsi-app::schedule";
    /// Log every step of the trip of this name (and check the trips' routes offscreen).
    CHECK_TRIPS = "OMSI_CHECK_TRIPS", Text, Diagnostic, "omsi-app::schedule";
    /// Log what the stops' departure boards show.
    DEBUG_BOARDS = "OMSI_DEBUG_BOARDS", Switch, Diagnostic, "omsi-app::schedule";
    /// How long (s) a vehicle set nobody uses is kept, in place of the default.
    FLEET_IDLE = "OMSI_FLEET_IDLE", Number, Test, "omsi-app::schedule";
    /// How far ahead (minutes, 25 by default) the fleet is read.
    FLEET_AHEAD = "OMSI_FLEET_AHEAD", Number, Test, "omsi-app::schedule";
    /// Log the timetable buses serving or passing their stops.
    DEBUG_STOPS = "OMSI_DEBUG_STOPS", Switch, Diagnostic, "omsi-app::bus_service";
    /// Log the passengers: who waits, boards and leaves, and why.
    DEBUG_PAX = "OMSI_DEBUG_PAX", Switch, Diagnostic, "omsi-app::humans";
    /// Log the people's types and what `OMSI_DEBUG_PAX` logs.
    DEBUG_HUMANS = "OMSI_DEBUG_HUMANS", Switch, Diagnostic, "omsi-app::humans";
    /// Log the people kept out of walls.
    DEBUG_WALLS = "OMSI_DEBUG_WALLS", Switch, Diagnostic, "omsi-app::humans";
    /// Log the passengers in a bus standing off its walkways.
    CHECK_WALLS = "OMSI_CHECK_WALLS", Switch, Diagnostic, "omsi-app::humans";
    /// Log, every two seconds, the people on foot with a walkable surface over their heads.
    CHECK_GROUND = "OMSI_CHECK_GROUND", Switch, Diagnostic, "omsi-app::humans";
    /// Log the people drawn with their arms out (the rest pose).
    CHECK_TPOSE = "OMSI_CHECK_TPOSE", Switch, Diagnostic, "omsi-app::humans";
    /// Send a few pedestrians across the signalised crossing nearest this point (`x,y`).
    PAX_CROSS = "OMSI_PAX_CROSS", Text, Diagnostic, "omsi-app::humans";
    /// This many people waiting at every stop.
    PAX_WAITING = "OMSI_PAX_WAITING", Number, Diagnostic, "omsi-app::humans";
    /// Write the passengers' steps to this file.
    TRACE_PAX = "OMSI_TRACE_PAX", Path, Diagnostic, "omsi-app::humans";
    /// Log the player walking: the doors and seats near and where the avatar is drawn.
    DEBUG_FOOT = "OMSI_DEBUG_FOOT", Switch, Diagnostic, "omsi-app::on_foot";
    /// Log how the driver's arms reach the wheel and the levers.
    DEBUG_DRIVER = "OMSI_DEBUG_DRIVER", Switch, Diagnostic, "omsi-app::driver";
    /// Both of the driver's hands held at these wheel angles (`left,right`), for close-ups.
    DRIVER_HANDS = "OMSI_DRIVER_HANDS", Text, Diagnostic, "omsi-app::driver";
    /// The mesh the driver's hand takes as the gear lever (part of its name).
    DRIVER_SHIFTER = "OMSI_DRIVER_SHIFTER", Text, Diagnostic, "omsi-app::driver";
    /// Log the player vehicle's meshes (and, for the ones whose file contains this text, the
    /// part of the texture each slot shows).
    DEBUG_MESHES = "OMSI_DEBUG_MESHES", Text, Diagnostic, "omsi-app::spawn";
    /// Log the player vehicle's mesh properties (and the timetable buses' display variables).
    DEBUG_PROPS = "OMSI_DEBUG_PROPS", Switch, Diagnostic, "omsi-app::spawn";
    /// Write each wheel's suspension travel, frame by frame, to this CSV file (window runs).
    SUSP_TRACE_WINDOW = "OMSI_SUSP_TRACE_WINDOW", Path, Diagnostic, "omsi-app::player";
    /// Log the jolts that cost points in the career.
    DEBUG_CAREER = "OMSI_DEBUG_CAREER", Switch, Diagnostic, "omsi-app::career";
    /// Log the petrol stations and how far the bus is from them.
    DEBUG_SERVICES = "OMSI_DEBUG_SERVICES", Switch, Diagnostic, "omsi-app::services";

    // --- omsi-app: settings of the window, the graphics and the controls

    /// The graphics interface to ask first: `vulkan`, `dx12` or `gl` (set by the game itself
    /// when it starts again after a lost device, and on phones without a good Vulkan).
    BACKEND = "OMSI_BACKEND", Text, User, "omsi-app::startup", env_txt;
    /// How many times in a row the game started again after a lost graphics device (set by
    /// the game itself; any number starts it with the safe graphics settings).
    SAFE_GPU = "OMSI_SAFE_GPU", Number, User, "omsi-app::settings", env_txt;
    /// The renderer for one run: `vanilla`, `vanilla_plus` or `enhanced`.
    GRAPHICS = "OMSI_GRAPHICS", Text, User, "omsi-app::settings", env_txt;
    /// The enhanced renderer, whatever the setting says.
    ENHANCED = "OMSI_ENHANCED", Switch, User, "omsi-app";
    /// The enhanced renderer with its extras (`--enhanced-plus`).
    ENHANCED_PLUS = "OMSI_ENHANCED_PLUS", Switch, User, "omsi-app";
    /// No clouds.
    NO_CLOUDS = "OMSI_NO_CLOUDS", Switch, User, "omsi-app", env_txt;
    /// The frame rate limit, in place of the setting.
    MAX_FPS = "OMSI_MAX_FPS", Number, User, "omsi-app::app_events", env_txt;
    /// The on-screen controls of a phone on a computer too.
    TOUCH = "OMSI_TOUCH", Switch, User, "omsi-app::platform", env_txt;
    /// The phone's layout on a computer (the rail kept clear of a camera hole).
    MOBILE = "OMSI_MOBILE", Switch, User, "omsi-app::ui", env_txt;
    /// Repeat a session: the scripts' `random` is seeded with this number.
    SEED = "OMSI_SEED", Number, User, "omsi-app", env_txt;
    /// VR through OpenXR, whatever the setting says (Windows; the old switch, kept for
    /// existing VR runs).
    OPENXR = "OMSI_OPENXR", Switch, User, "omsi-app::settings";
    /// The size of the VR picture against the headset's recommendation.
    OPENXR_SCALE = "OMSI_OPENXR_SCALE", Number, User, "omsi-app::openxr";
    /// How many mirror pictures a second are drawn in VR (-1: every mirror each frame).
    OPENXR_MIRROR_RATE = "OMSI_OPENXR_MIRROR_RATE", Number, User, "omsi-app::openxr";
    /// `0`: TrackIR read through its own interface left off.
    TRACKIR_NATIVE = "OMSI_TRACKIR_NATIVE", Text, User, "omsi-app::headtrack";
    /// `0` or `1`: the trees move in the wind or not, whatever the setting says.
    WINDY_TREES = "OMSI_WINDY_TREES", Text, Diagnostic, "omsi-app::settings";
    /// Another program opened as the launcher.
    LAUNCHER = "OMSI_LAUNCHER", Path, Path, "omsi-app::launcher_link";
    /// No restart of the game with the allocator settings it wants (macOS).
    KEEP_ALLOCATOR = "OMSI_KEEP_ALLOCATOR", Switch, Diagnostic, "omsi-app::startup";

    // --- omsi-app: diagnostics of the window, the frame and the offscreen run

    /// A test window that does not take the keyboard, and no dialog on a fatal error.
    BACKGROUND = "OMSI_BACKGROUND", Switch, Test, "omsi-app";
    /// Drive the window's handlers (mouse, keys, `look`, `turn`, `touch`) from this script
    /// (`t=3 move x,y; t=3.2 press; t=4 key F3; …`).
    INPUT = "OMSI_INPUT", Text, Test, "omsi-app::input_script";
    /// Treat the window as hidden between these seconds (`from,to`).
    HIDE_WINDOW = "OMSI_HIDE_WINDOW", Text, Test, "omsi-app::app_events";
    /// Draw even while the window is hidden.
    RENDER_OCCLUDED = "OMSI_RENDER_OCCLUDED", Switch, Test, "omsi-app::app_events";
    /// The render scale stays as set, not changed with the frame time.
    FIXED_SCALE = "OMSI_FIXED_SCALE", Switch, Diagnostic, "omsi-app::app_events";
    /// Wait for the GPU in each frame, so that its time shows as a stage of its own.
    PROFILE_GPU = "OMSI_PROFILE_GPU", Switch, Diagnostic, "omsi-app::app_events";
    /// Log every change of these variables of the player's bus (`a,b`).
    WATCH_VARS = "OMSI_WATCH_VARS", Text, Diagnostic, "omsi-app::app_events";
    /// Write the mouse steering, frame by frame, to this CSV file.
    TRACE_STEER = "OMSI_TRACE_STEER", Path, Diagnostic, "omsi-app::app_events";
    /// No spray from the tyres on wet roads.
    NO_SPRAY = "OMSI_NO_SPRAY", Switch, Diagnostic, "omsi-app::app_events";
    /// Log where the mirrors' cameras are.
    DEBUG_MIRRORS = "OMSI_DEBUG_MIRRORS", Switch, Diagnostic, "omsi-app::camera_util";
    /// The mirrors drawn with the window's light in the enhanced renderer, not darkened at
    /// dusk.
    MIRROR_ENHANCED = "OMSI_MIRROR_ENHANCED", Switch, Diagnostic, "omsi-app::camera_util";
    /// Log the camera's arm (1, or a higher number for more).
    DEBUG_CAMERA = "OMSI_DEBUG_CAMERA", Number, Diagnostic, "omsi-app::camera_arm";
    /// Offscreen: follow the vehicle with this camera (`dx,dy,dz,yaw,pitch[,1]`).
    FOLLOW_CAM = "OMSI_FOLLOW_CAM", Text, Diagnostic, "omsi-app::camera_util";
    /// Offscreen: how far (m, 18 by default) the outside camera sits from the vehicle.
    ORBIT_DIST = "OMSI_ORBIT_DIST", Number, Diagnostic, "omsi-app::camera_util";
    /// Offscreen: the camera at this place in the player's vehicle.
    CAM_VEHICLE = "OMSI_CAM_VEHICLE", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: `--view pax` from the vehicle's passenger camera of this number.
    PAX_CAM = "OMSI_PAX_CAM", Number, Diagnostic, "omsi-app::offscreen";
    /// Write the offscreen run's state, step by step, to this file (the B0 trace).
    STATE_TRACE = "OMSI_STATE_TRACE", Path, Test, "omsi-app::state_trace";
    /// Offscreen: hold the pedals and the wheel from each time on (`t throttle brake
    /// [steer]/ …`).
    DRIVE_PROFILE = "OMSI_DRIVE_PROFILE", Text, Test, "omsi-app::offscreen";
    /// Offscreen: the bus's speed (km/h) on the first frame.
    DRIVE_V0 = "OMSI_DRIVE_V0", Number, Test, "omsi-app::offscreen";
    /// Offscreen: drive the bus at this speed (km/h) along the straightest lanes.
    AUTOPILOT = "OMSI_AUTOPILOT", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the rear section held at this angle (degrees) to the front one.
    JOINT_ANGLE = "OMSI_JOINT_ANGLE", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: log these variables of the player's bus at the end (`a,b`).
    DEBUG_VARS = "OMSI_DEBUG_VARS", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: log `OMSI_DEBUG_VARS` every this many seconds of the drive.
    DEBUG_VARS_EVERY = "OMSI_DEBUG_VARS_EVERY", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: log these variables every half second (`a,b,$c`; `$` reads a string).
    TRACE_VARS = "OMSI_TRACE_VARS", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: write each wheel's suspension, frame by frame, to this CSV file.
    SUSP_TRACE = "OMSI_SUSP_TRACE", Path, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: log, once a second, how deep a drawn tyre goes into the road.
    WHEEL_TRACE = "OMSI_WHEEL_TRACE", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: log where the bus came to rest on the ground after it was placed.
    DEBUG_REST = "OMSI_DEBUG_REST", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: write the player vehicle's script textures into this folder.
    DUMP_SCRIPTTEX = "OMSI_DUMP_SCRIPTTEX", Path, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: aim at every clickable part of the cockpit and say which one is hit.
    CLICK_ALL = "OMSI_CLICK_ALL", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: fire every trigger of the player's bus and list the ones that change
    /// nothing.
    TRIGGER_ALL = "OMSI_TRIGGER_ALL", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: log the variables each trigger changes.
    DEBUG_TRIGGERS = "OMSI_DEBUG_TRIGGERS", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the same moment also drawn with the blended draws in the old order.
    BLEND_AB = "OMSI_BLEND_AB", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: pictures of the cars placed inside the camera's view.
    POPULATION_SHOTS = "OMSI_POPULATION_SHOTS", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: check that there is ground where a player is put down.
    CHECK_ENTRIES = "OMSI_CHECK_ENTRIES", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: drive every lane as a bus and list the objects that would stop it.
    CHECK_OBSTACLES = "OMSI_CHECK_OBSTACLES", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: list the driving lanes passing near this point (`x,y,r`).
    LANES_NEAR = "OMSI_LANES_NEAR", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: probe the ground along every lane's wheel tracks as a tyre does.
    CHECK_WHEELS = "OMSI_CHECK_WHEELS", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: write the ground under the lanes near the start to this file.
    GROUND_SAMPLE = "OMSI_GROUND_SAMPLE", Path, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the wheels' ground on a grid around a point (`x,y,half,step`).
    PROBE_GRID = "OMSI_PROBE_GRID", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the terrain and road heights along a line (`x0,y0,x1,y1[,n]`).
    PROBE = "OMSI_PROBE", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: photograph the road network from above and say where grass shows.
    ROAD_PHOTO = "OMSI_ROAD_PHOTO", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: how many points (400 by default) `OMSI_ROAD_PHOTO` photographs.
    ROAD_PHOTO_N = "OMSI_ROAD_PHOTO_N", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: how far to either side of the lane `OMSI_ROAD_PHOTO` looks.
    ROAD_PHOTO_SIDE = "OMSI_ROAD_PHOTO_SIDE", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: `OMSI_ROAD_PHOTO` from a driver's eye this far back along the lane.
    ROAD_PHOTO_SLANT = "OMSI_ROAD_PHOTO_SLANT", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: also photograph each place from above down to 25 m under the lane.
    HOLE_PHOTO = "OMSI_HOLE_PHOTO", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: draw the final picture this many more times and log the median times.
    BENCH = "OMSI_BENCH", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: log each frame of `OMSI_BENCH`.
    BENCH_FRAMES = "OMSI_BENCH_FRAMES", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: draw this many frames before the picture.
    WARM_FRAMES = "OMSI_WARM_FRAMES", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: meet the texture budget from this place first (`x,y[,MB]`).
    BUDGET_FROM = "OMSI_BUDGET_FROM", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: lay the mirror panels (1 to 3) over the picture.
    MIRROR_HUD = "OMSI_MIRROR_HUD", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the navigator shows the timetable.
    NAV_SCHEDULE = "OMSI_NAV_SCHEDULE", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the navigator shows its map.
    NAV_MAP = "OMSI_NAV_MAP", Switch, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the cabin air and the condensation on the glass after this long with this
    /// many aboard (`minutes,people[,engine 0/1]`).
    CONDENSATION = "OMSI_CONDENSATION", Text, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: the rain on the glass as the bus would meet it at this speed (m/s).
    GLASS_WIND = "OMSI_GLASS_WIND", Number, Diagnostic, "omsi-app::offscreen";
    /// Offscreen: with LAN, the other buses' sounds heard at the camera (for the logs).
    LAN_AUDIO = "OMSI_LAN_AUDIO", Switch, Test, "omsi-app::offscreen";

    // --- omsi-render: switches a phone's env.txt may set

    /// The settings as they are on a weak graphics chip too (no lighter defaults).
    FULL_GPU = "OMSI_FULL_GPU", Switch, User, "omsi-render::api::renderer", env_txt;
    /// `default` or `downlevel`: pretend the graphics card can do only that much.
    GPU_LIMITS = "OMSI_GPU_LIMITS", Text, Diagnostic, "omsi-render::api::renderer", env_txt;
    /// Only the simple pipelines, for drivers whose shader compiler fails on the others.
    BASIC_PIPELINES = "OMSI_BASIC_PIPELINES", Switch, User, "omsi-render::gpu::open", env_txt;
    /// Time each pass on the GPU (with `OMSI_PROFILE`).
    GPU_TIMERS = "OMSI_GPU_TIMERS", Switch, Diagnostic, "omsi-render::api::renderer", env_txt;
    /// No shadows.
    NO_SHADOWS = "OMSI_NO_SHADOWS", Switch, User, "omsi-render::lighting::model", env_txt;
    /// No coronas round the lamps.
    NO_CORONAS = "OMSI_NO_CORONAS", Switch, User, "omsi-render::passes::main", env_txt;
    /// One-sided faces drawn from both sides.
    NO_CULL = "OMSI_NO_CULL", Switch, Diagnostic, "omsi-render::passes::draw", env_txt;
    /// Textures uploaded as RGBA, not compressed on the GPU.
    NO_BC = "OMSI_NO_BC", Switch, User, "omsi-render::api::renderer", env_txt;
    /// No compression of loose pictures.
    NO_TEXCOMPRESS = "OMSI_NO_TEXCOMPRESS", Switch, User, "omsi-render::api::renderer", env_txt;
    /// No FXAA.
    NO_FXAA = "OMSI_NO_FXAA", Switch, User, "omsi-render::renderer::frame", env_txt;
    /// No ambient occlusion.
    NO_AO = "OMSI_NO_AO", Switch, User, "omsi-render::renderer::frame", env_txt;
    /// No ray queries.
    NO_RT = "OMSI_NO_RT", Switch, User, "omsi-render::api::renderer", env_txt;
    /// The ray-traced frame without its colour grade.
    NO_RT_GRADE = "OMSI_NO_RT_GRADE", Switch, User, "omsi-render::passes::post", env_txt;
    /// The enhanced renderer's HDR picture left out.
    NO_ENHANCED = "OMSI_NO_ENHANCED", Switch, User, "omsi-render::renderer::frame", env_txt;

    // --- omsi-render: its own settings and diagnostics

    /// On an Intel Vulkan adapter on Windows, the settings asked for instead of the stable driver profile.
    INTEL_FULL_GPU = "OMSI_INTEL_FULL_GPU", Switch, User, "omsi-render::api::renderer";
    /// `textures` or `nostorage`: read the scene's arrays as an older graphics chip has to.
    GPU_ARRAYS = "OMSI_GPU_ARRAYS", Text, Diagnostic, "omsi-render::api::renderer";
    /// With `OMSI_GPU_ARRAYS`, keep to OpenGL's sixteen texture units, as such a chip does.
    GL_TEXTURE_UNITS = "OMSI_GL_TEXTURE_UNITS", Switch, Diagnostic, "omsi-render::gpu::caps";
    /// No thread that keeps the GPU's work moving between frames.
    NO_POLL_THREAD = "OMSI_NO_POLL_THREAD", Switch, Diagnostic, "omsi-render::gpu::open";
    /// The draw lists encoded on the frame's own thread, not on a pool.
    NO_RENDER_POOL = "OMSI_NO_RENDER_POOL", Switch, Diagnostic, "omsi-render::gpu::context";
    /// Every instance's bounds worked out again each frame.
    NO_BOUNDS_CACHE = "OMSI_NO_BOUNDS_CACHE", Switch, Diagnostic, "omsi-render::api::scene";
    /// The main pass encoded directly, without render bundles.
    NO_BUNDLES = "OMSI_NO_BUNDLES", Switch, Diagnostic, "omsi-render::renderer::encode";
    /// The main pass in one part, not split in two.
    NO_MAIN_SPLIT = "OMSI_NO_MAIN_SPLIT", Switch, Diagnostic, "omsi-render::passes::main";
    /// No multisampled depth prepass.
    NO_MSAA_PREPASS = "OMSI_NO_MSAA_PREPASS", Switch, Diagnostic, "omsi-render::passes::main";
    /// The pipelines of these kinds left out (comma-separated numbers), to time what each costs.
    SKIP_PIPE = "OMSI_SKIP_PIPE", Text, Diagnostic, "omsi-render::renderer::lists";
    /// The depth bias of surfaces drawn on the ground (default -24).
    SURFACE_BIAS = "OMSI_SURFACE_BIAS", Number, Diagnostic, "omsi-render::renderer::build";
    /// Only the surfaces drawn.
    ONLY_SURFACES = "OMSI_ONLY_SURFACES", Switch, Diagnostic, "omsi-render::renderer::lists";
    /// The ray-traced frame of Enhanced+ left out.
    NO_RT_FRAME = "OMSI_NO_RT_FRAME", Switch, Diagnostic, "omsi-render::renderer::frame";
    /// The reflection rays traced per 2 x 2 pixels.
    RT_REFL_HALF = "OMSI_RT_REFL_HALF", Switch, Diagnostic, "omsi-render::effects::rt";
    /// Log the ray-tracing structures; a number shows one of its buffers.
    DEBUG_RT = "OMSI_DEBUG_RT", Number, Diagnostic, "omsi-render::effects::rt";
    /// The enhanced main pass shows one of its terms alone (a number).
    DEBUG_ENHANCED = "OMSI_DEBUG_ENHANCED", Number, Diagnostic, "omsi-render::lighting::model";
    /// Log the automatic exposure.
    DEBUG_EXPOSURE = "OMSI_DEBUG_EXPOSURE", Switch, Diagnostic, "omsi-render::lighting::exposure";
    /// The exposure meter's settings, up to six comma-separated numbers.
    METER = "OMSI_METER", Text, Diagnostic, "omsi-render::lighting::exposure";
    /// The tone curve's contrast by day and by night (`day[,night]`).
    TONE_CONTRAST = "OMSI_TONE_CONTRAST", Text, Diagnostic, "omsi-render::lighting::exposure";
    /// The air fixed for comparisons (`haze,angstrom[,height,strat]`).
    DAY_AIR = "OMSI_DAY_AIR", Text, Diagnostic, "omsi-render::sky::air";
    /// Log the sky, its cloud cover and the lamps' glow on it.
    DEBUG_SKY = "OMSI_DEBUG_SKY", Switch, Diagnostic, "omsi-render::renderer::sky";
    /// The strength of the `[matl_envmap]` photo on reflections (default 1, 0 leaves it out).
    ENV_PHOTO = "OMSI_ENV_PHOTO", Number, Diagnostic, "omsi-render::renderer::sky";
    /// Log how much of the view the lamps light.
    DEBUG_VIEW_LAMPS = "OMSI_DEBUG_VIEW_LAMPS", Switch, Diagnostic, "omsi-render::renderer::frame";
    /// Log the lights a full cell of the light grid leaves out.
    DEBUG_LIGHT_GRID = "OMSI_DEBUG_LIGHT_GRID", Switch, Diagnostic, "omsi-render::renderer::lights";
    /// Log the draw counts and the instances changed each frame.
    DEBUG_DRAWS = "OMSI_DEBUG_DRAWS", Switch, Diagnostic, "omsi-render::renderer::lists";
    /// Log the culling of instances within 150 m; `x,y,radius` logs those near that point.
    DEBUG_CULL = "OMSI_DEBUG_CULL", Text, Diagnostic, "omsi-render::renderer::lists";
    /// Log the near instances drawn in one frame and not in the next.
    DEBUG_FLICKER = "OMSI_DEBUG_FLICKER", Switch, Diagnostic, "omsi-render::renderer::lists";
    /// Log the shadow casters at least this large (a radius, default 3).
    DEBUG_SHADOW = "OMSI_DEBUG_SHADOW", Number, Diagnostic, "omsi-render::renderer::lists";
    /// Log why the far shadow map is drawn again.
    DEBUG_SHADOW_FAR = "OMSI_DEBUG_SHADOW_FAR", Switch, Diagnostic, "omsi-render::passes::shadow";
    /// The near shadow map drawn again every frame.
    SHADOW_NEAR_EVERY_FRAME = "OMSI_SHADOW_NEAR_EVERY_FRAME", Switch, Diagnostic, "omsi-render::passes::shadow";
    /// The far shadow map drawn again every frame.
    SHADOW_FAR_EVERY_FRAME = "OMSI_SHADOW_FAR_EVERY_FRAME", Switch, Diagnostic, "omsi-render::passes::shadow";
    /// Log the fog lamps, light shafts and glare of a frame.
    DEBUG_FOG_LAMPS = "OMSI_DEBUG_FOG_LAMPS", Switch, Diagnostic, "omsi-render::passes::fog_lamps";
    /// No glare from the sun.
    NO_GLARE = "OMSI_NO_GLARE", Switch, Diagnostic, "omsi-render::passes::fog_lamps";
    /// No smoke.
    NO_SMOKE = "OMSI_NO_SMOKE", Switch, Diagnostic, "omsi-render::passes::main";
    /// No falling snow.
    NO_SNOWFALL = "OMSI_NO_SNOWFALL", Switch, Diagnostic, "omsi-render::passes::main";
    /// No rain drawn on the glass.
    NO_GLASS_PICTURE = "OMSI_NO_GLASS_PICTURE", Switch, Diagnostic, "omsi-render::renderer::frame";
    /// Wet roads reflect no scene.
    NO_PUDDLE_REFLECTIONS = "OMSI_NO_PUDDLE_REFLECTIONS", Switch, Diagnostic, "omsi-render::renderer::frame";
    /// Puddles reflect no vehicles.
    NO_PUDDLE_VEHICLE = "OMSI_NO_PUDDLE_VEHICLE", Switch, Diagnostic, "omsi-render::effects::puddles";
    /// Puddles reflect glass without its depth.
    NO_PUDDLE_GLASS_DEPTH = "OMSI_NO_PUDDLE_GLASS_DEPTH", Switch, Diagnostic, "omsi-render::effects::puddles";
    /// The puddle reflection shows one of its terms (a number).
    DEBUG_PUDDLES = "OMSI_DEBUG_PUDDLES", Number, Diagnostic, "omsi-render::effects::puddles";
    /// The thickness of the puddle reflection's ray test (default 0.12).
    PUDDLE_THICKNESS = "OMSI_PUDDLE_THICKNESS", Number, Diagnostic, "omsi-render::effects::puddles";
    /// The reflectance of puddles head on (default 0.08, kept between 0.02 and 0.2).
    PUDDLE_F0 = "OMSI_PUDDLE_F0", Number, Diagnostic, "omsi-render::renderer::sky";
    /// Log the GPU's raw time stamps (with `OMSI_GPU_TIMERS`).
    GPU_TIMERS_RAW = "OMSI_GPU_TIMERS_RAW", Switch, Diagnostic, "omsi-render::output::diagnostics";
    /// Start the animation clock this many seconds later, for offscreen stills.
    RENDER_CLOCK = "OMSI_RENDER_CLOCK", Number, Test, "omsi-render::renderer::build";
    /// Fail the graphics on purpose to test the fallbacks (`open`, `open-panic`, `build`, `lost-build`, `pipeline`, `frame`, `lost`, `lost-poll`, `oom`).
    FAKE_GPU_ERROR = "OMSI_FAKE_GPU_ERROR", Text, Test, "omsi-render";

    // --- omsi-app: LAN, the launcher and plugins

    /// No plugins loaded.
    NO_PLUGINS = "OMSI_NO_PLUGINS", Switch, User, "omsi-app::plugins", env_txt;
    /// Write a picture of the plugins' panels to this file (`plugin_ui`'s preview test).
    UI_PREVIEW = "OMSI_UI_PREVIEW", Path, Test, "omsi-app::plugin_ui";
    /// The launcher's window at this size (`WxH`), to look at the layout.
    LAUNCHER_SIZE = "OMSI_LAUNCHER_SIZE", Text, User, "omsi-app::launcher", env_txt;
    /// Open the launcher at this page (`drive:2`, `settings:3`, `controls:1`, …).
    LAUNCHER_PAGE = "OMSI_LAUNCHER_PAGE", Text, Test, "omsi-app::launcher";
    /// Write a picture of the launcher after this many seconds (`secs:file.png`).
    LAUNCHER_SHOT = "OMSI_LAUNCHER_SHOT", Text, Test, "omsi-app::launcher";
    /// Close the launcher after this many seconds.
    LAUNCHER_EXIT = "OMSI_LAUNCHER_EXIT", Number, Test, "omsi-app::launcher";
    /// Work the launcher from this script (`t=2 click 412,60; t=3 type 76; …`).
    LAUNCHER_INPUT = "OMSI_LAUNCHER_INPUT", Text, Test, "omsi-app::launcher";
    /// Log, every few seconds, what the game knows of every LAN player and the bytes sent.
    DEBUG_LAN = "OMSI_DEBUG_LAN", Switch, Diagnostic, "omsi-app::lan";
    /// Log what arrives for this vehicle variable from the other LAN players.
    DEBUG_VAR_SYNC = "OMSI_DEBUG_VAR_SYNC", Text, Diagnostic, "omsi-app::lan";
    /// The vehicles' variables are not sent to the other LAN players.
    NO_VAR_SYNC = "OMSI_NO_VAR_SYNC", Switch, Diagnostic, "omsi-app::lan";
    /// The other LAN players' buses drawn where they were last heard of, not interpolated.
    NO_INTERP = "OMSI_NO_INTERP", Switch, Diagnostic, "omsi-app::lan";
    /// A LAN host opens no tunnel to the internet.
    NO_TUNNEL = "OMSI_NO_TUNNEL", Switch, Diagnostic, "omsi-app::lan";
    /// A LAN host serves no mods and a joining game fetches none.
    NO_LAN_MODS = "OMSI_NO_LAN_MODS", Switch, Diagnostic, "omsi-app::lan_mods";
    /// Chat lines said at these seconds of the session (`30=Hallo;45=Tschüss`), for LAN tests.
    LAN_SAY = "OMSI_LAN_SAY", Text, Test, "omsi-app::lan";
    /// Write where each other LAN player's bus is drawn, every frame, to this CSV file.
    TRACE_REMOTE = "OMSI_TRACE_REMOTE", Path, Diagnostic, "omsi-app::lan";
    /// Write the LAN world's poses to this file.
    LAN_TRACE = "OMSI_LAN_TRACE", Path, Diagnostic, "omsi-app::lan_world";
    /// The official server's signing key (a file): the server says where it is reached.
    OFFICIAL_KEY = "OMSI_OFFICIAL_KEY", Path, Path, "omsi-app::lan";

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
    /// The commit and date the game was built from, set by `omsi-app`'s `build.rs` for the
    /// compiler (read with `env!`, not at run time).
    BUILD = "OMSI_BUILD", Text, Internal, "omsi-app::startup";
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `docs/ENVIRONMENT.md`, made from the register.
    fn environment_md() -> String {
        let mut out = String::from(
            "# Environment variables\n\n\
             <!-- Made from `crates/omsi-cfg/src/flags.rs` by `cargo test -p omsi-cfg --lib \
             write_environment_md -- --ignored`; do not edit by hand. -->\n\n\
             The `OMSI_*` variables the game reads, as declared in `omsi_cfg::flags`. \
             \"env.txt\" marks the ones a phone's `depot/env.txt` may set.\n",
        );
        let groups = [
            (Kind::User, "Settings"),
            (Kind::Path, "Paths and programs"),
            (Kind::Diagnostic, "Diagnostics"),
            (Kind::Test, "Tests"),
            (Kind::Internal, "Set by the program itself"),
        ];
        for (kind, title) in groups {
            let mut flags: Vec<&Flag> = ALL.iter().filter(|f| f.kind == kind).collect();
            flags.sort_by_key(|f| f.name);
            out.push_str(&format!("\n## {title}\n\n"));
            out.push_str("| Variable | Value | Read by | env.txt | What it does |\n");
            out.push_str("| --- | --- | --- | --- | --- |\n");
            for f in flags {
                let about: Vec<&str> = f.about.lines().map(str::trim).collect();
                let value = match f.value {
                    Value::Switch => "switch",
                    Value::Number => "number",
                    Value::Text => "text",
                    Value::Path => "path",
                };
                out.push_str(&format!(
                    "| `{}` | {value} | `{}` | {} | {} |\n",
                    f.name,
                    f.owner,
                    if f.env_txt { "yes" } else { "" },
                    about.join(" ").replace('|', "\\|"),
                ));
            }
        }
        out
    }

    const ENVIRONMENT_MD: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../docs/ENVIRONMENT.md");

    #[test]
    fn environment_md_is_up_to_date() {
        let on_disk = std::fs::read_to_string(ENVIRONMENT_MD).unwrap_or_default();
        assert!(
            on_disk.replace("\r\n", "\n") == environment_md(),
            "docs/ENVIRONMENT.md is not what the register says: run \
             `cargo test -p omsi-cfg --lib write_environment_md -- --ignored`"
        );
    }

    #[test]
    #[ignore = "writes docs/ENVIRONMENT.md"]
    fn write_environment_md() {
        std::fs::write(ENVIRONMENT_MD, environment_md()).unwrap();
    }

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
