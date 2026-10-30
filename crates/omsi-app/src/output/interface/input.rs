//! What the game calls on the graphics output and reads from it: the backend the window draws
//! with and the window's output, the VR session (Windows only), the cameras and their orbit
//! limits, the mirrors drawn for the bus and laid over the picture, the driver's idle head and
//! the navigator's placements in VR.

pub(crate) use super::super::domain::backend::Backend;
pub(crate) use super::super::domain::head_idle::{standstill, HeadIdle, Hold};
pub(crate) use super::super::domain::mirror_hud::MirrorHud;
pub(crate) use super::super::domain::vr_navigator::{Editing, Profiles};
pub(crate) use super::super::implementation::camera::arm as camera_arm;
pub(crate) use super::super::implementation::camera::arm::{classify, Blocker, BlockerShape};
pub(crate) use super::super::implementation::camera::util::{
    adjusted, auto_outside_fov, cursor_ray, default_camera, driver_eye, follow_camera, follow_id,
    look_of, mirror_view, offscreen_orbit, pixel_angle, ray_may_hit, render_mirrors, ORBIT_DEFAULT,
    ORBIT_MAX, ORBIT_MIN,
};
pub(crate) use super::super::implementation::window::Output;
#[cfg(windows)]
pub(crate) use super::super::implementation::xr::session::Vr;
