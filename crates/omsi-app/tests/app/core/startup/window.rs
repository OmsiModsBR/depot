use openomsi_game::testing::core::startup::software_adapter;

/// The software renderers come after every graphics chip (#770).
#[test]
fn software_renderers_are_told_from_chips() {
    use wgpu::DeviceType::*;
    assert!(software_adapter("Microsoft Basic Render Driver", Cpu));
    assert!(software_adapter("Microsoft Basic Render Driver", Other));
    assert!(software_adapter("llvmpipe (LLVM 15.0.7, 256 bits)", Cpu));
    assert!(!software_adapter(
        "Intel(R) HD Graphics 2500",
        IntegratedGpu
    ));
    assert!(!software_adapter(
        "NVIDIA GeForce RTX 3050 Laptop GPU",
        DiscreteGpu
    ));
}

#[test]
fn the_window_fits_a_small_screen_and_sits_in_its_middle() {
    // 1920 x 1200 at 125 %: 1600 x 900 points would be 2000 px wide
    let ((w, h), (x, y)) =
        openomsi_game::testing::core::startup::fit_rect((1600.0, 900.0), (1920.0, 1200.0), 1.25);
    assert!(
        w * 1.25 <= 1920.0 * 0.9 + 1.0 && h * 1.25 <= 1200.0 * 0.85 + 1.0,
        "{w} x {h}"
    );
    assert!(((w / h) - 16.0 / 9.0).abs() < 0.01);
    assert!((x as f64 - (1920.0 - w * 1.25) / 2.0).abs() <= 1.0);
    assert!(y > 0);
    // a big screen keeps the size asked for
    let ((w, h), _) =
        openomsi_game::testing::core::startup::fit_rect((1600.0, 900.0), (3840.0, 2160.0), 1.5);
    assert_eq!((w, h), (1600.0, 900.0));
}
