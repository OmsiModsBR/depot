---
description: "Tasks for steerable axles in Project Depot Physics"
---

# Tasks: Steerable axles in Project Depot Physics

**Input**: design documents in `.ai/specs/004-steerable-axles/` ([plan](plan.md), [spec](spec.md),
[research](research.md), [data model](data-model.md), [quickstart](quickstart.md),
[contracts](contracts/))

**Prerequisites**: plan.md, spec.md (clarified 2026-11-21), research.md, data-model.md, contracts/

**Tests**: requested by the spec (SC-001 to SC-006a are automated tests) and by the constitution
(Principle VI: tests in each crate's `tests/`, never in `src/`; what a test needs from `src/` comes
through the `testing` module behind `test-support`). Every model test names the equations it checks
in its doc comment (`/// Checks: D-5`).

**Organization**: by user story, as Spec Kit asks. Each task also names its **delivery phase** of the
plan (`[ph N]`): the work ships as one pull request per phase on its own branch
(`docs/steer-axles-model`, `feat/steer-axle-key`, ...). A phase's pull request holds every task tagged
with it, whatever story the task serves.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: can run in parallel (different files, no dependency on an unfinished task)
- **[Story]**: US1..US3 of the spec; setup, foundational and polish tasks carry none
- **[ph N]**: the plan's delivery phase (0-6)

## Path conventions

Rust workspace: `crates/<crate>/src/` for code, `crates/<crate>/tests/` for tests, `docs/` for public
documents. Physics paths below are under `crates/omsi-sim/src/physics/`.

---

## Phase 1: Setup (in-game check, no code)

**Purpose**: the black-box check the `.bus` parameters depend on (FR-002, quickstart "In OMSI 2").

- [x] T001 [ph 0] Check in OMSI 2 that a stock 3-axle bus with `achse_steer_angle` and
  `achse_steer_centring` added inside its third `[newachse]` (also as the axle's first and last
  parameters) loads and drives exactly as the unmodified copy (done by the maintainer, 2026-11-21:
  OMSI 2 ignores them)

**Checkpoint**: the parameters stay inside `[newachse]`; no side file.

---

## Phase 2: Foundational (blocking prerequisites)

**Purpose**: the model document, the parameters read from the `.bus` file and the domain types every
story uses. No physics behaviour changes; every trace stays identical.

### Model document (docs only)

- [x] T002 [ph 1] Write `D-5` "Steerable axles and the turning line" in `docs/physics/depot-model.md`:
  inputs (`achse_steer_angle`, `achse_steer_centring`, `achse_long`, `D-1` loads, `[inv_min_turnradius]`,
  `[rot_pnt_long]`), the weight `w_i = F_i · r_i` (rigid 1, steerable `1 − e_i`, mode 2 counted as
  rigid), `y_turn`, the `C-6` fallback with no weight, `lock_front` from `C-6` and
  `κ_max = tan(lock_front) / |y_front − y_turn|`; closed forms for V2 (tag steering, tag locked) and
  V5's rear section (fourth axle steering, rigid); index row; status "checked by the conformance
  tests" (research R-4, R-5)
- [x] T003 [ph 1] Write `D-6` "Modes of a steerable axle" in `docs/physics/depot-model.md`: mode 0
  `δ = σ · s · lock` with `σ` −1 behind the turning line, +1 ahead, 0 on it; `s = κ / κ_max` in the
  leading section, `s_f = clamp(κ_joint / κ_max, −1, 1)` behind a joint; mode 1 straight; mode 2 the
  script's angle clamped to the lock; engagement `e` ramping at `centring / lock`, angle at most
  `centring` per second; no locking rule in the game (research R-6)
- [x] T004 [ph 1] Write `D-7` "Scrub" in `docs/physics/depot-model.md`: tyres off the centre by more
  than `SCRUB_TOLERANCE` held with the road's reach, sections with steerable axles behind a joint held at each tyre,
  their force in `D-3`; basis *physics, tuned*; risk and fallback of research R-7
- [x] T005 [P] [ph 1] Point `C-6`, `C-7` and `C-9` of `docs/physics/common-model.md` to `D-5`..`D-7` for
  sections with steerable axles, and update the `C-7` note "the steering of a rear axle's tyres is
  modelled in neither engine" to say Project Depot Physics models it now
- [x] T006 [P] [ph 1] Document the two axle parameters (contracts/bus-key.md) and the variables
  `Axle_Steer_Mode_<n>`, `Axle_Steer_Angle_<n>`, `Axle_Steer_Locked_<n>` and the meaning of
  `Axle_Steering_<n>` (contracts/script-vars.md) in `docs/FORMATS.md`

### Parameters in the `.bus` file

