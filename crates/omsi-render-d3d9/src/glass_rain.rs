//! Raindrops on glass: what a pane with a `[matl_raindropmap]` shows while it rains, and
//! what the wipers clear. Nothing here names a graphics API; the Direct3D 9 device draws it
//! (`d3d9::glass_rain`) from the state kept here and the drops' picture made here.
//!
//! The drops gather across the pane: how far they have formed is kept at [`BINS`] places
//! along its width (the texture coordinate u, which is also the direction the wiper sweeps),
//! each between 0 (clean) and 1 (fully covered). Rain builds them up, faster the harder it
//! rains, to a ceiling that also grows with the rain; without rain they dry in about a
//! minute; a wiper that moves wipes the places it passes over clean, and they gather again
//! afterwards.

/// Places along a pane's width the state is kept at (the pixel shader's constants hold them,
/// four to a vector).
pub const BINS: usize = 32;

/// Longest step of one picture, in seconds (a long stall does not wet or dry a pane at once).
const MAX_STEP: f32 = 0.25;
/// How fast drops gather, per second: a little in a drizzle, much in a downpour.
const BUILD_BASE: f32 = 0.05;
const BUILD_RAIN: f32 = 0.45;
/// How far they gather: light rain leaves the pane part clear.
const CEILING_BASE: f32 = 0.35;
const CEILING_RAIN: f32 = 0.65;
/// A dry pane dries in about a minute.
const DRY_PER_SECOND: f32 = 1.0 / 60.0;

/// What a picture says about the weather and the wiper.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct SceneRain {
    /// How hard it rains, 0..1 (0 when it does not, and while it snows).
    pub intensity: f32,
    /// Where the wiper of the vehicle the player drives is, 0..1 across its sweep; `None`
    /// when it has none or does not publish it.
    pub wiper: Option<f32>,
    /// Seconds since the last picture of the window. 0 for a picture that must not move the
    /// state on (a mirror's, or a second picture in one frame).
    pub seconds: f32,
}

/// How far the drops have formed along a pane, and where the wiper was.
#[derive(Debug, Clone, PartialEq)]
pub struct GlassRain {
    bins: [f32; BINS],
    wiper: Option<f32>,
}

impl Default for GlassRain {
    fn default() -> Self {
        Self {
            bins: [0.0; BINS],
            wiper: None,
        }
    }
}

impl GlassRain {
    /// The state after a picture: the rain builds drops up or they dry, then the wiper's
    /// stroke since the last picture wipes the places it passed over.
    pub fn step(&mut self, rain: &SceneRain) {
        let dt = rain.seconds.clamp(0.0, MAX_STEP);
        let wiper = rain
            .wiper
            .filter(|w| w.is_finite())
            .map(|w| w.clamp(0.0, 1.0));
        if dt > 0.0 {
            let r = rain.intensity.clamp(0.0, 1.0);
            let ceiling = if r > 0.0 {
                CEILING_BASE + CEILING_RAIN * r
            } else {
                0.0
            };
            for b in &mut self.bins {
                if r > 0.0 && *b < ceiling {
                    *b = (*b + dt * (BUILD_BASE + BUILD_RAIN * r)).min(ceiling);
                } else if r > 0.0 {
                    // (a pane wetter than this rain makes it dries down to the ceiling)
                    *b = (*b - dt * DRY_PER_SECOND).max(ceiling);
                } else {
                    *b = (*b - dt * DRY_PER_SECOND).max(0.0);
                }
            }
            if let (Some(from), Some(to)) = (self.wiper, wiper) {
                self.wipe(from.min(to), from.max(to));
            }
            self.wiper = wiper;
        }
    }

    /// Wipes the places between `lo` and `hi` (0..1 across the pane) clean, those half a
    /// place beyond them included. A wiper that did not move wipes nothing.
    fn wipe(&mut self, lo: f32, hi: f32) {
        if hi - lo < 1e-4 {
            return;
        }
        let half = 0.5 / (BINS - 1) as f32;
        for (k, b) in self.bins.iter_mut().enumerate() {
            let u = k as f32 / (BINS - 1) as f32;
            if u >= lo - half && u <= hi + half {
                *b = 0.0;
            }
        }
    }

    /// How far the drops have formed at each place.
    pub fn bins(&self) -> &[f32; BINS] {
        &self.bins
    }

    /// No drop anywhere: nothing to draw.
    pub fn is_dry(&self) -> bool {
        self.bins.iter().all(|&b| b <= 0.0)
    }
}

