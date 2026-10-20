//! The game's scene on the device: meshes and textures under the game's ids, and
//! [`D3d9Device::draw_scene`], which draws a [`SceneFrame`] with the scene shader
//! (`shaders/scene.hlsl`), one instanced draw per batch.

use windows::Win32::Graphics::Direct3D9::*;

use super::{make_mesh, map_err, shader, some, write_buffer, D3d9Device, Mesh};
use crate::scene::{
    mip_chain, surface_depth_bias, validate_mesh, DrawKind, SceneBatch, SceneFrame, SceneLamps,
    SceneMaterial, SceneTexture, TexAddress, TexFormat, ALPHA_CUT, INSTANCE_FLOATS,
    MATERIAL_VECTORS,
};
use crate::{BackendError, Result};

const INSTANCE_STRIDE: u32 = (INSTANCE_FLOATS * 4) as u32;

/// The scene shader's samplers: diffuse, transmap, night, light, sphere map, its mask, bump.
const SAMPLERS: u32 = 7;

/// The samplers of the light list and of the lamp grid.
const LIGHTS_STAGE: u32 = 7;
const GRID_STAGE: u32 = 8;
/// The table of byte ORs the reflection's spilling factor is put together with.
const OR_STAGE: u32 = 10;
/// The pixel shader's register of the material's border colour (unpacked from its flags).
const BORDER_REGISTER: u32 = 24;

/// What the scene layer may use beyond the floor, read when the device opens.
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct SceneCaps {
    /// DXT1, DXT3 and DXT5 textures.
    pub(super) dxt: [bool; 3],
    /// Alpha blended with its own factors (`SEPARATEALPHABLENDENABLE`).
    pub(super) separate_alpha: bool,
    /// The interface is blended in linear light: textures read as sRGB, the target written
    /// as sRGB and converted after the blend (`D3DPMISCCAPS_POSTBLENDSRGBCONVERT`).
    pub(super) linear_ui: bool,
    /// The lamps can be read from float textures (32-bit RGBA and R, dynamic), at most this
    /// wide and high.
    pub(super) lamps: bool,
    pub(super) max_texture: (u32, u32),
}

impl SceneCaps {
    pub(super) fn read(d3d: &IDirect3D9Ex, raw: &D3DCAPS9) -> Self {
        let mut mode = D3DDISPLAYMODE::default();
        // SAFETY: the out-parameter is a live local.
        let display = unsafe { d3d.GetAdapterDisplayMode(D3DADAPTER_DEFAULT, &mut mode) }
            .map(|_| mode.Format)
            .unwrap_or(D3DFMT_X8R8G8B8);
        let check = |usage: u32, kind: D3DRESOURCETYPE, format: D3DFORMAT| {
            // SAFETY: plain query on a live interface.
            unsafe {
                d3d.CheckDeviceFormat(
                    D3DADAPTER_DEFAULT,
                    D3DDEVTYPE_HAL,
                    display,
                    usage,
                    kind,
                    format,
                )
            }
            .is_ok()
        };
        let supported = |format: D3DFORMAT| check(0, D3DRTYPE_TEXTURE, format);
        let srgb_write = (D3DUSAGE_RENDERTARGET | D3DUSAGE_QUERY_SRGBWRITE) as u32;
        let caps = Self {
            dxt: [D3DFMT_DXT1, D3DFMT_DXT3, D3DFMT_DXT5].map(supported),
            separate_alpha: raw.PrimitiveMiscCaps & D3DPMISCCAPS_SEPARATEALPHABLEND as u32 != 0,
            linear_ui: raw.PrimitiveMiscCaps & D3DPMISCCAPS_POSTBLENDSRGBCONVERT as u32 != 0
                && check(
                    D3DUSAGE_QUERY_SRGBREAD as u32,
                    D3DRTYPE_TEXTURE,
                    D3DFMT_A8R8G8B8,
                )
                && check(srgb_write, D3DRTYPE_SURFACE, display)
                && check(srgb_write, D3DRTYPE_SURFACE, D3DFMT_A8R8G8B8),
            lamps: raw.Caps2 & D3DCAPS2_DYNAMICTEXTURES as u32 != 0
                && [D3DFMT_A32B32G32R32F, D3DFMT_R32F]
                    .into_iter()
                    .all(|f| check(D3DUSAGE_DYNAMIC as u32, D3DRTYPE_TEXTURE, f)),
            max_texture: (raw.MaxTextureWidth, raw.MaxTextureHeight),
        };
        log::info!("d3d9 scene: {caps:?}");
        caps
    }

    fn allows(&self, format: TexFormat) -> bool {
        match format {
            TexFormat::Rgba8 => true,
            TexFormat::Bc1 => self.dxt[0],
            TexFormat::Bc2 => self.dxt[1],
            TexFormat::Bc3 => self.dxt[2],
        }
    }
}

/// Shaders and the vertex layout of the scene, made at the first scene frame.
struct ScenePipeline {
    decl: IDirect3DVertexDeclaration9,
    vs: IDirect3DVertexShader9,
    ps: IDirect3DPixelShader9,
    /// Bound for a night, light or sphere map that is not there: it adds nothing.
    black: IDirect3DTexture9,
    /// x OR y of two bytes, at (x, y) of a 256 x 256 picture (`or_table`).
    or_table: IDirect3DTexture9,
}

