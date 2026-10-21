//! The window's surface on the renderer: made, resized and switched to or from vsync with
//! the renderer's device and format.

use crate::output::{winit_window, SurfaceState};
use crate::renderer::Renderer;
use anyhow::{Context, Result};
use std::sync::Arc;

impl<'w> SurfaceState<'w> {
    pub fn new(
        instance: &wgpu::Instance,
        window: Arc<winit_window::Window>,
        renderer: &Renderer,
        width: u32,
        height: u32,
    ) -> Result<Self> {
        Self::new_with(instance, window, renderer, width, height, true)
    }

    /// `vsync` false lets the frames go out as fast as they are drawn.
    pub fn new_with(
        instance: &wgpu::Instance,
        window: Arc<winit_window::Window>,
        renderer: &Renderer,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<Self> {
        let surface = instance.create_surface(window).context("create_surface")?;
        let mut config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: renderer.format(),
            width: width.max(1),
            height: height.max(1),
            present_mode: if vsync {
                wgpu::PresentMode::AutoVsync
            } else {
                wgpu::PresentMode::AutoNoVsync
            },
            // (two frames in flight keep the graphics chip busy while the next frame is
            // recorded: with one, the whole loop serialized behind the vsync'd drawable -
            // on Apple silicon a frame cost GPU + CPU instead of the larger of the two,
            // 50 ms where 24 of them were GPU. The price is one more frame of input
            // delay, 33 ms at 60 Hz.)
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        config.width = width.max(1);
        config.height = height.max(1);
        surface.configure(&renderer.device, &config);
        Ok(SurfaceState {
            surface: std::mem::ManuallyDrop::new(surface),
            config,
            lost: renderer.gpu.device_lost.clone(),
            elsewhere: false,
        })
    }

    /// The window of a renderer whose pictures another renderer shows (one on wgpu's noop
    /// device, with a frame plan sink that draws and presents them): its size and settings
    /// are kept as for a shown window, but the surface is never configured, and
    /// [`SurfaceState::acquire`] gives no frame.
    pub fn shown_elsewhere(
        instance: &wgpu::Instance,
        window: Arc<winit_window::Window>,
        renderer: &Renderer,
        width: u32,
        height: u32,
        vsync: bool,
    ) -> Result<Self> {
        let surface = instance.create_surface(window).context("create_surface")?;
        Ok(SurfaceState {
            surface: std::mem::ManuallyDrop::new(surface),
            config: wgpu::SurfaceConfiguration {
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                format: renderer.format(),
                width: width.max(1),
                height: height.max(1),
                present_mode: if vsync {
                    wgpu::PresentMode::AutoVsync
                } else {
                    wgpu::PresentMode::AutoNoVsync
                },
                desired_maximum_frame_latency: 2,
                alpha_mode: wgpu::CompositeAlphaMode::Auto,
                view_formats: vec![],
            },
            lost: renderer.gpu.device_lost.clone(),
            elsewhere: true,
        })
    }

    /// Whether another renderer shows this window's pictures.
    pub fn is_shown_elsewhere(&self) -> bool {
        self.elsewhere
    }

    /// The next frame to draw into; `Occluded` for a window shown elsewhere.
    pub fn acquire(&self) -> wgpu::CurrentSurfaceTexture {
        if self.elsewhere {
            return wgpu::CurrentSurfaceTexture::Occluded;
        }
        self.surface.get_current_texture()
    }

    pub fn resize(&mut self, renderer: &Renderer, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        // (not after a lost device: see `drop`)
        if renderer.device_lost().is_some() || self.elsewhere {
            return;
        }
        self.surface.configure(&renderer.device, &self.config);
    }

    pub fn set_vsync(&mut self, renderer: &Renderer, enabled: bool) {
        let mode = if enabled {
            wgpu::PresentMode::AutoVsync
        } else {
            wgpu::PresentMode::AutoNoVsync
        };
        if self.config.present_mode == mode || renderer.device_lost().is_some() {
            return;
        }
        if self.elsewhere {
            self.config.present_mode = mode;
            return;
        }
        self.config.present_mode = mode;
        // (two frames in flight: see `new_with`)
        self.config.desired_maximum_frame_latency = 2;
        self.surface.configure(&renderer.device, &self.config);
    }
}
