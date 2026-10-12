//! The scene: its instances and their bounds, the render phases, and the renderer's
//! methods that place and tune instances.

use super::*;

pub type MeshId = usize;

pub type TextureId = usize;

pub type MaterialId = usize;

pub struct GpuMesh {
    pub(crate) vertex_buf: wgpu::Buffer,
    pub(crate) index_buf: wgpu::Buffer,
    /// Where the mesh lies in its page's buffers (pages hold many meshes); `page` is its
    /// page in the scene's list, `u32::MAX` for a page of its own.
    pub(crate) page: u32,
    pub(crate) base_vertex: i32,
    pub(crate) first_index: u32,
    pub(crate) vertex_offset: u64,
    pub(crate) vertex_bytes: u64,
    pub(crate) index_bytes: u64,
    /// Which mesh this is (a page's range handed to another mesh is another): what the ray
    /// tracer's structures are kept by, with the mesh's place.
    pub(crate) gen: u64,
    pub ranges: Vec<(u32, u32, u32)>,
    pub bounds_center: Vec3,
    pub bounds_radius: f32,
    /// Back faces are culled (a content mesh, see `MeshData::one_sided`).
    pub one_sided: bool,
    /// Source asset for the optional draw-cost audit.
    pub source: Option<String>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct InstanceBounds {
    pub(crate) centre: Vec3,
    pub(crate) radius: f32,
    pub(crate) scale: f32,
}

impl InstanceBounds {
    pub(crate) fn new(mesh: &GpuMesh, transform: Mat4) -> Self {
        let scale = transform_scale(transform);
        Self {
            centre: transform.transform_point3(mesh.bounds_center),
            radius: mesh.bounds_radius * scale,
            scale,
        }
    }
}

pub(crate) const CULL_BLOCK: usize = 128;

pub(crate) const CULL_BLOCK_REBUILDS: usize = 8;

pub(crate) fn transform_scale(transform: Mat4) -> f32 {
    transform
        .x_axis
        .truncate()
        .length_squared()
        .max(transform.y_axis.truncate().length_squared())
        .max(transform.z_axis.truncate().length_squared())
        .sqrt()
}

/// The ordered world passes used by OMSI for ground and scenery geometry.
///
/// Keep these phases separate in the main pass: a later phase must be able to sit over an
/// earlier blended surface, while depth testing still lets nearer geometry win.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
#[repr(u8)]
pub enum RenderPhase {
    PreSurface = 0,
    Terrain = 1,
    Surface = 2,
    Spline = 3,
    OnSurface = 4,
    BeforeNormal = 5,
    #[default]
    Normal = 6,
    AfterNormal = 7,
    AfterVehicles = 8,
}

impl RenderPhase {
    pub(crate) const COUNT: usize = 9;
    pub(crate) const DRAW_ORDER: [Self; Self::COUNT] = [
        Self::PreSurface,
        Self::Terrain,
        Self::Surface,
        Self::Spline,
        Self::OnSurface,
        Self::BeforeNormal,
        Self::Normal,
        Self::AfterNormal,
        Self::AfterVehicles,
    ];
}

pub struct Instance {
    pub mesh: MeshId,
    /// Transform relative to `origin` (rotation/scale plus a small translation).
    pub transform: Mat4,
    /// World position of the instance's local frame.
    pub origin: DVec3,
    /// Material per mesh material slot.
    pub materials: Vec<MaterialId>,
    /// Dynamic parameters: per material slot alpha multiplier; per instance visibility and
    /// uv offset.
    pub slot_alpha: Vec<f32>,
    /// Per material slot `[matl_lightmap]` strength (script variable).
    pub slot_light: Vec<f32>,
    /// Per material slot: is the `[matl_item]` variant (night map) active (0/1).
    pub slot_night: Vec<f32>,
    pub visible: bool,
    /// `[texcoordtransX/Y]` offset per material slot: every band of the SD200's roller
    /// blind scrolls on its own, so one offset for the whole mesh is not enough.
    pub slot_uv: Vec<[f32; 2]>,
    /// Interior light brightness (0..1) added as warm ambient (`[interiorlight]`), for an
    /// instance without lamps of its own (a passenger standing in a lit bus).
    pub interior: f32,
    /// The `[interiorlight]` lamps that light this mesh (its `[illumination_interior]`): the
    /// first of its run of slots in `Scene::interior_lights` times `LAMP_CODE_STRIDE` plus
    /// how many; 0 = none, `interior` stands in.
    pub interior_lamps: u32,
    /// First entry of this instance in the per-draw storage buffers (set by `prepare`).
    pub(crate) base: u32,
    /// Transformed local bounds, refreshed with the GPU instance data. The floating
    /// render origin is applied per view, so changing it cannot leave stale spheres.
    pub(crate) bounds: InstanceBounds,
    /// Surface geometry classification (roads, markings, crossings), used for culling and
    /// weather/shading. `surface_bias` independently selects the vertex shader's depth pull.
    pub surface: bool,
    /// `[rendertype] presurface`: drawn before terrain, including blended materials whose
    /// transparent texels write depth to reveal excavations below the ground.
    pub presurface: bool,
    /// OMSI world-pass order. Most instances use `Normal`; road/surface assets are assigned
    /// their authored phase by the scene loader.
    pub render_phase: RenderPhase,
    /// Apply a planar view-space depth pull. Metric-lifted roads and ordered scenery phases skip
    /// it, so their placement does not change with the camera angle.
    pub surface_bias: bool,
    /// OMSI sorts blended spline pieces by their placement origin (horizontal distance), not
    /// the containing map tile's shared origin.
    pub blend_sort_origin: Option<DVec3>,
    /// Screen-size range [min, max) in which this instance is drawn (`[LOD]` levels).
    pub lod: (f32, f32),
    /// A vehicle's flat shadow blob (`[isshadow]`, a surface). It is drawn always, as OMSI
    /// draws it: it stood in for the sun shadow map only while that was off, and with the
    /// map on (the usual case, Enhanced always) no bus had anything under it - the sun's
    /// shadow falls beside the bus at any but a noon sun, while the blob is the sky light
    /// the body keeps off the road, which no shadow map and no screen-space AO supplies.
    pub blob: bool,
    /// A painted ground layer (`[groundtex]` through its brush mask): a surface that is the
    /// ground itself, so it does not get the roads' pull towards the camera (see vs_main).
    pub ground_layer: bool,
    /// A surface object (a crossing, markings) over the road splines. Legacy instances get a
    /// small view-space pull; ordered OMSI surfaces keep the flag for shading but skip it.
    pub decal: bool,
    /// The whole object this mesh belongs to (see `set_object_culling`): the radius of a
    /// sphere about `origin` that holds all of it (0 = the mesh is judged on its own sphere),
    /// its `[detail_factor]` and whether it is kept at any distance (`[noDistanceCheck]`).
    pub object_radius: f32,
    pub detail: f32,
    pub any_distance: bool,
    /// Drawn only while the camera stands in this area of the ground (world x0, y0, x1, y1):
    /// a stand-in for far tiles, which OMSI has loaded only around its own tile (see
    /// `set_near_only`).
    pub near_only: Option<[f64; 4]>,
    /// Seen only in the mirrors and other views drawn into textures, not in the window's
    /// picture: the driver at the wheel while the player looks from the driver's seat (the
    /// figure would fill the view, but the mirrors show him as OMSI does).
    pub mirror_only: bool,
    /// The model marks the mesh `[shadow]`: one OMSI casts a shadow from (with the
    /// option `omsi_shadow_casters` only these do).
    pub omsi_caster: bool,
    /// It may cast a sun shadow: every ordinary instance, no surface - a surface lies on
    /// the ground, and a caster in one plane with what it falls on paints dark patches into
    /// it - except a spline standing clear of the ground (a bridge deck, an elevated
    /// railway), which is raised with `set_casts_shadow`.
    pub casts_shadow: bool,
    /// Part of a vehicle whose roof lies this high over its origin (model frame): what faces
    /// up under the roof (the floor, the seats) is out of the weather - no snow nor wet on
    /// it. (Only the vehicle the camera is in was spared, by its box; every other bus showed
    /// its saloon under snow through the windows.)
    pub roof: Option<f32>,
    /// Drawn with every slot in model order among the blended draws, as Omsi.exe draws a
    /// model: mesh after mesh, each material subset with its own states and depth write
    /// (0x7c32c4 -> 0x7fd6c4, DrawSubset), not its opaque parts first. Set on the models
    /// where it matters: a blended slot that writes depth before an opaque one (a body with
    /// `[matl_alpha] 2` listed before its interior hides the interior as in the original,
    /// instead of showing it through the paint's alpha).
    pub ordered: bool,
}

pub struct Scene {
    pub meshes: Vec<GpuMesh>,
    pub textures: Vec<GpuTexture>,
    /// Texture slot read by procedural rain films.
    pub(crate) glass_slot: Option<TextureId>,
    pub materials: Vec<Material>,
    pub instances: Vec<Instance>,
    /// World position everything is expressed relative to on the GPU (updated per frame).
    pub render_origin: DVec3,
    /// Point lights and coronas for the next frame (set by the app every frame).
    pub lights: Vec<PointLight>,
    /// The vehicles' `[interiorlight]` lamps, in slots each vehicle keeps
    /// (`Renderer::alloc_interior_lights`): they light only the meshes that name them.
    pub interior_lights: Vec<PointLight>,
    pub(crate) interior_free: Vec<(u32, u32)>,
    pub coronas: Vec<Corona>,
    /// Smoke particles for the next frame (set by the app every frame).
    pub smoke: Vec<SmokeParticle>,
    pub(crate) smoke_buf: Option<wgpu::Buffer>,
    pub(crate) smoke_count: u32,
    /// Runs of this frame's coronas by picture: (texture, first, count).
    /// (and whether the run belongs to the vehicle the camera is in, drawn after it)
    pub(crate) corona_runs: Vec<(u16, u32, u32, bool)>,
    pub(crate) model_buf: Option<GpuArray>,
    pub(crate) params_buf: Option<GpuArray>,
    pub(crate) light_buf: Option<GpuArray>,
    pub(crate) grid_buf: Option<GpuArray>,
    pub(crate) corona_buf: Option<wgpu::Buffer>,
    pub(crate) corona_count: u32,
    /// The frame's draw list (see `Batch`), shared by the shadow, prepass and main passes.
    pub(crate) draw_buf: Option<GpuArray>,
    pub(crate) camera_bind_group: Option<wgpu::BindGroup>,
    pub(crate) shadow_bind_group: Option<wgpu::BindGroup>,
    pub(crate) sky_bind_group: Option<wgpu::BindGroup>,
    /// HUD images drawn after the scene: (texture, rect in pixels x0,y0,x1,y1).
    pub overlays: Vec<(TextureId, [f32; 4])>,
    /// Overlay textures that hold premultiplied alpha (drawn by `omsi-ui`, e.g. the
    /// navigator) rather than straight alpha.
    pub premultiplied: std::collections::HashSet<TextureId>,
    /// Overlay textures drawn on their side (their u down the rectangle, v across it): the
    /// mirror panels of a glass whose mesh lays the picture so.
    pub transposed: std::collections::HashSet<TextureId>,
    /// Per overlay: the texture its bind group was made for, its rect buffer and the group
    /// (kept between frames; only the rect is rewritten).
    pub(crate) overlay_res: Vec<(TextureId, wgpu::Buffer, wgpu::BindGroup, [f32; 8])>,
    /// Structural change (render origin moved, buffers too small): everything is rebuilt.
    pub(crate) dirty: bool,
    /// How many instances (and per-draw entries) the buffers hold; instances added since
    /// are appended to the buffers instead of rebuilding them, as long as they fit.
    pub(crate) uploaded_instances: usize,
    pub(crate) uploaded_entries: u32,
    /// Instances whose transform or parameters changed since the last `prepare`: only
    /// their entries are rewritten. Rebuilding the whole per-draw buffer for 17 000 objects
    /// because one bus moved was the biggest single CPU cost of a frame.
    pub(crate) changed: Vec<usize>,
    pub(crate) mesh_pages: Vec<MeshPage>,
    pub(crate) changed_mark: Vec<bool>,
    pub(crate) origin_moved: bool,
    pub(crate) cache_bounds: bool,
    pub(crate) bounds_meshes: Vec<bool>,
    /// The meshes ever reshaped (skinned), and the instances drawing one of them, so that a
    /// new pose rescans those instead of the whole scene; stale when an instance's mesh changes.
    pub(crate) bounds_known: Vec<bool>,
    pub(crate) bounds_users: Vec<usize>,
    pub(crate) bounds_users_stale: bool,
    /// A mesh slot was freed or taken by another mesh: its instances' bounds are redone once
    /// by a scan of the whole scene, without counting the slot as reshaped.
    pub(crate) bounds_rescan: bool,
    pub(crate) block_bounds: Vec<(DVec3, DVec3)>,
    pub(crate) block_dirty: Vec<bool>,
    pub(crate) block_cursor: usize,
    pub(crate) bounds_dirty: bool,
    /// What the per-draw buffers hold, kept on the CPU: changed entries are written here
    /// and uploaded as a few merged ranges. Every `write_buffer` makes a new staging buffer
    /// on the GPU, and one per changed vehicle or person was a hundred of them a frame.
    pub(crate) cpu_models: Vec<[[f32; 4]; 4]>,
    pub(crate) cpu_params: Vec<[f32; 4]>,
    /// The light grid and lights as last uploaded, so that unchanged ones are not sent again.
    pub(crate) last_grid: Vec<u32>,
    /// The street lamps that had a shadow map last frame (their places in centimetres):
    /// they keep it against a lamp only a little stronger (`prepare_lights`).
    pub(crate) lamp_shadow_last: Vec<[i64; 3]>,
    pub(crate) last_lights: Vec<u8>,
    /// Material bind groups and uniform buffers made since the last `prepare`, by what they
    /// hold: materials made in one go with the same textures and values share them (a C2's
    /// 965 materials need about a tenth as many). Only for a frame, so that nothing keeps a
    /// freed or replaced texture alive.
    pub(crate) bind_groups: HashMap<BindKey, (wgpu::BindGroup, wgpu::Buffer)>,
    /// The number of each material look seen so far, by its key's hash (see `Material::look`).
    pub(crate) looks: hashbrown::HashMap<u64, u32>,
    /// The PBR maps of a diffuse texture (register them before making its materials).
    pub pbr_maps: HashMap<TextureId, PbrMaps>,
    /// Textures that are a season's snow pictures (`WinterSnow` folders): a material drawn
    /// with one shows its snow as the map made it, as OMSI 2 shows snow, and gets no snow
    /// laid over it (register them before making their materials).
    pub snow_textures: std::collections::HashSet<TextureId>,
}

/// `MaterialUniform::ambient`'s w: 1 for a material whose texture is a season's snow
/// picture (`Scene::snow_textures`).
pub(crate) fn snow_texture_flag(scene: &Scene, texture: Option<TextureId>) -> f32 {
    if texture.is_some_and(|t| scene.snow_textures.contains(&t)) {
        1.0
    } else {
        0.0
    }
}

impl Scene {
    /// The look number of a material bind group made of `key` (numbers start at 1; 0 is the
    /// shared look of the opaque depth-only draws).
    pub(crate) fn look(&mut self, key: &BindKey) -> u32 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        key.hash(&mut h);
        self.intern_look(h.finish())
    }

