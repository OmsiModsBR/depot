//! How the HUD is drawn: the old interface (`legacy`, on its way to `omsi-ui`), and the
//! procedures on its objects.

pub(crate) mod legacy;
mod notice;

pub use notice::push_notice;
