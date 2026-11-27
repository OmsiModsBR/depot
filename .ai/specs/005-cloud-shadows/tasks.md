---
description: "Tasks for cloud shadows on the scenery in Enhanced"
---

# Tasks: Cloud shadows on the scenery in Enhanced

**Input**: design documents in `.ai/specs/005-cloud-shadows/` ([plan](plan.md), [spec](spec.md),
[research](research.md), [data model](data-model.md), [quickstart](quickstart.md),
[contracts](contracts/))

**Prerequisites**: plan.md, spec.md (clarified 2026-11-27), research.md, data-model.md, contracts/

**Tests**: requested by the spec (SC-002, SC-003, SC-004, SC-007, SC-010 are checked by automated
tests; the rest in game, see [quickstart](quickstart.md)) and by the constitution (principle VI:
tests in each crate's `tests/`, never in `src/`; what a test needs from `src/` comes through the
`testing` module behind `test-support`). GPU tests run on lavapipe as the existing
`crates/omsi-render/tests/*.rs` do.

**Organization**: by user story, as Spec Kit asks. Each task also names its **delivery phase** of the
plan (`[ph N]`): the work ships as one pull request per phase on its own branch (plan, "Delivery
phases"). A phase's pull request holds every task tagged with it, whatever story the task serves.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1..US5 of the spec; setup, foundational and polish tasks carry none
- **[ph N]**: the plan's delivery phase (P1 `feat/enhanced-cloud-shade-map`, P2
  `feat/enhanced-cloud-height`, P3 `fix/enhanced-cumulus-ceu-cinza`, P4
  `feat/enhanced-cloud-shade-moon`, P5 `docs/spec-005-results`)

## Path conventions

Renderer: `crates/omsi-render/src/` (Enhanced-only code in `modes/enhanced/`, shared sky code in
`shaders/sky/` and `sky/`), tests in `crates/omsi-render/tests/`. App: `crates/omsi-app/src/`, tests
in `crates/omsi-app/tests/`.

---

## Phase 1: Setup

**Purpose**: baselines that every later measurement compares against (principle VII).

- [ ] T001 [ph 1] Record the "before" frame time on lavapipe: a `cloud_shade_bench` test-binary mode in `crates/omsi-render/tests/cloud_shade.rs` (Enhanced, 1280x720, the render-modes survey scene, `Cumulus 3`, median of 50 frames), run on `main`, number written to the P1 PR body
- [x] T002 [P] [ph 1] Ask the maintainer for the "before" in-game frame time (quickstart Q6: Desafio, MAN A23, `Cumulus 3`, 60 s, fps counter) on the build of `main` the P1 branch starts from — **done 2026-11-27**: Desafio, NEOMAN Overhaul V3 cab view, 12:00, `Cumulus 3`, frame cap off: about 242 fps (4.13 ms), 1 % low 178 fps once settled (60 s); after the P1 change the average must stay at or above about 237 fps (+2 %)

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the cloud layer values and the map resource every story reads.

**⚠️ CRITICAL**: no user story work starts before this phase is done.

- [ ] T003 [ph 1] Add the cloud layer and map fields to the Enhanced uniform (`cloud_shade`, `cloud_layer`, `cloud_light`, data-model "Uniform additions") in `crates/omsi-render/src/shaders/common/enhanced_common.wgsl` and the matching Rust struct `EnhancedUniform` in `crates/omsi-render/src/sky/update.rs` (filled in `crates/omsi-render/src/renderer/sky.rs`), keeping the uniform's alignment
- [ ] T004 [ph 1] Raise `ENHANCED_SAMPLED_TEXTURES` to 18 and add the "17 but not 18" fallback flag (map read compiled out, log once) in `crates/omsi-render/src/gpu/caps.rs` and `crates/omsi-render/src/renderer/open.rs` (research R-10, FR-014)
- [ ] T005 [ph 1] Create `crates/omsi-render/src/modes/enhanced/cloud_shade.rs`: the R16Float 1024² map + blur scratch target, its view and sampler binding, built by `EnhancedResources` (`modes/enhanced/pipelines.rs` / `mod.rs`), with `-1` (no map) as the initial `cloud_light`
- [ ] T006 [ph 1] Bind the map in the Enhanced scene shader's camera group (new binding in `crates/omsi-render/src/modes/enhanced/shaders/enhanced.wgsl` and the bind group layout that holds `t_glass`), and a `cloud_shade_at(p)` function that returns 1 when `cloud_light < 0`

