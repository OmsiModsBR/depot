# Implementation Plan: Simplify the graphics modes

**Branch**: one per delivery phase (see "Delivery phases") | **Date**: 2026-11-08 |
**Spec**: issue #703 (no `spec.md`: the maintainer's request below is the specification)

> **Out of date (2026-11-13)**: [spec.md](spec.md) now exists and widens the scope: one directory
> per graphics mode with shared interfaces, the `omsi-render-d3d9` crate renamed after the Vanilla
> mode, and the graphics options offered per mode in the launcher and in the in-game menu. This plan
> and its companion documents predate it and are to be regenerated with `/speckit-plan`. Where they
> differ, the spec wins.

**Input**: the maintainer's request:

> - Remove the Vanilla mode: DirectX9 takes its place. Map the differences between Vanilla and
>   DirectX9 Vanilla to find any difference in graphics features, and which side is closer to the
>   OMSI 2 picture.
> - Remove Enhanced+: discontinued and its code removed.
> - Vanilla+: stays as it is, a more conservative mode with improvements, on modern graphics APIs.
> - Enhanced: the mode where the advanced graphics work goes. Compare it with Enhanced+ to find the
>   differences, especially in how transparent surfaces are handled. Ray tracing and path tracing
>   are not to be considered.
>
> Follow-up (2026-11-08): include in the plan a refactor of the code that addresses the points the
> initial analysis of the graphics modes raised.

**Execution notes**: `.ai/.specify/extensions.yml` does not exist, so no hooks ran. The feature
directory was created by hand (`setup-plan.ps1` was not run).

## Summary

Depot has five graphics modes today: Vanilla (wgpu), Vanilla+, Enhanced, Enhanced+ and DirectX9.
The plan leaves **three**:

| Mode | Renderer | Role |
|---|---|---|
| **Vanilla** | `omsi-render-d3d9` (Direct3D 9Ex), today's "DirectX9" | The OMSI 2 picture, nothing added |
| **Vanilla+** | wgpu, `shader.wgsl` without the `classic` branch (unchanged) | OMSI 2 with conservative improvements, modern APIs |
| **Enhanced** | wgpu, `enhanced.wgsl` + post passes | The advanced graphics work, no ray tracing |

Technical approach:

1. **Remove Enhanced+**: `effects/rt.rs` (1,307 lines), four `rt*.wgsl` shaders (~594 lines), the
   `//RT` lines of `enhanced.wgsl`, the `rt_frame` branches of the frame, the `OMSI_*RT*` flags, the
   ray-query device request, the `graphics=enhanced_plus` value (read as `enhanced`) and
   `--enhanced-plus`. Before that, Enhanced takes over what Enhanced+ does better **without** tracing
   rays: glass letting part of the sunlight through ([research.md](research.md), R-E).
2. **One type for the mode** (`GraphicsMode`, three values) resolved once at start-up, in place of
   the string, the `enhanced` bool, the three global `AtomicBool`s and the scattered bools
   ([data-model.md](data-model.md)).
3. **Vanilla becomes DirectX9**: choosing Vanilla chooses the D3D9 backend; `graphics_api` applies
   to Vanilla+ and Enhanced only. Before that, the differences between wgpu Vanilla and DirectX9
   that favour wgpu are fixed in DirectX9 ([research.md](research.md), R-V).
4. **Remove wgpu Vanilla**: the `classic` branch of `shader.wgsl` and of the puddle reflections, the
   bit in `camera.sky_color.w`, `Lighting::classic`, the `CLASSIC` global and what depends on it.
5. **Build only the chosen mode's pipelines** (Enhanced's only in Enhanced): changing the mode
   already needs a restart.
6. **Refactor the rendering code that stays** ([research.md](research.md), R-6), after the
   removals so that only live code is refactored, with no pixel and no performance change:
   - the frame's decisions in one pure, tested function, and the passes taking them as one value
     instead of loose bools;
   - environment flags out of the frame path;
   - `Renderer::build`, `begin_frame`, `encode_main_pass` and the DirectX9 `draw_scene_inner` split
     by pass;
   - a contract test of the `FramePlan` → DirectX9 translation;
   - tests out of `src/`;
   - the stale documents fixed.

## Technical Context

**Language/Version**: Rust 1.99 (CI toolchain), WGSL (naga), HLSL Shader Model 3.0 (compiled at run
time by `d3dcompiler_47.dll`)

**Primary Dependencies**: `wgpu` (Vulkan/DX12/GL), `windows` (Direct3D 9Ex), `omsi-cfg` (flags and
key parsing)

