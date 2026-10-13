//! The light model the game describes: the sun, sky and exposure settings (`Lighting`), point
//! lights, coronas and smoke particles, and their records on the GPU.

use glam::{DVec3, Vec3};

/// The illuminance a light's core gives (maplight colour 1, in the sky model's units:
/// 55 lux, so the street under a lamp gets its 15-25 lux).
pub(crate) const LAMP_E: f32 = 0.0055;

/// Illuminance of a bus saloon's lamps on the seats and the floor (300 lux).
pub(crate) const CABIN_E: f32 = 0.03;

/// A lit window's radiance at night.
pub(crate) const WINDOW_RADIANCE: f32 = 0.0022;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuPointLight {
    pub(crate) pos: [f32; 4],
    pub(crate) color: [f32; 4],
    /// Spot direction and cosine of the outer cone (-2 = a point light).
    pub(crate) dir: [f32; 4],
    /// Cosine of the inner cone, the core radius, a headlamp's beam (1 low, -1 full), the
    /// radius (`pos.w` is 0 on a light only the enhanced path draws, which the vanilla
    /// shader then passes by).
    pub(crate) extra: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct GpuCorona {
    pub(crate) pos: [f32; 3],
    pub(crate) size: f32,
    pub(crate) color: [f32; 4],
    pub(crate) dir: [f32; 4],
    /// xyz: the up axis; w: rotating mode
    pub(crate) up: [f32; 4],
    /// x: inner cone cosine, y: z offset, z: flags
    pub(crate) extra: [f32; 4],
}

/// A point light in world space (`[maplight]`, `[interiorlight]`, headlights).
#[derive(Debug, Clone, Copy)]
pub struct PointLight {
    pub position: DVec3,
    pub radius: f32,
    pub color: [f32; 3],
    pub intensity: f32,
    /// A spot light's direction (zero = a point light) and the cosines of its inner and
    /// outer cone (enhanced path).
    pub direction: Vec3,
    pub cone: [f32; 2],
    /// The radius within which the light is at full strength (`[maplight]`'s); 0 = an
    /// eighth of `radius` (enhanced path; vanilla always takes the eighth).
    pub core: f32,
    /// A headlamp (enhanced path), lit by a road lamp's profile instead of the cone: 1 a low
    /// beam, with its cut-off at the lamp's horizon, -1 a full beam, without; 0 any other light.
    pub beam: f32,
    /// A lamp in a housing - a street lamp's head, a platform's light (`[maplight]`): the
    /// enhanced path sends its light down and out, a few per cent above its horizon.
    pub housed: bool,
    /// Which path draws the light.
    pub mode: LightMode,
}

impl Default for PointLight {
    fn default() -> Self {
        Self {
            position: DVec3::ZERO,
            radius: 0.0,
            color: [1.0; 3],
            intensity: 1.0,
            direction: Vec3::ZERO,
            cone: [1.0, 0.0],
            core: 0.0,
            beam: 0.0,
            housed: false,
            mode: LightMode::Both,
        }
    }
}

/// Which renderer a light belongs to: a vehicle's headlight is three point lights along
/// its axis for the vanilla path and one real spot light for the enhanced one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightMode {
    #[default]
    Both,
    Vanilla,
    Enhanced,
}

