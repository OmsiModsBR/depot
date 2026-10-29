//! PCVR: the OpenXR session (Windows only) and the cockpit navigator placed in it.

pub(crate) mod navigator;
#[cfg(windows)]
pub(crate) mod session;
