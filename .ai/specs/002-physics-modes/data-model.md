# Data Model: Two physics modes

Phase 1 of the [plan](plan.md).

## PhysicsMode (new, `omsi-cfg/src/physics.rs`)

| Value | Saved as | Shown as |
|---|---|---|
| `Classic` | `classic` | OMSI Classic |
| `Depot` | `depot` | Project Depot Physics |

- `parse(&str) -> PhysicsMode`, ignoring case, `-`, `_` and spaces: `classic`, `omsi`, `omsi2`,
  `omsi_classic` → `Classic`; `depot`, `project_depot` → `Depot`; empty or missing → `Classic`;
  anything else → `Classic` with a warning in the log.
- `as_str()` returns only the two canonical spellings.
- `for_vehicle(global: PhysicsMode, depot_key: bool, driver: Driver) -> (PhysicsMode, Reason)`:
  `Depot` with `Reason::BusKey` when the key is present **and** `driver` is the player; otherwise
  `global` with `Reason::Setting` (an AI vehicle never takes the key, decision D6). There is no
  path from the key to `Classic` (#677: no "force Classic").

## Vehicle (`omsi-vehicle`, exists)

| Field | Type | From |
|---|---|---|
| `depot_physics` (new) | `bool` | `[depot_physics]` present in the file ([contracts/bus-key.md](contracts/bus-key.md)) |

The key stops being an `unknown_keywords` entry. Lines under it before the next section are reported
once as ignored.

## Resolution at spawn (once per vehicle)

```text
global  = settings.physics                       # read at session start
(mode, reason) = PhysicsMode::for_vehicle(global, spawned_file.depot_physics, driver)  # D6: AI ignores the key
for each section of the vehicle (front first):   # decision D4: the front section's file decides, for every section
    section.engine = mode
    if section's own file has [depot_physics] and it is not the spawned file:
        log "key in a rear section's file has no effect"
log "<vehicle>: <mode> physics (<reason>)"
```

The resolved mode is held by the vehicle instance and handed to its bodies; nothing reads the global
setting after spawn. A change of the setting applies to vehicles spawned afterwards (in practice, the
next session: the setting lives in the launcher).

## Engine (new, `omsi-sim::physics::domain`)

```text
enum Engine { Classic, Depot }     # From<PhysicsMode>
```

Held by `RigidBody` (every section of a train has the same). The stages that differ call
`implementation::classic::*` or `implementation::depot::*` by a `match` on it:

| Stage | Classic | Depot | Shared input |
|---|---|---|---|
| Static load reference of the struts (the torque term in `wheel_contacts`) | model origin: adds `cog × F` back | `[cog]`: adds nothing | strut force `F`, `cog` |
| Rest loads (`wheel_rest_loads`, used to place a body at rest and by the invariants) | balance about `y = 0` | balance about `cog_y` | axles, mass |

`PhysicsState` (snapshot for retries) keeps the engine with the body, so a restored state keeps its
engine.

## AiMotion (new, `omsi-sim::ai`)

```text
enum AiMotion {
    Classic(ClassicAiBody),   # phase 8
    Depot(AiBody),            # today's AiBody, moved to ai/depot/
}
```

For road vehicles only; `MotionKind::Rail` and `MotionKind::Air` keep their shared followers. Built by
`omsi-app`'s traffic from the vehicle's resolved mode (`traffic.rs`, where `AiBody::new` is called
today). Both arms expose what the traffic and the scripts read today: `position`, `heading`,
`pitch_deg`, `bank_deg`, `steer`, `a_long`, `a_lat`, `suspension`, `apply(&mut VehicleInstance)`.

### ClassicAiBody (phase 8), fields by concern

| Concern | Fields | Rows of R-A |
|---|---|---|
| Path | distance along the path, lateral offset, heading relative to the path, vehicle curvature (this and last frame), steer angle per axle, `a_lat`; pose from the path | KA-1, KA-4 |
| Speed | speed, `a_long`, regime (near or serving a stop: physical; far: target speed), throttle/brake set by the AI driver, grip | KA-2, KA-3, KA-9, KA-10 |
| Rear sections and release | articulation angle per joint (rear sections), `state: OnRoute \| Loose` for the whole vehicle, the "clear crash" command | KA-5..KA-8, KA-6b, KA-11 |

While loose, a Classic AI vehicle's sections run the player's `RigidBody` (Classic engine) with
throttle 0, brake 1 and hazard lights on, starting from their route poses and articulation angles;
`OnRoute` again, all sections together, when the crash is cleared or its tile leaves the detail radius
(the camera's tile and its eight neighbours), asking the planner for a new path.

## AI static pose

Depot keeps today's static pose (`kinematic_wheel_rest_loads` about `cog_y`, used by `vehicle.rs`,
`vehicle_type.rs` and `coupled/trailer.rs`). A Classic AI vehicle on its route takes its height and
attitude from the path (KA-4, KA-7); its scripts read, per axle, the static sag computed once at spawn
with the weight shared about the origin (K-1); loose, its rigid body shares the weight about the
origin (K-1).

## Settings (`omsi-app`, `omsi-launcher-core`)

| Field | Type | Key |
|---|---|---|
| `Settings::physics` (new) | `PhysicsMode` | `physics` ([contracts/settings-cfg.md](contracts/settings-cfg.md)) |
