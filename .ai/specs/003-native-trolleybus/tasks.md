---
description: "Tasks for native trolleybus support"
---

# Tasks: Native trolleybus support

**Input**: design documents in `.ai/specs/003-native-trolleybus/` ([plan](plan.md), [spec](spec.md),
[research](research.md), [data model](data-model.md), [quickstart](quickstart.md),
[contracts](contracts/))

**Tests**: required by the constitution (Principle VI) and the spec's scenarios. Tests live in
`crates/omsi-trolley/tests/` (synthetic data) and in the touched crates' `tests/`; content tests read
`OMSI_ROOT` and skip without it.

**Organization**: by user story; each task also names its delivery phase `[ph N]` of the plan, and a
phase's pull request holds every task tagged with it.

## Format: `[ID] [P?] [Story] [ph N] Description`

---

## Phase 1: Setup

- [ ] T001 [ph 1] Create `crates/omsi-trolley` (Cargo.toml with workspace deps `omsi-map`, `omsi-geometry`, `omsi-scenery`, `omsi-cfg`, `glam`; `test-support` feature; `src/lib.rs`, `domain/`, `interface/{input,output}.rs`, `implementation/`, `testing.rs`) and add it to `[workspace] members` before `crates/omsi-sim`

## Phase 2: Foundational (wire network, convention reader) — `feat/trolley-wire-network`

- [ ] T002 [P] [ph 1] `domain/convention.rs`: `ConventionParams` (global keys of [content-convention](contracts/content-convention.md) used by phases 1–2) and `RegistryEntry` (raw `[vehicle:*]` keys)
- [ ] T003 [P] [ph 1] `implementation/convention.rs`: ini reader (sections, `key=value`, `;` comments, case-insensitive keys) returning per-entry errors; `read_convention(roots)` looks up Depot content `Plugins\` then OMSI `plugins\`
- [ ] T004 [P] [ph 1] `domain/network.rs`: `WireSpan`, `SpanEnd`, `WireJoint`, `WireObject` (`Switch{Y,BranchLeft,BranchRight}`, `Crossing`, `Marker`), `TileWires`, `WireNetwork`
- [ ] T005 [ph 1] `implementation/network.rs`: `build_tile_wires(tile, origin, params, sli_lookup)` → `TileWires` (keyword match, sampling with `SplineCurve::offset_point` at the wire offsets, mirror, object recognition)
- [ ] T006 [ph 1] `implementation/network.rs`: `WireNetwork::insert/remove_tile`, joint search (distance and parallel/anti-parallel directions, Depot's join tolerance constant), uniform grid, `segments_near`
- [ ] T007 [P] [ph 1] Tests `tests/convention.rs`: sample registry and cfg written by the test; missing file; broken entry named in the error; disabled entry
- [ ] T008 [P] [ph 1] Tests `tests/network.rs`: straight and curved spans, mirror, reversed splice joins, gap above the tolerance does not join, tile insert/remove, `segments_near`, no wire → empty network
- [ ] T009 [ph 1] `omsi-app`: build `TileWires` in tile staging on the loader thread (only when the convention was read and the setting is on); keep in the world, insert/remove with the tile; one log line per map load ("wire network: N spans, M switches")
- [ ] T010 [ph 1] `omsi-app` settings: `trolleybus=1` (settings.cfg, launcher defaults and parse, round-trip test list)
- [ ] T011 [ph 1] Content test (`OMSI_ROOT`): read the installed convention and one wired map tile; spans > 0
- [ ] T012 [ph 1] Measure map load and frame time on a map without wire and a wired map, feature on/off; `omsi-check` before/after; CHANGELOG not touched (nothing the player notices)

**Checkpoint**: wired maps log their network; every other trace identical.

## Phase 3: User Story 1 — poles on the wire (P1) — `feat/trolley-poles`

- [ ] T013 [P] [US1] [ph 2] `domain/declaration.rs` + reader: `TrolleyDeclaration` from a `RegistryEntry` and `ConventionParams`, required keys per [content-convention](contracts/content-convention.md)
- [ ] T014 [US1] [ph 2] `omsi-app` spawn: match `bus_file`/`pole_carrier_bus_file` to the player's parts; log the match or the rejection
- [ ] T015 [P] [US1] [ph 2] `domain/pole.rs`: `PoleState`, phases, inputs, outputs ([data-model](data-model.md))
- [ ] T016 [US1] [ph 2] `implementation/pole.rs`: kinematics from base to shoe in the body frame, raise/lower, capture within the capture distance, tracking along a wire and across joints, contact force, stall reset by time
- [ ] T017 [US1] [ph 2] `omsi-sim/src/vehicle/trolley.rs`: step after the coupled parts are placed and before meshes animate; carrier pose; read inputs, write outputs with `set_engine_var` ([script-variables](contracts/script-variables.md))
- [ ] T018 [P] [US1] [ph 2] Tests: capture on a straight wire, tracking through a curve with body roll, reversed splice, 1 m joint, short span, no wire on the map, same result at 20 and 200 steps per second, a vehicle without declaration untouched
- [ ] T019 [US1] [ph 2] In-game validation with the four declared vehicles (quickstart phase 2), measurements, CHANGELOG line

## Phase 4: User Story 2 — dewirement and retriever (P2) — `feat/trolley-dewire`

- [ ] T020 [US2] [ph 3] `implementation/dewire.rs`: reach, force and side-angle limits with time tolerances; free rise, pause, retriever; retract command in phases
- [ ] T021 [P] [US2] [ph 3] Tests for each reason and the retract sequence; in-game validation; CHANGELOG line

## Phase 5: User Story 3 — switches and crossings (P3) — `feat/trolley-switches`

- [ ] T022 [US3] [ph 4] `implementation/switch.rs`: entry by distance and lateral alignment, branch from demand, exit capture, consecutive switches, Y and left/right branch kinds, merging, crossings
- [ ] T023 [P] [US3] [ph 4] Tests: neighbour wire at 1.9 m not captured, two switches in a row, branch from mid-span, merge from both sides; in-game validation; CHANGELOG line

## Phase 6: script signals — `feat/trolley-script-signals`

- [ ] T024 [ph 5] Special-piece speed limit, overspeed and engine-trip variables; no-wire warning; tests; in-game validation; CHANGELOG line

## Phase 7: User Story 4 — native-script convention (P4) — `feat/trolley-native-script`

- [ ] T025 [US4] [ph 6] Adapter from the native-script convention to `WireNetwork` and `TrolleyDeclaration`; tests; in-game validation; CHANGELOG line

## Dependencies

T001 → T002–T008 → T009–T012 (phase 1 PR) → US1 (phase 2) → US2 (3) → US3 (4) → phase 5 → US4 (6).
Within a phase, tasks marked [P] touch different files and can go in parallel.

## Implementation strategy

MVP = phases 1 and 2: the poles are drawn and stay on plain wire, which already makes the existing
trolleybus content drivable on lines without switches. Each later phase adds one behaviour and ships
alone.
