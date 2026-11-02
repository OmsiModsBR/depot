//! The bus the Drive page shows behind its panels: the game draws it (a picture kept from an
//! earlier time, or a helper's live one), so the page asks it through `Showroom`, which the game
//! implements.

use super::super::interface::output::Ui;
use omsi_ui::Rect;

/// The game's bus preview, as a page uses it.
pub trait Showroom {
    /// The bus across `r`, the whole page behind the panels. `focus` is where the panels end,
    /// as a share of the window: the bus is framed in what is left of it.
    fn full(&mut self, ui: &mut Ui, r: Rect, focus: f32);
    /// The wheel and the cursor over the bus, once the panels have had the mouse.
    fn pointer(&mut self, ui: &mut Ui, r: Rect);
}
