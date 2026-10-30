//! The game window's output: the backend it draws with and, under "DirectX9 (D3D9Ex)", the
//! Direct3D 9Ex device that shows its pictures. Off Windows there is only wgpu, and the
//! device's calls do nothing.

use super::super::domain::backend::Backend;
#[cfg(windows)]
use super::d3d9::D3d9Output;
use anyhow::Result;
use omsi_render::Renderer;
use std::sync::Arc;
use winit::window::Window;

/// What the game window draws with (wgpu until [`Output::attach`] hooks the Direct3D 9Ex
/// device on).
#[derive(Default)]
pub(crate) struct Output {
    backend: Backend,
    #[cfg(windows)]
    d3d9: Option<D3d9Output>,
}

impl Output {
    /// The backend the window draws with.
    pub(crate) fn backend(&self) -> Backend {
        self.backend
    }

    /// Opens the Direct3D 9Ex device on `window` and hooks it to `renderer` (which must run
    /// on wgpu's noop device, before its first asset goes up): the window then draws with
    /// [`Backend::D3d9`].
    #[cfg(windows)]
    pub(crate) fn attach(
        &mut self,
        renderer: &mut Renderer,
        window: &Arc<Window>,
        vsync: bool,
        quality: (u32, u32),
    ) -> Result<()> {
        use anyhow::Context;
        let out = D3d9Output::attach(renderer, window, vsync, quality)
            .context("The game cannot draw with DirectX9 (D3D9Ex) on this computer")?;
        self.d3d9 = Some(out);
        self.backend = Backend::D3d9;
        Ok(())
    }

    /// Off Windows there is no Direct3D 9Ex device to open.
    #[cfg(not(windows))]
    pub(crate) fn attach(
        &mut self,
        _renderer: &mut Renderer,
        _window: &Arc<Window>,
        _vsync: bool,
        _quality: (u32, u32),
    ) -> Result<()> {
        Err(anyhow::anyhow!("DirectX9 (D3D9Ex) draws on Windows only"))
    }

    /// The window's new size.
    #[cfg_attr(not(windows), allow(unused_variables))]
    pub(crate) fn resize(&self, width: u32, height: u32) {
        #[cfg(windows)]
        if let Some(d) = self.d3d9.as_ref() {
            d.resize(width, height);
        }
    }

    /// The game starts a new scene: what the old one put up goes.
    pub(crate) fn new_scene(&self) {
        #[cfg(windows)]
        if let Some(d) = self.d3d9.as_ref() {
            d.new_scene();
        }
    }

    /// Presents wait for the display's refresh, or not, with the anti-aliasing and
    /// anisotropic filtering options.
    #[cfg_attr(not(windows), allow(unused_variables))]
    pub(crate) fn set_vsync_and_quality(&self, vsync: bool, msaa: u32, anisotropy: u32) {
        #[cfg(windows)]
        if let Some(d) = self.d3d9.as_ref() {
            d.set_vsync(vsync);
            d.set_quality(msaa, anisotropy);
        }
    }

    /// Shows the picture the renderer drew this frame (under wgpu the surface does).
    pub(crate) fn present(&self) {
        #[cfg(windows)]
        if let Some(d) = self.d3d9.as_ref() {
            d.present();
        }
    }

    /// The mirror an `omsi-ui` Gpu drawing over the window's picture hands its draws to
    /// (none under wgpu).
    pub(crate) fn ui_mirror(&self) -> Option<omsi_ui::UiMirror> {
        #[cfg(windows)]
        return self.d3d9.as_ref().map(|d| d.ui_mirror());
        #[cfg(not(windows))]
        None
    }
}
