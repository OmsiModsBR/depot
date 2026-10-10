//! The A/B switches of the vehicle physics: environment variables that bring an older model
//! back for comparison, each read once a process.

/// The suspension as OMSI has it (see `step_slice`); `OMSI_TYRE_SUSPENSION=1` gives
/// the old one with a wheel mass, a tyre and bump stops (A/B).
pub(super) fn omsi_suspension() -> bool {
    static ON: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ON.get_or_init(|| omsi_cfg::env::var_os("OMSI_TYRE_SUSPENSION").is_none())
}
