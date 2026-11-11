//! A rail vehicle: its outermost axles on the track.

use super::AiBody;
use glam::DVec3;

impl AiBody {
    /// A rail vehicle: its outermost axles stand on the track, the body is the line
    /// between them.
    pub(super) fn ride(&mut self, way: &dyn Fn(f32) -> DVec3) {
        let f = way(self.front_long);
        let r = way(self.rear_long);
        let d = f - r;
        let run = d.truncate().length();
        if run > 1e-3 {
            self.heading = d.x.atan2(d.y).to_degrees().rem_euclid(360.0);
            self.pitch_deg = (d.z / run).atan().to_degrees() as f32;
        }
        let span = (self.front_long - self.rear_long).max(0.1);
        self.position = r + d * ((0.0 - self.rear_long) / span) as f64;
        self.bank_deg = 0.0;
        self.steer = 0.0;
        self.started = true;
    }
}