- [ ] T007 [P] [ph 2] Write the parser tests in `crates/omsi-vehicle/tests/steer_axle_key.rs`: both
  parameters read in any order and position inside `[newachse]`; a partial set gives `steer = None`
  and a log line naming the missing one; `achse_steer_angle` "finite, `> 0`, `< 90`";
  `achse_steer_centring` "finite, `> 0`"; not a number; a parameter given twice (second ignored); the
  keyword-less axle form never carries them; a file without them parses exactly as before
- [ ] T008 [ph 2] Add `AxleSteer { lock_deg, centring_deg_s }` and `Axle::steer: Option<AxleSteer>` in
  `crates/omsi-vehicle/src/vehicle.rs`, read `achse_steer_angle` and `achse_steer_centring` in
  `parse_axle`, validate per data-model.md, report invalid sets with the file and axle (never correct
  them), make T007 pass
- [ ] T009 [ph 2] At spawn, report and ignore a declaration on the leading section's front axle, and
  report once per vehicle type per session that the parameters are ignored when the vehicle runs OMSI
  Classic or is an AI copy (never once per AI spawn), in
  `crates/omsi-sim/src/vehicle/physics_mode.rs` (or where the engine is chosen at spawn)

### Domain types

- [ ] T010 [ph 2] Add `SteerAxle { axle, lock, centring }` (SI units) to the section's `PhysicsSpec` in
  `domain/spec.rs`, built in `implementation/spec.rs` only for `Engine::Depot`
- [ ] T011 [ph 2] Add `domain/steering.rs` with `SteerMode { Basic, Locked, ScriptAngle }` (0, 1, 2; any
  other value is `Basic`) and `SteerState { angle, engagement, mode, script_angle }`, held per steerable
  axle by `RigidBody` (`domain/body.rs`), started at `angle = 0`, `engagement = 1`; re-export through
  `physics/mod.rs`; expose it to tests in `crates/omsi-sim/src/testing.rs`
- [ ] T012 [ph 2] Add `SCRUB_TOLERANCE` to `domain/params.rs`, tagged *tuned* with its origin
- [ ] T013 [ph 2] Run `scripts/physics-trace-diff.sh origin/main`: every trace identical (SC-002)

**Checkpoint**: `cargo test --workspace` green, traces identical, nothing the player notices.

---

## Phase 3: User Story 1 - A steered tag axle makes the bus turn tighter (Priority: P1) 🎯 MVP

**Goal**: in a section with declarations, rigid axles, mode 0 steering, the turning line from the
non-steering axles, the lock that follows it, real angles to the scripts, and scrub.

**Independent Test**: V2 with a steerable tag axle, steady full-lock circle at 5 km/h with the tag axle steering and
declared rigid; radius and angles against the closed forms of `D-5`/`D-6` (SC-001); drive needed at
full lock (SC-006).

### Tests for User Story 1

- [ ] T014 [P] [US1] [ph 3] Add variants of V2 with steerable axles (tag axle declared, and no axle declared apart
  from the front one) and an 8x2 front section with its second axle declared, in
  `crates/omsi-sim/tests/support/vehicles.rs`
- [ ] T015 [P] [US1] [ph 3] Unit tests of the pure functions (turning line, `κ_max`, mode 0 angle with
  `σ`) in `crates/omsi-sim/tests/steer_axles.rs` (`/// Checks: D-5, D-6`)
- [ ] T016 [P] [US1] [ph 3] Closed-form tests in `crates/omsi-sim/tests/physics_model_depot.rs`: V2
  radius with the tag steering and rigid within 2 % (SC-001); tag angle `−s · lock` at half and full
  steering; 8x2 second axle turning the same way as the front; `Axle_Steering` 0 for rigid axles
- [ ] T017 [P] [US1] [ph 3] Scrub test in `crates/omsi-sim/tests/steer_axles.rs`: at full lock and
  constant speed V2 with both rear axles rigid needs more drive than with the tag axle steering
  (SC-006)
- [ ] T017a [P] [US1] [ph 3] Sliding test in `crates/omsi-sim/tests/steer_axles.rs`: V2 pushed past
  the tyres' hold, the yaw differs with the tag axle steering and rigid, each tyre's force using its
  real angle (FR-011)
- [ ] T017b [P] [US1] [ph 3] Classic identity test in `crates/omsi-sim/tests/physics_traces.rs`: V2 with
  the parameters under OMSI Classic gives the same trace bit for bit as V2 without them (FR-004)
- [ ] T017c [P] [US1] [ph 3] Edge tests: a section whose axles are all steerable turns about its
  `[rot_pnt_long]` line (`C-6`) with one log line, in `crates/omsi-sim/tests/steer_axles.rs`; a
  declaration on the leading section's front axle is reported and ignored at spawn (T009), in
  `crates/omsi-sim/tests/physics_mode_spawn.rs` (SC-005)
