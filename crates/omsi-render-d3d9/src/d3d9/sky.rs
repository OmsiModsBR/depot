//! The sky dome on the device: the picture behind the scene, drawn first with the sky shader
//! (`shaders/sky.hlsl`) from the three pictures of the map's `[sky_textures]`. The dome is a
//! hemisphere (a little more) that rides with the camera, so it never shows parallax and
//! turns with the camera's orientation; it is drawn without depth, before anything else, and
//! the scene is drawn over it. Without the pictures (none set, or not put up) the picture
//! keeps its clear colour. Over the pictures the same pass draws the stars (at night) and the
//! weather's layer of clouds from the cloud type's own picture (when the frame has one and it
//! is put up); both are in the sky's pixel shader, a pass of its own.
//!
//! The sun's glow is a second pass of its own (`shaders/sunglow.hlsl`), drawn after the scene
//! so that what stands in front of the sun hides it by the depth test.
//!
//! It is a pass of its own, with its own small shaders, so the scene's pixel shader keeps its
//! instruction slots. A device that cannot make it (the pass is tried once) draws the scene
//! over the clear colour and says so in the log.

use windows::Win32::Graphics::Direct3D9::*;

use super::scene::A2c;
use super::{map_err, shader, some, D3d9Device};
use crate::scene::SceneFrame;
use crate::{BackendError, Result};

/// The pixel shader's registers of the sky's light, of the fog and of what the clouds are lit
/// by and drawn from (the scene's own, below 32, are left alone).
const SKY_REGISTER: u32 = 200;
/// The dome's latitude rings and the segments of each, as the default renderer's.
const RINGS: u32 = 12;
const SEGMENTS: u32 = 32;

/// Shaders and the dome, made at the first scene frame.
pub(super) struct SkyPipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    ps: IDirect3DPixelShader9,
    vertices: IDirect3DVertexBuffer9,
    indices: IDirect3DIndexBuffer9,
    vertex_count: u32,
    triangles: u32,
}

/// The sun glow's shaders and its square.
pub(super) struct GlowPipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    ps: IDirect3DPixelShader9,
    quad: IDirect3DVertexBuffer9,
}

#[derive(Default)]
pub(super) struct SkyStore {
    pipeline: Option<SkyPipeline>,
    /// The pass cannot be made on this device (it is not tried again every frame).
    unsupported: bool,
    glow: Option<GlowPipeline>,
    glow_unsupported: bool,
}

/// Where the glow's square is put, in metres from the camera (inside the dome's reach, as far
/// as the sky is).
const GLOW_DISTANCE: f32 = 4000.0;
/// The glow's half angle: its square reaches this far from the sun's place.
/// **Provisional**: the original's size is not observed yet.
const GLOW_HALF_ANGLE_DEG: f32 = 7.0;

/// How strongly the sun's glow shows (0 none .. 1): the sun's own strength (it is 0 under an
/// overcast cover), faded out as the sun goes below the horizon, thinned by the cloud cover
/// and gone in a thick fog, in the way the fog covers the sky.
pub(crate) fn glow_strength(light: &crate::scene::SceneLight, cloud_cover: f32) -> f32 {
    let sun = light.sun_dir[3].clamp(0.0, 1.0);
    let horizon = ((light.sun_dir[2] + 0.03) / 0.08).clamp(0.0, 1.0);
    // (a closed deck, the Overcast types, hides the sun whatever strength the light carries)
    let cover = cloud_cover.clamp(0.0, 1.0);
    let closed = ((cover - 0.8) / 0.2).clamp(0.0, 1.0);
    let clouds = (1.0 - 0.6 * cover) * (1.0 - closed * closed * (3.0 - 2.0 * closed));
    let fog = 1.0 - (light.fog[3] * 150.0 - 0.15).clamp(0.0, 1.0);
    sun * horizon * clouds * fog
}

