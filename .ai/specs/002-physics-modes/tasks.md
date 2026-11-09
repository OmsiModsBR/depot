---
description: "Tasks for the two physics modes (OMSI Classic and Project Depot Physics)"
---

# Tasks: Two physics modes (OMSI Classic and Project Depot Physics)

**Input**: design documents in `.ai/specs/002-physics-modes/` ([plan](plan.md), [spec](spec.md),
[research](research.md), [data model](data-model.md), [quickstart](quickstart.md),
[contracts](contracts/))

**Prerequisites**: plan.md, spec.md (clarified 2026-11-09), research.md, data-model.md, contracts/

**Tests**: requested by the spec (FR-021, FR-022, SC-004) and by the constitution (Principle VI:
tests outside `src/`; a `fix/` always comes with its failing test). Every conformance test names the
equations it checks in its doc comment (`/// Checks: C-3, K-1`).

**Organization**: by user story, as Spec Kit asks. Each task also names its **delivery phase** of the
plan (`[ph N]`), because the work ships as one pull request per phase on its own branch
(`docs/physics-models`, `feat/physics-mode-type`, ...). A phase's pull request holds every task tagged
with it, whatever story the task serves.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1..US5 of the spec; setup, foundational and polish tasks carry none
- **[ph N]**: the plan's delivery phase (0-9)

## Path conventions

Rust workspace: `crates/<crate>/src/` for code, `crates/<crate>/tests/` for tests (Principle VI),
`docs/` for public documents.

---

## Phase 1: Setup (in-game checks, no code)

**Purpose**: the black-box checks the key and the Classic rules depend on (quickstart Q1-Q3), done by
the maintainer on a machine with OMSI 2. Only T001-T002 block code (the key, phase 3); T003 corrects
the Classic AI later by `fix/` (clarification 2026-11-09).

- [ ] T001 [ph 0] Q1: add `[depot_physics]` (alone, and with a stray line) to a copy of a stock bus, load and drive it in OMSI 2; record the result in #677 and in `.ai/specs/002-physics-modes/contracts/bus-key.md` ("Other readers")
- [ ] T002 [P] [ph 0] Q1 on openOMSI and each fork the maintainers name; record each result in `.ai/specs/002-physics-modes/contracts/bus-key.md`
- [ ] T003 [P] [ph 0] Q2: run the check of each KA-1..KA-11 and KA-6b row on a stock map; fill the "Status" column of R-A in `.ai/specs/002-physics-modes/research.md` with "confirmed" or "not so" and what was seen
- [ ] T004 [P] [ph 0] Q3: the Mi5 at rest on a flat stop in OMSI 2 and in Depot (Classic), each axle's `Axle_Suspension` and a side screenshot from the same camera; record the result in #677

**Checkpoint**: T001-T002 passed, or the `<name>.depot.cfg` fallback (T048) applies.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the model documents, the five test vehicles with today's physics checked against the
Classic document, the mode type, the traces per engine and the code split by engine. No behaviour
changes here: every player, AI and traffic trace stays bit-identical.

### Model documents (US5's documents, needed by every test) `docs/physics-models`

- [X] T005 [P] [ph 1] Write `docs/physics/README.md`: the two engines, how a vehicle gets one (setting, key, log line), the id scheme, how to run the conformance tests ([contracts/model-docs.md](contracts/model-docs.md))
- [X] T006 [P] [ph 1] Write `docs/physics/common-model.md` with C-1..C-11 (inputs with `.bus` key and unit, outputs, implementing module for each)
- [X] T007 [P] [ph 1] Write `docs/physics/classic-model.md`: index of the C-* it runs, K-1, KA-1..KA-11 and KA-6b as in R-A (each "validated" with its check or "to validate"), including the static sag the AI scripts read (KA-4), the detail radius about the camera and `[noDistanceCheck]` (KA-9), the "Clear the accident" button and the empty next trip (KA-6)
- [X] T008 [P] [ph 1] Write `docs/physics/depot-model.md`: index of the C-* it runs, D-1 and DA-1..DA-6 from today's `AiBody` (`crates/omsi-sim/src/ai/mod.rs`, `crates/omsi-sim/src/ai/road.rs`)
- [X] T009 [ph 1] Check provenance (Principle VIII, quickstart Q5): `grep -nE "0x[0-9a-fA-F]{5,}" docs/physics` empty, no name or threshold of the reference executable, every Classic rule marked

### Test vehicles and Classic player conformance `feat/physics-test-vehicles`

