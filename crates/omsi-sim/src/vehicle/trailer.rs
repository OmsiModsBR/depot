//! The parts coupled behind a vehicle (an articulated bus's rear sections, a lorry's
//! trailers): how they hang on their coupling and where they are drawn.

use super::pose::camera_in_body;
use super::props::{MeshProps, PropsPlan};
use super::skin::{rest_transforms, skin_key, skin_vertices};
use super::vehicle_type::VehicleType;
use super::visuals::{is_shadow_mesh, onto_plane};
use super::{articulation, VehicleInstance};
use crate::anim::MeshAnimator;
use crate::particles::ParticleSet;
use glam::{DVec3, Mat4, Quat, Vec3};
use omsi_script::Program;
use omsi_vehicle::Vehicle;
use std::path::Path;
use std::sync::Arc;

impl VehicleInstance {
    /// Coupled vehicles follow and animate from this vehicle's variables.
    pub(super) fn update_trailers(&mut self, dt: f32) {
        let mut trailers = std::mem::take(&mut self.trailers);
        let mut lead: Option<(DVec3, Mat4, f64)> = None;
        for t in &mut trailers {
            t.update(self, dt, lead);
            lead = Some((t.position, t.body_rotation(), t.heading));
        }
        self.trailers = trailers;
    }

    /// Put the coupled parts on a track: `at_behind(d)` is the point of the track `d` metres
    /// behind the vehicle's origin (None: not known, the part follows as it does on a road).
    /// Each part's turning axle is placed there, and the part follows its coupling.
    pub fn retrail(&mut self, dt: f32, at_behind: &dyn Fn(f64) -> Option<DVec3>) {
        let mut trailers = std::mem::take(&mut self.trailers);
        let mut lead: Option<(DVec3, Mat4, f64)> = None;
        let (mut prev_c, mut b_prev) = (self.position, 0.0f64);
        for t in &mut trailers {
            let (lp, lr) = lead
                .map(|(p, r, _)| (p, r))
                .unwrap_or((self.position, self.body_rotation()));
            let c = t.coupling_point(lp, lr);
            let b_c = b_prev + (c - prev_c).truncate().length();
            if let Some(p) = at_behind(b_c + t.pivot_length() as f64) {
                t.place_on_track(p);
            }
            t.update(self, dt, lead);
            lead = Some((t.position, t.body_rotation(), t.heading));
            prev_c = c;
            b_prev = b_c;
        }
        self.trailers = trailers;
    }

    /// Couple a vehicle behind this one (`[couple_back]`).
    pub fn attach_trailer(&mut self, ty: Arc<VehicleType>) {
        self.attach_trailer_ex(ty, false);
    }

    /// Uncouple the last part of the train (it is gone from this vehicle).
    pub fn detach_last_trailer(&mut self) -> Option<TrailerPart> {
        self.trailers.pop()
    }

    /// Couple a vehicle behind the last part of the train; `reversed` cars run backwards
    /// (their `[coupling_back]` becomes the front coupling).
    pub fn attach_trailer_ex(&mut self, ty: Arc<VehicleType>, reversed: bool) {
        let first_axle =
            self.motion.wheels.len() + self.trailers.iter().map(|t| t.axle_count).sum::<usize>();
        let (lead_ty, lead_reversed) = match self.trailers.last() {
            Some(t) => (t.ty.clone(), t.reversed),
            None => (self.ty.clone(), false),
        };
        let t = TrailerPart::new_ex(
            ty,
            &lead_ty,
            lead_reversed,
            reversed,
            self.ty.program.as_ref(),
            first_axle,
            self.trailers.len(),
        );
        self.trailers.push(t);
    }
}

/// A vehicle coupled behind another (articulated bus rear section, trailer, train car).
/// It has no script state of its own: meshes animate from the leading vehicle's
/// variables, and it is towed at the coupling point with trailer kinematics.
/// Sign of `articulation_<n>_alpha` relative to (heading in front − own heading), clockwise
/// degrees: settled with the GN92's joint dummies (see the vehicle tests).
pub(super) const ARTICULATION_SIGN: f64 = 1.0;

