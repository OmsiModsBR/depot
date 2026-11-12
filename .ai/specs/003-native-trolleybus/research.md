# Research: Native trolleybus support

Decisions taken while planning. Code references are at `a3f73101`.

## R1. Where the feature lives

- **Decision**: a new crate `omsi-trolley` with the model, the geometry and the convention reader;
  glue in `omsi-sim` (per-frame step, variables) and `omsi-app` (tiles, spawn, setting).
- **Rationale**: issue #525 §4 asks for a crate testable alone; `omsi-sim` already owns the vehicle
  frame (`vehicle.rs` `update`) and `omsi-app` the tile streamer (`world/implementation/tiles.rs`).
- **Alternatives**: everything in `omsi-sim` (mixes map parsing into the simulation crate); a module
  in `omsi-app` (not testable without the app).

## R2. When the poles are stepped in the frame

- **Decision**: after the bodies and coupled parts are placed, before `animate_meshes`; outputs
  written with `set_engine_var`, so meshes use them this frame and scripts read them next frame.
- **Rationale**: `VehicleInstance::update` runs physics → `run_frame` → `update_visuals`, and
  `update_visuals` places the coupled parts (`update_trailers`) before animating meshes. The poles of
  an articulated trolleybus sit on the rear part, whose pose exists only after `update_trailers`.
  `articulation_*_alpha` already uses this slot (written after the scripts, seen by them a frame
  later), so the pattern is known.
- **Alternatives**: before `run_frame` with the previous frame's rear pose (one frame behind on the
  drawn body, visible as the shoe sliding on the wire in corners).

## R3. Coordinates

- **Decision**: the network is kept in the one global frame Depot uses for vehicles (`DVec3`), built
  with `SplineCurve::from_map(&spline, tile_origin)` after `fit_to_world_grid`.
- **Rationale**: tiles of maps with world coordinates are already scaled onto Depot's grid, so the
  network needs no tile arithmetic of its own; the session start position never has to be guessed
  (spec scenario 1.3).

## R4. Wire position inside a wire spline

- **Decision**: the convention's global parameters `wire_height`, `wire_x_left`, `wire_x_right`
  (overridable per declaration where the registry gives contact positions); `mirror` flips sides.
- **Rationale**: these keys are part of the convention's public parameter file and every wire
  spline of the convention carries its two wires at those offsets. Reading the wires from the `.sli`
  profile was considered: profiles of the convention are thin strips whose points do not mark which
  strip is a wire, so the result would be a guess (Principle III).

## R5. Recognising wire splines and wire objects

- **Decision**: splines by the case-insensitive substring list `spline_keywords` of the parameter
  file; objects by the file-name markers the convention's content uses for switches (Y and the
  left/right branch kinds), crossings and sound markers, listed in
  [contracts/content-convention.md](contracts/content-convention.md).
- **Rationale**: this is how the convention's content is authored (object and spline names); no
  geometry heuristics (issue #525 §2).

## R6. Matching a declaration to the vehicle

- **Decision**: by `bus_file` (player's `.bus`) and `pole_carrier_bus_file` (the coupled part that
  carries the poles), paths relative to the OMSI root, case-insensitive. Entries with `enabled=0` or
  whose section name starts with a disabled marker are skipped.
- **Rationale**: explicit declaration only (FR-001); the `.bus` paths are the explicit part of the
  registry entry. Matching by model file name substrings was rejected: it can catch an unrelated
  vehicle whose model shares part of the name.

## R7. Where the files are read

- **Decision**: Depot content folder `Plugins\TrolleyWireVehicles.ini` and
  `Plugins\TrolleyWirePlugin.cfg` first; the OMSI 2 folder's `plugins\` copy when Depot's is absent.
  The parameter file's `active_map` key is ignored: the loaded map decides.
- **Rationale**: the maintainer keeps the OMSI 2 plugin's files untouched for OMSI 2 and tunes a copy
  for Depot (spec Assumptions). The content folder is already a read root (`omsi-cfg` roots).

## R8. Tolerances not given by the content

- **Decision**: Depot's own values for the join tolerance, capture radius, switch entry distance and
  lateral alignment, stall reset time and the frame-free versions of frame counts in the parameter
  file (a count is read as frames at 60 frames per second and converted to seconds). Each is a named
  constant in `domain/`, chosen to satisfy the spec's scenarios (for example joints up to about a
  metre, a neighbour wire at 1.9 m never captured) and calibrated in the game by the maintainer.
- **Rationale**: Principle VIII forbids carrying thresholds derived from the third-party binary;
  the scenarios in issue #525 §3 are the specification instead.

## R9. Performance

- **Decision**: per-tile wire data built on the loader thread; a uniform grid (cell about 20 m) for
  segment lookup; the per-frame step runs only when a declaration matched; a map with no wire spline
  builds nothing.
- **Rationale**: Principle VII; the tile loader already runs `prepare_tiles` off the main thread with
  rayon.

## R10. Variables

- **Decision**: the convention's variable names, written with `set_engine_var` (created when the
  scripts do not declare them, because models read some only in `[visible]` and animations); the
  list and meaning in [contracts/script-variables.md](contracts/script-variables.md).
- **Rationale**: existing trolleybus scripts (pole visibility, hooks, ropes, warnings) keep working
  unchanged; coupled parts animate from the lead's variables, so one set serves the whole vehicle.
