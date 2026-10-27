use openomsi_game::testing::head_idle::*;

/// The sway after `seconds` of steps of `dt`, as the game would have made it (at the pace
/// designed here unless another one is asked for).
fn after(dt: f32, seconds: f32, strength: f32, pace: f32) -> HeadIdle {
    let mut head = HeadIdle::default();
    for _ in 0..(seconds / dt).round() as u32 {
        head.step(dt, strength, pace);
    }
    head
}

#[test]
fn the_sway_is_for_a_bus_standing_still() {
    assert_eq!(openomsi_game::testing::head_idle::standstill(0.0), 1.0);
    assert_eq!(openomsi_game::testing::head_idle::standstill(0.3), 1.0);
    assert_eq!(openomsi_game::testing::head_idle::standstill(5.0), 0.0);
    assert_eq!(openomsi_game::testing::head_idle::standstill(50.0), 0.0);
    let mid = openomsi_game::testing::head_idle::standstill(2.75);
    assert!(mid > 0.4 && mid < 0.6, "{mid}");
}

#[test]
fn at_zero_strength_the_head_is_exactly_still() {
    let head = after(1.0 / 60.0, 30.0, 0.0, 1.0);
    assert!(head.is_still(), "{head:?}");
    assert_eq!(head, HeadIdle::default());
}

#[test]
fn the_sway_does_not_depend_on_the_frame_rate() {
    // (a tenth of a second is the longest step the sway takes whole, see below)
    let slow = after(0.1, 20.0, 1.0, 1.0);
    let fast = after(1.0 / 60.0, 20.0, 1.0, 1.0);
    assert!(
        (slow.offset - fast.offset).length() < 0.0005,
        "{slow:?} {fast:?}"
    );
    assert!(
        (slow.yaw - fast.yaw).abs() < 0.01 && (slow.roll - fast.roll).abs() < 0.01,
        "{slow:?} {fast:?}"
    );
}

#[test]
fn the_head_stays_where_a_head_can_be() {
    let mut head = HeadIdle::default();
    for _ in 0..(180.0 * 60.0) as u32 {
        head.step(1.0 / 60.0, 1.0, 1.0);
        assert!(head.offset.x.abs() <= SIDE_M + 0.001, "{head:?}");
        assert!(head.offset.y.abs() <= FORE_M + 0.001, "{head:?}");
        assert!(head.offset.z.abs() <= UP_M + 0.001, "{head:?}");
        assert!(
            head.yaw.abs() <= YAW_DEG + 0.01
                && head.pitch.abs() <= PITCH_DEG + 0.01
                && head.roll.abs() <= ROLL_DEG + 0.01,
            "{head:?}"
        );
    }
}

#[test]
fn the_head_is_never_frozen_and_never_drifts_away() {
    let mut head = HeadIdle::default();
    let (mut low, mut high, mut sum) = (f32::MAX, f32::MIN, 0.0);
    let frames = 300.0 * 60.0;
    for _ in 0..frames as u32 {
        head.step(1.0 / 60.0, 1.0, 1.0);
        low = low.min(head.offset.x);
        high = high.max(head.offset.x);
        sum += head.offset.x;
    }
    // it goes somewhere: at least a third of the way across its own range
    assert!(high - low > SIDE_M, "range {low}..{high}");
    // and it stays around the middle: no walking off to one side
    assert!(
        (sum / frames as f32).abs() < SIDE_M * 0.25,
        "mean {}",
        sum / frames as f32
    );
}

#[test]
fn the_sideways_sway_comes_back() {
    // (a movement that only drifts one way reads as nothing at all, however far it goes:
    // the sideways sway has to turn round again and again for the eye to catch it)
    let mut head = HeadIdle::default();
    let (mut changes, mut before) = (0, 0.0f32);
    for _ in 0..(60.0 * 60.0) as u32 {
        head.step(1.0 / 60.0, 1.0, 1.0);
        if before != 0.0 && head.offset.x.signum() != before.signum() {
            changes += 1;
        }
        before = head.offset.x;
    }
    assert!(
        changes >= 10,
        "the sideways sway turned round {changes} times in a minute"
    );
}

#[test]
fn the_strength_is_how_much_of_the_sway_there_is() {
    for t in [1.0, 3.7, 9.2, 25.0] {
        let full = after(1.0 / 60.0, t, 1.0, 1.0);
        let half = after(1.0 / 60.0, t, 0.5, 1.0);
        assert!(
            (half.offset - full.offset * 0.5).length() < 1e-5,
            "{t}: {half:?} {full:?}"
        );
        assert!(
            (half.roll - full.roll * 0.5).abs() < 1e-4,
            "{t}: {half:?} {full:?}"
        );
    }
}

#[test]
fn the_pace_is_how_fast_the_sway_moves() {
    // (twice the pace is twice as far into the sway: the same picture, reached sooner -
    // and it is the pace of every movement at once, not of one of them)
    let quick = after(1.0 / 60.0, 10.0, 1.0, 2.0);
    let patient = after(1.0 / 60.0, 20.0, 1.0, 1.0);
    assert!(
        (quick.offset - patient.offset).length() < 0.0005,
        "{quick:?} {patient:?}"
    );
    assert!(
        (quick.roll - patient.roll).abs() < 0.01,
        "{quick:?} {patient:?}"
    );
    // and a pace further out than the setting can ask for is held at its end
    let silly = after(1.0 / 60.0, 10.0, 1.0, 100.0);
    assert!(
        (silly.offset - quick.offset).length() < 1e-5,
        "{silly:?} {quick:?}"
    );
}

#[test]
fn a_long_frame_does_not_throw_the_head() {
    let mut jumped = HeadIdle::default();
    jumped.step(2.0, 1.0, 1.0);
    let patient = after(0.1, 0.1, 1.0, 1.0);
    assert!(
        (jumped.offset - patient.offset).length() < 0.002,
        "{jumped:?} {patient:?}"
    );
}

#[test]
fn a_control_under_the_cursor_holds_the_sway_at_once() {
    let mut hold = Hold::default();
    assert!(
        hold.step(1.0 / 60.0, true),
        "the sway went on with a switch under the cursor"
    );
}

#[test]
fn nothing_under_the_cursor_means_the_sway_goes_on() {
    let mut hold = Hold::default();
    assert!(!hold.step(1.0 / 60.0, false));
}

#[test]
fn the_sway_waits_a_moment_after_the_cursor_leaves_a_control() {
    let mut hold = Hold::default();
    assert!(hold.step(1.0 / 60.0, true));
    // (the frame the cursor leaves: still held, or the click itself would be carried off)
    assert!(hold.step(1.0 / 60.0, false));
    let mut waited = 0.0;
    while hold.step(1.0 / 60.0, false) {
        waited += 1.0 / 60.0;
        assert!(waited < 5.0, "the sway never went on again");
    }
    assert!(
        (waited - HOLD_S).abs() < 0.05,
        "it waited {waited} s, not {HOLD_S}"
    );
}
