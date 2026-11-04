//! The world built from a map: the scene (terrain, splines, scenery objects, their GPU
//! caches and the vehicles' render sets), the map-wide index and tile streaming, the
//! map's pre-built surface, opening a map, placing a vehicle with the mouse, the object
//! editor and the `--export-glb` export.
//!
//! In three parts: `domain/` holds the scene's types, `interface/` what the rest of the
//! game reaches of the world, and `implementation/` the procedures.

pub(crate) mod domain;
pub(crate) mod implementation;
mod interface;

pub(crate) use interface::*;
