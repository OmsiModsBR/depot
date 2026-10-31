//! The shape a preview is drawn for.

/// The shape a preview is drawn for: its width over its height and where the bus stands across
/// it, both in steps (tenths, twentieths) so that a window a few pixels bigger keeps them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Shape(pub(crate) u32, pub(crate) u32);

impl Shape {
    pub fn of(w: u32, h: u32, focus_x: f32) -> Shape {
        Shape(
            ((w as f32 / h.max(1) as f32) * 10.0).round() as u32,
            (focus_x * 20.0).round() as u32,
        )
    }
}
