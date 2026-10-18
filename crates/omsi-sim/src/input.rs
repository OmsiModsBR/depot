//! Input actions: the names from `Inputs/keyboard.cfg` and how they reach a vehicle.
//!
//! Vehicle actions are script triggers with the same name; on key release the trigger
//! `<name>_off` fires. A few actions are handled by the engine itself (throttle, brake,
//! clutch, steering, views).

/// Actions the engine handles instead of forwarding to the script.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EngineAction {
    Throttle,
    ThrottleAmplify,
    Brake,
    Clutch,
    SteeringLeft,
    SteeringRight,
    SteeringNeutral,
}

pub fn engine_action(name: &str) -> Option<EngineAction> {
    Some(match name.to_ascii_lowercase().as_str() {
        "throttle" => EngineAction::Throttle,
        "throttle_amplify" => EngineAction::ThrottleAmplify,
        "brake" => EngineAction::Brake,
        "clutch" => EngineAction::Clutch,
        "steering_left" => EngineAction::SteeringLeft,
        "steering_right" => EngineAction::SteeringRight,
        "steering_neutral" => EngineAction::SteeringNeutral,
        _ => return None,
    })
}

/// Keyboard-driven analogue inputs, integrated per frame like the original (keys ramp the
/// pedal/steering position instead of setting it).
#[derive(Debug, Clone, Default)]
pub struct KeyboardAxes {
    pub throttle_key: bool,
    pub amplify_key: bool,
    pub brake_key: bool,
    pub clutch_key: bool,
    pub left_key: bool,
    pub right_key: bool,
    pub neutral_key: bool,
    pub throttle: f32,
    pub brake: f32,
    pub clutch: f32,
    pub steering: f32,
    /// Road speed in km/h: the steering returns to centre by itself only while rolling.
    pub speed_kmh: f32,
    /// Rate of the steering wheel (fraction per second) while it swings back on its own.
    pub steer_vel: f32,
    /// "Steering linearity": the keys turn the wheel as OMSI does: the
    /// curvature changes by 0.00005 per millisecond whatever the speed, so the wheel goes at
    /// one steady pace; `lock_curvature` (`[inv_min_turnradius]`) turns that into a share
    /// of the lock.
    pub linear: bool,
    /// "Old Steering": let go, the wheel stays where it is and is turned back by hand - OMSI
    /// without `[autoCenter]`.
    pub old_steering: bool,
    /// OMSI's `[redSteerSpd]` option (off by default, as in OMSI): the keys turn the
    /// wheel, and `steering_neutral` and the self-centring bring it back, slower at speed -
    /// by min(1, 1.5 e^(-0.1 v)), v in m/s: all of
    /// the pace up to 14.6 km/h, 0.37 of it at 50.
    pub red_steer_spd: bool,
    pub lock_curvature: f32,
    /// OMSI's held brake (the default): let go, the brake stays where the key left it until
    /// the throttle key is pressed - tap the brake and it keeps that pressure. Off, the brake
    /// comes off with its key, as in most games. (The throttle never stays: see `update`.)
    pub pedal_hold: bool,
    /// The steering is on its way back to the middle: set by the
    /// `steering_neutral` key, cleared by a steering key.
    pub centering: bool,
}

impl KeyboardAxes {
    pub fn set(&mut self, action: EngineAction, pressed: bool) {
        match action {
            EngineAction::Throttle => self.throttle_key = pressed,
            EngineAction::ThrottleAmplify => self.amplify_key = pressed,
            EngineAction::Brake => self.brake_key = pressed,
            EngineAction::Clutch => self.clutch_key = pressed,
            EngineAction::SteeringLeft => self.left_key = pressed,
            EngineAction::SteeringRight => self.right_key = pressed,
            EngineAction::SteeringNeutral => self.neutral_key = pressed,
        }
    }

    /// Let go of every key: the window losing focus (alt-tab, a click outside it, an OS
    /// dialog) never delivers the matching key-up, so without this a throttle or steering
    /// key held at that moment stayed "pressed" forever (and, with a modifier key stuck the
    /// same way, a later plain key press could be misread as held with that modifier).
    pub fn release_all(&mut self) {
        *self = KeyboardAxes {
            throttle: self.throttle,
            brake: self.brake,
            clutch: self.clutch,
            steering: self.steering,
            speed_kmh: self.speed_kmh,
            linear: self.linear,
            old_steering: self.old_steering,
            red_steer_spd: self.red_steer_spd,
            lock_curvature: self.lock_curvature,
            pedal_hold: self.pedal_hold,
            centering: self.centering,
            ..Default::default()
        };
    }

    /// Road speed (km/h), as a number: a speed that is none made the return's step NaN, and
    /// its clamp stopped the game (#1045).
    fn speed(&self) -> f32 {
        if self.speed_kmh.is_finite() {
            self.speed_kmh.abs()
        } else {
            0.0
        }
    }

    /// `[redSteerSpd]`: the share of the pace left at speed (1 with it off).
    fn red(&self) -> f32 {
        if self.red_steer_spd {
            (1.5 * (-0.1 * self.speed() / 3.6).exp()).min(1.0)
        } else {
            1.0
        }
    }

