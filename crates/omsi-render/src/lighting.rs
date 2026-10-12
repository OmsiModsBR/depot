//! Lighting: the sun, sky and exposure settings (`Lighting`), point lights, coronas and
//! smoke sprites, the light grid and the street lamps' shadows.

use super::*;

/// The illuminance a light's core gives (maplight colour 1, in the sky model's units:
/// 55 lux, so the street under a lamp gets its 15-25 lux).
pub(crate) const LAMP_E: f32 = 0.0055;

/// Illuminance of a bus saloon's lamps on the seats and the floor (300 lux).
pub(crate) const CABIN_E: f32 = 0.03;

/// A lit window's radiance at night.
pub(crate) const WINDOW_RADIANCE: f32 = 0.0022;

/// The metering (see `meter_tuning`).
pub(crate) const METER_GAIN: f32 = 0.4;

pub(crate) const METER_TARGET: f32 = -2.84;

// (how far the eye adapts to what it looks at beyond what the light model knows: the sun
// in view, a dark cab or an underpass by day - two stops and a half and more for the
// eye; at the metering's gain a snow field still comes out darkened by well under a stop)
pub(crate) const METER_DARKEN: f32 = 2.0;

pub(crate) const METER_BRIGHTEN: f32 = 1.6;

/// The tone curve's contrast about mid grey by day and at night (see `tone_contrast`).
pub(crate) const TONE_CONTRAST_DAY: f32 = 1.22;

pub(crate) const TONE_CONTRAST_NIGHT: f32 = 0.94;

/// How far night vision takes the colour out of a dark scene (post.wgsl `night_vision`).
pub(crate) const NIGHT_VISION: f32 = 0.3;

/// The sun's angular radius as drawn (a little larger than the real 0.27°).
pub(crate) const SUN_RADIUS: f32 = 0.0065;

/// The moon's, as drawn (the real one is as large as the sun's; a little more, as the
/// eye sees it).
pub(crate) const MOON_RADIUS: f32 = 0.0062;

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

/// How high over its ground a smoke puff of half width `size` fades out into it (m): a fifth
/// of its drawn radius, between 6 and 25 cm - enough that no line shows where the road
/// meets it, little enough that a wheel's spray, all but a few tens of centimetres of which
/// is under the road, still shows as the mist round the tyres Omsi.exe draws.
pub(crate) fn smoke_ground_fade(size: f32) -> f32 {
    (0.2 * 0.9 * size).clamp(0.06, 0.25)
}

/// The corona shader's sprite for a smoke particle (`extra.w` 3: the smoke branch of
/// `vs_main`): `dir` the cosine and sine of its spin, `up` its ground (relative to the
/// render origin `ro`, `up.y` 1 when it has one), its z offset and how high the fade into
/// the ground reaches. `None` for a puff too faint or small to draw, or wholly under its
/// ground.
pub(crate) fn smoke_sprite(p: &SmokeParticle, ro: DVec3) -> Option<GpuCorona> {
    if !(p.alpha > 0.002 && p.size > 0.0) {
        return None;
    }
    // (drawn at 0.9 of its size, see `vs_main`)
    if p.ground
        .is_some_and(|g| p.position.z + (p.size * 0.9) as f64 <= g)
    {
        return None;
    }
    let (sin, cos) = p.spin.sin_cos();
    let ground = p.ground.filter(|g| g.is_finite());
    Some(GpuCorona {
        pos: (p.position - ro).as_vec3().to_array(),
        size: p.size,
        color: [p.color[0], p.color[1], p.color[2], p.alpha.clamp(0.0, 1.0)],
        dir: [cos, sin, 0.0, 0.0],
        up: [
            ground.map(|g| (g - ro.z) as f32).unwrap_or(0.0),
            if ground.is_some() { 1.0 } else { 0.0 },
            p.z_offset.max(0.0),
            smoke_ground_fade(p.size),
        ],
        extra: [-2.0, 0.0, 0.0, 3.0],
    })
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

pub(crate) const LIGHT_CELL: f32 = 25.0;

/// Enhanced: how many street lamps cast shadows (their maps are tiles of a quarter of the
/// shadow size under the far map), and how far from the camera a lamp's reach may end.
pub(crate) const LAMP_SHADOWS: usize = 4;

pub(crate) const LAMP_SHADOW_REACH: f32 = 45.0;

/// The far map's height over its width: the lamps' tiles take the quarter under it.
pub(crate) const FAR_MAP_ASPECT: f32 = 1.25;

/// A lamp's shadow map looks straight down from its head over this field of view (a
/// street lamp's light leaves it downwards, see `PointLight::housed`).
pub(crate) const LAMP_SHADOW_FOV: f32 = 150.0;

/// A street lamp that casts shadows this frame.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LampShadow {
    /// Its index in the light buffer, where it is (render-origin relative) and its reach.
    pub(crate) index: u32,
    pub(crate) position: Vec3,
    pub(crate) range: f32,
}

