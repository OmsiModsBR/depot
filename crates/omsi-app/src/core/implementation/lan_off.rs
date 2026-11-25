//! The game built without the `lan` feature has no LAN session: each of the game's LAN
//! calls on `App` does nothing and answers "no".

impl crate::core::app::App {
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
