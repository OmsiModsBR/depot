//! The game built without the `lan` feature: it is never in a LAN session.

impl crate::App {
    /// Whether this game joined another's LAN session: never without LAN play.
    #[inline]
    pub(crate) fn is_lan_client(&self) -> bool {
        false
    }

    /// Whether this game is in a LAN session at all: never without LAN play.
    #[inline]
    pub(crate) fn in_lan_session(&self) -> bool {
        false
    }

    /// Whether this game hosts a LAN session: never without LAN play.
    #[inline]
    pub(crate) fn is_lan_host(&self) -> bool {
        false
    }

    /// A host's start METAR for the players: no players without LAN play.
    #[inline]
    pub(crate) fn share_start_metar(&mut self) {}

    /// The weather for the players of a session this game hosts: none without LAN play.
    #[inline]
    pub(crate) fn share_weather_as_host(&mut self, _spec: &str) {}

    /// Whether a dedicated server said we administer it: no server without LAN play.
    #[inline]
    pub(crate) fn is_server_admin(&self) -> bool {
        false
    }

    /// The session's server code to the clipboard: there is none without LAN play (the
    /// menu line is not shown).
    #[inline]
    pub(crate) fn copy_server_code(&mut self) {}

    /// The host's map edits to the other players: none without LAN play.
    #[inline]
    pub(crate) fn editor_broadcast(&mut self, _all: bool) {}

    /// The LAN session's frame: nothing without LAN play.
    #[inline]
    pub(crate) fn tick_lan(&mut self, _dt: f32) {}

    /// Other players on foot: none.
    #[inline]
    pub(crate) fn sync_remote_walkers(&mut self) {}
}

/// What live code asks of the LAN module, answered for a game that is never in a session.
pub(crate) mod lan {
    use std::collections::HashSet;
    use std::sync::Arc;

    /// The buses a server offers: no server, so any bus.
    #[inline]
    pub(crate) fn server_offers() -> Option<Arc<HashSet<String>>> {
        None
    }

    /// Whether `offered` has `bus`: never asked, as `server_offers` is None.
    #[inline]
    pub(crate) fn offers(_offered: &HashSet<String>, _bus: &str) -> bool {
        true
    }
}

/// The server admin's list: never opened, as no one is a server's admin.
pub(crate) mod admin {
    use crate::App;

    /// The admin list's lines: none.
    pub(crate) fn items(_app: &App) -> Vec<(String, String)> {
        Vec::new()
    }

    /// A line of the admin list: nothing to do.
    pub(crate) fn run(_app: &mut App, _action: &str) {}
}
