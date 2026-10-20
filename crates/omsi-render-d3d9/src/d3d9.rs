//! Direct3D 9 device bring-up. Windows only; all `unsafe` FFI of the crate lives here.
//!
//! Scope of this step: open a HAL device, read its capabilities, clear an off-screen render
//! target and read it back, and report/handle device loss. No meshes, shaders or textures yet.

use windows::Win32::Graphics::Direct3D9::*;
use windows::Win32::UI::WindowsAndMessaging::GetDesktopWindow;

use crate::{BackendError, Caps, Result, ShaderModel};

// HRESULTs of the two recoverable device states (d3d9.h: D3DERR_DEVICELOST/NOTRESET).
const DEVICE_LOST: windows::core::HRESULT = windows::core::HRESULT(0x88760868_u32 as i32);
const DEVICE_NOT_RESET: windows::core::HRESULT = windows::core::HRESULT(0x88760869_u32 as i32);
// D3DERR_NOTAVAILABLE: no hardware device of the requested kind (no usable driver).
const NOT_AVAILABLE: windows::core::HRESULT = windows::core::HRESULT(0x8876086A_u32 as i32);

fn map_err(e: windows::core::Error) -> BackendError {
    match e.code() {
        c if c == DEVICE_LOST => BackendError::DeviceLost,
        c if c == DEVICE_NOT_RESET => BackendError::DeviceNotReset,
        c if c == NOT_AVAILABLE => BackendError::Unsupported(e.to_string()),
        _ => BackendError::Backend(e.to_string()),
    }
}

/// An open Direct3D 9 device on the default adapter.
pub struct D3d9Device {
    // Held for the device's lifetime; COM reference counting keeps the drop order safe.
    _d3d: IDirect3D9,
    device: IDirect3DDevice9,
    caps: Caps,
}

fn present_params() -> D3DPRESENT_PARAMETERS {
    D3DPRESENT_PARAMETERS {
        Windowed: true.into(),
        SwapEffect: D3DSWAPEFFECT_DISCARD,
        BackBufferFormat: D3DFMT_UNKNOWN,
        // 1x1 back buffer: the device is only used for off-screen targets in this step.
        BackBufferWidth: 1,
        BackBufferHeight: 1,
        ..Default::default()
    }
}

impl D3d9Device {
    /// Opens the default adapter's HAL device. Fails (without panicking) on machines with no
    /// Direct3D 9 driver, which callers and tests treat as "not available here".
    pub fn open() -> Result<Self> {
        // SAFETY: plain COM creation; every out-parameter points to a live local.
        unsafe {
            let d3d = Direct3DCreate9(D3D_SDK_VERSION)
                .ok_or_else(|| BackendError::Unsupported("Direct3D 9 is not available".into()))?;

            let mut raw = D3DCAPS9::default();
            d3d.GetDeviceCaps(D3DADAPTER_DEFAULT, D3DDEVTYPE_HAL, &mut raw)
                .map_err(map_err)?;
            let mut ident = D3DADAPTER_IDENTIFIER9::default();
            d3d.GetAdapterIdentifier(D3DADAPTER_DEFAULT, 0, &mut ident)
                .map_err(map_err)?;

            let name_bytes: Vec<u8> = ident
                .Description
                .iter()
                .take_while(|&&c| c != 0)
                .map(|&c| c as u8)
                .collect();
            let caps = Caps {
                adapter_name: String::from_utf8_lossy(&name_bytes).into_owned(),
                shader_model: ShaderModel::from_major(((raw.PixelShaderVersion >> 8) & 0xff) as u8),
                max_texture_size: raw.MaxTextureWidth.min(raw.MaxTextureHeight),
                non_pow2_textures: raw.TextureCaps & D3DPTEXTURECAPS_POW2 as u32 == 0,
                index32: raw.MaxVertexIndex > 0xFFFF,
                max_render_targets: raw.NumSimultaneousRTs.max(1),
            };

            let mut pp = present_params();
            let mut device = None;
            d3d.CreateDevice(
                D3DADAPTER_DEFAULT,
                D3DDEVTYPE_HAL,
                // The desktop window serves as focus window; no window of ours is needed.
                GetDesktopWindow(),
                D3DCREATE_SOFTWARE_VERTEXPROCESSING as u32,
                &mut pp,
                &mut device,
            )
            .map_err(map_err)?;
            let device =
                device.ok_or_else(|| BackendError::Backend("no device returned".into()))?;
            Ok(Self {
                _d3d: d3d,
                device,
                caps,
            })
        }
    }