/// The side of the drops' picture, in pixels.
pub const DROPS_SIDE: u32 = 128;
/// Cells of the drops' picture a side: one drop in each.
const CELLS: u32 = 4;

/// The picture the drops are drawn from, RGBA, `DROPS_SIDE` square, made to tile: in every
/// cell of a grid one drop, a little taller than wide, at a place and of a size the cell's
/// number decides. Red is the drop's shape (0 outside, soft at the edge), green when it
/// forms (a pane's covering has to pass it: the drops appear one by one as the rain builds
/// up), blue how it catches the light (a dark rim and a bright lower side). Alpha is 1.
pub fn drops_picture() -> Vec<u8> {
    let side = DROPS_SIDE as usize;
    let cell = side / CELLS as usize;
    let mut rgba = vec![0u8; side * side * 4];
    let mut state = 0x2545_f491_4f6c_dd1du64;
    let mut next = move || {
        // xorshift64*
        state ^= state >> 12;
        state ^= state << 25;
        state ^= state >> 27;
        (state.wrapping_mul(0x2545_f491_4f6c_dd1d) >> 40) as f32 / (1u64 << 24) as f32
    };
    for cy in 0..CELLS as usize {
        for cx in 0..CELLS as usize {
            let (rx, ry) = (
                cell as f32 * (0.14 + 0.1 * next()),
                cell as f32 * (0.18 + 0.12 * next()),
            );
            // (the whole drop lies in its cell: the picture tiles without a cut)
            let px = (cx * cell) as f32 + rx + 1.0 + (cell as f32 - 2.0 * rx - 2.0) * next();
            let py = (cy * cell) as f32 + ry + 1.0 + (cell as f32 - 2.0 * ry - 2.0) * next();
            let forms = 0.05 + 0.9 * next();
            for y in 0..cell {
                for x in 0..cell {
                    let (gx, gy) = ((cx * cell + x) as f32 + 0.5, (cy * cell + y) as f32 + 0.5);
                    let (dx, dy) = ((gx - px) / rx, (gy - py) / ry);
                    let d = (dx * dx + dy * dy).sqrt();
                    let shape = ((1.0 - d) * 4.0).clamp(0.0, 1.0);
                    if shape <= 0.0 {
                        continue;
                    }
                    // dark at the rim, bright on the lower side (the drop bends the light
                    // of the street into it)
                    let shade = (0.30 + 0.45 * d.powi(2) * (1.0 - d) * 4.0 + 0.35 * dy.max(0.0))
                        .clamp(0.0, 1.0);
                    let o = ((cy * cell + y) * side + cx * cell + x) * 4;
                    rgba[o..o + 4].copy_from_slice(&[
                        (shape * 255.0) as u8,
                        (forms * 255.0) as u8,
                        (shade * 255.0) as u8,
                        255,
                    ]);
                }
            }
        }
    }
    // (alpha 1 everywhere, outside the drops too)
    for px in rgba.chunks_exact_mut(4) {
        px[3] = 255;
    }
    rgba
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rain(intensity: f32, wiper: Option<f32>, seconds: f32) -> SceneRain {
        SceneRain {
            intensity,
            wiper,
            seconds,
        }
    }

    fn cover(g: &GlassRain) -> f32 {
        g.bins().iter().sum::<f32>() / BINS as f32
    }

    #[test]
    fn a_pane_starts_dry_and_stays_so_without_rain() {
        let mut g = GlassRain::default();
        assert!(g.is_dry());
        for _ in 0..100 {
            g.step(&rain(0.0, None, 0.1));
        }
        assert!(g.is_dry());
    }

    #[test]
    fn rain_builds_drops_up_and_harder_rain_faster_and_further() {
        let mut light = GlassRain::default();
        let mut heavy = GlassRain::default();
        for _ in 0..10 {
            light.step(&rain(0.2, None, 0.1));
            heavy.step(&rain(1.0, None, 0.1));
        }
        assert!(!light.is_dry());
        assert!(cover(&heavy) > cover(&light));
        for _ in 0..600 {
            light.step(&rain(0.2, None, 0.1));
            heavy.step(&rain(1.0, None, 0.1));
        }
        // (at their ceilings: a downpour covers all of it, a drizzle part of it)
        assert!((cover(&heavy) - 1.0).abs() < 1e-5);
        assert!(cover(&light) < 0.6 && cover(&light) > 0.4);
    }

    #[test]
    fn the_drops_dry_in_about_a_minute_when_the_rain_stops() {
        let mut g = GlassRain::default();
        for _ in 0..600 {
            g.step(&rain(1.0, None, 0.1));
        }
        for _ in 0..300 {
            g.step(&rain(0.0, None, 0.1));
        }
        assert!(cover(&g) > 0.4, "{}", cover(&g));
        for _ in 0..400 {
            g.step(&rain(0.0, None, 0.1));
        }
        assert!(g.is_dry());
    }

    #[test]
    fn a_picture_that_must_not_move_the_state_does_not() {
        let mut g = GlassRain::default();
        g.step(&rain(1.0, Some(0.2), 0.0));
        assert!(g.is_dry());
        g.step(&rain(1.0, None, 0.1));
        let kept = g.clone();
        g.step(&rain(0.0, Some(0.9), 0.0));
        assert_eq!(g, kept);
    }

    #[test]
    fn a_stroke_wipes_the_places_it_passes_over_and_they_gather_again() {
        let mut g = GlassRain::default();
        for _ in 0..600 {
            g.step(&rain(1.0, Some(0.0), 0.1));
        }
        assert!((cover(&g) - 1.0).abs() < 1e-5);
        // (the wiper moves from 0 to 0.5: the left half of the pane is wiped)
        g.step(&rain(1.0, Some(0.5), 0.1));
        let bins = g.bins();
        assert!(bins[..15].iter().all(|&b| b == 0.0), "{bins:?}");
        assert!(bins[18..].iter().all(|&b| b > 0.9), "{bins:?}");
        // and the rain brings them back
        for _ in 0..300 {
            g.step(&rain(1.0, Some(0.5), 0.1));
        }
        assert!(bins_all_above(&g, 0.9));
    }

    fn bins_all_above(g: &GlassRain, v: f32) -> bool {
        g.bins().iter().all(|&b| b > v)
    }

    #[test]
    fn a_wiper_that_stays_put_or_first_appears_wipes_nothing() {
        let mut g = GlassRain::default();
        for _ in 0..600 {
            g.step(&rain(1.0, None, 0.1));
        }
        g.step(&rain(1.0, Some(0.7), 0.1));
        assert!(bins_all_above(&g, 0.9));
        g.step(&rain(1.0, Some(0.7), 0.1));
        assert!(bins_all_above(&g, 0.9));
    }

    #[test]
    fn a_stroke_back_wipes_the_same_way() {
        let mut g = GlassRain::default();
        for _ in 0..600 {
            g.step(&rain(1.0, Some(1.0), 0.1));
        }
        g.step(&rain(1.0, Some(0.5), 0.1));
        assert!(g.bins()[20..].iter().all(|&b| b == 0.0));
        assert!(g.bins()[..12].iter().all(|&b| b > 0.9));
    }

    #[test]
    fn a_long_stall_is_one_short_step() {
        let mut g = GlassRain::default();
        g.step(&rain(1.0, None, 60.0));
        assert!(cover(&g) < 0.2);
    }

    #[test]
    fn nonsense_wiper_values_are_ignored_or_clamped() {
        let mut g = GlassRain::default();
        g.step(&rain(1.0, Some(f32::NAN), 0.1));
        g.step(&rain(5.0, Some(-3.0), 0.1));
        assert!(g
            .bins()
            .iter()
            .all(|b| b.is_finite() && (0.0..=1.0).contains(b)));
    }

    #[test]
    fn the_drops_picture_is_a_tile_of_separate_drops() {
        let a = drops_picture();
        assert_eq!(a.len(), (DROPS_SIDE * DROPS_SIDE * 4) as usize);
        assert_eq!(a, drops_picture());
        let drops = a.chunks_exact(4).filter(|p| p[0] > 0).count() as f32;
        let share = drops / (DROPS_SIDE * DROPS_SIDE) as f32;
        assert!((0.05..0.4).contains(&share), "{share}");
        // every cell holds a drop whose centre is solid; its edges do not touch the cell's
        let cell = (DROPS_SIDE / CELLS) as usize;
        let side = DROPS_SIDE as usize;
        for cy in 0..CELLS as usize {
            for cx in 0..CELLS as usize {
                let solid = (0..cell)
                    .flat_map(|y| (0..cell).map(move |x| (x, y)))
                    .filter(|&(x, y)| a[((cy * cell + y) * side + cx * cell + x) * 4] == 255)
                    .count();
                assert!(solid > 4, "cell {cx},{cy}");
                for i in 0..cell {
                    for (x, y) in [(i, 0), (i, cell - 1), (0, i), (cell - 1, i)] {
                        assert_eq!(a[((cy * cell + y) * side + cx * cell + x) * 4], 0);
                    }
                }
            }
        }
        assert!(a.chunks_exact(4).all(|p| p[3] == 255));
    }
}
