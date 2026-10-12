//! The air between the sun and the ground: the sun and moon discs, cloud shade, fog and
//! snowfall, and the sky model's input from the game's `Lighting`.

use super::{atmosphere, clouds};
use crate::lighting::Lighting;
use crate::CLOUD_ORIGIN_PERIOD;
use glam::{DVec3, Vec3};

/// The sun's angular radius as drawn (a little larger than the real 0.27°).
pub(crate) const SUN_RADIUS: f32 = 0.0065;

/// The moon's, as drawn (the real one is as large as the sun's; a little more, as the
/// eye sees it).
pub(crate) const MOON_RADIUS: f32 = 0.0062;

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
