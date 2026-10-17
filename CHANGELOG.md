# Changelog

Changes made by the Project Depot team. History inherited from openOMSI (up to the rebranding
release 0.2.17) is frozen in [CHANGELOG_UPSTREAM.md](CHANGELOG_UPSTREAM.md).
Releases are published manually after review; see [docs/VERSIONING.md](docs/VERSIONING.md).

## Unreleased

### Fixed
- The lines panel's timetable follows the session's date like the AI buses do: a tour that runs only in
  term time or only in school holidays (`Holidays.txt`) no longer shows up together with the other, and
  the timetable is read again when the date changes.
- Destination signs, IBIS screens and ticket printer output come back with a saved situation
  (#34): the scripts draw them only when something changes, so the variables alone brought
  them back empty. Each vehicle's `[scripttexture]` images are saved next to the situation as
  OMSI 2 does, `<situation>.osn_<vehicle>_<texture>.dds` (A8R8G8B8, the vehicle counted in the
  order the file lists them), and loaded back after the scripts' init at the file's own size.
  Situations saved by OMSI 2 itself bring their displays back as well. A missing file leaves
  the texture as the scripts draw it; a damaged one leaves it empty with a line in the log.
- A bus stopped under a canopy or a bridge measures the ground under its lift and ramp again,
  not the roof over them: `GetHeightAbovePoint` (OMSI's name, kept for compatibility, though it
  measures under the point) asks the faces the wheels stand on for the highest one at or below
  the point, where the plain height sampler gave the highest face there and the lift could not
  be lowered (#95).
- Splines that declare `[matl_alpha] 2` on a DXT3 / DXT5 texture whose alpha is 255 throughout
  (DavidM's walls and bridges, used across Krefrath) are drawn opaque and write depth again: they
  went into the blended pass without depth writes, so a wall further away showed over the wall in
  front and the road under a bridge showed through its deck. The same holds for surface objects.
- Passengers no longer board an articulated bus by its last door alone (#44): walking up while
  the doors were still shut, everybody made for the rear door with a button, and they stayed there
  once the driver opened the others. With a door open, a shut door with a button now draws nobody
  and people walk to the nearest open one; with every door shut the button door is still the one
  to wait at. (Same change as openOMSI 3bebe136, #2509, MIT.)
- A sound started by a trigger whose `[volcurve]` the script moves right after (a door or
  window clap, a door hit) keeps the loudest volume its curves gave it since the trigger fired,
  as OMSI 2 holds it, instead of falling silent the frame after it starts (#131).
- A bus coming to the player no longer plays the triggers its `{init}` fired before its sounds
  existed (the stock scripts fire `ev_engineshutdown` while setting the engine up); the
  announcements' file triggers are kept.
- A content file can no longer make the game look at another computer or another drive: names
  with a drive or stream (`C:`, `x.bmp:y`) or a NUL resolve to nothing, and a texture named by a
  full path (or a network path) is followed only when it lies inside the content folders.
- A `[mouseevent]` mesh behind another mesh of the bus (a panel, a wall, a window, the steering
  wheel) can no longer be clicked, hovered or turned with the wheel, from any camera: the ray
  through the cursor has to reach it first. Only the outside camera checked this before, and only
  against the mesh's bounding sphere. Meshes a camera does not draw (`[viewpoint]`) neither block
  nor take clicks from it.

### Changed
- `omsi-sim`'s `vehicle.rs` is split by subject into `vehicle/` submodules (the type and its
  loading, the scripts' variables and triggers, motion, the AI's frame, trailers, pose, visuals,
  mesh properties, skinning, radio text, weather, built-in variables and the exporter's winding),
  with its tests in `vehicle/tests.rs`. The code moved unchanged; `vehicle.rs` keeps the
  `VehicleInstance` struct, its construction and update, and re-exports the public items at their
  old paths. The submodules, `articulation` and `physics_bridge` included, import what they use
  instead of `use super::*`.
- `omsi-app`'s crate root no longer gathers its modules' names with `use module::*`: it imports
  what it uses itself, and the modules that reached names through it (`crate::App`, `crate::Args`,
  `crate::start_clock`, the launcher's showroom with `use super::super::*`, …) name the module that
  defines them.
- The last `omsi-app` modules that took the crate root's names with `use super::*` (`app_events`,
  `offscreen`, `startup`, `android`, `input/` and its `mouse`) import them from where they are
  defined, and the crate root keeps only the imports and globs something still uses.
- `actions`, `app`, `player` and `humans_pax` in `omsi-app` import what they use from where it is
  defined instead of `use super::*`; the crate root drops the imports nothing uses any more.
- Nine more modules of `omsi-app` (`camera_util`, `input/keyboard`, `input/menus`, `server`,
  `situation`, `spawn`, `touch`, `weather_setup`, `weather_time`) import what they use from where it
  is defined instead of `use super::*`.
- Ten modules of `omsi-app` (`cli`, `duty_start`, `input_script`, `input/editor_input`,
  `launcher_link`, `memory`, `online`, `services`, `traffic_link`, `world_load`) import what they
  use from where it is defined instead of taking everything from the crate root with `use super::*`.
- `omsi-render` reads its `OMSI_*` variables through `omsi_cfg::flags`, where its 45 remaining
  settings and diagnostics are now declared, so `docs/ENVIRONMENT.md` lists every variable the game
  reads.
- `omsi-app`'s LAN, launcher and plugin modules read their `OMSI_*` variables through
  `omsi_cfg::flags`. What a phone's `env.txt` may set now comes from the register (the variables
  marked `env_txt`, `omsi-render`'s switches among them) instead of a list of its own, and a test
  pins that set. `OMSI_NO_PRESENCE` and `OMSI_TILE_DIST`, which nothing reads any more, are no
  longer taken from `env.txt` (the log lists them as skipped). `docs/ENVIRONMENT.md` lists the
  declared variables and is made from the register; a test fails when it is out of date.
- The rest of `omsi-app` outside LAN and the launcher (the offscreen run, the window's events,
  start-up, settings, cameras, VR, input scripts) reads its `OMSI_*` variables through
  `omsi_cfg::flags`, where 73 more are now declared; the variables the game sets for itself
  (`OMSI_SAFE_GPU`, `OMSI_BACKEND`, `OMSI_CONTENT` on phones) are named through it too. No
  behaviour change.
- `omsi-app`'s traffic, people, timetable and driver modules (`traffic`, `humans`, `schedule`,
  `navigator`, `driver`, `bus_service`, `spawn`, `player`, `on_foot`, `route_arrows`, `career`,
  `services`) read their `OMSI_*` variables through `omsi_cfg::flags`, where 44 more are now
  declared. No behaviour change.
- `omsi-app`'s world modules (`scene`, `tiles`, `lights`, `rain`, `puddles`, `ground_gap`,
  `memory`, `weather_setup`, `world_load`) read their 66 `OMSI_*` variables through
  `omsi_cfg::flags`, where they are now declared. No behaviour change.
- `omsi-cfg` gets `flags`, the register of the `OMSI_*` environment variables: each is declared
  once with the kind of value it takes, who it is for (user, path, diagnostic, test, internal),
  the module that reads it, whether a phone's `env.txt` may set it and what it does. The crates
  outside `omsi-app` and `omsi-render` (`omsi-sim`, `omsi-net`, `omsi-plugin`,
  `omsi-launcher-core`, `omsi-audio`, `omsi-map`, `omsi-script`, `omsi-geometry`, `omsi-cfg`) read
  their 33 variables through it. Tests check that no path or program can come from `env.txt`.
  The reads that went straight to the process environment now go through `omsi_cfg::env`'s
  cache, like the rest. No behaviour change.
- `omsi-app`'s `lan` feature is now off by default, so the game's build no longer carries LAN play,
  the dedicated server or `omsi-net` (as with `plugins` and `omsi-plugin`). The LAN helpers left in
  shared modules are marked as used only with `lan`, and the off twin of `ask_server` goes.
  `cargo build -p omsi-app --features lan,server,plugins` builds everything again
  (`docs/removed/MULTIPLAYER_DISABLED.md`). Multiplayer was already switched off at run time, so
  nothing changes in the game.
- CI (`quality` job): the `omsi-app` tests run with every feature, and a check fails if
  `omsi-net` or `omsi-plugin` reach the game's default build.
- The offscreen run, the game's start-up and the window's graphics set-up in `omsi-app` keep their
  LAN code behind the `lan` feature, and the dedicated server's code (`server.rs`, the server admin
  commands, the server's status page and clock) moves behind the `server` feature. Without `server`,
  `ServerCfg` is an empty type, so the server paths are compiled out; `--server` already stopped
  with a message. `lan` is still on by default. No behaviour change.
- The game's frame and menus in `omsi-app` keep their LAN code behind the `lan` feature: the chat,
  the other players' buses, walkers, name tags and map arrows, the host's clock and weather, the
  server admin list and the session's goodbye. Without `lan`, `lan_off.rs` answers "no session"
  (as `plugins_off.rs` does for plugins). `lan` is still on by default. No behaviour change.
- `omsi-app` gets the features `lan` and `server` (`server` implies `lan`). `lan` is on by default
  for now, so nothing changes yet. The launcher's server browser (the Multiplayer page, the phone's
  Online tab and saved servers, the joined server's banner and clock) is the first part behind `lan`.
  The game's own LAN code follows in the next steps, and then `lan` turns off by default.
- `omsi-launcher-core` gets a `lan` feature, off by default, for the one thing it takes from
  `omsi-net`: checking a LAN join code or address. Without it, `check_join` answers that LAN play is
  not in this build and a duty's `join:` target goes to the game unchecked (the game ignores it).
  `omsi-app` still asks for the feature until its own `lan` feature exists. No behaviour change.
- Helpers the game uses outside LAN play move out of the LAN modules of `omsi-app`: the teleport and
  the fall-through guard to `teleport.rs`, clearing the AI traffic, the vehicle's box
  (`vehicle_box`, which `lan::footprint_of` now wraps) and its indicator state to `traffic_link.rs`,
  and the `~/.depot` folder (`data_dir`) to `settings.rs`. No behaviour change.
- The seven LAN fields of `omsi-app`'s `App` (`lan`, `remotes`, `voice`, `remote_walkers`,
  `inside_remote`, `is_admin`, `remote_added`) move into one `Online` struct, `App::online`, so the
  `lan` feature can switch them off together. No behaviour change.
- `omsi-app` asks `is_lan_client()` and `is_lan_host()` instead of comparing the LAN session's
  `omsi_net::Role` in 22 places (weather, clock, menus, the editor, lists). Without a session both say
  no, as the comparisons did. This prepares the `lan` feature. No behaviour change.
- The plugin code (`plugins.rs`, `plugin_ui.rs` and the `omsi-plugin` crate) moves behind a new
  `plugins` feature of `omsi-app`, off by default, so the game no longer builds it. Plugins were already
  switched off since the first Depot release; `--features plugins` builds them again. Nothing is
  deleted, and the game behaves as before.
- `omsi-app/src/input_script.rs` held 4,900 lines, most of them the game's key, mouse and menu handling; it now
  keeps only the `OMSI_INPUT` script (330 lines). The rest moves to files named for what they do:
  `input/keyboard.rs`, `input/mouse.rs`, `input/menus.rs` and `input/editor_input.rs`, `actions.rs` (game
  actions, vehicles, saving), `weather_time.rs` (weather, METAR, the clock) and `online.rs` (LAN and voice
  each frame). Code moved as is; no behaviour change.
- `vehicle_path` and `human_path`, which check a content path that came from another machine, move from
  `omsi-net` to `omsi_content::remote_path`; `omsi-net` re-exports them, and the game's driver and walker
  figures no longer need the network crate for it. The first step of building the game without
  multiplayer and plugins. Code moved as is; no behaviour change.
- `cargo fmt` (rustfmt defaults, 100 columns) on `omsi-cfg`, `omsi-o3d`, `omsi-model`, `omsi-map`, `omsi-vehicle`,
  `omsi-scenery`, `omsi-texture`, `omsi-timetable`, `omsi-content`, `omsi-script`, `omsi-audio`, `omsi-ui`,
  `omsi-geometry`, `tools/omsi-check` and `omsi-render`; CI's new `rustfmt (formatted crates)` step checks them
  and blocks. Formatting only; no behaviour change.
- The clippy warnings of the ten format crates are fixed (manual `div_ceil` and `is_multiple_of`, index loops,
  needless conversions and borrows, complex types named, doc lists). No behaviour change.
- `omsi-render`: `lib.rs` is split into modules, with the code moved as is and no behaviour change:
  `camera.rs`, `targets.rs` and `material.rs` for the types without logic; `gpu.rs` (adapter, video
  memory, the OpenGL paths) and `lighting.rs`; `pipelines.rs` for the pipelines `Renderer::build` makes
  apart from the main pass's (shadows, prepass, ambient occlusion, fog lamps, mip maps, post passes,
  overlays, upscaling), each in a function of its own; and `renderer.rs` (the `Renderer`, its options
  and its start), `resources.rs` (meshes, textures, materials), `scene.rs`, `frame.rs`, `draw.rs`,
  `shaders.rs` and `surface.rs`. `render_inner` moves to `render.rs` with its passes (shadows, prepass and
  ambient occlusion, sky cube, probe, main pass, reflections, fog lamps, glass, post, present) as
  methods in `passes.rs`, and the tests to `tests.rs` and `tests_units.rs`: no file of the crate is
  over 3,000 lines.
- The HTML engine's interface with the game (`ApiValue`, `HtmlRenderer`, `HtmlRequest`, `PageApi`, `PointerKind`,
  `rebase_css_urls`) moves into `omsi-sim/src/htmlengine/page.rs`, re-exported where it was
  (`vehicle_api`, `htmltex`): the engine no longer reaches into the rest of `omsi-sim`, the first step of the
  `omsi-html` crate. Code moved as is; no behaviour change.
- The HTML engine's tests that need the rest of the game (the vehicle snapshot, the clock, pages loaded from
  disk) move to `omsi-sim/tests/htmlengine_game.rs`; `EngineRenderer::text_of` is public (hidden from the docs)
  for them.
