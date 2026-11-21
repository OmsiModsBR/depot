# Data model: Steerable axles in Project Depot Physics

## AxleSteer (`omsi-vehicle`, `Axle::steer: Option<AxleSteer>`, from `achse_depot_steer`)

| Field | Type | Unit | Rule |
|---|---|---|---|
| `lock_deg` | f32 | deg | finite, `> 0`, `< 90` |
| `lock_speed_kmh` | f32 | km/h | finite, `>= 0`; 0 = never locks by speed |
| `centring_deg_s` | f32 | deg/s | finite, `> 0` |

Not on the leading section's front axle (known only at spawn, checked there); once per axle.
Invalid declarations are dropped (`steer = None`) with a log line naming the file, the axle and the
reason; the rest of the file is kept.

## SteerAxle (`physics/domain/spec.rs`, per section's `PhysicsSpec`)

`axle` (index of the axle in its section), `lock` (rad), `lock_speed` (m/s, `None` for 0), `centring` (rad/s).
Built from the declarations only when the engine is Depot.

## SteerMode (`physics/domain/steering.rs`)

`BuiltIn` (0), `Locked` (1), `Free` (2), `ScriptAngle` (3). Any other value of the script variable
is `BuiltIn` with one log line per vehicle.

## SteerState (per steerable axle, in the section's `RigidBody`)

| Field | Meaning |
|---|---|
| `angle` | current axle angle `δ_i` (rad, positive to the right) |
| `engagement` | `e_i ∈ [0, 1]`: 1 steering, 0 locked straight |
| `mode` | the `SteerMode` read this frame |
| `script_angle` | the script's angle (rad), used in `ScriptAngle` |

State transitions of `engagement` (target, then ramp at `centring / lock` per second):

```text
BuiltIn:      target = 1 if lock_speed is None or |v| < lock_speed, else 0
Locked:       target = 0
Free:         target = 1
ScriptAngle:  target = 0  (angle follows script_angle at `centring`)
```

Saved with the situation and restored on load; a situation without it starts with `angle = 0`,
`engagement = 1`.

## Turning line (computed, per section and substep)

`y_turn` from rigid axles (weight `F_i`) and steerable axles (weight `F_i · (1 − e_i)`), `F_i` the
static load of `D-1`; the leading section's front axle excluded; `C-6` line when the weight is 0.
Not stored: computed from the previous substep's states (research R-6).

## Script variables (per steerable axle, train-wide axle number `n`)

See [contracts/script-vars.md](contracts/script-vars.md): `Depot_AxleSteer_Mode_<n>` and
`Depot_AxleSteer_Angle_<n>` in, `Depot_AxleSteer_Locked_<n>` and `Axle_Steering_<n>_L/R` out.
