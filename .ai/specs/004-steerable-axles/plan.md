# Implementation Plan: Steerable axles in Project Depot Physics

**Branch**: one per delivery phase (see "Delivery phases") | **Date**: 2026-11-21 |
**Spec**: [spec.md](spec.md)

**Input**: the maintainer's request (2026-11-21): "steerable axles configurable in the physics, with
deactivation and steering logic that scripts can change; steerable axles take part in the bus's
turning physics", and the decisions of the same day: the turning line follows from the axles that do
not steer (Q1), any axle of any section can be declared steerable (Q2), and the declaration is new
parameters of each `[newachse]`, one per value (Q3).

**Execution notes**: `.ai/.specify/extensions.yml` does not exist, so no hooks ran. The feature
directory was created by hand (the setup script was not run in the planning environment). Code read
at `e7d6830` (2026-11-21).

## Summary

Today every axle of a section points at the centre of the turn on the `[rot_pnt_long]` line (`C-6`);
none is rigid, none can lock, and the scripts only read the angles. This plan adds, in Project Depot
Physics only:

1. two new axle parameters inside `[newachse]`, one value each (`achse_steer_angle`,
   `achse_steer_centring`), that declare that axle steerable
   ([contracts/bus-key.md](contracts/bus-key.md));
2. in a section with a declaration, rigid axles for every other axle (the leading section's front
   axle stays the driver's), and a turning line computed each substep from the axles that do not
   steer, weighted by their static load ([research.md](research.md) R-4);
3. the front wheels' lock kept as `C-6` gives it, so the turning circle follows what the steerable
   axles do (R-5);
4. per steerable axle, three modes chosen by the vehicle's script: 0 basic (in proportion to
   the steering axle and the axle's lock: opposite behind the turning line, as a 6x2-4, the same
   way ahead of it, as a twin steer), 1 locked straight, 2 script angle,
   with rate-limited changes (R-6); no locking rule in the game ([contracts/script-vars.md](contracts/script-vars.md), R-9);
5. scrub of tyres that do not point at the centre, through the road's reach in the existing lateral
   solver and `D-3` (R-7);
6. the same on any section behind a joint, on the curvature the joint gives it (R-8).

OMSI Classic, the AI and every section without a declaration run exactly as today (traces identical).

## Technical Context

**Language/Version**: Rust (workspace edition and toolchain as pinned)
**Primary Dependencies**: `glam` (already used); no new crate
**Storage**: `.bus` files (read only); the situation file keeps each steerable axle's angle and
engagement
**Testing**: `cargo test --workspace`; new tests in `crates/omsi-vehicle/tests/` (key parsing) and
`crates/omsi-sim/tests/` (`physics_model_depot.rs` closed forms, `physics_traces.rs` new keyed
scenarios, script-mode tests); test vehicles V2 and V5 of spec 002 get keyed variants
**Target Platform**: Windows x64
**Project Type**: game (Cargo workspace)
**Performance Goals**: no measurable frame-time change for keyed vehicles (SC-007); unkeyed
vehicles unchanged by construction
**Constraints**: physics traces of every existing scenario bit-identical (SC-002); no crash on
invalid content; nothing from the reference executable in public text
**Scale/Scope**: up to 5 sections and ~10 axles per vehicle; 3 script variables per steerable axle

## Constitution Check

*Gate before Phase 0 research; re-checked after Phase 1 design (below).*

| Principle | Check | Result |
|---|---|---|
| I. Own Path | Depot-only feature, no upstream tie | Pass |
| II. Fidelity only with validation | No "as OMSI 2" claim; only the key's harmlessness in OMSI 2 needed a game check (T001, passed 2026-11-21); keyed changes carry `physics-delta`, unkeyed traces identical | Pass |
| III. No compensation | Declarations read as given; invalid ones reported and ignored, not corrected; all four values required, no invented default | Pass |
| IV. No crash | Parser and script inputs validated; bad values logged, game continues (FR-005, SC-005) | Pass |
| V. Domains as packages | Domain objects in `physics/domain`, rules in `physics/implementation/depot/steering.rs`, chosen in `engine.rs`; scripts reach physics only through `physics_bridge.rs` | Pass |
| VI. Tests outside `src/` | New tests in `tests/` folders; what they need from `src/` via `testing` behind `test-support` | Pass |
| VII. Performance | New per-substep work is a handful of arithmetic per axle; measured before/after (SC-007) | Pass (feature exception, with measurements) |
| VIII. Provenance | Static-analysis reading used only to confirm OMSI 2 has no per-axle setting; nothing of it in public text | Pass |
| IX. Small changes | Six phases, five of them PRs, `feat/`/`docs/` prefixes | Pass |
| X. Lean code | One rule method, no trait (single implementation), no speculative options (no tyre model, no AI) | Pass |

## Project Structure

### Documentation (this feature)

```text
.ai/specs/004-steerable-axles/
├── spec.md
├── plan.md              # this file
├── research.md          # R-1..R-11
├── data-model.md
├── quickstart.md
├── contracts/
│   ├── bus-key.md       # achse_steer_* in [newachse]
│   └── script-vars.md   # Axle_Steer_* and Axle_Steering
└── checklists/
    └── requirements.md
```

### Source Code (repository root)

