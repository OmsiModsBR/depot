//! What a vehicle shows each frame: its animated meshes, its shadow on the ground under
//! its wheels, and the reports of how its wheels sit.

use crate::particles::ParticleSet;
use glam::{DVec3, Mat4, Quat, Vec3, Vec4};
use super::props::override_slot;
use super::vehicle_type::VehicleType;
use super::VehicleInstance;

impl VehicleInstance {
    /// Animations and per-mesh material properties after the scripts ran.
    pub(super) fn update_visuals(&mut self, dt: f32) {
        // coupled vehicles come first, because they write the joint's angles this vehicle's
        // plates and bellows turn with (a frame late, the joint lagged behind the rear section)
        self.update_trailers(dt);
        for (i, a) in self.animators.iter_mut().enumerate() {
            self.mesh_transforms[i] = a.update(dt, &self.state.vars);
        }
        crate::anim::apply_parents(&self.animators, &mut self.mesh_transforms);
        static DEBUG_ANIM: std::sync::OnceLock<Option<String>> = std::sync::OnceLock::new();
        if let Some(want) = DEBUG_ANIM.get_or_init(|| omsi_cfg::flags::DEBUG_ANIM.text()) {
            for (i, m) in self.ty.meshes.iter().enumerate() {
                let file = &self.ty.model.meshes[m.def_index].file;
                if file
                    .to_ascii_lowercase()
                    .contains(&want.to_ascii_lowercase())
                {
                    let a = &self.animators[i];
                    let vals: Vec<(String, f32, f32, [f32; 3], [f32; 3])> = a
                        .anims
                        .iter()
                        .map(|(d, o, st)| {
                            (
                                d.variable.clone(),
                                st.var
                                    .map(|v| self.state.vars[v as usize])
                                    .unwrap_or(f32::NAN),
                                st.value,
                                o.x_axis.truncate().to_array(),
                                o.w_axis.truncate().to_array(),
                            )
                        })
                        .collect();
                    log::info!(
                        "anim {file}: {vals:?} transform {:?}",
                        self.mesh_transforms[i]
                            .to_cols_array()
                            .map(|v| (v * 100.0).round() / 100.0)
                    );
                }
            }
        }
        self.props_plan.refresh(&self.ty, &self.var_index);
        self.props_plan
            .apply(&self.state.vars, &mut self.mesh_props);
        // Dirt overlays are engine-owned layers. A model may declare Dirt_Norm or
        // Dirt_Wiped in its cfg without putting the name in the script varlist; the
        // generic alphascale fallback is 1.0 in that case, which makes Dreck.tga an
        // opaque full-body decal. Resolve these values explicitly for every vehicle.
        for (i, vm) in self.ty.meshes.iter().enumerate() {
            let def = &self.ty.model.meshes[vm.def_index];
            for m in &def.materials {
                let Some(name) = m.alphascale.as_deref().map(str::trim) else {
                    continue;
                };
                // (compared in place: a lower-case copy per material and frame of every AI
                // vehicle was a steady stream of allocations)
                let value = if name.eq_ignore_ascii_case("dirt_norm") {
                    Some(self.dirt)
                } else if name.eq_ignore_ascii_case("dirt_wiped") {
                    Some(self.var("Dirt_Wiped").unwrap_or(0.0))
                } else {
                    None
                };
                let Some(value) = value else { continue };
                if let Some(slot) = override_slot(&vm.materials, m) {
                    if let Some(alpha) = self.mesh_props[i].slot_alpha.get_mut(slot) {
                        *alpha = value.clamp(0.0, 1.0);
                    }
                }
            }
        }
        // the lamps come on and go out with their `timeconst`
        let mut lf = std::mem::take(&mut self.light_fade);
        let mut part_fades: Vec<Vec<f32>> = self.trailers.iter_mut().map(|t| std::mem::take(&mut t.light_fade)).collect();
        let value = |n: &str| -> f32 { n.trim().parse::<f32>().ok().or_else(|| self.var(n.trim())).unwrap_or(0.0) };
        let fade = |model: &omsi_model::Model, fades: &mut Vec<f32>| {
            let mut k = 0;
            for md in &model.meshes {
                for l in &md.light_enh_2 {
                    let target = (value(&l.variable) * if l.factor > 0.0 { l.factor } else { 1.0 }).clamp(0.0, 2.0);
                    // (a NaN stays NaN through clamp: the lamp's glow was drawn black on Windows)
                    let target = if target.is_finite() { target } else { 0.0 };
                    if fades.len() <= k {
                        fades.push(target);
                    }
                    let b = &mut fades[k];
                    if !b.is_finite() {
                        *b = target;
                    }
                    *b = if l.time_const <= 0.001 {
                        target
                    } else {
                        let rate = if target > *b { 1.0 } else { 1.31 } / l.time_const;
                        *b + (target - *b) * (1.0 - (-dt * rate).exp())
                    };
                    k += 1;
                }
            }
        };
        fade(&self.ty.model, &mut lf);
        for (t, f) in self.trailers.iter().zip(part_fades.iter_mut()) {
            fade(&t.ty.model, f);
        }
        self.light_fade = lf;
        for (t, f) in self.trailers.iter_mut().zip(part_fades) {
            t.light_fade = f;
        }
        // the particle systems ([smoke]: exhaust, boiling coolant, wheel spray) of the
        // vehicle and its coupled parts, which read the same scripts' variables
        if !self.particles.is_empty() || self.trailers.iter().any(|t| !t.particles.is_empty()) {
            let mut ps = std::mem::take(&mut self.particles);
            let mut parts: Vec<ParticleSet> = self.trailers.iter_mut().map(|t| std::mem::take(&mut t.particles)).collect();
            {
                // (each puff keeps the height of the road under it - the plane the wheels
                // stand on - for the renderer to fade it out into, see `particles`)
                let value = |n: &str| self.var(n).unwrap_or(0.0);
                ps.update_over(dt, self.position, self.body_rotation(), &|| self.particle_ground(), &value);
                for (t, set) in self.trailers.iter().zip(parts.iter_mut()) {
                    set.update_over(dt, t.position, t.body_rotation(), &|| [-t.ground_lift(), 0.0, 0.0], &value);
                }
            }
            self.particles = ps;
            for (t, set) in self.trailers.iter_mut().zip(parts) {
                t.particles = set;
            }
        }
    }

