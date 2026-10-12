//! Materials and textures: what a material samples and how, its uniform and the GPU
//! texture slots.

use super::*;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct MaterialUniform {
    pub(crate) color: [f32; 4],
    pub(crate) params: [f32; 4],
    pub(crate) extra: [f32; 4],
    pub(crate) params2: [f32; 4],
    /// rgb: emissive colour; w: an explicitly identified transparent glass layer
    pub(crate) emissive: [f32; 4],
    pub(crate) specular: [f32; 4],
    /// x: `[matl_bumpmap]` factor, y: has a bump map, z/w: noZwrite/noZcheck
    pub(crate) bump: [f32; 4],
    /// The PBR maps beside the diffuse texture (`Scene::pbr_maps`): x has a normal map,
    /// y an occlusion, z a roughness, w a metalness channel.
    pub(crate) pbr: [f32; 4],
    /// x: a screen (`MaterialExtra::screen`); y: 1 `[matl_texadress_border]`, 2
    /// `[matl_texadress_mirroronce]`; z the border colour's rgb packed as r * 65536 + g * 256 + b (bytes), w its alpha.
    pub(crate) flags: [f32; 4],
    /// rgb: the D3D material's ambient colour, which takes the ambient light (C); w: 1 for
    /// a texture that is a season's snow picture (no snow laid over it), 2 the map's water,
    /// -1 a slot without a texture
    pub(crate) ambient: [f32; 4],
    /// `MaterialExtra::sway`: x 1 for foliage the wind moves, y its pivot's and z its top's
    /// height (mesh units), w how much it gives to the wind
    pub(crate) sway: [f32; 4],
}

/// The maps of a PBR set found beside a diffuse texture (`foo_n.png` and the rest, see
/// `omsi_texture::pbr`): a tangent-space normal map, and occlusion / roughness / metalness
/// packed into the red, green and blue of one texture.
#[derive(Debug, Clone, Copy)]
pub struct PbrMaps {
    pub normal: Option<TextureId>,
    pub orm: Option<TextureId>,
    /// x normal, y occlusion, z roughness, w metalness (1 = present)
    pub flags: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AlphaMode {
    Opaque,
    Test,
    Blend,
}

/// How a material's textures read outside [0, 1]: Omsi.exe sets the material's
/// `[matl_texadress_*]` mode as ADDRESSU/ADDRESSV of all eight sampler stages (0x7fff70).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum TexAddressing {
    /// Repeating, Direct3D's default.
    #[default]
    Wrap,
    /// `[matl_texadress_mirror]`: every other repeat mirrored.
    Mirror,
    /// `[matl_texadress_clamp]`, and `[matl_texadress_border]` (whose colour the shader
    /// puts outside, `MaterialExtra::border`).
    Clamp,
    /// `[matl_texadress_mirroronce]`: mirrored once about 0, then clamped (the shader takes
    /// the coordinates' absolute value under the clamping sampler).
    MirrorOnce,
}

/// An RGBA picture borrowed for an upload.
pub(crate) struct RgbaRef<'a> {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) rgba: &'a [u8],
}

pub struct GpuTexture {
    #[allow(dead_code)]
    pub(crate) texture: wgpu::Texture,
    pub(crate) view: wgpu::TextureView,
    pub(crate) size: (u32, u32),
    /// Bytes the texture takes on the GPU, all levels (0 for a freed slot's placeholder).
    pub(crate) bytes: u64,
    /// Which texture this is (a slot is reused and a texture replaced): part of the
    /// key of a material bind group made this frame (see `Scene::bind_groups`).
    pub(crate) gen: u64,
}

pub(crate) static TEXTURE_GEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

