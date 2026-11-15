//! The wet road: what the device's pass of that name (`d3d9::wet_road`) draws, worked out
//! without a device. Under rain a surface whose texture carries `[moisture]` or `[puddles]`
//! darkens, mirrors the sky a little and gathers pools of standing water, as the default
//! renderer's vanilla picture draws it; what the pools mirror besides the sky (houses,
//! vehicles, people) is not part of it yet.

use crate::scene::{DrawKind, SceneLight, SceneMaterial};

/// How wet the outdoor surfaces are in this light (0 dry .. 1 soaked): the weather's
/// wetness, none under snow (the snow covers the road instead).
pub fn wetness(light: &SceneLight) -> f32 {
    light.weather[0].clamp(0.0, 1.0) * (1.0 - light.ambient[3].clamp(0.0, 1.0))
}

/// The colour a wet road mirrors (as stored): half the light from above and a third of the
/// sun's, as the default renderer's vanilla wet road takes it.
pub fn sheen(light: &SceneLight) -> [f32; 3] {
    std::array::from_fn(|c| light.sky_color[c] * 0.5 + light.sun_color[c] * light.sun_dir[3] * 0.35)
}

/// Whether a batch of this kind with this material gets the wet road: a surface with
/// `[moisture]` or `[puddles]` (the material's uniform vector 3, z) that the scene drew in
/// colour; not its depth-only part, nor a painted ground layer, whose ground below takes
/// the wet look already.
pub fn wets(kind: DrawKind, material: &SceneMaterial<'_>) -> bool {
    matches!(
        kind,
        DrawKind::Opaque | DrawKind::AlphaTest | DrawKind::Blend
    ) && material.uniform.get(3).is_some_and(|v| v[2] > 0.0)
}
