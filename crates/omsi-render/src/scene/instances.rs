//! Instances: making a scene, adding, moving, removing and recycling instances.

use super::bounds::InstanceBounds;
use super::model::{Instance, MaterialId, MeshId, RenderPhase, Scene};
use crate::renderer::Renderer;
use glam::{DVec3, Mat4};
use std::collections::HashMap;

impl Renderer {
    pub fn new_scene(&self) -> Scene {
        Scene {
            meshes: Vec::new(),
            textures: Vec::new(),
            glass_slot: None,
            materials: Vec::new(),
            instances: Vec::new(),
            render_origin: DVec3::ZERO,
            lights: Vec::new(),
            interior_lights: Vec::new(),
            interior_free: Vec::new(),
            coronas: Vec::new(),
            model_buf: None,
            params_buf: None,
            light_buf: None,
            grid_buf: None,
            corona_buf: None,
            corona_count: 0,
            smoke: Vec::new(),
            smoke_buf: None,
            smoke_count: 0,
            corona_runs: Vec::new(),
            draw_buf: None,
            camera_bind_group: None,
            shadow_bind_group: None,
            sky_bind_group: None,
            overlays: Vec::new(),
            premultiplied: Default::default(),
            transposed: Default::default(),
            overlay_res: Vec::new(),
            dirty: true,
            changed: Vec::new(),
            mesh_pages: Vec::new(),
            changed_mark: Vec::new(),
            origin_moved: false,
            cache_bounds: omsi_cfg::env::var_os("OMSI_NO_BOUNDS_CACHE").is_none(),
            bounds_meshes: Vec::new(),
            bounds_known: Vec::new(),
            bounds_users: Vec::new(),
            bounds_users_stale: true,
            bounds_rescan: false,
            bounds_dirty: false,
            block_bounds: Vec::new(),
            block_dirty: Vec::new(),
            block_cursor: 0,
            uploaded_instances: 0,
            uploaded_entries: 0,
            cpu_models: Vec::new(),
            cpu_params: Vec::new(),
            last_grid: Vec::new(),
            lamp_shadow_last: Vec::new(),
            last_lights: Vec::new(),
            bind_groups: HashMap::new(),
            looks: hashbrown::HashMap::new(),
            pbr_maps: HashMap::new(),
            snow_textures: Default::default(),
        }
    }

    pub fn add_instance(
        &self,
        scene: &mut Scene,
        mesh: MeshId,
        origin: DVec3,
        transform: Mat4,
        materials: Vec<MaterialId>,
    ) -> usize {
        let slots = scene.meshes[mesh]
            .ranges
            .iter()
            .map(|r| r.2)
            .max()
            .map(|m| m as usize + 1)
            .unwrap_or(1);
        scene.instances.push(Instance {
            mesh,
            transform,
            origin,
            materials,
            slot_alpha: vec![1.0; slots],
            slot_light: vec![1.0; slots],
            slot_night: vec![1.0; slots],
            visible: true,
            slot_uv: vec![[0.0; 2]; slots],
            interior: 0.0,
            interior_lamps: 0,
            base: 0,
            bounds: InstanceBounds::default(),
            surface: false,
            presurface: false,
            render_phase: RenderPhase::Normal,
            surface_bias: false,
            blend_sort_origin: None,
            lod: (0.0, f32::MAX),
            blob: false,
            ground_layer: false,
            decal: false,
            object_radius: 0.0,
            detail: 1.0,
            any_distance: false,
            near_only: None,
            mirror_only: false,
            omsi_caster: false,
            ordered: false,
            casts_shadow: true,
            roof: None,
        });
        if scene.bounds_known.get(mesh).copied().unwrap_or(false) {
            scene.bounds_users.push(scene.instances.len() - 1);
        }
        scene.instances.len() - 1
    }

    pub fn add_surface_instance(
        &self,
        scene: &mut Scene,
        mesh: MeshId,
        origin: DVec3,
        transform: Mat4,
        materials: Vec<MaterialId>,
    ) -> usize {
        let slots = scene.meshes[mesh]
            .ranges
            .iter()
            .map(|r| r.2)
            .max()
            .map(|m| m as usize + 1)
            .unwrap_or(1);
        scene.instances.push(Instance {
            mesh,
            transform,
            origin,
            materials,
            slot_alpha: vec![1.0; slots],
            slot_light: vec![1.0; slots],
            slot_night: vec![1.0; slots],
            visible: true,
            slot_uv: vec![[0.0; 2]; slots],
            interior: 0.0,
            interior_lamps: 0,
            base: 0,
            bounds: InstanceBounds::default(),
            surface: true,
            presurface: false,
            render_phase: RenderPhase::Normal,
            surface_bias: true,
            blend_sort_origin: None,
            lod: (0.0, f32::MAX),
            blob: false,
            ground_layer: false,
            decal: false,
            object_radius: 0.0,
            detail: 1.0,
            any_distance: false,
            near_only: None,
            mirror_only: false,
            omsi_caster: false,
            ordered: false,
            casts_shadow: false,
            roof: None,
        });
        if scene.bounds_known.get(mesh).copied().unwrap_or(false) {
            scene.bounds_users.push(scene.instances.len() - 1);
        }
        scene.instances.len() - 1
    }

