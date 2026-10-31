//! Smoke and particles on the device: the camera-facing, alpha-blended squares a `[smoke]`
//! system or a `[particle_emitter]` makes (exhaust, chimneys, fireworks, wheel smoke).
//! [`D3d9Device::draw_scene`] draws a frame's [`SceneSmoke`](crate::scene::SceneSmoke) with the
//! smoke shader (`shaders/smoke.hlsl`) over the world's batches: tested against the depth,
//! not writing it, the farthest particle first as the renderer sorted them.
//!
//! It is a pass of its own, with its own small shaders, so the scene's pixel shader keeps its
//! instruction slots. A device that cannot make it (the pass is tried once) draws the scene
//! without smoke and says so in the log.

use windows::Win32::Graphics::Direct3D9::*;

use super::coronas::{sprite_elements, unit_quad, write_sprites};
use super::scene::{make_texture, TextureCopy};
use super::{map_err, shader, D3d9Device};
use crate::scene::{SceneFrame, SceneTexture};
use crate::{BackendError, Result};

/// A particle's record in the instance buffer, in bytes.
const RECORD: u32 = (crate::scene::CORONA_FLOATS * 4) as u32;
/// The vertex shader's register of the camera's right axis (the scene's own, below 32, are
/// left alone).
const AXES_REGISTER: u32 = 100;
/// The pixel shader's register of the light on the particles (the scene's lie below 32).
const LIGHT_REGISTER: u32 = 200;
/// The most the light on a particle adds up to.
const LIGHT_MAX: f32 = 1.2;
/// The share of the sun's light a particle takes besides the ambient.
const SUN_SHARE: f32 = 0.6;

/// Shaders and the unit quad, made at the first scene frame.
pub(super) struct SmokePipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    ps: IDirect3DPixelShader9,
    quad: IDirect3DVertexBuffer9,
    quad_indices: IDirect3DIndexBuffer9,
}

