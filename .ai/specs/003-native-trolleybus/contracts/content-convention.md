# Contract: plugin-based trolleybus content convention (read side)

What Depot reads from trolleybus content of the plugin-based convention, Depot's native trolleybus
format (decision 1). Described from the content files themselves; Depot never writes them.

## Files and where they are found

| File | Looked up in (first found wins) |
|---|---|
| `TrolleyWireVehicles.ini` (vehicle registry) | Depot content folder `Plugins\`, then OMSI 2 folder `plugins\` |
| `TrolleyWirePlugin.cfg` (global parameters) | same |

Both are `key=value` text with `[section]` headers and `;` comments. Unknown keys are ignored and
listed once in the detailed log. A missing file means no trolleybus support for the session (logged
once).

## Registry: `[vehicle:<name>]` sections

| Key | Use | Phase |
|---|---|---|
| `enabled` | `0` skips the entry | 1 |
| `bus_file` | the player's `.bus`, relative to the OMSI root (match, case-insensitive) | 2 |
| `pole_carrier_bus_file` | the coupled part's `.bus` that carries the poles (articulated) | 2 |
| `pole_base_left_x`, `pole_base_right_x`, `pole_base_forward`, `pole_base_height` | pole pivot, body frame of the carrier | 2 |
| `contact_left_x`, `contact_right_x`, `contact_forward`, `contact_height` | shoe at rest on the hook line, body frame | 2 |
| `shoe_contact_height_offset`, `shoe_visual_drop_m` | shoe height correction | 2 |
| `yaw_factor_left/right`, `pitch_factor`, `shoe_factor_left/right` | output scaling | 2 |
| `manual_capture_distance` | capture distance when raising | 2 |
| `contact_reach_release_error_m`, `contact_reach_release_time` | reach dewirement | 3 |
| `apply_suspension_pose` | read and logged; Depot always uses the drawn body (FR-008) | 2 |

Keys that name model files (`runtime_model`, `control_runtime_model`, `pole_model_file`,
`*_base_object`, `*_shoe_object`), the axle/spring block (`front_*`, `rear_*`) and the AI keys
(`ai_*`, `apply_ai_suspension_pose`) are read but not used in this feature.

A section whose required keys of its phase are missing or unreadable is rejected with the key named
in the log; the vehicle then runs as an ordinary bus.

## Global parameters: `TrolleyWirePlugin.cfg`

| Section / key | Use | Phase |
|---|---|---|
| `[global] spline_keywords` | comma list; a spline is a wire spline when its `.sli` path contains one (case-insensitive) | 1 |
| `[global] wire_height`, `wire_x_left`, `wire_x_right` | wire position inside a wire spline | 1 |
| `[global] pole_length`, `max_pitch`, `max_angle`, `reach_hard`, `model_pole_rest_elevation` | pole limits and rest pose | 2 |
| `[global] exact_capture_max_yaw`, `exact_tracking_max_yaw`, `exact_capture_min_heading` | which wire points a shoe may take | 2 |
| `[global] angle_response`, `pitch_response`, `max_angle_step`, `max_pitch_step` | output smoothing | 2 |
| `[global] lost_reset_frames` | read as frames at 60 fps, used as a time (R8) | 2 |
| `[global] switch_throttle_threshold` | branch demand from the throttle | 4 |
| `[physics] spring_*`, `damping_*`, `contact_force_*` | contact force | 2 |
| `[physics] dewire_*`, `pole_raise_duration`, `pole_lower_duration`, `failed_contact_*` | dewirement, retriever, raise/lower | 2–3 |
| `[physics] complex_speed_limit_kmh`, `complex_overspeed_grace` | special-piece speed limit | 5 |
| `[global] active_map`, `debug_level`, `tile_lock_radius` | ignored (Depot knows the map and the position) | – |
| `[sound]`, `[sparks]` | read, not used (FR-017) | – |

## Map content

| What | Recognised by |
|---|---|
| Wire spline | `.sli` path contains a `spline_keywords` entry |
| Y switch | `.sco` path contains `Troll_strelka_out_02` or `Troll_strelka_out_03` |
| Left / right branch switch | `.sco` path contains `chave_segmento_esquerda_depot` / `chave_segmento_direita_depot` |
| Crossing | `.sco` path contains `OnKrest` or `TwoKrest` |
| Sound marker | `.sco` path contains `Troll_podves_02` (read, not played) |

Names are matched case-insensitively against the whole path, so a folder name counts as the
content convention uses it.