    /// How far each tyre of the model (and of the coupled parts) stands above (+) or sinks
    /// into (-) what is under it, in metres, from the meshes as they are animated right now:
    /// the check that the wheels touch the road. (mesh file, gap). An AI type keeps no
    /// vertices on the CPU, so its wheel meshes are read again ([`VehicleType::mesh_data`]).
    pub fn wheel_ground_gaps(&self) -> Vec<(String, f32)> {
        let under = |x: f64, y: f64, top: f64| -> Option<f64> {
            match (&self.contact, &self.ground) {
                (Some(c), _) => c.probe(x, y, top).below,
                (None, Some(g)) => g(x, y),
                _ => None,
            }
        };
        let is_wheel = |ty: &VehicleType, def_index: usize| {
            ty.model.meshes[def_index].animations.iter().any(|a| {
                a.variable
                    .to_ascii_lowercase()
                    .starts_with("wheel_rotation_")
            })
        };
        let mut out = Vec::new();
        let mut measure = |ty: &VehicleType, origin: DVec3, xf: &dyn Fn(usize) -> Mat4| {
            for (i, vm) in ty.meshes.iter().enumerate() {
                if !is_wheel(ty, vm.def_index) {
                    continue;
                }
                let m = xf(i);
                let Some(data) = ty.mesh_data(i) else {
                    continue;
                };
                let Some(low) = data
                    .positions
                    .iter()
                    .map(|p| m.transform_point3(*p))
                    .min_by(|a, b| a.z.total_cmp(&b.z))
                else {
                    continue;
                };
                let w = origin + low.as_dvec3();
                if let Some(g) = under(w.x, w.y, w.z + 0.3) {
                    out.push((ty.model.meshes[vm.def_index].file.clone(), (w.z - g) as f32));
                }
            }
        };
        measure(&self.ty, self.position, &|i| self.mesh_local_transform(i));
        for t in &self.trailers {
            measure(&t.ty, t.position, &|i| t.mesh_local_transform(i));
        }
        out
    }