pub struct TrailerPart {
    pub ty: Arc<VehicleType>,
    /// The part's own `[smoke]` particle systems (the pusher's rear engine).
    pub particles: ParticleSet,
    /// Its lights' brightness as they come on and go out (see `VehicleInstance::light_fade`).
    pub light_fade: Vec<f32>,
    pub(super) animators: Vec<MeshAnimator>,
    pub mesh_transforms: Vec<Mat4>,
    pub mesh_props: Vec<MeshProps>,
    pub position: DVec3,
    pub heading: f64,
    /// World position of the rear pivot (the trailer axle) from the last frame.
    pub(super) pivot: Option<DVec3>,
    /// Coupling → rear axle distance.
    pub(super) length: f32,
    /// How far ahead of the ball, on the part in front, the joint pitches (see
    /// [`TrailerPart::pitch_offset`]).
    pub(super) pitch_offset: f32,
    pub(super) coupling_front: Vec3,
    pub(super) coupling_back: Vec3,
    pub(super) axle_long: f32,
    pub(super) axle_count: usize,
    /// First axle index of this part in the train's `Wheel_*` variables.
    pub(super) first_axle: usize,
    pub(super) wheel_radius: f32,
    /// `articulation_<n>_alpha` (the angle about the vertical axis, which the jackknife
    /// protection watches and the joint's plates and bellows turn with) and
    /// `articulation_<n>_beta` (about the transverse axis) of this part's joint.
    pub(super) v_alpha: Option<omsi_script::VarId>,
    pub(super) v_beta: Option<omsi_script::VarId>,
    pub(super) odometer: f32,
    pub reversed: bool,
    /// Per axle of this part: tyre radius less the unloaded hub height, the static load per
    /// wheel and the spring rate - it sits down on its springs like the part it follows.
    pub(super) rest: Vec<(f32, f32, f32)>,
    /// `Axle_Brakeforce_<axle>_L/R` of this part's axles, in the leading vehicle's scripts.
    pub(super) v_brakes: Vec<Option<omsi_script::VarId>>,
    /// How far the part's origin stands above the ground under its axles (m).
    pub(super) ground_lift: f32,
    /// The part's pitch (degrees, nose up) and lean (degrees, as the vehicle's `bank`) as
    /// it travels (not turned round for a reversed part), and the eased height of its axle.
    pub(super) pitch: f32,
    pub(super) bank: f32,
    pub(super) axle_z: Option<f64>,
    /// How fast the axle's height moves (m/s), for the road part's springs (see `follow`).
    pub(super) axle_vz: f64,
    /// The track under its turning axle, when it runs on rails (`VehicleInstance::retrail`):
    /// it stands at the track's height, not on whatever the ground probe finds there.
    pub(super) track: Option<DVec3>,
    /// Mesh property sources, resolved against the leading vehicle's variables.
    pub(super) props_plan: PropsPlan,
    /// The part's own body behind the player's vehicle (see `articulation`).
    pub(super) body: Option<articulation::PartBody>,
    /// The meshes' transforms in the modelled pose, for `[smoothskin]` (made when needed).
    skin_rest: Vec<Mat4>,
    /// The part's own `[texttexture]`s, written from the leading vehicle's string
    /// variables (`[scriptshare]`: the rear section of an articulated bus shows the
    /// number and the destination the front's scripts set).
    pub text_textures: Vec<crate::texttex::TextTextureState>,
}

/// A negative `[boogies]` swaps the rail body's ends relative to its path frame. This is
/// independent of the consist's coupling flags.
/// Keep `heading` in the direction of travel and turn the whole body,
/// including its lights and sound sources, rather than just its meshes.
pub fn body_reversed(def: &Vehicle, reversed: bool) -> bool {
    reversed ^ def.boogies.is_some_and(|b| b < 0.0)
}

/// Model heading in the world, from the path heading and the absolute consist flag.
pub fn body_heading(def: &Vehicle, heading: f64, reversed: bool) -> f64 {
    if body_reversed(def, reversed) {
        heading + 180.0
    } else {
        heading
    }
}

pub(super) fn body_rotation(heading: f64, pitch: f32, bank: f32, reversed: bool) -> Mat4 {
    let (h, p, b) = if reversed {
        (heading + 180.0, -pitch, -bank)
    } else {
        (heading, pitch, bank)
    };
    Mat4::from_quat(
        Quat::from_rotation_z((-h).to_radians() as f32)
            * Quat::from_rotation_x(p.to_radians())
            * Quat::from_rotation_y(b.to_radians()),
    )
}