    pub(crate) fn intern_look(&mut self, hash: u64) -> u32 {
        let next = self.looks.len() as u32 + 1;
        *self.looks.entry(hash).or_insert(next)
    }
}

impl Scene {
    /// Bytes of one texture on the GPU (0 for a freed slot or an unknown id).
    pub fn texture_bytes_of(&self, id: TextureId) -> u64 {
        self.textures.get(id).map(|t| t.bytes).unwrap_or(0)
    }

    /// The size of one texture.
    pub fn texture_size_of(&self, id: TextureId) -> Option<(u32, u32)> {
        self.textures.get(id).map(|t| t.size)
    }

    /// The GPU format of one texture, for statistics.
    pub fn texture_format_of(&self, id: TextureId) -> String {
        self.textures
            .get(id)
            .map(|t| format!("{:?}", t.texture.format()))
            .unwrap_or_default()
    }

    /// Bytes the meshes' pages take on the GPU, each page once, whatever share of it is in use.
    pub fn mesh_page_bytes(&self) -> u64 {
        let shared: u64 = self
            .mesh_pages
            .iter()
            .map(|p| p.vertex.size() + p.index.size())
            .sum();
        // (a set: without base vertices every mesh has buffers of its own, thousands of them)
        let mut own: std::collections::HashSet<&wgpu::Buffer> = std::collections::HashSet::new();
        for m in self
            .meshes
            .iter()
            .filter(|m| m.page == u32::MAX && !m.ranges.is_empty())
        {
            if own.insert(&m.vertex_buf) {
                own.insert(&m.index_buf);
            }
        }
        shared + own.iter().map(|b| b.size()).sum::<u64>()
    }