#[derive(Default)]
pub(super) struct SmokeStore {
    pipeline: Option<SmokePipeline>,
    /// The pass cannot be made on this device (it is not tried again every frame).
    unsupported: bool,
    /// The particles' picture (the game's smoke texture).
    picture: Option<IDirect3DTexture9>,
    /// The per-frame particle records, and the dynamic buffer they go up in (with the
    /// particles it holds).
    buffer: Option<(IDirect3DVertexBuffer9, usize)>,
}

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<SmokePipeline> {
    let vs_code = shader::compile(
        shader::SMOKE,
        "smoke.hlsl",
        "vs_main",
        shader::Stage::Vertex,
    )?;
    let ps_code = shader::compile(shader::SMOKE, "smoke.hlsl", "ps_main", shader::Stage::Pixel)?;
    let elements = sprite_elements();
    let (quad, quad_indices) = unit_quad(device)?;
    // SAFETY: the element array and the bytecode outlive the calls.
    unsafe {
        Ok(SmokePipeline {
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

/// The light on a particle: the ambient plus a share of the sun's, at most [`LIGHT_MAX`].
fn particle_light(ambient: [f32; 4], sun: [f32; 4]) -> [f32; 4] {
    let l = |k: usize| (ambient[k] + sun[k] * SUN_SHARE).min(LIGHT_MAX);
    [l(0), l(1), l(2), 0.0]
}

impl D3d9Device {
    /// Puts the particles' picture up (the game's smoke texture, whose alpha is the particle's
    /// mask), replacing the one there. It belongs to the renderer, not to a scene: a new scene
    /// keeps it.
    pub fn set_smoke_texture(&mut self, texture: &SceneTexture) -> Result<()> {
        texture.validate(self.caps.max_texture_size, self.caps.non_pow2_textures)?;
        let made = match make_texture(&self.device, texture) {
            Err(e) if !matches!(e, BackendError::DeviceLost) => return Err(e),
            made => made,
        };
        // (the copy also when the device was lost meanwhile: `recover` puts it up on the next)
        self.scene.copies.smoke = Some(TextureCopy::of(texture));
        self.scene.smoke.picture = Some(made?);
        Ok(())
    }

    /// Builds the smoke pass if it is not there. A pass that cannot be made is remembered and
    /// the scene is drawn without smoke.
    pub(super) fn ensure_smoke(&mut self) {
        if self.scene.smoke.pipeline.is_some() || self.scene.smoke.unsupported {
            return;
        }
        match build_pipeline(&self.device) {
            Ok(p) => self.scene.smoke.pipeline = Some(p),
            Err(BackendError::DeviceLost | BackendError::DeviceNotReset) => {}
            Err(e) => {
                log::warn!("d3d9: the smoke cannot be drawn on this device: {e}");
                self.scene.smoke.unsupported = true;
            }
        }
    }

    /// Puts the frame's particle records up. Whether the pass has anything to draw is
    /// [`D3d9Device::smoke_ready`].
    pub(super) fn upload_smoke(&mut self, frame: &SceneFrame) -> Result<()> {
        let sprites = frame.smoke.sprites;
        let store = &mut self.scene.smoke;
        if sprites.is_empty() || store.pipeline.is_none() || store.picture.is_none() {
            return Ok(());
        }
        write_sprites(&self.device, &mut store.buffer, sprites, "smoke")
    }

    /// The pass can draw the frame's smoke: there are particles, a picture and the pass.
    pub(super) fn smoke_ready(&self, frame: &SceneFrame) -> bool {
        let store = &self.scene.smoke;
        !frame.smoke.sprites.is_empty()
            && store.pipeline.is_some()
            && store.picture.is_some()
            && store.buffer.is_some()
    }

    /// Draws the frame's smoke over what is drawn: alpha-blended, tested against the depth
    /// but not writing it. Leaves the smoke pass's states set; the caller binds the scene's
    /// again. The scene's own constants (the view, the camera, the pixel offset) are those the
    /// particles use.
    pub(super) fn draw_smoke(&self, frame: &SceneFrame) -> Result<()> {
        let store = &self.scene.smoke;
        let (Some(p), Some(picture), Some((vb, _))) =
            (&store.pipeline, &store.picture, &store.buffer)
        else {
            return Ok(());
        };
        let count = frame.smoke.sprites.len() as u32;
        let d = &self.device;
        let linear = u32::from(self.scene_caps.linear_ui);
        let light = particle_light(frame.light.ambient, frame.light.sun_color);
        // SAFETY: plain calls on a live device; the constant arrays outlive the calls; the
        // buffer holds the whole list (put up by `upload_smoke`).
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
                (D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as u32),
                (D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32),
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
            d.SetVertexShaderConstantF(AXES_REGISTER, frame.light.cam_right.as_ptr().cast(), 1)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(LIGHT_REGISTER, light.as_ptr(), 1)
                .map_err(map_err)?;
            d.SetTexture(0, picture).map_err(map_err)?;
            d.SetStreamSource(0, &p.quad, 0, 8).map_err(map_err)?;
            d.SetIndices(&p.quad_indices).map_err(map_err)?;
            d.SetStreamSourceFreq(0, D3DSTREAMSOURCE_INDEXEDDATA | count)
                .map_err(map_err)?;
            d.SetStreamSource(1, vb, 0, RECORD).map_err(map_err)?;
            d.SetStreamSourceFreq(1, D3DSTREAMSOURCE_INSTANCEDATA | 1)
                .map_err(map_err)?;
            d.DrawIndexedPrimitive(D3DPT_TRIANGLELIST, 0, 0, 4, 0, 2)
                .map_err(map_err)?;
            for (&(state, _), &value) in sampler.iter().zip(&before) {
                d.SetSamplerState(0, state, value).map_err(map_err)?;
            }
            d.SetRenderState(D3DRS_SRGBWRITEENABLE, 0)
                .map_err(map_err)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_light_on_a_particle_is_ambient_and_a_share_of_the_sun_capped() {
        let l = particle_light([0.2, 0.3, 0.4, 1.0], [1.0, 0.5, 0.0, 1.0]);
        assert!(
            (l[0] - 0.8).abs() < 1e-6 && (l[1] - 0.6).abs() < 1e-6 && (l[2] - 0.4).abs() < 1e-6
        );
        // never brighter than 1.2, however bright the day
        assert_eq!(particle_light([1.0; 4], [1.0; 4])[..3], [LIGHT_MAX; 3]);
        // at night only the ambient
        assert_eq!(particle_light([0.1; 4], [0.0; 4])[..3], [0.1; 3]);
    }
}
