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
