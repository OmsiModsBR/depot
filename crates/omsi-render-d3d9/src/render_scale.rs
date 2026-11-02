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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_and_full_size_draw_at_the_window_size() {
        for s in [0.0, -1.0, f32::NAN, f32::INFINITY, 1.0, 0.995, 1.7] {
            assert_eq!(effective_scale(s), None, "{s}");
        }
    }

    #[test]
    fn scales_are_held_to_the_range() {
        assert_eq!(effective_scale(0.75), Some(0.75));
        assert_eq!(effective_scale(0.5), Some(0.5));
        assert_eq!(effective_scale(0.3), Some(MIN_SCALE));
    }

    #[test]
    fn scaled_sizes_round_and_are_never_zero_or_larger() {
        assert_eq!(scaled_size(1920, 1080, 0.5), (960, 540));
        assert_eq!(scaled_size(1920, 1080, 0.75), (1440, 810));
        assert_eq!(scaled_size(1366, 768, 0.66), (902, 507));
        assert_eq!(scaled_size(1, 1, 0.5), (1, 1));
        assert_eq!(scaled_size(3, 1, 0.5), (2, 1));
        assert_eq!(scaled_size(0, 0, 0.5), (1, 1));
        let (w, h) = scaled_size(7, 5, 0.99);
        assert!(w <= 7 && h <= 5);
    }

    #[test]
    fn the_mapping_puts_pixel_centres_on_pixel_centres() {
        // an output pixel centre at x + 0.5 lands on the input's x * r + 0.5 * r - 0.5 + ...:
        // for 2x, output 0 (centre 0.5) maps to input -0.25 (between input 0's centre 0.5 and
        // the edge), output 1 to 0.25
        let m = easu_mapping((4, 4), (8, 8));
        assert_eq!(m, [0.5, 0.5, -0.25, -0.25]);
        assert_eq!(0.0 * m[0] + m[2], -0.25);
        assert_eq!(1.0 * m[0] + m[2], 0.25);
    }

    #[test]
    fn the_sharpening_factor_is_below_full_strength() {
        assert!((rcas_factor() - 0.25f32.exp2().recip()).abs() < 1e-6);
        assert!(rcas_factor() < 1.0 && rcas_factor() > 0.5);
    }
}
