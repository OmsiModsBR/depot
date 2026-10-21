//! What the test binaries share: temporary packages, the trace writer of the trace
//! harnesses, and the synthetic vehicles and grounds.
//!
//! Every binary includes it (`mod support;`, `tests/sim` by its path) and uses what it
//! needs of it. It uses only the crate's public API: `scripts/physics-trace-diff.sh` copies
//! it with the harnesses into the commit it compares with.

#![allow(dead_code)]

pub mod ground;
pub mod scratch;
pub mod trace;
pub mod vehicles;
