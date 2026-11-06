//! The offscreen run (`--offscreen`): a scripted session rendered to pictures, with the
//! state trace and the ground-gap check it can write.
//!
//! `implementation/` holds the procedures: `run.rs` is the run itself, `state_trace.rs`
//! the per-frame trace and `ground_gap.rs` the ground-gap check.

pub(crate) mod implementation;

pub(crate) use implementation::run::run_offscreen;
