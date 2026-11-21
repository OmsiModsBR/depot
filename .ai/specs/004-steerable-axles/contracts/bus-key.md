# Contract: `achse_depot_steer_*` in a `[newachse]`

Three new axle parameters, written inside the axle's own `[newachse]` like `achse_long`, each
followed by one value line:

| Parameter | Value | Range |
|---|---|---|
| `achse_depot_steer_angle` | lock angle, degrees each side | `> 0`, `< 90` |
| `achse_depot_steer_lockspeed` | speed above which the axle locks straight, km/h | `>= 0`; 0 = never |
| `achse_depot_steer_centring` | rate at which the axle turns to and from straight, degrees per second | `> 0` |

Example: a 6x2 bus with a steered tag axle that locks above 30 km/h.

```text
[newachse]
achse_long
2.9
...
[newachse]
achse_long
-2.6
...
[newachse]
achse_long
-4.0
achse_maxwidth
...
achse_depot_steer_angle
15
achse_depot_steer_lockspeed
30
achse_depot_steer_centring
20
```

## Rules

- Read by Project Depot only; OMSI 2 and its derivatives skip them (checked in game, T001). They may
  stand anywhere among the axle's parameters, in any order.
- The three go together: an axle with only one or two of them is not steerable, and the log names
  the missing ones.
- Only the keyword form of an axle can carry them; the old form with bare values cannot.
- Take effect only when the vehicle runs Project Depot Physics (`[depot_physics]` in the leading
  section's file, or the launcher setting). Under OMSI Classic they are read and ignored, with one
  log line.
- In a section with at least one valid declaration, every other axle is rigid, except the leading
  section's front axle, which the driver steers.
- Each section of an articulated bus declares its own axles in its own file. The scripts number
  axles over the whole vehicle ([script-vars.md](script-vars.md)).
- Invalid (logged, ignored, never corrected): a value that is not a number or outside the ranges
  above; a parameter given twice in one axle (the second is ignored); a partial set; on the leading
  section's front axle.
- AI copies ignore them (spec 002, D6).