    pub fn caps(&self) -> &Caps {
        &self.caps
    }

    /// `Ok` while the device is usable; `DeviceLost`/`DeviceNotReset` otherwise.
    pub fn status(&self) -> Result<()> {
        // SAFETY: no arguments.
        unsafe { self.device.TestCooperativeLevel() }.map_err(map_err)
    }

    /// Resets a lost device. Every default-pool resource must be released first; this
    /// foundation creates none that outlive a call.
    pub fn reset(&mut self) -> Result<()> {
        let mut pp = present_params();
        // SAFETY: `pp` outlives the call.
        unsafe { self.device.Reset(&mut pp) }.map_err(map_err)
    }

    /// Clears a `width` x `height` off-screen target to `rgba` and returns it as RGBA8.
    pub fn clear_and_read(&self, width: u32, height: u32, rgba: [f32; 4]) -> Result<Vec<u8>> {
        if width == 0 || height == 0 || width.max(height) > self.caps.max_texture_size {
            return Err(BackendError::InvalidArgument("target size".into()));
        }
        let to8 = |c: f32| (c.clamp(0.0, 1.0) * 255.0 + 0.5) as u32;
        let argb = (to8(rgba[3]) << 24) | (to8(rgba[0]) << 16) | (to8(rgba[1]) << 8) | to8(rgba[2]);
        // SAFETY: out-pointers are live locals; the locked memory is read only between
        // LockRect and UnlockRect and within `pitch * height` bytes.
        unsafe {
            let mut target = None;
            self.device
                .CreateRenderTarget(
                    width,
                    height,
                    D3DFMT_A8R8G8B8,
                    D3DMULTISAMPLE_NONE,
                    0,
                    false,
                    &mut target,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
            let target = target.ok_or_else(|| BackendError::Backend("no render target".into()))?;
            let mut sysmem = None;
            self.device
                .CreateOffscreenPlainSurface(
                    width,
                    height,
                    D3DFMT_A8R8G8B8,
                    D3DPOOL_SYSTEMMEM,
                    &mut sysmem,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
            let sysmem =
                sysmem.ok_or_else(|| BackendError::Backend("no readback surface".into()))?;

            // The device's own target is put back on every exit, so no default-pool surface
            // of ours stays bound (which would make a later `Reset` fail).
            let previous = self.device.GetRenderTarget(0).map_err(map_err)?;
            let drawn = (|| {
                self.device.SetRenderTarget(0, &target)?;
                self.device.BeginScene()?;
                let cleared =
                    self.device
                        .Clear(0, std::ptr::null(), D3DCLEAR_TARGET as u32, argb, 1.0, 0);
                let ended = self.device.EndScene();
                cleared?;
                ended?;
                self.device.GetRenderTargetData(&target, &sysmem)
            })();
            let restored = self.device.SetRenderTarget(0, &previous);
            drawn.map_err(map_err)?;
            restored.map_err(map_err)?;

            let mut locked = D3DLOCKED_RECT::default();
            sysmem
                .LockRect(&mut locked, std::ptr::null(), D3DLOCK_READONLY as u32)
                .map_err(map_err)?;
            let mut out = Vec::with_capacity(width as usize * height as usize * 4);
            for y in 0..height as usize {
                let row = std::slice::from_raw_parts(
                    (locked.pBits as *const u8).add(y * locked.Pitch as usize),
                    width as usize * 4,
                );
                // Memory order is B, G, R, A.
                for px in row.chunks_exact(4) {
                    out.extend_from_slice(&[px[2], px[1], px[0], px[3]]);
                }
            }
            let _ = sysmem.UnlockRect();
            Ok(out)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Needs a Direct3D 9 driver (CI runners without a GPU may have none): skips if absent.
    #[test]
    fn clear_and_read_back() {
        // Only a missing driver skips; any other failure to open is a real failure.
        let dev = match D3d9Device::open() {
            Ok(dev) => dev,
            Err(BackendError::Unsupported(why)) => {
                eprintln!("no Direct3D 9 device here ({why}); skipping");
                return;
            }
            Err(e) => panic!("opening the device failed: {e}"),
        };
        // Printed so a test log shows which adapter (the default one) was exercised.
        eprintln!("D3D9 adapter: {:?}", dev.caps());
        dev.status().expect("fresh device is usable");
        let px = dev
            .clear_and_read(4, 2, [1.0, 0.0, 0.0, 1.0])
            .expect("clear and read");
        assert_eq!(px.len(), 4 * 2 * 4);
        assert!(px.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
    }
}
