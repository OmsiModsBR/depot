# Implementation Plan: Native trolleybus support

**Branch**: one per delivery phase (see "Delivery phases") | **Date**: 2026-11-11 |
**Spec**: [spec.md](spec.md) (issue #525 and the maintainer's decisions of 2026-11-11)

**Input**: Feature specification from `.ai/specs/003-native-trolleybus/spec.md`.

**Execution notes**: `.ai/.specify/extensions.yml` does not exist, so no hooks ran. Code read at
`a3f73101` (2026-11-10). `setup-plan.sh` was run with a POSIX path in `feature.json`; a Windows path
(`C:/...`) there makes the script write under `.ai/C:/...`.

## Summary

Depot draws trolleybus content today but nothing writes the pole variables, so the poles of a
plugin-based trolleybus are invisible (their `[visible]` variable stays 0) and the vehicle never sees
a wire. This plan adds:

1. **A wire network** built from the map while tiles load: wire splines recognised by the
   convention's keyword list, sampled into two wire polylines per spline, joined where their ends
   meet, plus the switch, crossing and marker objects. Kept per tile, released with the tile.
2. **A trolleybus declaration** read from the convention's registry (`TrolleyWireVehicles.ini`) and
   global parameter file (`TrolleyWirePlugin.cfg`), matched to the player's vehicle by its `.bus`
   files.
3. **A pole simulation** for the player vehicle, run each frame after the bodies are placed, in the
   frame of the drawn body of the carrying section, which writes the convention's script variables.

Everything lives in one new crate, `omsi-trolley` (pure model, geometry and the convention reader,
testable without render or app), plus thin glue in `omsi-sim` (the per-frame step and the variables)
and `omsi-app` (tile loading, spawn, setting, log). No declaration means no network lookup and no
step: ordinary buses and maps run the same code paths as before.

## Technical Context

**Language/Version**: Rust 1.99.0 (CI toolchain), edition 2021.

**Primary Dependencies**: workspace crates `omsi-map` (tile `MapSpline`/`MapObject`), `omsi-geometry`
(`SplineCurve::from_map`, `offset_point`, `heading_at`), `omsi-scenery` (`.sli` profiles),
`omsi-cfg` (paths, the virtual file system); `glam` for vectors. No new external dependency.

**Storage**: content files only (read): the convention's registry and parameter file, map tiles,
`.sli`/`.sco`. Nothing is written.

**Testing**: `cargo test --workspace`. The new crate's tests in `crates/omsi-trolley/tests/` with small
synthetic networks and declarations made for the test; tests that need real content read it from
`OMSI_ROOT` and skip without it (Principle VI).

**Target Platform**: Windows x64 (the only supported platform); DX12/Vulkan and DX9 renderers.

**Project Type**: desktop game engine (Cargo workspace).

**Performance Goals**: zero measurable frame-time change on a map without wire; on a wired map the
per-frame cost of one player trolleybus stays far below 0.1 ms (two poles, a handful of candidate
segments from a grid index); no added spawn wait (network built on the tile loader thread).

**Constraints**: physics and state traces of non-trolleybus vehicles identical (Principle II);
content errors logged, never fatal (IV); no repair of content (III); no thresholds or names derived
from the third-party plugin's binary in public text (VIII); domains as packages (V).

**Scale/Scope**: maps with a few thousand wire splines; one player trolleybus (AI trolleybuses out of
scope by decision 2).

## Constitution Check

*GATE: must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | How the plan complies | Status |
|---|---|---|
| I. Own path | Clean implementation in Depot; nothing from openOMSI's later (non-MIT) trolleybus code. | Pass |
| II. Fidelity only with validation | No claim of matching OMSI 2 or the plugin; behaviour described as Depot's, validated by the maintainer in the game (quickstart). `omsi-check` before/after each phase; traces identical without a declaration. | Pass |
| III. No compensation | A broken declaration is reported and the vehicle runs as an ordinary bus; wire gaps above the join tolerance are not bridged; the model's own `[visible]`/anim rules are not changed. | Pass |
| IV. Invalid content never crashes | Readers return errors per entry; one bad entry or spline is skipped and logged. | Pass |
| V. Domains as packages | `omsi-trolley` with `domain/`, `interface/{input,output}.rs`, `implementation/` (one module per subdomain: network, convention, pole, switch). | Pass |
| VI. Tests outside `src/` | `crates/omsi-trolley/tests/`; `testing` module behind `test-support`; each fix with a failing test. | Pass |
| VII. Performance | New feature: cost measured before/after on the same scenario and justified in each PR; zero cost without wire is a gate. | Pass (with measurements per PR) |
| VIII. Provenance | Parameters come from content files; Depot's own tolerances are chosen and calibrated by test in the game; no binary-derived names, addresses or thresholds; no AI credit in commits. | Pass |
| IX. Small changes | Six phases, one PR each, prefixes `feat/`; each adds an **Unreleased** changelog line when the player notices it. | Pass |
| X. Lean code | No speculative AI/electrical code; the native-script adapter only in its own phase. | Pass |

## Project Structure

### Documentation (this feature)

```text
.ai/specs/003-native-trolleybus/
├── spec.md
├── plan.md              # this file
├── research.md          # Phase 0
├── data-model.md        # Phase 1
├── quickstart.md        # Phase 1
├── contracts/
│   ├── content-convention.md   # files and keys Depot reads
│   └── script-variables.md     # variables written and read each frame
├── checklists/requirements.md
└── tasks.md             # /speckit-tasks (not made here)
```

### Source Code (repository root)

```text
crates/omsi-trolley/                 # NEW: pure model, no render/app dependency
├── Cargo.toml                       # deps: omsi-map, omsi-geometry, omsi-scenery, omsi-cfg, glam
├── src/
│   ├── lib.rs                       # re-exports interface::input; testing behind test-support
│   ├── domain/
│   │   ├── network.rs               # WireSpan, WireJoint, WireSwitch, Crossing, Marker, TileWires
│   │   ├── declaration.rs           # TrolleyDeclaration, PoleGeometry, PoleParams
│   │   └── pole.rs                  # PoleState, PolePhase, DewireReason, PoleOutputs, PoleInputs
│   ├── interface/
│   │   ├── input.rs                 # build_tile_wires, read_convention, PoleSim::step, NetworkView
│   │   └── output.rs                # wraps omsi-map/omsi-geometry types it uses
│   ├── implementation/
│   │   ├── network.rs               # spline → wire polylines, joints, grid index
│   │   ├── convention.rs            # plugin-based convention reader (ini + cfg)
│   │   ├── pole.rs                  # kinematics, capture, tracking, contact force
│   │   ├── dewire.rs                # dewirement, free rise, retriever, retract (phase 3)
│   │   └── switch.rs                # switch entry, branch, exit capture (phase 4)
│   └── testing.rs
└── tests/                           # synthetic networks and declarations

crates/omsi-sim/src/vehicle/trolley.rs   # per-frame step glue: carrier pose → PoleSim → vars
crates/omsi-app/src/world/...            # tile staging builds TileWires; World keeps/drops them
crates/omsi-app/src/gameplay/implementation/spawn.rs   # match declaration at spawn, log
crates/omsi-app/src/settings.rs (+ launcher)            # `trolleybus=1` setting
```

**Structure Decision**: one new domain crate, as issue #525 proposes, so the model and geometry are
tested alone with `cargo test -p omsi-trolley`. `omsi-sim` only calls `PoleSim::step` and moves
variables; `omsi-app` only feeds tiles and the declaration in. Registered in the workspace
`members` before `omsi-sim`.

## Design overview

**Network build (tile loader thread).** For each tile, splines whose `.sli` path contains one of the
convention's `spline_keywords` (case-insensitive) become a `WireSpan`: the curve
(`SplineCurve::from_map` with the tile origin, so maps with world coordinates and any tile size come
out in the one global frame) is sampled into two polylines, left and right wire, at the convention's
`wire_height` and `wire_x_left`/`wire_x_right` (mirror flips the side). Objects whose `.sco` path
names a switch, crossing or marker of the convention become `WireSwitch`/`Crossing`/`Marker` with
their world pose. Joints are found when the network of the loaded tiles changes: span ends closer
than the join tolerance with compatible direction (either sense, so reversed splices join). A uniform
grid over segments answers "segments near this point".

**Declaration (spawn).** The registry is read from Depot's content folder `Plugins\` first, then the
OMSI 2 folder's `plugins\`. An entry applies when its `bus_file` equals the player's `.bus` path
(relative to the OMSI root, case-insensitive) and, if it names `pole_carrier_bus_file`, that file is
one of the coupled parts, which then carries the poles. Entries that are disabled (`enabled=0`) or
broken are skipped and logged. No match: nothing else happens for that vehicle.

**Pole step (each frame, player only).** In `VehicleInstance::update`, after the bodies and coupled
parts are placed and before the meshes are animated (the same slot `articulation_*_alpha` uses), the
glue takes the carrier's body transform (`body_rotation` + `position`, suspension included), reads
the inputs from the script variables, steps each pole and writes the outputs with
`set_engine_var`. Scripts read them on the next frame, meshes animate with them in this one. Time
tolerances use `dt`; a stall longer than the reset time clears the state (FR-015).

**Logging.** One detailed-log line per decision (capture, joint taken, switch entry/exit,
dewirement with reason), with the frame number; one normal-log line per spawn ("trolleybus: entry X,
poles on part N") and per map ("wire network: N spans, M switches").

## Delivery phases

Each phase is one branch and one pull request, reviewed alone, with `omsi-check` and the
non-trolleybus traces before/after.

| # | Branch | Delivers | Player notices |
|---|---|---|---|
| 1 | `feat/trolley-wire-network` | crate, convention reader, network per tile, joints, grid index, map log line | no (log only) |
| 2 | `feat/trolley-poles` | declaration at spawn, pole kinematics in the body frame, capture and tracking on plain wire and joints, raise/lower, pole variables, setting | **poles drawn and on the wire** |
| 3 | `feat/trolley-dewire` | dewirement by reach/force/angle, free rise, retriever, retract control | dewirement |
| 4 | `feat/trolley-switches` | switches (Y, left/right 20°), consecutive switches, crossings, speed limit through special pieces | switches |
| 5 | `feat/trolley-script-signals` | no-wire warning and engine-trip variables, complex-piece overspeed | warnings |
| 6 | `feat/trolley-native-script` | native-script-convention adapter (decision 5: last) | that content runs |

AI trolleybuses, sounds, sparks and an electrical model are separate later features (FR-017).

## Complexity Tracking

No constitution violation to justify.
