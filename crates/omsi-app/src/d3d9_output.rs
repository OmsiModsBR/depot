//! The "DirectX9 (D3D9Ex)" renderer of the game window. `omsi-render` keeps doing the books
//! on wgpu's noop device (the scene, culling, level of detail, the alpha order), as the
//! dedicated server does, and hands its asset events and each window picture's frame plan
//! to the Direct3D 9Ex device of `omsi-render-d3d9`, which draws them into the window and
//! presents them.
//!
//! What the plan does not carry yet is not drawn in this mode: the UI, the HUD's pictures,
//! the sky, shadows, lamps and the OMSI material maps (see the crate's README).

use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use omsi_render::{FramePlan, MirrorEvent, MirrorTexture, PlanKind, Renderer, Scene};
use omsi_render_d3d9::d3d9::D3d9Device;
use omsi_render_d3d9::{
    BackendError, DrawKind, FrameDesc, RenderBackend, SceneBatch, SceneFrame, SceneLight,
    SceneMaterial, SceneTexture, TexFormat,
};
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// The device and what each frame reuses.
struct Inner {
    device: D3d9Device,
    size: (u32, u32),
    main: Vec<SceneBatch>,
    cab: Vec<SceneBatch>,
    /// Problems already logged (each kind once, not every frame or every asset).
    warned: hashbrown::HashSet<&'static str>,
    /// The device draws into this window: it lives as long as the device.
    _window: Arc<Window>,
}

// SAFETY: the device is made without D3DCREATE_MULTITHREADED, which asks only that its calls
// are not made at the same time: every call goes through the Mutex around `Inner`. The game
// calls it from its main thread (the renderer's hooks run there); the one place a renderer
// is sent to another thread drops it there, and COM objects may be released from any thread.
unsafe impl Send for Inner {}

impl Inner {
    fn warn_once(&mut self, what: &'static str, detail: impl std::fmt::Display) {
        if self.warned.insert(what) {
            log::warn!("d3d9: {what}: {detail} (further ones are not logged)");
        }
    }

    fn asset(&mut self, event: MirrorEvent<'_>) {
        let done = match event {
            MirrorEvent::Mesh {
                id,
                vertices,
                indices,
            } => self
                .device
                .set_scene_mesh(id, bytemuck::cast_slice(vertices), indices),
            MirrorEvent::MeshVertices { id, vertices } => self
                .device
                .set_scene_mesh_vertices(id, bytemuck::cast_slice(vertices)),
            MirrorEvent::FreeMesh(id) => {
                self.device.free_scene_mesh(id);
                Ok(())
            }
            MirrorEvent::MoveMesh { from, into } => {
                self.device.move_scene_mesh(from, into);
                Ok(())
            }
            MirrorEvent::Texture { id, texture } => {
                let t = match texture {
                    MirrorTexture::Image { image, mipmaps } => SceneTexture {
                        width: image.width,
                        height: image.height,
                        format: TexFormat::Rgba8,
                        levels: std::slice::from_ref(&image.rgba),
                        make_mips: mipmaps,
                    },
                    MirrorTexture::Data(data) => SceneTexture {
                        width: data.width,
                        height: data.height,
                        format: match data.format {
                            omsi_texture::PixelFormat::Rgba8 => TexFormat::Rgba8,
                            omsi_texture::PixelFormat::Bc1 => TexFormat::Bc1,
                            omsi_texture::PixelFormat::Bc2 => TexFormat::Bc2,
                            omsi_texture::PixelFormat::Bc3 => TexFormat::Bc3,
                        },
                        levels: &data.levels,
                        make_mips: data.gpu_mips,
                    },
                };
                self.device.set_scene_texture(id, &t)
            }
            MirrorEvent::FreeTexture(id) => {
                self.device.free_scene_texture(id);
                Ok(())
            }
            MirrorEvent::MoveTexture { from, into } => {
                self.device.move_scene_texture(from, into);
                Ok(())
            }
        };
        match done {
            // (kept by the device: the next frame finds it lost, and `recover` puts it up)
            Ok(()) | Err(BackendError::DeviceLost) => {}
            Err(e) => self.warn_once("an asset could not be put up", e),
        }
    }

