//! The types of the frame: the window's events and what the frame keeps (the info page's
//! mode and data, the frame rate's "1 %" low). Their procedures are in `implementation/`.

use std::collections::VecDeque;

#[derive(Default)]
pub struct FrameStats {
    pub(crate) times: VecDeque<f32>,
    pub(crate) total: f32,
}
