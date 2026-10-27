//! The Direct3D 9Ex renderer. Windows only; all `unsafe` FFI of the crate lives in this module.
//!
//! Direct3D 9Ex is used rather than plain Direct3D 9: this crate needs Windows 10 or later
//! (the minimum of Rust's Windows targets), where 9Ex is always present, and with it resources survive the device being occluded or the
//! display mode changing. Only a removed device (driver reset) has to be rebuilt.
//!
//! Two layers: [`RenderBackend`] (an off-screen or window target, textures of one level,
//! meshes and the three alpha modes), and the game's scene on top of it
//! (`D3d9Device::set_scene_mesh`, `set_scene_texture`, `draw_scene`, see [`crate::scene`]).
//! Shader Model 3.0 is the floor.

mod scene;
pub mod shader;
mod ui;

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Direct3D9::*;
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

use crate::backend::{
    handle_index, AlphaMode, DrawBatch, FrameDesc, MeshDesc, MeshHandle, RenderBackend,
    TextureDesc, TextureHandle,
};
use crate::{BackendError, Caps, Result, ShaderModel};

/// Reference value of [`AlphaMode::Test`]: a pixel is kept when its alpha is at least this
/// (0-255). PROVISIONAL: the value the original applies is a parity item still to be
/// observed; until then the midpoint is used.
pub const ALPHA_TEST_REF: u32 = 128;

// HRESULTs of the device states (d3d9.h). DEVICEHUNG/DEVICEREMOVED need a new device.
const DEVICE_LOST: windows::core::HRESULT = windows::core::HRESULT(0x88760868_u32 as i32);
const DEVICE_NOT_RESET: windows::core::HRESULT = windows::core::HRESULT(0x88760869_u32 as i32);
// D3DERR_NOTAVAILABLE: no hardware device of the requested kind (no usable driver).
const NOT_AVAILABLE: windows::core::HRESULT = windows::core::HRESULT(0x8876086A_u32 as i32);
const DEVICE_REMOVED: windows::core::HRESULT = windows::core::HRESULT(0x88760870_u32 as i32);
const DEVICE_HUNG: windows::core::HRESULT = windows::core::HRESULT(0x88760874_u32 as i32);

fn map_err(e: windows::core::Error) -> BackendError {
    match e.code() {
        c if c == DEVICE_LOST || c == DEVICE_REMOVED || c == DEVICE_HUNG => {
            BackendError::DeviceLost
        }
        c if c == DEVICE_NOT_RESET => BackendError::DeviceNotReset,
        c if c == NOT_AVAILABLE => BackendError::Unsupported(e.to_string()),
        _ => BackendError::Backend(e.to_string()),
    }
}

fn some<T>(v: Option<T>, what: &str) -> Result<T> {
    v.ok_or_else(|| BackendError::Backend(format!("no {what} returned")))
}

/// Where frames go.
#[derive(Debug, Clone, Copy)]
enum Target {
    /// Off-screen render targets sized per frame; `end_frame` reads them back.
    Offscreen,
    /// The window's back buffer; `end_frame` presents it.
    Window(HWND),
}

struct Mesh {
    vb: IDirect3DVertexBuffer9,
    ib: IDirect3DIndexBuffer9,
    vertices: u32,
    triangles: u32,
}

struct Offscreen {
    width: u32,
    height: u32,
    color: IDirect3DSurface9,
    depth: IDirect3DSurface9,
}

/// Pipeline objects that do not depend on content.
struct Pipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    ps: IDirect3DPixelShader9,
    /// Bound when a batch has no texture.
    white: IDirect3DTexture9,
}

/// An open Direct3D 9Ex device on the default adapter.
pub struct D3d9Device {
    // Held for the device's lifetime; COM reference counting keeps the drop order safe.
    _d3d: IDirect3D9Ex,
    device: IDirect3DDevice9Ex,
    caps: Caps,
    target: Target,
    window_size: (u32, u32),
    pipeline: Pipeline,
    textures: Vec<IDirect3DTexture9>,
    meshes: Vec<Mesh>,
    /// Bumped by `recover`, so handles from before it are rejected.
    generation: u32,
    offscreen: Option<Offscreen>,
    frame: Option<FrameDesc>,
    /// The game's scene ([`crate::scene`]), under the game's ids.
    scene: scene::SceneStore,
    /// What the scene layer may use beyond the floor.
    scene_caps: scene::SceneCaps,
    /// The game's interface ([`crate::ui`]), under each owner's ids.
    ui: ui::UiStore,
    /// Presents wait for the display's refresh.
    vsync: bool,
    /// The multisampling the player asked for (1 = off) and what the card gave of it.
    msaa: (u32, u32),
    /// The anisotropic filtering the player asked for (1 = off); the card's own limit.
    anisotropy: u32,
    max_anisotropy: u32,
    /// How the adapter's driver switches alpha-to-coverage on.
    a2c: scene::A2c,
}

