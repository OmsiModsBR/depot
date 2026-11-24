//! Steamworks API layer for displaying in-game status using steam's presence.

use super::super::domain::steam::APP_ID;

pub struct Steam {
    pub client: steamworks::Client,
}

impl Steam {
    pub fn start() -> Option<Steam> {
        if let Ok(client) = steamworks::Client::init_app(APP_ID) {
            log::info!("Steam: Connected");

            Some(Steam { client })
        } else {
            None
        }
    }
}