fn declared_coupling(def: &Vehicle, front: bool) -> Vec3 {
    let coupling = if front {
        &def.coupling_front
    } else {
        &def.coupling_back
    };
    // (none declared is the model origin, as OMSI keeps it; an AI copy's type is given the
    // coupling it has always had - see `VehicleType::load_ai`)
    let p = coupling
        .as_ref()
        .map(|c| Vec3::from(c.pos))
        .unwrap_or(Vec3::ZERO);
    // Rail couplings are declared in the path frame. Express them in the model frame
    // so body_rotation can turn a negative-bogie body without moving its joints.
    if body_reversed(def, false) {
        Vec3::new(-p.x, -p.y, p.z)
    } else {
        p
    }
}

/// Where the two parts meet, in their respective model frames. Bogie-defined trains
/// use declared couplings, as OMSI does, even when a mod's
/// joints do not match its bodies: the CR200J's inset rear coupling causes overlap
/// in vanilla too.
/// Other rail definitions retain body-end placement; road vehicles retain their
/// declared joint.
pub fn coupling_offsets(
    lead: &VehicleType,
    lead_reversed: bool,
    car: &VehicleType,
    car_reversed: bool,
) -> Option<(f32, f32)> {
    if !car.def.is_rail() {
        return None;
    }
    let lead_rear = if lead.def.boogies.is_some() {
        declared_coupling(&lead.def, lead_reversed).y
    } else {
        lead.model_box()
            .map(|(lo, hi)| if lead_reversed { hi.y } else { lo.y })?
    };
    let car_front = if car.def.boogies.is_some() {
        declared_coupling(&car.def, !car_reversed).y
    } else {
        car.model_box()
            .map(|(lo, hi)| if car_reversed { lo.y } else { hi.y })?
    };
    Some((lead_rear, car_front))
}

/// `(lead's rear, car's front)` in each body's model frame, including default joints.
pub fn coupling_points(
    lead: &VehicleType,
    lead_reversed: bool,
    car: &VehicleType,
    car_reversed: bool,
) -> (Vec3, Vec3) {
    let mut back = declared_coupling(&lead.def, lead_reversed);
    let mut front = declared_coupling(&car.def, !car_reversed);
    if let Some((b, f)) = coupling_offsets(lead, lead_reversed, car, car_reversed) {
        back.y = b;
        front.y = f;
    }
    // Between bogie-defined cars only distance along the track joins them;
    // lateral position and height come from the track, not a road joint.
    if lead.def.boogies.is_some() && car.def.boogies.is_some() {
        back = Vec3::new(0.0, back.y, 0.0);
        front = Vec3::new(0.0, front.y, 0.0);
    }
    (back, front)
}

/// Where a car of a consist stands, for a caller that lays the cars along one heading with
/// no turn of its own (`Traffic::blocked`): the world position and heading of the car whose
/// body front sits at the joint [`coupling_offsets`] named.
///
/// Both `lead_reversed` and `reversed` are absolute body orientations, including the
/// intrinsic turn of negative bogies, and `heading` is the consist's path heading.
/// A car's body sits along the consist's heading turned round when it is itself turned
/// round, so its heading is derived from `reversed` alone and never from the car in front
/// of it: two cars turned round in a row would otherwise come out turned twice and lie on
/// top of each other.
///
/// `back` is the leading car's body end in the leading car's frame and `front` this car's
/// body front in its own, so the joint lies `back` along the lead's heading (turned round
/// when the *lead* is) and the car's origin steps back from it by `front` along the car's
/// own heading. A caller that turns the part around itself (`TrailerPart::body_rotation`)
/// needs none of this.
pub fn coupling_placement(
    lead_origin: DVec3,
    heading: f64,
    lead_reversed: bool,
    back: f32,
    reversed: bool,
    front: f32,
) -> (DVec3, f64) {
    let turned = |base: f64, rev: bool| if rev { base + 180.0 } else { base };
    let lead_h = turned(heading, lead_reversed);
    let lh = lead_h.to_radians();
    let joint = lead_origin + DVec3::new(lh.sin(), lh.cos(), 0.0) * back as f64;
    let car_h = turned(heading, reversed);
    let ch = car_h.to_radians();
    let origin = joint - DVec3::new(ch.sin(), ch.cos(), 0.0) * front as f64;
    (origin, car_h)
}