- The HTML engine leaves `omsi-sim` for its own crate, `omsi-html` (`crates/omsi-html`), which depends only on
  `omsi-cfg` and `omsi-texture`; `omsi-sim` re-exports it as `omsi_sim::htmlengine`, so every path stays the same.
  Code moved as is; no behaviour change.

### Added
- "Muffle outside sounds" (Options and launcher, Sound; `muffle_outside` in the settings, off
  by default): from a cab, other vehicles' sounds and the bus's own outside sounds go through a
  low-pass, 450 Hz with the bus shut up to 7.2 kHz with a door or window open.
- `sound_levels <sound.cfg> lint` lists what is likely to sound wrong whatever the state (`[3d]`
  without a range, a loop without a reference value, variable names with spaces, odd
  `[viewpoint]` values) and notes the cab/outside pairs; `OMSI_DEBUG_SOUND` shows
  `Snd_OutsideVol` and the viewpoint of what is heard and warns of a `[3d]` with range 0.
- Lines panel timetables: double-clicking a line on the city map shows its departures by direction and day
  (Monday to Friday, Saturday, Sunday) and, for a chosen departure, when the bus is at each stop, also written
  on the map at the stops. The times come from the map's own timetable (`TTData`), the same figures the AI
  buses run by, and nothing is added to the package. Lines carry a kind of vehicle (bus, tram, metro, rail)
  with a tab for each, and a form of a line can have dates (eras), so only the forms in force on the session's
  date are shown. Select all / Clear, one click ticks both directions, trams, metro trains and trains have
  their own icons on the map, and the manifest can name which rail vehicles are which. Documented in
  `docs/MAP_PACKAGE.md`. Source: maintainer (Leo, `lmonteirotech`), fork lmonteirotech/openOMSI branch
  `local/lines-timetable` (commits `5327426`, `0999942`, `d287e0c`, `161fab8`), MIT. Not validated against
  real OMSI 2 content; only build and automated tests were run. One test of the train icon was corrected
  (it compared the train's triangle count with the bus's, which is lower).
  `docs/MAP_PACKAGE.md` now names `tools/mapgen` of lmonteirotech/openomsi-map-packages as the supported way
  to build packages, and `tools/synthetic-package/make_synthetic_package.py` writes a tiny invented package
  (no OpenStreetMap or OMSI 2 data) for trying the Lines panel.
