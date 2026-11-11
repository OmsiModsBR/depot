//! The wire's sounds and sparks, from what the poles did in a step: a switch passed (its
//! entry left for one of its ways, or one of its ways come in to its entry), a crossing or a
//! wire sound marker passed, a spark under a shoe, a pole leaving the wire under power.

use glam::DVec3;

use crate::domain::declaration::TrolleyDeclaration;
use crate::domain::network::{WireNetwork, WireObjectKind};
use crate::domain::pole::{BodyPose, PoleEvent, PoleOutputs, PoleSim};
use crate::domain::sound::{SparkParams, WireSound, WireSoundKind, WireWeather};

/// How long a switch sound is remembered (seconds), and how near (metres, along the ground)
/// another one is the same switch: the second shoe passes it too.
const HEARD_MEMORY: f64 = 10.0;
const SAME_SWITCH: f64 = 5.0;

/// The draws' state before the first one.
const SEED: u64 = 0x9E37_79B9_7F4A_7C15;

impl PoleSim {
    /// The sounds and sparks of the step just taken (`out` gets its sparks).
    pub(crate) fn wire_sounds(
        &mut self,
        decl: &TrolleyDeclaration,
        net: &WireNetwork,
        body: &BodyPose,
        out: &mut PoleOutputs,
        dt: f64,
    ) {
        self.sounds.clear();
        for h in self.heard.iter_mut() {
            h.1 += dt;
        }
        self.heard.retain(|h| h.1 < HEARD_MEMORY);
        let world = |p: DVec3| body.position + body.rotation * p;
        if let Some(s) = decl.sound {
            let events = std::mem::take(&mut self.events);
            for e in &events {
                let (kind, at, chance) = match *e {
                    PoleEvent::Fork {
                        kind: Some(k),
                        left,
                        at,
                        ..
                    } if k.is_switch() => (
                        WireSoundKind::SwitchSeparate { left },
                        at,
                        if left {
                            s.switch_left_sep_chance
                        } else {
                            s.switch_right_sep_chance
                        },
                    ),
                    PoleEvent::Joint {
                        merge_left: Some(left),
                        at,
                        ..
                    } => (
                        WireSoundKind::SwitchMerge { left },
                        at,
                        if left {
                            s.switch_left_merg_chance
                        } else {
                            s.switch_right_merg_chance
                        },
                    ),
                    _ => continue,
                };
                if self
                    .heard
                    .iter()
                    .any(|h| (h.0 - at).truncate().length() < SAME_SWITCH)
                {
                    continue;
                }
                self.heard.push((at, 0.0));
                if self.draw() < chance {
                    self.sounds.push(WireSound { kind, position: at });
                } else {
                    log::info!("trolleybus: {kind:?} left silent by its chance ({chance})");
                }
            }
            self.events = events;
            // crossings and sound markers: the middle of the two shoes on the wire passing one
            let near = if out.poles.iter().all(|p| p.on_wire) {
                let mid = (world(out.poles[0].shoe_pos) + world(out.poles[1].shoe_pos)) / 2.0;
                net.objects()
                    .filter(|o| {
                        matches!(
                            o.kind,
                            WireObjectKind::Crossing | WireObjectKind::SoundMarker
                        )
                    })
                    .map(|o| o.position)
                    .find(|p| (*p - mid).truncate().length() <= s.cross_trigger_radius)
            } else {
                None
            };
            if let Some(p) = near.filter(|p| self.near_object != Some(*p)) {
                if self.draw() < s.cross_chance {
                    self.sounds.push(WireSound {
                        kind: WireSoundKind::Cross,
                        position: p,
                    });
                }
            }
            self.near_object = near;
        }
        let Some(sp) = decl.sparks.filter(|sp| sp.enabled) else {
            return;
        };
        let gain = match self.weather {
            WireWeather::Dry => 1.0,
            WireWeather::Rain => sp.rain_gain,
            WireWeather::Snow => sp.snow_gain,
            WireWeather::Ice => sp.ice_gain,
        }
        .max(1e-3);
        let drawing = self.throttle >= sp.min_throttle;
        let powered = drawing && self.speed_kmh >= sp.min_speed_kmh;
        let audible = decl.sound.is_some();
        for i in 0..2 {
            self.spark_left[i] = (self.spark_left[i] - dt).max(0.0);
            let shoe = world(out.poles[i].shoe_pos);
            if out.poles[i].on_wire && powered {
                let next = match self.spark_next[i] {
                    Some(t) => t - dt,
                    None => self.between(sp.interval) / gain,
                };
                if next <= 0.0 {
                    self.spark(i, &sp, audible, WireSoundKind::Spark, shoe);
                    self.spark_next[i] = None;
                } else {
                    self.spark_next[i] = Some(next);
                }
            }
            let left_wire = self
                .events
                .iter()
                .any(|e| matches!(e, PoleEvent::Dewired { pole, .. } if *pole == i));
            if left_wire && drawing {
                self.spark(i, &sp, audible, WireSoundKind::DewireSpark, shoe);
            }
            out.poles[i].spark = self.spark_left[i] > 0.0;
        }
    }

    fn spark(&mut self, i: usize, sp: &SparkParams, audible: bool, kind: WireSoundKind, at: DVec3) {
        self.spark_left[i] = self.between(sp.duration);
        if audible {
            self.sounds.push(WireSound { kind, position: at });
        }
    }

    /// A number drawn between `range[0]` and `range[1]`.
    fn between(&mut self, range: [f64; 2]) -> f64 {
        range[0] + (range[1] - range[0]) * self.draw()
    }

    /// A number drawn in 0..1.
    fn draw(&mut self) -> f64 {
        if self.rng == 0 {
            self.rng = SEED;
        }
        // xorshift64*
        let mut x = self.rng;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.rng = x;
        (x.wrapping_mul(0x2545_F491_4F6C_DD1D) >> 11) as f64 / (1u64 << 53) as f64
    }
}