impl TrailerPart {
    /// Pitch (degrees, nose up), eased axle height and the track point it stands on (for
    /// the `OMSI_DEBUG_TRAILERS` trace).
    pub fn debug_pose(&self) -> (f32, Option<f64>, Option<DVec3>) {
        (self.pitch, self.axle_z, self.track)
    }

    /// How far the part's origin stands above the plane its wheels touch (m): the road is
    /// at z = -this in its own frame (where its shadow blob lies, and the tyre spray starts).
    pub fn ground_lift(&self) -> f32 {
        self.ground_lift
    }

    pub fn new(
        ty: Arc<VehicleType>,
        main: &VehicleType,
        program: &Program,
        first_axle: usize,
    ) -> TrailerPart {
        Self::new_ex(ty, main, false, false, program, first_axle, 0)
    }

    /// `joint`: which joint of the train this part hangs on (0 behind the leading vehicle).
    pub fn new_ex(
        ty: Arc<VehicleType>,
        main: &VehicleType,
        main_reversed: bool,
        reversed: bool,
        program: &Program,
        first_axle: usize,
        joint: usize,
    ) -> TrailerPart {
        let mut animators: Vec<MeshAnimator> = ty
            .meshes
            .iter()
            .map(|m| MeshAnimator::new(&ty.model.meshes[m.def_index], m.pivot, |n| program.var(n)))
            .collect();
        crate::anim::link_parents(
            &mut animators,
            &ty.meshes
                .iter()
                .map(|m| &ty.model.meshes[m.def_index])
                .collect::<Vec<_>>(),
        );
        let (coupling_back, coupling_front) = coupling_points(main, main_reversed, &ty, reversed);
        // the line the part turns about: its own `[rot_pnt_long]` where a road part names
        // one (OMSI runs every section as a body of its own on the same wheel physics,
        // each axle steered towards the turning centre on that line), else the axle
        // farthest from the coupled end. A rear section whose axle steers (the Van Hool
        // AG300's, set ahead of its axle) followed it as if it were a fixed one (#322);
        // the stock GN92's line is its axle, a semitrailer's the middle of its axle group,
        // and rail cars name none.
        let turning_line =
            (ty.def.rot_pnt_long != 0.0 && !ty.def.axles.is_empty()).then_some(ty.def.rot_pnt_long);
        let axle_long = if let Some(r) = turning_line {
            r
        } else if body_reversed(&ty.def, reversed) {
            let a = ty.def.axles.iter().map(|a| a.long).fold(f32::MIN, f32::max);
            if a == f32::MIN {
                0.5
            } else {
                a
            }
        } else {
            let a = ty.def.axles.iter().map(|a| a.long).fold(f32::MAX, f32::min);
            if a == f32::MAX {
                -0.5
            } else {
                a
            }
        };
        let wheel_radius = ty
            .def
            .axles
            .first()
            .map(|a| crate::physics::AxleSpec::of(a).shown_radius)
            .unwrap_or(0.5);
        let n = ty.meshes.len();
        let loads = crate::physics::kinematic_wheel_rest_loads(&ty.def);
        let rest = ty
            .def
            .axles
            .iter()
            .zip(ty.hub_heights(first_axle))
            .enumerate()
            .map(|(a, (axle, z))| {
                let axle = crate::physics::AxleSpec::of(axle);
                let r = axle.kinematic_radius;
                (
                    r - z.unwrap_or(r),
                    loads.get(a).copied().unwrap_or(0.0),
                    axle.sag_spring,
                )
            })
            .collect();
        let v_brakes = (0..ty.def.axles.len())
            .flat_map(|a| {
                ["L", "R"]
                    .map(|side| program.var(&format!("Axle_Brakeforce_{}_{side}", first_axle + a)))
            })
            .collect();
        TrailerPart {
            particles: ParticleSet::new(ty.model.particle_systems(), first_axle as u64 * 7919 + 17),
            light_fade: Vec::new(),
            rest,
            v_brakes,
            ground_lift: 0.0,
            pitch: 0.0,
            bank: 0.0,
            axle_z: None,
            axle_vz: 0.0,
            track: None,
            v_alpha: program.var(&format!("articulation_{joint}_alpha")),
            v_beta: program.var(&format!("articulation_{joint}_beta")),
            animators,
            mesh_transforms: vec![Mat4::IDENTITY; n],
            mesh_props: vec![MeshProps::default(); n],
            props_plan: PropsPlan::default(),
            body: None,
            position: DVec3::ZERO,
            heading: 0.0,
            pivot: None,
            length: (coupling_front.y - axle_long).abs(),
            coupling_front,
            coupling_back,
            axle_long,
            axle_count: ty.def.axles.len().max(1),
            first_axle,
            wheel_radius,
            odometer: 0.0,
            reversed,
            text_textures: Vec::new(),
            skin_rest: Vec::new(),
            pitch_offset: ty.def.pitch_offset(),
            ty,
        }
    }