- [ ] T010 [ph 2] Add V1-V5 to `crates/omsi-sim/tests/support/vehicles.rs` as in [contracts/test-vehicles.md](contracts/test-vehicles.md) (axles, driven and steered axles, `[rot_pnt_long]`, `[cog]` y 0.3-0.8 m forward)
- [ ] T011 [ph 2] Add the closed forms the tests compare with (static share about a reference y, sag per axle, pitch at rest; turning radius; steady-turn articulation angle) in `crates/omsi-sim/tests/support/closed_form.rs`
- [ ] T012 [P] [ph 2] Static sag per axle and pitch at rest, Classic (C-3, K-1), V1-V5, in `crates/omsi-sim/tests/physics_model_classic.rs`
- [ ] T013 [P] [ph 2] Pull-away and braking below the grip, stop distance (C-4, C-5, C-8), V1-V5, in `crates/omsi-sim/tests/physics_model_classic.rs`
- [ ] T014 [P] [ph 2] Steady turn at full lock: turning radius and each steered axle's angle, tag axle opposite (C-6, C-7), V1-V5, in `crates/omsi-sim/tests/physics_model_classic.rs`
- [ ] T015 [P] [ph 2] Steady articulation angles (C-9), V3-V5, and release when the bend asks more than the grip (C-7), V1-V5, in `crates/omsi-sim/tests/physics_model_classic.rs`
- [ ] T016 [ph 2] Optional static-sag check on installed buses (the Mi5 first), skipped without `OMSI_ROOT`, in `crates/omsi-sim/tests/physics_model_classic.rs`
- [ ] T017 [ph 2] Each failing row of T012-T016 becomes a `fix/` pull request with that test failing first, or a correction of `docs/physics/` (decided in the pull request)

### Mode type, settings key and `.bus` key `feat/physics-mode-type`

