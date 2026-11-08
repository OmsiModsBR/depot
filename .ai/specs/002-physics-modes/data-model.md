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
- `for_vehicle(global: PhysicsMode, depot_key: bool) -> (PhysicsMode, Reason)`: `Depot` with
  `Reason::BusKey` when the key is present, otherwise `global` with `Reason::Setting`. There is no
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
(mode, reason) = PhysicsMode::for_vehicle(global, spawned_file.depot_physics)
for each section of the vehicle (front first):   # decision D4: the spawned file decides
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
| further overrides (decision D2) | one row each | | |

`PhysicsState` (snapshot for retries) keeps the engine with the body, so a restored state keeps its
engine.

## AiMotion (new, `omsi-sim::ai`)

```text
enum AiMotion {
    Classic(ClassicAiBody),   # phases 8-10
    Depot(AiBody),            # today's AiBody, moved to ai/depot/
}
```

For road vehicles only; `MotionKind::Rail` and `MotionKind::Air` keep their shared followers. Built by
`omsi-app`'s traffic from the vehicle's resolved mode (`traffic.rs`, where `AiBody::new` is called
today). Both arms expose what the traffic and the scripts read today: `position`, `heading`,
`pitch_deg`, `bank_deg`, `steer`, `a_long`, `a_lat`, `suspension`, `apply(&mut VehicleInstance)`.

### ClassicAiBody (phases 8-10), fields by phase

| Phase | Fields | Rows of R-A |
|---|---|---|
| 8 | rotation point on the path (distance along it), heading, steer angle per axle, `a_lat` | KA-1 |
| 9 | speed, `a_long`, throttle/brake set by the AI driver, grip | KA-2, KA-3 |
| 10 | static sag per axle about the origin, ground plane, rear sections' poses, `released: bool` (after a hit, decision D3) | KA-4..KA-7 |

## AI static pose

`kinematic_wheel_rest_loads` (today about `cog_y`) becomes a function of the engine: about `0` for
Classic, about `cog_y` for Depot. Used by `vehicle.rs` (`kinematic_rest_sag`), `vehicle_type.rs` and
`coupled/trailer.rs`.

## Settings (`omsi-app`, `omsi-launcher-core`)

| Field | Type | Key |
|---|---|---|
| `Settings::physics` (new) | `PhysicsMode` | `physics` ([contracts/settings-cfg.md](contracts/settings-cfg.md)) |
