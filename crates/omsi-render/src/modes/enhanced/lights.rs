//! The lamps in the Enhanced picture: which it draws, and the colour its passes add.

use crate::lighting::{gpu_light, lights_up, GpuPointLight, LightMode, PointLight};
use glam::Vec3;

/// Does Enhanced draw the light? Its own lights and the shared ones; a vehicle's headlight
/// stand-in for the plain shader is left out (Enhanced has the real spot light).
pub fn draws(l: &PointLight) -> bool {
    lights_up(l) && l.mode != LightMode::Vanilla
}

/// A light's colour as the Enhanced picture adds it: the content gives it as sRGB numbers
/// (a `[spotlight]`'s 0..255, a `[light_enh]`'s or a map light's 0..1), and the Enhanced
/// passes add light in linear terms. Taken as it stands, every channel under the brightest
/// one came out lighter on the screen than the content asks: an amber lamp turned a pale
/// cream. Decoded, then scaled back to the luminance the numbers had, so the light keeps
/// its strength and only its hue and saturation change.
pub fn linear_light_colour(c: [f32; 3]) -> [f32; 3] {
    let decode = |x: f32| {
        let x = x.max(0.0);
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    let luma = |c: [f32; 3]| 0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2];
    let lin = c.map(decode);
    let (before, after) = (luma(c.map(|x| x.max(0.0))), luma(lin));
    if after <= 1e-6 {
        return lin;
    }
    lin.map(|x| x * before / after)
}

/// The light as the Enhanced shaders read it: the common record with the colour in linear terms.
pub fn record(l: &PointLight, p: Vec3) -> GpuPointLight {
    let mut g = gpu_light(l, p);
    let c = linear_light_colour(l.color);
    g.color = [c[0], c[1], c[2], l.intensity];
    g
}
