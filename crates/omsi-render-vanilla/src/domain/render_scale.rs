//! The render scale of the "DirectX9 (D3D9Ex)" renderer: the 3D picture is drawn at a fraction
//! of the window's size and scaled up to it, with the interface drawn at the window's size over
//! it. This module holds the arithmetic, which needs no device.
//!
//! The option is a performance feature and a *deliberate deviation* from OMSI 2, which draws
//! at the window's size: it is off by default and, at 100 %, the renderer takes exactly the
//! path it takes without the option (no extra target, no extra pass).

/// The smallest fraction the picture is drawn at.
pub const MIN_SCALE: f32 = 0.5;

/// How the smaller picture is brought up to the window's size.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Upscale {
    /// The card's bilinear filter: one texture read per pixel, soft.
    #[default]
    Bilinear,
    /// AMD's FidelityFX Super Resolution 1 (edge-adaptive upsampling, then a sharpening pass),
    /// adapted to Shader Model 3.0 (see `shaders/upscale.hlsl` and `THIRD_PARTY.md`).
    Fsr1,
}

/// The fraction to draw at for a requested `scale`: `None` when the picture is to be drawn at
/// the window's size (a scale at or above 99.5 %, or one that is no number or not positive:
/// "auto"), else the scale held between [`MIN_SCALE`] and 1.
pub fn effective_scale(scale: f32) -> Option<f32> {
    if !scale.is_finite() || scale <= 0.0 || scale >= 0.995 {
        return None;
    }
    Some(scale.max(MIN_SCALE))
}

/// The size of the picture drawn for a `width` x `height` window at `scale` (rounded to the
/// nearest pixel, never zero, never beyond the window's).
pub fn scaled_size(width: u32, height: u32, scale: f32) -> (u32, u32) {
    let side = |n: u32| (((n as f32) * scale).round() as u32).clamp(1, n.max(1));
    (side(width), side(height))
}

/// The constants the edge-adaptive upsampling needs for an `input` picture scaled up to
/// `output`: how an output pixel maps to the input (`xy` the ratio, `zw` the offset that puts
/// pixel centres on pixel centres). The same as `FsrEasuCon`'s first constant of AMD's
/// reference.
pub fn easu_mapping(input: (u32, u32), output: (u32, u32)) -> [f32; 4] {
    let (iw, ih) = (input.0.max(1) as f32, input.1.max(1) as f32);
    let (ow, oh) = (output.0.max(1) as f32, output.1.max(1) as f32);
    [iw / ow, ih / oh, 0.5 * iw / ow - 0.5, 0.5 * ih / oh - 0.5]
}

/// The sharpening pass's strength in AMD's unit (stops of reduction from the maximum; 0 is
/// the sharpest). 0.25 is the value their integration notes suggest as a default.
pub const RCAS_STOPS: f32 = 0.25;

/// `RCAS_STOPS` as the linear factor the pass multiplies its lobe by.
pub fn rcas_factor() -> f32 {
    (-RCAS_STOPS).exp2()
}
