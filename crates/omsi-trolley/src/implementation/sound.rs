//! The wire's sounds and sparks, from what the poles did in a step: a switch passed (its
//! entry left for one of its ways, or one of its ways come in to its entry), a crossing or a
//! wire sound marker passed, a spark under a shoe, a pole leaving the wire under power.

use glam::DVec3;

use crate::domain::declaration::TrolleyDeclaration;
use crate::domain::network::{SpanId, WireNetwork, WireObjectKind};
use crate::domain::pole::{BodyPose, PoleEvent, PoleOutputs, PoleSim};
use crate::domain::sound::{SparkParams, WireSound, WireSoundKind, WireWeather};

/// How far (metres, along the ground) from where the way through a switch was set its
/// object may stand, to be that switch's.
const SWITCH_REACH: f64 = 6.0;

/// How far (metres) past touching it both shoes must be before a switch can sound again.
const SWITCH_RELEASE: f64 = 1.0;

/// A shoe on span `span` at `shoe` (world), in a switch's gap: if the end it is at crosses
/// the gap into an entry where another way meets too, whether it comes in from the left of
/// the two, seen from the entry (None: it is not coming in to a switch's entry).
fn merging(net: &WireNetwork, span: SpanId, shoe: DVec3) -> Option<bool> {
    let here = net.span(span)?;
    let end = usize::from(here.ends[1].point.distance(shoe) < here.ends[0].point.distance(shoe));
    let from = (span, end);
    let other_end = |j: &crate::domain::network::WireJoint, e: (SpanId, usize)| {
        if j.a == e {
            j.b
        } else {
            j.a
        }
    };
    net.joints_at(span, end)
        .filter(|j| j.via.is_some_and(|k| k.is_switch()))
        .find_map(|j| {
            let into = other_end(j, from);
            let theirs = net
                .joints_at(into.0, into.1)
                .filter(|k| k.via.is_some())
                .map(|k| other_end(k, into))
                .find(|o| *o != from)?;
            let ours = here.ends[end].point;
            let theirs = net.span(theirs.0)?.ends[theirs.1].point;
            // (left as seen from the entry, looking along its ways)
            let heading = -net.span(into.0)?.ends[into.1].outward;
            Some(heading.cross(ours - theirs).z < 0.0)
        })
}

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
        let world = |p: DVec3| body.position + body.rotation * p;
        if let Some(s) = decl.sound {
            // a switch sounds as a shoe touches its object, whichever way it is passed; the
            // way taken through it (left or right, leaving its entry or coming in to it)
            // picks the recording
            let switches: Vec<(DVec3, WireObjectKind)> = net
                .objects()
                .filter(|o| o.kind.is_switch())
                .map(|o| (o.position, o.kind))
                .collect();
            let flat = |a: DVec3, b: DVec3| (a - b).truncate().length();
            let events = std::mem::take(&mut self.events);
            for e in &events {
                let (kind, at) = match *e {
                    PoleEvent::Fork {
                        kind: Some(k),
                        left,
                        at,
                        ..
                    } if k.is_switch() => (WireSoundKind::SwitchSeparate { left }, at),
                    PoleEvent::Joint {
                        merge_left: Some(left),
                        at,
                        ..
                    } => (WireSoundKind::SwitchMerge { left }, at),
                    _ => continue,
                };
                let Some(&(object, _)) = switches
                    .iter()
                    .filter(|(p, _)| flat(*p, at) <= SWITCH_REACH)
                    .min_by(|a, b| flat(a.0, at).total_cmp(&flat(b.0, at)))
                else {
                    continue;
                };
                if self.switch_touched.contains(&object)
                    || self.switch_waiting.iter().any(|w| w.0 == object)
                {
                    continue;
                }
                self.switch_waiting.push((object, kind, f64::INFINITY));
            }
            let shoes: Vec<(usize, DVec3)> = (0..2)
                .filter(|i| out.poles[*i].on_wire)
                .map(|i| (i, world(out.poles[i].shoe_pos)))
                .collect();
            let nearest = |p: DVec3| {
                shoes
                    .iter()
                    .map(|(_, s)| flat(*s, p))
                    .fold(f64::INFINITY, f64::min)
            };
            // touched now: the switch sounds - with the way's recording; a shoe coming in
            // from one of its ways (whose way is known only past the object) with that one's
            // merge; else with its own side's
            for &(p, k) in &switches {
                if self.switch_touched.contains(&p) {
                    continue;
                }
                let Some(&(i, shoe)) = shoes
                    .iter()
                    .find(|(_, sh)| flat(*sh, p) <= s.cross_trigger_radius)
                else {
                    continue;
                };
                let waiting = self.switch_waiting.iter().position(|w| w.0 == p);
                let kind = match waiting {
                    Some(w) => self.switch_waiting.swap_remove(w).1,
                    None => match self.poles[i]
                        .cursor
                        .and_then(|c| merging(net, c.span, shoe))
                    {
                        Some(left) => WireSoundKind::SwitchMerge { left },
                        None => WireSoundKind::SwitchSeparate {
                            left: k == WireObjectKind::SwitchBranchLeft,
                        },
                    },
                };
                self.switch_touched.push(p);
                // (heard from the wire above the object)
                self.sounds.push(WireSound {
                    kind,
                    position: p.truncate().extend(shoe.z),
                });
            }
            // passed by without touching: it sounds where a shoe came nearest
            let mut passed = Vec::new();
            self.switch_waiting.retain_mut(|w| {
                let d = nearest(w.0);
                if d.is_finite() && d > w.2 + 0.05 {
                    passed.push((w.0, w.1));
                    return false;
                }
                w.2 = w.2.min(d);
                w.2 <= SWITCH_REACH * 2.0 || !d.is_finite()
            });
            let wire_z = shoes.first().map(|(_, sh)| sh.z);
            for (p, kind) in passed {
                self.switch_touched.push(p);
                self.sounds.push(WireSound {
                    kind,
                    position: p.truncate().extend(wire_z.unwrap_or(p.z)),
                });
            }
            let release = s.cross_trigger_radius + SWITCH_RELEASE;
            self.switch_touched.retain(|p| nearest(*p) <= release);
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
                self.sounds.push(WireSound {
                    kind: WireSoundKind::Cross,
                    position: p,
                });
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
