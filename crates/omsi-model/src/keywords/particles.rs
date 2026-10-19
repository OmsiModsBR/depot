//! `model.cfg` keywords: smoke and particle emitters.

use crate::{Model, ParticleEmitter, Smoke};
use omsi_cfg::CfgReader;

impl Model {
    pub(super) fn particle_keyword(&mut self, k: &str, r: &mut CfgReader) -> bool {
        match k {
            "smoke" => {
                let params: Vec<String> = (0..19).map(|_| r.str().to_string()).collect();
                self.smokes.push(Smoke { params });
            }
            "particle_emitter" => {
                let lines: Vec<String> = r
                    .rest_of_block()
                    .into_iter()
                    .map(|s| s.to_string())
                    .collect();
                self.particle_emitters.push(ParticleEmitter {
                    lines,
                    attach_to: None,
                });
            }
            "ps_attachto" => {
                let v = r.f32s::<3>();
                if let Some(p) = self.particle_emitters.last_mut() {
                    p.attach_to = Some(v);
                }
            }
            _ => return false,
        }
        true
    }
}