impl LampShadow {
    /// The shadow map's view and projection: straight down, depth 0..1 over its reach.
    pub(crate) fn view_proj(&self) -> Mat4 {
        let proj =
            Mat4::perspective_rh(LAMP_SHADOW_FOV.to_radians(), 1.0, 0.1, self.range.max(1.0));
        let view = Mat4::look_to_rh(self.position, -Vec3::Z, Vec3::Y);
        proj * view
    }
}

pub(crate) const LIGHT_GRID_SIDE: usize = 64;

/// (32: a depot or a bus interior with many lamps lost the farthest past 16 in a cell)
pub(crate) const LIGHT_CELL_CAP: usize = 32;

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
            && omsi_cfg::env::var_os("OMSI_NO_SHADOWS").is_none()
    }

    pub fn casts_sun_shadows(&self) -> bool {
        self.shadows
            && self.sun_dir.normalize_or_zero().z > -0.02
            && self.sun_intensity > 0.05
            && omsi_cfg::env::var_os("OMSI_NO_SHADOWS").is_none()
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
    *VIEW.get_or_init(|| {
        omsi_cfg::env::var("OMSI_DEBUG_ENHANCED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(0.0)
    })
}

/// The metering: the share of the metered difference that is corrected, its target (log2 of
/// the picture's mean luminance), how far it may darken and brighten (EV), the exposure
/// bias (EV) and the night vision strength. `OMSI_METER=gain,target,dark,bright,bias,night`
/// overrides them for tuning.
pub(crate) fn meter_tuning() -> [f32; 6] {
    static METER: std::sync::OnceLock<[f32; 6]> = std::sync::OnceLock::new();
    *METER.get_or_init(|| {
        let mut m = [
            METER_GAIN,
            METER_TARGET,
            METER_DARKEN,
            METER_BRIGHTEN,
            0.0,
            NIGHT_VISION,
        ];
        if let Ok(v) = omsi_cfg::env::var("OMSI_METER") {
            for (k, x) in v.split(',').take(6).enumerate() {
                if let Ok(x) = x.trim().parse() {
                    m[k] = x;
                }
            }
        }
        m
    })
}

/// The tone curve's contrast about mid grey for a pre-exposure (its natural log): a
/// camera's by day, falling off through the dusk to a little under none at night, where
/// the eye's own adaptation to the dark between the street lamps lifts it out of black
/// (a street a few hundred times darker than the pool under a lamp is still seen). `OMSI_TONE_CONTRAST=day,night`
/// overrides it.
pub(crate) fn tone_contrast(log_pre: f32) -> f32 {
    static OVERRIDE: std::sync::OnceLock<Option<(f32, f32)>> = std::sync::OnceLock::new();
    let (day, night) = OVERRIDE
        .get_or_init(|| {
            let v = omsi_cfg::env::var("OMSI_TONE_CONTRAST").ok()?;
            let mut it = v.split(',').map(|x| x.trim().parse::<f32>().ok());
            Some((it.next()??, it.next().flatten().unwrap_or(1.0)))
        })
        .unwrap_or((TONE_CONTRAST_DAY, TONE_CONTRAST_NIGHT));
    // (the pre-exposure is about 1 by day, 70 in the blue hour and 400-700 at night)
    let t = atmosphere::smoothstep(2.0, 8.0, log_pre / std::f32::consts::LN_2);
    day + (night - day) * t
}

/// How much of the sun the enhanced sky's cumulus lets through to the camera: the same
/// cloud the sky shader draws (sky_enhanced.wgsl `cloud_base_shape`, `cloud_sigma`: the
/// shape map's heaps cut by the cover, rounded with height, without the billows), its
/// extinction integrated along the sun's direction through the layer, over a few rays a
/// few tens of metres apart (the penumbra of a cloud a kilometre and a half up).
pub(crate) const CLOUD_SHADOW_EROSION: f32 = 0.15;

/// How far the exposure follows the light into a cloud's shadow (log terms).
pub(crate) const CLOUD_SHADE_ADAPT: f32 = 0.5;

pub(crate) fn cloud_sun_transmittance(
    shape: &[u8],
    lighting: &Lighting,
    cam_rel: Vec3,
    ro: DVec3,
) -> f32 {
    const BOTTOM: f32 = 1400.0;
    const TOP: f32 = 2800.0;
    const PERIOD: f32 = 13000.0;
    const SIGMA: f32 = 0.035;
    let s = lighting.sun_dir.normalize_or_zero();
    if s.z <= 0.02 || shape.is_empty() {
        return 1.0;
    }
    let size = (clouds::SHAPE_SIZE / 2) as usize;
    if shape.len() < size * size * 4 {
        return 1.0;
    }
    let texel = |x: i64, y: i64| {
        let (x, y) = (
            x.rem_euclid(size as i64) as usize,
            y.rem_euclid(size as i64) as usize,
        );
        let i = (y * size + x) * 4;
        [
            shape[i] as f32 / 255.0,
            shape[i + 1] as f32 / 255.0,
            shape[i + 2] as f32 / 255.0,
        ]
    };
    let sample = |u: f32, v: f32| {
        let (x, y) = (u * size as f32 - 0.5, v * size as f32 - 0.5);
        let (x0, y0) = (x.floor(), y.floor());
        let (fx, fy) = (x - x0, y - y0);
        let (x0, y0) = (x0 as i64, y0 as i64);
        let a = texel(x0, y0);
        let b = texel(x0 + 1, y0);
        let c = texel(x0, y0 + 1);
        let d = texel(x0 + 1, y0 + 1);
        let mut o = [0.0f32; 3];
        for k in 0..3 {
            o[k] =
                (a[k] * (1.0 - fx) + b[k] * fx) * (1.0 - fy) + (c[k] * (1.0 - fx) + d[k] * fx) * fy;
        }
        o
    };
    let lin = |a: f32, b: f32, v: f32| ((v - a) / (b - a)).clamp(0.0, 1.0);
    let cover = lighting.cloud_density.clamp(0.0, 1.0);
    // (less the billows' erosion, which the sky shader takes off the heaps' edges and this
    // leaves out: on average about this much of the cover)
    let coverage = (0.3 + cover * 0.55 - CLOUD_SHADOW_EROSION).clamp(0.0, 1.0);
    let origin = glam::Vec2::new(
        ro.x.rem_euclid(CLOUD_ORIGIN_PERIOD) as f32,
        ro.y.rem_euclid(CLOUD_ORIGIN_PERIOD) as f32,
    );
    let drift = glam::Vec2::new(lighting.cloud_offset[0], lighting.cloud_offset[1]) * 2500.0;
    let t0 = (BOTTOM - cam_rel.z).max(0.0) / s.z;
    let t1 = ((TOP - cam_rel.z).max(0.0) / s.z).min(t0 + 12_000.0);
    let steps = 24;
    let ds = (t1 - t0) / steps as f32;
    let side = glam::Vec2::new(-s.y, s.x).normalize_or_zero();
    let mut sum = 0.0;
    let offsets = [
        glam::Vec2::ZERO,
        side * 40.0,
        -side * 40.0,
        glam::Vec2::new(s.x, s.y).normalize_or_zero() * 40.0,
        -glam::Vec2::new(s.x, s.y).normalize_or_zero() * 40.0,
    ];
    for off in offsets {
        let mut od = 0.0;
        for k in 0..steps {
            let t = t0 + (k as f32 + 0.5) * ds;
            let p = cam_rel + s * t;
            let h = (p.z - BOTTOM) / (TOP - BOTTOM);
            if !(0.0..1.0).contains(&h) {
                continue;
            }
            let g = glam::Vec2::new(p.x, p.y) + origin + off + drift;
            let m = sample(g.x / PERIOD, g.y / PERIOD);
            let lo = m[1] - 1.0;
            let n = h * h * (0.7 + m[2]) + (1.0 - h).powi(16);
            let base = (m[0] - n - lo) / (1.0 - lo) * (lin(0.0, 0.1, h) - lin(0.6, 1.0, h));
            let x = ((base + coverage - 1.0) / 0.12).clamp(0.0, 1.0);
            let dens = x * x * (3.0 - 2.0 * x) * (h / 0.2).min(1.0);
            od += dens * SIGMA * ds;
        }
        sum += (-od).exp();
    }
    sum / offsets.len() as f32
}

/// How the enhanced picture's weather fog thins out with height (1/m): a 300 m scale height
/// over the fog's base (`layer_depth` in enhanced_common.wgsl).
pub(crate) const FOG_FALLOFF: f32 = 1.0 / 300.0;

/// The weather's own fog in the enhanced picture, its extinction at the base (1/m):
/// vanilla's density, which the culling uses as well; none below 1e-4, where a clear day's
/// air is the sky model's.
pub(crate) fn enhanced_weather_fog(lighting: &Lighting) -> f32 {
    let fog = if lighting.fog_density > 1e-4 {
        lighting.fog_density
    } else {
        0.0
    };
    fog + snowfall_extinction(lighting.snowfall)
}

/// How much the lamps round the camera light the night sky over it, relative to a city's
/// (Spandau's middle is 1): a sky's glow is the lamps' upward and reflected light scattered
/// back down by the air, each source's part falling off with the distance to it as
/// d^-2.5 (Walker's law, "The effects of urban lighting on the brightness of the night
/// sky", 1977; the near field evened out over the first few hundred metres). A village's
/// handful of lamps leaves the stars; a city's thousands turn the sky orange-grey.
pub(crate) const SKY_GLOW_REF: f32 = 830.0;

pub(crate) fn lamp_sky_glow(scene: &Scene, cam_rel: Vec3) -> f32 {
    let ro = scene.render_origin;
    let mut sum = 0.0f32;
    for l in &scene.lights {
        if !l.housed || l.intensity <= 0.0 {
            continue;
        }
        let d = ((l.position - ro).as_vec3() - cam_rel).truncate().length();
        let lum = (l.color[0] + l.color[1] + l.color[2]) / 3.0;
        sum += l.intensity * lum * l.core * l.core / (1.0 + (d / 300.0).powf(2.5));
    }
    sum / SKY_GLOW_REF
}

/// The light of the lamps and the headlights on the ground the camera looks at (irradiance,
/// 1 = 10 000 lux): the log-average over the points where a grid of rays through the
/// picture meets the ground (`ground`: the street's height relative to the render origin,
/// the player's vehicle's), each lit as the enhanced pass lights it (a street lamp down
/// and out, a headlamp ahead and under its cut-off). What the eye adapts to by night,
/// rather than a fixed guess.
///
/// The rays are the picture's own: a dozen points 5 to 30 m ahead on a ground taken 1.6 m
/// under the camera missed the street the eye actually sees from any camera higher than a
/// driver's (it lay in the air there, out of every lamp's reach) and the lamp-lit pools to
/// the sides of a narrow view - the meter read a moonless night while the picture was full
/// of lit streets, and the eye's adaptation to that dark lifted every lamp's light to
/// white (the modded maps' lamps, spaced and placed differently from the stock ones, most).
pub(crate) fn view_lamp_light(
    lights: &[PointLight],
    ro: DVec3,
    cam_rel: Vec3,
    camera: &Camera,
    aspect: f32,
    ground: Option<f32>,
) -> f32 {
    let ground = ground
        .filter(|g| g.is_finite() && *g < cam_rel.z - 0.3)
        .unwrap_or(cam_rel.z - 1.6);
    let (f, r, u) = (camera.forward(), camera.right(), camera.up());
    let ty = (camera.fov_deg.to_radians() * 0.5).tan();
    let tx = ty * aspect.max(0.1);
    let mut points = Vec::with_capacity(35);
    for j in 0..5 {
        for i in 0..7 {
            let x = -0.9 + 1.8 * i as f32 / 6.0;
            let y = -0.95 + 1.9 * j as f32 / 4.0;
            let dir = (f + r * (x * tx) + u * (y * ty)).normalize();
            if dir.z >= -0.01 {
                continue;
            }
            // (as far as a lamp's light still shapes what the eye takes to: a pool 300 m
            // off is a point of light, not the scene)
            let t = (ground - cam_rel.z) / dir.z;
            if t > 0.0 && t < 300.0 {
                points.push(cam_rel + dir * t);
            }
        }
    }
    if points.is_empty() {
        // (looking up: the ground round the camera, which the eye has just seen)
        let f = Vec3::new(f.x, f.y, 0.0).normalize_or(Vec3::Y);
        for d in [5.0f32, 10.0, 18.0, 30.0] {
            for a in [-0.5f32, 0.0, 0.5] {
                let (s, c) = a.sin_cos();
                let dir = Vec3::new(f.x * c - f.y * s, f.x * s + f.y * c, 0.0);
                points.push(Vec3::new(cam_rel.x, cam_rel.y, ground) + dir * d);
            }
        }
    }
    let mut per_point = vec![0.0f32; points.len()];
    for l in lights {
        if l.intensity <= 0.0 || l.mode == LightMode::Vanilla {
            continue;
        }
        let p = (l.position - ro).as_vec3();
        let lum = 0.2126 * l.color[0] + 0.7152 * l.color[1] + 0.0722 * l.color[2];
        let core = if l.core > 0.0 {
            l.core
        } else {
            l.radius * 0.125
        };
        for (pi, x) in points.iter().enumerate() {
            let to = *x - p;
            let d2 = to.length_squared();
            if d2 >= l.radius * l.radius {
                continue;
            }
            let q = d2 / (l.radius * l.radius);
            let window = (1.0 - q * q) * (1.0 - q * q);
            let t = to / d2.sqrt().max(1e-3);
            let e = if l.beam != 0.0 {
                // a headlamp lights the road it stands on (some 0.8 m under it), by its
                // profile (lamp_air.wgsl `headlamp`)
                let g = Vec3::new(x.x, x.y, p.z - 0.8) - p;
                let gd2 = g.length_squared();
                headlamp_profile(g / gd2.sqrt().max(1e-3), l.direction, l.beam > 0.0) / gd2.max(0.3)
                    * window
            } else {
                let mut e = core * core / (d2 * d2 + core.powi(4)).sqrt() * window;
                if l.direction.length_squared() > 1e-6 {
                    e *= atmosphere::smoothstep(
                        l.cone[1],
                        l.cone[0],
                        t.dot(l.direction.normalize()),
                    );
                } else if l.housed {
                    e *= 0.05 + 0.95 * atmosphere::smoothstep(-0.1, 0.3, -t.z);
                }
                e
            };
            per_point[pi] += LAMP_E * l.intensity * lum * e;
        }
    }
    // the log-average, as the eye adapts to a field of view (Reinhard's and Krawczyk's
    // adapting luminance): a headlight's pool in front of the camera does not decide it
    // alone, as the plain mean let it (facing a bus's lights, all else went black). The
    // floor is a moonless night's light, which every point has.
    let floor = 2e-6f32;
    let log_sum: f32 = per_point.iter().map(|v| (v + floor).ln()).sum();
    (log_sum / per_point.len() as f32).exp() - floor
        + glare_veil(lights, ro, cam_rel, camera.forward())
}

/// The lamps' and headlights' glare in view as the illuminance whose mid grey has the same
/// luminance (1 = 10 000 lux): the light of every lamp the eye sees, scattered in the eye
/// over the field of view, is a veil the eye adapts to as to any other light - the
/// Stiles-Holladay disability glare, L = 10 x the illuminance at the eye (lux) / the angle
/// off the line of sight squared (degrees), summed over the sources (CIE 146). Metered on
/// the ground between the lamps alone, a village full of street lamps in view read as a
/// moonless night, and the eye's adaptation to that dark lifted every lamp's light to white.
pub(crate) fn glare_veil(lights: &[PointLight], ro: DVec3, cam_rel: Vec3, forward: Vec3) -> f32 {
    let f = forward.normalize_or(Vec3::Y);
    let mut veil = 0.0f32; // cd/m²
    for l in lights {
        if l.intensity <= 0.0 || l.mode == LightMode::Vanilla {
            continue;
        }
        let to_lamp = (l.position - ro).as_vec3() - cam_rel;
        let d2 = to_lamp.length_squared();
        // (a lamp out past a few hundred metres is a point among the stars to the eye; one
        // within a metre is the bus's own)
        if !(1.0..300.0 * 300.0).contains(&d2) {
            continue;
        }
        let dist = d2.sqrt();
        let theta = f.dot(to_lamp / dist).clamp(-1.0, 1.0).acos().to_degrees();
        if theta >= 90.0 {
            continue;
        }
        let lum = 0.2126 * l.color[0] + 0.7152 * l.color[1] + 0.0722 * l.color[2];
        let core = if l.core > 0.0 {
            l.core
        } else {
            l.radius * 0.125
        };
        // towards the eye: from the lamp
        let t = -to_lamp / dist;
        let k = if l.beam != 0.0 {
            headlamp_profile(t, l.direction, l.beam > 0.0) / (core * core).max(1e-3)
        } else if l.direction.length_squared() > 1e-6 {
            atmosphere::smoothstep(l.cone[1], l.cone[0], t.dot(l.direction.normalize()))
        } else if l.housed {
            0.05 + 0.95 * atmosphere::smoothstep(-0.1, 0.3, -t.z)
        } else {
            1.0
        };
        // the lamp's illuminance at the eye (lux): its core's level, falling off with the
        // square of the distance as on the street
        let e_eye = LAMP_E * 1e4 * l.intensity * lum * k * core * core / d2;
        veil += 10.0 * e_eye / theta.max(1.5).powi(2);
    }
    // (the luminance of a mid-grey surface, 18 %, under this illuminance)
    veil * std::f32::consts::PI / 0.18 / 1e4
}

/// A headlamp's intensity towards `t` (unit, from the lamp): lamp_air.wgsl `headlamp`.
pub(crate) fn headlamp_profile(t: Vec3, dir: Vec3, low: bool) -> f32 {
    let fwd = (dir.truncate() + glam::Vec2::new(1e-6, 0.0)).normalize();
    let ahead = t.truncate().dot(fwd);
    if ahead <= 0.0 {
        return 0.0;
    }
    let across = (t.x * fwd.y - t.y * fwd.x).abs() / ahead;
    let wide =
        0.12 * atmosphere::smoothstep(1.0, 0.45, across) + 0.88 * (-across * across / 0.06).exp();
    let drop = -t.z / t.truncate().length().max(1e-3);
    let mut up = (0.06 / drop.abs().max(1e-4)).powf(3.4).min(1.0);
    if low {
        up *= atmosphere::smoothstep(-0.012, 0.025, drop);
        return wide * up;
    }
    let hot = (-across * across / 0.012 - drop * drop / 0.0004).exp();
    wide * up + 6.0 * hot
}

/// The extinction of falling snow (1/m) for a snowfall of strength `s` (0..1): the
/// meteorological visibility (the distance at which a dark object keeps 5 % of its
/// contrast, 3.0 / extinction) is what a snowfall's strength is reported by - light snow
/// over 800 m, moderate down to 400 m, heavy under that (WMO / NWS) - and goes about
/// inversely with the snowfall rate (Rasmussen et al., J. Appl. Meteor. 1999). A
/// snowflake's big, flat cross-section takes several times the view of a raindrop of
/// the same water.
pub(crate) fn snowfall_extinction(s: f32) -> f32 {
    if s <= 0.01 {
        return 0.0;
    }
    let visibility = 300.0 / s.min(1.0).powf(0.9);
    3.0 / visibility
}

/// How much of the sun's light gets down through the enhanced fog layer (extinction
/// `sigma` at its base, thinning by `FOG_FALLOFF`) from a sun `sun_z` (the sine of its
/// altitude) high: the layer's whole depth along the way, as the sky shader dims the
/// sun's disc by it (`air_of` towards the sun).
pub(crate) fn sun_through_fog(sigma: f32, sun_z: f32) -> f32 {
    if sigma <= 0.0 {
        return 1.0;
    }
    (-sigma / (FOG_FALLOFF * sun_z.max(0.02))).exp()
}

/// What the enhanced sky is computed from for this light, and how much of the sun the
/// clouds let through (the sky shader's `lights.w`, which lights the clouds and the disc).
pub(crate) fn enhanced_sky_input(
    lighting: &Lighting,
    city_glow: f32,
) -> (atmosphere::SkyInput, f32) {
    let s = lighting.sun_dir.normalize_or_zero();
    // haze: the weather's visibility below a few kilometres thickens the aerosol
    let visibility = 2.3 / lighting.fog_density.max(1e-6);
    let mut day = day_air(lighting);
    if let Some([haze, angstrom, height]) = lighting.air {
        day.haze = haze;
        day.angstrom = angstrom;
        day.aerosol_height = height;
    }
    let haze = (8000.0 / visibility).clamp(1.0, 6.0) * day.haze + 2.0 * lighting.rain;
    // rain and snow fall from a closed deck: whatever the cloud type says, the sun is
    // gone and the sky is the grey dome (a low sun scattered orange in the snowfall)
    let wet_cover = (lighting.rain * 1.5).clamp(0.0, 1.0);
    let sun_visibility = lighting.sun_intensity.clamp(0.0, 1.0) * (1.0 - wet_cover);
    // The sun the street gets: none from under an overcast deck (from a cover of 0.85 on
    // the sky shader draws the closed grey dome, `closed` in `cloud_layer`), and in a
    // weather fog only what the fog above lets through - its disc in the sky is dimmed by
    // the same layer. With the whole of it, an overcast day lit the street with a sun no
    // one saw, and a dense fog glowed white all round the sun (#1106).
    let closed = atmosphere::smoothstep(0.85, 1.0, lighting.cloud_density);
    let reaching =
        sun_visibility * (1.0 - closed) * sun_through_fog(enhanced_weather_fog(lighting), s.z);
    let input = atmosphere::SkyInput {
        sun_dir: s,
        sun_visibility: reaching,
        overcast: lighting.overcast.clamp(0.0, 1.0).max(wet_cover),
        haze,
        rain: lighting.rain.clamp(0.0, 1.0),
        ground_albedo: 0.2 + 0.45 * lighting.snow.clamp(0.0, 1.0),
        tint: lighting.envir_tint,
        // (rain and mist are water: as grey as the droplets are large)
        angstrom: day.angstrom
            * (1.0 - 0.6 * lighting.rain.clamp(0.0, 1.0))
            * (1.0 - 0.5 * ((haze - 3.0) / 3.0).clamp(0.0, 1.0)),
        aerosol_height: day.aerosol_height,
        strat_aod: day.strat_aod,
        veil: lighting.veil.clamp(0.0, 3.0),
        cumulus: if lighting.enhanced {
            (lighting.cloud_density.min(0.84)) * (1.0 - closed)
        } else {
            0.0
        },
        moon_dir: lighting.moon_dir,
        moon_illum: lighting.moon_illum,
        city_glow,
    };
    // (the disc in the sky: what the veil lets through of it as well)
    let veil_t = (-input.veil / s.z.max(0.03)).exp();
    (input, sun_visibility * veil_t)
}

/// The day's own air: what the sky of one calendar day is made of, as the weather of a
/// real day leaves it (see `day_air`).
pub(crate) struct DayAir {
    /// aerosol amount relative to a clear day
    pub(crate) haze: f32,
    /// its Ångström exponent (fine dry particles 1.4-1.6, humid haze down to 0.5)
    pub(crate) angstrom: f32,
    /// the depth of the hazy boundary layer (m)
    pub(crate) aerosol_height: f32,
    /// the stratospheric aerosol's optical depth
    pub(crate) strat_aod: f32,
}

/// The day's own air, drawn from the calendar day so that no two days look quite alike -
/// and with it no two sunsets: winter's air is mostly clean and dry (a deep blue sky,
/// crisp distances, a pale yellow low sun) under a shallow inversion, a summer's often
/// hazy and humid (a milky sky, soft distances) and mixed high by the afternoon's heat,
/// and the far north cleaner than the middle of the continent. The boundary layer follows
/// the sun through the day as a real one does: low and dense in the morning (a pastel
/// sunrise through a thin bright haze), growing with the sun's heat until early afternoon
/// and left standing as the evening's residual layer (a low sun shining through all of
/// it). Now and then the stratosphere holds more aerosol than usual, and that evening's
/// twilight turns purple. `OMSI_DAY_AIR=haze,angstrom[,height,strat]` fixes it.
pub(crate) fn day_air(lighting: &Lighting) -> DayAir {
    static FIXED: std::sync::OnceLock<Option<Vec<f32>>> = std::sync::OnceLock::new();
    let fixed = FIXED.get_or_init(|| {
        let v = omsi_cfg::env::var("OMSI_DAY_AIR").ok()?;
        Some(
            v.split(',')
                .filter_map(|x| x.trim().parse::<f32>().ok())
                .collect(),
        )
    });
    let hash = |k: u32| {
        let mut x = lighting.day_seed.wrapping_mul(0x9E37_79B9) ^ k.wrapping_mul(0x85EB_CA6B);
        x ^= x >> 16;
        x = x.wrapping_mul(0x7FEB_352D);
        x ^= x >> 15;
        x = x.wrapping_mul(0x846C_A68B);
        x ^= x >> 16;
        (x & 0xFF_FFFF) as f32 / 16_777_216.0
    };
    // 0 in midwinter, 1 in high summer (half a year later south of the equator)
    let doy = lighting.day_of_year + if lighting.latitude < 0.0 { 182.0 } else { 0.0 };
    let summer = 0.5 - 0.5 * (2.0 * std::f32::consts::PI * (doy - 20.0) / 365.0).cos();
    let north = 1.0 - 0.3 * atmosphere::smoothstep(55.0, 70.0, lighting.latitude.abs());
    let snow = lighting.snow.clamp(0.0, 1.0);
    let base = (0.72 + 0.6 * summer) * north * (1.0 - 0.25 * snow);
    let mut haze = (base * ((hash(1) - 0.5) * 1.1).exp()).clamp(0.35, 3.0);
    let mut angstrom = (1.45 - 0.65 * summer * hash(2) - 0.15 * hash(3)).clamp(0.5, 1.6);
    // the day's deepest mixing (m): a few hundred metres in winter, up to two kilometres
    // on a hot summer afternoon
    let deepest = (500.0 + 1500.0 * summer) * (0.7 + 0.6 * hash(4));
    // how far the day has mixed it: by the sun's height in the morning (azimuth east of
    // south), all of it from the afternoon on (the residual layer stays into the night)
    let sun_alt = lighting
        .sun_dir
        .normalize_or_zero()
        .z
        .max(0.0)
        .asin()
        .to_degrees();
    let morning = lighting.sun_azimuth < std::f32::consts::PI;
    let grown = if morning {
        0.35 + 0.65 * atmosphere::smoothstep(0.0, 35.0, sun_alt)
    } else {
        1.0
    };
    let mut aerosol_height = (deepest * grown).max(250.0);
    // mostly a clean stratosphere; one day in a dozen or so a vivid one
    let mut strat_aod = 0.002 + 0.004 * hash(5) + 0.03 * hash(6).powi(8);
    if let Some(f) = fixed {
        haze = f.first().copied().unwrap_or(haze);
        angstrom = f.get(1).copied().unwrap_or(angstrom);
        aerosol_height = f.get(2).copied().unwrap_or(aerosol_height);
        strat_aod = f.get(3).copied().unwrap_or(strat_aod);
    }
    DayAir {
        haze,
        angstrom,
        aerosol_height,
        strat_aod,
    }
}

/// Has the sky moved on far enough from `a` to be computed again? The sun by a tenth of a
/// degree (half a minute of the day), the weather by a per cent.
pub(crate) fn sky_input_differs(a: &atmosphere::SkyInput, b: &atmosphere::SkyInput) -> bool {
    let near = |x: f32, y: f32, tol: f32| (x - y).abs() <= tol;
    a.sun_dir.dot(b.sun_dir) < 0.999_998
        || !near(a.sun_visibility, b.sun_visibility, 0.01)
        || !near(a.overcast, b.overcast, 0.01)
        || !near(a.haze, b.haze, 0.02)
        || !near(a.rain, b.rain, 0.01)
        || !near(a.ground_albedo, b.ground_albedo, 0.01)
        || !near(a.angstrom, b.angstrom, 0.02)
        || !near(a.aerosol_height, b.aerosol_height, 40.0)
        || !near(a.strat_aod, b.strat_aod, 0.0005)
        || !near(a.veil, b.veil, 0.01)
        || !near(a.cumulus, b.cumulus, 0.02)
        || a.moon_dir.dot(b.moon_dir) < 0.9999
        || !near(a.moon_illum, b.moon_illum, 0.01)
        || !near(a.city_glow, b.city_glow, 0.01)
        || a.tint
            .iter()
            .zip(&b.tint)
            .any(|(x, y)| (*x - *y).abs().max_element() > 0.01)
}

/// `OMSI_DEBUG_EXPOSURE=1`: the enhanced path's exposure as it adapts - the light model's
/// pre-exposure, the metered picture and the correction the tone mapping applies, logged
/// about four times a second (the metered value lives on the GPU and is read back).
pub(crate) struct ExposureLog {
    pub(crate) buf: wgpu::Buffer,
    pub(crate) ready: Arc<std::sync::atomic::AtomicBool>,
    pub(crate) waiting: bool,
    pub(crate) frame: u64,
    pub(crate) started: std::time::Instant,
    /// the pre-exposure (log2) and the metering settings of the frame being read back
    pub(crate) pending: (f32, [f32; 6]),
    /// The tone mapping's metering correction last read back (EV). Self-lit surfaces (a
    /// display's text, a script texture) are drawn this much brighter or darker in advance,
    /// so that they come out at their own brightness whatever the metering does to the
    /// rest of the picture: a destination display was darkened with a sunlit street and
    /// hardly readable by day.
    pub(crate) ev: f32,
    /// `OMSI_DEBUG_EXPOSURE`: log what is read.
    pub(crate) log: bool,
}

impl ExposureLog {
    pub(crate) fn new(device: &wgpu::Device) -> Option<ExposureLog> {
        let buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("exposure readback"),
            size: 256,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        Some(ExposureLog {
            buf,
            ready: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            waiting: false,
            frame: 0,
            started: std::time::Instant::now(),
            pending: (0.0, [0.0; 6]),
            ev: 0.0,
            log: omsi_cfg::env::var_os("OMSI_DEBUG_EXPOSURE").is_some(),
        })
    }

    pub(crate) fn sample(
        &mut self,
        encoder: &mut wgpu::CommandEncoder,
        adapted: &wgpu::TextureView,
        pre_log2: f32,
        meter: [f32; 6],
    ) {
        self.frame += 1;
        if self.waiting && self.ready.swap(false, std::sync::atomic::Ordering::Relaxed) {
            {
                let view = self.buf.slice(0..8).get_mapped_range();
                let bits = u16::from_le_bytes([view[0], view[1]]);
                let metered = half_to_f32(bits);
                let (pre, m) = self.pending;
                let ev = ((m[1] - metered) * m[0]).clamp(-m[2], m[3]) + m[4];
                if ev.is_finite() {
                    self.ev = ev;
                }
                if self.log {
                    log::info!("exposure t={:.2}s: light model {:+.2} EV, metered picture log2 {:+.2}, correction {:+.2} EV, total {:+.2} EV", self.started.elapsed().as_secs_f32(), pre, metered, ev, pre + ev);
                }
            }
            self.buf.unmap();
            self.waiting = false;
        }
        if self.waiting || self.frame % 8 != 0 {
            return;
        }
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: adapted.texture(),
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &self.buf,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        self.pending = (pre_log2, meter);
        let ready = self.ready.clone();
        // mapped once the frame's commands are submitted (see `render_inner`)
        self.waiting = true;
        encoder.map_buffer_on_submit(&self.buf, wgpu::MapMode::Read, .., move |r| {
            ready.store(r.is_ok(), std::sync::atomic::Ordering::Relaxed)
        });
    }
}

pub(crate) fn half_to_f32(b: u16) -> f32 {
    let sign = if b & 0x8000 != 0 { -1.0 } else { 1.0 };
    let e = ((b >> 10) & 0x1f) as i32;
    let m = (b & 0x3ff) as f32;
    match e {
        0 => sign * m / 1024.0 * 2f32.powi(-14),
        31 => sign * f32::INFINITY,
        _ => sign * (1.0 + m / 1024.0) * 2f32.powi(e - 15),
    }
}
