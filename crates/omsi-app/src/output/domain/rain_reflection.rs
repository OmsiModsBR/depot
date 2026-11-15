//! The street the puddles of a wet road mirror under "DirectX9 (D3D9Ex)": the scene drawn
//! into a small picture from the window's camera mirrored under the road, which the device's
//! wet road pass lays over the puddles (the options "rain reflections" and "people in rain
//! reflections").

use omsi_render::{Camera, Lighting};

/// The picture's size (width, height): small and fixed, whatever the window's size or the
/// real-time reflections' option.
pub const RAIN_REFLECTION_SIZE: (u32, u32) = (128, 256);

/// Below this wetness the road is too dry for its puddles to show anything.
const WET_ENOUGH: f32 = 0.01;

/// The camera seen in a level water surface at height `ground`: below it by as much as it
/// is above, looking up by as much as it looks down. What it sees through a spot of that
/// surface is what the surface mirrors there.
pub fn mirrored_camera(camera: &Camera, ground: f64) -> Camera {
    let mut position = camera.position;
    position.z = 2.0 * ground - position.z;
    Camera {
        position,
        pitch: -camera.pitch,
        roll: -camera.roll,
        // (as close as the mirrors' cameras reach)
        near: 0.1,
        ..*camera
    }
}

/// The mirrored camera, when the picture is wanted this frame: the option on (`humans` is
/// [`crate::settings::Settings::rain_reflection`]), the road wet and not under snow, and
/// the road's height known.
pub fn rain_reflection_camera(
    humans: Option<bool>,
    lighting: &Lighting,
    view: &Camera,
) -> Option<Camera> {
    humans?;
    let wet = lighting.wetness.clamp(0.0, 1.0) * (1.0 - lighting.snow.clamp(0.0, 1.0));
    if wet < WET_ENOUGH {
        return None;
    }
    lighting.puddle_ground.map(|h| mirrored_camera(view, h))
}

/// The light the picture is drawn in: the window's, without the ground (the camera is under
/// it), the shadows, the objects too small for the picture, and the people when the option
/// leaves them out.
pub fn rain_reflection_lighting(lighting: &Lighting, humans: bool) -> Lighting {
    let mut l = lighting.clone();
    l.ground_hidden = true;
    l.humans_hidden = !humans;
    l.shadows = false;
    let px = RAIN_REFLECTION_SIZE.0.max(RAIN_REFLECTION_SIZE.1) as f32;
    l.min_obj_size = l.min_obj_size.max((12.0 / px).clamp(0.03, 0.09));
    l
}