- [ ] T018 [P] [US1] [ph 3] New scenarios with steerable axles in `crates/omsi-sim/tests/physics_traces.rs`; old ones
  unchanged

### Implementation for User Story 1

- [ ] T019 [US1] [ph 3] Write `implementation/depot/steering.rs`: `turn_line` (research R-4),
  `kappa_max` (R-5) and the mode 0 target `σ · s · lock` (R-6), as pure functions
- [ ] T020 [US1] [ph 3] Add the rule `Engine::steering` in `implementation/engine.rs`: Classic and
  sections without `SteerAxle` run `C-6` unchanged; Depot sections with `SteerAxle` run
  `depot::steering`
- [ ] T021 [US1] [ph 3] Use the current `κ_max` for the leading section's `κ = s · κ_max` in
  `implementation/train.rs`, and make `steer_to` in `implementation/body.rs` set rigid axles to 0,
  steerable axles to their angle (rate-limited to `centring`), the front axle as `C-6`, tyres with
  Ackermann about their axle's angle
- [ ] T022 [US1] [ph 3] Make `axle_steer` in `implementation/body.rs` return each axle's actual angle
  (0 for rigid axles of a section with steerable axles), so `Axle_Steering_<n>_L/R` carry it through
  `vehicle/physics_bridge.rs`
- [ ] T023 [US1] [ph 3] Scrub in `implementation/tyre.rs` (`lateral_start`): while holding, tyres off
  the centre by more than `SCRUB_TOLERANCE` get the road's reach, and their force enters `D-3` with
  the others (research R-7)
- [ ] T024 [US1] [ph 3] Run the trace diff: only the new scenarios with steerable axles differ; label the PR
  `physics-delta` with the public justification

- [ ] T024a [US1] [ph 3] Add the player-facing line to **Unreleased** in `CHANGELOG.md` (buses whose
  files declare steerable axles turn with them in Project Depot Physics) (Principle IX)

**Checkpoint**: a 6x2 with a steerable tag axle turns tighter with its tag axle steering; nothing else changes.

---

## Phase 4: User Story 2 - Each vehicle's script locks and steers the axle (Priority: P1)

**Goal**: modes 1 and 2 from `Axle_Steer_Mode_<n>`, the engagement and centring ramps, a continuous
turning line, the locked flag, and the state saved with the situation.

**Independent Test**: a test script on V2 locks the tag axle above 30 km/h and unlocks it below; S-bend,
accelerate, slow down; angle, turning line and yaw rate over time (SC-003); then each mode in a steady
turn (SC-004).

### Tests for User Story 2

- [ ] T025 [P] [US2] [ph 4] Ramp and continuity tests in `crates/omsi-sim/tests/steer_axles.rs`:
  locking and unlocking by mode in a steady turn at 25 km/h never makes the yaw rate jump by more than
  2 % between two frames (SC-003); engagement at `centring / lock` per second; angle at most `centring`
  per second
- [ ] T026 [P] [US2] [ph 4] Script-mode tests in `crates/omsi-sim/tests/steer_axles.rs` with a small
  test script: modes 0, 1, 2 take effect in the first physics step after being written (SC-004);
  mode 2 angle clamped to the lock; unknown mode and not-a-number angle behave as mode 0 with one log
  line per vehicle (SC-005); `Axle_Steer_Locked_<n>` 1 only when the axle is straight in mode 1;
  mode 2 counts as rigid for the turning line
- [ ] T027 [P] [US2] [ph 4] Situation test: a saved situation restores each steerable axle's angle and
  engagement; an old situation starts at `angle = 0`, `engagement = 1`, in
  `crates/omsi-sim/tests/steer_axles.rs`

### Implementation for User Story 2

- [ ] T028 [US2] [ph 4] Engagement and angle ramps for all modes in `implementation/depot/steering.rs`
  (research R-6), and the weight `1 − e_i` in `turn_line`
- [ ] T029 [US2] [ph 4] Read `Axle_Steer_Mode_<n>` and `Axle_Steer_Angle_<n>` once per frame before the
  physics step and write `Axle_Steer_Locked_<n>`, numbered over the whole vehicle, in
  `vehicle/physics_bridge.rs` and `vehicle.rs` (variable ids like `v_springfactor`); match the names
  whole, never by prefix (`Axle_Steer_` is a prefix of `Axle_Steering_`, see `vehicle/builtins.rs`)
- [ ] T030 [US2] [ph 4] Save and restore `SteerState` with the situation in `vehicle.rs` /
  `vehicle/motion.rs`
- [ ] T031 [US2] [ph 4] Add the player-facing line to **Unreleased** in `CHANGELOG.md`; run the trace
  diff; label `physics-delta`

