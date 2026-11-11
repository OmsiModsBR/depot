//! An aircraft: on its path, banking into its turns.

use super::{wrap, AiBody};
use glam::DVec3;

impl AiBody {
    /// An aircraft: on its path, nose along it, banking into a turn as much as the turn
    /// asks for (tan bank = speed × turn rate / g).
    pub(super) fn fly(&mut self, dt: f32, speed: f32, way: &dyn Fn(f32) -> DVec3) {
        let p = way(0.0);
        let d = way(15.0) - way(-15.0);
        let run = d.truncate().length();
        let (h_t, p_t) = if run > 1e-3 {
            (
                d.x.atan2(d.y).to_degrees(),
                (d.z / run).atan().to_degrees() as f32,
            )
        } else {
            (self.heading, self.pitch_deg)
        };
        if !self.started || dt <= 0.0 {
            self.heading = h_t.rem_euclid(360.0);
            self.pitch_deg = p_t;
            self.bank_deg = 0.0;
            self.yaw_rate = 0.0;
            self.started = true;
        } else {
            let k = (dt / 0.6).min(1.0) as f64;
            let dh = wrap(h_t - self.heading) * k;
            self.heading = (self.heading + dh).rem_euclid(360.0);
            let rate = (dh.to_radians() / dt as f64) as f32;
            self.yaw_rate += (rate - self.yaw_rate) * (dt / 0.5).min(1.0);
            let bank = (speed * self.yaw_rate / 9.81)
                .atan()
                .to_degrees()
                .clamp(-30.0, 30.0);
            let k = (dt / 0.8).min(1.0);
            self.bank_deg += (bank - self.bank_deg) * k;
            self.pitch_deg += (p_t - self.pitch_deg) * k;
        }
        self.position = p;
        self.steer = 0.0;
    }
}
