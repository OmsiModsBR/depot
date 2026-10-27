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
pub struct FrameStats {
    times: VecDeque<f32>,
    total: f32,
}

impl FrameStats {
    /// Note a frame that took `dt` seconds.
    pub fn push(&mut self, dt: f32) {
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
    pub fn low(&self) -> Option<f32> {
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
