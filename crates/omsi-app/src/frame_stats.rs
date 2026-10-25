//! The frame rate's "1 % low": how slow the worst frames of the last few seconds were.
//!
//! The average frame rate hides stutter; the 1 % low is the frame rate the slowest one
//! percent of the frames averaged (the mean of their times, turned back into frames per
//! second). It is only read for the information bar and the frame-rate chip.

use std::collections::VecDeque;

/// How far back the frames count.
const WINDOW_SECS: f32 = 10.0;
/// Fewer frames than this and a "1 %" is not a fair share of anything.
const MIN_FRAMES: usize = 50;
/// A frame longer than this (a load, a pause, a drag of the window) says nothing about the
/// game's speed and is not counted.
const MAX_FRAME_SECS: f32 = 1.0;

#[derive(Default)]
pub(crate) struct FrameStats {
    times: VecDeque<f32>,
    total: f32,
}

impl FrameStats {
    /// Note a frame that took `dt` seconds.
    pub(crate) fn push(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 || dt > MAX_FRAME_SECS {
            return;
        }
        self.times.push_back(dt);
        self.total += dt;
        while self.total > WINDOW_SECS && self.times.len() > 1 {
            if let Some(old) = self.times.pop_front() {
                self.total -= old;
            }
        }
    }

    /// The 1 % low in frames per second; none before enough frames have been seen.
    pub(crate) fn low(&self) -> Option<f32> {
        if self.times.len() < MIN_FRAMES {
            return None;
        }
        let mut t: Vec<f32> = self.times.iter().copied().collect();
        t.sort_by(|a, b| b.total_cmp(a));
        let n = (t.len() / 100).max(1);
        let mean = t[..n].iter().sum::<f32>() / n as f32;
        (mean > 0.0).then(|| 1.0 / mean)
    }
}

#[cfg(test)]
mod tests {
    use super::FrameStats;

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
}