- City map orientation: a compass on the city map shows where north lies and a click turns the map between
  **Map** (as the package draws it), **North** up and **Heading** up. New setting `nav_orientation` (`map`,
  `north`, `heading`; a row in the game settings) and new manifest key `north` (degrees from the map's up to real
  north, `docs/MAP_PACKAGE.md` sections 2 and 8.3). Dragging, zoom, the street under the pointer and the vehicle
  arrows follow the turn. Packages without `north` behave as before. Source: maintainer (Leo, `lmonteirotech`), fork
  lmonteirotech/openOMSI branch `local/lines-timetable` (the orientation part of `b826977` and `cfb9e86`, without
  its calibration tools and map-specific parts), MIT. Not validated against real OMSI 2 content; only build and
  automated tests were run.
- City map terrain view: a **Terrain** button (and the setting `nav_contours`) shows the contour lines of the terrain
  from a map package that carries them (new line classes 5 and 6 of `docs/MAP_PACKAGE.md` section 4), the height
  written on the index lines, and the height of the street the player is on and of the street under the pointer.
  The view shows only water, rivers, rails, the game's roads and the contour lines. New manifest keys
  `terrain_offset` and `contour` (colour). Packages without contour lines behave as before. Source: maintainer
  (Leo, `lmonteirotech`), fork lmonteirotech/openOMSI branch `local/lines-timetable` (the contour part of
  `d09111f` and `ae77b95`), MIT. Not validated against real OMSI 2 content; only build and automated tests were run.
