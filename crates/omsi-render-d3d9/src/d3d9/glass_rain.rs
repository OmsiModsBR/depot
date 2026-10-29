//! Raindrops on glass on the device: a pass of its own over the panes of the materials that
//! name a `[matl_raindropmap]`, drawn with the scene's vertex shader and the pane's own
//! mesh and instances right after the scene's batches (`shaders/glass_rain.hlsl`). The
//! scene's pixel shader keeps its instruction slots; a device that cannot make the pass
//! (it is tried once) draws the world without drops and says so in the log.
//!
//! What the drops are and how the rain and the wiper move them is in [`crate::glass_rain`];
//! this module keeps that state between pictures and draws it.

use windows::Win32::Graphics::Direct3D9::*;

use super::scene::{address_mode, kind_states, make_texture, A2c, INSTANCE_STRIDE};
use super::{map_err, shader, D3d9Device};
use crate::glass_rain::{drops_picture, GlassRain, DROPS_SIDE};
use crate::scene::{DrawKind, SceneBatch, SceneFrame, SceneMaterial, SceneTexture, TexFormat};
use crate::{BackendError, Result};

/// The drops' picture repeats this often across a pane, and the most opaque a drop is.
const REPEATS: f32 = 3.0;
const STRENGTH: f32 = 0.85;
/// The least light the drops catch (by night they are dark, not gone).
const LEAST_LIGHT: f32 = 0.12;

/// The pass's pixel shader and the drops' picture, made at the first scene frame.
pub(super) struct GlassPipeline {
    ps: IDirect3DPixelShader9,
    drops: IDirect3DTexture9,
}

#[derive(Default)]
pub(super) struct GlassStore {
    pipeline: Option<GlassPipeline>,
    /// The pass cannot be made on this device (it is not tried again every frame).
    unsupported: bool,
    /// How far the drops have formed, kept from picture to picture.
    pub(super) state: GlassRain,
}

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<GlassPipeline> {
    let code = shader::compile(
        shader::GLASS_RAIN,
        "glass_rain.hlsl",
        "ps_main",
        shader::Stage::Pixel,
    )?;
    let drops = make_texture(
        device,
        &SceneTexture {
            width: DROPS_SIDE,
            height: DROPS_SIDE,
            format: TexFormat::Rgba8,
            levels: &[drops_picture()],
            make_mips: true,
        },
    )?;
    // SAFETY: the bytecode outlives the call.
    let ps = unsafe { device.CreatePixelShader(code.as_ptr()) }.map_err(map_err)?;
    Ok(GlassPipeline { ps, drops })
}

impl D3d9Device {
    /// Builds the pass if it is not there. A pass that cannot be made is remembered and the
    /// scene is drawn without drops.
    pub(super) fn ensure_glass_rain(&mut self) {
        let glass = &mut self.scene.glass;
        if glass.pipeline.is_some() || glass.unsupported {
            return;
        }
        match build_pipeline(&self.device) {
            Ok(p) => glass.pipeline = Some(p),
            Err(BackendError::DeviceLost | BackendError::DeviceNotReset) => {}
            Err(e) => {
                log::warn!("d3d9: raindrops on glass cannot be drawn on this device: {e}");
                glass.unsupported = true;
            }
        }
    }

    /// Moves the drops on by the picture's rain and wiper (the window's pictures only: a
    /// mirror's carries no time).
    pub(super) fn step_glass_rain(&mut self, frame: &SceneFrame) {
        self.scene.glass.state.step(&frame.rain);
    }