    /// Bytes on the GPU: (textures, mesh buffers, per-draw and light buffers). Freed slots
    /// share one small placeholder, which is not counted.
    pub fn gpu_bytes(&self) -> (u64, u64, u64) {
        let tex = self.textures.iter().map(|t| t.bytes).sum();
        let mesh = self.mesh_page_bytes();
        let other = [
            &self.model_buf,
            &self.params_buf,
            &self.light_buf,
            &self.grid_buf,
            &self.draw_buf,
        ]
        .iter()
        .filter_map(|b| b.as_ref())
        .map(|b| b.gpu_bytes())
        .sum::<u64>()
            + self.corona_buf.as_ref().map_or(0, |b| b.size());
        (tex, mesh, other)
    }
}

impl Renderer {
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

    /// Interior light brightness of an instance (warm ambient inside vehicles).
    /// Whether the model marks the instance's mesh `[shadow]` (see [`Instance::omsi_caster`]).
    pub fn set_omsi_caster(&self, scene: &mut Scene, instance: usize, on: bool) {
        if let Some(i) = scene.instances.get_mut(instance) {
            i.omsi_caster = on;
        }
    }

    /// Draw an instance with all its slots in model order (see [`Instance::ordered`]).
    pub fn set_ordered(&self, scene: &mut Scene, instance: usize, on: bool) {
        if let Some(i) = scene.instances.get_mut(instance) {
            i.ordered = on;
        }
    }

