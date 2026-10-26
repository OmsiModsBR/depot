//! The light coronas on the device: the camera-facing sprites a light shows round its lamp
//! (headlights, brake lights, indicators, beacons, street lamps), the cone a light draws in
//! fog and the halo round a light in fog. [`D3d9Device::draw_scene`] draws a frame's
//! [`SceneCoronas`](crate::scene::SceneCoronas) with the corona shader (`shaders/corona.hlsl`)
//! over the scene: after the world's batches, and again after the vehicle the camera is in.
//!
//! It is a pass of its own, with its own small shaders, so the scene's pixel shader keeps its
//! instruction slots. A device that cannot make it (the pass is tried once) draws the scene
//! without coronas and says so in the log.

use windows::Win32::Graphics::Direct3D9::*;

use super::scene::{make_texture, TextureCopy};
use super::{map_err, shader, some, D3d9Device};
use crate::scene::{SceneFrame, SceneTexture, TexFormat, CORONA_FLOATS};
use crate::{BackendError, Result};

/// A sprite's record in the instance buffer, in bytes.
const RECORD: u32 = (CORONA_FLOATS * 4) as u32;
/// The vertex shader's registers of the camera's axes and the fog (the scene's own, below
/// 32, are left alone).
const AXES_REGISTER: u32 = 100;
/// The standard glow's side, as the default renderer makes it.
const GLOW_SIDE: u32 = 64;

/// Shaders, the unit quad and the standard glow, made at the first scene frame.
pub(super) struct CoronaPipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    ps: IDirect3DPixelShader9,
    quad: IDirect3DVertexBuffer9,
    quad_indices: IDirect3DIndexBuffer9,
    /// Picture 0, and any picture that was not put up: a soft round glow.
    glow: IDirect3DTexture9,
}

#[derive(Default)]
pub(super) struct CoronaStore {
    pipeline: Option<CoronaPipeline>,
    /// The pass cannot be made on this device (it is not tried again every frame).
    unsupported: bool,
    /// The lights' own pictures, under the renderer's picture numbers.
    pictures: Vec<Option<IDirect3DTexture9>>,
    /// The per-frame sprite records, and the dynamic buffer they go up in (with the sprites
    /// it holds).
    buffer: Option<(IDirect3DVertexBuffer9, usize)>,
}

