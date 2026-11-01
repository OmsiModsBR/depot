//! The render scale on the device: the scene is drawn into a render target smaller than the
//! picture, and brought up to the picture's size by a full-screen pass (`shaders/upscale.hlsl`)
//! before the interface and the HUD's pictures are drawn over it at the picture's own size.
//!
//! * [`Upscale::Bilinear`]: one pass, the card's bilinear filter.
//! * [`Upscale::Fsr1`]: two passes (edge-adaptive upsampling into a picture-sized target of its
//!   own, then sharpening into the picture), adapted from AMD's FidelityFX Super Resolution 1.
//!   A device whose pixel shaders cannot take it draws with the bilinear filter, and says so.
//!
//! With no scale (the default, or 100 %) none of this runs: no target is made and the scene is
//! drawn straight into the frame, exactly as without the option. The scaled target has no
//! multisampling (the option is there to save pixels).

use windows::Win32::Graphics::Direct3D9::*;

use super::scene::A2c;
use super::{map_err, rgba_to_argb, shader, some, D3d9Device};
use crate::render_scale::{easu_mapping, effective_scale, rcas_factor, scaled_size, Upscale};
use crate::scene::{SceneFrame, SceneMaterial};
use crate::{BackendError, Result};

/// The pixel shader registers of the passes (the scene's are below 32; the sky's, the sun
/// glow's and the others' start at 200 and stay clear of these).
const PS_SIZES: u32 = 220;
const PS_MAPPING: u32 = 221;
const PS_RCAS: u32 = 222;
/// The vertex shader register of the half pixel (the scene's start at 0).
const VS_HALF_PIXEL: u32 = 250;

/// The shaders and the full-screen strip of the passes.
pub(super) struct UpscalePipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    bilinear: IDirect3DPixelShader9,
    /// The upsampling and sharpening passes; `None` until asked for, or when this device
    /// cannot take them.
    fsr: Option<(IDirect3DPixelShader9, IDirect3DPixelShader9)>,
    quad: IDirect3DVertexBuffer9,
}

/// The targets of one window size at one scale.
pub(super) struct Targets {
    pub(super) native: (u32, u32),
    small: (u32, u32),
    /// What the scene is drawn into, and the depth it is drawn with.
    scene: IDirect3DTexture9,
    depth: IDirect3DSurface9,
    /// The picture-sized target the upsampling pass writes (the sharpening reads it); only
    /// for [`Upscale::Fsr1`].
    mid: Option<IDirect3DTexture9>,
}

#[derive(Default)]
pub(super) struct ScaleStore {
    /// The fraction to draw at, `None` for the picture's own size.
    scale: Option<f32>,
    upscale: Upscale,
    pipeline: Option<UpscalePipeline>,
    /// The passes cannot be made on this device (they are not tried again every frame).
    unsupported: bool,
    /// The sharper passes cannot be made on this device; the bilinear one is used.
    fsr_unsupported: bool,
    pub(super) targets: Option<Targets>,
}

