//! What the showroom reads from the rest of the game: the command line it fills for a bus,
//! the player's bus and its parts, the scene, the clock, the weather and the settings.

pub(crate) use crate::core::cli::Args;
pub(crate) use crate::player;
pub(crate) use crate::scene;
pub(crate) use crate::situation::start_clock;
pub(crate) use crate::spawn::{load_coupled_parts, paint_scheme, player_bus_path};
pub(crate) use crate::weather_setup::{load_weather, setup_sky, weather_lighting};
