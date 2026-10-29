//! The copies of the bus's mirrors laid over the picture: the panels, how each lays its
//! mirror's picture out, and the editor's state.

use crate::scene::MirrorGlass;
use glam::Vec3;
use omsi_render::TextureId;

/// `mirror_hud` setting: the panels a bus without a saved layout starts with.
pub(crate) const RIGHT: u8 = 1;
pub(crate) const LEFT: u8 = 2;

pub(crate) const HINT: &str = "Mirror editor on: its keys are listed top left";

pub(in crate::output) const MIN_H: f32 = 0.08;
pub(in crate::output) const MAX_H: f32 = 0.70;
pub(in crate::output) const MIN_ASPECT: f32 = 0.25;
pub(in crate::output) const MAX_ASPECT: f32 = 4.0;
pub(in crate::output) const MARGIN: f32 = 0.012;

/// How a panel lays its picture out: the render texture is drawn for the mirror's glass, and
/// the glass's mesh puts it on with its own turn (the usual side mirror shows it left to right
/// over, one in a roof housing may have it on its side).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Turn {
    /// The picture's u runs up or down the panel (and its v across).
    pub transposed: bool,
    /// The rectangle is drawn from its right edge / its bottom edge: what runs across it
    /// runs to the left, what runs down it runs up.
    pub x_reversed: bool,
    pub y_reversed: bool,
}

impl Default for Turn {
    /// The usual side mirror: u runs to the left, v down.
    fn default() -> Self {
        Turn {
            transposed: false,
            x_reversed: true,
            y_reversed: false,
        }
    }
}

impl Turn {
    /// How the picture lies on a glass as seen from `eye`: the glass's `centre` and the way its
    /// position moves with u (`du`) and v (`dv`), all in the bus's frame (x right, y forward,
    /// z up). The viewer's up is the bus's up across the line of sight.
    pub fn of(eye: Vec3, centre: Vec3, du: Vec3, dv: Vec3) -> Turn {
        let Some(sight) = (centre - eye).try_normalize() else {
            return Turn::default();
        };
        let Some(up) = (Vec3::Z - sight * sight.z).try_normalize() else {
            return Turn::default();
        };
        let right = sight.cross(up);
        let (du, dv) = (
            du.try_normalize().unwrap_or(Vec3::ZERO),
            dv.try_normalize().unwrap_or(Vec3::ZERO),
        );
        let (ur, uu, vr, vu) = (du.dot(right), du.dot(up), dv.dot(right), dv.dot(up));
        if ur.abs() >= uu.abs() {
            // u across, v down the picture (the top row is v 0)
            Turn {
                transposed: false,
                x_reversed: ur < 0.0,
                y_reversed: vu > 0.0,
            }
        } else {
            // u up or down, v across
            Turn {
                transposed: true,
                x_reversed: vr < 0.0,
                y_reversed: uu > 0.0,
            }
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    /// Reflection camera (the bus's mirror) this panel shows.
    pub cam: usize,
    /// Top left corner, as fractions of the window's width and height.
    pub x: f32,
    pub y: f32,
    /// Height as a fraction of the window's height, and width / height.
    pub h: f32,
    pub aspect: f32,
}

#[derive(Default)]
pub struct MirrorHud {
    pub enabled: bool,
    pub panels: Vec<Panel>,
    /// The editor is on: the mouse and a few keys work on the panels.
    pub edit: bool,
    /// The bus the layout belongs to (its `.bus` path, lower case); empty before one is known.
    pub key: String,
    /// The panel being dragged and where it was grabbed (pixels from its corner).
    pub drag: Option<(usize, f32, f32)>,
    /// A plain yellow pixel for the editor's frames.
    pub frame: Option<TextureId>,
    /// Width / height of each mirror's glass (0: unknown), from the model.
    pub aspects: Vec<f32>,
    /// Where each mirror's glass is and how the picture lies on it, see `World::mirror_glass`.
    pub glass: Vec<Option<MirrorGlass>>,
    /// How each panel lays its mirror's picture out to match its glass (see `Turn`).
    pub turns: Vec<Turn>,
    /// The keys held in the editor that aim and shift the mirror: left, right, up, down,
    /// Page Up (the mirror forward), Page Down (back), minus (a narrower field of view), plus
    /// (a wider one).
    pub arrows: [bool; 8],
}
