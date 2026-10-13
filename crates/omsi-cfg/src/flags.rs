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
