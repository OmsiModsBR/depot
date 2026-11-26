use omsi_render_vanilla::testing::d3d9::sky::*;

#[test]
fn the_dome_is_closed_over_the_zenith_and_below_the_horizon() {
    let (v, i) = dome();
    assert_eq!(v.len(), ((RINGS + 1) * (SEGMENTS + 1) * 3) as usize);
    assert_eq!(i.len(), (RINGS * SEGMENTS * 6) as usize);
    assert!(i.iter().all(|&k| usize::from(k) < v.len() / 3));
    // unit directions from a little below the horizon (z < 0) to the zenith
    let z: Vec<f32> = v.chunks(3).map(|p| p[2]).collect();
    assert!(z.iter().cloned().fold(f32::MAX, f32::min) < 0.0);
    assert!((z.iter().cloned().fold(f32::MIN, f32::max) - 1.0).abs() < 1e-6);
    for p in v.chunks(3) {
        let len = (p[0] * p[0] + p[1] * p[1] + p[2] * p[2]).sqrt();
        assert!((len - 1.0).abs() < 1e-5);
    }
}
