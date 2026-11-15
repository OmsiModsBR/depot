//! `C-7`: whether the tyres still hold across, as OMSI 2 decides it.

/// Whether the tyres hold the bend: its pull `across` (N) within the grip of all of them
/// together (`grip_all`, N).
pub(in crate::physics) fn holds_across(grip_all: f32, across: f32) -> bool {
    across.abs() <= grip_all
}
