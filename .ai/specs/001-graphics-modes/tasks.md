---
description: "Tasks for the three graphics modes, each with its own code and its own options"
---

# Tasks: Three graphics modes, each with its own code and its own options

**Input**: design documents in `.ai/specs/001-graphics-modes/` ([plan](plan.md), [spec](spec.md),
[research](research.md), [data model](data-model.md), [quickstart](quickstart.md),
[contracts](contracts/))

**Prerequisites**: plan.md, spec.md (clarified 2026-11-13), research.md, data-model.md, contracts/

**Tests**: requested by the spec (FR-020, FR-020e, FR-023, FR-024) and by the constitution
(Principle VI: tests outside `src/`; a `fix/` always comes with its failing test; Principle VII:
before/after measurements).

**Organization**: by user story, as Spec Kit asks. Each task also names its **delivery phase** of the
plan (`[ph A1]`, `[ph B3]`, `[ph C5]`...), because the work ships as one pull request per phase on
its own branch. A phase's pull request holds every task tagged with it, whatever story the task
serves.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1..US4 of the spec; setup, foundational and polish tasks carry none
- **[ph X]**: the plan's delivery phase (A0-A6, B1-B4, C1-C10)

## Path conventions

Rust workspace: `crates/<crate>/src/` for code, `crates/<crate>/tests/` for tests (Principle VI),
`docs/` for public documents, `scripts/` for the CI checks.

## Rules every task follows

- Vanilla+ never changes a pixel (FR-004): `offscreen_golden` and the GPU tests pass with unchanged
  references in every pull request.
- A task tagged with a C phase changes no pixel, no trace and no frame time beyond noise, and leaves
  `src/` no larger unless the pull request says what it added and why (FR-030, quickstart Q10).
- A pull request that changes what the player notices adds its line to **Unreleased** in
  `CHANGELOG.md` and updates `docs/USER_GUIDE.md` / `docs/ENVIRONMENT.md` in the same pull request.
- No public text says Vanilla matches OMSI 2 in a point T001 has not confirmed (FR-032).

---

## Phase 1: Setup (in-game checks and measurements, no code)

**Purpose**: what phases A4 and A5 wait on, done by the maintainer on a machine with OMSI 2
(quickstart Q1, Q2), and the reference numbers every later comparison uses.

- [ ] T001 [ph A0] Q1: three same-camera screenshots (OMSI 2, Depot `graphics=vanilla` on DirectX 12, Depot on DirectX9) for each row V1-V14; fill an "OMSI 2 = ?" column in R-V of `.ai/specs/001-graphics-modes/research.md` and list which rows become `fix/dx9-*` (decision D1, FR-007). **Done 2026-11-13**: V1, V2, V4, V5, V6, V7, V8 (keep DirectX9's behaviour), V13 (keep DirectX9's snow; its default to confirm), V14 (OMSI 2 has dynamic shadows; a Vanilla gap outside this feature). **Still open**: V3 (to confirm from the screenshots), V9, V10, V11, V12
- [ ] T002 [P] [ph A0] Q2: `OMSI_PROFILE=1`, 5 minutes on the same map, bus and stretch, `graphics=vanilla` on DirectX 12 and then `graphics_api=d3d9`; record the `profile window:` lines and loading times under "Baseline measurements" in `.ai/specs/001-graphics-modes/research.md` and state pass or fail of FR-008
- [ ] T003 [P] Record the reference numbers at the current `main` in `.ai/specs/001-graphics-modes/research.md`: the per-mode measurement (one process per mode, lavapipe), the renderer start-up time in Vanilla+ and Enhanced, and `wc -l` of `crates/omsi-render/src` and `crates/omsi-render-d3d9/src`

**Checkpoint**: T001 and T002 decide the content of phase A4; T003 is the "before" of SC-006, SC-007 and SC-009.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the mode as one type resolved once. Every story reads it. Enhanced+ goes first because
the type has three values, not four.

### Remove Enhanced+ `feat/remove-enhanced-plus`

