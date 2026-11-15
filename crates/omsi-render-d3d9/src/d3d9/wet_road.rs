//! The wet road on the device: a pass of its own over the surfaces whose texture carries
//! `[moisture]` or `[puddles]` (the material's uniform vector 3, z), drawn with the scene's
//! vertex shader and the surface's own mesh and instances right after the scene's batches
//! (`shaders/wet_road.hlsl`). The scene's pixel shader keeps its instruction slots; a device
//! that cannot make the pass (it is tried once) draws the world dry and says so in the log.
//!
//! Not part of it yet: what the puddles mirror besides the sky (houses, vehicles, people).

use windows::Win32::Graphics::Direct3D9::*;

use super::scene::{address_mode, kind_states, A2c, INSTANCE_STRIDE};
use super::{map_err, shader, D3d9Device};
use crate::scene::{
    surface_depth_bias, DrawKind, SceneBatch, SceneFrame, SceneMaterial, ALPHA_CUT,
};
use crate::wet_road::{sheen, wetness, wets};
use crate::{BackendError, Result};

/// The pass's pixel shader, made at the first scene frame.
pub(super) struct WetPipeline {
    ps: IDirect3DPixelShader9,
}

#[derive(Default)]
pub(super) struct WetStore {
    pipeline: Option<WetPipeline>,
    /// The pass cannot be made on this device (it is not tried again every frame).
    unsupported: bool,
}

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<WetPipeline> {
    let code = shader::compile(
        shader::WET_ROAD,
        "wet_road.hlsl",
        "ps_main",
        shader::Stage::Pixel,
    )?;
    // SAFETY: the bytecode outlives the call.
    let ps = unsafe { device.CreatePixelShader(code.as_ptr()) }.map_err(map_err)?;
    Ok(WetPipeline { ps })
}

impl D3d9Device {
    /// Builds the pass if it is not there. A pass that cannot be made is remembered and the
    /// scene is drawn dry.
    pub(super) fn ensure_wet_road(&mut self) {
        let wet = &mut self.scene.wet;
        if wet.pipeline.is_some() || wet.unsupported {
            return;
        }
        match build_pipeline(&self.device) {
            Ok(p) => wet.pipeline = Some(p),
            Err(BackendError::DeviceLost | BackendError::DeviceNotReset) => {}
            Err(e) => {
                log::warn!("d3d9: the wet road cannot be drawn on this device: {e}");
                wet.unsupported = true;
            }
        }
    }

    /// Draws the wet road over the wet surfaces among `batches`, which the scene has just
    /// drawn. Leaves the pass's states set: the caller binds the scene's again. Returns
    /// whether anything was drawn.
    pub(super) fn draw_wet_road<'m>(
        &self,
        frame: &SceneFrame,
        batches: &[SceneBatch],
        material: &impl Fn(usize) -> Option<SceneMaterial<'m>>,
        coverage: A2c,
    ) -> Result<bool> {
        let (Some(p), Some(scene), Some((instances, _))) = (
            &self.scene.wet.pipeline,
            &self.scene.pipeline,
            &self.scene.instances,
        ) else {
            return Ok(false);
        };
        let light = &frame.light;
        let wet = wetness(light);
        if wet <= 0.0 {
            return Ok(false);
        }
        let d = &self.device;
        let [r, g, b] = sheen(light);
        let constants = [
            [r, g, b, wet],
            [
                light.cam_pos[0],
                light.cam_pos[1],
                light.cam_pos[2],
                light.fog[3],
            ],
            [light.weather[2], light.weather[3], 0.0, 0.0],
        ];
        let (bias, slope) = surface_depth_bias(frame.surface_bias);
        let mut drawn = false;
        let mut current: (Option<bool>, Option<bool>) = (None, None);
        // SAFETY: plain calls on a live device; the constant arrays outlive the calls; the
        // meshes, textures and buffers belong to this device; a run lies in the instance
        // list (checked by `SceneFrame::validate`), which the buffer holds whole.
        unsafe {
            for b in batches {
                if b.count == 0 || b.instances.is_empty() {
                    continue;
                }
                let (Some(m), Some(Some(mesh))) =
                    (material(b.material), self.scene.meshes.get(b.mesh))
                else {
                    continue;
                };
                if !wets(b.kind, &m) {
                    continue;
                }
                if !drawn {
                    kind_states(d, DrawKind::BlendNoWrite, coverage)?;
                    for (state, value) in [
                        // (the colour only: the picture's alpha is left as the scene made it)
                        (D3DRS_COLORWRITEENABLE, 0x7),
                        (D3DRS_SRCBLEND, D3DBLEND_ONE.0 as u32),
                        (D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32),
                    ] {
                        d.SetRenderState(state, value).map_err(map_err)?;
                    }
                    d.SetVertexDeclaration(&scene.decl).map_err(map_err)?;
                    d.SetVertexShader(&scene.vs).map_err(map_err)?;
                    d.SetPixelShader(&p.ps).map_err(map_err)?;
                    // (the surface's vertex-lit specular is the scene's business: none here)
                    d.SetVertexShaderConstantF(11, [[0.0f32; 4]; 2].as_ptr().cast(), 2)
                        .map_err(map_err)?;
                    d.SetPixelShaderConstantF(0, constants.as_ptr().cast(), constants.len() as u32)
                        .map_err(map_err)?;
                    drawn = true;
                }
                if current.0 != Some(b.cull) {
                    let mode = if b.cull { D3DCULL_CCW } else { D3DCULL_NONE };
                    d.SetRenderState(D3DRS_CULLMODE, mode.0 as u32)
                        .map_err(map_err)?;
                    current.0 = Some(b.cull);
                }
                // (a road lies at the depth the scene drew it at: pulled the same way)
                if current.1 != Some(b.surface) {
                    let (c, s) = if b.surface { (bias, slope) } else { (0.0, 0.0) };
                    d.SetRenderState(D3DRS_DEPTHBIAS, c.to_bits())
                        .map_err(map_err)?;
                    d.SetRenderState(D3DRS_SLOPESCALEDEPTHBIAS, s.to_bits())
                        .map_err(map_err)?;
                    current.1 = Some(b.surface);
                }
                let mode = match b.kind {
                    DrawKind::AlphaTest => 1.0,
                    DrawKind::Blend => 2.0,
                    _ => 0.0,
                };
                let terrain = m.uniform.get(2).map_or(0.0, |v| v[0]);
                let per_batch = [mode, ALPHA_CUT, terrain, 0.0];
                d.SetPixelShaderConstantF(3, per_batch.as_ptr(), 1)
                    .map_err(map_err)?;
                let texture = m
                    .texture
                    .and_then(|t| self.scene.textures.get(t))
                    .and_then(Option::as_ref);
                d.SetTexture(0, texture.unwrap_or(&self.pipeline.white))
                    .map_err(map_err)?;
                let address = address_mode(m.address);
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
                d.SetPixelShader(&scene.ps).map_err(map_err)?;
                d.SetRenderState(D3DRS_COLORWRITEENABLE, 0xF)
                    .map_err(map_err)?;
            }
        }
        Ok(drawn)
    }
}
