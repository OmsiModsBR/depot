# Data Model: Simplify the graphics modes

Design for the [plan](plan.md) and the [spec](spec.md). Phase names (A1, B1, C4...) are the plan's.

## GraphicsMode (new, `omsi-cfg`)

| Value | Saved as | Backend | Shading path |
|---|---|---|---|
| `Vanilla` | `vanilla` | D3D9 (always) | gamma, in `scene.hlsl` |
| `VanillaPlus` | `vanilla_plus` | wgpu (`graphics_api`) | `ShadingPath::Linear` |
| `Enhanced` | `enhanced` | wgpu (`graphics_api`) | `ShadingPath::Hdr` |

- `parse(&str) -> GraphicsMode` takes, ignoring case, `-` and spaces: `vanilla`, `classic`,
  `original`, `omsi`, `omsi2`, `omsi_2` → `Vanilla`; `enhanced`, `1`, `enhanced_plus`, `enhanced+`,
  `enhancedplus`, `enhanced_+`, `2` → `Enhanced`; anything else → `VanillaPlus` (today's default). A
  value that is none of the known spellings leaves a warning in the log.
- `as_str()` returns only the three canonical spellings.
- Replaces: `omsi-app::settings::graphics_mode`, `omsi-launcher-core::graphics_mode`, the literal
  names in the three crates, `Settings::classic()`, `Settings::ray_tracing()`.

## Resolution at start-up (once)

```text
mode = parse(settings.cfg "graphics")   # or the old key enhanced=1 when "graphics" is missing
mode = OMSI_GRAPHICS, when set          # the one mode variable that stays
mode = Enhanced, with --enhanced        # stays; --enhanced-plus goes (see the contract)
if mode == Vanilla and (VR or triple screen):   # FR-018a: not offered in Vanilla
    log a warning; Vanilla opens without them; the saved values are kept
backend = D3D9 if mode == Vanilla, else wgpu(graphics_api)
if backend == D3D9 cannot open:                 # FR-011a
    error naming the cause, "choose Vanilla+ in the launcher"; the game closes; nothing is rewritten
```

The result lives in one value (the resolved `Settings`/`App`) handed to whoever needs it. Gone:
`Settings::enhanced`, the globals `ENHANCED`, `ENHANCED_PLUS`, `CLASSIC`, the function
`enhanced_wanted`, and the reads of `OMSI_ENHANCED`, `OMSI_ENHANCED_PLUS`, `OMSI_NO_ENHANCED` spread
over the frame.

## Backend (exists, `omsi-app/src/output/domain/backend.rs`)

`Backend::of` stops reading `graphics_api == d3d9` and derives from the mode: `Vanilla → D3d9`, the
others → `Wgpu`. `graphics_api` takes `auto | vulkan | dx12 | gl`. `locked_for_api` / `lock_vanilla` and the `api_limits::D3D9` table are replaced by the graphics option
table below (phase B1).

## Lighting / RenderOptions (`omsi-render`)

| Today | After |
|---|---|
| `Lighting::classic: bool`, `Lighting::enhanced: bool` | `Lighting::path: ShadingPath` (`Linear`, `Hdr`) |
| `RenderOptions::no_enhanced: bool` | `RenderOptions::path: ShadingPath` (builds only that path's mode, phase C6) |
| `RenderOptions::ray_tracing: bool` | removed |
| `camera.sky_color.w` = 1 in Vanilla | no longer used (0 until it leaves the layout) |
| puddle `vehicle_info.w` ∈ {1, 0, -1} | {1, -1} (Enhanced, Vanilla+) |
| `LightMode { Both, Vanilla, Enhanced }` | stays (it describes the light path, not the mode); renaming `Vanilla` → `Linear` is optional |

## Material classification (one rule)

Written twice today (`renderer/lists.rs` and `effects/rt.rs`). With ray tracing removed one copy is
left; phase A2 extends it with the glass case:

| Material | Shadow map | Transmission (phase A2) |
|---|---|---|
| opaque | solid | — |
| alpha-tested | cut-out | — |
| blended with a transmap, writing depth | cut-out | — |
| blended, depth-checked | casts nothing | **casts transmission** (from the material's alpha/colour) |
| `[matl_noZcheck]`, or instance alpha < 0.01 | casts nothing | casts nothing |

The rule lives in one place (`scene` or `passes::shadow`), with a test in `omsi-render/tests/`.

## FrameFeatures (exists, `omsi-render/src/renderer/frame.rs`; phase C1)

Today a struct of 13 bools filled inside `begin_frame` and passed to the passes field by field.
After phase C1:

```text
FrameFeatures::decide(path, &RenderOptions, &FrameInputs) -> FrameFeatures   # pure, no GPU
FrameInputs  { view: Window | Mirror | XrEye | TriplePanel, lead_view, full_size, picture_size,
               rain, wetness, snow, has_glass_slot, has_reflective_material }
```

The passes take `&FrameFeatures` (or the `FrameContext` that holds it) instead of separate bools.
`rt_frame` leaves with phase A1; `vanilla_fxaa` is renamed `fxaa_without_msaa` with phase A6. With
phase C4 the decisions split: the core decides what both modes share (scaled target, glass, prepass,
AO, puddles wanted), and each mode adds its own through the mode interface.

## RenderOptions and DebugOptions (phase C2)

| Kind of flag | Today | After |
|---|---|---|
| Behaviour a player may set (`OMSI_NO_FXAA`, `OMSI_NO_AO`, `OMSI_NO_SHADOWS`, `OMSI_NO_PUDDLE_REFLECTIONS`, `OMSI_NO_GLASS_PICTURE`...) | read in the frame | a `RenderOptions` field, set by the app from the settings or the flag once |
| Diagnostic (`OMSI_DEBUG_*`, `OMSI_GPU_TIMERS`, `OMSI_PROFILE`...) | read where used | `DebugOptions`, read when the renderer opens, held on `Renderer` |
| Fault injection (`OMSI_FAKE_GPU_ERROR`) | 22 places in 6 files | `gpu::fault::due(Hook)`, one module |
| Unused | — | removed |

## Graphics option table (new, `omsi-cfg`, phase B1)

One row per graphics option key; the single source for the launcher and the in-game menu (FR-013,
FR-014). Starting content: [research.md](research.md#r-11-the-options-per-mode-what-the-code-does-today).

```text
GraphicsOption {
    key:      &str,                       # the settings key ("ssao", "night_brightness", ...)
    per_mode: [Applies; 3],               # indexed by GraphicsMode
    effect:   Effect,                     # Live | NextStart
}
Applies = Offered
        | Choices(&[&str])                # offered with fewer values (Vanilla's mirror sizes)
        | Fixed { shown: &str, reason: &str }   # a fixed line: value and why
        | NotOffered
```

- `offered_in_launcher(key, mode)` = `per_mode[mode]` is not `NotOffered`.
- `offered_in_game(key, mode)` = offered in the launcher **and** `effect == Live` (FR-020c); a
  `Fixed` row is shown in the game only if it is `Live` too.
- `fit(key, mode, value)`: the value the game uses: the saved one where offered, the nearest allowed
  one for `Choices`, the fixed one for `Fixed`; the saved value is never rewritten (FR-016).
- `graphics` and `graphics_api` are rows like any other, with `effect = NextStart`.
- A preset applies only the keys `Offered` or `Choices` in the current mode (FR-017); a mode in
  which a preset would change fewer than two keys does not offer presets.
- The reasons are message keys translated by the UI crates; `omsi-cfg` holds no translated text.
- Replaces: `omsi-launcher-ui/src/api_limits.rs`, the `classic`/`traced` rules of `shown()`, the
  `limit_or` and `Backend::D3d9` tests of the in-game list, `Settings::lock_vanilla`.

Validation (tests, FR-020): every option of the launcher's Graphics, Advanced graphics, VR and
triple-screen groups has a row; every row's key exists; the in-game list equals
`{ key : offered_in_game(key, mode) }` for each mode; every `Live` row's setter changes a value the
frame reads (checked per key by a `render_to_image` difference where the option draws something).

## Mode interface (new, `omsi-render/src/interface/mode.rs`, phase C4)

See [contracts/render-structure.md](contracts/render-structure.md). One trait, two implementations
(`modes::vanilla_plus`, `modes::enhanced`), one boxed value on `Renderer`, chosen from
`RenderOptions::path` when the renderer is built.

## Frame plan (moves, `omsi-render/src/interface/frame_plan.rs`, phase C4)

`FramePlan` and its parts, unchanged in content, out of `renderer/mirror.rs`. Its public names stay
re-exported by `api`. Consumed by `omsi-app/src/output/implementation/vanilla.rs`, whose translation
to `SceneFrame` gets the contract test of phase C8.

## Vanilla crate (renamed, phase C8)

| Today (`omsi-render-d3d9/src/`) | After (`omsi-render-vanilla/src/`) |
|---|---|
| `scene.rs`, `ui.rs`, `caps.rs`, `render_scale.rs`, `glass_rain.rs` | `domain/` |
| `backend.rs`: the `RenderBackend` trait, the descriptors | `interface/input.rs` |
| `backend.rs`: `NullBackend` | `implementation/null.rs` |
| `d3d9/` (device, passes, HLSL) | `implementation/d3d9/` |
| `d3d9/*_tests.rs`, `d3d9/tests.rs` | `tests/` (phase C9) |

The crate's public names keep their spelling apart from the crate name; `Dx9Effects` in `omsi-app`
becomes `VanillaEffects`; the `dx9_*` settings keys do not change.
