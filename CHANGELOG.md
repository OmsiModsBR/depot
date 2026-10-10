# Changelog

Changes made by the Project Depot team. History inherited from openOMSI (up to the rebranding
release 0.2.17) is frozen in [CHANGELOG_UPSTREAM.md](CHANGELOG_UPSTREAM.md).
Releases are published manually after review; see [docs/VERSIONING.md](docs/VERSIONING.md).

## Unreleased

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