- [ ] T018 [P] [ph 3] Tests for `PhysicsMode::parse`, `as_str` and `for_vehicle` (every row of [contracts/settings-cfg.md](contracts/settings-cfg.md); the key never applies to AI; no path to Classic from the key) in `crates/omsi-cfg/tests/physics_mode.rs`
- [ ] T019 [ph 3] `PhysicsMode`, `Reason`, `Driver`, `parse`, `as_str`, `for_vehicle` in `crates/omsi-cfg/src/physics.rs`, exported from `crates/omsi-cfg/src/lib.rs`
- [ ] T020 [P] [ph 3] Tests for the `physics` key: every row, round trip, no line added to a file that had none, in `crates/omsi-app/tests/app/settings/physics.rs` and `crates/omsi-launcher-core/tests/settings.rs`
- [ ] T021 [ph 3] `Settings::physics` in `crates/omsi-app/src/settings.rs`; read and write in `crates/omsi-launcher-core/src/lib.rs` (not shown in the launcher yet)
- [ ] T022 [P] [ph 3] Tests for every row of [contracts/bus-key.md](contracts/bus-key.md) (once, twice, stray lines, no `unknown_keywords` entry) in `crates/omsi-vehicle/tests/depot_physics_key.rs`
- [ ] T023 [ph 3] Read `[depot_physics]` into `Vehicle::depot_physics` in `crates/omsi-vehicle/src/vehicle.rs` (stray lines reported once, never a load failure)
- [ ] T024 [ph 3] Resolve the mode once per vehicle at spawn (front section's file decides, D4; AI ignores the key, D6), hold it in the vehicle instance and log "<vehicle>: <mode> physics (<reason>)" and the rear-section key note, in `crates/omsi-sim/src/vehicle.rs` and `crates/omsi-app/src/gameplay/implementation/spawn.rs`
- [ ] T025 [P] [ph 3] Tests of the resolution at spawn (player with key in a Classic session; AI copy of the same bus; key only in a rear section's file; setting Depot) in `crates/omsi-sim/tests/physics_mode_spawn.rs`

### Traces per engine `feat/physics-traces-per-engine`

- [ ] T026 [ph 4] Run each scenario per engine with the engine in the trace file name, in `crates/omsi-sim/tests/physics_traces.rs`, `crates/omsi-sim/tests/ai_traces.rs`, `crates/omsi-sim/tests/traffic_traces.rs`
- [ ] T027 [ph 4] Invariant I1 per engine (the static share differs) in `crates/omsi-sim/tests/physics_invariants.rs`
- [ ] T028 [ph 4] Both engines in `scripts/physics-trace-diff.sh` and in the `physics-traces` job of `.github/workflows/depot-ci.yml`

### Engine split `feat/physics-engine-split`

- [ ] T029 [ph 5] `Engine { Classic, Depot }` (`From<PhysicsMode>`) held by `RigidBody` and kept by `PhysicsState`, in `crates/omsi-sim/src/physics/domain/vehicle.rs`
- [ ] T030 [ph 5] Move the static load reference of the struts (the torque term of `wheel_contacts`, `crates/omsi-sim/src/physics/implementation/suspension.rs`) and the rest loads (`wheel_rest_loads`, `crates/omsi-sim/src/physics/implementation/spec.rs`) into `crates/omsi-sim/src/physics/implementation/classic/load_share.rs` and `crates/omsi-sim/src/physics/implementation/depot/load_share.rs`, both still today's rule, chosen by a `match` on `Engine`

### AI split `feat/ai-engine-split`

- [ ] T031 [ph 6] Move `AiBody` and `road.rs` unchanged to `crates/omsi-sim/src/ai/depot/`; add `AiMotion` with only the Depot arm built, in `crates/omsi-sim/src/ai/mod.rs`
- [ ] T032 [ph 6] Build the AI body from the vehicle's resolved mode in `crates/omsi-app/src/gameplay/implementation/traffic.rs` (Depot arm for both modes until phase 8)
- [ ] T033 [ph 6] AI static pose by engine (both still today's) in `crates/omsi-sim/src/vehicle/vehicle_type.rs`, `crates/omsi-sim/src/vehicle/visuals.rs`, `crates/omsi-sim/src/vehicle/coupled/trailer.rs`
- [ ] T034 [ph 3-6] Each structural pull request shows the physics benchmark scenes' frame time before and after, within noise (Principle VII, quickstart Q7)

**Checkpoint**: every trace identical in both engines; the mode is resolved and logged per vehicle;
nothing a player can see has changed.

---

## Phase 3: User Story 1 - Mods behave as they were tuned for OMSI 2 (Priority: P1) 🎯 MVP

**Goal**: with the default setting, every vehicle runs OMSI Classic, and the player's vehicle moves
exactly as before.

**Independent test**: the Mi5 at a flat stop with the default setting: 9.1 / 9.8 / 9.1 cm and a level
body, as in OMSI 2 (quickstart Q3).

- [ ] T035 [P] [US1] [ph 3] Test: a settings file without `physics` gives Classic for every vehicle, in `crates/omsi-app/tests/app/settings/physics.rs`
- [ ] T036 [US1] [ph 4-7] Every Classic player trace identical to `origin/main` in each phase (SC-001), checked by `scripts/physics-trace-diff.sh` in each pull request
- [ ] T037 [US1] [ph 2] The Mi5 static sag within 1 cm of OMSI 2 (SC-003): the `OMSI_ROOT` test of T016 compared with T004's numbers

**Checkpoint**: User Story 1 holds through every later phase.

---

## Phase 4: User Story 2 - The player chooses Depot physics in the launcher (Priority: P2)

**Goal**: Depot shares the player's weight about `[cog]`, and the player can choose the engine in the
launcher's "Simulation" section.

**Independent test**: choose "Project Depot Physics", start a session; the log names Depot for every
vehicle and the Mi5 stands nose down (front 12.7 cm) (quickstart Q8, Q10).

### Tests for User Story 2

- [ ] T038 [P] [US2] [ph 7] Static sag per axle and pitch at rest, Depot (C-3, D-1), V1-V5, differing from Classic by the closed form, in `crates/omsi-sim/tests/physics_model_depot.rs`
- [ ] T039 [P] [US2] [ph 7] The other player rows of the matrix (C-4..C-9) in Depot, V1-V5, in `crates/omsi-sim/tests/physics_model_depot.rs`
- [ ] T040 [P] [US2] [ph 9] Launcher test: the "Physics" list saves `physics=classic|depot` and reads it back, with the one-line notice, in `crates/omsi-launcher-ui/tests/settings.rs`

### Implementation for User Story 2

- [ ] T041 [US2] [ph 7] Depot rule (weight about `[cog]`) in `crates/omsi-sim/src/physics/implementation/depot/load_share.rs`; only the `.depot` player traces change, label `physics-delta` with the delta predicted from D-1
- [ ] T042 [US2] [ph 9] "Physics" list (OMSI Classic, Project Depot Physics) and the one-line notice in the `SIMULATION` section of `crates/omsi-launcher-ui/src/window/domain/settings.rs`
- [ ] T043 [P] [US2] [ph 9] Describe the setting in `docs/USER_GUIDE.md`

**Checkpoint**: a player can switch engine and see it in the next session (SC-008).

---

## Phase 5: User Story 3 - A modder ships a bus made for Depot physics (Priority: P2)

**Goal**: the key makes the player's vehicle Depot in a Classic session; AI copies follow the setting;
the player sees which engine a vehicle uses.

**Independent test**: a test bus with the key, setting Classic: as the player's vehicle it runs Depot,
as an AI vehicle Classic (quickstart Q10).

- [ ] T044 [P] [US3] [ph 9] Tests: the launcher's bus list marks a bus whose file has the key (`crates/omsi-launcher-ui/tests/drive.rs`); the menu's "Vehicle" list shows "Physics: <engine> (<reason>)" (`crates/omsi-app/tests/app/hud/vehicle_physics_line.rs`)
- [ ] T045 [US3] [ph 9] The notice on keyed buses in the launcher's bus list, in `crates/omsi-launcher-ui/src/window/domain/drive.rs` (and the bus scan that feeds it in `crates/omsi-launcher-core/src/lib.rs`)
- [ ] T046 [US3] [ph 9] The line "Physics: <engine> (<reason>)" for the player's vehicle in the menu's "Vehicle" list (`ListKind::Vehicle`, `crates/omsi-app/src/hud/domain/lists.rs`) (FR-008a)
- [ ] T047 [P] [US3] [ph 9] Document the key in `docs/FORMATS.md` and `docs/MODDING.md` from [contracts/bus-key.md](contracts/bus-key.md), with the list of D-* items
- [ ] T048 [US3] [ph 3] Only if T001 or T002 fails (FR-009a): read the key from `<name>.depot.cfg` next to the `.bus` file instead, in `crates/omsi-vehicle/src/vehicle.rs`, with the tests of T022 moved to that file

**Checkpoint**: keyed and plain mods share a session, each with its engine.

---

## Phase 6: User Story 4 - AI traffic behaves as in OMSI 2 (Priority: P2)

**Goal**: in OMSI Classic, a new AI body that follows KA-1..KA-11 and KA-6b; in Depot, today's AI,
unchanged. One pull request (`feat/classic-ai`, clarification 2026-11-09), so the default traffic
never runs a partial Classic AI.

**Independent test**: the same junction in OMSI 2 and in Depot Classic, side by side; then a nudge on
an AI car with the player's bus (quickstart Q9).

### Tests for User Story 4

- [ ] T049 [P] [US4] [ph 8] AI rows of the matrix in Classic on V1-V5: KA-1 rotation point within 1 cm of the path on the test bends (SC-006); KA-2/3/9/10 speed regimes, with `[noDistanceCheck]`; KA-4/7 pose from the path and the static sag the scripts read; KA-5 articulation angle (V3-V5); KA-6/6b/11 release, chain and whole-vehicle return (V1, V3, V4); KA-8 no contacts on the route (V1); in `crates/omsi-sim/tests/physics_model_classic.rs`
- [ ] T050 [P] [US4] [ph 8] DA-1..DA-6 rows on V1-V5 from today's `AiBody`, without a bit changed, in `crates/omsi-sim/tests/physics_model_depot.rs`
- [ ] T051 [P] [US4] [ph 8] Tests: the "Clear the accident" button is enabled only while a loose AI vehicle exists and clears all of them; a cleared AI bus goes back empty to its next trip; in `crates/omsi-app/tests/app/gameplay/classic_ai_clear.rs`

### Implementation for User Story 4

- [ ] T052 [P] [US4] [ph 8] Route following (KA-1) and pose from the path (KA-4) in `crates/omsi-sim/src/ai/classic/path.rs`
- [ ] T053 [P] [US4] [ph 8] Static sag the scripts read per axle, computed once at spawn about the origin (FR-015, KA-4), in `crates/omsi-sim/src/ai/classic/pose.rs`
- [ ] T054 [P] [US4] [ph 8] Speed in two regimes by the detail radius about the camera, `[noDistanceCheck]`, the stop exception and grip (KA-2, KA-3, KA-9, KA-10) in `crates/omsi-sim/src/ai/classic/speed.rs`
- [ ] T055 [P] [US4] [ph 8] Rear sections as kinematic trailers, not updated far away (KA-5) in `crates/omsi-sim/src/ai/classic/trailer.rs`
- [ ] T056 [US4] [ph 8] Release and return of the whole vehicle (KA-6, KA-6b, KA-7, KA-11): the player's `RigidBody` (Classic) with throttle 0, brake 1 and hazard lights, from the route poses without a jump; back to a route when cleared, in `crates/omsi-sim/src/ai/classic/loose.rs`
- [ ] T057 [US4] [ph 8] Contacts (KA-8): no AI-AI or AI-scenery contacts on the route, a loose vehicle against everything, in `crates/omsi-sim/src/physics/implementation/collide.rs` and `crates/omsi-app/src/gameplay/implementation/traffic.rs`
- [ ] T058 [US4] [ph 8] `AiMotion::Classic` built for Classic road vehicles; a cleared AI bus goes back empty and starts its next trip, in `crates/omsi-sim/src/ai/mod.rs` and `crates/omsi-app/src/gameplay/implementation/traffic.rs`
- [ ] T059 [US4] [ph 8] Enable the "Clear the accident" button of the Alt menu in `crates/omsi-app/src/ui/domain/alt.rs` and its action in `crates/omsi-app/src/input/implementation/actions.rs`
- [ ] T060 [US4] [ph 8] Cost (R-P, SC-007): AI step per vehicle and frame on 40 AI vehicles, Classic against `AiBody`, and the frame time with 10 loose AI vehicles, reported in the pull request
- [ ] T061 [US4] [ph 8] Classic AI traces change (label `physics-delta`), Depot AI traces identical (SC-002)

**Checkpoint**: Classic traffic as in OMSI 2 for every row built; Depot traffic as before.

---

## Phase 7: User Story 5 - Maintainers and modders can read and check each physics (Priority: P3)

**Goal**: every equation of both documents is checked by a test, and the matrix is complete.

**Independent test**: pick an equation, find the tests that name it, run them.

- [ ] T062 [US5] [ph 8] Coverage check: every C-*, K-*, KA-*, D-*, DA-* id in `docs/physics/` is named by at least one test, and every cell of the matrix in [contracts/test-vehicles.md](contracts/test-vehicles.md) has its test (SC-004), in `scripts/physics-model-coverage.sh`
- [ ] T063 [US5] [ph 8] Update the status of each Classic rule in `docs/physics/classic-model.md` from T003's results; each "not so" row becomes a `fix/` pull request

---

## Phase 8: Polish and cross-cutting

- [ ] T064 [ph 9] `CHANGELOG.md` line under **Unreleased** (quickstart Q10)
- [ ] T065 [P] [ph 9] Run quickstart Q7-Q10 end to end and record the results in the phase 9 pull request
- [ ] T066 [P] [ph 9] Run `cargo test --workspace`, `cargo fmt --check` and clippy on the final state

---

## Dependencies and execution order

### Phases

- **Setup (T001-T004)**: no dependency; only T001-T002 block the key (T022-T025, T048).
- **Foundational**: plan phases 1 → 2 → 3 → 4 → 5 → 6 in order; T005-T008 in parallel; the tests
  T018, T020, T022 before T019, T021, T023.
- **US1**: runs with the foundational phases (its check is "nothing changes").
- **US2**: phase 7 after phases 2 and 5; phase 9 after phases 7 and 8.
- **US3**: the key is read in phase 3; what the player sees comes in phase 9.
- **US4**: phase 8 after phase 6; it does not wait for T003 (clarification 2026-11-09).
- **US5**: T062 once phases 7 and 8 are in; T063 whenever T003 reports.
- **Polish**: with phase 9.

### Order of the pull requests

```text
0 (checks) ── T001-T002 gate phase 3
1 docs → 2 test vehicles → 3 mode type → 4 traces → 5 engine split → 6 AI split ─┬→ 7 Depot [cog] ─┐
                                                                                └→ 8 Classic AI ──┴→ 9 setting + docs
```

### Parallel opportunities

- T005-T008 (four documents).
- T012-T015 (independent test functions of the Classic matrix).
- T018, T020, T022 (tests in three crates).
- Phases 7 and 8 after phase 6.
- Inside phase 8: T049-T051 (tests) together; T052-T055 in separate files.

### Example: phase 8

```text
Tests first:  T049 (Classic AI rows), T050 (Depot AI rows), T051 (clear button, next trip)
Then:         T052 path.rs, T053 pose.rs, T054 speed.rs, T055 trailer.rs (parallel)
Then:         T056 loose.rs → T057 contacts → T058 wiring → T059 button
Last:         T060 cost, T061 traces
```

## Implementation strategy

- **MVP**: Setup T001-T002 plus Foundational plus US1. The game is unchanged for players, the code is
  split by engine, and every later change has a fixed reference.
- **Then**: US2's phase 7 and US4's phase 8 in parallel; then phase 9 (US2, US3, polish), the only
  phase that lets a player choose Depot.
- Every phase is one pull request on its own branch; a mismatch with the model found on the way is a
  `fix/` with its failing test, never folded silently into a `feat/`.
