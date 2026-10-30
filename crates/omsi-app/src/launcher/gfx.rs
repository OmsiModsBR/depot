//! The launcher's own graphics device: a plain one for its interface, with none of the
//! game's renderer on it. Opening the game's renderer meant compiling every scene, sky,
//! shadow and post pass before the window showed anything (most of a minute on a slow
//! driver); the interface needs one pipeline. The bus preview is drawn by a helper process
//! of its own (see `crate::showroom`), so nothing 3D waits in front of the launcher's window.

use anyhow::{anyhow, Result};
use std::sync::{Arc, Mutex};
use winit::window::Window;

/// The device the interface is drawn with.
pub struct Gfx {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    format: wgpu::TextureFormat,
    pub adapter_name: String,
    /// Why the device was lost, once it was.
    lost: Arc<Mutex<Option<String>>>,
}

impl Gfx {
    /// A device for `window`: on `instance` if it can, else on the next graphics interface
    /// and adapter that can (`instance` then becomes that one's), as the game opens its own.
    pub fn for_window(instance: &mut wgpu::Instance, window: &Arc<Window>) -> Result<Gfx> {
        crate::core::startup::for_window(instance, window, |adapter, surface| {
            Gfx::open(adapter, surface)
        })
    }

    fn open(adapter: wgpu::Adapter, surface: &wgpu::Surface<'static>) -> Result<Gfx> {
        let info = adapter.get_info();
        let caps = surface.get_capabilities(&adapter);
        // (sRGB, as the interface's colours are given; else what the window offers first)
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .or_else(|| caps.formats.first().copied())
            .ok_or_else(|| anyhow!("{}: no format for the window", info.name))?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("launcher"),
            required_limits: wgpu::Limits::downlevel_defaults().using_resolution(adapter.limits()),
            ..Default::default()
        }))
        .map_err(|e| anyhow!("{}: {e}", info.name))?;
        let lost = Arc::new(Mutex::new(None));
        {
            let lost = lost.clone();
            device.set_device_lost_callback(move |reason, message| {
                log::warn!("launcher: graphics device lost ({reason:?}): {message}");
                *lost.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(format!("{reason:?}: {message}"));
            });
        }
        device.on_uncaptured_error(Arc::new(|e: wgpu::Error| {
            log::warn!("launcher: graphics error: {e}")
        }));
        let adapter_name = format!("{} ({:?})", info.name, info.backend);
        log::info!("launcher: drawing on {adapter_name}, {format:?}");
        Ok(Gfx {
            device,
            queue,
            format,
            adapter_name,
            lost,
        })
    }

    /// Let the device go (a lost one without wgpu's panic reaching the launcher).
    pub fn close(self) {
        let Gfx { device, queue, .. } = self;
        omsi_render::catch(move || {
            drop(queue);
            device.destroy();
            drop(device);
        });
    }

    pub fn format(&self) -> wgpu::TextureFormat {
        self.format
    }

    pub fn device_lost(&self) -> Option<String> {
        self.lost.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    /// The bytes of a buffer the GPU copied into (none when the device went on the way).
    pub fn read_buffer(&self, buf: &wgpu::Buffer) -> Option<Vec<u8>> {
        let (tx, rx) = std::sync::mpsc::channel();
        let slice = buf.slice(..);
        omsi_render::catch(|| slice.map_async(wgpu::MapMode::Read, move |r| drop(tx.send(r))))?;
        omsi_render::catch(|| self.device.poll(wgpu::PollType::wait_indefinitely()))?.ok()?;
        rx.try_recv().ok()?.ok()?;
        let data = omsi_render::catch(|| slice.get_mapped_range().to_vec())?;
        omsi_render::catch(|| buf.unmap());
        Some(data)
    }
}

/// The window's surface on the launcher's device.
pub struct WinSurface {
    /// (let go of in `drop`, unless the device was lost - see there)
    pub surface: std::mem::ManuallyDrop<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    lost: Arc<Mutex<Option<String>>>,
}

impl Drop for WinSurface {
    fn drop(&mut self) {
        // As the game's own window surface (`omsi_render::SurfaceState`): after a lost device
        // the frame it was drawing never finishes, and letting the surface go tears the
        // swapchain down under it (a Vulkan validation panic, and the launcher gone instead of
        // drawing again on a new device, #274). The window keeps it until the process ends.
        if !std::thread::panicking()
            && self
                .lost
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_none()
        {
            // SAFETY: dropped only here, once
            unsafe { std::mem::ManuallyDrop::drop(&mut self.surface) };
        }
    }
}

impl WinSurface {
    pub fn new(
        instance: &wgpu::Instance,
        window: Arc<Window>,
        gfx: &Gfx,
        width: u32,
        height: u32,
    ) -> Result<WinSurface> {
        let surface = instance
            .create_surface(window)
            .map_err(|e| anyhow!("create_surface: {e}"))?;
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: gfx.format(),
            width: width.max(1),
            height: height.max(1),
            present_mode: wgpu::PresentMode::AutoVsync,
            desired_maximum_frame_latency: 2,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
        };
        surface.configure(&gfx.device, &config);
        Ok(WinSurface {
            surface: std::mem::ManuallyDrop::new(surface),
            config,
            lost: gfx.lost.clone(),
        })
    }

    pub fn resize(&mut self, gfx: &Gfx, width: u32, height: u32) {
        self.config.width = width.max(1);
        self.config.height = height.max(1);
        // (not on a lost device: configuring it again then tore the swapchain down)
        if gfx.device_lost().is_some() {
            return;
        }
        self.surface.configure(&gfx.device, &self.config);
    }
}