    /// The `[smoothskin]` meshes of this part.
    pub fn skinned_meshes(&self) -> Vec<usize> {
        (0..self.ty.meshes.len())
            .filter(|&i| !self.ty.meshes[i].skin.is_empty())
            .collect()
    }

    /// See [`VehicleInstance::skin_key`].
    pub fn skin_key(&self, i: usize) -> Vec<Mat4> {
        skin_key(&self.ty, i, &self.mesh_transforms)
    }

    /// Skinned mesh `i` as its bones hold it now; `n_vars`: the leading vehicle's variable
    /// count (this part animates from its variables).
    pub fn skinned(&mut self, i: usize, n_vars: usize) -> Option<(Vec<Vec3>, Vec<Vec3>)> {
        if self.skin_rest.len() != self.animators.len() {
            self.skin_rest = rest_transforms(&self.animators, n_vars);
        }
        skin_vertices(&self.ty, i, &self.mesh_transforms, &self.skin_rest)
    }

    /// Prepare the part's text textures with fonts from `lib`.
    pub fn init_text_textures(
        &mut self,
        lib: &mut crate::texttex::FontLibrary,
        decode: &dyn Fn(&Path) -> Option<(u32, u32, Vec<u8>)>,
    ) {
        self.text_textures = self
            .ty
            .model
            .text_textures
            .iter()
            .map(|t| crate::texttex::TextTextureState::new(t.clone(), lib.get(&t.font, decode)))
            .collect();
    }

    /// Re-render the text textures whose variable (of the leading vehicle) changed;
    /// returns the indices with a pending image.
    pub fn update_text_textures(&mut self, main: &VehicleInstance) -> Vec<usize> {
        let mut changed = Vec::new();
        for (i, t) in self.text_textures.iter_mut().enumerate() {
            let text = main.text_texture_string(&t.def.variable);
            if t.update(&text) {
                changed.push(i);
            }
        }
        changed
    }

    /// A trailer follows its tractor's heading; it pitches between the coupling and the
    /// ground under its axle, and leans as the vehicle does.
    pub fn body_rotation(&self) -> Mat4 {
        body_rotation(
            self.heading,
            self.pitch,
            self.bank,
            body_reversed(&self.ty.def, self.reversed),
        )
    }

    pub fn body_heading(&self) -> f64 {
        body_heading(&self.ty.def, self.heading, self.reversed)
    }

    /// World transform of the part's body (f32, for sound positions).
    pub fn world_transform(&self) -> Mat4 {
        Mat4::from_translation(self.position.as_vec3()) * self.body_rotation()
    }

    /// One of the part's own `.bus` cameras fixed to its body: (eye, yaw, pitch, roll), as
    /// `VehicleInstance::camera_world_full` for the front part (`dist` and the body's pitch
    /// and bank included).
    pub fn camera_world_full(&self, cam: &omsi_vehicle::Camera) -> (DVec3, f32, f32, f32) {
        camera_in_body(self.position, self.body_rotation(), cam)
    }

