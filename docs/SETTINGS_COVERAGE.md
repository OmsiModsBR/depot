# Settings coverage inventory

Status: **inventory only**, generated from the source on the commit this file was written. It is the starting point of the public coverage matrix of OPT-12 (#165), under the parent tracker #151. Nothing here says an option is *accepted*: the columns show where a key appears in the code, not that it has a measured effect.

## How to read it

Every key listed is one the game reads from `settings.cfg` and that the round-trip test (`crates/omsi-app/src/settings/round_trip.rs`) varies, so the list is the test's own list of keys.

| Column | Meaning |
|---|---|
| Launcher page | The key name appears in the launcher's source (`crates/omsi-app/src/launcher/`). A dash can also mean the page reaches the key under another name (a text field, a combined control); check before treating it as missing. |
| In-game options | The key name appears in the in-game option menus (`game_lists.rs`, `game_controller_menu.rs`, `menu.rs`). Same caveat. |
| Launcher core | The key is read and written by `omsi-launcher-core` (the launcher's side of the file). |
| Round trip | The key goes game to launcher and back in the round-trip test. "known difference" means the test lists a deviation for it (`KNOWN` in that file); each one is a separate fix. |

Not covered by this inventory, and still to do under #151: the measured effect of each option, its apply timing (live / next session / restart), the original-behaviour comparison, and options that have no setting key yet.

## Keys

| Key | Launcher page | In-game options | Launcher core | Round trip |
|---|---|---|---|---|
| `ai_max_humans` | yes | yes | yes | yes |
| `ai_max_parked` | yes | yes | yes | yes |
| `ai_max_scheduled` | yes | yes | yes | yes |
| `ai_unsched_factor` | yes | yes | yes | yes, known difference (launcher>game, game>launcher) |
| `alt_view` | yes | yes | yes | yes |
| `anisotropy` | yes | yes | yes | yes, known difference (game>launcher) |
| `arrows_switch_cams` | yes | yes | yes | yes |
| `auto_clutch` | yes | yes | yes | yes |
| `auto_shift` | yes | yes | yes | yes |
| `autosave_minutes` | yes | yes | yes | yes |
| `blinker_cancel` | yes | yes | yes | yes |
| `boarding` | yes | yes | yes | yes, known difference (launcher>game) |
| `brake_hold` | yes | yes | yes | yes |
| `camera_collision` | yes | yes | yes | yes |
| `chat` | yes | yes | yes | yes |
| `chat_size` | yes | yes | yes | yes |
| `clouds` | yes | yes | yes | yes |
| `collision_objects` | yes | yes | yes | yes |
| `collision_pedestrians` | yes | yes | yes | yes |
| `collision_vehicles` | yes | yes | yes | yes |
| `ctrl_deadzone` | yes | yes | yes | yes |
| `ctrl_off` | yes | yes | yes | yes |
| `detail_textures` | yes | yes | yes | yes |
| `doppler` | yes | yes | yes | yes |
| `dx9_snow` | yes | yes | yes | yes |
| `drive_keys` | yes | yes | yes | yes, known difference (launcher>game) |
| `driver` | yes | yes | yes | yes |
| `driverview_smooth` | yes | yes | yes | yes |
| `enhanced` | yes | yes | yes | yes |
| `exact_fare` | yes | yes | yes | yes |
| `ff_enabled` | yes | yes | yes | yes |
| `ff_engine_vib` | yes | - | yes | yes, known difference (launcher>game) |
| `ff_fade` | yes | - | yes | yes, known difference (launcher>game) |
| `ff_invert` | yes | yes | yes | yes |
| `ff_road_vib` | yes | - | yes | yes, known difference (launcher>game) |
| `fov` | yes | yes | yes | yes |
| `fullscreen` | yes | yes | yes | yes |
| `get_up` | yes | yes | yes | yes |
| `graphics` | yes | yes | yes | yes |
| `graphics_api` | yes | - | yes | yes, known difference (game>launcher) |
| `hands_in_cab` | yes | yes | yes | yes |
| `head_idle` | yes | yes | yes | yes |
| `head_idle_pace` | yes | yes | yes | yes |
| `head_movement` | yes | yes | yes | yes |
| `head_tracking` | yes | yes | yes | yes |
| `head_tracking_bind` | - | - | - | yes |
| `head_tracking_invert` | - | - | - | yes |
| `head_tracking_invert_pitch` | yes | yes | yes | yes |
| `head_tracking_invert_roll` | yes | yes | yes | yes |
| `head_tracking_invert_x` | yes | yes | yes | yes |
| `head_tracking_invert_y` | yes | yes | yes | yes |
| `head_tracking_invert_yaw` | yes | yes | yes | yes |
| `head_tracking_invert_z` | yes | yes | yes | yes |
| `head_tracking_pitch_sens` | yes | yes | yes | yes, known difference (game>launcher, launcher>game) |
| `head_tracking_port` | - | - | - | yes |
| `head_tracking_roll_sens` | yes | yes | yes | yes, known difference (game>launcher, launcher>game) |
| `head_tracking_x_sens` | yes | yes | yes | yes, known difference (game>launcher, launcher>game) |
| `head_tracking_y_sens` | yes | yes | yes | yes, known difference (game>launcher, launcher>game) |
| `head_tracking_yaw_sens` | yes | yes | yes | yes, known difference (game>launcher, launcher>game) |
| `head_tracking_z_sens` | yes | yes | yes | yes, known difference (game>launcher, launcher>game) |
| `info_bar` | yes | yes | yes | yes |
| `info_bar_collapsed` | yes | yes | yes | yes |
| `info_bar_fps` | yes | yes | yes | yes |
| `info_bar_labels` | yes | yes | yes | yes |
| `info_bar_lowfuel` | yes | yes | yes | yes |
| `info_bar_mode` | yes | yes | yes | yes |
| `info_bar_pos` | yes | yes | yes | yes |
| `info_item_time` | yes | yes | yes | yes |
| `info_item_speed` | yes | yes | yes | yes |
| `info_item_trip` | yes | yes | yes | yes |
| `info_item_odometer` | yes | yes | yes | yes |
| `info_item_temps` | yes | yes | yes | yes |
| `info_item_tank` | yes | yes | yes | yes |
| `info_item_pax` | yes | yes | yes | yes |
| `info_item_line` | yes | yes | yes | yes |
| `info_item_next` | yes | yes | yes | yes |
| `language` | yes | yes | yes | yes |
| `led_glow` | yes | yes | yes | yes |
| `led_mips` | yes | yes | yes | yes |
| `look_sens` | yes | yes | yes | yes |
| `look_smoothing_ms` | yes | yes | yes | yes |
| `maintenance` | yes | yes | yes | yes, known difference (launcher>game) |
| `max_fps` | yes | yes | yes | yes, known difference (launcher>game) |
| `max_obj_dist` | yes | yes | yes | yes, known difference (launcher>game, game>launcher) |
| `metar_station` | - | yes | yes | yes |
| `metar_sync` | yes | yes | yes | yes |
| `min_obj_size` | yes | yes | yes | yes, known difference (launcher>game, game>launcher) |
| `mirror_hud` | - | - | - | yes |
| `mirror_refresh` | yes | - | yes | yes |
| `mirror_size` | yes | yes | yes | yes |
| `momentary_gears` | yes | yes | yes | yes |
| `mouse_right_off` | yes | yes | yes | yes |
| `mouse_sens` | yes | yes | yes | yes, known difference (launcher>game) |
| `mouse_smooth` | yes | yes | yes | yes |
| `mouse_steering` | - | - | yes | yes |
| `msaa` | yes | yes | yes | yes, known difference (launcher>game) |
| `muffle_outside` | yes | yes | yes | yes |
| `name_tags` | yes | yes | yes | yes |
| `nav_ai` | yes | yes | yes | yes |
| `nav_arrows` | yes | yes | yes | yes |
| `nav_contours` | - | yes | yes | yes |
| `nav_follow_view` | yes | yes | yes | yes |
| `nav_light` | - | yes | yes | yes |
| `nav_orientation` | - | yes | yes | yes |
| `nav_surface` | - | yes | yes | yes |
| `navigator` | yes | yes | yes | yes |
| `navigator_corner` | yes | yes | yes | yes, known difference (launcher>game) |
| `night_brightness` | yes | yes | yes | yes |
| `notes` | yes | yes | yes | yes |
| `old_steering` | yes | yes | yes | yes |
| `outside_fov` | yes | yes | yes | yes |
| `pad_steer_linear` | yes | yes | yes | yes |
| `pad_steer_smooth` | yes | yes | yes | yes |
| `pax_density` | yes | yes | yes | yes, known difference (launcher>game, game>launcher) |
| `pax_voices` | yes | yes | yes | yes, known difference (launcher>game) |
| `pedal_brake` | yes | yes | yes | yes, known difference (launcher>game) |
| `pedal_throttle` | yes | yes | yes | yes, known difference (launcher>game) |
| `post_aa` | - | - | - | yes |
| `precision_zoom` | yes | yes | yes | yes |
| `red_steer_spd` | yes | yes | yes | yes |
| `reflections` | yes | yes | yes | yes |
| `render_scale` | yes | yes | yes | yes, known difference (launcher>game) |
| `right_stick_look` | yes | - | yes | yes |
| `seat_pitch_deg` | yes | - | yes | yes |
| `seat_x` | yes | - | yes | yes, known difference (launcher>game) |
| `seat_y` | yes | - | yes | yes, known difference (launcher>game) |
| `seat_z` | yes | - | yes | yes, known difference (launcher>game) |
| `shadow_blobs` | yes | - | yes | yes |
| `smoke` | yes | - | yes | yes |
| `smoke_in_mirrors` | yes | - | yes | yes |
| `smoke_max` | yes | - | yes | yes |
| `smoke_user_only` | yes | - | yes | yes |
| `shadow_casters` | yes | yes | yes | yes |
| `shadow_size` | yes | yes | yes | yes, known difference (launcher>game) |
| `shadows` | yes | yes | yes | yes |
| `show_fps` | yes | yes | yes | yes |
| `ssao` | yes | yes | yes | yes |
| `steer_look` | yes | yes | yes | yes |
| `steer_look_angle` | yes | yes | yes | yes |
| `steer_look_response` | yes | yes | yes | yes |
| `steering_linear` | yes | yes | yes | yes |
| `texture_compression` | yes | yes | yes | yes |
| `texture_memory` | yes | yes | yes | yes |
| `time_speed` | yes | yes | yes | yes, known difference (launcher>game) |
| `time_sync` | yes | yes | yes | yes |
| `tooltips` | yes | yes | yes | yes |
| `triple_bezel_mm` | yes | yes | yes | yes |
| `triple_distance_mm` | yes | yes | yes | yes |
| `triple_eye_height_mm` | yes | yes | yes | yes |
| `triple_fov_deg` | yes | yes | yes | yes |
| `triple_hud_center` | yes | yes | yes | yes |
| `triple_left_angle_deg` | yes | yes | yes | yes |
| `triple_right_angle_deg` | yes | yes | yes | yes |
| `triple_screen` | yes | yes | yes | yes |
| `triple_span` | yes | yes | yes | yes |
| `triple_width_mm` | yes | yes | yes | yes |
| `ui_opacity` | yes | yes | yes | yes, known difference (launcher>game) |
| `ui_scale` | yes | yes | yes | yes |
| `ui_scale_window` | yes | yes | yes | yes |
| `voice_chat` | yes | - | yes | yes |
| `vol_ai` | yes | yes | yes | yes, known difference (launcher>game, game>launcher) |
| `vol_scenery` | yes | yes | yes | yes, known difference (launcher>game, game>launcher) |
| `volume` | yes | yes | yes | yes, known difference (game>launcher, launcher>game) |
| `vr` | yes | yes | yes | yes |
| `vr_desktop_mirror` | yes | yes | yes | yes |
| `vr_head_smoothing_ms` | yes | yes | yes | yes, known difference (game>launcher) |
| `vr_mirror_rate` | yes | yes | yes | yes, known difference (game>launcher) |
| `vr_scale` | yes | yes | yes | yes |
| `vsync` | yes | yes | yes | yes |
| `wheel_lock` | yes | yes | yes | yes |
| `wheel_range` | yes | yes | yes | yes, known difference (launcher>game) |
| `sun_glow` | yes | yes | yes | yes |
| `windy_trees` | yes | yes | yes | yes |

## Keys with no UI string match (6), checked by hand

| Key | What it is | Where it is reached |
|---|---|---|
| `head_tracking_port` | UDP port of the head-tracking listener | File only; documented in the user guide (opentrack, port 4242) |
| `head_tracking_bind` | Address the listener binds to (empty means this computer only) | File only on purpose; written by hand, documented in the user guide, with a log warning when it opens the port to the network |
| `head_tracking_invert` | Older combined invert text | Superseded by the six `head_tracking_invert_*` switches, which the launcher shows; kept for old files |
| `mirror_hud` | Mirror panels over the picture: 0 off, 1 right, 2 left | In game by Ctrl+M and the panel editing keys, not by a settings screen; documented in the user guide |
| `mouse_steering` | Steering with the mouse | Toggled in game with its key and remembered; the launcher reads and writes it through `omsi-launcher-core` |
| `post_aa` | Post-process anti-aliasing: `fxaa`, the enhanced renderer's, or `off` | File only; documented in the user guide. No screen offers it |

Whether `post_aa` and the head-tracking port and bind should get a screen is a product decision for #151 (OPT-04 and OPT-08); they are not missing by accident.

## Items reported by players (comment on #151), checked against the code

- *Languages that exist but do not appear in the language list:* every language that has texts in `crates/omsi-app/locales/app.yml` is in the launcher's list (27 entries). Not checked: content languages of an OMSI 2 installation that have no interface texts.
- The other items in that comment (remembered livery and fleet number, triple screen, live weather, head tracking, key hints by layout, assignable vehicle buttons, time display opacity, pre-world traffic screen, editable date) are not checked yet.

## Regenerating

The inventory is derived from `VARIED` and `KNOWN` in `round_trip.rs` and string matches in the launcher, `game_*.rs`, `menu.rs` and `omsi-launcher-core`. When an option is added or a known difference is fixed, update this file in the same change.
