# Implementation Plan: Two physics modes (OMSI Classic and Depot)

**Branch**: one per delivery phase (see "Delivery phases") | **Date**: 2026-11-08 |
**Spec**: issue #677 and its decision comment (no `spec.md`: the maintainer's request below is the
specification)

**Input**: the maintainer's request (2026-11-08), on top of the decisions already recorded in #677:

> - The code of each physics is kept apart, so that the switch changes from one module to the other
>   where the two differ.
> - Depot physics is switched on in two ways: (1) a setting in the launcher, (2) a keyword in the
>   `.bus` file. The keyword forces Depot physics even when the setting is OMSI 2 Classic. The switch
>   applies to the player's vehicle and to the AI vehicles.
> - AI physics: a new implementation brings the AI in line with OMSI 2's. The current Depot AI
>   physics becomes part of the new Depot physics.
> - Documentation: one logical-mathematical model document for each physics engine (OMSI Classic and
>   Depot), with the equations, calculations and concepts both use.
> - Tests: the tests validate the implementation against the logical-mathematical model with at
>   least five kinds of vehicle: a 4x2 bus, a 6x2 bus with a steered third axle, a 6x2 articulated
>   bus, a bi-articulated bus, and a 23 m articulated 8x2 bus.

Decisions already taken in #677 that this plan keeps: one global setting (default OMSI Classic); the
Depot mode holds only changes that *override* OMSI 2 behaviour, while changes that only *add*
(articulated bus work, standing passengers, skip stop) stay on in both modes; no "automatic" mode and
no "force Classic" option; the `.bus` key must be ignored by OMSI 2, openOMSI and other forks; nothing
is implemented before (a) the black-box check that OMSI 2 ignores the key and (b) the key's final name
and format are settled.

**Execution notes**: `.ai/.specify/extensions.yml` does not exist, so no hooks ran. The feature
directory was created by hand (`setup-plan.ps1` was not run: no PowerShell in the planning
environment). Code read at `0f4b334` (2026-11-08).

## Summary

Depot has one road-vehicle physics today, with two bodies:

| Who | Body today | Where |
|---|---|---|
| Player's road vehicle (and each articulated section) | rigid body on spring struts, fixed 1/240 s substeps; weight shared about the model origin since #675 | `omsi-sim/src/physics/` |
| AI road vehicle | `AiBody`: bicycle model steered by pure pursuit, speed handed in by the traffic planner, body on heave/pitch/roll springs over a least-squares contact plane; static sag about `[cog]` | `omsi-sim/src/ai/`, `omsi-app/src/gameplay/implementation/traffic.rs` |
| Player's rail vehicle, AI rail and air vehicles | kinematic model / path followers | `physics/implementation/kinematic.rs`, `ai/{rail,air}.rs` |

The plan leaves **two engines** that share every stage they have in common and differ only where a
Depot rule overrides an OMSI 2 one:

| Engine | Player road vehicle | AI road vehicle |
|---|---|---|
| **OMSI Classic** (default) | today's rigid body (weight about the model origin) | **new** Classic AI body, brought in line with OMSI 2's AI ([research.md](research.md), R-A) |
| **Depot** | the same rigid body with Depot's overrides; the first is the weight shared about `[cog]` | today's `AiBody`, moved into the Depot engine unchanged |

Rail and air vehicles have no override and are the same in both engines.

Technical approach:

1. **Write the two model documents first** (`docs/physics/`): a common part with the equations both
   engines share, and one document per engine with what is its own, every equation numbered so a
   test can name it ([contracts/model-docs.md](contracts/model-docs.md)).
2. **Five synthetic test vehicles and conformance tests** of today's player physics against the
   Classic document, before any behaviour changes, so the later phases have a fixed reference.
3. **One type for the mode**, `PhysicsMode { Classic, Depot }` in `omsi-cfg`, read from
   `settings.cfg` (`physics=classic|depot`) and resolved **per vehicle** at spawn: the `.bus` key
   `[depot_physics]` makes that vehicle Depot whatever the setting says
   ([data-model.md](data-model.md), [contracts/](contracts/)).
4. **Split the code by engine** where the engines differ: `classic/` and `depot/` modules beside the
   shared stages, chosen by an enum held by the body (two real implementations, Principle V).
   Moving today's code there changes no bit of any trace.
5. **Depot overrides**: weight shared about `[cog]` for the player's body in Depot; the AI's static
   pose follows the vehicle's engine (origin in Classic, `[cog]` in Depot).
6. **Classic AI**: a new AI body that behaves as OMSI 2's AI is observed to behave (R-A), delivered
   in small steps, each with its conformance tests on the five vehicles and the Depot AI's traces
   unchanged.
