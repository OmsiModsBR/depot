//! The graphics API the game window draws with.

/// What draws the game window: wgpu (on the API `graphics_api` picks), or "DirectX9
/// (D3D9Ex)", where `omsi-render` keeps the books on wgpu's noop device and the Direct3D 9Ex
/// device draws the window. The settings choose it ([`Backend::of`]) and the window keeps
/// the one it opened with.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Backend {
    #[default]
    Wgpu,
    D3d9,
}
