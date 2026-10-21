//! The interface on the device: each owner's pictures, atlas and vertex lists (an owner is
//! one `omsi-ui` Gpu whose draws are mirrored here), and [`D3d9Device::draw_ui`] and
//! [`D3d9Device::draw_overlays`], which draw into the frame begun with
//! [`crate::RenderBackend::begin_frame`], over what it holds (the scene).

use std::collections::HashMap;

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Direct3D9::*;

use super::scene::make_texture;
use super::{map_err, shader, some, D3d9Device};
use crate::scene::{SceneTexture, TexFormat};
use crate::ui::{
    atlas_region_fits, overlay_vertices, UiFrame, UiLayer, UiOverlay, UI_VERTEX_FLOATS,
};
use crate::{BackendError, Result};

const STRIDE: u32 = (UI_VERTEX_FLOATS * 4) as u32;

/// Shaders and the vertex layout of the interface, made at its first draw.
struct UiPipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    ps: IDirect3DPixelShader9,
}

/// The atlas: a default-pool texture and the system-memory one its changed regions are
/// written into and copied from.
struct Atlas {
    size: u32,
    texture: IDirect3DTexture9,
    staging: IDirect3DTexture9,
}

/// A picture and the pixels it was made from.
struct Image {
    texture: IDirect3DTexture9,
    width: u32,
    height: u32,
    pixels: Vec<u8>,
}

/// One owner's resources, with the pixels they were made from (a device opened again after
/// a loss gets them back, see [`D3d9Device::restore_ui`]).
#[derive(Default)]
pub(super) struct Owner {
    images: Vec<Option<Image>>,
    atlas: Option<Atlas>,
    atlas_pixels: Vec<u8>,
    buffers: Vec<Vec<f32>>,
}

