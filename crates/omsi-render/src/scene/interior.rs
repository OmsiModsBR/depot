//! The interior lights of vehicles: their slots and the lamps an instance uses.
use super::model::Scene;
use crate::lighting::PointLight;
use crate::renderer::{Renderer, LAMP_CODE_STRIDE, MAX_LAMPS_PER_MESH};

impl Renderer {
    /// `n` consecutive slots for a vehicle's `[interiorlight]` lamps; give them back with
    /// [`Renderer::free_interior_lights`].
    pub fn alloc_interior_lights(&self, scene: &mut Scene, n: u32) -> u32 {
        if let Some(k) = scene.interior_free.iter().position(|f| f.1 >= n) {
            let (first, len) = scene.interior_free[k];
            if len == n {
                scene.interior_free.remove(k);
            } else {
                scene.interior_free[k] = (first + n, len - n);
            }
            return first;
        }
        let first = scene.interior_lights.len() as u32;
        scene.interior_lights.extend((0..n).map(|_| PointLight {
            intensity: 0.0,
            ..Default::default()
        }));
        first
    }

    pub fn free_interior_lights(&self, scene: &mut Scene, first: u32, n: u32) {
        for l in scene
            .interior_lights
            .iter_mut()
            .skip(first as usize)
            .take(n as usize)
        {
            l.intensity = 0.0;
        }
        scene.interior_free.push((first, n));
    }

    pub fn set_interior_light(&self, scene: &mut Scene, slot: u32, light: PointLight) {
        if let Some(l) = scene.interior_lights.get_mut(slot as usize) {
            *l = light;
        }
    }

    /// Which lamps light an instance: `count` (up to `MAX_LAMPS_PER_MESH`) slots from `first` (see
    /// `Instance::interior_lamps`).
    pub fn set_interior_lamps(&self, scene: &mut Scene, instance: usize, first: u32, count: u32) {
        let code = if count == 0 {
            0
        } else {
            first * LAMP_CODE_STRIDE + count.min(MAX_LAMPS_PER_MESH)
        };
        let i = &mut scene.instances[instance];
        if i.interior_lamps != code {
            i.interior_lamps = code;
            Self::mark_changed(scene, instance);
        }
    }
}
