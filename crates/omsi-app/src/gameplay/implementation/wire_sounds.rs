//! Playing the wire's sounds: a switch or crossing passed, a spark under a shoe. Outside
//! they come from where they happen, heard up to the parameter file's `max_distance`; with
//! the camera in the bus a switch or crossing is its `_int` recording, coming from the
//! convention's interior source and as loud as the cab or the saloon hears it.

use std::sync::Arc;

use omsi_audio::mixer::Clip;
use omsi_audio::{AudioEngine, VoiceParams};
use omsi_trolley::{SoundParams, WireSound, WireSoundKind, SOUND_DIR};

use crate::gameplay::domain::wire_sounds::WireSounds;

/// A spark heard from inside comes through the bodywork (Hz).
const THROUGH_THE_BODY_HZ: f32 = 2500.0;

fn gain(db: f64) -> f32 {
    10f64.powf(db / 20.0) as f32
}

impl WireSounds {
    fn clip(&mut self, audio: &AudioEngine, name: &str) -> Option<Arc<Clip>> {
        self.clips
            .entry(name.to_string())
            .or_insert_with(|| {
                let rel = format!("{SOUND_DIR}/{name}.wav");
                let found = omsi_cfg::find_in_roots(&rel).and_then(|(_, p)| audio.load_clip(&p));
                if found.is_none() {
                    log::warn!("trolleybus: no sound {rel}");
                }
                found
            })
            .clone()
    }

    /// Play `sounds` for the vehicle whose pole-carrying part stands at `carrier` (its world
    /// transform). `inside`: the camera is in the bus, `saloon`: a passenger's camera.
    pub(crate) fn play(
        &mut self,
        audio: &AudioEngine,
        params: &SoundParams,
        carrier: &glam::Mat4,
        sounds: &[WireSound],
        inside: bool,
        saloon: bool,
    ) {
        if !audio.enabled {
            return;
        }
        let listener = audio.listener_position();
        for s in sounds {
            let at = s.position.as_vec3();
            let spark = matches!(s.kind, WireSoundKind::Spark | WireSoundKind::DewireSpark);
            let db = match s.kind {
                WireSoundKind::Spark => params.spark_volume_db,
                WireSoundKind::DewireSpark => params.dewire_volume_db,
                _ => params.volume_db,
            };
            let level = if spark {
                gain(db) * params.spark_sample_gain as f32
            } else {
                gain(db)
            };
            let stem = s.kind.file_stem();
            let distance = at.distance(listener);
            if inside {
                // from the interior source, as loud as the cab or the saloon hears it: the
                // `_int` recording, or the outside one through the bodywork
                let room = if saloon {
                    params.interior_salon_gain_db
                } else {
                    params.interior_cab_gain_db
                };
                let int = s
                    .kind
                    .has_interior()
                    .then(|| self.clip(audio, &format!("{stem}_int")))
                    .flatten();
                let (clip, lowpass_hz, name) = match int {
                    Some(c) => (Some(c), 0.0, format!("{stem}_int")),
                    None => (
                        self.clip(audio, stem),
                        THROUGH_THE_BODY_HZ,
                        stem.to_string(),
                    ),
                };
                let Some(clip) = clip else {
                    continue;
                };
                let source = carrier.transform_point3(params.interior_source.as_vec3());
                let g = level * gain(room);
                log::info!("trolleybus: sound {name} inside (gain {g:.2})");
                audio.play(
                    clip,
                    VoiceParams {
                        gain: g,
                        position: Some(source),
                        doppler: false,
                        range: (source - listener).length().max(1.0),
                        lowpass_hz,
                        ..Default::default()
                    },
                );
                continue;
            }
            if distance > params.max_distance as f32 {
                log::info!("trolleybus: sound {stem} not heard ({distance:.0} m away)");
                continue;
            }
            let Some(clip) = self.clip(audio, stem) else {
                continue;
            };
            log::info!("trolleybus: sound {stem} outside, {distance:.1} m away (gain {level:.2})");
            audio.play(
                clip,
                VoiceParams {
                    gain: level,
                    position: Some(at),
                    range: params.min_distance as f32,
                    ..Default::default()
                },
            );
        }
    }
}
