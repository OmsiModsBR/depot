//! Easing curves.

/// Hermite step from 0 at `e0` to 1 at `e1` (as WGSL's `smoothstep`).
pub fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothstep_is_clamped_and_symmetric() {
        assert_eq!(smoothstep(1.0, 3.0, 0.0), 0.0);
        assert_eq!(smoothstep(1.0, 3.0, 2.0), 0.5);
        assert_eq!(smoothstep(1.0, 3.0, 9.0), 1.0);
        assert_eq!(smoothstep(0.0, 1.0, 0.25), 1.0 - smoothstep(0.0, 1.0, 0.75));
    }
}