    /// Let an instance cast a sun shadow or not (see [`Instance::casts_shadow`]).
    pub fn set_casts_shadow(&self, scene: &mut Scene, instance: usize, on: bool) {
        if let Some(i) = scene.instances.get_mut(instance) {
            i.casts_shadow = on;
        }
    }

    /// The vehicle roof height of an instance (see [`Instance::roof`]).
    pub fn set_roof(&self, scene: &mut Scene, instance: usize, roof: Option<f32>) {
        if let Some(i) = scene.instances.get_mut(instance) {
            if i.roof != roof {
                i.roof = roof;
                Self::mark_changed(scene, instance);
            }
        }
    }

    /// Show an instance only in the mirrors (see [`Instance::mirror_only`]).
    pub fn set_mirror_only(&self, scene: &mut Scene, instance: usize, on: bool) {
        if let Some(i) = scene.instances.get_mut(instance) {
            i.mirror_only = on;
        }
    }

    pub fn set_interior(&self, scene: &mut Scene, instance: usize, interior: f32) {
        let i = &mut scene.instances[instance];
        if (i.interior - interior).abs() > 1e-4 {
            i.interior = interior;
            Self::mark_changed(scene, instance);
        }
    }

    /// Per-slot lightmap strengths (`[matl_lightmap]` variables).
    pub fn set_slot_light(&self, scene: &mut Scene, instance: usize, light: &[f32]) {
        let i = &mut scene.instances[instance];
        let mut changed = false;
        for (k, l) in i.slot_light.iter_mut().enumerate() {
            let v = light.get(k).copied().unwrap_or(1.0);
            changed |= *l != v;
            *l = v;
        }
        if changed {
            Self::mark_changed(scene, instance);
        }
    }

