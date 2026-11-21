# Contract: `achse_depot_steer` in a `[newachse]`

A new axle parameter, written inside the axle's own `[newachse]` like `achse_long`, followed by
three value lines:

```text
achse_depot_steer
15         lock angle, degrees each side (> 0, < 90)
30         lock speed, km/h (>= 0; 0 = never locks by speed)
20         centring rate, degrees per second (> 0)
```

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
achse_depot_steer
15
30
20
```

## Rules

- Read by Project Depot only; OMSI 2 and its derivatives skip it (checked in game, T001). It may
  stand anywhere among the axle's parameters.
- Only the keyword form of an axle can carry it; the old form with bare values cannot.
- Takes effect only when the vehicle runs Project Depot Physics (`[depot_physics]` in the leading
  section's file, or the launcher setting). Under OMSI Classic it is read and ignored, with one log
  line.
- In a section with at least one valid declaration, every other axle is rigid, except the leading
  section's front axle, which the driver steers.
- Each section of an articulated bus declares its own axles in its own file. The scripts number
  axles over the whole vehicle ([script-vars.md](script-vars.md)).
- Invalid (logged, ignored, never corrected): fewer than three numbers after the keyword; a value
  that is not a number or outside the ranges above; given twice in one axle (the second is ignored);
  on the leading section's front axle.
- AI copies ignore it (spec 002, D6).