**Checkpoint**: a script can lock the tag axle at speed with no jerk and steer it by its own law.

---

## Phase 5: User Story 3 - A rear section with a steered axle follows tighter (Priority: P2)

**Goal**: the same on any section behind a joint: its own turning line, mode 0 from the joint's bend,
steering tyres on its steerable axles, held at each tyre.

**Independent Test**: V5 steady turn at walking speed with the fourth axle steering and rigid;
articulation angle and swept path against the closed forms (SC-006a).

### Tests for User Story 3

- [ ] T032 [P] [US3] [ph 5] V5 with its fourth axle declared steerable and V4 (a rear axle declared) in
  `crates/omsi-sim/tests/support/vehicles.rs`
- [ ] T033 [P] [US3] [ph 5] Closed-form tests in `crates/omsi-sim/tests/physics_model_depot.rs`: V5
  articulation angle with the fourth axle steering and rigid within 2 %, narrower swept path with it
  steering (SC-006a); a rear section without declarations identical to today (`C-9`); with the joint
  straight the rear axles stand straight whatever the steering wheel does; mode 1 on the rear axle
  moves the line continuously
- [ ] T034 [P] [US3] [ph 5] New articulated scenarios with steerable axles in `crates/omsi-sim/tests/physics_traces.rs`

### Implementation for User Story 3

- [ ] T035 [US3] [ph 5] In `implementation/train.rs`, give a section with steerable axles behind a joint its own
  `y_turn` and measure the joint's arm from it (`Joint::follower_kappa`, `implementation/engine.rs`);
  mode 0 uses `s_f = clamp(κ_joint / κ_max, −1, 1)` with the leading section's current `κ_max`
- [ ] T036 [US3] [ph 5] In `implementation/body.rs` (`steer_to`), let the steerable axles of a
  follower steer their tyres (rigid ones stay square), and in `implementation/tyre.rs`
  (`lateral_start`) hold the tyres of such a follower at each tyre with the road's reach instead of on the
  projected line (research R-8)
- [ ] T037 [US3] [ph 5] Map each section's local axle to the vehicle-wide `<n>` for the three script
  variables in `vehicle/coupled/articulation.rs` and `vehicle/coupled/trailer.rs`
- [ ] T038 [US3] [ph 5] Run the trace diff; label `physics-delta`

- [ ] T038a [US3] [ph 5] Add the player-facing line to **Unreleased** in `CHANGELOG.md` (steerable
  axles on rear sections of articulated buses) (Principle IX)

**Checkpoint**: an articulated bus with a steered last axle cuts the corner less.

---

## Phase 6: Polish & cross-cutting concerns

- [ ] T039 [ph 5] Measure frame time of V2/V5 with steerable axles against copies without the parameters, same machine and
  scenario (`cargo run -p omsi-sim --example handling` and in game), and put the numbers in the phase 5
  pull request (SC-007, Principle VII)
- [ ] T040 [P] [ph 5] Tune `SCRUB_TOLERANCE` on V2 so SC-006 holds without a kick at the threshold;
  record the value's origin in `domain/params.rs` and `D-7`
- [ ] T041 [ph 6] Maintainer in game: 6x2 with a steerable tag axle (tighter turn, script lock at speed, mode 2) and
  articulated bus with a steered last axle (SC-008); findings go to `fix/` pull requests with their
  failing test
- [ ] T042 [ph 6] Update the statuses in `docs/physics/depot-model.md` (`D-5`..`D-7`) after T041, and remove their "Planned" notes

---

## Dependencies

- **Setup (T001)**: done.
- **Foundational**: T002-T006 (ph 1, docs) and T007-T013 (ph 2) block every story. T007 before T008.
- **US1 (ph 3)**: needs ph 2. T014-T018 can be written first in parallel; T019 before T020-T023.
- **US2 (ph 4)**: needs US1 (it changes the same steering code and the line's weights).
- **US3 (ph 5)**: needs US1; independent of US2 except that mode 1 in T033 uses T028.
- **Polish**: T039-T040 with ph 5; T041-T042 after ph 5 merges.

## Parallel examples

- ph 1: T005 and T006 alongside T002-T004.
- ph 2: T007 (tests) with T010-T012 (domain types, other crate).
- US1: T014, T015, T016, T017, T017a-c, T018 together before T019.
- US2: T025, T026, T027 together before T028.
- US3: T032, T033, T034 together before T035.

## Implementation strategy

1. MVP = ph 1 to ph 3 (US1): a 6x2-4 steers its tag axle by mode 0 and turns tighter.
2. ph 4 (US2) adds what makes it usable on real buses: the script locks it at speed.
3. ph 5 (US3) extends it to articulated buses.
4. Each phase is one pull request; each keeps `cargo test --workspace` green and the traces of
   vehicles without the parameters identical.