    fn frame(&mut self, plan: &FramePlan, scene: &Scene) {
        let batch = |b: &omsi_render::PlanBatch| SceneBatch {
            kind: match b.kind {
                PlanKind::Opaque => DrawKind::Opaque,
                PlanKind::AlphaTest => DrawKind::AlphaTest,
                PlanKind::Blend => DrawKind::Blend,
                PlanKind::BlendNoWrite => DrawKind::BlendNoWrite,
                PlanKind::SurfaceDepth => DrawKind::SurfaceDepth,
                PlanKind::TerrainPaint => DrawKind::TerrainPaint,
            },
            cull: b.cull,
            surface: b.surface,
            mesh: b.mesh,
            first: b.first,
            count: b.count,
            material: b.material,
            instances: b.instances.clone(),
        };
        self.main.clear();
        self.main.extend(plan.main.iter().map(batch));
        self.cab.clear();
        self.cab.extend(plan.cab.iter().map(batch));
        if plan.sky_color[3] < 0.5 {
            self.warn_once(
                "look",
                "only the vanilla look is drawn here; the chosen one is drawn vanilla",
            );
        }
        let frame = SceneFrame {
            view_proj: plan.view_proj,
            light: SceneLight {
                cam_pos: plan.cam_pos,
                sun_dir: plan.sun_dir,
                sun_color: plan.sun_color,
                ambient: plan.ambient,
                fog: plan.fog,
                sky_color: plan.sky_color,
            },
            list: &plan.list,
            models: scene.slot_models(),
            params: scene.slot_params(),
            main: &self.main,
            cab: &self.cab,
            surface_bias: omsi_cfg::flags::SURFACE_BIAS.number::<i32>().unwrap_or(-24),
        };
        let material = |id: usize| {
            scene.materials.get(id).map(|m| SceneMaterial {
                texture: m.texture,
                transmap: m.transmap.map(|t| t.0),
                uniform: m.uniform_vectors(),
            })
        };
        let (width, height) = self.size;
        let device = &mut self.device;
        let drawn = device
            .begin_frame(&FrameDesc {
                width,
                height,
                clear_rgba: plan.clear,
            })
            .and_then(|_| {
                let scene_drawn = device.draw_scene(&frame, material);
                // the frame is ended (and shown) even when the scene failed
                let ended = device.end_frame().map(drop);
                scene_drawn.and(ended)
            });
        if let Err(e) = drawn {
            self.failed(e);
        }
    }

    fn begin(&mut self, clear: [f32; 4]) -> std::result::Result<(), BackendError> {
        let (width, height) = self.size;
        self.device.begin_frame(&FrameDesc {
            width,
            height,
            clear_rgba: clear,
        })
    }

    fn clear(&mut self, rgb: [f32; 3]) {
        let shown = self
            .begin([rgb[0], rgb[1], rgb[2], 1.0])
            .and_then(|_| self.device.end_frame().map(drop));
        if let Err(e) = shown {
            self.failed(e);
        }
    }

    fn failed(&mut self, e: BackendError) {
        match e {
            BackendError::DeviceLost => {
                // a removed device (driver reset): a new one is opened, and the scene's
                // meshes and textures go up again from the copies the device keeps
                log::error!("d3d9: the device was lost; opening it again");
                if let Err(e) = self.device.recover() {
                    self.warn_once("the device could not be opened again", e);
                }
            }
            // occluded or being reset: the next frame tries again
            BackendError::DeviceNotReset => {}
            e => self.warn_once("a frame could not be drawn", e),
        }
    }
}

/// The Direct3D 9Ex output of the game window, fed by the renderer's hooks.
pub(crate) struct D3d9Output {
    inner: Arc<Mutex<Inner>>,
}

impl D3d9Output {
    /// Opens the device on `window` and hooks it to `renderer` (which must run on wgpu's noop
    /// device, before its first asset goes up).
    pub(crate) fn attach(
        renderer: &mut Renderer,
        window: &Arc<Window>,
        vsync: bool,
    ) -> Result<Self> {
        let hwnd = match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(h)) => {
                windows::Win32::Foundation::HWND(h.hwnd.get() as *mut std::ffi::c_void)
            }
            other => return Err(anyhow!("not a Windows window: {other:?}")),
        };
        let size = window.inner_size();
        let (w, h) = (size.width.max(1), size.height.max(1));
        // SAFETY: the window is held by `Inner` for as long as the device lives.
        let mut device = unsafe { D3d9Device::open_window(hwnd, w, h) }
            .map_err(|e| anyhow!("Direct3D 9Ex: {e}"))?;
        device
            .set_vsync(vsync)
            .map_err(|e| anyhow!("Direct3D 9Ex: {e}"))?;
        log::info!("graphics: DirectX9 (D3D9Ex) on {:?}", device.caps());
        let inner = Arc::new(Mutex::new(Inner {
            device,
            size: (w, h),
            main: Vec::new(),
            cab: Vec::new(),
            warned: Default::default(),
            _window: window.clone(),
        }));
        let assets = inner.clone();
        renderer.set_asset_mirror(Some(Box::new(move |event| {
            assets
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .asset(event);
        })));
        let frames = inner.clone();
        renderer.set_plan_sink(Some(Box::new(move |plan, scene| {
            frames
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .frame(plan, scene);
        })));
        Ok(Self { inner })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// The window's new size.
    pub(crate) fn resize(&self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            // (minimised: nothing is shown, the size comes back with the window)
            return;
        }
        let mut inner = self.lock();
        inner.size = (width, height);
        if let Err(e) = inner.device.resize(width, height) {
            inner.failed(e);
        }
    }

    /// The game starts a new scene: what the old one put up goes (its ids begin again).
    pub(crate) fn new_scene(&self) {
        self.lock().device.clear_scene();
    }

    /// Presents wait for the display's refresh, or not.
    pub(crate) fn set_vsync(&self, on: bool) {
        let mut inner = self.lock();
        if let Err(e) = inner.device.set_vsync(on) {
            inner.failed(e);
        }
    }

    /// Shows the window cleared to `rgb` (the screens this renderer does not draw yet: the
    /// loading screen, the menus).
    pub(crate) fn present_clear(&self, rgb: [f32; 3]) {
        self.lock().clear(rgb);
    }
}
