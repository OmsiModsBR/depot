//! The launcher's widgets: the immediate-mode kit now lives in `omsi-ui` (`omsi_ui::widgets`),
//! where the in-game menus can draw with it too.

pub use omsi_ui::widgets::*;

#[cfg(test)]
mod tests {
    use super::*;

    /// Shift and Ctrl seen from their own keys (where the window never tells them, Android)
    /// make the chord a key is bound with (#634).
    #[test]
    fn modifier_keys_make_the_binding_chord() {
        use winit::keyboard::KeyCode as K;
        let mut m = Modifiers::default();
        let mut input = Input::default();
        m.key(K::ShiftLeft, true);
        m.key(K::ControlRight, true);
        m.apply(&mut input);
        assert_eq!(
            omsi_content::input::chord(input.shift, input.ctrl, input.alt),
            omsi_content::input::KEY_SHIFT | omsi_content::input::KEY_CTRL
        );
    }
}