- City map: the street under the pointer (any street the map's package names, also where the game has no road)
  in a small box, and the speed limit of the street the player is on as a road sign (and the limit of heavy
  vehicles when it is another), from a new line class of the package, "street with limits" (class 4 of
  `docs/MAP_PACKAGE.md` section 4). Packages without that class behave as before. Source: maintainer (Leo,
  `lmonteirotech`), fork lmonteirotech/openOMSI branch `local/lines-timetable` (commits `cae2286` and the limits
  part of `d09111f`), MIT. Not validated against real OMSI 2 content; only build and automated tests were run.
- Offscreen state trace (B0 of the refactoring plan): `OMSI_STATE_TRACE=<file>` makes `depot --offscreen` write,
  for every frame, the player's vehicle, the AI cars, the lights, the rail signals and the duty. A new test runs it
  on a synthetic street (`crates/omsi-app/tests/offscreen_state_trace.rs`), `scripts/offscreen-trace-diff.sh`
  compares a commit with its base, and the CI job `offscreen-traces` (informational for now) runs it on pull
  requests on the software Vulkan adapter. Test and CI only; no behaviour change.
- Round-trip test of `settings.cfg` between the game and the launcher
  (`crates/omsi-app/src/settings/round_trip.rs`): writes the file from one side and reads it
  from the other, in both directions, for the defaults, one value per key and values at the
  edges of the ranges. The differences found at the edges are listed in the test, not fixed;
  see docs/QUALITY.md. The game's test-only `Settings::to_text` now writes every field.
  Test only; no behaviour change.
- CI job `offscreen-golden` (informational for now): the offscreen golden runs on every push and pull
  request on the software Vulkan adapter (lavapipe), with `OMSI_GOLDEN_REQUIRE=1` so a missing adapter or a
  missing golden for it fails instead of skipping. Golden files are now kept per adapter in
  `crates/omsi-app/tests/golden/offscreen_synthetic/`. Test and CI only; no behaviour change.

### Fixed
- A graphics device the driver gives up in the middle of a frame (a driver reset, the card
  removed, the card out of memory) no longer closes the game: the session ends in order (or
  starts again with safer graphics) as it did when the loss was seen between two frames. wgpu
  panics instead of failing in a few places - waiting for the device, reading a buffer back,
  letting the device go at the end - and those panics are now caught and taken as the device
  lost (#48). The card's memory budget is enforced when textures and buffers are made (95 %),
  so running short is an "out of memory" the game gets over, and an "out of memory" that
  switched the multisampling off now also cuts the textures (it was forgotten in the switch).
  `OMSI_FAKE_GPU_ERROR=lost-poll` and `=oom` test both ways.
- AI buses of a depot group whose depot file exists nowhere (Mapa Transcol's garages, whose buses carry
  only `Transcol.hof`) drove with a blank destination sign. They now run with the first depot file of their
  own folder, as a plain group's buses do, and a bus with no depot file at all is logged as a warning (#58).
- The offscreen golden's synthetic root kept the box next to the map, where the game does not look for
  scenery objects, so the picture showed only ground and sky and the "box is in the picture" check failed
  on any adapter. The box now sits in `Sceneryobjects/Golden/` with its mesh in `model/` and its texture in
  `texture/`, as in OMSI 2. Test only.

## 0.2.52 - 2026-10-10

Release notes: [docs/releases/0.2.52.md](docs/releases/0.2.52.md).

### Added
- Golden test for the offscreen run (`crates/omsi-app/tests/offscreen_golden.rs`): runs `depot --offscreen` on a
  synthetic root the test writes itself and checks that the picture is repeatable, shows the scene and matches a
  stored thumbnail within a tolerance. It skips cleanly when the machine has no graphics adapter. It also records
  that the window gives the player's rail to `signal_aspects` while the offscreen run gives none. Test only; no
  behaviour change.
- Navigator map packages: the navigator and the city map can draw a surface (water, green, rivers, names),
  icons, regions, street names and bus lines supplied by an optional per-map package
  (`docs/MAP_PACKAGE.md`), also looked up in the content folder and used for offscreen pictures. New
  settings `nav_surface` and `nav_light`. Source: maintainer (Leo, `lmonteirotech`), fork
  lmonteirotech/openOMSI branch `feat/map-package` (commits `80fb1482`, `dbcb93f9`, `4c7a284d`,
  `d898c8fb`), MIT. Example packages (OpenStreetMap/ODbL data) left out pending licence review. Not
  validated against real OMSI 2 content; only build and automated tests were run.
- Outside camera (F3) gets an independent `outside_fov` setting, 20-120 vertical degrees or Auto. Auto
  preserves the 16:9 horizontal extent on ultrawide windows when no general override is set. Source:
  openOMSI `ea47e092ee6e0a2c472d2c7261ea7a7913e6ca9f` (MIT), selective backport authored by zane_thruston;
  no later upstream architecture imported. See `docs/integrations/OUTSIDE-FOV-ULTRAWIDE.md`.
- Articulated buses can opt into a pusher joint with `[coupling_pitch_offset]` in the rear section's
  `.bus`. Without the key, or with an invalid/zero offset, the baseline puller behaviour is unchanged.
  Source: openOMSI `a97595b1b254cf94abc27bb67844a8a1dfcee2ab` (MIT), authored by zane_thruston
  (contribution confirmed MIT by the maintainer). See `docs/integrations/PUSHER-ARTICULATION.md`.

### Changed
- omsi-sim: the rest of the game reads a vehicle's motion through `VehicleInstance::motion()` (speed, steering,
  accelerations, wheels: `VehicleMotion`), `controls()`, `mass_kg()` and `rigid()`/`rigid_mut()` instead of the
  `physics` and `rigid` fields, so that the physics can change how it keeps them. No behaviour change: the
  physics traces are identical bit for bit.
- omsi-sim: the rigid body (`rigid.rs`) split into a module per stage under `rigid/` (`ground`, `suspension`,
  `wheel`, `tyre`, `body`, `collide`), with its calibration values named in `rigid/params.rs`, each marked with
  where it comes from, and the A/B switches `OMSI_TYRE_SUSPENSION`, `OMSI_NO_WHEEL_SLIP` and
  `OMSI_KINEMATIC_TRAILERS` read once a process in `rigid/toggles.rs`. No behaviour change: the physics traces
  are identical bit for bit.
- omsi-sim: what the physics reads from the scripts (`M_Wheel`, the brake forces, the spring factors) and writes
  back (`Velocity`, `n_Wheel`, `A_Trans_*`, `Wheel_*`, `Axle_*`) goes through one place, `vehicle/physics_bridge.rs`,
  for the rigid body, the kinematic model, the AI and the coupled parts alike; it was written out four times. The
  physics traces gain an AI vehicle with a coupled section and a kinematic crash. No behaviour change: the
  traces are identical bit for bit.
- omsi-sim: a vehicle's `physics` and `rigid` fields become `enum Dynamics` (its rigid body, or the simple model
  renamed `KinematicModel`) next to the `VehicleMotion` the physics writes each frame; `VehicleInstance` gains
  `physics_state()`/`set_physics_state()` (the offscreen run's save and restore) and `set_wheel_travel()` (the LAN's
  copies). No behaviour change: the physics traces are identical bit for bit.
