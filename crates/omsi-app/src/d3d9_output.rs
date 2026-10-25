//! The "DirectX9 (D3D9Ex)" renderer of the game window. `omsi-render` keeps doing the books
//! on wgpu's noop device (the scene, culling, level of detail, the alpha order), as the
//! dedicated server does, and hands its asset events and each window picture's frame plan
//! to the Direct3D 9Ex device of `omsi-render-d3d9`, which draws them into the window and
//! presents them.
//!
//! A window picture is drawn in three steps into one Direct3D frame: the plan sink draws the
//! scene and the HUD's pictures over it and leaves the frame open; the new interface's
//! `omsi-ui` draws, mirrored ([`D3d9Output::ui_mirror`]), go over that while the renderer
//! records its own; and [`D3d9Output::present`] ends the frame and shows it once the game's
//! frame is done.
//!
//! What the plan does not carry yet is not drawn in this mode: the sky, shadows, lamps, the
//! OMSI material maps, and pictures drawn on the GPU into textures (maps, mirrors; see the
//! crate's README).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use omsi_render::{
    FramePlan, MirrorEvent, MirrorTexture, PlanKind, Renderer, Scene, TexAddressing,
};
use omsi_render_d3d9::d3d9::D3d9Device;
use omsi_render_d3d9::{
    BackendError, DrawKind, FrameDesc, RenderBackend, SceneBatch, SceneFrame, SceneLamps,
    SceneLight, SceneMaterial, SceneTexture, TexAddress, TexFormat, UiDraw, UiFrame, UiLayer,
    UiOverlay,
};
use omsi_ui::UiEvent;
use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
use winit::window::Window;