- [ ] T004 [ph A1] Delete `crates/omsi-render/src/effects/rt.rs`, `crates/omsi-render/src/shaders/effects/rt.wgsl`, `rt_avg.wgsl`, `rt_reflect.wgsl`, `crates/omsi-render/src/shaders/common/rt_common.wgsl` and `crates/omsi-render/tests/rt_smoke.rs`; remove the `//RT` lines and `rt_at` from `crates/omsi-render/src/shaders/scene/enhanced.wgsl` and the ray-tracing assembly from `crates/omsi-render/src/shaders/mod.rs`
- [ ] T005 [ph A1] Remove the `rt_frame` branches, `RenderOptions::ray_tracing`, the ray-query device request and the `RT_GBUF`/`RT_BUFFERS` formats from `crates/omsi-render/src/renderer/{build,frame,encode,passes,prepare,reflections,effects,options,open}.rs`, `crates/omsi-render/src/gpu/{formats,errors,context}.rs` and `crates/omsi-render/src/targets/hdr.rs`
- [ ] T006 [ph A1] Remove Enhanced+'s colour grade, vignette and sharpening from `crates/omsi-render/src/passes/post.rs` and `crates/omsi-render/src/shaders/post/post.wgsl` (FR-005); the Enhanced golden stays unchanged
- [ ] T007 [ph A1] Keep one copy of the material classification rule in `crates/omsi-render/src/renderer/lists.rs` (the copy in `rt.rs` went with T004) and add its test in `crates/omsi-render/tests/material_classes.rs`
- [ ] T008 [P] [ph A1] Remove `OMSI_ENHANCED_PLUS`, `OMSI_NO_RT`, `OMSI_NO_RT_GRADE`, `OMSI_NO_RT_FRAME`, `OMSI_RT_REFL_HALF`, `OMSI_DEBUG_RT` from `crates/omsi-cfg/src/flags.rs`; make a removed variable in `env.txt` and the `--enhanced-plus` argument (`crates/omsi-app/src/core/domain/cli.rs`) ignored with one log line, never an error ([contracts/cli-env.md](contracts/cli-env.md))
- [ ] T009 [ph A1] Read `graphics=enhanced_plus` (and its old spellings) as Enhanced with one log line in `crates/omsi-app/src/settings.rs` and `crates/omsi-launcher-core/src/lib.rs`; remove `Settings::ray_tracing()`, the "Enhanced+" entry of `crates/omsi-app/src/hud/implementation/lists/select.rs` and `crates/omsi-launcher-ui/src/window/domain/settings.rs`, and the `traced` rules of `shown()` and of `crates/omsi-app/src/hud/implementation/lists/pages.rs`
- [ ] T010 [P] [ph A1] Tests in `crates/omsi-app/tests/app/settings/` for the `enhanced_plus` rows of [contracts/settings-cfg.md](contracts/settings-cfg.md); run quickstart Q3 (`grep` clean, golden unchanged); `CHANGELOG.md`, `docs/USER_GUIDE.md`, `docs/ENVIRONMENT.md`

### The mode type `feat/graphics-mode-type`

- [ ] T011 [ph A3] Create `crates/omsi-cfg/src/graphics.rs` with `GraphicsMode { Vanilla, VanillaPlus, Enhanced }`, `parse` (takes, ignoring case, `-` and spaces: `vanilla`, `classic`, `original`, `omsi`, `omsi2`, `omsi_2` → `Vanilla`; `enhanced`, `1`, `enhanced_plus`, `enhanced+`, `enhancedplus`, `enhanced_+`, `2` → `Enhanced`; anything else → `VanillaPlus`, with a log warning for an unknown spelling) and `as_str` (only the three canonical spellings); tests in `crates/omsi-cfg/tests/graphics_mode.rs`
- [ ] T012 [ph A3] Resolve the mode once at start-up in `crates/omsi-app/src/settings.rs` and `crates/omsi-app/src/core/implementation/startup.rs` (file, then the old `enhanced=1` key when `graphics` is missing, then `OMSI_GRAPHICS`, then `--enhanced`); remove `Settings::enhanced`, `Settings::classic()`, `settings::graphics_mode`, `enhanced_wanted` and the reads of `OMSI_ENHANCED`, `OMSI_NO_ENHANCED` in the frame
- [ ] T013 [ph A3] Remove the globals `ENHANCED`, `ENHANCED_PLUS`, `CLASSIC` from `crates/omsi-app/src/core/implementation/{startup,make}.rs` and hand the resolved mode to their readers (`crates/omsi-app/src/weather/implementation/weather_setup.rs`, `crates/omsi-app/src/lights.rs`, `crates/omsi-app/src/world/implementation/scene.rs`, `crates/omsi-app/src/showroom/implementation/scene.rs`, `crates/omsi-app/src/output/implementation/camera/util.rs`)
- [ ] T014 [P] [ph A3] Replace `graphics_mode` and the literal mode names in `crates/omsi-launcher-core/src/lib.rs`, `crates/omsi-launcher-core/src/settings/implementation/api.rs` and `crates/omsi-launcher-ui/src/window/domain/settings.rs` with `GraphicsMode`
- [ ] T015 [ph A3] Tests in `crates/omsi-app/tests/app/settings/`: every `graphics` and `enhanced` row of [contracts/settings-cfg.md](contracts/settings-cfg.md), the round trip, and that reading does not rewrite the file (FR-012); `grep -rn "CLASSIC\|ENHANCED\.load\|graphics_mode(" crates` empty outside `GraphicsMode` (quickstart Q5)

**Checkpoint**: one mode value, read everywhere; four choices still reachable (Vanilla on wgpu, Vanilla+, Enhanced, DirectX9 by `graphics_api`). No pixel changed except for Enhanced+ users.

