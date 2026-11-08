# Contract: the `physics` key in `settings.cfg`

Read by the game (`omsi-app::settings`) and by the launcher (`omsi-launcher-core`,
`omsi-launcher-ui`) through the same `PhysicsMode::parse` ([data model](../data-model.md)).

## `physics`

| Saved value | Read as |
|---|---|
| missing (every file saved before this feature) | `classic` |
| `classic` (and `omsi`, `omsi2`, `omsi_classic`, any case) | `classic` |
| `depot` (and `project_depot`, any case) | `depot` |
| anything else | `classic`, with a warning in the log |

Written only as `classic` or `depot`, and only when the player saves.

## Launcher

- A "Physics" list in the "Simulation" section of the launcher's Settings page ("Simulação" in
  Portuguese; `SIMULATION` in `omsi-launcher-ui/src/window/domain/settings.rs`): `OMSI Classic` (default), `Project Depot Physics`.
- One line under it: a vehicle you drive with `[depot_physics]` in its file uses Depot physics whatever is
  chosen here; mods tuned for OMSI 2 may stand or handle differently in Depot physics.
- Shown only from phase 11; before that the key is read but not offered.
- The game's own menu does not offer it: the engine is chosen for each vehicle at spawn.

## No other way in

No command-line argument and no environment variable select the mode (the request names two ways:
the setting and the `.bus` key). Tests set the mode through the API.

## Tests the contract asks for (`omsi-app/tests/`, `omsi-launcher-core/tests/`)

- every row above, read from a text, gives the expected mode;
- round trip: loading and saving keeps a canonical `physics=` line, and adds none to a file that had
  none until the player saves.
