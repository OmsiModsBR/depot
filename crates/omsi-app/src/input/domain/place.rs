//! Where a vehicle chosen to be placed is put down.

/// Where a vehicle chosen to be placed is put down.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PlaceAt {
    /// It follows the mouse until a click puts it down.
    Mouse,
    /// At the map's entry point at this place of `global.cfg`'s list.
    EntryPoint(usize),
}
