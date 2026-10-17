//! Where a vehicle's body is in the world, and its cameras.

use glam::{DVec3, Mat4, Vec3};
use super::trailer::{body_heading, body_reversed, body_rotation};
use super::visuals::{fit_plane, is_shadow_mesh, onto_plane, wheel_ground};
use super::VehicleInstance;

impl VehicleInstance {
    /// Rotation of the vehicle body (model frame → world, without translation).
    pub fn body_rotation(&self) -> Mat4 {
        body_rotation(self.heading, self.pitch, self.bank, body_reversed(&self.ty.def, false))
    }

    pub fn body_heading(&self) -> f64 {
        body_heading(&self.ty.def, self.heading, false)
    }

    /// World transform of the vehicle body (f32; for local computations such as sound
    /// positions - use `body_rotation` + `position` for rendering).
    pub fn world_transform(&self) -> Mat4 {
        Mat4::from_translation(self.position.as_vec3()) * self.body_rotation()
    }

    /// Transform for mesh `i` relative to the vehicle position (rotation + animation). A
    /// flat shadow blob (`[isshadow]`, drawn `[matl_noZcheck]` in the original, i.e. over the
    /// road whatever the depth) is put onto the plane the wheels stand on: in the model it
    /// lies at z = 0, which the springs' sag takes 10-16 cm under the road, where no depth
    /// bias brings it through.
    pub fn mesh_local_transform(&self, i: usize) -> Mat4 {
        if is_shadow_mesh(&self.ty, i) {
            return self.body_rotation()
                * onto_plane(self.contact_plane())
                * self.mesh_transforms[i];
        }
        self.body_rotation() * self.mesh_transforms[i]
    }

    /// The plane the wheels stand on, in the body frame: z = p[0] + p[1]·x + p[2]·y (m).
    /// The rigid body's tyres touch it a radius under their hubs (which the springs have
    /// pushed up into the body), the simple physics asks the ground under each wheel; an
    /// AI copy stands `ai_rest_offset` above its plane.
    pub fn contact_plane(&self) -> [f32; 3] {
        let mut points: Vec<Vec3> = Vec::new();
        if let Some(rb) = self.rigid() {
            let standing = rb.wheels.iter().any(|w| w.on_ground);
            for w in rb.wheels.iter().filter(|w| w.on_ground || !standing) {
                points.push(
                    w.attach + Vec3::Z * (w.compression.max(-crate::physics::DROOP) - w.radius),
                );
            }
        } else {
            let rot = self.body_rotation();
            let inv = rot.inverse();
            for w in self.motion.wheels.iter().flatten() {
                let p = self.position
                    + rot
                        .transform_vector3(Vec3::new(w.lat, w.long, 0.0))
                        .as_dvec3();
                if let Some(z) = wheel_ground(self.contact.as_deref(), self.ground.as_deref(), p) {
                    points.push(
                        inv.transform_vector3((DVec3::new(p.x, p.y, z) - self.position).as_vec3()),
                    );
                }
            }
        }
        if points.is_empty() {
            return [-self.ai_rest_offset().0, 0.0, 0.0];
        }
        fit_plane(&points)
    }

    /// The ground its `[smoke]` puffs are set off over, as a plane of the body frame (see
    /// `contact_plane`): the driven one's where its tyres touch the road; an AI copy's the
    /// plane it is placed `ai_rest_offset` over (the road its axles were set on), without
    /// asking the ground under every wheel again for the exhaust of every car on the map.
    pub(super) fn particle_ground(&self) -> [f32; 3] {
        if self.rigid().is_some() {
            self.contact_plane()
        } else {
            [-self.ai_rest_offset().0, 0.0, 0.0]
        }
    }

    /// Position/direction of a `.bus` camera in world space: (eye, yaw, pitch).
    pub fn camera_world(&self, cam: &omsi_vehicle::Camera) -> (DVec3, f32, f32) {
        let local = Vec3::new(cam.pos[0], cam.pos[1], cam.pos[2]);
        let eye = self.position + self.body_rotation().transform_point3(local).as_dvec3();
        (eye, self.body_heading() as f32 + cam.yaw, cam.pitch)
    }

    /// A camera fixed to the body as OMSI keeps one (`[add_camera_reflexion]`: OMSI
    /// puts it in the vehicle's own matrix): its eye, and yaw, pitch and roll (deg)
    /// of its view with the body's pitch and bank in them - a mirror leans with the bus. A
    /// `dist` above zero puts the eye that far behind the point along the view.
    pub fn camera_world_full(&self, cam: &omsi_vehicle::Camera) -> (DVec3, f32, f32, f32) {
        camera_in_body(self.position, self.body_rotation(), cam)
    }
}

/// A camera in a body's own matrix (`rot`, at `position`): see
/// `VehicleInstance::camera_world_full`.
pub(super) fn camera_in_body(position: DVec3, rot: Mat4, cam: &omsi_vehicle::Camera) -> (DVec3, f32, f32, f32) {
    let (sy, cy) = cam.yaw.to_radians().sin_cos();
    let (sp, cp) = cam.pitch.to_radians().sin_cos();
    let f_local = Vec3::new(sy * cp, cy * cp, sp);
    let r_local = Vec3::new(cy, -sy, 0.0);
    let f = rot.transform_vector3(f_local).normalize_or(Vec3::Y);
    let up = rot.transform_vector3(r_local.cross(f_local)).normalize_or(Vec3::Z);
    let local = Vec3::new(cam.pos[0], cam.pos[1], cam.pos[2]);
    let eye = position + rot.transform_point3(local).as_dvec3() - (f * cam.dist.max(0.0)).as_dvec3();
    let yaw = f.x.atan2(f.y).to_degrees();
    let pitch = f.z.clamp(-1.0, 1.0).asin().to_degrees();
    // (the roll the renderer's `Camera::up` turns back into this up)
    let r0 = Vec3::new(f.y, -f.x, 0.0).normalize_or(Vec3::X);
    let u0 = r0.cross(f);
    let roll = up.dot(r0).atan2(up.dot(u0)).to_degrees();
    (eye, yaw, pitch, roll)
}
