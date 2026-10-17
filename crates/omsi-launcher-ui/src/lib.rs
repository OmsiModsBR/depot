//! The new launcher: a separate program, drawn with `omsi-ui`, with no 3D world in it.
//!
//! `Shell` is everything that is not a window: the sidebar, the page chosen and the page's
//! body. The window (`window`) and the picture written to a file (`offscreen`) both draw it,
//! so what a test or a screenshot sees is what the window shows.

pub mod look;
pub mod offscreen;
pub mod pages;
pub mod shell;
pub mod window;

pub use pages::Page;
pub use shell::Shell;