/// The device and what each frame reuses.
struct Inner {
    device: D3d9Device,
    size: (u32, u32),
    main: Vec<SceneBatch>,
    cab: Vec<SceneBatch>,
    overlays: Vec<UiOverlay>,
    layers: Vec<UiLayer>,
    draws: Vec<UiDraw>,
    /// A frame is begun and waits for [`D3d9Output::present`].
    open: bool,
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
            MirrorEvent::LightMapTile { slot, rgba } => self.device.set_tile_light_map(
                omsi_render::LM_ATLAS_TILES * omsi_render::LM_TILE_PX,
                omsi_render::LM_TILE_PX,
                slot,
                rgba,
            ),
        };
        match done {
            // (kept by the device: the next frame finds it lost, and `recover` puts it up)
            Ok(()) | Err(BackendError::DeviceLost) => {}
            Err(e) => self.warn_once("an asset could not be put up", e),
        }
    }

    fn frame(&mut self, plan: &FramePlan, scene: &Scene) {
        // (a render texture's picture - a mirror's - is drawn on its own, whatever frame is
        // open: it is no window picture, and has no overlays)
        let target = plan.target;
        if target.is_none() {
            // (a frame the game never presented - a second picture in one game frame - is
            // shown first, so the device is not left inside a scene)
            self.present();
            if let Err(e) = self.begin(plan.clear) {
                self.failed(e);
                return;
            }
        }
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
        let lights = scene.frame_lights();
        let frame = SceneFrame {
            view_proj: plan.view_proj,
            light: SceneLight {
                cam_pos: plan.cam_pos,
                sun_dir: plan.sun_dir,
                sun_color: plan.sun_color,
                ambient: plan.ambient,
                fog: plan.fog,
                sky_color: plan.sky_color,
                cam_right: plan.cam_right,
                cam_up: plan.cam_up,
            },
            list: &plan.list,
            models: scene.slot_models(),
            params: scene.slot_params(),
            main: &self.main,
            cab: &self.cab,
            surface_bias: omsi_cfg::flags::SURFACE_BIAS.number::<i32>().unwrap_or(-24),
            lamps: SceneLamps {
                lights: &lights,
                grid: scene.frame_light_grid(),
                place: plan.light_grid,
                tile_map: plan.light_map,
            },
        };
        let material = |id: usize| {
            scene.materials.get(id).map(|m| SceneMaterial {
                texture: m.texture,
                transmap: m.transmap.map(|t| t.0),
                nightmap: m.nightmap,
                lightmap: m.lightmap,
                envmap: m.envmap.map(|e| e.0),
                env_mask: m.env_mask,
                bump: m.bump.map(|b| b.0),
                address: match m.addressing() {
                    TexAddressing::Wrap => TexAddress::Wrap,
                    TexAddressing::Mirror => TexAddress::Mirror,
                    TexAddressing::Clamp => TexAddress::Clamp,
                    TexAddressing::MirrorOnce => TexAddress::MirrorOnce,
                },
                uniform: m.uniform_vectors(),
            })
        };
        self.overlays.clear();
        self.overlays
            .extend(plan.overlays.iter().map(|&(texture, rect)| UiOverlay {
                texture,
                rect,
                premultiplied: scene.premultiplied.contains(&texture),
                transposed: scene.transposed.contains(&texture),
            }));
        let device = &mut self.device;
        if let Some(texture) = target {
            if let Err(e) = device.draw_scene_into(
                texture,
                (plan.width, plan.height),
                plan.clear,
                &frame,
                material,
            ) {
                match e {
                    BackendError::DeviceLost | BackendError::DeviceNotReset => self.failed(e),
                    e => self.warn_once("a mirror could not be drawn", e),
                }
            }
            return;
        }
        let drawn = device
            .draw_scene(&frame, material)
            .and_then(|_| device.draw_overlays(&self.overlays));
        if let Err(e) = drawn {
            // (the frame stays open: what follows is still drawn, and it is shown)
            self.warn_once("the scene could not be drawn", e);
        }
    }

    fn begin(&mut self, clear: [f32; 4]) -> std::result::Result<(), BackendError> {
        let (width, height) = self.size;
        self.device.begin_frame(&FrameDesc {
            width,
            height,
            clear_rgba: clear,
        })?;
        self.open = true;
        Ok(())
    }

    /// Ends the open frame and shows it.
    fn present(&mut self) {
        if !std::mem::take(&mut self.open) {
            return;
        }
        if let Err(e) = self.device.end_frame() {
            self.failed(e);
        }
    }

    fn ui(&mut self, owner: usize, event: UiEvent<'_>) {
        let done = match event {
            UiEvent::Image {
                id,
                width,
                height,
                rgba,
            } => self.device.ui_image(owner, id, width, height, rgba),
            UiEvent::Free(id) => {
                self.device.ui_free(owner, id);
                Ok(())
            }
            UiEvent::Atlas { size, rgba, region } => {
                self.device.ui_atlas(owner, size, rgba, region)
            }
            UiEvent::Vertices { buffer, vertices } => {
                self.device
                    .ui_vertices(owner, buffer, bytemuck::cast_slice(vertices))
            }
            UiEvent::Render {
                target,
                size,
                clear,
                layers,
                draws,
            } => {
                if target.is_none() && !self.open {
                    // (drawn outside a window picture: nothing to draw into)
                    return;
                }
                self.layers.clear();
                self.layers.extend(layers.iter().map(|l| UiLayer {
                    view_proj: l.view_proj.to_cols_array_2d(),
                    viewport: l.viewport,
                    clip: l.clip,
                    radius: l.radius,
                    opacity: l.opacity,
                    px_scale: l.px_scale,
                }));
                self.draws.clear();
                self.draws.extend(draws.iter().map(|d| UiDraw {
                    buffer: d.buffer,
                    range: d.range.clone(),
                    layer: d.layer,
                    texture: d.texture,
                }));
                let frame = UiFrame {
                    size,
                    clear,
                    layers: &self.layers,
                    draws: &self.draws,
                };
                match target {
                    // (a render texture of the renderer's: the minimap, the city map)
                    Some(texture) => self.device.draw_ui_into(owner, texture, &frame),
                    None => self.device.draw_ui(owner, &frame),
                }
            }
            UiEvent::Gone => {
                self.device.ui_gone(owner);
                Ok(())
            }
        };
        if let Err(e) = done {
            self.warn_once("the interface could not be drawn", e);
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
            overlays: Vec::new(),
            layers: Vec::new(),
            draws: Vec::new(),
            open: false,
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

    /// Ends and shows the frame the renderer's picture began (none: nothing happens).
    pub(crate) fn present(&self) {
        self.lock().present();
    }

    /// A mirror for an `omsi-ui` Gpu that draws over the window's picture: its pictures,
    /// vertex lists and draws go to this device too, under an owner of their own.
    pub(crate) fn ui_mirror(&self) -> omsi_ui::UiMirror {
        static OWNERS: AtomicUsize = AtomicUsize::new(1);
        let owner = OWNERS.fetch_add(1, Ordering::Relaxed);
        let inner = self.inner.clone();
        Box::new(move |event| {
            inner
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .ui(owner, event);
        })
    }
}