7. **Expose the setting** in the launcher and document the key publicly once both engines are
   complete, so no player can choose a half-built mode.

## Technical Context

**Language/Version**: Rust 1.99 (CI toolchain)

**Primary Dependencies**: `glam` (vectors), `omsi-cfg` (settings and key parsing), `omsi-vehicle`
(`.bus` parser), `omsi-script` (the scripts' `M_Wheel`, brake and spring variables)

**Storage**: `settings.cfg` in `~/.depot` (new key `physics`); the launcher reads and writes the same
file. `.bus` files are read-only content (new key `[depot_physics]`)

**Testing**: `cargo test --workspace`; physics, AI and traffic traces (`tests/physics_traces.rs`,
`tests/ai_traces.rs`, `tests/traffic_traces.rs`) compared bit for bit by
`scripts/physics-trace-diff.sh` and the `physics-traces` CI job (label `physics-delta` for intended
changes); invariants (`tests/physics_invariants.rs`); new conformance tests per model equation; tests
reading OMSI 2 content skip without `OMSI_ROOT`

**Target Platform**: Windows x64 (the only supported platform)

**Project Type**: desktop application (game) in a crate workspace

**Performance Goals**: no regression (Principle VII). The mode is resolved once per vehicle at spawn;
choosing a module per stage is an enum match, not a lookup per frame. The Classic AI body must not
cost more per vehicle and frame than `AiBody` (measured, R-P)

**Constraints**: with the setting at Classic and no `[depot_physics]` key, the player's traces stay
bit-identical through every phase (today's player physics *is* the Classic one after #675); the
Depot AI's traces stay bit-identical through every phase; a phase that changes behaviour does it in
one engine only and carries `physics-delta` with the predicted delta

**Scale/Scope**: `omsi-sim/src/physics` 3.9k lines, `omsi-sim/src/ai` 0.75k, `vehicle/coupled`
1.3k, the AI's driver in `omsi-app` `gameplay/implementation/traffic.rs`; existing traces: 26 player
scenarios (`t01`-`t16`, `b1`-`b7`) and the AI/traffic scenarios

## Constitution Check

*GATE: before research, and again after design.*

| Principle | Status | How the plan meets it |
|---|---|---|
| I. Own path | ✓ | The Depot engine is Depot's own; nothing depends on openOMSI |
| II. OMSI 2 fidelity only with validation | ✓ with a condition | The Classic engine is "OMSI Classic" by name, so every Classic rule that is not yet proven carries a "to validate" mark in the model document until a black-box test or the maintainer's in-game check settles it (R-A, R-V). The `.bus` key goes in only after the black-box check (a) of #677. Each behaviour change carries `physics-delta` |
| III. No compensation for misconfigured content | ✓ | Neither engine repairs a file. A mod tuned for Classic that stands unlevel in Depot shows that as it is (R-N) |
| IV. Invalid content never crashes | ✓ | An unknown `physics=` value falls back to Classic with a log line; `[depot_physics]` with stray lines, or in a rear section's file, is reported and never fails the load |
| V. Domains as packages | ✓ | Shared stages stay in `physics/implementation/`; the engine-specific ones go to `implementation/classic/` and `implementation/depot/`; `PhysicsMode` is a domain object in `omsi-cfg`. The engine choice is an enum with two real implementations, the case Principle V allows |
| VI. Tests outside `src/` | ✓ | Conformance tests and the five vehicles live in `omsi-sim/tests/`; no new test in `src/` |
| VII. No performance regression | ✓ with a condition | Structural phases measure nothing new but must show no frame-time change; the Classic AI phases carry a before/after of the AI step per vehicle (R-P) |
| VIII. Provenance and public text | ✓ | The model documents describe observed behaviour and the tests that show it; no addresses or internal names of the reference executable; hypotheses about OMSI 2 are written as observable checks |
| IX. Small, traceable changes | ✓ | About twelve pull requests, one topic each, `docs/`, `feat/` or `fix/` by content |
| X. Lean, clean code | ✓ | Only the stages that differ are split; everything else is shared, not copied. Old AI code is moved, not duplicated. No env flag or CLI switch for the mode (the setting and the key are the two ways in, as requested) |

Result: **pass**, with two conditions that become tasks: validate each Classic rule that the model
document marks (II) and measure the Classic AI against `AiBody` (VII). Re-checked after design:
unchanged.

## Project Structure

### Documentation (this feature)

```text
.ai/specs/002-physics-modes/
├── plan.md              # this file
├── research.md          # Phase 0: inventory of overrides, Classic AI, key, dispatch, tests
├── data-model.md        # Phase 1: PhysicsMode, per-vehicle resolution, engine dispatch
├── quickstart.md        # Phase 1: how each phase is validated
└── contracts/
    ├── settings-cfg.md  # the `physics` key in settings.cfg and the launcher
    ├── bus-key.md       # the `[depot_physics]` key in a .bus file
    ├── model-docs.md    # structure of the two model documents, equation ids
    └── test-vehicles.md # the five synthetic vehicles and the conformance matrix
```

### Documentation (repository, delivered by the phases)

```text
docs/physics/
├── README.md            # what the two engines are, how a vehicle gets one, how to read the docs
├── common-model.md      # C-*: equations both engines use (units, step, strut, tyre, steering, body, joints, hits)
├── classic-model.md     # K-* (player) and KA-* (AI): OMSI Classic's own rules, each marked validated or to validate
└── depot-model.md       # D-* (player) and DA-* (AI): Depot's overrides and the Depot AI
docs/FORMATS.md, docs/MODDING.md   # the [depot_physics] key (phase 11)
docs/USER_GUIDE.md                 # the launcher setting (phase 11)
```

### Source Code (repository root)

```text
crates/
├── omsi-cfg/src/
│   └── physics.rs (new)                 # PhysicsMode: parse/as_str, for_vehicle
├── omsi-vehicle/src/vehicle.rs          # reads [depot_physics] into Vehicle::depot_physics
├── omsi-sim/src/
│   ├── physics/
│   │   ├── domain/vehicle.rs            # Dynamics keeps Rigid/Kinematic; RigidBody holds its engine
│   │   ├── implementation/              # shared stages: spec, ground, suspension, wheel, tyre, body, collide, train, kinematic
│   │   ├── implementation/classic/      # load_share.rs (weight about the origin)
│   │   └── implementation/depot/        # load_share.rs (weight about [cog])
│   ├── ai/
│   │   ├── mod.rs                       # AiMotion { Classic(..), Depot(AiBody) } for road vehicles; rail and air shared
│   │   ├── classic/ (new)               # the Classic AI body (phases 8-10)
│   │   └── depot/                       # road.rs and AiBody moved here unchanged (phase 6)
│   └── vehicle/
│       ├── vehicle_type.rs, visuals.rs  # AI static pose by engine (rest loads about origin or [cog])
│       └── coupled/trailer.rs           # rear sections of AI vehicles by engine
├── omsi-sim/tests/
│   ├── support/vehicles.rs              # + the five vehicles of contracts/test-vehicles.md
│   ├── physics_model_classic.rs (new)   # conformance K-*/C-* (player) and KA-* (AI)
│   ├── physics_model_depot.rs (new)     # conformance D-*/C-* (player) and DA-* (AI)
│   ├── physics_traces.rs, ai_traces.rs  # every scenario run per engine; trace file names carry the engine
│   └── physics_invariants.rs            # I1 per engine (the static share differs)
├── omsi-app/src/
│   ├── settings.rs                      # Settings::physics: PhysicsMode
│   └── gameplay/implementation/traffic.rs  # builds the AI body of the vehicle's engine
├── omsi-launcher-core/src/lib.rs        # reads/writes `physics`
└── omsi-launcher-ui/src/window/domain/settings.rs  # "Physics" list: OMSI Classic, Project Depot Physics
scripts/physics-trace-diff.sh            # runs both engines
CHANGELOG.md
```

**Structure Decision**: no new crate. `PhysicsMode` goes to `omsi-cfg` because the app, the launcher
core and the launcher UI already depend on it (the same reason `GraphicsMode` goes there in
`001-graphics-modes`). The engine split lives inside `omsi-sim`'s `physics` and `ai` packages, next to
the shared stages it plugs into.

## Delivery phases

Ordered so that each pull request is small, revertible and does not wait on an open decision. The
launcher setting comes last: until then nothing a player can choose changes, except the Classic AI,
which improves the default.

| # | Branch | Content | Needs | Behaviour changes? |
|---|---|---|---|---|
| 0 | (no code) | Black-box checks in OMSI 2: the key is ignored (Q1); the Classic AI observations KA-1..KA-7 (Q2); the Mi5 levels as in OMSI 2 with #675 (Q3); the override candidates of R-V (Q4) | — | — |
| 1 | `docs/physics-models` | `docs/physics/` with the common, Classic and Depot models; Classic AI and the R-V candidates marked "to validate" | — | no |
| 2 | `feat/physics-test-vehicles` | The five synthetic vehicles; conformance tests of today's player physics against C-*/K-* (the Classic engine as it stands) | 1 | no (a mismatch found becomes a `fix/` PR) |
| 3 | `feat/physics-mode-type` | `PhysicsMode` in `omsi-cfg`; `settings.cfg` key `physics` read by the game and the launcher core (not shown in the UI yet); `[depot_physics]` parsed; mode resolved per vehicle at spawn and held by the vehicle | 0 (Q1), D1 | no |
| 4 | `feat/physics-traces-per-engine` | Trace and invariant harness runs each scenario per engine; `physics-trace-diff.sh` and CI cover both | 3 | no (Depot traces identical to Classic until phase 7) |
| 5 | `feat/physics-engine-split` | `implementation/classic/` and `implementation/depot/`; `RigidBody` holds its engine; the load-share stage moved behind it, both engines still doing today's rule | 3 | no |
| 6 | `feat/ai-engine-split` | `AiBody` and `road.rs` moved to `ai/depot/`; `AiMotion` with only the Depot arm built until phase 8; AI static pose computed by engine (both still today's) | 5 | no |
| 7 | `feat/depot-cog-load-share` | Depot: the player's weight shared about `[cog]` (struts and `wheel_rest_loads`); conformance D-1 on the five vehicles | 5, 2 | yes, Depot player only (`physics-delta`) |
| 8 | `feat/classic-ai-path` | Classic AI body: the rotation point on the lane, heading from the path, rear axle on the path; speed as the planner hands it (as today) | 6, 0 (Q2) | yes, AI in Classic (`physics-delta`) |
| 9 | `feat/classic-ai-drive` | Classic AI speed from the vehicle's own drive torque, brakes and grip, the AI driver setting throttle and brake for the target speed (KA-2, KA-3), if Q2 confirms it | 8 | yes, AI in Classic |
| 10 | `feat/classic-ai-pose` | Classic AI static pose about the origin; body pose on the ground per KA-4; articulated rear sections per KA-5; release to the free rigid body after a hit per KA-6, if Q2 confirms it | 8 | yes, AI in Classic |
| 11 | `feat/physics-mode-setting` | Launcher "Physics" list (OMSI Classic, Project Depot Physics); the key documented in `FORMATS.md`/`MODDING.md`; `USER_GUIDE.md`; the log line that names a vehicle's engine; `CHANGELOG.md` | 7, 10 | yes: the player can choose Depot |
| 12 | `feat/depot-<override>` (one per item) | Each further override decided under D2, with its D-* equation and conformance test | 11, D2 | yes, Depot only |

Phases 7 and 8-10 can run in parallel after 6. A phase that finds today's code disagreeing with the
model document fixes the code (a `fix/` PR with its failing test) or corrects the document, never
both silently.

## Decisions for the maintainer

- **D1** (phase 3): name and format of the key. Recommended: `[depot_physics]` with no value lines,
  presence means Depot ([contracts/bus-key.md](contracts/bus-key.md)). Alternative: a value line
  (`1`), which adds a way to write a meaningless `0` and does not let a file force Classic anyway.
- **D2** (phases 1 and 12): which of today's behaviours are overrides and move to Depot only. The
  candidates and their checks are in [research.md](research.md), R-V. Recommended: only `[cog]` load
  sharing is certain now; each other candidate moves only after its black-box check shows OMSI 2
  behaves differently.
- **D3** (phase 10): after a hit, does a Classic AI vehicle become a free rigid body without a driver
  until it is cleared (KA-6)? Recommended: yes if Q2 confirms it in OMSI 2, otherwise the Classic AI
  stays on its path and the hit is an impulse, as today.
- **D4** (phase 3): in an articulated vehicle, which file's key counts. Recommended: the file the
  vehicle is spawned from (the front section); a key found only in a rear section's file is reported
  in the log and has no effect.
- **D5** (phases 2 and 7): the 23 m articulated 8x2 is read as one vehicle (front section on two
  axles, rear section on two axles with the fourth steered), making five vehicles in total. If a
  separate rigid 8x2 bus was meant, it becomes a sixth vehicle in
  [contracts/test-vehicles.md](contracts/test-vehicles.md).

## Complexity Tracking

| Item | Why it is needed | Simpler alternative rejected because |
|---|---|---|
| Two engines (a second implementation of the load share and of the AI body) | Requested; the Classic engine keeps mods working as tuned, the Depot engine holds deliberate improvements | One engine with a per-constant switch: scatters `if depot` through the shared stages, the opposite of "each physics kept apart" |
| Enum dispatch per stage (`Engine::Classic`/`Engine::Depot`) | Exactly two implementations, chosen once per vehicle | A trait object per stage: allocation and indirection for a closed set of two; a whole second `RigidBody`: duplicates every shared stage (Principle X) |