    /// Per-slot `[matl_item]` activity (night maps of `[matl_change]` variants).
    pub fn set_slot_night(&self, scene: &mut Scene, instance: usize, night: &[f32]) {
        let i = &mut scene.instances[instance];
        let mut changed = false;
        for (k, l) in i.slot_night.iter_mut().enumerate() {
            let v = night.get(k).copied().unwrap_or(1.0);
            changed |= *l != v;
            *l = v;
        }
        if changed {
            Self::mark_changed(scene, instance);
        }
    }

    /// Restrict an instance to a screen-size range (LOD levels).
    pub fn set_lod_range(&self, scene: &mut Scene, instance: usize, min: f32, max: f32) {
        scene.instances[instance].lod = (min, max);
    }

    /// Judge this mesh as part of a whole object, the way the original does: the object is
    /// drawn or left out as one - by its screen size against `performance_minObjSize` times
    /// its `[detail_factor]`, and beyond `performance_maxObjDist` unless it says
    /// `[noDistanceCheck]` - and its `[LOD]` level is chosen by the object's size, not by
    /// each mesh's own. `radius` (m, in the instance's own scale) holds the whole object
    /// about the instance's origin. Judged mesh by mesh, the small parts of a bus (mirrors,
    /// wipers, lamps) went missing at a distance while its body was still there, and the
    /// meshes of one LOD level switched at different distances.
    pub fn set_object_culling(
        &self,
        scene: &mut Scene,
        instance: usize,
        radius: f32,
        detail: f32,
        any_distance: bool,
    ) {
        let i = &mut scene.instances[instance];
        i.object_radius = radius.max(0.0);
        i.detail = if detail > 0.0 { detail } else { 1.0 };
        i.any_distance = any_distance;
    }

