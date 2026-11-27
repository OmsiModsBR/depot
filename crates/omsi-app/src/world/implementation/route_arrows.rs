//! Putting the route arrows into the world and taking them away again. The state of the arrows
//! and their objects are in `domain/route_arrows.rs`.

use super::super::domain::route_arrows::{sco, RouteArrows};
use crate::world::scene::World;
use glam::DVec3;
use omsi_render::{Renderer, Scene};

impl RouteArrows {
    /// Twice a second: the arrows of the route ahead stand, the others go.
    pub(crate) fn tick(
        &mut self,
        dt: f32,
        world: &World,
        renderer: &Renderer,
        scene: &mut Scene,
        spots: &[(u64, DVec3, f64, &'static str, String)],
    ) {
        self.wait -= dt;
        if self.wait > 0.0 {
            return;
        }
        self.wait = 0.5;
        let wanted: Vec<u64> = spots
            .iter()
            .map(|s| s.0 ^ (s.3.len() as u64) << 56)
            .collect();
        // gone: behind the bus, or the route changed
        let mut kept = Vec::with_capacity(self.placed.len());
        for (k, tg) in self.placed.drain(..) {
            if wanted.contains(&k) {
                kept.push((k, tg));
            } else {
                world.remove_helper_object(renderer, scene, tg);
            }
        }
        self.placed = kept;
        for (s, key) in spots.iter().zip(wanted) {
            if self.placed.iter().any(|p| p.0 == key) {
                continue;
            }
            let (_, pos, heading, kind, text) = s;
            // (on the road surface there, not the lane's own height: a lane may lie a few
            // centimetres off it; a stop's helper stands where the stop object stands)
            let z = if *kind == "busstop" {
                pos.z
            } else {
                world
                    .walk_height(pos.x, pos.y)
                    .filter(|z| (z - pos.z).abs() < 1.5)
                    .unwrap_or(pos.z)
            };
            if let Some(tg) = world.add_helper_object(
                renderer,
                scene,
                sco(kind),
                DVec3::new(pos.x, pos.y, z),
                *heading,
                std::slice::from_ref(text),
            ) {
                if omsi_cfg::flags::DEBUG_NAV.is_set() {
                    log::info!(
                        "route arrow {kind} '{text}' at ({:.1}, {:.1}, {:.2}) heading {:.0}",
                        pos.x,
                        pos.y,
                        z,
                        heading
                    );
                }
                self.placed.push((key, tg));
            } else {
                log::warn!(
                    "route arrow {}: the object could not be put down",
                    sco(kind)
                );
            }
        }
    }

    /// Every arrow goes at once: the setting was switched off (`tick` is no longer called
    /// then, so those standing would stay where they were for good).
    pub(crate) fn clear(&mut self, world: &World, renderer: &Renderer, scene: &mut Scene) {
        if omsi_cfg::flags::DEBUG_NAV.is_set() {
            log::info!(
                "route arrows: the {} standing taken away",
                self.placed.len()
            );
        }
        for (_, tg) in self.placed.drain(..) {
            world.remove_helper_object(renderer, scene, tg);
        }
        self.wait = 0.0;
    }
}
