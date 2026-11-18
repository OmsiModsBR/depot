# Contract: graphics mode keys in `settings.cfg`

Read by the game (`omsi-app::settings`) and by the launcher (`omsi-launcher-core`,
`omsi-launcher-ui`) through the same `GraphicsMode::parse` ([data model](../data-model.md)).

## `graphics`

| Saved value | Today | After |
|---|---|---|
| `vanilla` (and `classic`, `original`, `omsi`, `omsi2`) | Vanilla on wgpu, or on DirectX9 with `graphics_api=d3d9` | Vanilla = DirectX9 |
| `vanilla_plus` / missing / unknown | Vanilla+ | Vanilla+ (unknown: a log warning) |
| `enhanced` (and `1`) | Enhanced (DirectX9 forces Vanilla) | Enhanced |
| `enhanced_plus` (and `enhanced+`, `2`) | Enhanced+ | **Enhanced** (logged once) |

The launcher and the game's menu offer only `Vanilla`, `Vanilla+`, `Enhanced`. The file is rewritten
only when the player saves; until then the old spelling keeps being read.

## `enhanced` (old key, files from before `graphics`)

Read only when `graphics` is missing: `enhanced=1` → `enhanced`, otherwise `vanilla_plus` (as today).
No longer written. The `Settings::enhanced` field goes.

## `graphics_api`

| Saved value | Today | After |
|---|---|---|
| `auto`, `vulkan`, `dx12`, `gl` | wgpu backend | the same, for Vanilla+ and Enhanced; ignored in Vanilla |
| `d3d9`, `d3d9ex`, `dx9` | DirectX9 backend, mode locked to Vanilla | read as `graphics=vanilla` once (it is what the player saw, whatever `graphics` says), and `graphics_api` as `auto`; logged once |

## Vanilla's own options

`dx9_snow`, `fewer_night_lights`, `dx9_upscale`, `dx9_glass_rain`, `dx9_coronas`, `dx9_stars`,
`dx9_cloud_picture`, `dx9_texture_limit`: same key names and values (renaming them would break
saved files); offered when the mode is Vanilla; their labels describe the effect, not the API
(FR-018).

## `vr`, `triple_screen` and their sub-keys

Kept in the file as saved. With `graphics=vanilla` they are not applied and not offered; the game
logs once that Vanilla opened without them (FR-018a).

## Values of options the mode hides or fixes

Never rewritten because of the mode: the game uses the fitted value
([data model](../data-model.md#graphics-option-table-new-omsi-cfg-phase-b1)) and the file keeps the
player's (FR-016).

## Tests the contract asks for (`omsi-app/tests/app/settings/`)

- every row above, read from a text, gives the expected mode and backend;
- round trip: loading and saving does not change a file with canonical spellings;
- a saved `enhanced_plus` becomes `enhanced` when saved.
- `graphics=vanilla` with `vr=1` or `triple_screen=1`: Vanilla, neither applied, both still in the
  file after a save;
- a value saved for an Enhanced-only key survives a save made in Vanilla+.
