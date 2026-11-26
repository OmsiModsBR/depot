use omsi_render_vanilla::testing::d3d9::smoke::*;

#[test]
fn the_light_on_a_particle_is_ambient_and_a_share_of_the_sun_capped() {
    let l = particle_light([0.2, 0.3, 0.4, 1.0], [1.0, 0.5, 0.0, 1.0]);
    assert!((l[0] - 0.8).abs() < 1e-6 && (l[1] - 0.6).abs() < 1e-6 && (l[2] - 0.4).abs() < 1e-6);
    // never brighter than 1.2, however bright the day
    assert_eq!(particle_light([1.0; 4], [1.0; 4])[..3], [LIGHT_MAX; 3]);
    // at night only the ambient
    assert_eq!(particle_light([0.1; 4], [0.0; 4])[..3], [0.1; 3]);
}