    /// The static pose of an AI copy on the road: how far its origin stands above the plane
    /// its wheels touch (m) and the `Axle_Suspension` that goes with it. OMSI sets an AI copy
    /// `[ai_deltaheight]` lower than its model origin and lifts the wheels by as much through
    /// `Axle_Suspension` (every stock AI road model animates it), so it stands exactly like a
    /// driven one that has settled on its springs - the delta is that sag; left at zero the
    /// AI buses rode 10 cm high. On top of it, the model's wheel geometry (see `ai_lift`).
    /// `AiBody` adds its own travel on its springs, which is zero at rest.
    pub fn ai_rest_offset(&self) -> (f32, f32) {
        let delta = if self.ty.suspension_axles.is_empty() {
            0.0
        } else {
            self.ty.def.ai_delta_height
        };
        (delta + self.ai_lift, delta)
    }

    /// Where the wheel meshes turn (`OMSI_DEBUG_WHEELS`): for every mesh animated by a
    /// `Wheel_Rotation_*` or `Axle_Steering_*` variable, how far the centre of its vertices
    /// moves when that variable alone is set - a wheel spinning or steering about its own
    /// centre does not move at all, one with an offset pivot swings around. An AI type keeps
    /// no vertices on the CPU (`VehicleType::load_ai`), so the mesh is read again here.
    pub fn wheel_pivot_report(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (i, m) in self.ty.meshes.iter().enumerate() {
            let a = &self.animators[i];
            let names: Vec<&str> = a
                .anims
                .iter()
                .map(|(d, _, _)| d.variable.as_str())
                .filter(|v| v.starts_with("Wheel_Rotation") || v.starts_with("Axle_Steering"))
                .collect();
            if names.is_empty() {
                continue;
            }
            let Some(data) = self.ty.mesh_data(i) else {
                continue;
            };
            if data.positions.is_empty() {
                continue;
            }
            let (lo, hi) = data.positions.iter().fold(
                (Vec3::splat(f32::MAX), Vec3::splat(f32::MIN)),
                |(lo, hi), p| (lo.min(*p), hi.max(*p)),
            );
            let c = (lo + hi) * 0.5;
            let pv = m.pivot.col(3);
            let px = m.pivot.col(0);
            let mut line = format!("{} {}: centre ({:.3}, {:.3}, {:.3}) pivot ({:.3}, {:.3}, {:.3}) x ({:.2}, {:.2}, {:.2}) det {:.2}", self.ty.def.path.file_name().unwrap_or_default().to_string_lossy(), self.ty.model.meshes[m.def_index].file, c.x, c.y, c.z, pv.x, pv.y, pv.z, px.x, px.y, px.z, m.pivot.determinant());
            for name in names {
                let Some(id) = self.ty.program.var(name) else {
                    continue;
                };
                let mut vars = vec![0.0; self.state.vars.len()];
                vars[id as usize] = 0.35;
                let mut probe = a.clone();
                for (_, _, st) in probe.anims.iter_mut() {
                    st.initialized = false;
                }
                let t = probe.update(0.0, &vars);
                let moved = t.transform_point3(c) - c;
                let (axis, angle) = Quat::from_mat4(&t).to_axis_angle();
                line.push_str(&format!("; {name} 0.35 rad moves it ({:.3}, {:.3}, {:.3}), axis ({:.2}, {:.2}, {:.2}) {:.1} deg", moved.x, moved.y, moved.z, axis.x, axis.y, axis.z, angle.to_degrees()));
            }
            out.push(line);
        }
        out
    }
}