/// The dome: latitude rings from a little below the horizon to the zenith, and the triangles
/// between them.
pub(crate) fn dome() -> (Vec<f32>, Vec<u16>) {
    let mut vertices = Vec::with_capacity(((RINGS + 1) * (SEGMENTS + 1) * 3) as usize);
    for r in 0..=RINGS {
        let elev = -0.15 + (std::f32::consts::FRAC_PI_2 + 0.15) * r as f32 / RINGS as f32;
        for s in 0..=SEGMENTS {
            let az = s as f32 / SEGMENTS as f32 * std::f32::consts::TAU;
            vertices.extend_from_slice(&[elev.cos() * az.sin(), elev.cos() * az.cos(), elev.sin()]);
        }
    }
    let mut indices = Vec::with_capacity((RINGS * SEGMENTS * 6) as usize);
    for r in 0..RINGS {
        for s in 0..SEGMENTS {
            let a = r * (SEGMENTS + 1) + s;
            let b = a + SEGMENTS + 1;
            for i in [a, b, a + 1, a + 1, b, b + 1] {
                indices.push(i as u16);
            }
        }
    }
    (vertices, indices)
}

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<SkyPipeline> {
    let vs_code = shader::compile(shader::SKY, "sky.hlsl", "vs_main", shader::Stage::Vertex)?;
    let ps_code = shader::compile(shader::SKY, "sky.hlsl", "ps_main", shader::Stage::Pixel)?;
    let elements = [
        D3DVERTEXELEMENT9 {
            Stream: 0,
            Offset: 0,
            Type: D3DDECLTYPE_FLOAT3.0 as u8,
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
    let (verts, indices) = dome();
    let vertex_bytes = std::mem::size_of_val(verts.as_slice()) as u32;
    let index_bytes = std::mem::size_of_val(indices.as_slice()) as u32;
    // SAFETY: out-parameters are live locals; the locked ranges are exactly the sizes the
    // buffers were created with; the bytecode and element arrays outlive the calls.
    unsafe {
        let mut vb = None;
        device
            .CreateVertexBuffer(
                vertex_bytes,
                D3DUSAGE_WRITEONLY as u32,
                0,
                D3DPOOL_DEFAULT,
                &mut vb,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let vb = some(vb, "sky vertices")?;
        super::write_buffer(&vb, &verts)?;
        let mut ib = None;
        device
            .CreateIndexBuffer(
                index_bytes,
                D3DUSAGE_WRITEONLY as u32,
                D3DFMT_INDEX16,
                D3DPOOL_DEFAULT,
                &mut ib,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let ib = some(ib, "sky indices")?;
        let mut p = std::ptr::null_mut();
        ib.Lock(0, index_bytes, &mut p, 0).map_err(map_err)?;
        std::ptr::copy_nonoverlapping(
            indices.as_ptr() as *const u8,
            p as *mut u8,
            index_bytes as usize,
        );
        ib.Unlock().map_err(map_err)?;
        Ok(SkyPipeline {
            vertex_count: (verts.len() / 3) as u32,
            triangles: (indices.len() / 3) as u32,
            vertices: vb,
            indices: ib,
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

fn build_glow(device: &IDirect3DDevice9Ex) -> Result<GlowPipeline> {
    let vs_code = shader::compile(
        shader::SUN_GLOW,
        "sunglow.hlsl",
        "vs_main",
        shader::Stage::Vertex,
    )?;
    let ps_code = shader::compile(
        shader::SUN_GLOW,
        "sunglow.hlsl",
        "ps_main",
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
    // a strip of four corners
    let corners: [f32; 8] = [-1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, 1.0];
    // SAFETY: out-parameters are live locals; the bytecode and element arrays outlive the calls.
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
        let quad = some(vb, "sun glow corners")?;
        super::write_buffer(&quad, &corners)?;
        Ok(GlowPipeline {
            quad,
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
    /// Builds the sky pass if it is not there. A pass that cannot be made is remembered and
    /// the scene is drawn over the clear colour.
    pub(super) fn ensure_sky(&mut self) {
        if self.scene.sky.glow.is_none() && !self.scene.sky.glow_unsupported {
            match build_glow(&self.device) {
                Ok(g) => self.scene.sky.glow = Some(g),
                Err(BackendError::DeviceLost | BackendError::DeviceNotReset) => {}
                Err(e) => {
                    log::warn!("d3d9: the sun's glow cannot be drawn on this device: {e}");
                    self.scene.sky.glow_unsupported = true;
                }
            }
        }
        if self.scene.sky.pipeline.is_some() || self.scene.sky.unsupported {
            return;
        }
        match build_pipeline(&self.device) {
            Ok(p) => self.scene.sky.pipeline = Some(p),
            Err(BackendError::DeviceLost | BackendError::DeviceNotReset) => {}
            Err(e) => {
                log::warn!("d3d9: the sky cannot be drawn on this device: {e}");
                self.scene.sky.unsupported = true;
            }
        }
    }

    /// Draws the frame's sky over the cleared picture: no depth test and no depth written, so
    /// the scene is drawn over it whatever lies where; nothing when the frame has no sky
    /// pictures or one of them is not put up. Leaves the states the scene sets for itself in
    /// its own setup (filters, depth test, blending, shaders and their constants) to that
    /// setup, which runs after this; the alpha-to-coverage switch is turned off here, as the
    /// first batch sets it for itself.
    pub(super) fn draw_sky(
        &self,
        frame: &SceneFrame,
        pixel_offset: [f32; 4],
        coverage: A2c,
    ) -> Result<()> {
        let (Some(p), Some(ids)) = (&self.scene.sky.pipeline, frame.sky.textures) else {
            return Ok(());
        };
        let mut pictures = Vec::with_capacity(4);
        for id in ids {
            match self.scene_texture(id) {
                Some(t) => pictures.push(t),
                None => return Ok(()),
            }
        }
        // the cloud layer: its picture, or (no clouds) any other picture with a cover of 0
        let mut clouds = frame.sky.clouds.params;
        match frame
            .sky
            .clouds
            .texture
            .and_then(|id| self.scene_texture(id))
        {
            Some(t) => pictures.push(t),
            None => {
                clouds[0] = 0.0;
                pictures.push(pictures[0]);
            }
        }
        let d = &self.device;
        let light = &frame.light;
        let ps = [
            frame.sky.params,
            light.fog,
            light.sun_dir,
            light.sun_color,
            light.ambient,
            light.sky_color,
            clouds,
        ];
        let vs = [light.cam_pos, pixel_offset];
        // SAFETY: plain calls on a live device; the constant arrays outlive the calls; the
        // buffers and textures belong to this device.
        unsafe {
            coverage.set(d, false)?;
            for (state, value) in [
                (D3DRS_ZENABLE, D3DZB_FALSE.0 as u32),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_ALPHABLENDENABLE, 0),
                (D3DRS_COLORWRITEENABLE, 0xF),
                (D3DRS_SRGBWRITEENABLE, 0),
            ] {
                d.SetRenderState(state, value).map_err(map_err)?;
            }
            for (stage, texture) in pictures.iter().enumerate() {
                let stage = stage as u32;
                d.SetTexture(stage, *texture).map_err(map_err)?;
                // the pictures are read as stored; round the dome they repeat, from the
                // zenith to the horizon they stop at the edge
                for (state, value) in [
                    (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                    (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                    // (the clouds' picture is laid flat and far away: it takes its mips)
                    (
                        D3DSAMP_MIPFILTER,
                        if stage == 3 {
                            D3DTEXF_LINEAR.0 as u32
                        } else {
                            D3DTEXF_NONE.0 as u32
                        },
                    ),
                    (D3DSAMP_ADDRESSU, D3DTADDRESS_WRAP.0 as u32),
                    (
                        D3DSAMP_ADDRESSV,
                        if stage == 3 {
                            D3DTADDRESS_WRAP.0 as u32
                        } else {
                            D3DTADDRESS_CLAMP.0 as u32
                        },
                    ),
                    (D3DSAMP_SRGBTEXTURE, 0),
                ] {
                    d.SetSamplerState(stage, state, value).map_err(map_err)?;
                }
            }
            d.SetStreamSourceFreq(0, 1).map_err(map_err)?;
            d.SetStreamSourceFreq(1, 1).map_err(map_err)?;
            d.SetVertexDeclaration(&p.decl).map_err(map_err)?;
            d.SetVertexShader(&p.vs).map_err(map_err)?;
            d.SetPixelShader(&p.ps).map_err(map_err)?;
            d.SetVertexShaderConstantF(0, frame.view_proj.as_ptr().cast(), 4)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(4, vs.as_ptr().cast(), vs.len() as u32)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(SKY_REGISTER, ps.as_ptr().cast(), ps.len() as u32)
                .map_err(map_err)?;
            d.SetStreamSource(0, &p.vertices, 0, 12).map_err(map_err)?;
            d.SetIndices(&p.indices).map_err(map_err)?;
            d.DrawIndexedPrimitive(D3DPT_TRIANGLELIST, 0, 0, p.vertex_count, 0, p.triangles)
                .map_err(map_err)?;
        }
        Ok(())
    }
}

impl D3d9Device {
    /// Draws the sun's glow over the scene drawn so far: a bright glow added around the sun's
    /// place, depth-tested at the far plane so that whatever stands in front of the sun
    /// covers it (glass, which writes no depth, lets it through). Nothing when the frame has
    /// the glow off, the sun gives no light (night, overcast, below the horizon, thick fog) or
    /// the shaders could not be made.
    pub(super) fn draw_sun_glow(
        &self,
        frame: &SceneFrame,
        pixel_offset: [f32; 4],
        coverage: A2c,
    ) -> Result<()> {
        let (true, Some(g)) = (frame.sky.sun_glow, &self.scene.sky.glow) else {
            return Ok(());
        };
        let light = &frame.light;
        // (cloud cover counts only when the clouds are really drawn)
        let clouds = frame.sky.clouds;
        let cover = match clouds.texture {
            Some(id) if self.scene_texture(id).is_some() => clouds.params[0],
            _ => 0.0,
        };
        let strength = glow_strength(light, cover);
        let [sx, sy, sz] = [light.sun_dir[0], light.sun_dir[1], light.sun_dir[2]];
        let len = (sx * sx + sy * sy + sz * sz).sqrt();
        let max = light.sun_color[..3].iter().cloned().fold(0.0, f32::max);
        if !(strength > 0.002 && len > 0.5 && max > 0.001) {
            return Ok(());
        }
        // the sun's own colour (warm at its rise and set), at the strength worked out
        let colour = [
            light.sun_color[0] / max * strength,
            light.sun_color[1] / max * strength,
            light.sun_color[2] / max * strength,
            0.0,
        ];
        let sun = [sx / len, sy / len, sz / len, GLOW_DISTANCE];
        let half_side = [
            GLOW_DISTANCE * GLOW_HALF_ANGLE_DEG.to_radians().tan(),
            0.0,
            0.0,
            0.0,
        ];
        let d = &self.device;
        let vs = [
            light.cam_pos,
            pixel_offset,
            light.cam_right,
            light.cam_up,
            sun,
            half_side,
        ];
        // SAFETY: plain calls on a live device; the constant arrays outlive the calls; the
        // buffer belongs to this device.
        unsafe {
            coverage.set(d, false)?;
            for (state, value) in [
                (D3DRS_ZENABLE, D3DZB_TRUE.0 as u32),
                (D3DRS_ZFUNC, D3DCMP_GREATEREQUAL.0 as u32),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_DEPTHBIAS, 0),
                (D3DRS_SLOPESCALEDEPTHBIAS, 0),
                (D3DRS_ALPHABLENDENABLE, 1),
                (D3DRS_SEPARATEALPHABLENDENABLE, 0),
                // added to the picture
                (D3DRS_SRCBLEND, D3DBLEND_ONE.0 as u32),
                (D3DRS_DESTBLEND, D3DBLEND_ONE.0 as u32),
                (D3DRS_COLORWRITEENABLE, 0x7),
                (D3DRS_SRGBWRITEENABLE, 0),
            ] {
                d.SetRenderState(state, value).map_err(map_err)?;
            }
            d.SetStreamSourceFreq(0, 1).map_err(map_err)?;
            d.SetStreamSourceFreq(1, 1).map_err(map_err)?;
            d.SetVertexDeclaration(&g.decl).map_err(map_err)?;
            d.SetVertexShader(&g.vs).map_err(map_err)?;
            d.SetPixelShader(&g.ps).map_err(map_err)?;
            d.SetVertexShaderConstantF(0, frame.view_proj.as_ptr().cast(), 4)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(4, vs.as_ptr().cast(), vs.len() as u32)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(SKY_REGISTER, colour.as_ptr(), 1)
                .map_err(map_err)?;
            d.SetStreamSource(0, &g.quad, 0, 8).map_err(map_err)?;
            d.DrawPrimitive(D3DPT_TRIANGLESTRIP, 0, 2)
                .map_err(map_err)?;
            // (the scene's blending, as the pass that follows expects it)
            d.SetRenderState(D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_COLORWRITEENABLE, 0xF)
                .map_err(map_err)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dome_is_closed_over_the_zenith_and_below_the_horizon() {
        let (v, i) = dome();
        assert_eq!(v.len(), ((RINGS + 1) * (SEGMENTS + 1) * 3) as usize);
        assert_eq!(i.len(), (RINGS * SEGMENTS * 6) as usize);
        assert!(i.iter().all(|&k| usize::from(k) < v.len() / 3));
        // unit directions from a little below the horizon (z < 0) to the zenith
        let z: Vec<f32> = v.chunks(3).map(|p| p[2]).collect();
        assert!(z.iter().cloned().fold(f32::MAX, f32::min) < 0.0);
        assert!((z.iter().cloned().fold(f32::MIN, f32::max) - 1.0).abs() < 1e-6);
        for p in v.chunks(3) {
            let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
            assert!((len - 1.0).abs() < 1e-5);
        }
    }
}
