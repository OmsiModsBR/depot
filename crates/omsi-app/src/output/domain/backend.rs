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

/// The options that take effort off "DirectX9 (D3D9Ex)" (all ours, none OMSI 2's; every one
/// defaults to what the renderer draws without them). Read from the settings
/// ([`crate::settings::Settings::dx9_effects`]) and handed to the device.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Dx9Effects {
    /// Raindrops gather on the panes in the rain.
    pub glass_rain: bool,
    /// The glow round the lights.
    pub coronas: bool,
    /// The night's stars.
    pub stars: bool,
    /// The weather's clouds drawn over the sky (their light on the scene stays).
    pub cloud_picture: bool,
    /// The most texels a side of a texture is put up at (0 = no ceiling).
    pub texture_limit: u32,
}

impl Default for Dx9Effects {
    fn default() -> Self {
        Self {
            glass_rain: true,
            coronas: true,
            stars: true,
            cloud_picture: true,
            texture_limit: 0,
        }
    }
}