- the smaller crates (geometry, map, vehicle, model, texture, content, cfg, scenery, script, launcher-core, o3d, audio example): comments cleaned of addresses, offsets and mentions of the reference
- omsi-app: comments cleaned of addresses, offsets and mentions of the reference
- omsi-sim: comments cleaned of addresses, offsets and mentions of the reference
  executable; they describe observable behaviour only. Comment text only, no code change.
- Mixer, vehicle sound flags and player input/camera: comments cleaned of addresses and mentions of the reference
  executable; they describe observable behaviour only. Comment text only, no code change.
- `omsi-audio` sound set: comments cleaned of references to addresses of the reference executable; they
  describe observable behaviour only. Comment text only, no code change.
- Articulated buses and trailers: every section is a body of its own. Behind the player's vehicle the rear
  section of an articulated bus and a lorry's trailer run on the same wheel physics as the front, each with
  its own `.bus` mass, inertia, centre of gravity, axles, springs and brakes, held by a joint that follows
  `[coupling_front_character]` (a bus turntable keeps the sections from rolling against each other, a lorry's
  hitch does not). The AI's vehicles, LAN players and trains on rails follow as before, and so does every part
  with `OMSI_KINEMATIC_TRAILERS=1` (A/B). Source: maintainer's fork lmonteirotech/openOMSI branch
  `feat/multibody-articulation`, code authored by zane_thruston, MIT (confirmed by the maintainer); comments
  and docs cleaned of references to addresses of the reference executable. High-risk physics change, not
  validated against real articulated buses. See `docs/FORMATS.md`, `docs/MODDING.md`.