/// The byte ORs as RGBA pixels: x OR y in every channel at column x, row y.
fn or_table() -> Vec<u8> {
    let mut px = Vec::with_capacity(256 * 256 * 4);
    for y in 0..256u32 {
        for x in 0..256u32 {
            px.extend_from_slice(&[(x | y) as u8; 4]);
        }
    }
    px
}

/// The sampler states of an addressing.
fn address_mode(a: TexAddress) -> u32 {
    match a {
        TexAddress::Wrap => D3DTADDRESS_WRAP.0 as u32,
        TexAddress::Mirror => D3DTADDRESS_MIRROR.0 as u32,
        TexAddress::Clamp | TexAddress::MirrorOnce => D3DTADDRESS_CLAMP.0 as u32,
    }
}

/// The scene's resources. Ids are the game's (slots it reuses), so they index sparse lists.
#[derive(Default)]
pub(super) struct SceneStore {
    meshes: Vec<Option<Mesh>>,
    textures: Vec<Option<IDirect3DTexture9>>,
    pipeline: Option<ScenePipeline>,
    /// The per-frame instance records, and the dynamic buffer they go up in (with the
    /// records it holds).
    records: Vec<f32>,
    instances: Option<(IDirect3DVertexBuffer9, usize)>,
    /// What every mesh and texture was made from.
    pub(super) copies: SceneCopies,
    lamps: Lamps,
}

/// The frame's lights and the grid naming them, as the textures the scene shader reads (made
/// again after a loss with everything else: they are uploaded every frame).
#[derive(Default)]
struct Lamps {
    /// The light list, four texels a row, and its rows.
    lights: Option<(IDirect3DTexture9, u32)>,
    /// The grid's texture, its width and height.
    grid: Option<(IDirect3DTexture9, u32, u32)>,
    /// What they hold, so that an unchanged list or grid is not uploaded again.
    last_lights: Vec<[f32; 4]>,
    last_grid: Vec<f32>,
    texels: Vec<f32>,
    /// The list as the shader reads it (`scene::pack_light`).
    packed: Vec<[f32; 4]>,
    warned: bool,
}

/// A dynamic float texture of `width` by `height`.
fn float_texture(
    device: &IDirect3DDevice9Ex,
    width: u32,
    height: u32,
    format: D3DFORMAT,
) -> Result<IDirect3DTexture9> {
    let mut texture = None;
    // SAFETY: the out-parameter is a live local.
    unsafe {
        device
            .CreateTexture(
                width,
                height,
                1,
                D3DUSAGE_DYNAMIC as u32,
                format,
                D3DPOOL_DEFAULT,
                &mut texture,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
    }
    some(texture, "float texture")
}

/// Writes `data` into `texture` row by row, `row` floats a row, `rows` rows (zero past the
/// data).
fn fill_float_texture(
    texture: &IDirect3DTexture9,
    row: usize,
    rows: usize,
    data: &[f32],
) -> Result<()> {
    let mut locked = D3DLOCKED_RECT::default();
    // SAFETY: the level is locked whole; each row is written within its pitch, and the
    // texture has `rows` rows of at least `row` floats.
    unsafe {
        texture
            .LockRect(0, &mut locked, std::ptr::null(), D3DLOCK_DISCARD as u32)
            .map_err(map_err)?;
        for y in 0..rows {
            let dst = std::slice::from_raw_parts_mut(
                (locked.pBits as *mut u8).add(y * locked.Pitch as usize) as *mut f32,
                row,
            );
            let src = data.get(y * row..).unwrap_or(&[]);
            let n = src.len().min(row);
            dst[..n].copy_from_slice(&src[..n]);
            dst[n..].fill(0.0);
        }
        texture.UnlockRect(0).map_err(map_err)?;
    }
    Ok(())
}

/// What the scene's meshes and textures were made from, kept in system memory as Direct3D
/// 9's managed pool keeps it (the default pool of a 9Ex device does not), so that a device
/// opened again after a loss gets them all back ([`D3d9Device::restore_scene`]).
#[derive(Default)]
pub(super) struct SceneCopies {
    meshes: Vec<Option<(Vec<f32>, Vec<u32>)>>,
    textures: Vec<Option<TextureCopy>>,
}

struct TextureCopy {
    width: u32,
    height: u32,
    format: TexFormat,
    /// The levels as given (a texture whose mips are made here keeps its first one only).
    levels: Vec<Vec<u8>>,
    make_mips: bool,
}

impl TextureCopy {
    fn of(t: &SceneTexture) -> Self {
        let kept = if t.wants_mips() { 1 } else { t.levels.len() };
        Self {
            width: t.width,
            height: t.height,
            format: t.format,
            levels: t.levels[..kept].to_vec(),
            make_mips: t.make_mips,
        }
    }

    fn texture(&self) -> SceneTexture<'_> {
        SceneTexture {
            width: self.width,
            height: self.height,
            format: self.format,
            levels: &self.levels,
            make_mips: self.make_mips,
        }
    }
}