pub(crate) fn next_gen() -> u64 {
    TEXTURE_GEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl GpuTexture {
    /// A slot showing `view` (a picture the renderer draws: the one behind the rain on the
    /// glass), with `texture` a small stand-in the slot's bookkeeping asks about.
    pub(crate) fn showing(
        texture: wgpu::Texture,
        view: wgpu::TextureView,
        size: (u32, u32),
    ) -> GpuTexture {
        GpuTexture {
            texture,
            view,
            size,
            bytes: 0,
            gen: next_gen(),
        }
    }

    pub(crate) fn new(texture: wgpu::Texture, size: (u32, u32), bytes: u64) -> GpuTexture {
        let view = texture.create_view(&Default::default());
        GpuTexture {
            texture,
            view,
            size,
            bytes,
            gen: next_gen(),
        }
    }
}

/// What a material's bind group is made of: its textures (slot and generation), the
/// sampler and the uniform values.
#[derive(Clone, PartialEq, Eq, Hash)]
pub(crate) struct BindKey {
    pub(crate) textures: [(usize, u64); 7],
    pub(crate) address: TexAddressing,
    pub(crate) uniform: [u32; 44],
}

/// Bytes of a texture of `format` with `levels` mip levels.
pub(crate) fn texture_bytes(format: wgpu::TextureFormat, w: u32, h: u32, levels: u32) -> u64 {
    let (bw, bh) = format.block_dimensions();
    let block = format.block_copy_size(None).unwrap_or(4) as u64;
    (0..levels)
        .map(|l| ((w >> l).max(1).div_ceil(bw) * (h >> l).max(1).div_ceil(bh)) as u64 * block)
        .sum()
}

pub struct Material {
    pub texture: Option<TextureId>,
    pub alpha: AlphaMode,
    pub color: [f32; 4],
    pub unlit: bool,
    /// `[matl_noZwrite]`: a blended surface (glass, rain film, dirt) that must not write
    /// depth, or everything blended behind it is thrown away - which is what punched holes
    /// into the world seen through a window or a mirror.
    pub no_z_write: bool,
    /// See [`MaterialExtra::writes_depth`].
    pub writes_depth: bool,
    /// `[matl_noZcheck]`: a decal drawn over the surface it lies on - blended, without
    /// depth write, with the surfaces' depth bias (see the blended draw items).
    pub no_z_check: bool,
    /// `[matl_Zbias]`: a positive bias pulls a decal in front of the coplanar surface
    /// under it (drawn with the depth bias of the road surfaces).
    pub z_bias: i32,
    pub nightmap: Option<TextureId>,
    pub lightmap: Option<TextureId>,
    pub envmap: Option<(TextureId, f32)>,
    /// `[matl_envmap_mask]`: the reflection mask is this texture's alpha instead of the
    /// diffuse texture's.
    pub env_mask: Option<TextureId>,
    /// `[matl_bumpmap]` height map and factor.
    pub bump: Option<(TextureId, f32)>,
    pub emissive: [f32; 3],
    /// `[matl_transmap]` (texture, its alpha channel is used).
    pub transmap: Option<(TextureId, bool)>,
    /// Its textures' addressing (`[matl_texadress_*]`).
    pub(crate) address: TexAddressing,
    /// Keep the exact material parameters so a CTC texture swap can change only the diffuse
    /// map without losing map lighting, moisture, screen, or other renderer flags.
    pub(crate) uniform: MaterialUniform,
    pub(crate) buf: wgpu::Buffer,
    pub(crate) bind_group: wgpu::BindGroup,
    /// Materials that draw alike (same textures, sampler and values) share this number.
    pub(crate) look: u32,
}

impl Material {
    /// Whether the material samples texture `id`.
    pub fn uses_texture(&self, id: TextureId) -> bool {
        self.texture == Some(id)
            || self.nightmap == Some(id)
            || self.lightmap == Some(id)
            || self.envmap.map(|e| e.0) == Some(id)
            || self.transmap.map(|t| t.0) == Some(id)
            || self.env_mask == Some(id)
            || self.bump.map(|b| b.0) == Some(id)
    }

    /// `[matl_transmap]` was given, its file there or not (the shader's
    /// `has_transmap_declared`).
    pub fn transmap_declared(&self) -> bool {
        (self.uniform.params2[3] + 0.5) as u32 & 2 != 0
    }
}

/// The material manager's settings beyond the maps of `add_material_all`: depth handling,
/// the reflection mask and the o3d material's specular term.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MaterialExtra {
    /// `[matl_envmap_mask]`
    pub env_mask: Option<TextureId>,
    /// `[matl_noZwrite]`
    pub no_z_write: bool,
    /// A slot `no_z_write` marks as a see-through layer (a pane, a dirt film, a sticker
    /// on a window - for the glass shading and the shadow map) that the model.cfg does not
    /// give `[matl_noZwrite]`: Omsi.exe draws it with its depth written (0x7fd6c4 sets
    /// ZWRITEENABLE from that flag alone), and so is it drawn here. Stacked panes of a
    /// door or a window then hide each other in model order as in the original, instead
    /// of all being blended over each other whichever is in front (#211).
    pub writes_depth: bool,
    /// `[matl_noZcheck]`
    pub no_z_check: bool,
    /// `[matl_Zbias]`
    pub z_bias: i32,
    /// The D3D material's ambient colour, its share of the ambient light (C); None: the
    /// diffuse colour's.
    pub ambient: Option<[f32; 3]>,
    /// Specular colour (rgb) and power (w) of the D3D material; black = no highlight.
    pub specular: [f32; 4],
    /// `[matl_bumpmap]`: a height map (in its alpha, `Image::bump_height_map`) whose slope
    /// shifts the `[matl_envmap]` lookup, times the factor.
    pub bump: Option<(TextureId, f32)>,
    /// A named transparent window layer. This is separate from envmap/transmap because
    /// stock and add-on buses often use a plain alpha-blended window texture.
    pub glass: bool,
    /// The night map is switched by something other than the time of day - a `[matl_item]`'s
    /// variable, a vehicle mesh's `[visible]`: it glows by day as well (warning lamps,
    /// dashboard displays), not only at night.
    pub night_switched: bool,
    /// A display's text (`[useTextTexture]`): in the enhanced picture it glows a little
    /// by itself, as a lit matrix does, instead of taking only the light that reaches it
    /// under the bus's front overhang, where it was hardly readable by day.
    pub display: bool,
    /// A screen the bus draws itself - a `[useTextTexture]` or `[useScriptTexture]` slot:
    /// the IBIS, the matrix displays, the dashboard's LCDs. The enhanced picture's glow
    /// and FXAA leave it alone (see `MASK_FORMAT`): FXAA took half the contrast out of
    /// their letters and they read as blurred.
    pub screen: bool,
    /// An LED matrix - a display whose lit dots are the `\S:n` script texture's
    /// (`[matl_transmap]`), the Krueger and K++ destination panels: the dots are the
    /// panel's own light, so the enhanced picture lets them burn in HDR and blooms them
    /// (the glow's source keeps them, where the other screens are left out of it -
    /// something no Direct3D 9 without shaders of its own could do). `MASK_FORMAT`'s g.
    /// (Only a panel whose `[matl_lightmap]` is white all over: a flipdot carries the same
    /// mask, but its light map is a picture of the lamps over it, and it does not glow.)
    pub led: bool,
    /// The film of water on a window (`[alphascale] Rain_Window_…`): drawn as drops that sit,
    /// gather and run down the glass instead of the texture sliding down as a whole.
    pub rain_film: bool,
    /// The map's water (`texture/water.tga`): Enhanced draws it as water - a smooth surface
    /// mirroring the sky more the flatter it is seen, rippled by small waves.
    pub water: bool,
    /// `[nomaplighting]`: the map's lamps (`[maplight]`) do not light it - a street lamp
    /// is not lit by its own light.
    pub no_map_lights: bool,
    /// A `[tree]`'s leaf cards: the vanilla picture leaves the map's lamps off them, as
    /// OMSI 2 shows a tree standing right under a street lamp dark; Vanilla+ and Enhanced
    /// still light them.
    pub tree: bool,
    /// 1 when the texture's `.cfg` sidecar carries `[moisture]`/`[puddles]`: the road of a
    /// junction or crossing object gets wet and collects puddles like a spline's.
    pub moisture: f32,
    /// `[matl_transmap]` was given, whether or not its file is there: Omsi.exe raises the
    /// material's transmap flag before it reads the name (0x7fbbf4), and with it the
    /// `[matl_envmap]` reflection goes by the texture's alpha instead of the factor.
    pub transmap_declared: bool,
    /// `[matl_texadress_border]`: its colour (RGBA, 0..1). Where the (scrolled) texture
    /// coordinates leave [0, 1] the diffuse texture reads this colour instead of its edge,
    /// as Direct3D's border addressing does: a roller blind's band that has scrolled away
    /// vanishes in a transparent border.
    pub border: Option<[f32; 4]>,
    /// An opaque, sphere-mapped part of a vehicle that is not its body (a handrail, a
    /// bumper, a wheel trim): the enhanced picture may make it metal by its `[matl_envmap]`
    /// factor alone, as the vanilla one shows the sphere map on it - chrome read as a
    /// faint clear coat there. A body needs a mask of its own for that (a Golf's bonnet).
    pub metal_ok: bool,
    /// Windy trees: foliage the wind moves - the height (mesh units) of the pivot where the
    /// crown leaves the trunk (nothing below it moves), of the crown's top, and how much the
    /// tree gives to the wind (1 a broadleaf). `None`: not foliage.
    pub sway: Option<[f32; 3]>,
}

/// The textures a material's bind group samples.
#[derive(Clone, Copy)]
pub(crate) struct MaterialMaps {
    pub(crate) texture: Option<TextureId>,
    pub(crate) transmap: Option<(TextureId, bool)>,
    pub(crate) nightmap: Option<TextureId>,
    pub(crate) lightmap: Option<TextureId>,
    pub(crate) envmap: Option<(TextureId, f32)>,
    pub(crate) env_mask: Option<TextureId>,
    pub(crate) bump: Option<(TextureId, f32)>,
    pub(crate) pbr: Option<PbrMaps>,
}