fn present_params(
    target: Target,
    width: u32,
    height: u32,
    vsync: bool,
    samples: u32,
) -> D3DPRESENT_PARAMETERS {
    let (hwnd, w, h) = match target {
        // 1x1 back buffer: an off-screen device never presents.
        Target::Offscreen => (HWND::default(), 1, 1),
        Target::Window(hwnd) => (hwnd, width.max(1), height.max(1)),
    };
    D3DPRESENT_PARAMETERS {
        Windowed: true.into(),
        SwapEffect: D3DSWAPEFFECT_DISCARD,
        BackBufferFormat: D3DFMT_UNKNOWN,
        BackBufferWidth: w,
        BackBufferHeight: h,
        hDeviceWindow: hwnd,
        EnableAutoDepthStencil: true.into(),
        AutoDepthStencilFormat: D3DFMT_D24X8,
        MultiSampleType: D3DMULTISAMPLE_TYPE(if samples > 1 { samples as i32 } else { 0 }),
        PresentationInterval: if vsync {
            D3DPRESENT_INTERVAL_ONE as u32
        } else {
            D3DPRESENT_INTERVAL_IMMEDIATE as u32
        },
        ..Default::default()
    }
}

fn read_caps(d3d: &IDirect3D9Ex) -> Result<(Caps, D3DCAPS9)> {
    let mut raw = D3DCAPS9::default();
    let mut ident = D3DADAPTER_IDENTIFIER9::default();
    // SAFETY: out-parameters point to live locals.
    unsafe {
        d3d.GetDeviceCaps(D3DADAPTER_DEFAULT, D3DDEVTYPE_HAL, &mut raw)
            .map_err(map_err)?;
        d3d.GetAdapterIdentifier(D3DADAPTER_DEFAULT, 0, &mut ident)
            .map_err(map_err)?;
    }
    let name: Vec<u8> = ident
        .Description
        .iter()
        .take_while(|&&c| c != 0)
        .map(|&c| c as u8)
        .collect();
    let major = |v: u32| ((v >> 8) & 0xff) as u8;
    let caps = Caps {
        adapter_name: String::from_utf8_lossy(&name).into_owned(),
        // The weaker of the two stages decides.
        shader_model: ShaderModel::from_major(major(raw.PixelShaderVersion))
            .min(ShaderModel::from_major(major(raw.VertexShaderVersion))),
        max_texture_size: raw.MaxTextureWidth.min(raw.MaxTextureHeight),
        non_pow2_textures: raw.TextureCaps & D3DPTEXTURECAPS_POW2 as u32 == 0,
        index32: raw.MaxVertexIndex > 0xFFFF,
        max_render_targets: raw.NumSimultaneousRTs.max(1),
    };
    Ok((caps, raw))
}

fn rgba_to_argb(c: [f32; 4]) -> u32 {
    let to8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
    (to8(c[3]) << 24) | (to8(c[0]) << 16) | (to8(c[1]) << 8) | to8(c[2])
}

