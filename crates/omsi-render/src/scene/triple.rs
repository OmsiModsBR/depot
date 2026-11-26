//! Physical, monoscopic three-screen rig. Dimensions are millimetres; angles are
//! measured inward from a flat row of monitors. All views share the driver's eye.

use super::camera::Camera;
use glam::{DVec3, Mat4, Vec3, Vec4};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TripleScreen {
    pub enabled: bool,
    pub width_mm: f32,
    pub distance_mm: f32,
    /// Combined hidden width at each join (both adjacent bezels).
    pub bezel_mm: f32,
    pub left_angle_deg: f32,
    pub right_angle_deg: f32,
    /// Eye above the centre of the panels.
    pub eye_height_mm: f32,
    /// Optional centre panel vertical FOV. Zero uses measured eye distance.
    pub fov_deg: f32,
}

impl Default for TripleScreen {
    fn default() -> Self {
        Self {
            enabled: false,
            width_mm: 600.0,
            distance_mm: 650.0,
            bezel_mm: 0.0,
            left_angle_deg: 45.0,
            right_angle_deg: 45.0,
            eye_height_mm: 0.0,
            fov_deg: 0.0,
        }
    }
}

/// One panel's width in pixels. All three are drawn at this same size, so they share the
/// renderer's size-keyed targets (depth, AO, rain glass) instead of rebuilding them per panel.
pub fn panel_width(width: u32) -> u32 {
    width.div_ceil(3)
}

pub struct ScreenView {
    pub camera: Camera,
    pub projection: Mat4,
    /// x, y, width, height in physical window pixels.
    pub viewport: [u32; 4],
}

impl TripleScreen {
    /// Zoom moves the common eye instead of changing one panel's projection.
    pub fn zoomed(&self, width: u32, height: u32, zoom: f32) -> Self {
        let mut rig = *self;
        if zoom.is_finite() && (zoom - 1.0).abs() > 1e-6 {
            let panel_height = self.width_mm * height as f32 / panel_width(width).max(1) as f32;
            let base = if self.fov_deg > 0.0 {
                self.fov_deg
            } else {
                (2.0 * (panel_height * 0.5 / self.distance_mm).atan()).to_degrees()
            };
            rig.fov_deg = (base * zoom).clamp(8.0, 120.0);
        }
        rig
    }

    /// The three panels, all of the same pixel size (`panel_width`): the right one is
    /// cropped at the window's edge when the width is not divisible by three.
    pub fn views(&self, camera: &Camera, width: u32, height: u32) -> [ScreenView; 3] {
        let pw = panel_width(width);
        // One common physical height: rounding a viewport by a pixel must not
        // move the horizon at the joins.
        let panel_height = self.width_mm * height as f32 / pw.max(1) as f32;
        // FOV zoom moves the common virtual eye, keeping all panel joins aligned.
        let eye_distance = if self.fov_deg > 0.0 {
            panel_height * 0.5 / (self.fov_deg.to_radians() * 0.5).tan()
        } else {
            self.distance_mm
        };
        std::array::from_fn(|i| {
            let angle = match i {
                0 => -self.left_angle_deg,
                2 => self.right_angle_deg,
                _ => 0.0,
            }
            .to_radians();
            let (s, c) = angle.sin_cos();
            let hinge = if i == 1 {
                0.0
            } else {
                self.width_mm * 0.5 + self.bezel_mm
            };
            let distance = eye_distance * c + hinge * s.abs();
            let offset = if i == 1 {
                0.0
            } else {
                angle.signum() * (hinge * c - eye_distance * s.abs() + self.width_mm * 0.5)
            };
            let half = self.width_mm * 0.5;
            let l = (offset - half) / distance;
            let r = (offset + half) / distance;
            let b = (-panel_height * 0.5 - self.eye_height_mm) / distance;
            let t = (panel_height * 0.5 - self.eye_height_mm) / distance;
            // Right-handed, 0..1 reversed depth. Unlike a symmetric FOV, the
            // shift accounts for the eye not being on the side panel's normal.
            let n = camera.near;
            let f = camera.far;
            let projection = Mat4::from_cols(
                Vec4::new(2.0 / (r - l), 0.0, 0.0, 0.0),
                Vec4::new(0.0, 2.0 / (t - b), 0.0, 0.0),
                Vec4::new((r + l) / (r - l), (t + b) / (t - b), n / (f - n), -1.0),
                Vec4::new(0.0, 0.0, f * n / (f - n), 0.0),
            );
            let mut panel = *camera;
            let forward = camera.forward() * c + camera.right() * s;
            let up = camera.right().cross(camera.forward()).normalize();
            panel.yaw = forward.x.atan2(forward.y).to_degrees();
            panel.pitch = forward.z.clamp(-1.0, 1.0).asin().to_degrees();
            let right = Vec3::new(forward.y, -forward.x, 0.0).normalize_or(Vec3::X);
            panel.roll = up
                .dot(right)
                .atan2(up.dot(right.cross(forward)))
                .to_degrees();
            panel.fov_deg = (2.0 * ((t - b) * 0.5).atan()).to_degrees();
            ScreenView {
                camera: panel,
                projection,
                viewport: [i as u32 * pw, 0, pw, height],
            }
        })
    }

    /// Tangents of the widest horizontal and vertical half-angles the three panels show,
    /// measured from the centre camera's axis (capped at 85°): the frustum the whole rig
    /// covers, for "nothing appears or vanishes in sight".
    pub fn view_extent(&self, camera: &Camera, width: u32, height: u32) -> (f32, f32) {
        let cap = 85f32.to_radians().tan();
        let (f, r, u) = (camera.forward(), camera.right(), camera.up());
        let mut extent = (0.0f32, 0.0f32);
        for v in self.views(camera, width, height) {
            let inv = (v.projection
                * Mat4::look_to_rh(Vec3::ZERO, v.camera.forward(), v.camera.up()))
            .inverse();
            for (x, y) in [(-1.0, -1.0), (1.0, -1.0), (-1.0, 1.0), (1.0, 1.0)] {
                let d = inv.project_point3(Vec3::new(x, y, 0.0)).normalize_or_zero();
                let z = d.dot(f);
                let (tx, ty) = if z > 1e-3 {
                    ((d.dot(r) / z).abs().min(cap), (d.dot(u) / z).abs().min(cap))
                } else {
                    (cap, cap)
                };
                extent = (extent.0.max(tx), extent.1.max(ty));
            }
        }
        extent
    }

    pub fn cursor_ray(
        &self,
        camera: &Camera,
        cursor: (f32, f32),
        size: (u32, u32),
    ) -> (DVec3, Vec3, f32) {
        let views = self.views(camera, size.0, size.1);
        let view = views
            .iter()
            .find(|v| cursor.0 < (v.viewport[0] + v.viewport[2]) as f32)
            .unwrap_or(&views[2]);
        let [x, _, w, h] = view.viewport;
        let ndc = Vec3::new(
            (cursor.0 - x as f32) / w.max(1) as f32 * 2.0 - 1.0,
            1.0 - cursor.1 / h.max(1) as f32 * 2.0,
            0.0,
        );
        let vp =
            view.projection * Mat4::look_to_rh(Vec3::ZERO, view.camera.forward(), view.camera.up());
        let direction = vp.inverse().project_point3(ndc).normalize();
        let spread = 2.0 / view.projection.y_axis.y / h.max(1) as f32;
        (camera.position, direction, spread)
    }
}
