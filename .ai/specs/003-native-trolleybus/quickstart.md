# Quickstart: validating native trolleybus support

## Prerequisites

- Rust 1.99.0 and the Visual Studio C++ build tools with CMake on `PATH` (an OpenXR dependency builds
  with CMake).
- An OMSI 2 installation with trolleybus content of the plugin-based convention: a map with wire
  splines and switch objects, a vehicle declared in `TrolleyWireVehicles.ini`, and
  `TrolleyWirePlugin.cfg`. Set `OMSI_ROOT` to it for the content tests.
- For tuning without touching the OMSI 2 copy: put `TrolleyWireVehicles.ini` and
  `TrolleyWirePlugin.cfg` in Depot's content folder `Plugins\` ([contracts/content-convention.md](contracts/content-convention.md)).

## Build and automated checks

```sh
cargo test -p omsi-trolley
cargo test --workspace
cargo build --release
cargo run --release -p omsi-check -- "$OMSI_ROOT"   # before and after each phase
```

## In-game scenarios (maintainer)

Run with the detailed log on (`detailed_log=1`); the log is `%USERPROFILE%\.depot\game.log`.

| Phase | Scenario | Expected |
|---|---|---|
| 1 | Load a wired map | one line "wire network: N spans, M switches"; same load time as before |
| 1 | Load a map without wire | no wire line; frame time unchanged |
| 2 | Spawn a declared rigid trolleybus, poles down | poles drawn on the hooks; spawn line names the entry |
| 2 | Spawn a declared articulated trolleybus | poles drawn on the rear section |
| 2 | Raise the poles under a straight wire | shoes settle on the wires; `shtanga_wire_present` = 1 |
| 2 | Drive curves, brake hard, cross bumps | shoes stay on the wires while the body rolls and pitches |
| 2 | Drive over reversed splices and loose joints | no contact loss; a "joint" log line each |
| 2 | Pause or alt-tab for a few seconds | poles still on the wire |
| 2 | A diesel bus on the same map | identical to the release without the feature |
| 3 | Steer away from the wire | dewirement with reason, free rise, pause, retriever to the hook |
| 3 | Use the retract control | both poles go down in phases |
| 4 | Y switch, 20° left/right branches, two switches in a row | the asked branch is taken every time |
| 4 | Neighbouring wire about 1.9 m beside a switch | never captured |
| 2–4 | Repeat at 30 and 144 fps (`max_fps`) | same result |
| 5 | Leave the wire with the vehicle powered | warning variables on; the vehicle's own script reacts |

## Measurements for each PR (Principle VII)

Same machine, same map and route, feature off and on: average and 99th-percentile frame time, map
load time, spawn time. Record them in the pull request.