    /// The paces (share of the lock a second) a key turns the wheel at, and it comes back
    /// at by itself. A bus's wheel is about two and a half turns from lock to lock. It still
    /// came back too slowly for how fast a key could turn it (1.25 s to full lock against
    /// 15+ s to come back on its own), so every correction overshot and had to be walked
    /// back by hand. Now the return (the castor of the front axle pulling the wheel to the
    /// middle, harder the faster the bus rolls) is a little brisker standing still, and the
    /// key turns the wheel at that same pace, only a tenth faster - never a swerve, because
    /// a correction can only be as fast as the wheel would come back on its own anyway.
    fn paces(&self) -> (f32, f32) {
        let v = self.speed();
        let (rate, back) = if self.linear {
            // OMSI: 0.05 of curvature a second, from the middle to the lock in
            // `[inv_min_turnradius]` / 0.05 seconds (2 s for a bus with a 10 m radius); it
            // comes back (unless Old Steering) at the same pace, as `[autoCenter]` does
            let r = (0.05 / self.lock_curvature.max(0.01)).clamp(0.05, 5.0);
            (r, r)
        } else {
            let base = 0.8 / (1.0 + v / 45.0);
            let back = base * (0.25 + 0.75 * (v / 25.0).min(1.0));
            (back * 1.1, back)
        };
        // `[redSteerSpd]`: the key's pace, and OMSI's return (the linear one), less at speed
        let red = self.red();
        (rate * red, if self.linear { back * red } else { back })
    }

    /// Where a wheel let go at `steering` is `dt` later, as this bus's comes back by itself:
    /// at the return's pace, easing out over the last bit so that it settles instead of
    /// stopping dead in the middle; with Old Steering it stays where the hands left it. The
    /// keys' wheel, and the one on a phone's screen (#1090: that one sprang back to the
    /// middle in a third of a second whatever the speed).
    pub fn let_go(&self, steering: f32, dt: f32) -> f32 {
        if self.old_steering {
            return steering;
        }
        let ease = (steering.abs() / 0.08).clamp(0.3, 1.0);
        let step = self.paces().1 * ease * dt;
        let s = steering - steering.clamp(-step, step);
        // (a snap from farther out was a visible jolt of the wheel and the driver's hands)
        if s.abs() < 0.0003 {
            0.0
        } else {
            s
        }
    }

    pub fn update(&mut self, dt: f32) {
        // The pedals as OMSI works them from the keys:
        // the throttle key raises the throttle at 2 a second up to 0.85 - to
        // the floor only with throttle_amplify held - and takes the brake off at once; let
        // go, the throttle falls at 1 a second. The brake key raises the brake at 1 a second
        // and takes the throttle off; let go, the brake stays where it is until the throttle
        // key is pressed, or eases off at 0.5 a second while throttle_amplify is held.
        let top = if self.amplify_key { 1.0 } else { 0.85 };
        if self.throttle_key {
            self.brake = 0.0;
            self.throttle = (self.throttle + 2.0 * dt).min(top);
        } else {
            self.throttle = (self.throttle - dt).max(0.0);
        }
        if self.brake_key {
            self.throttle = 0.0;
            self.brake = (self.brake + dt).min(1.0);
        } else if !self.pedal_hold {
            self.brake = (self.brake - 3.0 * dt).max(0.0);
        } else if self.amplify_key {
            self.brake = (self.brake - 0.5 * dt).max(0.0);
        }
        // The clutch as OMSI works it from a key: pressed, the
        // pedal is down at once; let go, it comes up at 0.7 a second - a foot letting the
        // clutch in, which is what makes a gear change on the keyboard smooth.
        if self.clutch_key {
            self.clutch = 1.0;
        } else {
            self.clutch = (self.clutch - 0.7 * dt).max(0.0);
        }
        // Steering (the paces: `paces`)
        let (rate, back) = self.paces();
        let red = self.red();
        // The centring key held with the steering key towards the middle brings the wheel
        // back at both paces together, as in OMSI - the quick way back out of a full lock
        // (#1652, #851): the steering key used to end the centring, so Num 5 held with
        // Num 4 did no more than Num 4 alone. It stops in the middle.
        let one_key = self.left_key != self.right_key;
        let towards_middle = one_key
            && ((self.left_key && self.steering > 0.0)
                || (self.right_key && self.steering < 0.0)
                || self.steering == 0.0);
        if self.neutral_key && towards_middle {
            let omsi = (0.05 / self.lock_curvature.max(0.01)).clamp(0.05, 5.0) * red;
            let own = if self.old_steering { 0.0 } else { back };
            let step = (rate + omsi.max(own)) * dt;
            self.steering -= self.steering.clamp(-step, step);
            self.steer_vel = 0.0;
            self.centering = true;
            return;
        }
        if self.neutral_key {
            self.centering = true;
        }
        if self.left_key || self.right_key {
            self.centering = false;
        }
        if self.left_key && self.right_key {
            // OMSI keeps the wheel where it is while both steering keys are held.
            // Releasing either key immediately hands control to the direction still held.
            self.steer_vel = 0.0;
        } else if self.left_key {
            self.steering = (self.steering - rate * dt).max(-1.0);
            self.steer_vel = 0.0;
        } else if self.right_key {
            self.steering = (self.steering + rate * dt).min(1.0);
            self.steer_vel = 0.0;
        } else if self.centering {
            // steering_neutral as in OMSI: the wheel goes back
            // to the middle at the pace the keys turn it - in OMSI both are 0.05 of curvature
            // a second - in a straight line, and stays there until a
            // steering key is pressed; it used to jump to the middle, a jerk of the whole bus
            // at speed. Here the keys turn faster than that while rolling, and the wheel comes
            // back faster on its own: at OMSI's figure alone the centring key brought the
            // wheel back slower than the key had turned it, and slower than letting go (#851)
            let omsi = (0.05 / self.lock_curvature.max(0.01)).clamp(0.05, 5.0) * red;
            let own = if self.old_steering { 0.0 } else { back };
            let step = omsi.max(rate).max(own) * dt;
            self.steering -= self.steering.clamp(-step, step);
            self.steer_vel = 0.0;
        } else {
            // released: back by itself (Old Steering: it stays where the hands left it)
            self.steering = self.let_go(self.steering, dt);
            self.steer_vel = 0.0;
        }
    }
}