### Removed (reversible records in [docs/removed](docs/removed))
- The vehicle physics' A/B switches `OMSI_TYRE_SUSPENSION`, `OMSI_NO_WHEEL_SLIP` and `OMSI_KINEMATIC_TRAILERS`,
  with the older models they brought back for comparison: the suspension with a wheel mass, a tyre and bump
  stops (and the kerb lift across the tread only it read), every wheel gripping, and the coupled parts
  following kinematically behind the player's vehicle. What the game does by default is unchanged: the
  physics traces are identical bit for bit. `docs/removed/PHYSICS_AB_SWITCHES_REMOVED.md`.
- Wheel seating (`seat_wheels`): the drawn tyres of the rigid-body vehicle are no longer moved up or down
  onto the hub the physics has when the model's `Axle_Suspension` animation does not match the spring's
  travel. A model's 3D animation is shown as configured; the correction only ever applied to the front
  section of an articulated bus. `docs/removed/SEAT_WHEELS_REMOVED.md`.
- In-app updater (check, download, install, `update_*` settings, flags, UI, Android install flow). The
  Settings "About" section shows the version and a link to the releases page. `docs/removed/UPDATER_DISABLED.md`.
- Discord Rich Presence and its borrowed application ID, until Depot registers its own. The community
  Discord links stay. `docs/removed/DISCORD_DISABLED.md`.
