//! How the input is read and what it does: the keyboard, the mouse, the menus, the editor's
//! keys, the game controllers and their devices, the touch controls, head tracking, the input
//! script and the game actions.

pub(crate) mod actions;
pub(crate) mod controllers;
#[cfg(windows)]
pub(crate) mod dinput;
mod editor_input;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(crate) mod evdev_buttons;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(crate) mod evdev_ff;
pub(crate) mod game_controller_menu;
pub(crate) mod headtrack;
pub(crate) mod input_script;
pub(crate) mod keyboard;
#[cfg(target_os = "macos")]
pub(crate) mod mac_game_controller;
#[cfg(target_os = "macos")]
pub(crate) mod mac_hid;
pub(crate) mod menus;
pub(crate) mod mouse;
pub(crate) mod touch;
