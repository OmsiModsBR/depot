# Implementation Plan: Three graphics modes, each with its own code and its own options

**Branch**: one per delivery phase (see "Delivery phases") | **Date**: 2026-11-13 |
**Spec**: [spec.md](spec.md) (issue #703)

**Input**: Feature specification from `.ai/specs/001-graphics-modes/spec.md` (clarified
2026-11-13). This plan replaces the one of 2026-11-08, which predated the spec; its removal and
refactor phases are kept and renumbered, and two tracks are new: the options per mode, and one
directory per mode with shared interfaces.

**Execution notes**: `.ai/.specify/scripts/bash/setup-plan.sh --json` ran (the plan file existed, so
the template was not copied over it). `.ai/.specify/extensions.yml` does not exist, so no hooks ran.
Code read at `cbeda16`.

## Summary

Depot has five graphics choices today: Vanilla (wgpu), Vanilla+, Enhanced, Enhanced+ and "DirectX9
(D3D9Ex)". The plan leaves **three modes**, each with its own code and its own options:

| Mode | Renderer | Lives in |
|---|---|---|
| **Vanilla** | Direct3D 9Ex (today's "DirectX9") | crate `omsi-render-vanilla` (renamed from `omsi-render-d3d9`) |
| **Vanilla+** | wgpu, linear shading (picture unchanged) | `omsi-render/src/modes/vanilla_plus/` + the shared core |
| **Enhanced** | wgpu, HDR shading and post passes, no ray tracing | `omsi-render/src/modes/enhanced/` + the shared core |

Three tracks, in this order of start:

- **Track A, the modes** (spec User Stories 1 and 4): remove Enhanced+; Enhanced takes the sunlight
  dimmed by glass; one `GraphicsMode` type resolved once; Vanilla becomes the Direct3D 9Ex renderer
  after its differences are settled and its speed measured; the wgpu Vanilla path is removed.
- **Track B, the options** (User Story 2): one applicability table (per mode: offered, fixed, not
  offered; and whether a change is live or needs a start) that the launcher and the in-game menu
  both read; the launcher's Graphics page puts the mode in its top section and the graphics API in
  "Picture"; the in-game menu offers only live options; `ssao`, `reflections` and `clouds` are made
  live.
- **Track C, the structure** (User Story 3): the frame's decisions and flags are untangled first;
  then `interface/` (the mode trait, the frame plan) and `modes/` are created and each mode's code
  moves in; only the chosen mode is built; the Vanilla crate is renamed and laid out as domain /
  interface / implementation; tests leave `src/`; the documents follow.

Track C starts after A6, so that only code that stays is moved (FR-029). Track B needs only A3 (the
mode type) to start; its Vanilla rows take effect with A5.

## Technical Context

**Language/Version**: Rust 1.99 (CI toolchain), WGSL (naga), HLSL Shader Model 3.0 (compiled at run
time by `d3dcompiler_47.dll`)

**Primary Dependencies**: `wgpu` (Vulkan/DX12/GL), `windows` (Direct3D 9Ex), `omsi-cfg` (flags and
key parsing; gains the mode type and the option table), `omsi-ui` (the launcher's and the menu's
widgets and translations)

**Storage**: `settings.cfg` in `~/.depot` (keys `graphics`, `graphics_api`, `enhanced`, and one key
per option); the launcher and the game read and write the same file

**Testing**: `cargo test --workspace`; GPU tests marked `#[ignore]` run on lavapipe; the
`offscreen_golden` and `offscreen-traces` jobs in CI; `NullBackend` for the Vanilla renderer without
a GPU; `scripts/check-render-layers.sh --strict` (blocking in CI); same-camera screenshots in the
game (`OMSI_INPUT` with `shot`)

**Target Platform**: Windows x64 (the only supported platform). Vanilla needs Windows 10+ and Shader
Model 3.0 with 2048 textures

**Project Type**: desktop application (game) in a crate workspace

**Performance Goals**: no regression in any mode (Principle VII, SC-006); opening the renderer gets
faster in Vanilla+ (SC-007); Vanilla on Direct3D 9Ex not slower than wgpu Vanilla before the switch
(FR-008, no tolerance)

**Constraints**: Vanilla+ never changes a pixel (FR-004); Enhanced changes only in phase A2
(FR-005); a moving or reorganising step changes no pixel, no trace and no frame time beyond noise,
and leaves the code smaller or less coupled (FR-030); no saved file is rewritten on reading (FR-012,
FR-016); the modes never import each other and the core never imports a mode (FR-024)

**Scale/Scope**: `omsi-render` about 28.5k lines, `omsi-render-d3d9` about 11.4k; 38 graphics option
keys plus the VR and triple-screen groups in the table; about 55 files touched by the Enhanced+
removal, about 12 by the wgpu Vanilla removal; about 30 files moved or split by track C

## Constitution Check

*GATE: before research, and again after design.*

| Principle | Status | How the plan meets it |
|---|---|---|
| I. Own path | ✓ | Removes inherited code; renames a crate; compatibility with openOMSI is not a cost |
| II. OMSI 2 fidelity only with validation | ✓ with a condition | Vanilla becomes "the OMSI 2 mode" only after phase A0: every difference of research R-V stays "to validate" until the maintainer checks it in the game (FR-007, FR-032) |
| III. No compensation for content | ✓ | Nothing changes in how content is read |
| IV. Invalid content never crashes | ✓ | Old or unknown `graphics` values, removed arguments and variables fall back with a log line (FR-010, FR-011). A machine that cannot open Vanilla gets an error and the game closes (FR-011a): that is a missing device capability, not content, and the maintainer chose it over a silent mode switch |
| V. Domains as packages | ✓ | `GraphicsMode` and the option table are domain objects in `omsi-cfg`; `omsi-render` gains `interface/` (a trait with two real implementations, as the principle requires for a trait) and `modes/`; the Vanilla crate gets `domain/`, `interface/`, `implementation/`. `omsi-render` keeps its layered core (its `ARCHITECTURE.md`), with `interface` and `modes` as two new layers |
| VI. Tests outside `src/` | ✓ | New tests go to `tests/`; phase C9 moves the rest out of both rendering crates |
| VII. No performance regression | ✓ with conditions | (a) A5 goes in only after the measurement of quickstart Q2 passes; (b) A2 and B4 are new features with before/after numbers and tests; (c) every C phase carries a before/after measurement, and C4 (the trait object) a `profile window:` comparison |
| VIII. Provenance and public text | ✓ | Differences describe only what is seen; no commit or pull request credits an assistant or links to one |
| IX. Small, traceable changes | ✓ | One pull request per phase (some phases are several, marked); each with its changelog line where the player notices |
| X. Lean, clean code | ✓ | Removal first (YAGNI: nothing of Enhanced+ or wgpu Vanilla kept behind a switch); one copy of the mode rule, of the options rule and of the material classification; the long mixed functions are split as they move (FR-030); `ARCHITECTURE.md` rule 8 ("unused code is not deleted") contradicts this principle and is corrected in C10 |

Result: **pass**, with the conditions above as tasks. Re-checked after design: unchanged.

## Project Structure

### Documentation (this feature)

```text
.ai/specs/001-graphics-modes/
├── spec.md              # the specification (clarified 2026-11-13)
├── plan.md              # this file
├── research.md          # R-V, R-E, R-1..R-6 (2026-11-08, re-checked), R-7..R-12 (structure, options)
├── data-model.md        # GraphicsMode, option table, mode interface, frame plan, Vanilla crate
├── quickstart.md        # how each phase is validated
├── tasks.md             # /speckit-tasks: 79 tasks by user story, each tagged with its phase
├── checklists/requirements.md
└── contracts/
    ├── settings-cfg.md        # saved keys and their migration
    ├── cli-env.md             # arguments and variables that go or change
    ├── graphics-options.md    # what the launcher and the in-game menu offer
    └── render-structure.md    # directories, file moves, import rules
```

### Source Code (repository root)

```text
crates/
├── omsi-cfg/src/
│   ├── graphics.rs (new)            # GraphicsMode: parse / as_str (A3)
│   ├── graphics_options.rs (new)    # the option table (B1)
│   └── flags.rs                     # mode and ray-tracing flags removed (A1, A3); frame flags (C2)
├── omsi-render/src/
│   ├── api/
│   ├── interface/ (new, C4)         # mode.rs, frame_plan.rs
│   ├── modes/ (new, C5)
│   │   ├── vanilla_plus/            # mod, pipelines, frame, sky, shaders/
│   │   └── enhanced/                # mod, pipelines, frame, lights, post, exposure, atmosphere/,
│   │                                # fog_lamps, targets, shaders/
│   ├── gpu/ assets/ lighting/ sky/ scene/ targets/ passes/ effects/ shaders/ output/   # shared core
│   ├── renderer/                    # frame decisions (C1), options (C2), build by pass (C3),
│   │                                # one mode built (C6), frame stages (C7)
│   ├── effects/rt.rs, shaders/effects/rt*.wgsl, shaders/common/rt_common.wgsl   # REMOVED (A1)
│   └── tests.rs, tests_units.rs     # MOVED to tests/ (C9)
├── omsi-render-vanilla/ (renamed from omsi-render-d3d9, C8)
│   ├── src/{domain,interface,implementation}/
│   └── tests/                       # scene, ui, upscale tests out of src (C9)
├── omsi-app/src/
│   ├── settings.rs                  # GraphicsMode; migration; no classic()/ray_tracing() (A1, A3, A5)
│   ├── core/implementation/{startup,make}.rs    # no ENHANCED, ENHANCED_PLUS, CLASSIC (A3)
│   ├── output/implementation/backend.rs         # Backend::of(mode) (A5)
│   ├── output/implementation/vanilla.rs         # was d3d9.rs; contract-tested translation (C8)
│   ├── hud/implementation/lists/                # in-game list generated from the table (B1, B3)
│   └── weather/implementation/{weather_setup,puddles}.rs   # classic case removed (A6)
├── omsi-launcher-core/src/lib.rs    # uses GraphicsMode; second graphics_mode removed (A3)
└── omsi-launcher-ui/src/
    ├── api_limits.rs                # REMOVED: replaced by the table (B1)
    └── window/{domain,implementation}/settings.rs   # shown() from the table (B1); page layout (B2)
scripts/check-render-layers.sh       # knows interface and modes; checks the crates' independence (C4, C8)
docs/USER_GUIDE.md, docs/ENVIRONMENT.md, crates/omsi-render/ARCHITECTURE.md,
crates/omsi-render-vanilla/README.md, README.md, THIRD_PARTY.md, CHANGELOG.md
```

**Structure Decision**: the directories approved in the spec. No new crate; one crate renamed. The
mode type and the option table go to `omsi-cfg` because the game, the launcher core and the launcher
UI already depend on it. Inside `omsi-render` the two modes are two domains of one new layer above
the core and below the renderer, reached through `interface/`; the file-level mapping is
[contracts/render-structure.md](contracts/render-structure.md). The **base surface shading stays in
the core**, because Enhanced draws its mirrors with it (research R-8).

## Delivery phases

One pull request per row unless the row says otherwise. "Pixels" says whether a player may see a
different picture.

### Track A: the modes

| # | Branch | Content | Needs | Pixels |
|---|---|---|---|---|
| A0 | (no code) | In-game check of the R-V differences (D1); Direct3D 9Ex against wgpu Vanilla measured (quickstart Q1, Q2) | — | — |
| A1 | `feat/remove-enhanced-plus` | Remove ray tracing, its options, flags and arguments (ignored with a log line); `enhanced_plus` read as `enhanced`; its grade, vignette and sharpening go with it; the second copy of the material rule goes; docs | — | only for Enhanced+ users |
| A2 | `feat/enhanced-glass-sunlight` | Glass dims the sun in Enhanced's shadow map (research E1), from the material's alpha and colour | A1 | yes, Enhanced (before/after, measured) |
| A3 | `feat/graphics-mode-type` | `GraphicsMode` in `omsi-cfg`; one resolution at start-up; `Settings::enhanced`, the globals `ENHANCED`, `ENHANCED_PLUS`, `CLASSIC` and the launcher core's `graphics_mode` go | A1 | no |
| A4 | `fix/dx9-*` (one per item) | The differences A0 decides to bring to Direct3D 9Ex; a speed fix if Q2 fails | A0 | yes, in DirectX9 |
| A5 | `feat/vanilla-is-dx9` | Vanilla chooses the Direct3D 9Ex backend; `graphics_api` loses `d3d9` and applies to Vanilla+ and Enhanced; saved `graphics_api=d3d9` read as Vanilla; VR and triple screen not applied in Vanilla (FR-018a); the open-failure error (FR-011a) | A0, A3, A4 | yes, for wgpu Vanilla users |
| A6 | `feat/remove-wgpu-vanilla` | Remove the `classic` branch of the shaders, `Lighting::classic`, the `sky_color.w` bit, the dead `flags.z` branch, the classic case of the spray and the puddles | A5 | no (the path can no longer be reached) |

### Track B: the options

| # | Branch | Content | Needs | Pixels |
|---|---|---|---|---|
| B1 | `feat/graphics-option-table` | The table in `omsi-cfg` with every row audited (research R-11 completed: each "to confirm" settled by reading the code and, for Vanilla, the device); the launcher's `shown()` and the in-game list read it; `api_limits.rs`, the loose `classic`/`traced` rules and the `Backend::D3d9` tests in the menu go; presets apply only offered keys; options the audit finds dead are removed with their code; the tests of FR-020; the table printed into the user guide | A3 | no; fewer options shown |
| B2 | `feat/launcher-graphics-mode-section` | Launcher Graphics page: the top section holds the mode with one line per mode; "Graphics API" in "Picture", absent in Vanilla; Vanilla's own options labelled by what they do (FR-018, FR-020a, FR-020b); translations | B1 | no |
| B3 | `feat/ingame-live-options` | The in-game graphics list is generated from the launcher's options filtered by the table (mode, live): no mode, API, preset or next-start option; the "next start" hints go (FR-020c, FR-020d) | B1 | no |
| B4 | `feat/live-ssao`, `feat/live-reflections`, `feat/live-clouds` (three) | Each option read per frame instead of at build; its row becomes `Live`; the fresh-start-equals-switch test; frame time before/after (FR-020e) | B3; best after C2 | no (same picture as a start with that value) |

Until A5 lands, the table's Vanilla column is applied where the Direct3D 9Ex backend is running
(exactly what `api_limits::D3D9` does today), so B1 to B3 do not wait for it.

### Track C: the structure

Starts after A6. Every phase: no pixel, no trace, no frame-time change (FR-030, quickstart Q10).

| # | Branch | Content | Needs |
|---|---|---|---|
| C1 | `feat/render-frame-features` | The frame's decisions (FXAA, glass, scaled target, HDR frame, puddles, AO, prepass) as one pure function, unit-tested without a GPU; `encode_main_pass` (22 parameters, 9 bools), `encode_enhanced_post` and `encode_glass` take the result as one value | A6 |
| C2 | `feat/render-options-flags` | Behaviour flags read in the frame become `RenderOptions` fields the app sets once; diagnostic flags become `DebugOptions` read at open; the `OMSI_FAKE_GPU_ERROR` hooks (22 places, 6 files) go to `gpu::fault`; flags nothing reads are removed | A3, A6 |
| C3 | `feat/render-build-by-pass` | `Renderer::build` (about 500 lines) split: each pass builds its pipelines in its own module, grouped by shading path | A6 |
| C4 | `feat/render-mode-interface` | `interface/mode.rs` (the trait and the parts) and `interface/frame_plan.rs` (`FramePlan` out of `renderer/mirror.rs`); two thin implementations that still call the code where it is; the renderer holds one boxed mode; the layer script learns `interface` and `modes`, with its planted-import tests; `profile window:` before/after | C1, C3 |
| C5 | `feat/render-mode-enhanced`, `feat/render-mode-vanilla-plus` (two) | Each mode's code moves into its directory per [contracts/render-structure.md](contracts/render-structure.md): the files that are one mode's whole, the mode's part of the split files (`renderer/frame.rs`, `encode.rs`, `passes.rs`, `passes/main.rs`, `lighting/model.rs`, `effects/puddles.rs`), and its WGSL (`corona.wgsl`, `snow.wgsl` split; research R-10). The base shading stays in the core. After both: no `if enhanced` left in the core | C4 |
| C6 | `feat/render-mode-pipelines` | `RenderOptions::path` replaces `no_enhanced`; only the chosen mode is built (FR-026); start-up time measured (SC-007) | C5 |
| C7 | `feat/render-frame-stages` | `begin_frame` (about 500 lines) and `encode_main_pass` (about 480) split into their stages, what is left of them after C5; optional: visibility out of `renderer/lists.rs` into `scene::visibility` | C5 |
| C8 | `feat/render-vanilla-crate` (two pull requests: the rename, then the layout) | `omsi-render-d3d9` renamed `omsi-render-vanilla` (workspace, CI, docs, `THIRD_PARTY.md`); `domain/`, `interface/`, `implementation/`; `omsi-app`'s `d3d9.rs` becomes `vanilla.rs` with the `FramePlan` → `SceneFrame` translation as a function of its own and its contract test on `NullBackend` (shared Rust/HLSL constants checked there); `draw_scene_inner` (about 270 lines) split by pass; `Dx9Effects` renamed `VanillaEffects` | A5, C4 |
| C9 | `feat/render-tests-out`, `feat/vanilla-tests-out` (two) | Tests out of `src/` in both crates, through each crate's `testing` module (Principle VI, SC-010) | — (any time; before C5 it makes the moves smaller) |
| C10 | `docs/render-docs-modes` | `omsi-render/ARCHITECTURE.md` (the two new layers, the mode rules, rule 8 corrected, the drift on `RenderOptions`), the Vanilla crate's README, `docs/USER_GUIDE.md` (the three modes, the option table), `docs/ENVIRONMENT.md`, `README.md`, `docs/MACRO_REFACTOR_DRAFT.md` references | C5, C8 |

### Order

```text
A0 ─────────────────────────────┐
A1 → A2                         │
A1 → A3 ────────────────────────┼→ A5 → A6 → C1 → C3 → C4 → C5 → C6
          └→ B1 → B2            │              └→ C2        └→ C7
                └→ B3 → B4      │                     C4 + A5 → C8
A0 → A4 ────────────────────────┘      C9 any time;  C10 last
```

Not in this plan, noted for later: `open_on` (432 lines), the XR session's `render` (469 lines,
`omsi-app`), the repeated full-screen-pass, render-target and bind-group set-up in `omsi-render`;
new Enhanced features (screen-space reflections, a new ambient occlusion); making further options
live.

## Requirements to phases

| Spec | Phases |
|---|---|
| FR-001, FR-006, FR-010, FR-011, FR-011a, FR-012, FR-018a | A3, A5 |
| FR-002 | A5, A6 |
| FR-003, FR-005 | A1, A2 |
| FR-004 | every phase (golden unchanged) |
| FR-007, FR-008 | A0, A4 |
| FR-009 | A3 |
| FR-013 to FR-019, FR-020 | B1 |
| FR-020a, FR-020b | B2 |
| FR-020c, FR-020d | B3 |
| FR-020e | B4 |
| FR-021, FR-022, FR-024, FR-025, FR-027 | C4, C5, C8 |
| FR-023 | C4, C8 |
| FR-026 | C6 |
| FR-028 | C9 |
| FR-029 | the order above |
| FR-030 | C1 to C9 (quickstart Q10) |
| FR-031, FR-032 | each phase's own docs; C10 |

## Decisions for the maintainer

Settled on 2026-11-13 and no longer open: VR and triple screen in Vanilla (not offered), a slower
Vanilla renderer (fixed first), what Enhanced inherits (the glass only), a machine that cannot open
Vanilla (clear error), options made live (a small set).

- **D1** (phase A0, open by nature): for each R-V difference, which side matches OMSI 2. A4 and A5
  wait on it.
- **D5** (decided 2026-11-13: agreed): the base surface shading stays in the shared core, because
  Enhanced draws its mirrors with it (0.6 ms against 12.7 ms per mirror with the enhanced shading).
  So `modes/vanilla_plus/` holds the sky, FXAA without MSAA, the puddles' own pass and the sprite
  entries, not the scene's fragment shader (research R-8).
- **D6** (decided 2026-11-13: agreed): the set made live is `ssao`, `reflections`, `clouds`
  (research R-12).

## Complexity Tracking

No violation to justify.