- Worker "playing now" presence counter. `docs/removed/PRESENCE_WORKER.md`.
- Multiplayer, dedicated server and Lua plugins are disabled in the client (code kept, unreachable).
  `docs/removed/MULTIPLAYER_DISABLED.md`.
- RAR and 7z installation and automatic ZIP extraction: the installer takes a folder or a `.zip`, which
  is stored under `Archives/` unextracted. Drops `unrar-rs`, `sevenz-rust2`, `reedsolomon-rs` (GPL-3.0
  dependency removed). `docs/removed/RAR_7Z_ZIP_EXTRACTION_REMOVED.md`.
- Donation links and button on the website.

### Security
- ZIP64: capacity and allocation limits in the archive reader (`docs/security/ZIP64_FIX.md`).
- ZIP entry names: length/depth caps and no panic on non-ASCII case folding.
- `.x` models: no panic on non-ASCII bytes, nesting limit; terrain size limit and clamped sampling.
- Patchwork respects the station cap; outline self-crossing test no longer quadratic.
- One shared ceiling for texture sizes and list counts in content parsers (`omsi_cfg::limits`),
  mirrors and dynamic textures bounded.
- HTML/JS engine: parse and DOM depth limits, bounded arrays/strings, no deadlock on self-referencing
  arrays, bounded inline-block layout, `font-size` clamp; script textures and `STDrawRect` bounded;
  script string builtins capped.
- The terrain editor no longer overwrites an unreadable `.terrain`, and writes atomically with a `.bak`.
- Radio and METAR URLs: https only, public hosts only, ICAO code validated and encoded.
- Head tracking listens on 127.0.0.1 by default (`head_tracking_bind` to change).
- Android: `env.txt` is limited to an allow-list, no default signing password, unused permissions dropped.
- Website: Markdown sanitised with DOMPurify, Content-Security-Policy, marked and DOMPurify vendored.
- Release builds use `overflow-checks`.

