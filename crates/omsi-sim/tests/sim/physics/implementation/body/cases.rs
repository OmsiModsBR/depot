/// The body's pitch and roll are damped as much a second at any frame rate: as OMSI
/// damps them at 30 frames a second (a 12 t bus on 240-280 kN/m springs).
#[test]
fn body_damping_does_not_depend_on_the_frame_rate() {
    let f = ((2.0 * 240.0 + 2.0 * 280.0) / 12.0f32).sqrt();
    let second = |fps: f32| {
        (0..fps as usize).fold(1.0f32, |k, _| {
            k * omsi_sim::testing::physics::implementation::body::body_damping(f, 1.0 / fps)
        })
    };
    let (a, b, c) = (second(30.0), second(60.0), second(144.0));
    assert!((a - b).abs() < 1e-3 && (a - c).abs() < 1e-3, "{a} {b} {c}");
    assert!(a > 0.2 && a < 0.5, "{a}");
}
