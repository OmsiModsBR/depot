use openomsi_game::testing::offscreen::RealTime;
use std::time::{Duration, Instant};

/// The waits `RealTime` asks for after steps of 1/30 s that take `work` each, and the
/// wall time they all took.
fn run(work: &[u64]) -> (Vec<Duration>, f64) {
    let t0 = Instant::now();
    let (mut now, mut pace, mut waits) = (t0, RealTime::default(), Vec::new());
    for w in work {
        now += Duration::from_millis(*w);
        let wait = pace.wait(now, 1.0 / 30.0);
        waits.push(wait);
        now += wait;
    }
    (waits, (now - t0).as_secs_f64())
}

#[test]
fn a_server_keeps_to_the_clock_whatever_its_steps_take() {
    // a dedicated server on Gladbeck: 8 ms of work a step; 300 steps (10 s of its world)
    // took 12.4 s with a whole step slept after each
    let (_, wall) = run(&[8; 300]);
    assert!((wall - 10.0).abs() < 0.04, "300 steps in {wall} s");
    // uneven steps, some longer than a step: made up by the next ones
    let uneven: Vec<u64> = (0..300).map(|i| [2, 45, 9, 60, 20][i % 5]).collect();
    let (_, wall) = run(&uneven);
    assert!((wall - 10.0).abs() < 0.07, "300 uneven steps in {wall} s");
    // a stall of a second is not made up by thirty steps at once: on from there at the
    // steps' pace
    let mut stalled = vec![5; 100];
    stalled[50] = 1000;
    let (waits, _) = run(&stalled);
    let steady = |w: &Duration| (w.as_secs_f64() - 0.0283).abs() < 0.001;
    assert!(waits[51..].iter().all(steady), "{:?}", &waits[50..60]);
}