    /// A vehicle's shadow blob: a surface instance (see `Instance::blob`).
    pub fn add_shadow_blob_instance(
        &self,
        scene: &mut Scene,
        mesh: MeshId,
        origin: DVec3,
        transform: Mat4,
        materials: Vec<MaterialId>,
    ) -> usize {
        let i = self.add_surface_instance(scene, mesh, origin, transform, materials);
        scene.instances[i].blob = true;
        i
    }

    /// Note that instance `i` needs its per-draw entries rewritten.
    pub(crate) fn mark_changed(scene: &mut Scene, i: usize) {
        if scene.dirty || i >= scene.uploaded_instances {
            return;
        }
        if scene.changed_mark.len() < scene.instances.len() {
            scene.changed_mark.resize(scene.instances.len(), false);
        }
        if !scene.changed_mark[i] {
            scene.changed_mark[i] = true;
            scene.changed.push(i);
        }
    }

    /// Draw instance `instance` with mesh `mesh` from now on (a vehicle's skinned mesh gets
    /// a copy of its own).
    pub fn set_instance_mesh(&self, scene: &mut Scene, instance: usize, mesh: MeshId) {
        if scene.instances[instance].mesh != mesh {
            scene.instances[instance].mesh = mesh;
            scene.bounds_users_stale = true;
            Self::mark_changed(scene, instance);
        }
    }

    /// Change an instance transform (re-uploaded on the next `prepare`).
    pub fn set_transform(
        &self,
        scene: &mut Scene,
        instance: usize,
        origin: DVec3,
        transform: Mat4,
    ) {
        let i = &mut scene.instances[instance];
        if i.transform != transform || i.origin != origin {
            i.transform = transform;
            i.origin = origin;
            Self::mark_changed(scene, instance);
        }
    }

    /// Hide an instance for good (its tile was unloaded); `recycle_instance` reuses it.
    pub fn remove_instance(&self, scene: &mut Scene, instance: usize) {
        if instance >= scene.instances.len() {
            return;
        }
        self.set_params(scene, instance, &[], false, &[]);
        scene.instances[instance].lod = (0.0, f32::MAX);
        scene.instances[instance].interior_lamps = 0;
    }

    /// Cut the scene's arrays down to these lengths (their tails are freed slots nothing
    /// refers to any more); the per-draw buffers are rebuilt when instances went.
    pub fn truncate(
        &self,
        scene: &mut Scene,
        meshes: usize,
        textures: usize,
        materials: usize,
        instances: usize,
    ) {
        fn cut<T>(v: &mut Vec<T>, n: usize) {
            if n < v.len() {
                v.truncate(n);
                if v.capacity() > v.len() * 2 + 64 {
                    v.shrink_to_fit();
                }
            }
        }
        cut(&mut scene.meshes, meshes);
        scene.bounds_known.truncate(meshes);
        cut(&mut scene.textures, textures);
        cut(&mut scene.materials, materials);
        // hidden (freed) instances may still name a cut mesh or material: slot 0 instead,
        // they are not drawn and whoever takes them over sets their own
        let (nm, nt) = (scene.meshes.len(), scene.materials.len());
        scene.bounds_users_stale = true;
        for inst in scene.instances.iter_mut() {
            if inst.mesh >= nm {
                debug_assert!(!inst.visible, "a drawn instance lost its mesh");
                inst.mesh = 0;
                inst.visible = false;
            }
            for m in inst.materials.iter_mut() {
                if *m >= nt {
                    *m = 0;
                }
            }
        }
        if instances < scene.instances.len() {
            cut(&mut scene.instances, instances);
            scene.changed.retain(|i| *i < instances);
            scene.changed_mark.truncate(instances);
            scene.uploaded_instances = scene.uploaded_instances.min(instances);
            scene.dirty = true;
        }
    }

    /// Move the instance just added into the removed instance `into`, which must have as many
    /// material slots (its per-draw entries are rewritten in place). Returns the id the
    /// instance has now.
    pub fn recycle_instance(&self, scene: &mut Scene, new: usize, into: usize) -> usize {
        if new + 1 != scene.instances.len() || into >= new || new < scene.uploaded_instances {
            return new;
        }
        if scene.instances[into].slot_alpha.len() != scene.instances[new].slot_alpha.len() {
            return new;
        }
        let mut inst = scene.instances.pop().unwrap();
        inst.base = scene.instances[into].base;
        scene.instances[into] = inst;
        scene.bounds_users_stale = true;
        Self::mark_changed(scene, into);
        into
    }

    /// Material slots of an instance (what `recycle_instance` must match).
    pub fn instance_slots(&self, scene: &Scene, instance: usize) -> usize {
        scene
            .instances
            .get(instance)
            .map(|i| i.slot_alpha.len())
            .unwrap_or(0)
    }
}
