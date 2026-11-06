//! Coronas and smoke sprites: their textures and their per-frame lists.

use super::model::{GpuCorona, SmokeParticle};
use glam::DVec3;

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

/// How far from the eye the sun's glare is laid out (m), towards the sun: its sizes below
/// are in metres at this distance.
pub const SUN_GLARE_DISTANCE: f64 = 10.0;
/// The glare's lamp at full strength: its size (the glow's diameter, m at
/// [`SUN_GLARE_DISTANCE`]) and its brightness, the numbers a `[light_enh_2]` would give it.
/// The star (`Texture\light_effect1.bmp`) is drawn as a lamp with effect bit 1 draws its
/// own: 2.5 times the size, growing with the brightness.
pub const SUN_GLARE_SIZE: f32 = 15.0;
pub const SUN_GLARE_BRIGHTNESS: f32 = 1.5;

impl super::Lighting {
    /// How strongly the sun's glare shows (0 none .. 1): the daylight and the sun's own
    /// strength (none at night or under an overcast cover), faded out as the sun goes below
    /// the horizon, thinned by the cloud cover and gone in a thick fog. 0 with the `sun_glow`
    /// setting off and in Enhanced graphics, whose high-range picture has its own glare.
    pub fn sun_glare_strength(&self) -> f32 {
        if !self.sun_glow || self.enhanced {
            return 0.0;
        }
        let dir = self.sun_dir.normalize_or_zero();
        let day = 1.0 - self.night.clamp(0.0, 1.0);
        let sun = self.sun_intensity.clamp(0.0, 1.0);
        let horizon = ((dir.z + 0.03) / 0.08).clamp(0.0, 1.0);
        // (a closed deck, the Overcast types, hides the sun whatever strength the light carries)
        let cover = self.cloud_density.clamp(0.0, 1.0);
        let closed = ((cover - 0.8) / 0.2).clamp(0.0, 1.0);
        let clouds = (1.0 - 0.6 * cover) * (1.0 - closed * closed * (3.0 - 2.0 * closed));
        let fog = 1.0 - (self.fog_density * 150.0 - 0.15).clamp(0.0, 1.0);
        let k = day * sun * horizon * clouds * fog;
        if k.is_finite() {
            k
        } else {
            0.0
        }
    }

    /// The sun's glare, seen from `eye`: a lamp [`SUN_GLARE_DISTANCE`] away towards the sun,
    /// its glow (`glow`, the picture of a light with none of its own) and its star (`star`)
    /// both turned to the viewer, in the sun's colour, their size and brightness the
    /// strength's share of [`SUN_GLARE_SIZE`] and [`SUN_GLARE_BRIGHTNESS`]. Drawn at the sky's
    /// depth: what stands in front of the sun covers it, glass lets it through. Nothing when
    /// the strength is (near) 0.
    pub fn sun_glare(&self, eye: DVec3, glow: u16, star: u16) -> Vec<super::Corona> {
        let k = self.sun_glare_strength();
        let dir = self.sun_dir.normalize_or_zero();
        if !(k > 0.002) || dir == glam::Vec3::ZERO {
            return Vec::new();
        }
        let size = SUN_GLARE_SIZE * k;
        let lamp = super::Corona {
            position: eye + dir.as_dvec3() * SUN_GLARE_DISTANCE,
            size: size * 0.5,
            color: self
                .sun_color
                .clamp(glam::Vec3::ZERO, glam::Vec3::ONE)
                .to_array(),
            brightness: SUN_GLARE_BRIGHTNESS * k,
            direction: glam::Vec3::ZERO,
            cone_cos: -1.0,
            rotating: 2,
            z_offset: 0.0,
            texture: glow,
            sky: true,
            ..Default::default()
        };
        vec![
            lamp,
            super::Corona {
                size: size * 1.25,
                flags: 8,
                texture: star,
                ..lamp
            },
        ]
    }
}
