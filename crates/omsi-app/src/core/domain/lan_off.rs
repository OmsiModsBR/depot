//! What the game answers of a LAN session, server or admin list in a game built without the
//! `lan` feature: none of them is ever true.

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
    use crate::core::app::App;

    /// The admin list's lines: none.
    pub(crate) fn items(_app: &App) -> Vec<(String, String)> {
        Vec::new()
    }

    /// A line of the admin list: nothing to do.
    pub(crate) fn run(_app: &mut App, _action: &str) {}
}