/// How far above the plane its wheels stand on a shadow blob is drawn (m): over the road's
/// crown between the wheels, and clear of it for the surfaces' depth bias.
pub const SHADOW_LIFT: f32 = 0.02;

/// How far over the wheel's own plane the face it stands on may lie for a `[isshadow]`
/// blob's plane (m): a kerb or a ramp, the step the AI's wheels climb
/// (as much as an AI car's wheels used to climb).
const SHADOW_STEP_UP: f64 = 0.6;

/// How far under it (m). Loose: the model's origin plane is the contact plane of the
/// *unloaded* springs, so a body at rest stands its ground 10-16 cm below its own plane,
/// and a map may put a vehicle down a little over its road. Farther down is another level -
/// a road under a bridge - and not the face this wheel stands on.
const SHADOW_STEP_DOWN: f64 = 3.0;

pub(super) fn is_shadow_mesh(ty: &VehicleType, i: usize) -> bool {
    ty.meshes
        .get(i)
        .is_some_and(|m| ty.model.meshes[m.def_index].is_shadow)
}

/// What one wheel of a body without a rigid body stands on at world `p` (the point on the
/// model's z = 0 plane under it): the drawn road there, within a step of the wheel - the same
/// level-limited probe the AI bodies ask (`ai_motion::AiBody::settle`). The plain height
/// sampler knows only x and y and gives the *highest* face, so a vehicle under a bridge or a
/// canopy had its `[isshadow]` blob laid onto the deck over it (the same sampler lifted the
/// coupled parts onto the bridge, #140). The plain sampler stays the fallback where the
/// tiles put no road face near the wheel.
pub(super) fn wheel_ground(
    contact: Option<&dyn crate::rigid::Ground>,
    ground: Option<&(dyn Fn(f64, f64) -> Option<f64> + Send + Sync)>,
    p: DVec3,
) -> Option<f64> {
    if let Some(c) = contact {
        if let Some(g) = c
            .probe(p.x, p.y, p.z + SHADOW_STEP_UP)
            .below
            .filter(|g| *g >= p.z - SHADOW_STEP_DOWN)
        {
            return Some(g);
        }
    }
    ground.and_then(|g| g(p.x, p.y))
}

/// Body frame → body frame with the plane z = 0 laid onto z = p[0] + p[1]·x + p[2]·y
/// (lifted by `SHADOW_LIFT`).
pub fn onto_plane(p: [f32; 3]) -> Mat4 {
    Mat4::from_cols(
        Vec4::new(1.0, 0.0, p[1], 0.0),
        Vec4::new(0.0, 1.0, p[2], 0.0),
        Vec4::Z,
        Vec4::new(0.0, 0.0, p[0] + SHADOW_LIFT, 1.0),
    )
}

/// The least-squares plane z = p[0] + p[1]·x + p[2]·y through `points`; level through their
/// mean height when they do not span a plane (one axle).
pub fn fit_plane(points: &[Vec3]) -> [f32; 3] {
    let n = points.len().max(1) as f32;
    let mean = points.iter().copied().sum::<Vec3>() / n;
    let (mut sxx, mut sxy, mut syy, mut sxz, mut syz) = (0.0f32, 0.0f32, 0.0f32, 0.0f32, 0.0f32);
    for p in points {
        let d = *p - mean;
        sxx += d.x * d.x;
        sxy += d.x * d.y;
        syy += d.y * d.y;
        sxz += d.x * d.z;
        syz += d.y * d.z;
    }
    let det = sxx * syy - sxy * sxy;
    let (b, c) = if det > 1e-3 * (sxx + syy).max(1e-6) {
        ((sxz * syy - syz * sxy) / det, (syz * sxx - sxz * sxy) / det)
    } else {
        (0.0, 0.0)
    };
    [mean.z - b * mean.x - c * mean.y, b, c]
}
