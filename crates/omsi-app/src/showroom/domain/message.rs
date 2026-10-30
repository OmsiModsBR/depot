//! What the launcher and the helper tell each other.

use super::look::Look;
use super::shape::Shape;
use serde::{Deserialize, Serialize};

/// What the launcher tells the helper.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Cmd {
    /// Show this bus in this light.
    Look(Look),
    /// The picture's size in pixels.
    Size(u32, u32),
    /// The mouse dragged over the picture (see `Showroom::orbit`).
    Orbit(f32, f32),
    Zoom(f32),
    /// Where the bus stands across the picture (see `Showroom::focus_x`).
    Focus(f32),
}

/// What the helper sends back.
#[derive(Debug, Clone, PartialEq)]
pub enum Msg {
    /// A picture: width, height, which bus it shows (`tag`), RGBA (sRGB).
    Frame(u32, u32, u64, Vec<u8>),
    /// Whether a bus is being read, and why the last one could not be shown.
    Status { busy: bool, error: Option<String> },
    /// The settled picture of this bus (`tag`), for this shape, is kept now.
    Kept(u64, Shape),
}