    /// Transform for mesh `i` relative to the part's position; a shadow blob lies on the
    /// ground under its axles (see `VehicleInstance::mesh_local_transform`).
    pub fn mesh_local_transform(&self, i: usize) -> Mat4 {
        if is_shadow_mesh(&self.ty, i) {
            return self.body_rotation()
                * onto_plane([-self.ground_lift, 0.0, 0.0])
                * self.mesh_transforms[i];
        }
        self.body_rotation() * self.mesh_transforms[i]
    }

    /// Where this part hangs on the one in front: the coupling point in the frame of the
    /// part in front, and its own coupling point in its own frame (the two meet in the
    /// world). With the parts straight behind each other, a point `p` of this part lies at
    /// `p + back - front` in the frame of the part in front.
    pub fn couplings(&self) -> (Vec3, Vec3) {
        (self.coupling_back, self.coupling_front)
    }

    /// Put this part where another source says it is (a LAN player's rear section): the
    /// pivot is set so that the next follow step keeps this heading.
    pub fn set_pose(&mut self, position: DVec3, heading: f64) {
        self.heading = heading;
        let rot = self.body_rotation();
        let c = position + rot.transform_point3(self.coupling_front).as_dvec3();
        let h = heading.to_radians();
        self.pivot = Some(c - DVec3::new(h.sin(), h.cos(), 0.0) * self.length as f64);
        self.position = position;
    }

    /// `set_pose` for a part another game drives (LAN), which sends where the part is and
    /// its heading but not how it leans: tilted so that its front coupling meets the leading
    /// part's at `coupling` (`coupling_point`). Drawn level, an articulated bus's rear
    /// section climbing a slope sank into the road in the other players' games (#1702).
    pub fn set_remote_pose(&mut self, position: DVec3, heading: f64, coupling: DVec3) {
        let ahead = self.coupling_front.y.abs().max(0.5) as f64;
        let rise = coupling.z - (position.z + self.coupling_front.z as f64);
        self.pitch = (rise.atan2(ahead).to_degrees() as f32).clamp(-20.0, 20.0);
        self.set_pose(position, heading);
    }

    /// Forget where this part was: the next step puts it straight behind the leading part
    /// (after the vehicle was moved somewhere else).
    pub fn realign(&mut self) {
        self.unplace();
        self.pivot = None;
        self.axle_z = None;
        self.track = None;
    }

    /// Where this part couples to the part in front of it, in the world, with the leading
    /// part as it stands now (`lead`: position, rotation of the part in front; the vehicle
    /// itself for the first trailer).
    pub fn coupling_point(&self, lead_pos: DVec3, lead_rot: Mat4) -> DVec3 {
        lead_pos + lead_rot.transform_point3(self.coupling_back).as_dvec3()
    }

    /// The distance (m) from the ball to the axis the joint pitches about, ahead on the part
    /// in front (`[coupling_pitch_offset]` of this part; 0: a puller's joint, pitching at
    /// the ball).
    pub fn pitch_offset(&self) -> f32 {
        self.pitch_offset
    }

    /// Where the joint pitches, in the world (`lead` as for [`Self::coupling_point`]): on the
    /// part in front, `pitch_offset` ahead of the coupling towards its body; the coupling
    /// point itself for a puller's joint.
    pub fn pitch_hinge(&self, lead_pos: DVec3, lead_rot: Mat4) -> DVec3 {
        let ahead = if self.coupling_back.y < 0.0 {
            1.0
        } else {
            -1.0
        };
        let p = self.coupling_back + Vec3::new(0.0, ahead * self.pitch_offset, 0.0);
        lead_pos + lead_rot.transform_point3(p).as_dvec3()
    }

    /// The distance from the coupling to the axle the part turns about.
    pub fn pivot_length(&self) -> f32 {
        self.length
    }

    /// Put the part's turning axle at `pivot` (a point on the road behind the coupling):
    /// a bus placed on a bend stands with its rear section following the bend rather than
    /// straight behind it.
    pub fn place_pivot(&mut self, pivot: DVec3) {
        self.unplace();
        self.pivot = Some(pivot);
    }

    /// Put the part's turning axle on a track at `p` (a rail vehicle's coupled car or
    /// section): it also takes the track's height there.
    pub fn place_on_track(&mut self, p: DVec3) {
        self.pivot = Some(p);
        self.track = Some(p);
    }
}