#[derive(Default)]
pub(super) struct UiStore {
    pub(super) owners: HashMap<usize, Owner>,
    pipeline: Option<UiPipeline>,
    /// The dynamic vertex buffer and its capacity in vertices.
    vb: Option<(IDirect3DVertexBuffer9, usize)>,
    scratch: Vec<f32>,
}

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<UiPipeline> {
    let vs_code = shader::compile(shader::UI, "ui.hlsl", "vs_main", shader::Stage::Vertex)?;
    let ps_code = shader::compile(shader::UI, "ui.hlsl", "ps_main", shader::Stage::Pixel)?;
    let element =
        |offset: u16, ty: D3DDECLTYPE, usage: D3DDECLUSAGE, index: u8| D3DVERTEXELEMENT9 {
            Stream: 0,
            Offset: offset,
            Type: ty.0 as u8,
            Method: D3DDECLMETHOD_DEFAULT.0 as u8,
            Usage: usage.0 as u8,
            UsageIndex: index,
        };
    let elements = [
        element(0, D3DDECLTYPE_FLOAT3, D3DDECLUSAGE_POSITION, 0),
        element(12, D3DDECLTYPE_FLOAT2, D3DDECLUSAGE_TEXCOORD, 0),
        element(20, D3DDECLTYPE_FLOAT2, D3DDECLUSAGE_TEXCOORD, 1),
        element(28, D3DDECLTYPE_FLOAT2, D3DDECLUSAGE_TEXCOORD, 2),
        element(36, D3DDECLTYPE_FLOAT4, D3DDECLUSAGE_TEXCOORD, 3),
        element(52, D3DDECLTYPE_FLOAT2, D3DDECLUSAGE_TEXCOORD, 4),
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
        Ok(UiPipeline {
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

/// An sRGB-encoded value of linear `c` (a clear colour, which wgpu takes linear).
/// Whether `t` is a render target of `w` by `h`.
fn is_target_of(t: &IDirect3DTexture9, w: u32, h: u32) -> bool {
    let mut desc = D3DSURFACE_DESC::default();
    // SAFETY: the out-parameter is a live local.
    unsafe { t.GetLevelDesc(0, &mut desc) }.is_ok()
        && desc.Usage & D3DUSAGE_RENDERTARGET as u32 != 0
        && (desc.Width, desc.Height) == (w, h)
}

fn to_srgb(c: f32) -> f32 {
    let c = c.clamp(0.0, 1.0);
    if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    }
}

/// Writes `region` (x, y, w, h) of the RGBA square `rgba` (`size` wide) into level 0 of
/// `staging`, as B, G, R, A.
///
/// # Safety
/// `staging` is a system-memory A8R8G8B8 texture at least `size` square, and the region was
/// checked with [`atlas_region_fits`].
unsafe fn write_region(
    staging: &IDirect3DTexture9,
    size: u32,
    rgba: &[u8],
    region: [u32; 4],
) -> Result<()> {
    let [x, y, w, h] = region;
    if w == 0 || h == 0 {
        return Ok(());
    }
    let rect = RECT {
        left: x as i32,
        top: y as i32,
        right: (x + w) as i32,
        bottom: (y + h) as i32,
    };
    let mut locked = D3DLOCKED_RECT::default();
    staging
        .LockRect(0, &mut locked, &rect, 0)
        .map_err(map_err)?;
    for row in 0..h as usize {
        let dst = std::slice::from_raw_parts_mut(
            (locked.pBits as *mut u8).add(row * locked.Pitch as usize),
            w as usize * 4,
        );
        let at = ((y as usize + row) * size as usize + x as usize) * 4;
        let src = &rgba[at..at + w as usize * 4];
        for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
            d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
        }
    }
    staging.UnlockRect(0).map_err(map_err)
}

impl D3d9Device {
    fn texture_fits(&self, width: u32, height: u32, rgba: &[u8]) -> Result<()> {
        let expected = (width as usize)
            .checked_mul(height as usize)
            .and_then(|n| n.checked_mul(4));
        if width == 0 || height == 0 || expected != Some(rgba.len()) {
            return Err(BackendError::InvalidArgument(format!(
                "a {width}x{height} picture of {} bytes",
                rgba.len()
            )));
        }
        let max = self.caps.max_texture_size;
        if width > max || height > max {
            return Err(BackendError::InvalidArgument(format!(
                "a {width}x{height} picture on a device of {max}"
            )));
        }
        if !self.caps.non_pow2_textures && !(width.is_power_of_two() && height.is_power_of_two()) {
            return Err(BackendError::Unsupported(format!(
                "a {width}x{height} picture on a device without such sizes"
            )));
        }
        Ok(())
    }

    /// Picture `id` of `owner` is (now) `rgba` (straight alpha, sRGB, `width * height * 4`
    /// bytes). Id 0 is the atlas ([`Self::ui_atlas`]) and is refused here.
    pub fn ui_image(
        &mut self,
        owner: usize,
        id: usize,
        width: u32,
        height: u32,
        rgba: &[u8],
    ) -> Result<()> {
        if id == 0 {
            return Err(BackendError::InvalidArgument(
                "picture 0 is the atlas".into(),
            ));
        }
        self.texture_fits(width, height, rgba)?;
        let pixels = rgba.to_vec();
        let texture = make_texture(
            &self.device,
            &SceneTexture {
                width,
                height,
                format: TexFormat::Rgba8,
                levels: std::slice::from_ref(&pixels),
                make_mips: false,
            },
        )?;
        let images = &mut self.ui.owners.entry(owner).or_default().images;
        if images.len() <= id {
            images.resize_with(id + 1, || None);
        }
        images[id] = Some(Image {
            texture,
            width,
            height,
            pixels,
        });
        Ok(())
    }

    /// Picture `id` of `owner` is gone: draws with it draw nothing.
    pub fn ui_free(&mut self, owner: usize, id: usize) {
        if let Some(slot) = self
            .ui
            .owners
            .get_mut(&owner)
            .and_then(|o| o.images.get_mut(id))
        {
            *slot = None;
        }
    }

    /// The atlas of `owner` (its picture 0) is `rgba`, `size` pixels square; `region` (x, y,
    /// w, h) changed. A new size makes the atlas anew from all of `rgba`.
    pub fn ui_atlas(
        &mut self,
        owner: usize,
        size: u32,
        rgba: &[u8],
        region: [u32; 4],
    ) -> Result<()> {
        if !atlas_region_fits(size, rgba.len(), region) {
            return Err(BackendError::InvalidArgument(format!(
                "atlas region {region:?} of a {size} square of {} bytes",
                rgba.len()
            )));
        }
        self.texture_fits(size, size, rgba)?;
        let o = self.ui.owners.entry(owner).or_default();
        let region = if o.atlas.as_ref().is_some_and(|a| a.size == size) {
            region
        } else {
            let make = |pool: D3DPOOL| {
                let mut t = None;
                // SAFETY: out-parameter is a live local.
                unsafe {
                    self.device
                        .CreateTexture(
                            size,
                            size,
                            1,
                            0,
                            D3DFMT_A8R8G8B8,
                            pool,
                            &mut t,
                            std::ptr::null_mut(),
                        )
                        .map_err(map_err)?;
                }
                some(t, "atlas texture")
            };
            o.atlas = Some(Atlas {
                size,
                texture: make(D3DPOOL_DEFAULT)?,
                staging: make(D3DPOOL_SYSTEMMEM)?,
            });
            [0, 0, size, size]
        };
        let atlas = o.atlas.as_ref().expect("made above");
        // SAFETY: the staging texture is `size` square and A8R8G8B8, the region checked; the
        // copy goes between this device's own textures.
        unsafe {
            write_region(&atlas.staging, size, rgba, region)?;
            self.device
                .UpdateTexture(&atlas.staging, &atlas.texture)
                .map_err(map_err)?;
        }
        o.atlas_pixels.clear();
        o.atlas_pixels.extend_from_slice(rgba);
        Ok(())
    }

    /// Vertex buffer `buffer` of `owner` holds `vertices` ([`UI_VERTEX_FLOATS`] each).
    pub fn ui_vertices(&mut self, owner: usize, buffer: usize, vertices: &[f32]) -> Result<()> {
        if !vertices.len().is_multiple_of(UI_VERTEX_FLOATS) {
            return Err(BackendError::InvalidArgument(format!(
                "{} floats of interface vertices",
                vertices.len()
            )));
        }
        let buffers = &mut self.ui.owners.entry(owner).or_default().buffers;
        if buffers.len() <= buffer {
            buffers.resize_with(buffer + 1, Vec::new);
        }
        buffers[buffer].clear();
        buffers[buffer].extend_from_slice(vertices);
        Ok(())
    }

    /// Everything `owner` put up goes.
    pub fn ui_gone(&mut self, owner: usize) {
        self.ui.owners.remove(&owner);
    }

    /// Whether picture `id` of `owner` is up (0: its atlas).
    pub fn has_ui_texture(&self, owner: usize, id: usize) -> bool {
        self.ui.owners.get(&owner).is_some_and(|o| {
            if id == 0 {
                o.atlas.is_some()
            } else {
                o.images.get(id).is_some_and(Option::is_some)
            }
        })
    }

    /// Puts `owners` (a lost device's) up again on this one.
    pub(super) fn restore_ui(&mut self, owners: HashMap<usize, Owner>) -> Result<()> {
        let mut done = Ok(());
        for (owner, old) in owners {
            self.ui.owners.entry(owner).or_default().buffers = old.buffers;
            if let Some(size) = old.atlas.as_ref().map(|a| a.size) {
                done = done.and(self.ui_atlas(owner, size, &old.atlas_pixels, [0, 0, size, size]));
            }
            for (id, image) in old.images.into_iter().enumerate() {
                if let Some(i) = image {
                    done = done.and(self.ui_image(owner, id, i.width, i.height, &i.pixels));
                }
            }
        }
        done
    }

    fn upload_ui(&mut self, vertices: &[f32]) -> Result<()> {
        let count = vertices.len() / UI_VERTEX_FLOATS;
        if count == 0 {
            return Ok(());
        }
        let too_many = || BackendError::InvalidArgument(format!("{count} interface vertices"));
        let bytes = u32::try_from(vertices.len() * 4).map_err(|_| too_many())?;
        if self.ui.vb.as_ref().is_none_or(|v| v.1 < count) {
            let capacity = count.next_power_of_two().max(4096);
            let capacity_bytes = u32::try_from(capacity)
                .ok()
                .and_then(|n| n.checked_mul(STRIDE))
                .ok_or_else(too_many)?;
            let mut vb = None;
            // SAFETY: out-parameter is a live local.
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
            self.ui.vb = Some((some(vb, "interface vertex buffer")?, capacity));
        }
        let (vb, _) = self.ui.vb.as_ref().expect("made above");
        // SAFETY: the buffer holds at least `count` vertices; locked with DISCARD and
        // written whole within `bytes`.
        unsafe {
            let mut p = std::ptr::null_mut();
            vb.Lock(0, bytes, &mut p, D3DLOCK_DISCARD as u32)
                .map_err(map_err)?;
            std::ptr::copy_nonoverlapping(
                vertices.as_ptr() as *const u8,
                p as *mut u8,
                bytes as usize,
            );
            vb.Unlock().map_err(map_err)
        }
    }

    /// The interface's states, for a target `w` by `h`.
    fn ui_states(&self) -> Result<()> {
        let d = &self.device;
        let pipeline = self.ui.pipeline.as_ref().expect("built before");
        let linear = u32::from(self.scene_caps.linear_ui);
        // SAFETY: plain state setters on a live device.
        unsafe {
            for (state, value) in [
                (D3DRS_ZENABLE, D3DZB_FALSE.0 as u32),
                (D3DRS_ZWRITEENABLE, 0),
                (D3DRS_CULLMODE, D3DCULL_NONE.0 as u32),
                (D3DRS_ALPHATESTENABLE, 0),
                (D3DRS_ALPHABLENDENABLE, 1),
                // premultiplied alpha
                (D3DRS_SRCBLEND, D3DBLEND_ONE.0 as u32),
                (D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32),
                (D3DRS_SCISSORTESTENABLE, 1),
                (D3DRS_SRGBWRITEENABLE, linear),
            ] {
                d.SetRenderState(state, value).map_err(map_err)?;
            }
            for (state, value) in [
                (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_MIPFILTER, D3DTEXF_LINEAR.0 as u32),
                (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_SRGBTEXTURE, linear),
            ] {
                d.SetSamplerState(0, state, value).map_err(map_err)?;
            }
            d.SetVertexDeclaration(&pipeline.decl).map_err(map_err)?;
            d.SetVertexShader(&pipeline.vs).map_err(map_err)?;
            d.SetPixelShader(&pipeline.ps).map_err(map_err)?;
            if let Some((vb, _)) = &self.ui.vb {
                d.SetStreamSource(0, vb, 0, STRIDE).map_err(map_err)?;
            }
        }
        Ok(())
    }

    /// Draws vertices `first..first + count` of the uploaded list as `layer` with `texture`.
    #[allow(clippy::too_many_arguments)]
    fn ui_draw(
        &self,
        layer: &UiLayer,
        (w, h): (u32, u32),
        target: (u32, u32),
        texture: &IDirect3DTexture9,
        premultiplied: bool,
        first: u32,
        count: u32,
    ) -> Result<()> {
        let (tw, th) = target;
        let Some(sc) = layer.scissor_in(w.min(tw), h.min(th)) else {
            return Ok(());
        };
        let [vx, vy, vw, vh] = layer.viewport_in(w.min(tw), h.min(th));
        let vs = [
            layer.layout_box(w, h),
            [
                0.0,
                0.0,
                layer.px_scale,
                f32::from(u8::from(self.scene_caps.linear_ui)),
            ],
            [-1.0 / vw as f32, 1.0 / vh as f32, 0.0, 0.0],
        ];
        let ps = [
            layer.clip,
            [layer.radius, layer.opacity, 0.0, 0.0],
            [f32::from(u8::from(premultiplied)), 0.0, 0.0, 0.0],
        ];
        let d = &self.device;
        // SAFETY: plain calls on a live device; the constant arrays outlive them; the range
        // was checked against the uploaded list.
        unsafe {
            d.SetViewport(&D3DVIEWPORT9 {
                X: vx,
                Y: vy,
                Width: vw,
                Height: vh,
                MinZ: 0.0,
                MaxZ: 1.0,
            })
            .map_err(map_err)?;
            d.SetScissorRect(&RECT {
                left: sc[0] as i32,
                top: sc[1] as i32,
                right: sc[2] as i32,
                bottom: sc[3] as i32,
            })
            .map_err(map_err)?;
            d.SetVertexShaderConstantF(0, layer.view_proj.as_ptr().cast(), 4)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(4, vs.as_ptr().cast(), vs.len() as u32)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(0, ps.as_ptr().cast(), ps.len() as u32)
                .map_err(map_err)?;
            d.SetTexture(0, texture).map_err(map_err)?;
            d.DrawPrimitive(D3DPT_TRIANGLELIST, first, count / 3)
                .map_err(map_err)?;
        }
        Ok(())
    }

    /// Back to the backend's own states, whatever happened.
    fn ui_restore(&self, target: (u32, u32)) -> Result<()> {
        let d = &self.device;
        // SAFETY: plain calls on a live device.
        unsafe {
            d.SetViewport(&D3DVIEWPORT9 {
                X: 0,
                Y: 0,
                Width: target.0,
                Height: target.1,
                MinZ: 0.0,
                MaxZ: 1.0,
            })
            .and_then(|_| d.SetRenderState(D3DRS_SCISSORTESTENABLE, 0))
            .and_then(|_| d.SetRenderState(D3DRS_SRGBWRITEENABLE, 0))
            .and_then(|_| d.SetSamplerState(0, D3DSAMP_SRGBTEXTURE, 0))
            .and_then(|_| d.SetSamplerState(0, D3DSAMP_MIPFILTER, D3DTEXF_NONE.0 as u32))
            .and_then(|_| d.SetRenderState(D3DRS_ALPHABLENDENABLE, 0))
            .map_err(map_err)?;
        }
        self.set_frame_states()
    }

    fn ui_target(&mut self, what: &str) -> Result<(u32, u32)> {
        let Some(frame) = self.frame.as_ref() else {
            return Err(BackendError::InvalidArgument(format!(
                "{what} outside a frame"
            )));
        };
        let target = (frame.width, frame.height);
        if self.ui.pipeline.is_none() {
            self.ui.pipeline = Some(build_pipeline(&self.device)?);
        }
        Ok(target)
    }

    /// Draws `frame` of `owner`'s interface over what the frame holds, as `omsi-ui` draws it:
    /// each draw with its layer (camera, viewport, rounded clip, opacity) and picture. A draw
    /// whose buffer, layer or picture is not there draws nothing; one outside its buffer is
    /// refused.
    pub fn draw_ui(&mut self, owner: usize, frame: &UiFrame) -> Result<()> {
        let target = self.ui_target("interface")?;
        self.ui_frame(owner, frame, target)
    }

    /// Draws `frame` of `owner`'s interface into scene texture `target` instead of over the
    /// frame: a render target of the frame's size (transparent where nothing is drawn), made
    /// here, or made again at a new size, and then drawn like any scene texture (the HUD's
    /// overlays, a material). It is drawn now, inside a frame or outside one. A device
    /// opened again does not keep it: the owner draws it again.
    pub fn draw_ui_into(&mut self, owner: usize, target: usize, frame: &UiFrame) -> Result<()> {
        let (w, h) = frame.size;
        if w == 0 || h == 0 || w.max(h) > self.caps.max_texture_size {
            return Err(BackendError::InvalidArgument(format!(
                "an interface target of {w}x{h}"
            )));
        }
        if self.ui.pipeline.is_none() {
            self.ui.pipeline = Some(build_pipeline(&self.device)?);
        }
        let texture = match self.scene_texture(target).filter(|t| is_target_of(t, w, h)) {
            Some(t) => t.clone(),
            None => {
                let mut texture = None;
                // SAFETY: the out-parameter is a live local.
                unsafe {
                    self.device
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
                let texture = some(texture, "interface target")?;
                self.put_scene_target(target, texture.clone());
                texture
            }
        };
        // SAFETY: plain calls on a live device; the surfaces belong to it. What was bound is
        // bound again whatever happened.
        unsafe {
            let surface = texture.GetSurfaceLevel(0).map_err(map_err)?;
            let color = self.device.GetRenderTarget(0).map_err(map_err)?;
            // (none bound: GetDepthStencilSurface fails, and none is bound again)
            let depth = self.device.GetDepthStencilSurface().ok();
            self.device.SetRenderTarget(0, &surface).map_err(map_err)?;
            let drawn = self
                .device
                .SetDepthStencilSurface(None::<&IDirect3DSurface9>)
                .map_err(map_err)
                .and_then(|_| {
                    let own_scene = self.frame.is_none();
                    if own_scene {
                        self.device.BeginScene().map_err(map_err)?;
                    }
                    let drawn = self.ui_frame(owner, frame, (w, h));
                    if own_scene {
                        let ended = self.device.EndScene().map_err(map_err);
                        return drawn.and(ended);
                    }
                    drawn
                });
            // (setting the colour target again sets the whole viewport again too)
            let restored = self
                .device
                .SetRenderTarget(0, &color)
                .and_then(|_| self.device.SetDepthStencilSurface(depth.as_ref()))
                .map_err(map_err);
            drawn.and(restored)
        }
    }

    /// Draws `frame` of `owner`'s interface into the bound target, `target` pixels.
    fn ui_frame(&mut self, owner: usize, frame: &UiFrame, target: (u32, u32)) -> Result<()> {
        let Some(o) = self.ui.owners.get(&owner) else {
            return Ok(());
        };
        // the owner's buffers one after the other, each draw's range moved by its buffer's start
        let mut all = std::mem::take(&mut self.ui.scratch);
        all.clear();
        let mut starts = Vec::with_capacity(o.buffers.len());
        for b in &o.buffers {
            starts.push((all.len() / UI_VERTEX_FLOATS) as u32);
            all.extend_from_slice(b);
        }
        for d in frame.draws {
            let len = o
                .buffers
                .get(d.buffer)
                .map_or(0, |b| b.len() / UI_VERTEX_FLOATS);
            if d.range.start > d.range.end || d.range.end as usize > len {
                self.ui.scratch = all;
                return Err(BackendError::InvalidArgument(format!(
                    "interface draw {:?} of buffer {} with {len} vertices",
                    d.range, d.buffer
                )));
            }
        }
        let uploaded = self.upload_ui(&all);
        self.ui.scratch = all;
        uploaded?;
        // SAFETY: the clear is a plain call on a live device.
        if let Some(c) = frame.clear {
            let argb = super::rgba_to_argb([to_srgb(c[0]), to_srgb(c[1]), to_srgb(c[2]), c[3]]);
            unsafe {
                self.device
                    .Clear(0, std::ptr::null(), D3DCLEAR_TARGET as u32, argb, 1.0, 0)
                    .map_err(map_err)?;
            }
        }
        let drawn = self.ui_states().and_then(|_| {
            let o = &self.ui.owners[&owner];
            for d in frame.draws {
                let (Some(layer), Some(start)) = (frame.layers.get(d.layer), starts.get(d.buffer))
                else {
                    continue;
                };
                let texture = if d.texture == 0 {
                    o.atlas.as_ref().map(|a| &a.texture)
                } else {
                    o.images
                        .get(d.texture)
                        .and_then(Option::as_ref)
                        .map(|i| &i.texture)
                };
                let (Some(texture), false) = (texture, d.range.is_empty()) else {
                    continue;
                };
                self.ui_draw(
                    layer,
                    frame.size,
                    target,
                    texture,
                    false,
                    start + d.range.start,
                    d.range.end - d.range.start,
                )?;
            }
            Ok(())
        });
        let restored = self.ui_restore(target);
        drawn.and(restored)
    }

    /// Draws the HUD's pictures (scene textures) over what the frame holds, in order. One
    /// whose texture is not there is left out.
    pub fn draw_overlays(&mut self, overlays: &[UiOverlay]) -> Result<()> {
        if overlays.is_empty() {
            return Ok(());
        }
        let target = self.ui_target("overlays")?;
        let mut all = std::mem::take(&mut self.ui.scratch);
        all.clear();
        for o in overlays {
            all.extend(overlay_vertices(o).iter().flatten());
        }
        let uploaded = self.upload_ui(&all);
        self.ui.scratch = all;
        uploaded?;
        let layer = UiLayer::flat([0.0, 0.0, target.0 as f32, target.1 as f32], 0.0, 1.0);
        let drawn = self.ui_states().and_then(|_| {
            for (k, o) in overlays.iter().enumerate() {
                let Some(texture) = self.scene_texture(o.texture) else {
                    continue;
                };
                self.ui_draw(
                    &layer,
                    target,
                    target,
                    texture,
                    o.premultiplied,
                    k as u32 * 6,
                    6,
                )?;
            }
            Ok(())
        });
        let restored = self.ui_restore(target);
        drawn.and(restored)
    }
}