**Checkpoint**: Enhanced builds and draws exactly as before (the map reads 1 everywhere).

---

## Phase 3: User Story 1 - Sun and shade patches under a cumulus sky (Priority: P1) 🎯 MVP

**Goal**: each point gets its own sun through the clouds; a cloud over the camera no longer takes the
sun from the whole scene.

**Independent test**: quickstart Q1 in game; `cloud_shade` GPU test (two points, one frame).

### Tests for User Story 1

- [ ] T007 [P] [US1] [ph 1] GPU test in `crates/omsi-render/tests/cloud_shade.rs`: a synthetic cloud field (one heap), sun at 45°, ground plane; in one frame a point under the heap gets < 10 % of the clear sun and a point in a gap > 90 %, whatever the camera's own position (camera under the heap and in the gap) (FR-001, FR-003, SC-001 automated part)
- [ ] T008 [P] [US1] [ph 1] GPU test in the same file: clouds setting off -> every point 100 % (FR-012); a device flagged "no map" (T004 fallback) -> today's camera-wide value (FR-014); `shadows` setting off -> no object shadows, but a point under a heap still gets < 10 % of the sun (spec edge case)
- [ ] T009 [P] [US1] [ph 1] GPU test in the same file: a receiver 50 m above the ground (roof) and one on the ground under the same heap both read the shade through the base-plane lookup (research R-2)

### Implementation for User Story 1

