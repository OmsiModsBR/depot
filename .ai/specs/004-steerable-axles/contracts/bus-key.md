# Contract: `[depot_steer_axle]` in a `.bus` file

One section per steerable axle, after the file's `[newachse]` sections, with four value lines:

```text
[depot_steer_axle]
2          axle index in this file, counted from 0 in the order of [newachse]
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
...

[depot_steer_axle]
2
15
30
20
```

## Rules

- Read by Project Depot only; OMSI 2 and its derivatives skip it (checked in game, T001).
- Takes effect only when the vehicle runs Project Depot Physics (`[depot_physics]` in the leading
  section's file, or the launcher setting). Under OMSI Classic it is read and ignored, with one log
  line.
- In a section with at least one valid declaration, every other axle is rigid, except the leading
  section's front axle, which the driver steers.
- The index is local to the file. The scripts number axles over the whole train
  ([script-vars.md](script-vars.md)).
- Each section of an articulated bus declares its own axles in its own file.
- Invalid (logged, ignored, never corrected): fewer than four value lines before the next section or
  the end of the file; a value that is not a number; index out of range; the leading section's front
  axle; the same index twice (the second is ignored); a value outside the ranges above.
- AI copies ignore it (spec 002, D6).