**Storage**: `settings.cfg` in `~/.depot` (keys `graphics`, `graphics_api`, `enhanced`); the
launcher reads and writes the same file

**Testing**: `cargo test --workspace`; GPU tests marked `#[ignore]` run on lavapipe; the
`offscreen_golden` job in CI (lavapipe, informational); `omsi-render-d3d9`'s `NullBackend` for
DirectX9 without a GPU; same-camera screenshots in the game (`OMSI_INPUT` with `shot`)

**Target Platform**: Windows x64 (the only supported platform). DirectX9 needs Windows 10+ and
Shader Model 3.0 with 2048 textures

**Project Type**: desktop application (game) in a crate workspace

**Performance Goals**: no regression (Principle VII). Removals must not make any mode slower;
opening the renderer should get faster in Vanilla+ (fewer pipelines)

**Constraints**: no pixel changes in Vanilla+ and Enhanced in the removal phases, except the phase
that brings the glass behaviour to Enhanced (a visible change, with before/after); players with
`graphics=vanilla` or `enhanced_plus` saved must not end up in a mode that does not exist

**Refactor rule**: a refactor phase changes no pixel, no physics or state trace and no frame
time beyond measurement noise (Principle VII gives refactors no exception), and leaves the code
smaller or less coupled than before (Principle X)

**Scale/Scope**: `omsi-render` 28.5k lines, `omsi-render-d3d9` 11.4k; about 55 files touched by
the Enhanced+ removal (counted by `grep` on `19290ac`), about 12 by the wgpu Vanilla removal

## Constitution Check

*GATE: before research, and again after design.*

