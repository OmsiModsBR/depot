# Contract: script variables of a declared trolleybus

Written by the game each frame, after the bodies are placed and before the meshes animate (scripts
see them on the next frame). Created when the vehicle's scripts do not declare them. One set per
vehicle: coupled parts animate from the lead's variables. Left = `lev`, right = `prav`.

Angles are degrees in the carrier's body frame; positions are metres in the body frame.

## Written by the game

| Variable | Meaning | Phase |
|---|---|---|
| `shtanga_manual_command_left/right` | 1 while the pole is fitted to the vehicle (the scripts copy it into the pole's visibility control) | 2 |
| `shtanga_lev_rot`, `shtanga_prav_rot` | pole yaw | 2 |
| `shtanga_lev_pitch`, `shtanga_prav_pitch` | pole elevation above the hook line | 2 |
| `shtanga_lev_shoe_rot`, `shtanga_prav_shoe_rot` | shoe rotation relative to the pole | 2 |
| `shtanga_lev_shoe_pos_x/y/z`, `shtanga_prav_shoe_pos_x/y/z` | shoe position | 2 |
| `shtanga_contact_force_left/right` | contact force, N | 2 |
| `shtanga_wire_present` | 1 when both shoes are on the wire | 2 |
| `shtanga_wire_segment`, `shtanga_wire_progress` | span and position of the left shoe (diagnostic) | 2 |
| `shtanga_dewired_left/right` | 1 from dewirement until the pole is back on the hook | 3 |
| `shtanga_spark_left/right` | always 0 (FR-017) | 3 |
| `shtanga_speed_limit_active`, `shtanga_complex_overspeed` | inside a special piece / above its speed limit | 5 |
| `shtanga_engine_trip`, `shtanga_engine_trip_latched` | traction trip asked of the vehicle script | 5 |
| `shtanga_plugin_heartbeat` | counts up each frame the simulation runs | 2 |

## Read by the game

| Variable | Meaning | Phase |
|---|---|---|
| `cp_SHTANGALEV`, `cp_SHTANGAPRAV` | pole visibility control as the script set it | 2 |
| `shtanga_manual_grab_left/right` | the player holds the rope: raise or lower by hand | 2 |
| `shtanga_recolher_cmd` | retract both poles | 3 |
| `shtanga_switch_pedal` | branch demand (compared with `switch_throttle_threshold`) | 4 |
| `shtanga_wire_reset` | clear the pole state (back to the hooks) | 2 |

Variables the convention's scripts compute themselves from these (`*_pitch_v`, `*_rot_v`,
`*_hook_k`, `rope_*`, `cp_zummer`, warning lamps) are left to the scripts.

The exact meaning of each written value is checked in phase 2 against the four declared vehicles in
the game before the PR is opened; any correction updates this contract in the same PR.
