# Contract: script variables of steerable axles

`<n>` is the axle number over the whole vehicle, as in `Axle_Steering_<n>_L` (the leading section's
axles first, then each section behind in order). Variables exist only if the script's varlist
declares them; an undeclared variable reads 0 and is never written. OMSI 2 treats these names as
ordinary user variables.

The game never locks a steerable axle by itself: speed, gear, fault or dashboard logic is the
vehicle's script's, through the mode.

## Written by the script, read by the physics once per frame

| Variable | Values | Meaning |
|---|---|---|
| `Axle_Steer_Mode_<n>` | 0, 1, 2 | 0 basic: opposite to the driver's steering axle, in proportion to the axle's lock (`δ = −(δ_front / lock_front) · lock`, a 6x2-4); 1 locked straight; 2 script angle. Any other value: 0, one log line per vehicle |
| `Axle_Steer_Angle_<n>` | rad, positive to the right | the axle's angle in mode 2, clamped to its lock. Not a number: treated as mode 0 |

The axle moves to its mode's angle at its centring rate (`achse_steer_centring`).

## Written by the physics, read by the script

| Variable | Values | Meaning |
|---|---|---|
| `Axle_Steering_<n>_L`, `_R` | rad | the axle's actual angle `δ_i`, the same for both sides (as today); 0 for a rigid axle of a keyed section |
| `Axle_Steer_Locked_<n>` | 0 or 1 | 1 while the axle is locked straight (mode 1 reached): for a dashboard lamp or a sound |

## Examples

Lock the tag axle (axle 2) above 30 km/h or in reverse:

```text
{frame}
  (L.L.Velocity) 30 >  (L.L.reverse) ||  {if} 1 {else} 0 {endif} (S.L.Axle_Steer_Mode_2)
```

Steer a twin-steer second axle (axle 1) the same way as the front axle, at 80 % of its angle:

```text
2 (S.L.Axle_Steer_Mode_1)
(L.L.Axle_Steering_0_L) 0.8 * (S.L.Axle_Steer_Angle_1)
```

(`reverse` stands for whatever variable the bus's own scripts use for the reverse gear.)