```text
crates/omsi-vehicle/
├── src/vehicle.rs                       # parse_axle: achse_steer_* -> Axle::steer
└── tests/steer_axle_key.rs              # new: valid, missing values, out of range, duplicate

crates/omsi-sim/
├── src/physics/domain/spec.rs           # SteerAxle in PhysicsSpec (SI units)
├── src/physics/domain/steering.rs       # new: SteerState, SteerMode
├── src/physics/domain/params.rs         # SCRUB_TOLERANCE (tuned)
├── src/physics/implementation/spec.rs   # declarations -> SteerAxle, per section
├── src/physics/implementation/depot/steering.rs  # new: turn line, lock, law, ramps
├── src/physics/implementation/engine.rs # rule: C-6 or Depot steering
├── src/physics/implementation/body.rs   # steer_to / axle_steer use the rule
├── src/physics/implementation/tyre.rs   # reach of scrubbing tyres; keyed followers held at tyre
├── src/physics/implementation/train.rs  # kappa from kappa_max of the current line
├── src/vehicle/physics_bridge.rs        # Axle_Steer_* in and out
├── src/vehicle/coupled/articulation.rs  # section axle numbers for the variables
├── src/vehicle.rs / motion.rs           # situation save/restore of SteerState
├── src/testing.rs                       # test-support access to SteerState
└── tests/
    ├── physics_model_depot.rs           # closed forms: V2 keyed, V5 keyed (SC-001, SC-006a)
    ├── steer_axles.rs                   # new: ramps, continuity, modes, scrub (SC-003..006)
    ├── physics_traces.rs                # new keyed scenarios; old ones identical
    └── support/                         # V2/V5 keyed variants

docs/physics/depot-model.md              # D-5 steerable axles and turning line, D-6 scrub, D-7 logic
docs/physics/common-model.md             # C-6, C-7, C-9 point to D-5..D-7 for keyed sections
docs/FORMATS.md                          # the key and the script variables
CHANGELOG.md                             # Unreleased line in the phase the player notices it
```

**Structure Decision**: the existing physics package; one new domain file and one new Depot rule
file, no new crate.

## Design

### Turning, per section and substep (data in [data-model.md](data-model.md))

1. If the section has no `SteerAxle`, run `C-6` / `C-9` as today and stop.
2. Update each steerable axle's mode from its script variables (once per frame) and its engagement
   `e_i` towards its target (1 in mode 0, 0 in modes 1 and 2) at `centring_i / lock_i` per second
   (R-6).
3. `y_turn` from the rigid axles and `1 − e_i` of the steerable ones, weighted by static load; with
   no weight, the `C-6` line (R-4).
4. Leading section: `κ = s · tan(lock_front) / |y_front − y_turn|` (R-5). Section behind: the joint's
   curvature, its `arm` measured from its own `y_turn`.
5. Angles: front axle as `C-6`; steerable axles move, at most `centring_i` per second, to
   `σ_i · s · lock_i` (mode 0, `s = κ / κ_max`, behind a joint `s = clamp(κ_joint / κ_max, −1, 1)`, `σ_i` −1 behind
   the turning line, +1 ahead), 0 (mode 1) or the script's angle clamped to the lock
   (mode 2); rigid axles 0. Tyres keep Ackermann about their axle's angle.
6. Hold: tyres pointing at the centre (within `SCRUB_TOLERANCE`) as today; others with the road's
   reach; keyed sections behind a joint held at each tyre (R-7, R-8). `D-3` sees every tyre's
   force.
7. Outputs: `Axle_Steering_<n>` = `δ_i` (0 for rigid), `Axle_Steer_Locked_<n>` = `e_i == 0`.

### Engine boundary

`Engine::steering(...)` in `engine.rs`: `Classic` always runs `C-6`/`C-9`; `Depot` runs the above
when the section has declarations, else `C-6`/`C-9`. A train runs the leading section's engine, so a
keyed rear section under a Classic leading section is ignored with one log line.

## Delivery phases

Each phase is one pull request; each keeps `cargo test --workspace` green and the existing traces
identical.

| Phase | Branch | Content | Spec | Label |
|---|---|---|---|---|
| 0 | (in game, maintainer) | T001: a stock bus with the two `achse_steer_*` parameters added to an axle loads and drives the same in OMSI 2. **Done 2026-11-21**: OMSI 2 ignores the new parameters (maintainer) | FR-002 | - |
| 1 | `docs/steer-axles-model` | `depot-model.md` D-5..D-7 with equations and closed forms; notes in `C-6`, `C-7`, `C-9`; `FORMATS.md` | FR-017, FR-018 | - |
| 2 | `feat/steer-axle-key` | parse, validate, log; `Axle::steer`; no physics effect | FR-001, FR-003 (parse), FR-005 | - |
| 3 | `feat/steer-axles-turning` | leading section: rigid axles, mode 0 law, turning line, lock, `Axle_Steering`, scrub; V2 keyed closed forms | US1, FR-003..FR-011, FR-014 (angles), SC-001, SC-006 | `physics-delta` |
| 4 | `feat/steer-axles-scripts` | modes 1 and 2, engagement and centring ramps, continuity, locked flag; situation save; changelog line | US2, FR-009, FR-012..FR-014, SC-003, SC-004 | `physics-delta` |
| 5 | `feat/steer-axles-sections` | sections behind a joint; V5 and V4 keyed | US3, FR-015, SC-006a | `physics-delta` |
| 6 | (measure + in game) | SC-007 frame time in the phase 5 PR; SC-008 by the maintainer on a keyed 6x2 and a keyed articulated bus | SC-007, SC-008 | - |

Phase 0 passed (2026-11-21), so the parameters stay in `[newachse]`; the next phase is 1.

## Constitution Check (after design)

No change from the gate above. The design adds one domain file, one rule file and one rule method;
nothing is duplicated (the `C-6` code is reused as the fallback line), and no speculative option is
added (no tyre model, no AI support, no default values).

## Complexity Tracking

No deviation to justify.
