//! The cockpit navigator in VR: where it sits in each vehicle, the key a vehicle's placement is
//! kept under, and the display it makes in the cab.

use glam::{DVec3, Mat4, Quat, Vec3, Vec4};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

#[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct Placement {
    pub enabled: bool,
    /// Metres relative to the bus's standard driver camera, in vehicle axes.
    pub offset: [f32; 3],
    pub width: f32,
    pub yaw: f32,
    pub tilt: f32,
    pub roll: f32,
    pub opacity: f32,
}

impl Default for Placement {
    fn default() -> Self {
        Self {
            enabled: false,
            offset: [0.28, 0.65, -0.35],
            width: 0.28,
            yaw: -23.0,
            tilt: 26.0,
            roll: 0.0,
            opacity: 0.95,
        }
    }
}

impl Placement {
    pub fn value(&self, field: &str) -> Option<f32> {
        Some(match field {
            "x" => self.offset[0],
            "y" => self.offset[1],
            "z" => self.offset[2],
            "width" => self.width,
            "yaw" => self.yaw,
            "tilt" => self.tilt,
            "roll" => self.roll,
            "opacity" => self.opacity,
            _ => return None,
        })
    }

    pub fn set_value(&mut self, field: &str, value: f32) {
        match field {
            "x" => self.offset[0] = value,
            "y" => self.offset[1] = value,
            "z" => self.offset[2] = value,
            "width" => self.width = value,
            "yaw" => self.yaw = value,
            "tilt" => self.tilt = value,
            "roll" => self.roll = value,
            "opacity" => self.opacity = value,
            _ => return,
        }
        *self = self.sanitize();
    }

    pub fn scroll(&mut self, amount: f32, resize: bool, eye: Vec3, driver: Vec3) {
        if !amount.is_finite() {
            return;
        }
        if resize {
            self.width *= 1.08_f32.powf(amount.clamp(-20.0, 20.0));
        } else {
            let ray = driver + Vec3::from(self.offset) - eye;
            let distance = ray.length();
            if distance > 0.001 {
                let new_distance = (distance - amount * 0.04).clamp(0.25, 2.5);
                self.offset = (eye + ray / distance * new_distance - driver).to_array();
            }
        }
        *self = self.sanitize();
    }

    pub(in crate::output) fn sanitize(mut self) -> Self {
        let default = Self::default();
        let clamp = |v: f32, fallback: f32, lo: f32, hi: f32| {
            if v.is_finite() {
                v.clamp(lo, hi)
            } else {
                fallback
            }
        };
        for i in 0..3 {
            self.offset[i] = clamp(self.offset[i], default.offset[i], -2.0, 2.0);
        }
        self.width = clamp(self.width, default.width, 0.12, 0.65);
        self.yaw = clamp(self.yaw, default.yaw, -180.0, 180.0);
        self.tilt = clamp(self.tilt, default.tilt, -80.0, 80.0);
        self.roll = clamp(self.roll, default.roll, -180.0, 180.0);
        self.opacity = clamp(self.opacity, default.opacity, 0.3, 1.0);
        self
    }

    pub fn adjust(&mut self, field: &str, direction: f32) {
        match field {
            "x" => self.offset[0] += direction * 0.02,
            "y" => self.offset[1] += direction * 0.02,
            "z" => self.offset[2] += direction * 0.02,
            "width" => self.width += direction * 0.02,
            "yaw" => self.yaw += direction * 2.0,
            "tilt" => self.tilt += direction * 2.0,
            "roll" => self.roll += direction * 2.0,
            "opacity" => self.opacity += direction * 0.05,
            "enabled" => self.enabled = !self.enabled,
            "reset" => {
                *self = Self {
                    enabled: self.enabled,
                    ..Self::default()
                }
            }
            _ => return,
        }
        *self = self.sanitize();
    }
}

#[derive(Default, Serialize, Deserialize)]
#[serde(default)]
pub struct Profiles {
    pub buses: BTreeMap<String, Placement>,
}

pub fn bus_key(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
        .to_lowercase()
}

pub fn driver_origin(camera: &omsi_vehicle::Camera) -> Vec3 {
    let (sy, cy) = camera.yaw.to_radians().sin_cos();
    let (sp, cp) = camera.pitch.to_radians().sin_cos();
    Vec3::from(camera.pos) - Vec3::new(sy * cp, cy * cp, sp) * camera.dist.max(0.0)
}

/// A display is anchored to the vehicle, independently of view, seat and HMD recentering.
#[derive(Clone, Copy)]
pub struct Display {
    pub placement: Placement,
    pub local_center: Vec3,
}

impl Display {
    pub fn transform(
        &self,
        bus: DVec3,
        body: Mat4,
        eye: &omsi_render::Camera,
        projection: Mat4,
        aspect: f32,
    ) -> Mat4 {
        let rotation = Quat::from_rotation_z(self.placement.yaw.to_radians())
            * Quat::from_rotation_x(-self.placement.tilt.to_radians())
            * Quat::from_rotation_y(self.placement.roll.to_radians());
        let right = body.transform_vector3(rotation * Vec3::X) * self.placement.width * 0.5;
        let up = body.transform_vector3(rotation * Vec3::Z) * self.placement.width / aspect * 0.5;
        let origin = bus + body.transform_point3(self.local_center).as_dvec3();
        let plane = Mat4::from_cols(
            right.extend(0.0),
            up.extend(0.0),
            Vec4::Z,
            (origin - eye.position).as_vec3().extend(1.0),
        );
        projection * Mat4::look_to_rh(Vec3::ZERO, eye.forward(), eye.up()) * plane
    }
}

pub(crate) struct Editing {
    pub moving: bool,
    pub rotating: bool,
    pub(in crate::output) paused_before: bool,
    pub(in crate::output) mouse_drive_before: bool,
}
