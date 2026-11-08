# Data Model: Simplify the graphics modes

Phase 1 of the [plan](plan.md).

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
if mode == Vanilla and (VR or triple screen):   # decision D3; recommended: the launcher does not offer it
    log a warning; the combination is not opened
backend = D3D9 if mode == Vanilla, else wgpu(graphics_api)
```

The result lives in one value (the resolved `Settings`/`App`) handed to whoever needs it. Gone:
`Settings::enhanced`, the globals `ENHANCED`, `ENHANCED_PLUS`, `CLASSIC`, the function
`enhanced_wanted`, and the reads of `OMSI_ENHANCED`, `OMSI_ENHANCED_PLUS`, `OMSI_NO_ENHANCED` spread
over the frame.

## Backend (exists, `omsi-app/src/output/domain/backend.rs`)

`Backend::of` stops reading `graphics_api == d3d9` and derives from the mode: `Vanilla → D3d9`, the
others → `Wgpu`. `graphics_api` takes `auto | vulkan | dx12 | gl`. `locked_for_api` / `lock_vanilla`
become "the limits of the Vanilla mode" (the `api_limits::D3D9` table is named after the mode).

## Lighting / RenderOptions (`omsi-render`)

| Today | After |
|---|---|
| `Lighting::classic: bool`, `Lighting::enhanced: bool` | `Lighting::path: ShadingPath` (`Linear`, `Hdr`) |
| `RenderOptions::no_enhanced: bool` | `RenderOptions::path: ShadingPath` (builds only that path's pipelines, phase 7) |
| `RenderOptions::ray_tracing: bool` | removed |
| `camera.sky_color.w` = 1 in Vanilla | no longer used (0 until it leaves the layout) |
| puddle `vehicle_info.w` ∈ {1, 0, -1} | {1, -1} (Enhanced, Vanilla+) |
| `LightMode { Both, Vanilla, Enhanced }` | stays (it describes the light path, not the mode); renaming `Vanilla` → `Linear` is optional |

## Material classification (one rule)

Written twice today (`renderer/lists.rs` and `effects/rt.rs`). With ray tracing removed one copy is
left; phase 2 extends it with the glass case:

| Material | Shadow map | Transmission (phase 2) |
|---|---|---|
| opaque | solid | — |
| alpha-tested | cut-out | — |
| blended with a transmap, writing depth | cut-out | — |
| blended, depth-checked | casts nothing | **casts transmission** (from the material's alpha/colour) |
| `[matl_noZcheck]`, or instance alpha < 0.01 | casts nothing | casts nothing |

The rule lives in one place (`scene` or `passes::shadow`), with a test in `omsi-render/tests/`.
