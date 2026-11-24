# Data model: Steerable axles in Project Depot Physics

## AxleSteer (`omsi-vehicle`, `Axle::steer: Option<AxleSteer>`, from `achse_steer_*`)

| Field | Type | Unit | Rule |
|---|---|---|---|
| `lock_deg` (`achse_steer_angle`) | f32 | deg | finite, `> 0`, `< 90` |
| `centring_deg_s` (`achse_steer_centring`) | f32 | deg/s | finite, `> 0` |

Both parameters or none (a partial set is invalid). Not on the leading section's front axle (known only at spawn, checked there); once per axle.
Invalid declarations are dropped (`steer = None`) with a log line naming the file, the axle and the
reason; the rest of the file is kept.

## SteerAxle (`physics/domain/spec.rs`, per section's `PhysicsSpec`)

`axle` (index of the axle in its section), `lock` (rad), `centring` (rad/s).
Built from the declarations only when the engine is Depot.

## SteerMode (`physics/domain/steering.rs`)

`Basic` (0), `Locked` (1), `ScriptAngle` (2). Any other value of the script variable is `Basic`
with one log line per vehicle.

## SteerState (per steerable axle, in the section's `RigidBody`)

| Field | Meaning |
|---|---|
| `angle` | current axle angle `δ_i` (rad, positive to the right) |
| `mode` | the `SteerMode` read this frame |
| `script_angle` | the script's angle (rad), used in `ScriptAngle` |

The angle's target (reached at `centring`):

```text
Basic:        σ · s · lock, s = κ / κ_max, σ = −1 behind the turning line, +1 ahead, 0 on it;
              behind a joint s = clamp(κ_joint / κ_max, −1, 1)
Locked:       0
ScriptAngle:  script_angle, clamped to lock
```

(Changed 2026-11-21, in PR #820: the engagement `e_i` is gone; the turning line follows the angle
itself, see below.)

Saved with the situation and restored on load; a situation without it starts with `angle = 0`.

## Turning line (computed, per section and substep)

`y_turn` = the mean of where each axle points on the body's middle, `y_i − tan(δ_i) / κ` (`y_i` for
a rigid axle), weighted by its static load `F_i` of `D-1`; the leading section's front axle
excluded; the `C-6` line when every other axle steers. Not stored: computed from the previous
substep's curvature and angles (research R-4, R-6).

## Script variables (per steerable axle, train-wide axle number `n`)

See [contracts/script-vars.md](contracts/script-vars.md): `Axle_Steer_Mode_<n>` and
`Axle_Steer_Angle_<n>` in, `Axle_Steer_Locked_<n>` and `Axle_Steering_<n>_L/R` out.
