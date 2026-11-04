mod actions;
mod controllers;
#[cfg(windows)]
mod dinput;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod evdev_buttons;
#[cfg(all(target_os = "linux", target_pointer_width = "64"))]
mod evdev_ff;
mod game_controller_menu;
mod headtrack;
mod hpattern;
mod keys;
#[cfg(target_os = "macos")]
mod mac_game_controller;
mod mouse;
mod touch;
