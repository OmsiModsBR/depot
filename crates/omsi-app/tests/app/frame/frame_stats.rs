use openomsi_game::testing::frame_stats::FrameStats;

#[test]
fn steady_frames_have_the_same_low_as_the_average() {
    let mut s = FrameStats::default();
    for _ in 0..300 {
        s.push(1.0 / 60.0);
    }
    assert!((s.low().unwrap() - 60.0).abs() < 0.5);
}

#[test]
fn a_few_slow_frames_pull_the_low_down() {
    let mut s = FrameStats::default();
    for i in 0..600 {
        s.push(if i % 100 == 0 { 0.05 } else { 1.0 / 60.0 });
    }
    // six 50 ms frames of 600: the worst 1 % (6 frames) average 20 fps
    assert!((s.low().unwrap() - 20.0).abs() < 0.5);
}

#[test]
fn too_few_frames_say_nothing() {
    let mut s = FrameStats::default();
    for _ in 0..10 {
        s.push(0.016);
    }
    assert!(s.low().is_none());
}

#[test]
fn a_long_stall_is_not_a_frame() {
    let mut s = FrameStats::default();
    for _ in 0..100 {
        s.push(1.0 / 60.0);
    }
    s.push(5.0);
    assert!((s.low().unwrap() - 60.0).abs() < 0.5);
}

#[test]
fn only_the_last_seconds_count() {
    let mut s = FrameStats::default();
    for _ in 0..200 {
        s.push(0.1);
    }
    for _ in 0..1200 {
        s.push(1.0 / 60.0);
    }
    assert!(s.low().unwrap() > 50.0);
}