fn corner_strip(device: &IDirect3DDevice9Ex) -> Result<IDirect3DVertexBuffer9> {
    // a strip of four corners
    let corners: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
    // SAFETY: the out-parameter is a live local.
    unsafe {
        let mut vb = None;
        device
            .CreateVertexBuffer(
                std::mem::size_of_val(&corners) as u32,
                D3DUSAGE_WRITEONLY as u32,
                0,
                D3DPOOL_DEFAULT,
                &mut vb,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let quad = some(vb, "upscale corners")?;
        super::write_buffer(&quad, &corners)?;
        Ok(quad)
    }
}

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<UpscalePipeline> {
    let vs_code = shader::compile(
        shader::UPSCALE,
        "upscale.hlsl",
        "vs_main",
        shader::Stage::Vertex,
    )?;
    let ps_code = shader::compile(
        shader::UPSCALE,
        "upscale.hlsl",
        "ps_bilinear",
        shader::Stage::Pixel,
    )?;
    let elements = [
        D3DVERTEXELEMENT9 {
            Stream: 0,
            Offset: 0,
            Type: D3DDECLTYPE_FLOAT2.0 as u8,
            Method: D3DDECLMETHOD_DEFAULT.0 as u8,
            Usage: D3DDECLUSAGE_POSITION.0 as u8,
            UsageIndex: 0,
        },
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
        Ok(UpscalePipeline {
            quad: corner_strip(device)?,
            decl: device
                .CreateVertexDeclaration(elements.as_ptr())
                .map_err(map_err)?,
            vs: device
                .CreateVertexShader(vs_code.as_ptr())
                .map_err(map_err)?,
            bilinear: device
                .CreatePixelShader(ps_code.as_ptr())
                .map_err(map_err)?,
            fsr: None,
        })
    }
}

/// The upsampling and sharpening pixel shaders, for a device whose pixel shaders may take
/// `ps_slots` instruction slots.
fn build_fsr(
    device: &IDirect3DDevice9Ex,
    ps_slots: u32,
) -> Result<(IDirect3DPixelShader9, IDirect3DPixelShader9)> {
    let mut made = Vec::with_capacity(2);
    for entry in ["ps_easu", "ps_rcas"] {
        let code = shader::compile(shader::UPSCALE, "upscale.hlsl", entry, shader::Stage::Pixel)?;
        let slots = crate::caps::instruction_slots(&code);
        log::info!("d3d9 upscale: {entry} takes about {slots} instruction slots of {ps_slots}");
        if slots > ps_slots {
            return Err(BackendError::Unsupported(format!(
                "{entry} takes about {slots} instruction slots and this graphics card allows \
                 {ps_slots}"
            )));
        }
        // SAFETY: the bytecode outlives the call.
        made.push(unsafe { device.CreatePixelShader(code.as_ptr()) }.map_err(map_err)?);
    }
    let rcas = made.pop().expect("two shaders");
    let easu = made.pop().expect("two shaders");
    Ok((easu, rcas))
}

fn render_target_texture(
    device: &IDirect3DDevice9Ex,
    (w, h): (u32, u32),
) -> Result<IDirect3DTexture9> {
    let mut texture = None;
    // SAFETY: the out-parameter is a live local.
    unsafe {
        device
            .CreateTexture(
                w,
                h,
                1,
                D3DUSAGE_RENDERTARGET as u32,
                D3DFMT_A8R8G8B8,
                D3DPOOL_DEFAULT,
                &mut texture,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
    }
    some(texture, "render scale target")
}

impl ScaleStore {
    /// The up-scaling asked for.
    pub(super) fn upscale(&self) -> Upscale {
        self.upscale
    }
}

impl D3d9Device {
    /// Draws the scene at `scale` of the picture's size (0.5 to 1) and brings it up with
    /// `upscale`: the render scale option. `0`, a number that is none, or 1 draw at the
    /// picture's size, as without the option. Takes effect from the next scene.
    pub fn set_render_scale(&mut self, scale: f32, upscale: Upscale) {
        let scale = effective_scale(scale);
        if scale != self.scale.scale || upscale != self.scale.upscale {
            // (made again at the next scene, at the new size)
            self.scale.targets = None;
        }
        self.scale.scale = scale;
        self.scale.upscale = upscale;
    }

    /// The scale in force (`None`: the picture's own size).
    pub fn render_scale(&self) -> Option<f32> {
        self.scale.scale
    }

    /// Lets go of the targets, which are made for a picture size and a depth format: they are
    /// made again when next needed.
    pub(super) fn drop_scale_targets(&mut self) {
        self.scale.targets = None;
    }

    /// The size the scene is drawn at for a frame of `native` size: `None` when it is drawn
    /// at that size (no scale, a scale that rounds to it, or passes this device cannot make).
    pub(super) fn scene_size_for(&self, native: (u32, u32)) -> Option<(u32, u32)> {
        let scale = self.scale.scale.filter(|_| !self.scale.unsupported)?;
        let small = scaled_size(native.0, native.1, scale);
        (small != native && small.0.max(small.1) <= self.caps.max_texture_size).then_some(small)
    }

    /// Builds the passes if they are not there. A pass that cannot be made is remembered, and
    /// the scene is drawn at the picture's size, with a line in the log.
    fn ensure_upscale(&mut self) -> Result<()> {
        if self.scale.pipeline.is_none() {
            match build_pipeline(&self.device) {
                Ok(p) => self.scale.pipeline = Some(p),
                Err(e @ (BackendError::DeviceLost | BackendError::DeviceNotReset)) => {
                    return Err(e)
                }
                Err(e) => {
                    log::warn!("d3d9: the render scale cannot be used on this device: {e}");
                    self.scale.unsupported = true;
                    return Err(e);
                }
            }
        }
        if self.scale.upscale == Upscale::Fsr1 && !self.scale.fsr_unsupported {
            let ps_slots = self.scene_caps.ps_slots;
            let p = self.scale.pipeline.as_mut().expect("built above");
            if p.fsr.is_none() {
                match build_fsr(&self.device, ps_slots) {
                    Ok(f) => p.fsr = Some(f),
                    Err(e @ (BackendError::DeviceLost | BackendError::DeviceNotReset)) => {
                        return Err(e)
                    }
                    Err(e) => {
                        log::warn!(
                            "d3d9: the sharper up-scaling cannot be used on this device, the \
                             bilinear filter is used: {e}"
                        );
                        self.scale.fsr_unsupported = true;
                    }
                }
            }
        }
        Ok(())
    }

    /// Whether the sharper passes are in use (asked for, and made).
    pub(super) fn fsr_in_use(&self) -> bool {
        self.scale.upscale == Upscale::Fsr1
            && !self.scale.fsr_unsupported
            && self
                .scale
                .pipeline
                .as_ref()
                .is_some_and(|p| p.fsr.is_some())
    }

    /// The targets for a frame of `native` size drawn at `small`, made if they are not there.
    fn ensure_scale_targets(&mut self, native: (u32, u32), small: (u32, u32)) -> Result<()> {
        let fsr = self.fsr_in_use();
        if let Some(t) = &self.scale.targets {
            if t.native == native && t.small == small && t.mid.is_some() == fsr {
                return Ok(());
            }
        }
        self.scale.targets = None;
        let scene = render_target_texture(&self.device, small)?;
        let mut depth = None;
        // SAFETY: the out-parameter is a live local.
        unsafe {
            self.device
                .CreateDepthStencilSurface(
                    small.0,
                    small.1,
                    self.depth_format,
                    D3DMULTISAMPLE_NONE,
                    0,
                    true,
                    &mut depth,
                    std::ptr::null_mut(),
                )
                .map_err(map_err)?;
        }
        let depth = some(depth, "render scale depth")?;
        let mid = if fsr {
            Some(render_target_texture(&self.device, native)?)
        } else {
            None
        };
        log::info!(
            "d3d9: the scene is drawn at {}x{} for {}x{} ({}) ",
            small.0,
            small.1,
            native.0,
            native.1,
            if fsr {
                "sharper up-scaling"
            } else {
                "bilinear"
            }
        );
        self.scale.targets = Some(Targets {
            native,
            small,
            scene,
            depth,
            mid,
        });
        Ok(())
    }

    /// Draws `frame` at `small` into the render scale's target and brings it up into what is
    /// bound (the frame's picture): the scaled counterpart of the inner scene draw of
    /// [`D3d9Device::draw_scene`], which then does what it does after a scene. What was bound
    /// is bound again whatever happened.
    pub(super) fn draw_scene_scaled<'m>(
        &mut self,
        frame: &SceneFrame,
        material: &impl Fn(usize) -> Option<SceneMaterial<'m>>,
        native: (u32, u32),
        small: (u32, u32),
        clear_rgba: [f32; 4],
    ) -> Result<()> {
        if let Err(e) = self.ensure_upscale() {
            if self.scale.unsupported {
                // (the scene is drawn at the picture's size from now on)
                return self.draw_scene_inner(frame, material, native.0, native.1, self.coverage());
            }
            return Err(e);
        }
        self.ensure_scale_targets(native, small)?;
        let targets = self.scale.targets.as_ref().expect("made above");
        let (scene_texture, depth) = (targets.scene.clone(), targets.depth.clone());
        // SAFETY: plain calls on a live device; the surfaces belong to it. What was bound is
        // bound again whatever happened.
        let (drawn, rebound) = unsafe {
            let surface = scene_texture.GetSurfaceLevel(0).map_err(map_err)?;
            let color = self.device.GetRenderTarget(0).map_err(map_err)?;
            let prev_depth = self.device.GetDepthStencilSurface().ok();
            let drawn = self
                .device
                .SetRenderTarget(0, &surface)
                .and_then(|_| self.device.SetDepthStencilSurface(&depth))
                .and_then(|_| {
                    self.device.Clear(
                        0,
                        std::ptr::null(),
                        (D3DCLEAR_TARGET | D3DCLEAR_ZBUFFER) as u32,
                        rgba_to_argb(clear_rgba),
                        1.0,
                        0,
                    )
                })
                .map_err(map_err)
                .and_then(|_| self.draw_scene_inner(frame, material, small.0, small.1, A2c::None))
                // (the backend's own states, with the scaled depth still bound)
                .and_then(|_| self.restore_scene_states());
            // (setting the colour target again sets the whole viewport again too)
            let rebound = self
                .device
                .SetRenderTarget(0, &color)
                .and_then(|_| self.device.SetDepthStencilSurface(prev_depth.as_ref()))
                .map_err(map_err);
            (drawn, rebound)
        };
        drawn?;
        rebound?;
        self.upscale_passes(native, small)
    }

    /// Brings the scaled picture up into the bound target.
    fn upscale_passes(&mut self, native: (u32, u32), small: (u32, u32)) -> Result<()> {
        let fsr = self.fsr_in_use();
        let (Some(p), Some(t)) = (&self.scale.pipeline, &self.scale.targets) else {
            return Err(BackendError::Backend("render scale passes missing".into()));
        };
        let d = &self.device;
        let half_pixel = [-1.0 / native.0 as f32, 1.0 / native.1 as f32, 0.0, 0.0];
        let (nw, nh) = (native.0 as f32, native.1 as f32);
        let (sw, sh) = (small.0 as f32, small.1 as f32);
        // SAFETY: plain calls on a live device; the constant arrays outlive the calls; the
        // resources belong to this device.
        unsafe {
            for (state, value) in [
                (D3DRS_ZENABLE, D3DZB_FALSE.0 as u32),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_ALPHABLENDENABLE, 0),
                (D3DRS_SEPARATEALPHABLENDENABLE, 0),
                (D3DRS_STENCILENABLE, 0),
                (D3DRS_SCISSORTESTENABLE, 0),
                (D3DRS_COLORWRITEENABLE, 0xF),
                (D3DRS_SRGBWRITEENABLE, 0),
                (D3DRS_DEPTHBIAS, 0),
                (D3DRS_SLOPESCALEDEPTHBIAS, 0),
            ] {
                d.SetRenderState(state, value).map_err(map_err)?;
            }
            d.SetStreamSourceFreq(0, 1).map_err(map_err)?;
            d.SetStreamSourceFreq(1, 1).map_err(map_err)?;
            d.SetVertexDeclaration(&p.decl).map_err(map_err)?;
            d.SetVertexShader(&p.vs).map_err(map_err)?;
            d.SetVertexShaderConstantF(VS_HALF_PIXEL, half_pixel.as_ptr(), 1)
                .map_err(map_err)?;
            d.SetStreamSource(0, &p.quad, 0, 8).map_err(map_err)?;
            let sampler = |filter: D3DTEXTUREFILTERTYPE| -> Result<()> {
                for (kind, value) in [
                    (D3DSAMP_MINFILTER, filter.0 as u32),
                    (D3DSAMP_MAGFILTER, filter.0 as u32),
                    (D3DSAMP_MIPFILTER, D3DTEXF_NONE.0 as u32),
                    (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                    (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
                    (D3DSAMP_SRGBTEXTURE, 0),
                ] {
                    d.SetSamplerState(0, kind, value).map_err(map_err)?;
                }
                Ok(())
            };
            let draw = |texture: &IDirect3DTexture9, ps: &IDirect3DPixelShader9| -> Result<()> {
                d.SetPixelShader(ps).map_err(map_err)?;
                d.SetTexture(0, texture).map_err(map_err)?;
                d.DrawPrimitive(D3DPT_TRIANGLESTRIP, 0, 2)
                    .map_err(map_err)?;
                d.SetTexture(0, None::<&IDirect3DBaseTexture9>)
                    .map_err(map_err)
            };
            match (&p.fsr, &t.mid, fsr) {
                (Some((easu, rcas)), Some(mid), true) => {
                    // the upsampling, into the picture-sized target
                    let surface = mid.GetSurfaceLevel(0).map_err(map_err)?;
                    let color = d.GetRenderTarget(0).map_err(map_err)?;
                    let prev_depth = d.GetDepthStencilSurface().ok();
                    let sizes = [nw, nh, 1.0 / sw, 1.0 / sh];
                    let mapping = easu_mapping(small, native);
                    let first = d
                        .SetRenderTarget(0, &surface)
                        .and_then(|_| d.SetDepthStencilSurface(None::<&IDirect3DSurface9>))
                        .map_err(map_err)
                        .and_then(|_| {
                            sampler(D3DTEXF_POINT)?;
                            d.SetPixelShaderConstantF(PS_SIZES, sizes.as_ptr(), 1)
                                .map_err(map_err)?;
                            d.SetPixelShaderConstantF(PS_MAPPING, mapping.as_ptr(), 1)
                                .map_err(map_err)?;
                            draw(&t.scene, easu)
                        });
                    let rebound = d
                        .SetRenderTarget(0, &color)
                        .and_then(|_| d.SetDepthStencilSurface(prev_depth.as_ref()))
                        .map_err(map_err);
                    first?;
                    rebound?;
                    // the sharpening, into the picture
                    let sizes = [nw, nh, 1.0 / nw, 1.0 / nh];
                    let strength = [rcas_factor(), 0.0, 0.0, 0.0];
                    sampler(D3DTEXF_POINT)?;
                    d.SetPixelShaderConstantF(PS_SIZES, sizes.as_ptr(), 1)
                        .map_err(map_err)?;
                    d.SetPixelShaderConstantF(PS_RCAS, strength.as_ptr(), 1)
                        .map_err(map_err)?;
                    draw(mid, rcas)
                }
                _ => {
                    sampler(D3DTEXF_LINEAR)?;
                    draw(&t.scene, &p.bilinear)
                }
            }
        }
    }
}