/// Uploads a mesh's vertices (8 floats each) and indices (16-bit when they fit) into
/// write-only default-pool buffers. The data must have been validated.
fn make_mesh(device: &IDirect3DDevice9Ex, vertices: &[f32], indices: &[u32]) -> Result<Mesh> {
    let count = (vertices.len() / crate::backend::VERTEX_FLOATS) as u32;
    let wide = count > 0x1_0000;
    let vb_len = std::mem::size_of_val(vertices);
    let ib_len = indices.len() * if wide { 4 } else { 2 };
    // SAFETY: out-parameters are live locals; locked ranges are exactly the lengths the
    // buffers were created with.
    unsafe {
        let mut vb = None;
        device
            .CreateVertexBuffer(
                vb_len as u32,
                D3DUSAGE_WRITEONLY as u32,
                0,
                D3DPOOL_DEFAULT,
                &mut vb,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let vb = some(vb, "vertex buffer")?;
        write_buffer(&vb, vertices)?;

        let format = if wide { D3DFMT_INDEX32 } else { D3DFMT_INDEX16 };
        let mut ib = None;
        device
            .CreateIndexBuffer(
                ib_len as u32,
                D3DUSAGE_WRITEONLY as u32,
                format,
                D3DPOOL_DEFAULT,
                &mut ib,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let ib = some(ib, "index buffer")?;
        let mut p = std::ptr::null_mut();
        ib.Lock(0, 0, &mut p, 0).map_err(map_err)?;
        if wide {
            std::ptr::copy_nonoverlapping(indices.as_ptr() as *const u8, p as *mut u8, ib_len);
        } else {
            let narrow: Vec<u16> = indices.iter().map(|&i| i as u16).collect();
            std::ptr::copy_nonoverlapping(narrow.as_ptr() as *const u8, p as *mut u8, ib_len);
        }
        ib.Unlock().map_err(map_err)?;
        Ok(Mesh {
            vb,
            ib,
            vertices: count,
            triangles: indices.len() as u32 / 3,
        })
    }
}

/// Overwrites the start of `vb` with `data`.
///
/// # Safety
/// `vb` must hold at least `size_of_val(data)` bytes.
unsafe fn write_buffer(vb: &IDirect3DVertexBuffer9, data: &[f32]) -> Result<()> {
    let len = std::mem::size_of_val(data);
    let mut p = std::ptr::null_mut();
    // SAFETY: the caller guarantees the buffer's size; the lock covers `len` bytes.
    unsafe {
        vb.Lock(0, len as u32, &mut p, 0).map_err(map_err)?;
        std::ptr::copy_nonoverlapping(data.as_ptr() as *const u8, p as *mut u8, len);
        vb.Unlock().map_err(map_err)
    }
}

impl D3d9Device {
    /// Opens a device that renders off-screen; [`RenderBackend::end_frame`] returns pixels.
    pub fn open() -> Result<Self> {
        Self::open_with(Target::Offscreen, 1, 1)
    }

    /// Opens a device that renders to `hwnd`'s client area of `width` x `height`.
    ///
    /// # Safety
    /// `hwnd` must be a valid window that outlives the device.
    pub unsafe fn open_window(hwnd: HWND, width: u32, height: u32) -> Result<Self> {
        Self::open_with(Target::Window(hwnd), width, height)
    }

    fn open_with(target: Target, width: u32, height: u32) -> Result<Self> {
        // SAFETY: plain COM creation; every out-parameter points to a live local.
        unsafe {
            let d3d = Direct3DCreate9Ex(D3D_SDK_VERSION)
                .map_err(|e| BackendError::Unsupported(format!("Direct3D 9Ex: {e}")))?;
            let (caps, raw) = read_caps(&d3d)?;
            if !caps.meets_floor() {
                return Err(BackendError::Unsupported(format!(
                    "{} has {:?} and textures up to {}; Shader Model 3.0 and 2048 are needed",
                    caps.adapter_name, caps.shader_model, caps.max_texture_size
                )));
            }
            let vertex_processing = if raw.DevCaps & D3DDEVCAPS_HWTRANSFORMANDLIGHT as u32 != 0 {
                D3DCREATE_HARDWARE_VERTEXPROCESSING
            } else {
                D3DCREATE_SOFTWARE_VERTEXPROCESSING
            };
            // FPU_PRESERVE: without it Direct3D 9 switches the thread's x87 unit to single
            // precision, which would silently change floating-point results on that thread.
            let flags = (vertex_processing | D3DCREATE_FPU_PRESERVE) as u32;
            let focus = match target {
                Target::Offscreen => GetDesktopWindow(),
                Target::Window(hwnd) => hwnd,
            };
            let mut pp = present_params(target, width, height, true, 1);
            let mut device = None;
            d3d.CreateDeviceEx(
                D3DADAPTER_DEFAULT,
                D3DDEVTYPE_HAL,
                focus,
                flags,
                &mut pp,
                std::ptr::null_mut(),
                &mut device,
            )
            .map_err(map_err)?;
            let device = some(device, "device")?;
            let pipeline = Self::build_pipeline(&device)?;
            let a2c = scene::A2c::probe(&d3d);
            let scene_caps = scene::SceneCaps::read(&d3d, &raw);
            Ok(Self {
                _d3d: d3d,
                device,
                caps,
                target,
                window_size: (width.max(1), height.max(1)),
                pipeline,
                textures: Vec::new(),
                meshes: Vec::new(),
                generation: 0,
                offscreen: None,
                frame: None,
                scene: scene::SceneStore::default(),
                scene_caps,
                ui: ui::UiStore::default(),
                vsync: true,
                msaa: (1, 1),
                anisotropy: 1,
                max_anisotropy: raw.MaxAnisotropy.max(1),
                a2c: a2c,
            })
        }
    }

    fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<Pipeline> {
        let vs_code = shader::compile(
            shader::BASIC,
            "basic.hlsl",
            "vs_main",
            shader::Stage::Vertex,
        )?;
        let ps_code =
            shader::compile(shader::BASIC, "basic.hlsl", "ps_main", shader::Stage::Pixel)?;
        let element = |offset: u16, ty: D3DDECLTYPE, usage: D3DDECLUSAGE| D3DVERTEXELEMENT9 {
            Stream: 0,
            Offset: offset,
            Type: ty.0 as u8,
            Method: D3DDECLMETHOD_DEFAULT.0 as u8,
            Usage: usage.0 as u8,
            UsageIndex: 0,
        };
        let elements = [
            element(0, D3DDECLTYPE_FLOAT3, D3DDECLUSAGE_POSITION),
            element(12, D3DDECLTYPE_FLOAT3, D3DDECLUSAGE_NORMAL),
            element(24, D3DDECLTYPE_FLOAT2, D3DDECLUSAGE_TEXCOORD),
            // D3DDECL_END()
            D3DVERTEXELEMENT9 {
                Stream: 0xFF,
                Offset: 0,
                Type: D3DDECLTYPE_UNUSED.0 as u8,
                Method: 0,
                Usage: 0,
                UsageIndex: 0,
            },
        ];
        // SAFETY: the bytecode and element arrays outlive the calls.
        unsafe {
            let decl = device
                .CreateVertexDeclaration(elements.as_ptr())
                .map_err(map_err)?;
            let vs = device
                .CreateVertexShader(vs_code.as_ptr())
                .map_err(map_err)?;
            let ps = device
                .CreatePixelShader(ps_code.as_ptr())
                .map_err(map_err)?;
            let white = Self::upload_texture(device, 1, 1, &[255, 255, 255, 255])?;
            Ok(Pipeline {
                decl,
                vs,
                ps,
                white,
            })
        }
    }

    /// Uploads RGBA8 through a system-memory staging texture into a default-pool one.
    fn upload_texture(
        device: &IDirect3DDevice9Ex,
        w: u32,
        h: u32,
        rgba: &[u8],
    ) -> Result<IDirect3DTexture9> {
        // SAFETY: out-parameters are live locals; the locked memory is written only between
        // LockRect and UnlockRect and within `pitch * h` bytes.
        unsafe {
            let mut staging = None;
            device
                .CreateTexture(
                    w,
                    h,
                    1,
                    0,
                    D3DFMT_A8R8G8B8,
                    D3DPOOL_SYSTEMMEM,
                    &mut staging,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
            let staging = some(staging, "staging texture")?;
            let mut locked = D3DLOCKED_RECT::default();
            staging
                .LockRect(0, &mut locked, std::ptr::null(), 0)
                .map_err(map_err)?;
            for y in 0..h as usize {
                let dst = std::slice::from_raw_parts_mut(
                    (locked.pBits as *mut u8).add(y * locked.Pitch as usize),
                    w as usize * 4,
                );
                let src = &rgba[y * w as usize * 4..(y + 1) * w as usize * 4];
                // Memory order is B, G, R, A.
                for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                    d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
                }
            }
            staging.UnlockRect(0).map_err(map_err)?;
            let mut texture = None;
            device
                .CreateTexture(
                    w,
                    h,
                    1,
                    0,
                    D3DFMT_A8R8G8B8,
                    D3DPOOL_DEFAULT,
                    &mut texture,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
            let texture = some(texture, "texture")?;
            device.UpdateTexture(&staging, &texture).map_err(map_err)?;
            Ok(texture)
        }
    }

    /// Resizes the window target. Resources survive (Direct3D 9Ex).
    pub fn resize(&mut self, width: u32, height: u32) -> Result<()> {
        if let Target::Window(_) = self.target {
            self.window_size = (width.max(1), height.max(1));
            let mut pp = present_params(self.target, width, height, self.vsync, self.msaa.1);
            // SAFETY: `pp` outlives the call.
            unsafe { self.device.ResetEx(&mut pp, std::ptr::null_mut()) }.map_err(map_err)?;
        }
        Ok(())
    }

    /// The multisampling (2, 4, 8; 1 = off) and anisotropic filtering (2..16; 1 = off) of the
    /// options, as far as the card has them: the nearest lower multisample count it offers
    /// for the window's picture. Changing the count resets the device.
    pub fn set_quality(&mut self, msaa: u32, anisotropy: u32) -> Result<()> {
        let msaa = msaa.max(1);
        self.anisotropy = anisotropy.clamp(1, self.max_anisotropy);
        if self.msaa.0 == msaa {
            return Ok(());
        }
        let granted = match self.target {
            Target::Offscreen => 1,
            Target::Window(_) => self.samples_for(msaa),
        };
        self.msaa = (msaa, granted);
        if granted != msaa {
            log::warn!("d3d9: {msaa}x multisampling is not offered here; {granted}x is used");
        }
        let (w, h) = self.window_size;
        self.resize(w, h)
    }

    /// The largest sample count up to `wanted` the card offers for the window's back buffer and
    /// its depth surface (1 when none).
    fn samples_for(&self, wanted: u32) -> u32 {
        let mut mode = D3DDISPLAYMODE::default();
        // SAFETY: the out-parameter is a live local.
        let display = unsafe {
            self._d3d
                .GetAdapterDisplayMode(D3DADAPTER_DEFAULT, &mut mode)
        }
        .map(|_| mode.Format)
        .unwrap_or(D3DFMT_X8R8G8B8);
        let offered = |format: D3DFORMAT, n: u32| {
            // SAFETY: plain query on a live interface; the quality-levels out-parameter is not
            // wanted.
            unsafe {
                self._d3d.CheckDeviceMultiSampleType(
                    D3DADAPTER_DEFAULT,
                    D3DDEVTYPE_HAL,
                    format,
                    true,
                    D3DMULTISAMPLE_TYPE(n as i32),
                    std::ptr::null_mut(),
                )
            }
            .is_ok()
        };
        (2..=wanted.min(8))
            .rev()
            .find(|&n| offered(display, n) && offered(D3DFMT_D24X8, n))
            .unwrap_or(1)
    }

    /// Whether presents wait for the display's refresh (on when the device opens).
    pub fn set_vsync(&mut self, on: bool) -> Result<()> {
        if self.vsync == on {
            return Ok(());
        }
        self.vsync = on;
        let (w, h) = self.window_size;
        self.resize(w, h)
    }

    /// `Ok` while the device is usable; `DeviceLost` when it was removed and needs
    /// [`RenderBackend::recover`].
    pub fn status(&self) -> Result<()> {
        let hwnd = match self.target {
            Target::Offscreen => HWND::default(),
            Target::Window(hwnd) => hwnd,
        };
        // SAFETY: `hwnd` is the device's own window or null.
        let r = unsafe { self.device.CheckDeviceState(hwnd) };
        // S_PRESENT_OCCLUDED and friends are success codes: the window is hidden, not lost.
        r.map_err(map_err)
    }

    fn offscreen_for(&mut self, w: u32, h: u32) -> Result<(IDirect3DSurface9, IDirect3DSurface9)> {
        if let Some(o) = &self.offscreen {
            if (o.width, o.height) == (w, h) {
                return Ok((o.color.clone(), o.depth.clone()));
            }
        }
        // SAFETY: out-parameters are live locals.
        unsafe {
            let mut color = None;
            self.device
                .CreateRenderTarget(
                    w,
                    h,
                    D3DFMT_A8R8G8B8,
                    D3DMULTISAMPLE_NONE,
                    0,
                    false,
                    &mut color,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
            let mut depth = None;
            self.device
                .CreateDepthStencilSurface(
                    w,
                    h,
                    D3DFMT_D24X8,
                    D3DMULTISAMPLE_NONE,
                    0,
                    true,
                    &mut depth,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
            let (color, depth) = (some(color, "render target")?, some(depth, "depth surface")?);
            self.offscreen = Some(Offscreen {
                width: w,
                height: h,
                color: color.clone(),
                depth: depth.clone(),
            });
            Ok((color, depth))
        }
    }

    fn read_back(&self, w: u32, h: u32) -> Result<Vec<u8>> {
        let color = &self
            .offscreen
            .as_ref()
            .expect("off-screen frame has targets")
            .color;
        // SAFETY: as in `upload_texture`, reading instead of writing.
        unsafe {
            let mut sysmem = None;
            self.device
                .CreateOffscreenPlainSurface(
                    w,
                    h,
                    D3DFMT_A8R8G8B8,
                    D3DPOOL_SYSTEMMEM,
                    &mut sysmem,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
            let sysmem = some(sysmem, "read-back surface")?;
            self.device
                .GetRenderTargetData(color, &sysmem)
                .map_err(map_err)?;
            let mut locked = D3DLOCKED_RECT::default();
            sysmem
                .LockRect(&mut locked, std::ptr::null(), D3DLOCK_READONLY as u32)
                .map_err(map_err)?;
            let mut out = Vec::with_capacity((w * h * 4) as usize);
            for y in 0..h as usize {
                let row = std::slice::from_raw_parts(
                    (locked.pBits as *const u8).add(y * locked.Pitch as usize),
                    w as usize * 4,
                );
                for px in row.chunks_exact(4) {
                    out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                }
            }
            let _ = sysmem.UnlockRect();
            Ok(out)
        }
    }

    /// Fixed states of phase 1. Culling is off until the meshes' winding is settled against
    /// the content loader (phase 3).
    fn set_frame_states(&self) -> Result<()> {
        let d = &self.device;
        // SAFETY: plain state setters on a live device.
        unsafe {
            d.SetRenderState(D3DRS_ZENABLE, D3DZB_TRUE.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_ZWRITEENABLE, 1).map_err(map_err)?;
            d.SetRenderState(D3DRS_ZFUNC, D3DCMP_LESSEQUAL.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_CULLMODE, D3DCULL_NONE.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_ALPHAREF, ALPHA_TEST_REF)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_ALPHAFUNC, D3DCMP_GREATEREQUAL.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32)
                .map_err(map_err)?;
            d.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32)
                .map_err(map_err)?;
            d.SetSamplerState(0, D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32)
                .map_err(map_err)?;
            d.SetSamplerState(0, D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP.0 as u32)
                .map_err(map_err)?;
            d.SetSamplerState(0, D3DSAMP_ADDRESSV, D3DTADDRESS_WRAP.0 as u32)
                .map_err(map_err)?;
            d.SetVertexDeclaration(&self.pipeline.decl)
                .map_err(map_err)?;
            d.SetVertexShader(&self.pipeline.vs).map_err(map_err)?;
            d.SetPixelShader(&self.pipeline.ps).map_err(map_err)?;
        }
        Ok(())
    }
}

impl RenderBackend for D3d9Device {
    fn caps(&self) -> &Caps {
        &self.caps
    }

    fn create_texture(&mut self, desc: &TextureDesc) -> Result<TextureHandle> {
        desc.validate(&self.caps)?;
        let t = Self::upload_texture(&self.device, desc.width, desc.height, &desc.rgba)?;
        self.textures.push(t);
        Ok(TextureHandle {
            index: self.textures.len() as u32 - 1,
            generation: self.generation,
        })
    }

    fn create_mesh(&mut self, desc: &MeshDesc) -> Result<MeshHandle> {
        desc.validate(&self.caps)?;
        let mesh = make_mesh(&self.device, &desc.vertices, &desc.indices)?;
        self.meshes.push(mesh);
        Ok(MeshHandle {
            index: self.meshes.len() as u32 - 1,
            generation: self.generation,
        })
    }

    fn begin_frame(&mut self, frame: &FrameDesc) -> Result<()> {
        if self.frame.is_some() {
            return Err(BackendError::InvalidArgument("frame already begun".into()));
        }
        // SAFETY: plain calls on a live device with live surfaces.
        unsafe {
            match self.target {
                Target::Offscreen => {
                    if frame.width == 0
                        || frame.height == 0
                        || frame.width.max(frame.height) > self.caps.max_texture_size
                    {
                        return Err(BackendError::InvalidArgument("target size".into()));
                    }
                    let (color, depth) = self.offscreen_for(frame.width, frame.height)?;
                    self.device.SetRenderTarget(0, &color).map_err(map_err)?;
                    self.device
                        .SetDepthStencilSurface(&depth)
                        .map_err(map_err)?;
                }
                Target::Window(_) => {
                    let back = self
                        .device
                        .GetBackBuffer(0, 0, D3DBACKBUFFER_TYPE_MONO)
                        .map_err(map_err)?;
                    self.device.SetRenderTarget(0, &back).map_err(map_err)?;
                }
            }
            self.set_frame_states()?;
            self.device.BeginScene().map_err(map_err)?;
            let cleared = self.device.Clear(
                0,
                std::ptr::null(),
                (D3DCLEAR_TARGET | D3DCLEAR_ZBUFFER) as u32,
                rgba_to_argb(frame.clear_rgba),
                1.0,
                0,
            );
            if let Err(e) = cleared {
                // Close the scene, or the next `begin_frame` could never open one.
                let _ = self.device.EndScene();
                return Err(map_err(e));
            }
        }
        self.frame = Some(frame.clone());
        Ok(())
    }

    fn draw(&mut self, batch: &DrawBatch) -> Result<()> {
        if self.frame.is_none() {
            return Err(BackendError::InvalidArgument("draw outside a frame".into()));
        }
        let m = batch.mesh;
        let mesh =
            &self.meshes[handle_index(m.index, m.generation, self.generation, self.meshes.len())?];
        let texture = match batch.texture {
            Some(t) => {
                &self.textures
                    [handle_index(t.index, t.generation, self.generation, self.textures.len())?]
            }
            None => &self.pipeline.white,
        };
        let (test, blend) = match batch.alpha {
            AlphaMode::Opaque => (0, 0),
            AlphaMode::Test => (1, 0),
            AlphaMode::Blend => (0, 1),
        };
        let d = &self.device;
        // SAFETY: the constant arrays outlive the calls; the buffers belong to this device.
        unsafe {
            d.SetRenderState(D3DRS_ALPHATESTENABLE, test)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_ALPHABLENDENABLE, blend)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(0, batch.mvp.as_ptr(), 4)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(0, batch.tint.as_ptr(), 1)
                .map_err(map_err)?;
            d.SetTexture(0, texture).map_err(map_err)?;
            d.SetStreamSource(0, &mesh.vb, 0, 32).map_err(map_err)?;
            d.SetIndices(&mesh.ib).map_err(map_err)?;
            d.DrawIndexedPrimitive(D3DPT_TRIANGLELIST, 0, 0, mesh.vertices, 0, mesh.triangles)
                .map_err(map_err)?;
        }
        Ok(())
    }

    fn end_frame(&mut self) -> Result<Vec<u8>> {
        let frame = self
            .frame
            .take()
            .ok_or_else(|| BackendError::InvalidArgument("no frame".into()))?;
        // SAFETY: plain calls on a live device.
        unsafe { self.device.EndScene() }.map_err(map_err)?;
        match self.target {
            Target::Offscreen => self.read_back(frame.width, frame.height),
            Target::Window(_) => {
                // SAFETY: presents the device's own swap chain to its own window.
                let r = unsafe {
                    self.device.PresentEx(
                        std::ptr::null(),
                        std::ptr::null(),
                        HWND::default(),
                        std::ptr::null(),
                        0,
                    )
                };
                r.map_err(map_err)?;
                Ok(Vec::new())
            }
        }
    }

    fn recover(&mut self) -> Result<()> {
        let (w, h) = self.window_size;
        let generation = self.generation.wrapping_add(1);
        let vsync = self.vsync;
        let (msaa, anisotropy) = (self.msaa.0, self.anisotropy);
        let fresh = Self::open_with(self.target, w, h)?;
        // (the scene's meshes and textures go up again from their copies: the game does not
        // send them twice)
        let copies = std::mem::take(&mut self.scene.copies);
        let owners = std::mem::take(&mut self.ui.owners);
        *self = fresh;
        self.generation = generation;
        let restored = self.restore_scene(copies).and(self.restore_ui(owners));
        let quality = self.set_quality(msaa, anisotropy);
        self.set_vsync(vsync).and(restored).and(quality)
    }
}

#[cfg(test)]
mod scene_tests;
#[cfg(test)]
mod tests;
#[cfg(test)]
mod ui_tests;