- [ ] T010 [US1] [ph 1] Write `crates/omsi-render/src/modes/enhanced/shaders/cloud_shade.wgsl` depth pass: one fullscreen fragment per texel of the base plane, optical depth along the light through the layer with `cloud_base_shape`/`cloud_sigma` from `shaders/sky/sky_light.wgsl` (no detail volume), 24 steps (research R-2, R-4)
- [ ] T011 [US1] [ph 1] Add the two blur passes to `cloud_shade.wgsl` (separable, in transmittance, radius from the uniform; P1 uses the fixed layer's height for the radius) and compute the map's mean transmittance for the border blend (research R-3, R-4)
- [ ] T012 [US1] [ph 1] Redraw policy and drift in `crates/omsi-render/src/modes/enhanced/cloud_shade.rs`: redraw on the research R-5 triggers, keep the draw's centre and drift, fill `cloud_shade`/`cloud_layer.zw` each frame with the drift since the draw
- [ ] T013 [US1] [ph 1] Encode the map passes before the main pass when a redraw is due, through a new `ModeResources::encode_before_main` hook (default no-op) in `crates/omsi-render/src/interface/mode.rs`, implemented by `EnhancedResources` in `modes/enhanced/mod.rs` and called by the core before the main pass, only when the sun is above the horizon and the clouds are on
- [ ] T014 [US1] [ph 1] In `enhanced.wgsl`, multiply the sun term by `cloud_shade_at(in.world)` wherever the sun shadow applies (`sun_shadow_soft`, `sun_shadow_hard`, rain light sun term, leaves' transmitted sun), border blend to the mean beyond 90 % of the extent (FR-001, FR-004, FR-008, FR-015)
- [ ] T015 [US1] [ph 1] Drop `* cloud_t` from the scene sun in `crates/omsi-render/src/renderer/sky.rs` (`sun:` field), keep `cloud_t` for the exposure only (research R-6)
- [ ] T016 [US1] [ph 1] Mirrors and the reflection probe: fill the same uniform fields and bind the same map for the mirror camera in `crates/omsi-render/src/renderer/mirror.rs` and the probe capture (FR-011)
- [ ] T017 [US1] [ph 1] Remove the now unused camera-only sun path (dead code after T015) per principle X; keep `cloud_sun_transmittance` for the exposure

**Checkpoint**: Q1 passes in game; Enhanced frame time measured (T001/T002 "after"), at most +2 %.

---

## Phase 4: User Story 4 - The picture under a cloud is no longer flat and bright (Priority: P2)

**Goal**: the exposure makes up only a quarter of a cloud's shade at the camera (Q1).

**Independent test**: quickstart Q4.

- [ ] T018 [P] [US4] [ph 1] Unit test in `crates/omsi-render/tests/cloud_shade.rs` (or `sky_atmosphere.rs`): with the sun share of the light at 80 %, the settled exposure under a cloud lies 1.5-1.9 stops below the clear one's picture brightness (SC-006)
- [ ] T019 [US4] [ph 1] Set `CLOUD_SHADE_ADAPT` to 0.25 in `crates/omsi-render/src/sky/air.rs` and update its doc comment (FR-010)

---

## Phase 5: User Story 3 - The cloud height shapes the shadow (Priority: P2)

**Goal**: the weather's cloud height places the clouds in the sky and in the shade, no floor; the
soft edge widens with height.

**Independent test**: quickstart Q3; edge-width GPU test.

### Tests for User Story 3

- [ ] T020 [P] [US3] [ph 2] GPU test in `crates/omsi-render/tests/cloud_shade.rs`: the same heap at base 1000 m and 3000 m; the 10-90 % edge width on flat ground at 3000 m is at least twice the width at 1000 m (SC-004, FR-007)
- [ ] T021 [P] [US3] [ph 2] Test in `crates/omsi-render/tests/sky_clouds.rs`: the drawn clouds' base follows `Lighting::cloud_height` (300 m and 3000 m); heights 0, negative and NaN fall back to 1400 m without a panic or a log at warn level (FR-006, contract)
- [ ] T022 [P] [US3] [ph 2] GPU test: a receiver above a 300 m base (a tower reaching into the layer) is shaded by its depth in the layer (research R-2, spec edge case)

### Implementation for User Story 3

- [ ] T023 [US3] [ph 2] Replace `CLOUD_BOTTOM`/`CLOUD_TOP` constants in `crates/omsi-render/src/shaders/sky/sky_light.wgsl` (and their users) with the uniform's base/top (`cloud_layer.xy`); top = base + 1400 m (research R-11)
- [ ] T024 [US3] [ph 2] Fill base/top from `Lighting::cloud_height` with the fallback in `crates/omsi-render/src/renderer/sky.rs`; pass the same base/top to `cloud_sun_transmittance` in `crates/omsi-render/src/sky/air.rs`
- [ ] T025 [US3] [ph 2] Redraw the sky cube and the map when the base changes by more than 1 m (`crates/omsi-render/src/renderer/sky.rs`, `modes/enhanced/cloud_shade.rs`)
- [ ] T026 [US3] [ph 2] Blur radius from height: `(base - ground under camera) * (0.0047 + K_SCATTER)` with `K_SCATTER = 0.01` as one named constant in `modes/enhanced/cloud_shade.rs` (research R-4)

- [ ] T043 [P] [US3] [ph 2] GPU test in `crates/omsi-render/tests/sky_clouds.rs`: base 300 m, camera at 600 m (above the layer) and at 500 m (inside it): clouds are drawn below/around the camera, and their shade on a 0 m ground matches their drawn positions (FR-005, quickstart Q9)
- [ ] T044 [US3] [ph 2] In `cloud_layer` (`crates/omsi-render/src/shaders/sky/sky_light.wgsl`), drop the early return for a camera above the base: start the march at the camera inside the layer, and march down through the layer for directions below the horizon when the camera is above it (research R-11, finding U1)

---

## Phase 6: User Story 2 - Every cloud kind shades as dark as it is opaque (Priority: P1)

**Goal**: only `Overcast`, a closed deck and precipitation grey the sky and its light in Enhanced;
the direct sun is cut only by the map (rain draws a closed deck into it); `Overcast` no longer
switches shadows off but shades through the map.

**Independent test**: quickstart Q2; `weather_lighting` test; Vanilla+ golden.

### Tests for User Story 2

- [ ] T027 [P] [US2] [ph 3] Test in `crates/omsi-app/tests/weather_lighting.rs`: `weather_lighting(.., GraphicsMode::Enhanced)` with `Cumulus 3` leaves `sun_intensity`, `sky_color`, `ambient` as with no clouds; with `Overcast` they grey as today; with Vanilla+ every value equals today's (FR-018, FR-013, SC-010)
- [ ] T028 [P] [US2] [ph 3] Same file: Enhanced keeps `lighting.shadows` on under `Overcast` and turns it off below 350 m visibility; Vanilla+ keeps today's rule (FR-009)
- [ ] T029 [P] [US2] [ph 3] GPU test in `crates/omsi-render/tests/cloud_shade.rs`: share of ground in shade over a wide plane grows `Cumulus 1` < `Cumulus 2` < `Cumulus 3`; `Overcast` shades every point to < 5 % of the sun; a shaded point gets the sky light of the same sky without sun, never less; a veil alone leaves the map at 100 % (FR-002, SC-002, SC-003)
- [ ] T030 [P] [US2] [ph 3] Run `crates/omsi-render/tests/render_modes.rs` and the offscreen golden/traces: Vanilla+ pictures and offscreen traces unchanged (SC-007)

### Implementation for User Story 2

- [ ] T031 [US2] [ph 3] In `apply_weather` (`crates/omsi-app/src/world/domain/lights.rs`), for `ShadingPath::Hdr` take the sky greying (`sky_color`, `ambient`, `secondary`, `fog_color`) and the cloud part of the gloom (`l.night`) from `smoothstep(0.85, 1.0, o)` (closed share) instead of `(o - 0.45) / 0.55`, and leave `sun_intensity` undimmed by clouds (the map removes the sun, FR-009); Linear path unchanged (research R-7 and its revision)
- [ ] T032 [US2] [ph 3] In `weather_lighting` (`crates/omsi-app/src/weather/implementation/weather_setup.rs`), for Hdr drop the `!overcast` and `closed < 0.5` shadow switches, keep the visibility term; for Hdr the model sky no longer sets `sun_intensity = 1 - closed` (research R-7 revision)
- [ ] T033 [US2] [ph 3] Check `enhanced_sky_input` (`crates/omsi-render/src/sky/air.rs`): the sky's `overcast` input is the closed share for cumulus types after T031; drop the `(1.0 - closed)` factor from `reaching` so the map alone removes the sun, also under a closed cover (FR-009); adjust the doc comments that describe the old behaviour

- [ ] T046 [P] [US2] [ph 3] Tests: in `crates/omsi-app/tests/weather_lighting.rs`, Enhanced with `Cumulus 2` and rain keeps `sun_intensity` undimmed by the rain (Vanilla+ unchanged); in `crates/omsi-render/tests/cloud_shade.rs`, `Cumulus 2` with rain 0.7 shades every ground point to < 5 % of the sun, and a veil alone dims the sun evenly once (FR-009, research R-7)
- [ ] T047 [US2] [ph 3] Draw the map with the cover `max(cover, wet_cover)` in `crates/omsi-render/src/modes/enhanced/cloud_shade.rs`, and for Hdr drop the even rain cuts of direct sun: `sun_intensity *= 1 - 0.75 * rain` in `crates/omsi-app/src/world/domain/lights.rs` and `* (1 - wet_cover)` in `enhanced_sky_input` (`crates/omsi-render/src/sky/air.rs`) (research R-7)

---

## Phase 7: User Story 5 - Clouds block the moonlight (Priority: P2)

**Goal**: the map is drawn for the moon at night; the moonlight's even dimming by the cover goes.

**Independent test**: quickstart Q5; moon case of `cloud_shade`.

- [ ] T034 [P] [US5] [ph 4] GPU test in `crates/omsi-render/tests/cloud_shade.rs` (and `moonlight.rs` regression): moon up, sun down, `Cumulus 3`: a point under a heap gets < 10 % of the moonlight, a point in a gap > 90 %; `Overcast` < 5 % everywhere (FR-016, SC-009)
- [ ] T035 [US5] [ph 4] Draw the map for the moon when `Lighting::casts_moon_shadows()` holds and the sun is down (`cloud_light = 1`), for the sun otherwise, in `modes/enhanced/cloud_shade.rs` (research R-8)
- [ ] T036 [US5] [ph 4] Multiply the moon term (`enh.moon_light` and its shadow) by `cloud_shade_at(in.world)` when `cloud_light == 1` in `enhanced.wgsl`
- [ ] T037 [US5] [ph 4] Replace `moon_disc * (1.0 - 0.9 * oc)` by plain `moon_disc` (the map removes the moonlight, a closed deck included) in `crates/omsi-render/src/sky/atmosphere.rs` (FR-017); the exposure at night reads the moon's transmittance at the camera with the same 0.25 share

---

## Phase 8: Polish & cross-cutting

- [ ] T038 [ph 1] Measure the "after" of every phase PR (T001 bench on lavapipe; maintainer's Q6) and write both numbers in the PR body; stop and report if a phase passes +2 % (SC-005)
- [ ] T039 [P] [ph 1] `CHANGELOG.md` entry per phase PR, in English, describing Depot's behaviour only (principle VIII)
- [ ] T040 [P] [ph 3] Update `docs/` text that describes the Enhanced clouds' fixed height or the `Overcast` shadow switch, if any (search `docs/` for "1400", "Overcast")
- [ ] T041 [ph 5] Run the in-game quickstart Q1-Q9 with the maintainer, record results and measurements in `plan.md` (new "Results" section) and tick SC-001..SC-010 here and in spec.md
- [ ] T042 [P] [ph 5] Open a separate `fix/` issue for the mirror `clouds.w` = cloud height vs SSAO flag found in research R-6 (not part of this feature)

- [ ] T045 [P] [ph 1] GPU test in `crates/omsi-render/tests/cloud_shade.rs`: 300 frames with a steady wind and a weather blend `Cumulus 1` -> `Cumulus 3` in the middle; no ground point's sun share changes by more than 0.1 between two consecutive frames (SC-008)

---

## Dependencies & execution order

- **Setup (T001-T002)** -> **Foundational (T003-T006)** -> stories.
- **US1 (T007-T017)** first: it is the MVP and the base of every other story.
- **US4 (T018-T019)** ships with US1 in P1 (one constant).
- **US3 (T020-T026, T043-T044)** needs US1's map (P2 after P1).
- **US2 (T027-T033, T046-T047)** needs US1: without the map, removing the cumulus' even dimming would leave the
  scene with no cloud darkening at all (P3 after P1; independent of P2).
- **US5 (T034-T037)** needs US1 (P4 after P1; independent of P2/P3).
- **Polish**: T038/T039 in every phase; T045 with P1; T041 last.

```text
Setup -> Foundational -> US1+US4 (P1) -> { US3 (P2), US2 (P3), US5 (P4) } -> Results (P5)
```

## Parallel opportunities

- T001 and T002 (different people).
- Tests of a story marked [P] (different test cases, written before the implementation).
- After P1: P2, P3 and P4 touch different files except `cloud_shade.rs`/`enhanced.wgsl`; run them as
  separate branches in order, or in parallel with a rebase on the second.

Example for US1:

```text
T007, T008, T009 (tests, in parallel)  ->  T010 -> T011 -> T012 -> T013 -> T014 -> T015 -> T016 -> T017
```

## Implementation strategy

- **MVP = P1 (US1 + US4)**: fixes the reported bug (no shadows under a cumulus sky, flat bright
  picture). Ship, measure, let the maintainer try it in game before P2.
- **Then P3 (US2)**: removes the grey cumulus sky; the most visible change after P1.
- **Then P2 (US3)** and **P4 (US5)**: height and moonlight.
- **P5**: results and SC ticks.

## Summary

| Story | Tasks | Phase PR |
| --- | --- | --- |
| Setup + Foundational | T001-T006 (6) | P1 |
| US1 | T007-T017 (11) | P1 |
| US4 | T018-T019 (2) | P1 |
| US3 | T020-T026, T043-T044 (9) | P2 |
| US2 | T027-T033, T046-T047 (9) | P3 |
| US5 | T034-T037 (4) | P4 |
| Polish | T038-T042, T045 (6) | P1-P5 |
| **Total** | **47** | |
