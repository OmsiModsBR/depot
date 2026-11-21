# Contract: `achse_steer_*` in a `[newachse]`

Two new axle parameters, written inside the axle's own `[newachse]` like `achse_long`, each
followed by one value line:

| Parameter | Value | Range |
|---|---|---|
| `achse_steer_angle` | lock angle, degrees each side | `> 0`, `< 90` |
| `achse_steer_centring` | the most the axle turns per second, between modes and following the steering, degrees per second | `> 0` |

Example: a 6x2-4 bus with a steered tag axle (when it locks is up to the bus's script).

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
achse_steer_angle
15
achse_steer_centring
20
```

## Rules

- Read by Project Depot only; OMSI 2 and its derivatives skip them (checked in game by the maintainer, 2026-11-21). They may
  stand anywhere among the axle's parameters, in any order.
- The two go together: an axle with only one of them is not steerable, and the log names the
  missing one.
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
