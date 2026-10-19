# Keyboard focus in the widget kit

`omsi-ui` can be driven from the keyboard. It is off unless a page asks for it:

```rust
ui.nav.enabled = true; // once, after Ui::new()
```

With it on:

- Tab and Shift+Tab move the focus through the widgets in the order they are drawn, wrapping around.
- Enter or Space presses the chosen widget (a held Enter repeats, like a held mouse button would not).
- A ring marks the chosen widget. A mouse press or Esc takes it away; the Esc that does so is not passed on to the page, an Esc with no ring showing is.
- An open dropdown or date picker keeps the keys. Tab leaves a focused text field.

Stops are the widgets that do something when clicked: buttons, icon buttons, switches, dropdowns, date fields and list rows (they use `Ui::interact_key`). Sliders, text fields, scroll bars and the cells inside a field use plain `Ui::interact` and are not stops, since Enter would do nothing on them.

Known gaps, for later steps: arrow keys on sliders and lists, Tab entering a text field, and scrolling the chosen widget into view (a row scrolled out of a list is skipped, not reached).

A new clickable widget that should be reachable calls `interact_key` instead of `interact`.
