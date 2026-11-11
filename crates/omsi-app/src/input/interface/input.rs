//! What the rest of the game reaches of the input, by subject.

pub(crate) use super::super::domain::hpattern;
pub(crate) use super::super::domain::keys;
pub(crate) use super::super::domain::place::PlaceAt;
pub(crate) use super::super::implementation::actions;
pub(crate) use super::super::implementation::controllers;
#[cfg(windows)]
pub(crate) use super::super::implementation::dinput;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(crate) use super::super::implementation::evdev_buttons;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
pub(crate) use super::super::implementation::evdev_ff;
pub(crate) use super::super::implementation::game_controller_menu;
pub(crate) use super::super::implementation::headtrack;
pub(crate) use super::super::implementation::input_script;
pub(crate) use super::super::implementation::keyboard::route_char;
pub(crate) use super::super::implementation::keyboard::ROUTE_NUMBER_MAX;
#[cfg(target_os = "macos")]
pub(crate) use super::super::implementation::mac_game_controller;
#[cfg(target_os = "macos")]
pub(crate) use super::super::implementation::mac_hid;
pub(crate) use super::super::implementation::menus::*;
pub(crate) use super::super::implementation::mouse::*;
pub(crate) use super::super::implementation::touch;