### Fixed
- Vehicle physics: driving and braking pitch the body as OMSI 2 does. The extra lever of the tyres' pull
  (the wheel radius, taken at the hubs on top of the centre of gravity's height) is now the radius of the
  first axle's wheels, where it was the mean radius of all of them, and the pitch from it acts whenever the
  bus moves or its drive beats the brakes, where it waited for 0.2 m/s: a bus pulling away pitches from the
  first frame the drive overcomes the brakes, and one braking to a stop until it stands. It still stays off
  while the bus stands with its brakes holding. The tuned constant `PITCH_SPEED` is gone. New trace
  scenario T16 (a bus on smaller wheels in front than behind). The physics traces differ in 15 of the 26
  scenarios, each from the frame where the old and the new pitch lever first differ; the largest change of
  pitch is 0.2°, and the handling report is unchanged.
- Vehicle physics: the tyres take the road's grip of the frame they drive in. The grip (`road_grip` of
  `StreetCond` and the air temperature) was set after the physics' step, so a road that froze over or dried
  took hold one frame late, and an articulated bus's rear sections copied the late value. It is now set at
  the start of the rigid body's step. The physics traces differ only there: the scenario whose road freezes
  over and thaws is bit for bit the old code with the weather changed a frame earlier.
- Vehicle viewer in the launcher: the rear sections of an articulated bus no longer stand with their wheels
  above the floor. The viewer gave the bus no ground and ran its rear sections kinematically, hung level with
  their coupling while their wheels went up into the arches by the springs' sag. The viewer now puts the bus
  on a flat floor at z = 0 and on the same wheel physics as the game (the bus and each rear section a rigid
  body on its own springs) with the springs held at full air (`Axle_Springfactor` 1, new
  `VehicleInstance::springs_full`: the scripts' kneeling and levelling left the bus knelt and leaning in the
  picture), and steps it until every body has been still for 0.5 s (at most 10 s) before showing it
  (`crates/omsi-app/src/launcher/showroom.rs`).
- Vehicle physics: the rear section of an articulated bus loses its tyres' hold to a bend too fast for their
  grip, as the front section does. The test (the bend's pull against the grip of all of a body's tyres) worked
  the bend out again from the steering angle of the body's front axle; a rear section with its one axle on its
  own turning line has no angle there, so the test never saw its bend, and on ice its tyres went on passing up
  to several times their grip. It now takes the curvature the body turns on in the slice (the steering's for
  the front section, its joint's for a section behind); the constant `KAPPA_ARM_MIN`, which only the old
  reckoning used, is gone. New trace scenario B7 (an articulated bus into a bend on ice at 13 km/h): the
  physics traces differ only there, from the first frame the rear section's bend outgrows its grip.
- The game opens on the monitor the launcher is on (upstream #2025: after 0.2.2 it always opened on the primary monitor). The launcher passes a point of its window as `--monitor-at X,Y`; without it, or when no monitor holds the point, the primary monitor is used as before. Independent implementation; the start-up window code is otherwise unchanged.
- Timetable: the delay shown on the way between two stops follows where the bus is (the last stop's
  departure and the next arrival shared out by the distance covered) instead of standing still at the delay
  it left the last stop with (upstream #1898). Behaviour taken from openOMSI commit `3c75499` (MIT,
  turbo-devv), reimplemented; the existing test now expects the new value. Parity with the reference still
  to be confirmed by observation.
- Sound: an entry that is about to start playing starts at full volume. DirectSound refuses a request
  over 0 dB and keeps the buffer's last volume, so a `[volume]` over 1 that a curve or condition held at 0
  while silent started "audible" at gain 0 (hand brake, door-closing and indicator sounds; upstream #1851,
  #1852; may contribute to #1418, which still needs a reproduction with the reporter's `sound.cfg`).
  Reimplemented from the behaviour described in openOMSI commit `ea26060` (MIT, turbo-devv); regression
  tests added.
- `omsi-plugin` test `panels_clicks_and_focus_through_omsi_ui` failed on Windows CI because the reload is detected by
  modification time and the rewritten file could keep the old timestamp; the test now sets a newer one. The Lua
  feature itself is unchanged (it stays in the crate; it is only unreachable from the client).

### Project
- Identity and links point to OmsiModsBR/depot, omsimods.com.br and omsimods.com.br/discord; new logos and icons.
- LICENSE keeps the upstream copyright and adds the Project Depot line.
- CI: informational rustfmt, clippy, cargo audit and cargo deny (`docs/QUALITY.md`, `deny.toml`);
  blocking check of the committed Steam binaries' hashes; provenance notes in
  `docs/DEPENDENCY-PROVENANCE.md`.