/// The standard glow: grey falling off with the distance from the middle, as
/// `omsi-render`'s default one (RGBA, opaque).
fn glow_pixels() -> Vec<u8> {
    let cs = GLOW_SIDE as usize;
    let mut rgba = vec![0u8; cs * cs * 4];
    for y in 0..cs {
        for x in 0..cs {
            let dx = (x as f32 + 0.5) / cs as f32 * 2.0 - 1.0;
            let dy = (y as f32 + 0.5) / cs as f32 * 2.0 - 1.0;
            let r = (dx * dx + dy * dy).sqrt();
            let v = ((1.0 - r).max(0.0)).powf(1.6) * 255.0;
            let o = (y * cs + x) * 4;
            rgba[o..o + 4].copy_from_slice(&[v as u8, v as u8, v as u8, 255]);
        }
    }
    rgba
}

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<CoronaPipeline> {
    let vs_code = shader::compile(
        shader::CORONA,
        "corona.hlsl",
        "vs_main",
        shader::Stage::Vertex,
    )?;
    let ps_code = shader::compile(
        shader::CORONA,
        "corona.hlsl",
        "ps_main",
        shader::Stage::Pixel,
    )?;
    let element = |stream: u16, offset: u16, ty: D3DDECLTYPE, index: u8| D3DVERTEXELEMENT9 {
        Stream: stream,
        Offset: offset,
        Type: ty.0 as u8,
        Method: D3DDECLMETHOD_DEFAULT.0 as u8,
        Usage: D3DDECLUSAGE_TEXCOORD.0 as u8,
        UsageIndex: index,
    };
    // stream 0 the quad's corners, stream 1 one record per sprite: five vectors
    let mut elements = vec![element(0, 0, D3DDECLTYPE_FLOAT2, 0)];
    for k in 0..5u8 {
        elements.push(element(1, k as u16 * 16, D3DDECLTYPE_FLOAT4, k + 1));
    }
    // D3DDECL_END()
    elements.push(D3DVERTEXELEMENT9 {
        Stream: 0xFF,
        Offset: 0,
        Type: D3DDECLTYPE_UNUSED.0 as u8,
        Method: 0,
        Usage: 0,
        UsageIndex: 0,
    });
    let glow = make_texture(
        device,
        &SceneTexture {
            width: GLOW_SIDE,
            height: GLOW_SIDE,
            format: TexFormat::Rgba8,
            levels: &[glow_pixels()],
            make_mips: false,
        },
    )?;
    // SAFETY: out-parameters are live locals; the locked ranges are exactly the sizes the
    // buffers were created with; the bytecode and element arrays outlive the calls.
    unsafe {
        let corners: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, 1.0, 1.0, -1.0, 1.0];
        let mut quad = None;
        device
            .CreateVertexBuffer(
                std::mem::size_of_val(&corners) as u32,
                D3DUSAGE_WRITEONLY as u32,
                0,
                D3DPOOL_DEFAULT,
                &mut quad,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let quad = some(quad, "corona quad")?;
        super::write_buffer(&quad, &corners)?;
        let indices: [u16; 6] = [0, 1, 2, 0, 2, 3];
        let mut ib = None;
        device
            .CreateIndexBuffer(
                std::mem::size_of_val(&indices) as u32,
                D3DUSAGE_WRITEONLY as u32,
                D3DFMT_INDEX16,
                D3DPOOL_DEFAULT,
                &mut ib,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let quad_indices = some(ib, "corona indices")?;
        let mut p = std::ptr::null_mut();
        quad_indices
            .Lock(0, std::mem::size_of_val(&indices) as u32, &mut p, 0)
            .map_err(map_err)?;
        std::ptr::copy_nonoverlapping(
            indices.as_ptr() as *const u8,
            p as *mut u8,
            std::mem::size_of_val(&indices),
        );
        quad_indices.Unlock().map_err(map_err)?;
        Ok(CoronaPipeline {
            glow,
            quad,
            quad_indices,
            decl: device
                .CreateVertexDeclaration(elements.as_ptr())
                .map_err(map_err)?,
            vs: device
                .CreateVertexShader(vs_code.as_ptr())
                .map_err(map_err)?,
            ps: device
                .CreatePixelShader(ps_code.as_ptr())
                .map_err(map_err)?,
        })
    }
}

impl D3d9Device {
    /// Puts picture `id` of the light coronas up (a light's own bitmap, whose brightness is
    /// the glow's shape), replacing what had that number. They belong to the renderer, not to
    /// a scene: a new scene keeps them.
    pub fn set_corona_texture(&mut self, id: u16, texture: &SceneTexture) -> Result<()> {
        texture.validate(self.caps.max_texture_size, self.caps.non_pow2_textures)?;
        let made = match make_texture(&self.device, texture) {
            Err(e) if !matches!(e, BackendError::DeviceLost) => return Err(e),
            made => made,
        };
        let id = usize::from(id);
        let copies = &mut self.scene.copies.coronas;
        if copies.len() <= id {
            copies.resize_with(id + 1, || None);
        }
        // (the copy also when the device was lost meanwhile: `recover` puts it up on the next)
        copies[id] = Some(TextureCopy::of(texture));
        let pictures = &mut self.scene.coronas.pictures;
        if pictures.len() <= id {
            pictures.resize_with(id + 1, || None);
        }
        pictures[id] = Some(made?);
        Ok(())
    }

    /// Builds the corona pass if it is not there. A pass that cannot be made is remembered
    /// and the scene is drawn without coronas.
    pub(super) fn ensure_coronas(&mut self) {
        if self.scene.coronas.pipeline.is_some() || self.scene.coronas.unsupported {
            return;
        }
        match build_pipeline(&self.device) {
            Ok(p) => self.scene.coronas.pipeline = Some(p),
            Err(BackendError::DeviceLost | BackendError::DeviceNotReset) => {}
            Err(e) => {
                log::warn!("d3d9: the lights' coronas cannot be drawn on this device: {e}");
                self.scene.coronas.unsupported = true;
            }
        }
    }

    /// Puts the frame's sprite records up, once for the frame's two corona draws.
    pub(super) fn upload_coronas(&mut self, frame: &SceneFrame) -> Result<()> {
        let sprites = frame.coronas.sprites;
        if sprites.is_empty() || self.scene.coronas.pipeline.is_none() {
            return Ok(());
        }
        let bytes = u32::try_from(sprites.len())
            .ok()
            .and_then(|n| n.checked_mul(RECORD))
            .ok_or_else(|| BackendError::InvalidArgument(format!("{} coronas", sprites.len())))?;
        if self
            .scene
            .coronas
            .buffer
            .as_ref()
            .is_none_or(|b| b.1 < sprites.len())
        {
            let capacity = sprites.len().next_power_of_two().max(256);
            let capacity_bytes = u32::try_from(capacity)
                .ok()
                .and_then(|n| n.checked_mul(RECORD))
                .ok_or_else(|| BackendError::InvalidArgument(format!("{capacity} coronas")))?;
            let mut vb = None;
            // SAFETY: the out-parameter is a live local.
            unsafe {
                self.device
                    .CreateVertexBuffer(
                        capacity_bytes,
                        (D3DUSAGE_DYNAMIC | D3DUSAGE_WRITEONLY) as u32,
                        0,
                        D3DPOOL_DEFAULT,
                        &mut vb,
                        std::ptr::null_mut(),
                    )
                    .map_err(map_err)?;
            }
            self.scene.coronas.buffer = Some((some(vb, "corona buffer")?, capacity));
        }
        if let Some((vb, _)) = &self.scene.coronas.buffer {
            // SAFETY: the buffer holds at least `bytes`, locked with DISCARD and written whole.
            unsafe {
                let mut p = std::ptr::null_mut();
                vb.Lock(0, bytes, &mut p, D3DLOCK_DISCARD as u32)
                    .map_err(map_err)?;
                std::ptr::copy_nonoverlapping(
                    sprites.as_ptr() as *const u8,
                    p as *mut u8,
                    bytes as usize,
                );
                vb.Unlock().map_err(map_err)?;
            }
        }
        Ok(())
    }

    /// Draws the frame's coronas of the vehicle the camera is in (`late`) or of the rest of
    /// the world, over what is drawn: added to it as `src + dst * (1 - src)`, tested against
    /// the depth but not writing it. Leaves the corona pass's states set; the caller binds the
    /// scene's again. The scene's own constants (the view, the camera, the pixel offset) are
    /// those the sprites use.
    pub(super) fn draw_coronas(&self, frame: &SceneFrame, late: bool) -> Result<()> {
        let store = &self.scene.coronas;
        let (Some(p), Some((vb, _))) = (&store.pipeline, &store.buffer) else {
            return Ok(());
        };
        if !frame.coronas.runs.iter().any(|r| r.3 == late && r.2 > 0) {
            return Ok(());
        }
        let d = &self.device;
        let linear = u32::from(self.scene_caps.linear_ui);
        let light = &frame.light;
        let axes = [light.cam_right, light.cam_up, light.fog];
        // SAFETY: plain calls on a live device; the constant arrays outlive the calls; each
        // run lies in the sprite list (checked by `SceneFrame::validate`), which the buffer
        // holds whole.
        unsafe {
            for (state, value) in [
                (D3DRS_ZENABLE, D3DZB_TRUE.0 as u32),
                (D3DRS_ZFUNC, D3DCMP_GREATEREQUAL.0 as u32),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_DEPTHBIAS, 0),
                (D3DRS_SLOPESCALEDEPTHBIAS, 0),
                (D3DRS_ALPHABLENDENABLE, 1),
                // the lamps' sprites: SRCBLEND ONE, DESTBLEND INVSRCCOLOR, which keeps a
                // coloured sprite's hue over a lit background instead of washing it to white
                (D3DRS_SRCBLEND, D3DBLEND_ONE.0 as u32),
                (D3DRS_DESTBLEND, D3DBLEND_INVSRCCOLOR.0 as u32),
                // (the picture's alpha is left as the scene made it)
                (D3DRS_COLORWRITEENABLE, 0x7),
                // blended in linear light where the device can, as the default renderer does
                (D3DRS_SRGBWRITEENABLE, linear),
            ] {
                d.SetRenderState(state, value).map_err(map_err)?;
            }
            // (what stage 0 held is put back after the pass, whatever the scene set it to)
            let sampler = [
                (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MIPFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_SRGBTEXTURE, linear),
            ];
            let mut before = [0u32; 6];
            for (kept, &(state, _)) in before.iter_mut().zip(&sampler) {
                d.GetSamplerState(0, state, kept).map_err(map_err)?;
            }
            for (state, value) in sampler {
                d.SetSamplerState(0, state, value).map_err(map_err)?;
            }
            d.SetVertexDeclaration(&p.decl).map_err(map_err)?;
            d.SetVertexShader(&p.vs).map_err(map_err)?;
            d.SetPixelShader(&p.ps).map_err(map_err)?;
            d.SetVertexShaderConstantF(AXES_REGISTER, axes.as_ptr().cast(), axes.len() as u32)
                .map_err(map_err)?;
            d.SetStreamSource(0, &p.quad, 0, 8).map_err(map_err)?;
            d.SetIndices(&p.quad_indices).map_err(map_err)?;
            for &(picture, first, count, _) in
                frame.coronas.runs.iter().filter(|r| r.3 == late && r.2 > 0)
            {
                let texture = store
                    .pictures
                    .get(usize::from(picture))
                    .and_then(Option::as_ref)
                    .unwrap_or(&p.glow);
                d.SetTexture(0, texture).map_err(map_err)?;
                d.SetStreamSourceFreq(0, D3DSTREAMSOURCE_INDEXEDDATA | count)
                    .map_err(map_err)?;
                d.SetStreamSource(1, vb, first * RECORD, RECORD)
                    .map_err(map_err)?;
                d.SetStreamSourceFreq(1, D3DSTREAMSOURCE_INSTANCEDATA | 1)
                    .map_err(map_err)?;
                d.DrawIndexedPrimitive(D3DPT_TRIANGLELIST, 0, 0, 4, 0, 2)
                    .map_err(map_err)?;
            }
            for (&(state, _), &value) in sampler.iter().zip(&before) {
                d.SetSamplerState(0, state, value).map_err(map_err)?;
            }
            d.SetRenderState(D3DRS_SRGBWRITEENABLE, 0)
                .map_err(map_err)?;
        }
        Ok(())
    }
}