    /// Whether this batch is a pane the pass draws drops on.
    fn has_raindrop_map<'m>(
        &self,
        b: &SceneBatch,
        material: &impl Fn(usize) -> Option<SceneMaterial<'m>>,
    ) -> Option<usize> {
        if b.count == 0 || b.instances.is_empty() || b.surface {
            return None;
        }
        material(b.material)?
            .raindrop_map
            .filter(|&t| self.scene.textures.get(t).is_some_and(Option::is_some))
    }

    /// Draws the drops over the panes among `batches` (the world's or the cab's), which the
    /// scene has just drawn. Leaves the pass's states set: the caller binds the scene's
    /// again. Returns whether anything was drawn.
    pub(super) fn draw_glass_rain<'m>(
        &self,
        frame: &SceneFrame,
        batches: &[SceneBatch],
        material: &impl Fn(usize) -> Option<SceneMaterial<'m>>,
        coverage: A2c,
    ) -> Result<bool> {
        let glass = &self.scene.glass;
        let (Some(p), Some(scene), Some((instances, _))) =
            (&glass.pipeline, &self.scene.pipeline, &self.scene.instances)
        else {
            return Ok(false);
        };
        if glass.state.is_dry() {
            return Ok(false);
        }
        let d = &self.device;
        let light = &frame.light;
        let tint: [f32; 4] = std::array::from_fn(|c| {
            if c == 3 {
                1.0
            } else {
                (light.ambient[c] + 0.35 * light.sun_color[c]).clamp(LEAST_LIGHT, 1.0)
            }
        });
        let look = [REPEATS, STRENGTH, 0.0, 0.0];
        let mut constants = Vec::with_capacity(10);
        for four in glass.state.bins().chunks_exact(4) {
            constants.push([four[0], four[1], four[2], four[3]]);
        }
        constants.push(look);
        constants.push(tint);
        let mut drawn = false;
        let mut current: Option<bool> = None;
        // SAFETY: plain calls on a live device; the constant arrays outlive the calls; the
        // meshes, textures and buffers belong to this device; a run lies in the instance
        // list (checked by `SceneFrame::validate`), which the buffer holds whole.
        unsafe {
            for b in batches {
                let (Some(map), Some(Some(mesh))) = (
                    self.has_raindrop_map(b, material),
                    self.scene.meshes.get(b.mesh),
                ) else {
                    continue;
                };
                if !drawn {
                    kind_states(d, DrawKind::BlendNoWrite, coverage)?;
                    for (state, value) in [
                        // (the colour only: the picture's alpha is left as the scene made it)
                        (D3DRS_COLORWRITEENABLE, 0x7),
                        (D3DRS_DEPTHBIAS, 0),
                        (D3DRS_SLOPESCALEDEPTHBIAS, 0),
                    ] {
                        d.SetRenderState(state, value).map_err(map_err)?;
                    }
                    d.SetVertexDeclaration(&scene.decl).map_err(map_err)?;
                    d.SetVertexShader(&scene.vs).map_err(map_err)?;
                    d.SetPixelShader(&p.ps).map_err(map_err)?;
                    // (the pane's vertex-lit specular is the scene's business: none here)
                    d.SetVertexShaderConstantF(11, [[0.0f32; 4]; 2].as_ptr().cast(), 2)
                        .map_err(map_err)?;
                    d.SetPixelShaderConstantF(0, constants.as_ptr().cast(), constants.len() as u32)
                        .map_err(map_err)?;
                    d.SetTexture(1, &p.drops).map_err(map_err)?;
                    for (state, value) in [
                        (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                        (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                        (D3DSAMP_MIPFILTER, D3DTEXF_LINEAR.0 as u32),
                        (D3DSAMP_MAXANISOTROPY, 1),
                        (D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP.0 as u32),
                        (D3DSAMP_ADDRESSV, D3DTADDRESS_WRAP.0 as u32),
                    ] {
                        d.SetSamplerState(1, state, value).map_err(map_err)?;
                    }
                    d.SetSamplerState(0, D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32)
                        .map_err(map_err)?;
                    d.SetSamplerState(0, D3DSAMP_MAXANISOTROPY, 1)
                        .map_err(map_err)?;
                    drawn = true;
                }
                if current != Some(b.cull) {
                    let mode = if b.cull { D3DCULL_CCW } else { D3DCULL_NONE };
                    d.SetRenderState(D3DRS_CULLMODE, mode.0 as u32)
                        .map_err(map_err)?;
                    current = Some(b.cull);
                }
                if let Some(Some(texture)) = self.scene.textures.get(map) {
                    d.SetTexture(0, texture).map_err(map_err)?;
                }
                let address = material(b.material)
                    .map_or(D3DTADDRESS_WRAP.0 as u32, |m| address_mode(m.address));
                d.SetSamplerState(0, D3DSAMP_ADDRESSU, address)
                    .map_err(map_err)?;
                d.SetSamplerState(0, D3DSAMP_ADDRESSV, address)
                    .map_err(map_err)?;
                d.SetStreamSource(0, &mesh.vb, 0, 32).map_err(map_err)?;
                d.SetIndices(&mesh.ib).map_err(map_err)?;
                d.SetStreamSourceFreq(0, D3DSTREAMSOURCE_INDEXEDDATA | b.instances.len() as u32)
                    .map_err(map_err)?;
                d.SetStreamSource(
                    1,
                    instances,
                    b.instances.start * INSTANCE_STRIDE,
                    INSTANCE_STRIDE,
                )
                .map_err(map_err)?;
                d.SetStreamSourceFreq(1, D3DSTREAMSOURCE_INSTANCEDATA | 1)
                    .map_err(map_err)?;
                d.DrawIndexedPrimitive(
                    D3DPT_TRIANGLELIST,
                    0,
                    0,
                    mesh.vertices,
                    b.first,
                    b.count / 3,
                )
                .map_err(map_err)?;
            }
            if drawn {
                // the scene's pixel shader and filtering come back
                d.SetPixelShader(&scene.ps).map_err(map_err)?;
                for stage in 0..2u32 {
                    let min = if self.anisotropy > 1 {
                        D3DTEXF_ANISOTROPIC
                    } else {
                        D3DTEXF_LINEAR
                    };
                    for (state, value) in [
                        (D3DSAMP_MINFILTER, min.0 as u32),
                        (D3DSAMP_MAXANISOTROPY, self.anisotropy),
                    ] {
                        d.SetSamplerState(stage, state, value).map_err(map_err)?;
                    }
                }
                d.SetRenderState(D3DRS_COLORWRITEENABLE, 0xF)
                    .map_err(map_err)?;
            }
        }
        Ok(drawn)
    }
}
