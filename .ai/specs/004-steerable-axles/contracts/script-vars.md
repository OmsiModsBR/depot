# Contract: script variables of steerable axles

`<n>` is the axle number over the whole vehicle, as in `Axle_Steering_<n>_L` (the leading section's
axles first, then each section behind in order). Variables exist only if the script's varlist
declares them; an undeclared variable reads 0 and is never written. OMSI 2 treats these names as
ordinary user variables.

## Written by the script, read by the physics once per frame

| Variable | Values | Meaning |
|---|---|---|
| `Depot_AxleSteer_Mode_<n>` | 0, 1, 2, 3 | 0 built-in logic (lock above the lock speed); 1 locked straight; 2 steering, no speed lock; 3 script angle. Any other value: 0, one log line per vehicle |
| `Depot_AxleSteer_Angle_<n>` | rad, positive to the right | the axle's angle in mode 3, clamped to its lock; reached at its centring rate. Not a number: treated as mode 0 |

## Written by the physics, read by the script

| Variable | Values | Meaning |
|---|---|---|
| `Axle_Steering_<n>_L`, `_R` | rad | the axle's actual angle `δ_i`, the same for both sides (as today); 0 for a rigid axle of a keyed section |
| `Depot_AxleSteer_Locked_<n>` | 0 or 1 | 1 while the axle is locked straight (engagement 0): for a dashboard lamp or a sound |

## Examples

Lock the tag axle (axle 2) in reverse:

```text
{frame}
  (L.L.reverse) {if} 1 {else} 0 {endif} (S.L.Depot_AxleSteer_Mode_2)
```

Steer it in proportion to the front wheels (half the angle, opposite):

```text
3 (S.L.Depot_AxleSteer_Mode_2)
(L.L.Axle_Steering_0_L) -0.5 * (S.L.Depot_AxleSteer_Angle_2)
```

(`reverse` stands for whatever variable the bus's own scripts use for the reverse gear.)