---

## Phase 3: User Story 1 - Three modes to choose from (Priority: P1) 🎯 MVP

**Goal**: exactly Vanilla, Vanilla+ and Enhanced; Vanilla drawn by the Direct3D 9Ex renderer; every
old saved choice opens the mode that replaces it.

**Independent Test**: start the game once with each old saved choice (five modes, with and without
the DirectX9 API) and check which mode opens and what the settings show (quickstart Q5).

### Bring the missing pieces to Direct3D 9Ex `fix/dx9-*`

- [ ] T016 [US1] [ph A4] One `fix/dx9-<item>` pull request per row T001 decides (candidates: V3 wet road and puddles, V10 transmap depth, V11 surface uv, V12 fog through the player's vehicle, V13 snow on by default) in `crates/omsi-render-d3d9/src/d3d9/scene.rs` and `crates/omsi-render-d3d9/src/d3d9/shaders/scene.hlsl`, each with its failing test in `crates/omsi-render-d3d9/` on `NullBackend`
- [ ] T017 [US1] [ph A4] If T002 failed: find and fix the difference in `crates/omsi-render-d3d9/src/d3d9/` as its own `fix/dx9-<cause>` pull request, then repeat T002; A5 does not start until it passes (FR-008, no tolerance)

### Vanilla is the Direct3D 9Ex renderer `feat/vanilla-is-dx9`

- [ ] T018 [US1] [ph A5] `Backend::of` derives from the mode in `crates/omsi-app/src/output/implementation/backend.rs` (`Vanilla → D3d9`, others → `Wgpu`); remove `is_d3d9`, the `OMSI_BACKEND` flag (`crates/omsi-cfg/src/flags.rs`) and `Settings::lock_vanilla` / `d3d9_fixes_vanilla` (`crates/omsi-launcher-core/src/lib.rs`)
- [ ] T019 [US1] [ph A5] Read a saved `graphics_api=d3d9|d3d9ex|dx9` as `graphics=vanilla` with `graphics_api=auto`, logged once, in `crates/omsi-app/src/settings.rs` and `crates/omsi-launcher-core/src/lib.rs`; `graphics_api` takes `auto | vulkan | dx12 | gl` and is ignored in Vanilla (FR-006, FR-010)
- [ ] T020 [US1] [ph A5] With `GraphicsMode::Vanilla`, do not apply `vr` and `triple_screen`: Vanilla opens without them, one log line, the saved values kept, in `crates/omsi-app/src/settings.rs` and `crates/omsi-app/src/core/implementation/startup.rs` (FR-018a)
- [ ] T021 [US1] [ph A5] When the Direct3D 9Ex device cannot open, show an error that names the cause and says "choose Vanilla+ in the launcher", then close; no other mode opens and `settings.cfg` is not written, in `crates/omsi-app/src/output/implementation/window.rs` and `crates/omsi-app/src/output/implementation/d3d9.rs` (FR-011a)
- [ ] T022 [US1] [ph A5] Remove "DirectX9 (D3D9Ex)" from `api_options()` and the Vanilla lock rows in `crates/omsi-launcher-ui/src/window/domain/settings.rs`, `crates/omsi-launcher-ui/src/api_limits.rs` and `crates/omsi-app/src/hud/implementation/lists/select.rs`; the limits now follow the mode, not the API
- [ ] T023 [P] [US1] [ph A5] Tests in `crates/omsi-app/tests/app/settings/` and `crates/omsi-app/tests/app/output/`: the `graphics_api` rows and the VR/triple rows of [contracts/settings-cfg.md](contracts/settings-cfg.md); the open-failure path with a refusing backend (error text, no write); `CHANGELOG.md`, `docs/USER_GUIDE.md` (the three modes, the launcher preview drawn as Vanilla+ in Vanilla), `docs/ENVIRONMENT.md`

### Remove the wgpu Vanilla path `feat/remove-wgpu-vanilla`

- [ ] T024 [US1] [ph A6] Remove the `classic` branch and the never-taken `camera.flags.z` branch from `crates/omsi-render/src/shaders/scene/shader.wgsl`, the `vehicle_info.w == 0` case from `crates/omsi-render/src/shaders/effects/puddle_reflection.wgsl`, and the `sky_color.w` bit from `crates/omsi-render/src/scene/camera.rs`
- [ ] T025 [US1] [ph A6] Replace `Lighting::classic` and `Lighting::enhanced` with `Lighting::path: ShadingPath { Linear, Hdr }` in `crates/omsi-render/src/lighting/model.rs` and its readers in `crates/omsi-render/src/renderer/`, `crates/omsi-render/src/effects/puddles.rs`, `crates/omsi-render/src/sky/` and `crates/omsi-app/src/frame/implementation/app_events.rs`; rename `vanilla_fxaa` to `fxaa_without_msaa`
- [ ] T026 [US1] [ph A6] Remove the classic case of the spray and the puddles in `crates/omsi-app/src/weather/implementation/{weather_setup,puddles}.rs`; run quickstart Q6 (golden unchanged)

**Checkpoint**: three modes, each reachable one way. User Story 1 passes its acceptance scenarios; the option lists still follow the old rules.

---

## Phase 4: User Story 2 - Only the options that do something in the chosen mode (Priority: P2)

**Goal**: one table decides what the launcher and the in-game menu offer per mode; the launcher's
page leads with the mode; the in-game menu holds only options that act at once.

**Independent Test**: per mode, the offered lists equal the table and every offered option has an
observable effect; every in-game option acts in the session (quickstart Q7, Q8).

### The table `feat/graphics-option-table`

- [ ] T027 [US2] [ph B1] Audit every row of R-11 in `.ai/specs/001-graphics-modes/research.md` against the code: settle each "to confirm" (`led_glow`, `led_mips` in Vanilla+; `shadow_blobs` in Vanilla; whether the `dx9_*` options act live), and list any option that acts in no mode
- [ ] T028 [US2] [ph B1] Create `crates/omsi-cfg/src/graphics_options.rs`: `GraphicsOption { key, per_mode: [Applies; 3], effect: Live | NextStart }`, `Applies = Offered | Choices(&[&str]) | Fixed { shown, reason } | NotOffered`, the `GRAPHICS_OPTIONS` table from T027 (in Vanilla the shadow options are `NotOffered` with the reason "not drawn by Vanilla yet", never "an improvement OMSI 2 does not have": OMSI 2 has dynamic shadows, research R-V V14), and `offered_in_launcher`, `offered_in_game` (offered **and** `Live`), `fit` ([data model](data-model.md)); reasons are message keys, no translated text
- [ ] T029 [P] [US2] [ph B1] Tests in `crates/omsi-cfg/tests/graphics_options.rs`: `fit` for each `Applies` kind (nearest allowed value for `Choices`, never 0 for a positive one), `offered_in_game` implies `offered_in_launcher`, `graphics` and `graphics_api` are `NextStart`
- [ ] T030 [US2] [ph B1] Make `shown()` and `api_limit()` in `crates/omsi-launcher-ui/src/window/domain/settings.rs` ask the table; delete `crates/omsi-launcher-ui/src/api_limits.rs` and the `classic` rules, and port `crates/omsi-launcher-ui/tests/api_limits.rs` to the table; presets apply only keys `Offered` or `Choices` in the current mode and are not offered where they would change fewer than two keys (FR-017)
- [ ] T031 [US2] [ph B1] Make the game apply `fit` to the settings it loads in `crates/omsi-app/src/settings.rs` (the file is not rewritten, FR-016) and make `crates/omsi-app/src/hud/implementation/lists/{pages,rows,select}.rs` ask the table instead of `limit_or` and `Backend::D3d9`; remove the `api_limits` module path from `crates/omsi-app/src/testing.rs`
- [ ] T032 [US2] [ph B1] Remove each option T027 found to act in no mode, with its key, its field in `crates/omsi-app/src/settings.rs`, its row in the launcher and the code that reads it (FR-019)
- [ ] T033 [P] [US2] [ph B1] Tests of FR-020: `crates/omsi-launcher-ui/tests/` (every key of the Graphics, Advanced graphics, VR and triple-screen groups has a table row; per mode the page's rows equal `offered_in_launcher`) and `crates/omsi-app/tests/app/settings/` (a value saved for an Enhanced-only key survives a save made in Vanilla+)
- [ ] T034 [P] [US2] [ph B1] Print the table into `docs/USER_GUIDE.md` ("Graphics options per mode") from a test or example in `crates/omsi-cfg/` so the page cannot drift; `CHANGELOG.md`

### The launcher's Graphics page `feat/launcher-graphics-mode-section`

- [ ] T035 [US2] [ph B2] In `crates/omsi-launcher-ui/src/window/domain/settings.rs` and `crates/omsi-launcher-ui/src/window/implementation/settings.rs`: the top highlighted section becomes "Graphics mode" and holds `graphics` with one line per mode and "decides which other graphics options are offered"; its text about the API limiting options goes (FR-020a)
- [ ] T036 [US2] [ph B2] Move `graphics_api` into the "Picture" group at the row `graphics` had, offered in Vanilla+ and Enhanced only (FR-020b); changing the mode redraws the page for the new mode at once
- [ ] T037 [P] [US2] [ph B2] Relabel Vanilla's own options by what they do in `crates/omsi-launcher-ui/src/window/domain/settings.rs` (no "DirectX9" in a label or hint; the keys keep their names, FR-018); add the new and changed texts to the locale files and run `.github/scripts/check_locales.py`
- [ ] T038 [P] [US2] [ph B2] Tests in `crates/omsi-launcher-ui/tests/`: the top section holds `graphics`; `graphics_api` is in "Picture" in Vanilla+ and Enhanced and absent in Vanilla; VR and triple screen absent in Vanilla; search finds only offered options ([contracts/graphics-options.md](contracts/graphics-options.md))

### The in-game menu `feat/ingame-live-options`

- [ ] T039 [US2] [ph B3] Generate the in-game graphics list in `crates/omsi-app/src/hud/implementation/lists/pages.rs` from the launcher's option list filtered by `offered_in_game(key, running mode)`, in place of the hand-written list; `graphics`, `graphics_api`, the preset and every `NextStart` option are absent; the `later` hints go (FR-020c)
- [ ] T040 [US2] [ph B3] For every `Live` row, check that its setter in `crates/omsi-app/src/hud/implementation/lists/values.rs` changes a value the frame reads (`crates/omsi-app/src/frame/implementation/app_events.rs`); correct the table's `effect` where it does not (FR-020d); a group or page left without rows is not shown
- [ ] T041 [P] [US2] [ph B3] Tests in `crates/omsi-app/tests/app/hud/`: per mode the in-game list equals `offered_in_game`; it never contains `graphics` or `graphics_api`; the list follows the running mode when `OMSI_GRAPHICS` differs from the file; `CHANGELOG.md`, `docs/USER_GUIDE.md`

### Options made live `feat/live-ssao`, `feat/live-reflections`, `feat/live-clouds`

- [ ] T042 [US2] [ph B4] `ssao`: read per frame instead of at build in `crates/omsi-render/src/renderer/{options,frame}.rs` and `crates/omsi-render/src/targets/ao.rs` (AO targets made on first use), set from `crates/omsi-app/src/hud/implementation/lists/values.rs`; row becomes `Live` in `crates/omsi-cfg/src/graphics_options.rs`; test in `crates/omsi-render/tests/live_ssao.rs` (picture after the switch equals a fresh start, both directions); frame time before/after (quickstart Q8)
- [ ] T043 [P] [US2] [ph B4] `reflections`: the same for `RenderOptions::reflections` and the puddles' read of it in `crates/omsi-render/src/renderer/frame.rs` and `crates/omsi-render/src/effects/puddles.rs`; test in `crates/omsi-render/tests/live_reflections.rs`
- [ ] T044 [P] [US2] [ph B4] `clouds`: keep the cloud layer and switch its drawing in `crates/omsi-app/src/core/implementation/make.rs` and `crates/omsi-app/src/weather/implementation/weather_setup.rs`; test in `crates/omsi-app/tests/app/weather/` or `crates/omsi-render/tests/live_clouds.rs`
- [ ] T045 [US2] [ph B4] If T042, T043 or T044 turns out to rebuild pipelines, targets of another size or textures, drop that key from the set and record why in R-12 of `.ai/specs/001-graphics-modes/research.md` instead of forcing it

**Checkpoint**: User Story 2 passes quickstart Q7 and Q8 in the three modes.

---

## Phase 5: User Story 3 - Each mode's code in its own place (Priority: P3)

**Goal**: `interface/`, `modes/vanilla_plus/`, `modes/enhanced/`, the shared core, and the Vanilla
crate laid out as domain / interface / implementation, with the import rules checked in CI.

**Independent Test**: delete one modern-API mode's directory in a scratch copy and build the other;
the layer check passes; goldens and frame times unchanged (quickstart Q9, Q10).

**Starts after T026** (FR-029): only code that stays is moved.

### Untangle first `feat/render-frame-features`, `feat/render-options-flags`, `feat/render-build-by-pass`

- [ ] T046 [US3] [ph C1] `FrameFeatures::decide(path, &RenderOptions, &FrameInputs) -> FrameFeatures`, a pure function, out of `begin_frame` in `crates/omsi-render/src/renderer/frame.rs`; `FrameInputs { view: Window | Mirror | XrEye | TriplePanel, lead_view, full_size, picture_size, rain, wetness, snow, has_glass_slot, has_reflective_material }`
- [ ] T047 [US3] [ph C1] `encode_main_pass`, `encode_enhanced_post` and `encode_glass` take `&FrameFeatures` instead of loose bools in `crates/omsi-render/src/passes/{main,post,glass}.rs` and `crates/omsi-render/src/renderer/{encode,passes}.rs`
- [ ] T048 [P] [US3] [ph C1] Unit tests without a GPU in `crates/omsi-render/tests/frame_features.rs`: each view and each path, including "a mirror in Enhanced uses the base shading"
- [ ] T049 [US3] [ph C2] Behaviour flags read in the frame (`NO_FXAA`, `NO_AO`, `NO_GLASS_PICTURE`, `NO_PUDDLE_REFLECTIONS`, `NO_SHADOWS`, `MIRROR_ENHANCED`...) become `RenderOptions` fields in `crates/omsi-render/src/renderer/options.rs`, set once by `crates/omsi-app/src/core/implementation/make.rs`
- [ ] T050 [P] [US3] [ph C2] Diagnostic flags become `DebugOptions` read when the renderer opens (`crates/omsi-render/src/renderer/open.rs`); the `OMSI_FAKE_GPU_ERROR` hooks move to a new `crates/omsi-render/src/gpu/fault.rs`; flags nothing reads are removed from `crates/omsi-cfg/src/flags.rs` and `docs/ENVIRONMENT.md`
- [ ] T051 [US3] [ph C3] Split `Renderer::build` in `crates/omsi-render/src/renderer/build.rs`: each pass builds its pipelines in its own module under `crates/omsi-render/src/passes/` (scene, corona, snow and sky pipelines are built inline today), grouped by shading path

### The interfaces `feat/render-mode-interface`

- [ ] T052 [US3] [ph C4] Create `crates/omsi-render/src/interface/mode.rs`: the trait with the steps of research R-9 (build, features for a frame, prepare, light records, scene target and pipelines for a view, after the scene) and the parts the core hands a mode
- [ ] T053 [P] [US3] [ph C4] Move `FramePlan` and its parts from `crates/omsi-render/src/renderer/mirror.rs` to `crates/omsi-render/src/interface/frame_plan.rs`; `crates/omsi-render/src/api/types.rs` keeps re-exporting the same names (`crates/omsi-render/tests/public_api.rs` unchanged)
- [ ] T054 [US3] [ph C4] Create `crates/omsi-render/src/modes/{vanilla_plus,enhanced}/mod.rs` as thin implementations that call the code where it is; `Renderer` holds one boxed mode built in `crates/omsi-render/src/renderer/build.rs`, the one function that names both; `renderer/` calls the mode instead of testing `enhanced`
- [ ] T055 [US3] [ph C4] Teach `scripts/check-render-layers.sh` the order `... effects < interface < modes/vanilla_plus | modes/enhanced < output < renderer < api` and rules 1-4 and 6 of [contracts/render-structure.md](contracts/render-structure.md); add planted-import tests under `scripts/tests/` that the script must fail on
- [ ] T056 [P] [US3] [ph C4] Measure the frame before and after the boxed mode (`profile window:` on the same scene, and the per-mode measurement); the pull request carries both (Principle VII)

### Move each mode `feat/render-mode-enhanced`, `feat/render-mode-vanilla-plus`

- [ ] T057 [US3] [ph C5] Enhanced, whole files: `passes/post.rs` → `modes/enhanced/post.rs`, `lighting/exposure.rs` → `exposure.rs`, `sky/{atmosphere,air,clouds,probe}.rs` → `atmosphere/`, `passes/fog_lamps.rs` → `fog_lamps.rs`, `targets/hdr.rs` → `targets.rs` (all under `crates/omsi-render/src/`); a file found to be used by both modes stays in the core and [contracts/render-structure.md](contracts/render-structure.md) is corrected in the same pull request
- [ ] T058 [US3] [ph C5] Enhanced, its part of the split files: the HDR frame, masked frame and lead-view updates from `renderer/{frame,encode,prepare,passes}.rs` and `passes/main.rs` into `modes/enhanced/frame.rs`; its pipelines from the pass modules into `modes/enhanced/pipelines.rs`; `linear_light_colour`, the spot profiles and `gpu_light(.., true)` from `lighting/model.rs` and `renderer/lights.rs` into `modes/enhanced/lights.rs`; its puddle branch from `effects/puddles.rs`
- [ ] T059 [US3] [ph C5] Enhanced, WGSL: `enhanced.wgsl`, `enhanced_common.wgsl`, `sky_enhanced.wgsl`, `post.wgsl`, `fog_lamps.wgsl`, `lamp_air.wgsl` to `crates/omsi-render/src/modes/enhanced/shaders/`; split `corona.wgsl` and `snow.wgsl` into a common part in the core and `fs_enhanced`, `fs_smoke_enhanced`, `fs_snow_enhanced` in the mode; the mode assembles its modules (research R-10); the assembled text and naga's output are unchanged
- [ ] T060 [US3] [ph C5] Vanilla+: its frame decisions (FXAA without MSAA, the puddles' own pass) into `crates/omsi-render/src/modes/vanilla_plus/frame.rs`, its pipelines into `pipelines.rs`, the sky from the `envir.cfg` pictures into `sky.rs`, and `sky.wgsl`, `fs_vanilla_reflections`, `fs_smoke`, `fs_snow` and the corona's plain fragment entry into `shaders/`; the base shading (`fs_main`) stays in `crates/omsi-render/src/shaders/scene/` (decision D5)
- [ ] T061 [US3] [ph C5] After both moves: no test of the shading path left in the core or in `renderer/` outside the build function (`grep -rnE "\benhanced\b" crates/omsi-render/src --include=*.rs` finds comments only outside `modes/enhanced` and `interface`); the scratch-copy test of quickstart Q10 (delete one mode, the other builds and draws)

### Build only the chosen mode `feat/render-mode-pipelines`

- [ ] T062 [US3] [ph C6] `RenderOptions::path: ShadingPath` replaces `no_enhanced` in `crates/omsi-render/src/renderer/options.rs`; `build` creates only the chosen mode; the app sets the path from `GraphicsMode` in `crates/omsi-app/src/core/implementation/make.rs`; the launcher's vehicle preview builds Vanilla+ only
- [ ] T063 [P] [US3] [ph C6] Test in `crates/omsi-render/tests/mode_build.rs`: a `ShadingPath::Linear` renderer creates no Enhanced pipeline, look-up table or probe, and the other way round (counted through `crates/omsi-render/src/testing.rs`); record the start-up time against T003 (SC-007, quickstart Q9)

### Frame stages `feat/render-frame-stages`

- [ ] T064 [US3] [ph C7] Split what is left of `begin_frame` (`crates/omsi-render/src/renderer/frame.rs`) and `encode_main_pass` (`crates/omsi-render/src/passes/main.rs`) into their stages (targets, lights and sprites, camera uniform; sky, world, cab, overlays), each a function of its own

### The Vanilla crate `feat/render-vanilla-crate`

- [ ] T065 [US3] [ph C8] Rename the crate: `git mv crates/omsi-render-d3d9 crates/omsi-render-vanilla`; update `Cargo.toml`, `crates/omsi-app/Cargo.toml`, `.github/workflows/depot-ci.yml`, `README.md`, `THIRD_PARTY.md`, `docs/QUALITY.md`, `docs/DEPENDENCY-PROVENANCE.md`, `crates/omsi-launcher-core/src/instances.rs` and every `omsi_render_d3d9` path; the `dx9_*` settings keys do not change (first pull request, rename only)
- [ ] T066 [US3] [ph C8] Lay the crate out: `scene.rs`, `ui.rs`, `caps.rs`, `render_scale.rs`, `glass_rain.rs` → `crates/omsi-render-vanilla/src/domain/`; the `RenderBackend` trait and its descriptors → `interface/input.rs`; `NullBackend` → `implementation/null.rs`; `d3d9/` → `implementation/d3d9/`; the crate's public names keep their spelling
- [ ] T067 [US3] [ph C8] `crates/omsi-app/src/output/implementation/d3d9.rs` → `vanilla.rs`, with the `FramePlan` → `SceneFrame` translation as a function of its own; `Dx9Effects` → `VanillaEffects` in `crates/omsi-app/src/output/domain/backend.rs`
- [ ] T068 [P] [US3] [ph C8] Contract test in `crates/omsi-app/tests/app/output/frame_plan_vanilla.rs`: the same `FramePlan` on `NullBackend` records the same batches, coronas, sky and lights; the constants shared by Rust and HLSL are checked there (FR-023)
- [ ] T069 [US3] [ph C8] Split `draw_scene_inner` in `crates/omsi-render-vanilla/src/implementation/d3d9/scene.rs` by pass (sky, world, smoke, coronas, glass rain)
- [ ] T070 [P] [US3] [ph C8] Extend `scripts/check-render-layers.sh` with rule 5: neither rendering crate depends on the other (read from the two `Cargo.toml`); `cargo tree -p omsi-render-vanilla` has no `omsi-render`

**Checkpoint**: User Story 3's independent test passes; SC-004 and SC-008 hold.

---

## Phase 6: User Story 4 - Enhanced keeps the sunlight dimmed by glass (Priority: P4)

**Goal**: in Enhanced, sunlight through a pane is dimmed by the glass.

**Independent Test**: the same camera inside a bus in direct sun, before and after; frame time
measured on both (quickstart Q4).

**Can start as soon as T010 is merged**; it does not wait for the other stories.

- [ ] T071 [US4] [ph A2] Failing test first in `crates/omsi-render/tests/glass_sunlight.rs` (lavapipe): an opaque panel, a pane and the ground with the sun behind them; the pane's shadow lies between the panel's and the lit ground, and a pane with a higher alpha lets less through
- [ ] T072 [US4] [ph A2] Extend the material classification (T007) with "blended, depth-checked → casts transmission (from the material's alpha and colour)" in `crates/omsi-render/src/renderer/lists.rs`; draw those materials into a transmission target beside the shadow map in `crates/omsi-render/src/passes/shadow.rs`; multiply the sun's soft shadow by it in `crates/omsi-render/src/shaders/scene/enhanced.wgsl`
- [ ] T073 [P] [US4] [ph A2] Measure Enhanced before and after (the per-mode measurement and a `profile window:` session on the same scene); same-camera screenshots of the cab in the sun; `CHANGELOG.md`; the pull request carries the numbers and the justification (Principle VII)

**Checkpoint**: User Story 4 passes; Vanilla+ golden unchanged.

---

## Phase 7: Polish & cross-cutting

- [ ] T074 [P] [ph C9] Move `crates/omsi-render/src/tests.rs`, `src/tests_units.rs` and the in-file `#[cfg(test)]` modules to `crates/omsi-render/tests/`, through `crates/omsi-render/src/testing.rs` behind `test-support`; remove the test-only globs from `crates/omsi-render/src/lib.rs`; the test count stays the same (SC-010). Best before T057 (smaller moves)
- [ ] T075 [P] [ph C9] Move `scene_tests.rs`, `ui_tests.rs`, `upscale_tests.rs` and `tests.rs` of the Vanilla crate's `d3d9/` folder to its `tests/`
- [ ] T076 [ph C10] Rewrite `crates/omsi-render/ARCHITECTURE.md`: the two new layers, the mode rules, where new code goes per mode; correct rule 8 ("unused code is not deleted") to Principle X, rule 7 (tests) to Principle VI, and the drift on `RenderOptions`
- [ ] T077 [P] [ph C10] Update `crates/omsi-render-vanilla/README.md` (it says smoke and mirrors are not drawn; they are; the crate is the Vanilla mode), `README.md`, `docs/USER_GUIDE.md` (three modes, the option table), `docs/ENVIRONMENT.md`, and the references in `docs/MACRO_REFACTOR_DRAFT.md` and `docs/OMSI_APP_REFACTOR.md`
- [ ] T078 Final measurements against T003: per-mode frame measurement (SC-006), Vanilla+ start-up (SC-007), `wc -l` of the rendering crates (SC-009); record them in `.ai/specs/001-graphics-modes/research.md`
- [ ] T079 Run every section of `.ai/specs/001-graphics-modes/quickstart.md` once more on the final `main` and tick the spec's success criteria SC-001 to SC-010 in issue #703

---

## Dependencies

```text
Setup:        T001 ─→ T016        T002 ─→ T017        T003 ─→ T063, T078
Foundational: T004..T010 (A1) ─→ T011..T015 (A3)
US1:          T015 + T016 + T017 ─→ T018..T023 (A5) ─→ T024..T026 (A6)
US2:          T015 ─→ T027..T034 (B1) ─→ T035..T038 (B2)
                                    └──→ T039..T041 (B3) ─→ T042..T045 (B4; best after T049)
US3:          T026 ─→ T046..T048 (C1) ─→ T051 (C3) ─→ T052..T056 (C4) ─→ T057..T061 (C5) ─→ T062, T063 (C6)
              T026 ─→ T049, T050 (C2)                                              └─→ T064 (C7)
              T023 + T053 ─→ T065..T070 (C8)
US4:          T010 ─→ T071..T073 (A2)
Polish:       T074, T075 any time (T074 best before T057); T076, T077 after T061 and T070; T078, T079 last
```

- US1 and US2 share only the mode type (T011-T015). Until T018 lands, the table's Vanilla column is
  applied where the Direct3D 9Ex backend is running, so US2 does not wait for US1.
- US3 waits for US1 to finish (FR-029). US4 is independent of US1-US3 after T010.
- T022 (US1) and T030 (US2) both edit the launcher's `shown()`: whichever lands second rebases.

## Parallel examples

```text
# While the maintainer does T001 and T002 in the game:
T004..T010 (remove Enhanced+)        T074, T075 (tests out of src)

# After T015 (the mode type):
US1: T018..T023 (once T016, T017 are in)     US2: T027..T034     US4: T071..T073

# Inside B4, three pull requests at once:
T042 (ssao)      T043 (reflections)      T044 (clouds)

# Inside C5, after T056:
T057..T059 (Enhanced)      then T060 (Vanilla+), which touches the same split files
```

## Implementation strategy

**MVP = User Story 1** (phases 1 to 3, T001-T026): three modes, saved settings migrated, the two
removed implementations gone. It ships on its own and already removes most of the code this feature
removes.

Then, in order of value to the player:

1. **US2** (T027-T045): the options follow the mode. B1 to B3 can ship before A5; B4 last.
2. **US4** (T071-T073): one visible Enhanced improvement, any time after T010.
3. **US3** (T046-T070): invisible to the player; one small pull request per phase, each proving
   "nothing changed" (quickstart Q10). C4 and C5 are the steps that create the per-mode directories.
4. **Polish** (T074-T079): tests out of `src/` early if convenient; documents and final numbers last.

Each pull request: `cargo test --workspace`, `cargo build --release`,
`scripts/check-render-layers.sh --strict`, the goldens, and its section of the quickstart.