fn shift<T>(list: &mut Vec<Option<T>>, from: usize, into: usize) {
    if let Some(value) = take(list, from) {
        put(list, into, value);
    } else {
        take(list, into);
    }
}

fn put<T>(list: &mut Vec<Option<T>>, id: usize, value: T) {
    if list.len() <= id {
        list.resize_with(id + 1, || None);
    }
    list[id] = Some(value);
}

fn take<T>(list: &mut [Option<T>], id: usize) -> Option<T> {
    list.get_mut(id).and_then(Option::take)
}

fn d3d_format(format: TexFormat) -> D3DFORMAT {
    match format {
        TexFormat::Rgba8 => D3DFMT_A8R8G8B8,
        TexFormat::Bc1 => D3DFMT_DXT1,
        TexFormat::Bc2 => D3DFMT_DXT3,
        TexFormat::Bc3 => D3DFMT_DXT5,
    }
}

/// Uploads every level through a system-memory texture into a default-pool one. The texture
/// must have been validated.
pub(super) fn make_texture(
    device: &IDirect3DDevice9Ex,
    t: &SceneTexture,
) -> Result<IDirect3DTexture9> {
    let made;
    let levels: &[Vec<u8>] = if t.wants_mips() {
        let mut all = vec![t.levels[0].clone()];
        all.extend(mip_chain(t.width, t.height, &t.levels[0]));
        made = all;
        &made
    } else {
        t.levels
    };
    let format = d3d_format(t.format);
    // SAFETY: out-parameters are live locals; each locked level is written row by row within
    // its pitch and its number of rows.
    unsafe {
        let mut staging = None;
        device
            .CreateTexture(
                t.width,
                t.height,
                levels.len() as u32,
                0,
                format,
                D3DPOOL_SYSTEMMEM,
                &mut staging,
                std::ptr::null_mut(),
            )
            .map_err(map_err)?;
        let staging = some(staging, "staging texture")?;
        for (i, level) in levels.iter().enumerate() {
            let (w, h) = crate::scene::level_size(t.width, t.height, i);
            let (row_bytes, rows) = t.format.rows(w, h);
            let mut locked = D3DLOCKED_RECT::default();
            staging
                .LockRect(i as u32, &mut locked, std::ptr::null(), 0)
                .map_err(map_err)?;
            for y in 0..rows {
                let dst = std::slice::from_raw_parts_mut(
                    (locked.pBits as *mut u8).add(y * locked.Pitch as usize),
                    row_bytes,
                );
                let src = &level[y * row_bytes..(y + 1) * row_bytes];
                if t.format == TexFormat::Rgba8 {
                    // memory order B, G, R, A
                    for (d, s) in dst.chunks_exact_mut(4).zip(src.chunks_exact(4)) {
                        d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
                    }
                } else {
                    dst.copy_from_slice(src);
                }
            }
            staging.UnlockRect(i as u32).map_err(map_err)?;
        }
        let mut texture = None;
        device
            .CreateTexture(
                t.width,
                t.height,
                levels.len() as u32,
                0,
                format,
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

fn build_pipeline(device: &IDirect3DDevice9Ex) -> Result<ScenePipeline> {
    let vs_code = shader::compile(
        shader::SCENE,
        "scene.hlsl",
        "vs_main",
        shader::Stage::Vertex,
    )?;
    let ps_code = shader::compile(shader::SCENE, "scene.hlsl", "ps_main", shader::Stage::Pixel)?;
    let element = |stream: u16, offset: u16, ty: D3DDECLTYPE, usage: D3DDECLUSAGE, index: u8| {
        D3DVERTEXELEMENT9 {
            Stream: stream,
            Offset: offset,
            Type: ty.0 as u8,
            Method: D3DDECLMETHOD_DEFAULT.0 as u8,
            Usage: usage.0 as u8,
            UsageIndex: index,
        }
    };
    let mut elements = vec![
        element(0, 0, D3DDECLTYPE_FLOAT3, D3DDECLUSAGE_POSITION, 0),
        element(0, 12, D3DDECLTYPE_FLOAT3, D3DDECLUSAGE_NORMAL, 0),
        element(0, 24, D3DDECLTYPE_FLOAT2, D3DDECLUSAGE_TEXCOORD, 0),
    ];
    // stream 1, one record per instance: the model's columns and the two parameter vectors
    for k in 0..6u8 {
        elements.push(element(
            1,
            k as u16 * 16,
            D3DDECLTYPE_FLOAT4,
            D3DDECLUSAGE_TEXCOORD,
            k + 1,
        ));
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
    let black = make_texture(
        device,
        &SceneTexture {
            width: 1,
            height: 1,
            format: TexFormat::Rgba8,
            levels: &[vec![0, 0, 0, 0]],
            make_mips: false,
        },
    )?;
    let or_table = make_texture(
        device,
        &SceneTexture {
            width: 256,
            height: 256,
            format: TexFormat::Rgba8,
            levels: &[or_table()],
            make_mips: false,
        },
    )?;
    // SAFETY: the bytecode and element arrays outlive the calls.
    unsafe {
        Ok(ScenePipeline {
            black,
            or_table,
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

/// The render states of a kind of batch.
fn kind_states(device: &IDirect3DDevice9Ex, kind: DrawKind) -> Result<()> {
    let flags = [
        ALPHA_CUT,
        f32::from(u8::from(kind == DrawKind::AlphaTest)),
        f32::from(u8::from(kind == DrawKind::SurfaceDepth)),
        0.0,
    ];
    // SAFETY: plain state setters; the constant array outlives the call.
    unsafe {
        device
            .SetRenderState(D3DRS_ALPHABLENDENABLE, u32::from(kind.blends()))
            .map_err(map_err)?;
        device
            .SetRenderState(D3DRS_ZWRITEENABLE, u32::from(kind.writes_depth()))
            .map_err(map_err)?;
        device
            .SetRenderState(
                D3DRS_COLORWRITEENABLE,
                if kind.writes_color() { 0xF } else { 0 },
            )
            .map_err(map_err)?;
        device
            .SetPixelShaderConstantF(17, flags.as_ptr(), 1)
            .map_err(map_err)?;
    }
    Ok(())
}

impl D3d9Device {
    /// Puts mesh `id` of the game's scene up (8 floats per vertex: position, normal, uv),
    /// replacing what had that id.
    pub fn set_scene_mesh(&mut self, id: usize, vertices: &[f32], indices: &[u32]) -> Result<()> {
        self.put_scene_mesh(id, vertices.to_vec(), indices.to_vec())
    }

    fn put_scene_mesh(&mut self, id: usize, vertices: Vec<f32>, indices: Vec<u32>) -> Result<()> {
        validate_mesh(&vertices, &indices, self.caps.index32)?;
        let mesh = make_mesh(&self.device, &vertices, &indices)?;
        put(&mut self.scene.meshes, id, mesh);
        put(&mut self.scene.copies.meshes, id, (vertices, indices));
        Ok(())
    }

    /// New vertices for mesh `id`, as many as it has (a mesh posed on the CPU).
    pub fn set_scene_mesh_vertices(&mut self, id: usize, vertices: &[f32]) -> Result<()> {
        let mesh = self
            .scene
            .meshes
            .get(id)
            .and_then(Option::as_ref)
            .ok_or_else(|| BackendError::InvalidArgument(format!("no mesh {id}")))?;
        if vertices.len() != mesh.vertices as usize * crate::backend::VERTEX_FLOATS {
            return Err(BackendError::InvalidArgument(format!(
                "mesh {id} has {} vertices",
                mesh.vertices
            )));
        }
        // SAFETY: the buffer was made for exactly this many vertices.
        unsafe { write_buffer(&mesh.vb, vertices) }?;
        if let Some((kept, _)) = self
            .scene
            .copies
            .meshes
            .get_mut(id)
            .and_then(Option::as_mut)
        {
            kept.copy_from_slice(vertices);
        }
        Ok(())
    }

    pub fn free_scene_mesh(&mut self, id: usize) {
        take(&mut self.scene.meshes, id);
        take(&mut self.scene.copies.meshes, id);
    }

    /// Mesh `from` is now mesh `into` (the game moved it to another slot).
    pub fn move_scene_mesh(&mut self, from: usize, into: usize) {
        shift(&mut self.scene.meshes, from, into);
        shift(&mut self.scene.copies.meshes, from, into);
    }

    /// Puts texture `id` of the game's scene up, replacing what had that id.
    pub fn set_scene_texture(&mut self, id: usize, texture: &SceneTexture) -> Result<()> {
        texture.validate(self.caps.max_texture_size, self.caps.non_pow2_textures)?;
        if !self.scene_caps.allows(texture.format) {
            return Err(BackendError::Unsupported(format!(
                "{:?} textures on this device",
                texture.format
            )));
        }
        let t = make_texture(&self.device, texture)?;
        put(&mut self.scene.textures, id, t);
        put(
            &mut self.scene.copies.textures,
            id,
            TextureCopy::of(texture),
        );
        Ok(())
    }

    pub fn free_scene_texture(&mut self, id: usize) {
        take(&mut self.scene.textures, id);
        take(&mut self.scene.copies.textures, id);
    }

    /// Texture `from` is now texture `into`.
    pub fn move_scene_texture(&mut self, from: usize, into: usize) {
        shift(&mut self.scene.textures, from, into);
        shift(&mut self.scene.copies.textures, from, into);
    }

    /// Lets every scene mesh and texture go (the game starts a new scene, whose ids begin
    /// again).
    pub fn clear_scene(&mut self) {
        self.scene.meshes.clear();
        self.scene.textures.clear();
        self.scene.copies = SceneCopies::default();
    }

    /// Puts `copies` (a lost device's) up on this one under the same ids. One that cannot be
    /// made is left out and the first such failure returned, after the others went up.
    pub(super) fn restore_scene(&mut self, copies: SceneCopies) -> Result<()> {
        let mut done = Ok(());
        for (id, kept) in copies.meshes.into_iter().enumerate() {
            if let Some((vertices, indices)) = kept {
                done = done.and(self.put_scene_mesh(id, vertices, indices));
            }
        }
        for (id, kept) in copies.textures.into_iter().enumerate() {
            if let Some(kept) = kept {
                done = done.and(self.set_scene_texture(id, &kept.texture()));
            }
        }
        done
    }

    /// Whether mesh `id` and texture `id` are up (for the game's own checks and tests).
    pub fn has_scene_mesh(&self, id: usize) -> bool {
        self.scene.meshes.get(id).is_some_and(Option::is_some)
    }

    pub fn has_scene_texture(&self, id: usize) -> bool {
        self.scene.textures.get(id).is_some_and(Option::is_some)
    }

    pub(super) fn scene_texture(&self, id: usize) -> Option<&IDirect3DTexture9> {
        self.scene.textures.get(id).and_then(Option::as_ref)
    }

    /// Draws `frame` into the frame begun with [`crate::RenderBackend::begin_frame`] (whose
    /// clear colour is the sky's): the main batches, then the cab's. `material` gives each
    /// batch's material; a batch whose mesh or material is missing is skipped, a missing
    /// texture draws white. Depth is cleared for the scene and again after it, so the
    /// backend's own batches (the UI) can follow in the same frame.
    pub fn draw_scene<'m>(
        &mut self,
        frame: &SceneFrame,
        material: impl Fn(usize) -> Option<SceneMaterial<'m>>,
    ) -> Result<()> {
        let Some(target) = self.frame.clone() else {
            return Err(BackendError::InvalidArgument(
                "scene outside a frame".into(),
            ));
        };
        frame.validate()?;
        if self.scene.pipeline.is_none() {
            self.scene.pipeline = Some(build_pipeline(&self.device)?);
        }
        let drawn = self.draw_scene_inner(frame, &material, target.width, target.height);
        // Back to the backend's own states whatever happened, with depth as it expects it.
        // SAFETY: plain calls on a live device.
        let restored = unsafe {
            self.device
                .SetStreamSourceFreq(0, 1)
                .and_then(|_| self.device.SetStreamSourceFreq(1, 1))
                .and_then(|_| {
                    self.device
                        .SetStreamSource(1, None::<&IDirect3DVertexBuffer9>, 0, 0)
                })
                .and_then(|_| self.device.SetRenderState(D3DRS_COLORWRITEENABLE, 0xF))
                .and_then(|_| {
                    self.device
                        .SetRenderState(D3DRS_SEPARATEALPHABLENDENABLE, 0)
                })
                .and_then(|_| self.device.SetRenderState(D3DRS_DEPTHBIAS, 0))
                .and_then(|_| self.device.SetRenderState(D3DRS_SLOPESCALEDEPTHBIAS, 0))
                .and_then(|_| {
                    self.device
                        .Clear(0, std::ptr::null(), D3DCLEAR_ZBUFFER as u32, 0, 1.0, 0)
                })
                .map_err(map_err)
        };
        drawn?;
        restored?;
        self.set_frame_states()
    }

    /// Uploads the frame's lights and grid where they changed and binds them; returns the
    /// shader's constants c20..c22 for them.
    fn bind_lamps(&mut self, lamps: &SceneLamps) -> Result<[[f32; 4]; 3]> {
        // (no grid: one of no cells, which no point is on)
        const NO_GRID: [f32; 4] = [0.0; 4];
        let none = [NO_GRID, [0.0; 4], [0.0; 4]];
        self.bind_lamp_counts(0, 0)?;
        let (max_w, max_h) = self.scene_caps.max_texture;
        if !self.scene_caps.lamps || lamps.count() == 0 {
            if !self.scene_caps.lamps && lamps.count() > 0 && !self.scene.lamps.warned {
                log::warn!("d3d9 scene: no float textures here; the lamps are not drawn");
                self.scene.lamps.warned = true;
            }
            return Ok(none);
        }
        let device = self.device.clone();
        let store = &mut self.scene.lamps;
        // the light list: the first as many as the texture can have rows
        let kept = lamps.count().min(max_h as usize);
        if store.lights.as_ref().is_none_or(|l| (l.1 as usize) < kept) {
            let rows = (kept.next_power_of_two().max(64) as u32).min(max_h);
            store.lights = Some((float_texture(&device, 4, rows, D3DFMT_A32B32G32R32F)?, rows));
            store.last_lights.clear();
        }
        let (lights, rows) = store.lights.clone().expect("made above");
        let list = &lamps.lights[..kept * 4];
        if store.last_lights != list {
            store.packed.clear();
            store
                .packed
                .extend(list.chunks_exact(4).flat_map(crate::scene::pack_light));
            fill_float_texture(&lights, 16, rows as usize, store.packed.as_flattened())?;
            store.last_lights.clear();
            store.last_lights.extend_from_slice(list);
        }
        // the grid: one texel a place, a row of cells a row
        let mut grid_info = [0.0; 4];
        if let Some((side, places)) = lamps.grid_shape() {
            let (w, h) = ((side * places) as u32, side as u32);
            if w > max_w || h > max_h {
                if !store.warned {
                    log::warn!("d3d9 scene: a lamp grid of {w} x {h} is larger than textures here; the map's lamps are not drawn");
                    store.warned = true;
                }
            } else {
                if store.grid.as_ref().is_none_or(|g| (g.1, g.2) != (w, h)) {
                    store.grid = Some((float_texture(&device, w, h, D3DFMT_R32F)?, w, h));
                    store.last_grid.clear();
                }
                lamps.grid_texels(kept, &mut store.texels);
                let grid = &store.grid.as_ref().expect("made above").0;
                if store.last_grid != store.texels {
                    fill_float_texture(grid, w as usize, h as usize, &store.texels)?;
                    std::mem::swap(&mut store.last_grid, &mut store.texels);
                }
                grid_info = [0.0, 1.0 / w as f32, 1.0 / h as f32, places as f32];
            }
        }
        grid_info[0] = 1.0 / rows as f32;
        let grid = store
            .grid
            .as_ref()
            .map(|g| &g.0)
            .filter(|_| grid_info[3] > 0.0);
        // SAFETY: plain calls on a live device; the textures belong to it.
        unsafe {
            device.SetTexture(LIGHTS_STAGE, &lights).map_err(map_err)?;
            match grid {
                Some(g) => device.SetTexture(GRID_STAGE, g),
                None => device.SetTexture(GRID_STAGE, None::<&IDirect3DBaseTexture9>),
            }
            .map_err(map_err)?;
            for stage in [LIGHTS_STAGE, GRID_STAGE] {
                for (state, value) in [
                    (D3DSAMP_MINFILTER, D3DTEXF_POINT.0 as u32),
                    (D3DSAMP_MAGFILTER, D3DTEXF_POINT.0 as u32),
                    (D3DSAMP_MIPFILTER, D3DTEXF_NONE.0 as u32),
                    (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                    (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
                ] {
                    device
                        .SetSamplerState(stage, state, value)
                        .map_err(map_err)?;
                }
            }
        }
        let places = grid_info[3] as i32;
        self.bind_lamp_counts(kept, places)?;
        // the cell of a point is floor(xy / side - corner / side)
        let [x, y, side, cells] = lamps.place;
        let place = if places > 0 && side > 0.0 {
            [1.0 / side, -x / side, -y / side, cells]
        } else {
            NO_GRID
        };
        Ok([place, grid_info, [kept as f32, 0.0, 0.0, 0.0]])
    }

    /// The table of byte ORs, read texel by texel.
    fn bind_or_table(&self) -> Result<()> {
        let table = &self.scene.pipeline.as_ref().expect("built before").or_table;
        // SAFETY: plain calls on a live device; the texture belongs to it.
        unsafe {
            self.device.SetTexture(OR_STAGE, table).map_err(map_err)?;
            for (state, value) in [
                (D3DSAMP_MINFILTER, D3DTEXF_POINT.0 as u32),
                (D3DSAMP_MAGFILTER, D3DTEXF_POINT.0 as u32),
                (D3DSAMP_MIPFILTER, D3DTEXF_NONE.0 as u32),
                (D3DSAMP_ADDRESSU, D3DTADDRESS_CLAMP.0 as u32),
                (D3DSAMP_ADDRESSV, D3DTADDRESS_CLAMP.0 as u32),
            ] {
                self.device
                    .SetSamplerState(OR_STAGE, state, value)
                    .map_err(map_err)?;
            }
        }
        Ok(())
    }

    /// The loop counts of the lamps: the lights in the list (the vertex stage's, for the
    /// interior lamps) and the places of a grid cell (the pixel stage's loop count).
    fn bind_lamp_counts(&self, lights: usize, places: i32) -> Result<()> {
        let vs = [lights as f32, 0.0, 0.0, 0.0];
        let ps = [places.max(0), 0, 1, 0];
        // SAFETY: plain calls on a live device; the arrays outlive them.
        unsafe {
            self.device
                .SetVertexShaderConstantF(13, vs.as_ptr(), 1)
                .and_then(|_| self.device.SetPixelShaderConstantI(0, ps.as_ptr(), 1))
                .map_err(map_err)
        }
    }

    fn draw_scene_inner<'m>(
        &mut self,
        frame: &SceneFrame,
        material: &impl Fn(usize) -> Option<SceneMaterial<'m>>,
        width: u32,
        height: u32,
    ) -> Result<()> {
        let lamps = self.bind_lamps(&frame.lamps)?;
        self.bind_or_table()?;
        self.scene.records.clear();
        frame.instance_records(&mut self.scene.records);
        let records = frame.list.len();
        let too_many = || BackendError::InvalidArgument(format!("{records} instances"));
        let bytes = u32::try_from(records)
            .ok()
            .and_then(|n| n.checked_mul(INSTANCE_STRIDE))
            .ok_or_else(too_many)?;
        if records > 0 && self.scene.instances.as_ref().is_none_or(|i| i.1 < records) {
            let capacity = records.next_power_of_two().max(1024);
            let capacity_bytes = u32::try_from(capacity)
                .ok()
                .and_then(|n| n.checked_mul(INSTANCE_STRIDE))
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
            self.scene.instances = Some((some(vb, "instance buffer")?, capacity));
        }
        let d = &self.device;
        let pipeline = self.scene.pipeline.as_ref().expect("built above");
        let light = &frame.light;
        let lights = [
            light.cam_pos,
            light.sun_dir,
            light.sun_color,
            light.ambient,
            light.fog,
            light.sky_color,
        ];
        let pixel = [-1.0 / width as f32, 1.0 / height as f32, 0.0, 0.0];
        let (bias, slope) = surface_depth_bias(frame.surface_bias);
        // SAFETY: plain calls on a live device; constant arrays outlive the calls; the
        // instance buffer holds `records` records, locked with DISCARD and written whole.
        unsafe {
            if let (Some((vb, _)), true) = (&self.scene.instances, records > 0) {
                let mut p = std::ptr::null_mut();
                vb.Lock(0, bytes, &mut p, D3DLOCK_DISCARD as u32)
                    .map_err(map_err)?;
                std::ptr::copy_nonoverlapping(
                    self.scene.records.as_ptr() as *const u8,
                    p as *mut u8,
                    bytes as usize,
                );
                vb.Unlock().map_err(map_err)?;
            }
            // reversed depth: cleared to the far plane, 0, and nearer is greater
            d.Clear(0, std::ptr::null(), D3DCLEAR_ZBUFFER as u32, 0, 0.0, 0)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_ZENABLE, D3DZB_TRUE.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_ZFUNC, D3DCMP_GREATEREQUAL.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_ALPHATESTENABLE, 0)
                .map_err(map_err)?;
            // the picture's alpha as the default renderer blends it: one, over what is there
            d.SetRenderState(D3DRS_SRCBLEND, D3DBLEND_SRCALPHA.0 as u32)
                .map_err(map_err)?;
            d.SetRenderState(D3DRS_DESTBLEND, D3DBLEND_INVSRCALPHA.0 as u32)
                .map_err(map_err)?;
            if self.scene_caps.separate_alpha {
                d.SetRenderState(D3DRS_SEPARATEALPHABLENDENABLE, 1)
                    .map_err(map_err)?;
                d.SetRenderState(D3DRS_SRCBLENDALPHA, D3DBLEND_ONE.0 as u32)
                    .map_err(map_err)?;
                d.SetRenderState(D3DRS_DESTBLENDALPHA, D3DBLEND_INVSRCALPHA.0 as u32)
                    .map_err(map_err)?;
            }
            d.SetVertexDeclaration(&pipeline.decl).map_err(map_err)?;
            d.SetVertexShader(&pipeline.vs).map_err(map_err)?;
            d.SetPixelShader(&pipeline.ps).map_err(map_err)?;
            for stage in 0..SAMPLERS {
                for (state, value) in [
                    (D3DSAMP_MINFILTER, D3DTEXF_LINEAR.0 as u32),
                    (D3DSAMP_MAGFILTER, D3DTEXF_LINEAR.0 as u32),
                    (D3DSAMP_MIPFILTER, D3DTEXF_LINEAR.0 as u32),
                ] {
                    d.SetSamplerState(stage, state, value).map_err(map_err)?;
                }
            }
            let vs_light = [
                light.sun_dir,
                light.sun_color,
                light.sky_color,
                light.cam_right,
                light.cam_up,
            ];
            d.SetVertexShaderConstantF(6, vs_light.as_ptr().cast(), vs_light.len() as u32)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(0, frame.view_proj.as_ptr().cast(), 4)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(4, light.cam_pos.as_ptr(), 1)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(5, pixel.as_ptr(), 1)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(11, lights.as_ptr().cast(), lights.len() as u32)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(20, lamps.as_ptr().cast(), lamps.len() as u32)
                .map_err(map_err)?;
            if let Some((vb, _)) = &self.scene.instances {
                d.SetStreamSource(1, vb, 0, INSTANCE_STRIDE)
                    .map_err(map_err)?;
            }
        }
        let mut current: (Option<DrawKind>, Option<bool>, Option<bool>, Option<usize>) =
            (None, None, None, None);
        let mut skipped = 0usize;
        for b in frame.main.iter().chain(frame.cab) {
            if b.count == 0 || b.instances.is_empty() {
                continue;
            }
            let (Some(Some(mesh)), Some(m)) = (self.scene.meshes.get(b.mesh), material(b.material))
            else {
                skipped += 1;
                continue;
            };
            check_batch(b, mesh, &m)?;
            // SAFETY: as above; the buffers and textures belong to this device.
            unsafe {
                if current.0 != Some(b.kind) {
                    kind_states(d, b.kind)?;
                    current.0 = Some(b.kind);
                }
                if current.1 != Some(b.cull) {
                    let mode = if b.cull { D3DCULL_CCW } else { D3DCULL_NONE };
                    d.SetRenderState(D3DRS_CULLMODE, mode.0 as u32)
                        .map_err(map_err)?;
                    current.1 = Some(b.cull);
                }
                if current.2 != Some(b.surface) {
                    let (c, s) = if b.surface { (bias, slope) } else { (0.0, 0.0) };
                    d.SetRenderState(D3DRS_DEPTHBIAS, c.to_bits())
                        .map_err(map_err)?;
                    d.SetRenderState(D3DRS_SLOPESCALEDEPTHBIAS, s.to_bits())
                        .map_err(map_err)?;
                    current.2 = Some(b.surface);
                }
                if current.3 != Some(b.material) {
                    self.bind_material(&m)?;
                    current.3 = Some(b.material);
                }
                let n = b.instances.end - b.instances.start;
                d.SetStreamSource(0, &mesh.vb, 0, 32).map_err(map_err)?;
                d.SetStreamSourceFreq(0, D3DSTREAMSOURCE_INDEXEDDATA | n)
                    .map_err(map_err)?;
                if let Some((vb, _)) = &self.scene.instances {
                    // (no overflow: the run lies in the list, whose bytes were checked above)
                    d.SetStreamSource(1, vb, b.instances.start * INSTANCE_STRIDE, INSTANCE_STRIDE)
                        .map_err(map_err)?;
                }
                d.SetStreamSourceFreq(1, D3DSTREAMSOURCE_INSTANCEDATA | 1)
                    .map_err(map_err)?;
                d.SetIndices(&mesh.ib).map_err(map_err)?;
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
        }
        if skipped > 0 {
            log::debug!("d3d9 scene: {skipped} batches without their mesh or material");
        }
        Ok(())
    }
}

impl D3d9Device {
    /// The material's uniform, maps, addressing and the constants worked out from them.
    fn bind_material(&self, m: &SceneMaterial) -> Result<()> {
        let d = &self.device;
        let pipeline = self.scene.pipeline.as_ref().expect("built before");
        let found = |id: Option<usize>| {
            id.and_then(|i| self.scene.textures.get(i))
                .and_then(Option::as_ref)
        };
        let white = &self.pipeline.white;
        let black = &pipeline.black;
        let env = found(m.envmap);
        let bump = found(m.bump);
        let u = m.uniform;
        // (a sphere map that is not there reflects nothing; the bits and the border colour
        // unpacked here, not per pixel)
        let [has_mask, declared] = crate::scene::env_bits(u[3][3]);
        let env_on = [env.is_some() as u32 as f32, has_mask, declared, 0.0];
        let border = crate::scene::border_rgba(u[8]);
        let mut bump_texel = [0.0f32; 4];
        if let Some(t) = bump {
            let mut desc = D3DSURFACE_DESC::default();
            // SAFETY: the out-parameter is a live local; level 0 exists.
            unsafe { t.GetLevelDesc(0, &mut desc) }.map_err(map_err)?;
            bump_texel = [
                1.0 / desc.Width.max(1) as f32,
                1.0 / desc.Height.max(1) as f32,
                0.0,
                0.0,
            ];
        }
        let terrain = u[2][0] > 0.5;
        let address = address_mode(m.address);
        // a terrain's transmap and night map cover one tile: clamped at its edge
        let tile = if terrain {
            D3DTADDRESS_CLAMP.0 as u32
        } else {
            address
        };
        let maps = [
            (found(m.texture).unwrap_or(white), address),
            (found(m.transmap).unwrap_or(white), tile),
            (found(m.nightmap).unwrap_or(black), tile),
            (found(m.lightmap).unwrap_or(black), address),
            (env.unwrap_or(black), address),
            (found(m.env_mask).unwrap_or(white), address),
            (bump.unwrap_or(white), address),
        ];
        let vs = [u[5], u[1]];
        let ps = [bump_texel, env_on];
        // SAFETY: plain calls on a live device; the constant arrays outlive them; the
        // textures belong to this device.
        unsafe {
            d.SetPixelShaderConstantF(0, u.as_ptr().cast(), MATERIAL_VECTORS as u32)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(18, ps.as_ptr().cast(), ps.len() as u32)
                .map_err(map_err)?;
            d.SetPixelShaderConstantF(BORDER_REGISTER, border.as_ptr(), 1)
                .map_err(map_err)?;
            d.SetVertexShaderConstantF(11, vs.as_ptr().cast(), vs.len() as u32)
                .map_err(map_err)?;
            for (stage, (texture, address)) in maps.iter().enumerate() {
                let stage = stage as u32;
                d.SetTexture(stage, *texture).map_err(map_err)?;
                d.SetSamplerState(stage, D3DSAMP_ADDRESSU, *address)
                    .map_err(map_err)?;
                d.SetSamplerState(stage, D3DSAMP_ADDRESSV, *address)
                    .map_err(map_err)?;
            }
        }
        Ok(())
    }
}

/// The batch's range lies in its mesh and its material has the whole uniform.
fn check_batch(b: &SceneBatch, mesh: &Mesh, m: &SceneMaterial) -> Result<()> {
    let end = b.first.checked_add(b.count);
    if !b.count.is_multiple_of(3) || end.is_none_or(|e| e > mesh.triangles * 3) {
        return Err(BackendError::InvalidArgument(format!(
            "batch indices {}..{} outside mesh {} of {} indices",
            b.first,
            b.first as u64 + b.count as u64,
            b.mesh,
            mesh.triangles * 3
        )));
    }
    if m.uniform.len() != MATERIAL_VECTORS {
        return Err(BackendError::InvalidArgument(format!(
            "material {} has {} uniform vectors, not {MATERIAL_VECTORS}",
            b.material,
            m.uniform.len()
        )));
    }
    Ok(())
}