| Principle | Status | How the plan meets it |
|---|---|---|
| I. Own path | ✓ | Removes inherited code; compatibility with openOMSI is not a cost |
| II. OMSI 2 fidelity only with validation | ✓ with a condition | DirectX9 becomes "the OMSI 2 mode"; every difference in the R-V table is marked "to validate in the game" until the maintainer checks it. No public text says "as OMSI 2" without that proof |
| III. No compensation for content | ✓ | Nothing changes in how content is read |
| IV. Invalid content never crashes | ✓ | Old or unknown `graphics` values fall back to a valid mode with a log line |
| V. Domains as packages | ✓ | `GraphicsMode` is a domain object (`omsi-cfg`); choosing the backend stays in the app's `output` implementation |
| VI. Tests outside `src/` | ✓ | New tests go to `tests/`; `omsi-render/src/tests.rs`, `src/tests_units.rs` and `omsi-render-d3d9/src/d3d9/scene_tests.rs` move out when each crate is touched |
| VII. No performance regression (refactors) | ✓ with a condition | Each refactor phase carries a before/after of the mode measurement in [research.md](research.md#baseline-measurements) and, where it touches the frame, a `profile window:` comparison on the same scene |
| VII. No performance regression | **condition** | Players on wgpu Vanilla move to DirectX9, which has **not been measured** against it yet: phase 5 goes in only with before/after measurements on the same scene (quickstart Q4) |
| VIII. Provenance and public text | ✓ | Differences describe only what is seen |
| IX. Small, traceable changes | ✓ | Seven pull requests, one topic each |
| X. Lean, clean code | ✓ | Mostly removal (YAGNI) and de-duplication (`graphics_mode` in two crates; the "a body cut by its transmap is a cut-out" rule in `lists.rs` and `rt.rs`). The refactor phases (8-14) target the long functions, the bool parameter lists and the environment flags read in the frame |

Result: **pass**, with two conditions that become tasks: validate the differences in the game (II)
and measure DirectX9 before it replaces wgpu Vanilla (VII). Re-checked after design: unchanged.

## Project Structure

### Documentation (this feature)

```text
.ai/specs/001-graphics-modes/
├── plan.md              # this file
├── research.md          # Phase 0: R-V (Vanilla x DirectX9), R-E (Enhanced x Enhanced+), decisions
├── data-model.md        # Phase 1: GraphicsMode, backend, saved-settings migration
├── quickstart.md        # Phase 1: how each phase is validated
└── contracts/
    ├── settings-cfg.md  # accepted keys and values, migration
    └── cli-env.md       # arguments and variables that go or change
```

### Source Code (repository root)

```text
crates/
├── omsi-cfg/src/
│   ├── flags.rs                     # drops OMSI_BACKEND, OMSI_ENHANCED, OMSI_ENHANCED_PLUS, OMSI_NO_ENHANCED, OMSI_NO_RT*, OMSI_RT_REFL_HALF, OMSI_DEBUG_RT
│   └── graphics.rs (new)            # GraphicsMode: parse/as_str (replaces both graphics_mode copies)
├── omsi-render/src/
│   ├── effects/rt.rs                # REMOVED
│   ├── shaders/effects/rt*.wgsl     # REMOVED; shaders/common/rt_common.wgsl REMOVED
│   ├── shaders/scene/enhanced.wgsl  # no //RT lines, no rt_at
│   ├── shaders/scene/shader.wgsl    # no classic branch, no dead flags.z branch
│   ├── shaders/effects/puddle_reflection.wgsl  # no vehicle_info.w == 0 (classic) case
│   ├── lighting/model.rs            # Lighting { path } in place of classic/enhanced
│   ├── renderer/{open,build,frame,lists,prepare,reflections,effects,encode,passes}.rs
│   ├── gpu/{formats,errors,context}.rs  # no RT_GBUF/RT_BUFFERS, no ray-query request
│   ├── passes/{main,post,shadow}.rs # glass transmitting sunlight in the shadow map (phase 2); passes take &FrameFeatures (phase 8)
│   ├── renderer/frame.rs            # FrameFeatures::decide, a pure function (phase 8); begin_frame split (phase 11)
│   ├── renderer/options.rs          # behaviour flags become RenderOptions fields; DebugOptions (phase 9)
│   ├── gpu/fault.rs (new)           # the OMSI_FAKE_GPU_ERROR hooks in one place (phase 9)
│   ├── renderer/build.rs            # each pass builds its own pipelines (phase 10)
│   ├── tests.rs, tests_units.rs     # MOVED to tests/ (phase 13)
│   └── ARCHITECTURE.md              # drift fixed (phase 14)
├── omsi-render-d3d9/                # becomes the Vanilla mode; README updated (phase 14)
│   ├── src/d3d9/scene.rs            # draw_scene_inner split by pass (phase 12)
│   └── src/d3d9/scene_tests.rs      # MOVED to tests/ (phase 13)
├── omsi-app/src/
│   ├── settings.rs                  # GraphicsMode; migration; no classic()/ray_tracing()
│   ├── core/implementation/{startup,make}.rs  # no ENHANCED, ENHANCED_PLUS, CLASSIC
│   ├── output/implementation/backend.rs       # Backend::of from the mode
│   ├── output/implementation/d3d9.rs          # FramePlan → SceneFrame translation, contract-tested (phase 12)
│   ├── weather/implementation/{weather_setup,puddles}.rs
│   └── hud/implementation/lists/{select,pages}.rs
├── omsi-launcher-core/src/lib.rs    # uses GraphicsMode
└── omsi-launcher-ui/src/{api_limits.rs, window/domain/settings.rs}
docs/USER_GUIDE.md, docs/ENVIRONMENT.md, CHANGELOG.md
```

**Structure Decision**: no new crate. The mode type goes to `omsi-cfg` because the app, the
launcher core and the launcher UI already depend on it and today copy the same function;
`omsi-render` receives only what it needs to draw (`ShadingPath`) and knows nothing of backends.

## Delivery phases

Ordered so that each pull request is small, revertible and does not wait on an open decision.

| # | Branch | Content | Needs | Pixels change? |
|---|---|---|---|---|
| 0 | (no code) | In-game validation of the R-V differences; DirectX9 x wgpu Vanilla measurement (quickstart Q1, Q4) | — | — |
| 1 | `feat/remove-enhanced-plus` | Remove ray tracing, its flags, `--enhanced-plus`; `enhanced_plus` read as `enhanced`; docs | — | only for Enhanced+ users |
| 2 | `feat/enhanced-glass-sunlight` | Glass lets part of the sun through in Enhanced's shadow map (R-E1); optionally Enhanced+'s grade, vignette and sharpening (decision D2) | 1 | yes, in Enhanced (before/after) |
| 3 | `feat/graphics-mode-type` | `GraphicsMode` in `omsi-cfg`; one resolution at start-up; `Settings::enhanced`, `ENHANCED`, `ENHANCED_PLUS`, `CLASSIC` and the second `graphics_mode` go | 1 | no |
| 4 | `fix/dx9-*` (one per item) | The differences phase 0 decides to bring to DirectX9 | 0 | yes, in DirectX9 |
| 5 | `feat/vanilla-is-dx9` | Vanilla chooses the D3D9 backend; `graphics_api` for Vanilla+/Enhanced only; Vanilla with VR or triple screen per decision D3 | 0, 3, 4 | yes, for wgpu Vanilla users |
| 6 | `feat/remove-wgpu-vanilla` | Remove the `classic` branch of the shaders, `Lighting::classic`, the `sky_color.w` bit, the dead `flags.z` branch, the classic case of the spray and the puddles | 5 | no (the mode can no longer be reached) |
| 7 | `feat/mode-pipelines` | Build only the chosen mode's pipelines (end of "a computer always builds Enhanced") | 3, 6, 10 | no; faster renderer start-up |

### Refactor track (R-6)

Starts once phase 6 has removed the dead modes, so nothing is refactored only to be deleted. Phases
13 and 14 do not depend on the modes and can go in at any time. Each phase is one pull request (or
one per crate, where marked).

| # | Branch | Content | Needs | Pixels change? |
|---|---|---|---|---|
| 8 | `feat/render-frame-features` | `FrameFeatures::decide`: the frame's decisions (FXAA, glass, scaled target, Enhanced, puddles, AO, prepass) as one pure function, unit-tested without a GPU. `encode_main_pass` (22 parameters, 9 of them bools), `encode_enhanced_post` and `encode_glass` take `&FrameFeatures` | 6 | no |
| 9 | `feat/render-options-flags` | Behaviour flags read in the frame (`NO_FXAA`, `NO_AO`, `NO_GLASS_PICTURE`, `NO_PUDDLE_REFLECTIONS`, `NO_SHADOWS`...) become `RenderOptions` fields the app reads once; diagnostic flags become a `DebugOptions` read when the renderer opens; the `OMSI_FAKE_GPU_ERROR` hooks (22 places in 6 files) go into `gpu::fault`; flags nothing reads any more are removed | 3, 6 | no |
| 10 | `feat/render-build-by-pass` | `Renderer::build` (504 lines) split: each pass builds its own pipelines in its module (the scene, corona, snow and sky pipelines are built inline today, against `ARCHITECTURE.md` rule 5), grouped by shading path so phase 7 can leave a path out | 6 | no |
| 11 | `feat/render-frame-stages` | `begin_frame` (507 lines) and `encode_main_pass` (478) split into their stages (targets, lights and sprites, camera uniform; sky, world, cab, overlays). Optional: visibility out of `renderer/lists.rs` into `scene::visibility` (an `ARCHITECTURE.md` "not yet in place" item) | 8 | no |
| 12 | `feat/dx9-frame-contract` | The `FramePlan` → `SceneFrame` translation (`omsi-app` `d3d9.rs`, `frame` 175 lines) becomes a function of its own with a contract test on `NullBackend`: the same plan gives the same batches, coronas, sky and lights; the shared Rust/HLSL constants are checked there too (item C7 of `docs/MACRO_REFACTOR_DRAFT.md`). `draw_scene_inner` (271 lines) split by pass (sky, world, smoke, coronas, glass rain) | 5 | no |
| 13 | `feat/render-tests-out`, `feat/d3d9-tests-out` | Tests out of `src/` (Principle VI): `omsi-render/src/tests.rs` (2,270 lines), `src/tests_units.rs` (1,308), the in-file test modules (30 files with `#[cfg(test)]` in the two crates), `omsi-render-d3d9/src/d3d9/scene_tests.rs` (2,477), through each crate's `testing` module | — | no |
| 14 | `docs/render-docs-drift` | `omsi-render-d3d9/README.md` (says smoke and mirrors are not drawn; they are), the `d3d9.rs` header (navigator maps), `ARCHITECTURE.md` (`RenderOptions` said to live in `api`) | — | no |

Not in this plan, noted for later: `open_on` (432 lines), the XR session's `render` (469 lines,
`omsi-app`), the repeated full-screen-pass, render-target and bind-group set-up in `omsi-render` (no shared helper today).

## Decisions for the maintainer

- **D1** (phase 0): for each R-V difference, which side matches OMSI 2. Phase 5 waits on it
  (Principle II).
- **D2** (phase 2): bring only the glass, or also the colour grade, vignette and sharpening that
  Enhanced+ adds in its post pass? The grade is an aesthetic choice, not a fix.
- **D3** (phase 5): Vanilla with VR or triple screen. DirectX9 has neither (Depot's OpenXR session
  needs DirectX 12). Options: (a) Vanilla hides VR and triple screen in the launcher; (b) Vanilla
  with VR/triple falls back to Vanilla+ with a notice. Recommended: (a), simpler and with no
  implicit mode.
- **D4** (phase 5): a card without Shader Model 3.0 or without `d3dcompiler_47.dll` today gets a
  fatal "choose another graphics API". With Vanilla = DirectX9 the text becomes "choose Vanilla+".
  Falling back to Vanilla+ silently would be an implicit mode; recommended: keep the clear error.

## Complexity Tracking

No violation to justify.
