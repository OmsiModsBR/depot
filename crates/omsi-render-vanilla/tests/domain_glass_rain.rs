use omsi_render_vanilla::testing::domain::glass_rain::*;

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