    /// Draw an instance only while the camera stands in `area` (world x0, y0, x1, y1 on the
    /// ground; None: wherever it is loaded).
    pub fn set_near_only(&self, scene: &mut Scene, instance: usize, area: Option<[f64; 4]>) {
        scene.instances[instance].near_only = area;
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

    /// Choose the render origin. Everything is re-uploaded when it moves.
    pub fn set_render_origin(&self, scene: &mut Scene, origin: DVec3) {
        if scene.render_origin != origin {
            if origin.z != scene.render_origin.z || scene.model_buf.is_none() {
                scene.dirty = true;
            } else {
                scene.origin_moved = true;
            }
            scene.render_origin = origin;
        }
    }

    /// Bounding sphere of an instance relative to the render origin: the mesh's sphere
    /// through the instance transform (the model matrix is that transform moved by the
    /// origin, so no matrix product is needed), the radius scaled by the largest axis.
    pub(crate) fn bounding_sphere(scene: &Scene, i: &Instance) -> (Vec3, f32) {
        let b = if scene.cache_bounds {
            i.bounds
        } else {
            InstanceBounds::new(&scene.meshes[i.mesh], i.transform)
        };
        (
            b.centre + (i.origin - scene.render_origin).as_vec3(),
            b.radius,
        )
    }

    pub(crate) fn instance_scale(scene: &Scene, i: &Instance) -> f32 {
        if scene.cache_bounds {
            i.bounds.scale
        } else {
            transform_scale(i.transform)
        }
    }

    pub(crate) fn mesh_bounds_changed(scene: &mut Scene, mesh: MeshId) {
        scene.bounds_meshes.resize(scene.meshes.len(), false);
        scene.bounds_meshes[mesh] = true;
        scene.bounds_dirty = true;
        scene.bounds_known.resize(scene.meshes.len(), false);
        if !scene.bounds_known[mesh] {
            scene.bounds_known[mesh] = true;
            scene.bounds_users_stale = true;
        }
    }

    /// Slot `mesh` holds another mesh now (freed, or recycled): no longer a reshaped one.
    pub(crate) fn mesh_slot_replaced(scene: &mut Scene, mesh: MeshId) {
        scene.bounds_meshes.resize(scene.meshes.len(), false);
        scene.bounds_meshes[mesh] = true;
        scene.bounds_dirty = true;
        scene.bounds_rescan = true;
        if scene.bounds_known.get(mesh).copied().unwrap_or(false) {
            scene.bounds_known[mesh] = false;
            scene.bounds_users_stale = true;
        }
    }

    pub(crate) fn prepare_bounds(scene: &mut Scene) {
        if !scene.cache_bounds {
            return;
        }
        let blocks = scene.instances.len().div_ceil(CULL_BLOCK);
        let all = scene.dirty || scene.block_bounds.len() != blocks;
        scene
            .block_bounds
            .resize(blocks, (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN)));
        scene.block_dirty.resize(blocks, false);
        let grow = |bb: &mut (DVec3, DVec3), i: &Instance| {
            let c = i.origin + i.bounds.centre.as_dvec3();
            let r = i.bounds.radius as f64;
            bb.0 = bb.0.min(c - r);
            bb.1 = bb.1.max(c + r);
        };
        if scene.dirty {
            for i in &mut scene.instances {
                i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
            }
        } else {
            for k in scene.uploaded_instances..scene.instances.len() {
                let i = &mut scene.instances[k];
                i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
                grow(&mut scene.block_bounds[k / CULL_BLOCK], i);
            }
            for &idx in &scene.changed {
                if let Some(i) = scene.instances.get_mut(idx) {
                    i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
                    grow(&mut scene.block_bounds[idx / CULL_BLOCK], i);
                    scene.block_dirty[idx / CULL_BLOCK] = true;
                }
            }
            if scene.bounds_dirty {
                // Skinning can alter a shared mesh without changing any model matrix.
                // Scan once per pose update, rather than once per shadow/mirror view.
                if scene.bounds_users_stale {
                    let known = &scene.bounds_known;
                    scene.bounds_users = (0..scene.instances.len())
                        .filter(|&k| known.get(scene.instances[k].mesh).copied().unwrap_or(false))
                        .collect();
                    scene.bounds_users_stale = false;
                }
                let all: Vec<usize>;
                let scan: &[usize] = if std::mem::take(&mut scene.bounds_rescan) {
                    all = (0..scene.instances.len()).collect();
                    &all
                } else {
                    &scene.bounds_users
                };
                for &k in scan {
                    let Some(i) = scene.instances.get_mut(k) else {
                        continue;
                    };
                    if scene.bounds_meshes.get(i.mesh).copied().unwrap_or(false) {
                        i.bounds = InstanceBounds::new(&scene.meshes[i.mesh], i.transform);
                        grow(&mut scene.block_bounds[k / CULL_BLOCK], i);
                        scene.block_dirty[k / CULL_BLOCK] = true;
                    }
                }
            }
        }
        if scene.bounds_dirty {
            scene.bounds_meshes.fill(false);
            scene.bounds_dirty = false;
        }
        let rebuild = |scene: &mut Scene, b: usize| {
            let mut bb = (DVec3::splat(f64::MAX), DVec3::splat(f64::MIN));
            for i in
                &scene.instances[b * CULL_BLOCK..((b + 1) * CULL_BLOCK).min(scene.instances.len())]
            {
                grow(&mut bb, i);
            }
            scene.block_bounds[b] = bb;
            scene.block_dirty[b] = false;
        };
        if all {
            for b in 0..blocks {
                rebuild(scene, b);
            }
            return;
        }
        let mut left = CULL_BLOCK_REBUILDS;
        for _ in 0..blocks {
            if left == 0 {
                break;
            }
            scene.block_cursor = (scene.block_cursor + 1) % blocks.max(1);
            if scene.block_dirty[scene.block_cursor] {
                rebuild(scene, scene.block_cursor);
                left -= 1;
            }
        }
    }