/// One particle of a `[smoke]` system (exhaust, boiling coolant, wheel spray, chimneys).
///
/// Omsi.exe draws its puffs (0x5a4180: fog and lighting off, blended over the scene, depth
/// tested but not written) as squares facing the screen, each turned by its own angle and
/// moved a tenth of a metre towards the eye in depth (0x5a2b5c), and lets them sink on
/// through the road, which cuts every one off in a straight line where it goes in. Under a
/// wheel's spray - dozens of fresh puffs a second, falling in - that is a stack of bright
/// bands across the road. Here a puff that knows its `ground` fades out over its lowest
/// part into it instead (see `vs_main` in corona.wgsl).
#[derive(Debug, Clone, Copy, Default)]
pub struct SmokeParticle {
    pub position: DVec3,
    /// Half its width (m).
    pub size: f32,
    pub color: [f32; 3],
    pub alpha: f32,
    /// The angle its picture is turned by about the line of sight (radians).
    pub spin: f32,
    /// How far its depth is moved towards the eye (m), its place and size on the screen
    /// kept: Omsi.exe's 0.1 for every `[smoke]` particle (0x5a1d54 sets it, 0x5a2b5c
    /// subtracts it from the view depth it projects the depth at).
    pub z_offset: f32,
    /// World z of the ground under it (the road its vehicle stands on): it fades out
    /// towards it, and one wholly under it is left out. `None`: drawn as it is.
    pub ground: Option<f64>,
}

/// The light map atlas: tiles a side and pixels a tile.
pub const LM_ATLAS_TILES: u32 = 5;

pub const LM_TILE_PX: u32 = 256;

/// A light corona sprite (`[light_enh]`, `[light_enh_2]`).
#[derive(Debug, Clone, Copy)]
pub struct Corona {
    pub position: DVec3,
    pub size: f32,
    pub color: [f32; 3],
    /// 0..2 (a `[light_enh_2]` fading variable of 2 is double brightness).
    pub brightness: f32,
    /// Facing direction (zero = omnidirectional) and cosine of the visibility half-cone
    /// (the outer cone: where it begins to be seen).
    pub direction: Vec3,
    pub cone_cos: f32,
    /// Cosine of the inner cone (full brightness); below `cone_cos` = no inner cone.
    pub inner_cos: f32,
    /// `[light_enh_2]` rotating: 0 a flat sprite facing `direction`, 1 turned to the viewer
    /// about `up`, 2 turned to the viewer about every axis (a billboard).
    pub rotating: u8,
    pub up: Vec3,
    /// How far the spot is moved from its place towards the viewer (m), so that a lamp
    /// inside its housing still shows; negative = the old default (half its size, at most
    /// half a metre).
    pub z_offset: f32,
    /// `[light_enh_2]` parameter bits: 1 star, 2 no fog, 4 only effects.
    pub flags: u8,
    /// Its picture: 0 the standard glow, else one registered with
    /// [`Renderer::set_corona_texture`] (a light's own `bitmap`, the fog cone's picture).
    pub texture: u16,
    /// A light's cone in the fog rather than its glow (OMSI's `light_cone.bmp` fan, see
    /// corona.wgsl): `size` is the fan's radius, `cone_cos` and `inner_cos` hold the outer
    /// and inner half angles (radians), `beam_width` the fog's visibility (m).
    pub beam: bool,
    pub beam_width: f32,
    /// The halo round a light in fog: a billboard of `size`, pulled
    /// towards the viewer, seen from in front of the light; the angles and the visibility
    /// travel as for a cone.
    pub halo: bool,
}

