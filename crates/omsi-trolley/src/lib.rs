//! Trolleybus support: the overhead wire network of a map, read from the content convention
//! that declares trolleybus content (the vehicle registry `TrolleyWireVehicles.ini`, the
//! parameter file `TrolleyWirePlugin.cfg`, and the wire splines and wire objects of the map).
//!
//! Nothing here draws, plays or runs a vehicle: the game builds a [`TileWires`] for each
//! tile while it loads, keeps the loaded ones in a [`WireNetwork`], and asks it for the wire
//! near a point. A map without wire splines builds nothing.
//!
//! `domain` holds the convention's parameters and the network's parts, `implementation`
//! reads the convention and builds the network, `interface` is what the game calls
//! (`input`) and what this crate takes from the rest of the workspace (`output`).

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub use interface::input::*;

#[cfg(feature = "test-support")]
pub mod testing;
