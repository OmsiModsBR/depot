# Contract: the graphics options the player is offered

What the launcher and the in-game menu show, per mode. The rule behind both is the graphics option
table ([data model](../data-model.md#graphics-option-table-new-omsi-cfg-phase-b1)); its starting
content is in [research.md](../research.md#r-11-the-options-per-mode-what-the-code-does-today).

## Launcher, Settings > Graphics

```text
+-- GRAPHICS MODE --------------------------------- decides which other graphics options are offered --+
|  [ Vanilla+            v ]   Vanilla   the OMSI 2 picture, nothing added                             |
|   Vanilla . Vanilla+ . Enhanced       Vanilla+  the OMSI 2 picture with conservative improvements    |
|   Next start                          Enhanced  the advanced graphics                                |
+------------------------------------------------------------------------------------------------------+
+-- PICTURE ---------------------------------------+   +-- VIEW ----------------------------------------+
|  Quality preset        (Vanilla+, Enhanced)      |   |  ...                                           |
|  Graphics API          (Vanilla+, Enhanced)      |
|  Anti-aliasing                                   |
|  Render scale                                    |
|  Upscaling             (Vanilla, scale < 100 %)  |
|  Anisotropic filtering                           |
+--------------------------------------------------+
```

- The highlighted section at the top is named after the mode and holds the mode list with one line
  per mode. Its text no longer says the API limits options (FR-020a).
- "Graphics API" takes the row "Graphics" has today in "Picture": `Automatic`, `Vulkan`,
  `DirectX 12`, `OpenGL`; not offered in Vanilla (FR-020b). "DirectX9 (D3D9Ex)" leaves the list.
- Changing the mode redraws the page for the new mode at once (the change itself applies on the
  next start).
- A row the table marks `Fixed` is a fixed line: value and reason. A `NotOffered` row is absent, and
  a group left without rows is absent.
- VR and triple screen (Display and VR section) are absent in Vanilla (FR-018a).
- Search finds only offered options.

## In-game menu, Options > Graphics

- Lists exactly the launcher's graphics options that the table marks `Live` and offers in the
  running mode. `graphics`, `graphics_api`, the preset and every `NextStart` option are absent
  (FR-020c).
- No row carries a "takes effect when the game starts the next time" hint any more.
- The mode followed is the one the session runs (it may come from the command line or the
  environment), not the one in the file.
- A group or page left without rows is absent.

## Options made live by this feature (FR-020e)

`ssao`, `reflections`, `clouds` ([research.md](../research.md#r-12-which-options-are-made-to-apply-at-once-fr-020e)).
Until each one's pull request lands it is `NextStart` and so launcher-only.

## Tests the contract asks for

- `omsi-cfg/tests/`: the table has a row for every graphics key; no row for a key that does not
  exist.
- `omsi-launcher-ui/tests/`: for each mode, the rows of the Graphics page equal the table's
  `offered_in_launcher`; the top section holds `graphics`; `graphics_api` is in "Picture" in Vanilla+
  and Enhanced and absent in Vanilla.
- `omsi-app/tests/app/hud/`: for each mode, the in-game graphics list equals the table's
  `offered_in_game`; it never contains `graphics` or `graphics_api`.
- Per live option that draws something: `render_to_image` before and after the in-session change
  differs, and after the change equals a fresh start with the new value.