impl Default for Corona {
    fn default() -> Self {
        Corona {
            position: DVec3::ZERO,
            size: 0.1,
            color: [1.0; 3],
            brightness: 0.0,
            direction: Vec3::ZERO,
            cone_cos: -1.0,
            inner_cos: -2.0,
            rotating: 2,
            up: Vec3::Z,
            z_offset: -1.0,
            flags: 0,
            texture: 0,
            beam: false,
            beam_width: 0.0,
            halo: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct Lighting {
    /// Objects smaller on the screen than this are not drawn: the original's
    /// `performance_minObjSize`, in its own measure (the object's diameter over its distance,
    /// as a share of the vertical field of view; see `render_inner`), times the object's
    /// `[detail_factor]`. OMSI's presets say 0.013 (0.020 for the slowest machines). The
    /// renderer's `RenderOptions::min_obj_size` is the floor; a picture may ask for more
    /// (the mirrors take the original's `performance_minObjSizeRefl`).
    pub min_obj_size: f32,
    pub sun_dir: Vec3,
    pub sun_intensity: f32,
    /// Direct sun colour (envir light A).
    pub sun_color: Vec3,
    /// Light from above (envir light B).
    pub secondary: Vec3,
    /// Undirected light (envir light C).
    pub ambient: Vec3,
    pub fog_color: Vec3,
    pub fog_density: f32,
    pub sky_color: Vec3,
    /// 0 at day, 1 at night: strength of nightmaps and coronas.
    pub night: f32,
    /// The night maps (`[matl_nightmap]`, the tiles' light maps) switched as Omsi.exe
    /// switches them - on with the lamps, not faded in with the dusk (its stage is set when
    /// the object's `NightlightA` is over 0.5, 0x61197a/0x7fee02); None: by `night`.
    pub night_maps: Option<f32>,
    /// Sun azimuth (radians, clockwise from north) and day/twilight/night sky texture weights.
    pub sun_azimuth: f32,
    pub sky_weights: [f32; 3],
    /// Cloud layer: density 0..1 and the texture offset (wind drift), 0 = no clouds.
    pub cloud_density: f32,
    pub cloud_offset: [f32; 2],
    /// Sun shadow map (off in mirrors and at night).
    pub shadows: bool,
    /// How wet the roads are (0..1): rain darkens them and makes them mirror the sky.
    pub wetness: f32,
    /// Snow cover on the ground and the roads (0..1).
    pub snow: f32,
    /// The roads are kept clear of the snow (the weather's "snow on road" off): no cover is
    /// laid on road surfaces.
    pub roads_clear: bool,
    /// Enhanced graphics: the physically based high-range renderer (enhanced.wgsl) with its
    /// computed sky, automatic exposure, glow and tone mapping (post.wgsl).
    pub enhanced: bool,
    /// Vanilla graphics - the picture as OMSI 2 draws it: none of the extras the
    /// rewrite's own vanilla renderer (Vanilla+) adds (snow laid on the surfaces, rain drops
    /// running down the panes). Shadows, ambient occlusion and the detail grain are switched off by the settings.
    pub classic: bool,
    /// The player's vehicle (origin, heading in degrees, `[boundingbox]` w l h cx cy cz):
    /// no rain sheen or snow cover is shaded inside it.
    pub inside: Option<(DVec3, f64, [f32; 6])>,
    /// Actual road height beneath the player's vehicle; independent of suspension motion.
    /// The local puddle capture is skipped when no road height is known.
    pub puddle_ground: Option<f64>,
    /// Upward normal of that actual road face (including road grade and camber).
    pub puddle_normal: Vec3,
    /// Coupled parts of the player's vehicle (same layout as `inside`). Their own
    /// origins keep shared AI meshes out of the local puddle capture.
    pub puddle_parts: Vec<(DVec3, f64, [f32; 6])>,
    /// Procedural (fractal) detail texturing of the ground and roads up close - the
    /// `detail_textures` setting; independent of `enhanced`.
    pub detail: bool,
    /// Enhanced path: how closed the cloud cover is (0..1; `sun_intensity` already says
    /// how much sun comes through), how hard it rains (0..1), the height the weather's fog
    /// lies on (the ground under the player; `None` = just under the camera), and
    /// envir.cfg's light colours relative to the stock ones (A sun, B sky, C ambient).
    pub overcast: f32,
    pub rain: f32,
    /// Enhanced path: the street lamps cast shadows (the shadows setting; unlike the sun's,
    /// whatever the weather).
    pub lamp_shadows: bool,
    /// How hard it snows (0..1): the snowfall's flakes (snow.wgsl, every path), and in the
    /// enhanced picture the view they take (`enhanced_weather_fog`) - falling snow takes far
    /// more of it than rain of the same water does.
    pub snowfall: f32,
    /// The weather's wind (m/s, world): the snowfall drifts with it.
    pub wind: Vec3,
    /// The condensation on the player's bus's panes as optical depths (the share of the
    /// light they scatter is 1 - e^-depth): windscreen, side windows, rear window, and how
    /// far the defroster has cleared the windscreen (0..1; enhanced path).
    pub condensation: [f32; 4],
    pub fog_base: Option<f64>,
    pub envir_tint: [Vec3; 3],
    /// How bright an LED panel's dots burn (`MaterialExtra::led`; the settings' 16 levels
    /// give 0 = off .. 3.75): the enhanced picture draws them this much above their own
    /// colour, bright enough for the glow to bloom a halo around the panel.
    pub led_glow: f32,
    /// How much of the mip chain an LED panel is held at - the `\S:n` mask's (`STFilter`)
    /// and the panel's own grid picture's: both are sampled at the level their screen
    /// footprint asks for, never coarser than this. 0 point-samples them (the sharpest
    /// dots, and the worst shimmer - a regular grid is the worst case for a point sample);
    /// 1.3 (the default) keeps a matrix's dots a couple of pixels across where the full
    /// chain has run them together, and what shimmer is left is a fraction of a
    /// full-resolution sample's; 4 is near the calm of the full chain.
    pub led_mips: f32,
    /// How much brighter the night is shown, in exposure steps after sunset (the settings' 0 .. 3).
    pub night_brightness: f32,
    /// The player's vehicle's velocity (m/s, world): at speed the airstream drives the drops
    /// on its glass up the windscreen and back along the side windows.
    pub glass_wind: Vec3,
    /// Windy trees (the `windy_trees` setting): the foliage of trees bends and sways in
    /// `wind` (see shader.wgsl `tree_sway`).
    pub windy_trees: bool,
    /// Towards the moon (world space) and how much of its disc is lit (0 new .. 1 full):
    /// the enhanced night's moonlight and the moon in its sky.
    pub moon_dir: Vec3,
    pub moon_illum: f32,
    /// The day of the year (1..366) and the latitude (degrees north): the season's air.
    pub day_of_year: f32,
    pub latitude: f32,
    /// One number per calendar day, which the enhanced sky draws the day's own air from.
    pub day_seed: u32,
    /// The optical depth of a high ice-cloud veil (0 none .. 2 a thick milky one).
    pub veil: f32,
    /// The air as a weather model knows it: aerosol amount relative to a clear day, its
    /// Ångström exponent and its layer's depth (m); without it the day's own air is drawn
    /// from the calendar (`day_air`).
    pub air: Option<[f32; 3]>,
}

impl Lighting {
    /// Whether the sun shadow map is drawn with this light (not once the sun is about a
    /// degree below the horizon - Omsi.exe's cutoff, sun z -0.02 in sub_754c80 - nor with
    /// the sun dim, nor with OMSI_NO_SHADOWS).
    /// By night the moon casts the shadows the sun casts by day (the enhanced path): the
    /// sun well down, the moon up and more than a quarter lit - a full moon's 0.3 lux leave
    /// sharp shadows on a road beyond the lamps.
    pub fn casts_moon_shadows(&self) -> bool {
        self.enhanced
            && self.lamp_shadows
            && self.sun_dir.normalize_or_zero().z < -0.1
            && self.moon_dir.normalize_or_zero().z > 0.1
            && self.moon_illum > 0.25
            && !omsi_cfg::flags::NO_SHADOWS.is_set()
    }

    pub fn casts_sun_shadows(&self) -> bool {
        self.shadows
            && self.sun_dir.normalize_or_zero().z > -0.02
            && self.sun_intensity > 0.05
            && !omsi_cfg::flags::NO_SHADOWS.is_set()
    }
}

impl Default for Lighting {
    fn default() -> Self {
        Self {
            min_obj_size: 0.0,
            sun_dir: Vec3::new(0.3, 0.2, 0.9).normalize(),
            sun_intensity: 0.9,
            sun_color: Vec3::ONE,
            secondary: Vec3::splat(0.15),
            ambient: Vec3::splat(0.25),
            fog_color: Vec3::new(0.70, 0.78, 0.90),
            fog_density: 0.0006,
            sky_color: Vec3::new(0.55, 0.70, 0.92),
            night: 0.0,
            night_maps: None,
            sun_azimuth: 0.0,
            sky_weights: [1.0, 0.0, 0.0],
            cloud_density: 0.0,
            cloud_offset: [0.0; 2],
            shadows: true,
            snowfall: 0.0,
            wind: Vec3::ZERO,
            condensation: [0.0; 4],
            lamp_shadows: false,
            wetness: 0.0,
            snow: 0.0,
            roads_clear: false,
            enhanced: false,
            classic: false,
            inside: None,
            puddle_ground: None,
            puddle_normal: Vec3::Z,
            puddle_parts: Vec::new(),
            detail: true,
            overcast: 0.0,
            rain: 0.0,
            fog_base: None,
            envir_tint: [Vec3::ONE; 3],
            night_brightness: 0.0,
            led_glow: 1.5,
            led_mips: 1.3,
            glass_wind: Vec3::ZERO,
            windy_trees: false,
            moon_dir: Vec3::new(0.0, -0.5, -0.866),
            moon_illum: 0.0,
            day_of_year: 150.0,
            latitude: 52.5,
            day_seed: 0,
            veil: 0.0,
            air: None,
        }
    }
}

/// Does this path draw the light? (See `Renderer::prepare_lights`.)
pub(crate) fn drawn_by(l: &PointLight, enhanced: bool) -> bool {
    l.radius > 0.0
        && l.intensity > 0.0
        && l.mode
            != if enhanced {
                LightMode::Vanilla
            } else {
                LightMode::Enhanced
            }
}

/// A light as the shaders read it, at `p` relative to the render origin.
pub(crate) fn gpu_light(l: &PointLight, p: Vec3) -> GpuPointLight {
    let spot = l.direction.length_squared() > 1e-6;
    let dir = if spot {
        l.direction.normalize().extend(l.cone[1]).to_array()
    } else if l.mode == LightMode::Vanilla {
        // (a vehicle's headlight stand-in: lights a light-mapped road too, see
        // shader.wgsl point_lights)
        [1.0, 0.0, 0.0, -2.0]
    } else if l.housed {
        // (z: a housed lamp, see `PointLight::housed`; the classic shader reads only x)
        [0.0, 0.0, -1.0, -2.0]
    } else {
        [0.0, 0.0, 0.0, -2.0]
    };
    let vanilla_radius = if l.mode == LightMode::Enhanced {
        0.0
    } else {
        l.radius
    };
    GpuPointLight {
        pos: [p.x, p.y, p.z, vanilla_radius],
        color: [l.color[0], l.color[1], l.color[2], l.intensity],
        dir,
        extra: [l.cone[0], l.core, l.beam, l.radius],
    }
}

/// `OMSI_DEBUG_ENHANCED=n`: the enhanced main pass shows one of its terms alone (1 sun
/// shadow, 2 AO, 3 normal, 4 air transmittance, 5 ambient light, 6 reflection, 7 albedo,
/// 8 direct sun, 9 in-scattered air, 11 distance/depth, 12 alpha mode/terrain/surface,
/// 13 cab/AO/specular occlusion, 14 direct + ambient, 15 lamps and headlights, 16 what
/// glows by itself, 17 roughness/F0/metalness, 10 alpha/glass/envmap); `OMSI_ENV_PHOTO=0`
/// leaves the `[matl_envmap]` photo's structure out of the reflections. A blended surface
/// shows its values as if it were opaque: an invisible layer round the player's bus hides
/// the bus in these views.
pub(crate) fn debug_view() -> f32 {
    static VIEW: std::sync::OnceLock<f32> = std::sync::OnceLock::new();
    *VIEW.get_or_init(|| omsi_cfg::flags::DEBUG_ENHANCED.number().unwrap_or(0.0))
}

/// Hermite step from 0 at `e0` to 1 at `e1` (as WGSL's `smoothstep`).
pub(crate) fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}
