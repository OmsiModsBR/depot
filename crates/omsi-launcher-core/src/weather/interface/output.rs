//! What the weather choice reads from the rest of the workspace: the weather and its humidity.

pub(crate) use omsi_content::weather::psychro::{
    absolute_humidity_of_relative as absolute_humidity,
    relative_humidity_percent as relative_humidity,
};
pub(crate) use omsi_content::weather::Weather;
