# Traffic and crowd density diagnostics

Reproduction and reading guide for issue #613 (AI traffic and people do not get denser with
the maximum options). This page describes what Depot logs; it makes no claim about what
OMSI 2 does.

## Capture

1. Set the AI options to their maximum (road traffic factor, maximum non-timetable vehicles,
   maximum people, passenger factor) and raise the traffic count in the Start dialog.
2. Start Depot with `RUST_LOG=debug`, drive for a few minutes on the same route, then open
   `~/.depot/game.log`.
3. Repeat with the options lowered and compare the lines.

## `traffic density:` fields

| Field | Meaning |
| --- | --- |
| `base` | Vehicles asked for around the player (Start dialog / `--traffic`). The options do not change it. |
| `map_hour` | The map's own density for the hour (0 to 2). |
| `unsched_factor` | Road traffic factor option (0.01 to 3.0). |
| `density` | `map_hour * unsched_factor`. |
| `road_lanes`, `road_scale` | Street lanes near the player and the 1 to 4 scale derived from them. |
| `street_target` | `base * density * road_scale`, capped by `cap_random`. |
| `cap_random` | Maximum non-timetable vehicles option. It is only an upper limit. |
| `random_on_road`, `dormant`, `buses_on_road` | What exists now. |
| `created_in_pass` | Cars made by the last population pass; high with a flat population means cars come and go (churn). |

If `street_target` stays far below `cap_random`, raising the cap cannot make traffic denser:
only `base`, `density` and `road_scale` do.

## `crowd:` fields

`base_pedestrians` (fixed), `pavement_lanes` and `crowd` (1 to 3 scale from the pavements
around), `density` (hour curve times the passenger factor, up to 3), `cap_people` (maximum
people option), `cap_walkers` (half of it), `pool_used` (people existing) and
`target_walkers` (what the street walkers are brought up to).
