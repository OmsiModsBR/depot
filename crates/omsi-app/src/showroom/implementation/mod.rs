//! How the preview is drawn and carried: the showroom's scene, the helper process and the
//! launcher's link to it, the messages between them, the pictures kept on disk and the ones
//! drawn ahead of time (`prerender`).

pub(crate) mod helper;
pub(crate) mod kept;
pub(crate) mod link;
pub(crate) mod prerender;
pub(crate) mod preview;
pub(crate) mod scene;
pub(crate) mod wire;