    pub(crate) fn cull_blocks(scene: &Scene) -> Option<Vec<(Vec3, f32)>> {
        (scene.cache_bounds
            && scene.block_bounds.len() == scene.instances.len().div_ceil(CULL_BLOCK))
        .then(|| {
            scene
                .block_bounds
                .iter()
                .map(|&(lo, hi)| {
                    if lo.x > hi.x {
                        (Vec3::ZERO, 0.0)
                    } else {
                        (
                            ((lo + hi) * 0.5 - scene.render_origin).as_vec3(),
                            ((hi - lo).length() * 0.5) as f32,
                        )
                    }
                })
                .collect()
        })
    }

    /// A dynamic alpha value is never allowed to fade an opaque body panel; only
    /// blended materials follow the script value. An alpha-tested slot is cut out by its
    /// texture or transmap alone: the Thüringer Wald buses put `[alphascale]
    /// Envir_Brightness` on their transmapped body and roof (`[matl_alpha] 1`), which is 0
    /// at night, and scaled by it the whole roof went at dusk - with alpha to coverage
    /// under MSAA the colour pass drew none of its samples - while in OMSI it stays.
    /// Except a blended slot with a declared `[matl_transmap]`: Omsi.exe's transmap stage
    /// (0x7ffeb7) replaces the diffuse alpha that `[alphascale]` scaled (0x7feb8f).
    pub fn clamp_slot_alpha(alpha: f32, material_alpha: AlphaMode, transmap_declared: bool) -> f32 {
        match material_alpha {
            AlphaMode::Opaque | AlphaMode::Test => 1.0,
            AlphaMode::Blend if transmap_declared => 1.0,
            AlphaMode::Blend => alpha,
        }
    }

    /// Change the dynamic parameters of an instance. `slot_alpha` entries beyond the mesh's
    /// slot count are ignored; missing ones keep 1.0.
    pub fn set_params(
        &self,
        scene: &mut Scene,
        instance: usize,
        slot_alpha: &[f32],
        visible: bool,
        slot_uv: &[[f32; 2]],
    ) {
        let i = &mut scene.instances[instance];
        let mut changed = i.visible != visible;
        for (k, a) in i.slot_alpha.iter_mut().enumerate() {
            let requested = slot_alpha.get(k).copied().unwrap_or(1.0);
            let v = i
                .materials
                .get(k)
                .and_then(|id| scene.materials.get(*id))
                .map_or(requested, |m| {
                    Self::clamp_slot_alpha(requested, m.alpha, m.transmap_declared())
                });
            changed |= *a != v;
            *a = v;
        }
        for (k, u) in i.slot_uv.iter_mut().enumerate() {
            let v = slot_uv.get(k).copied().unwrap_or([0.0; 2]);
            changed |= *u != v;
            *u = v;
        }
        i.visible = visible;
        if changed {
            Self::mark_changed(scene, instance);
        }
    }
}
