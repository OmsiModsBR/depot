# Original OMSI 2 mode (OmsiLaunch)

Tracking issue: #459. This page describes what the mode does and what it is allowed to do.
The mode is not released: it stays hidden until the rollout change (OL-31) turns it on, and
nothing in it runs without the player's action.

## What it is

Project Depot can start games with the original OMSI 2 instead of its own engine. It does
this through [OmsiLaunch](https://github.com/OmsiModsBR/OmsiLaunch) (LGPL-3.0-only), a
separate program that prepares an OMSI 2 session, starts it and restores the OMSI 2 files
afterwards. Depot only ever **runs** OmsiLaunch as a separate process. It never links,
copies or ships it; the player installs it from Setup after a confirmation.

* Windows only (OmsiLaunch needs Windows 10 or later, 64-bit, .NET 6 Desktop Runtime x64
  and the .NET 6 runtime x86, and a writable OMSI 2 folder outside Program Files).
* Off by default (Setup, "Original OMSI 2 engine"). The Depot | OMSI 2 switch appears only
  when the flag is on and OmsiLaunch is installed, whole and of a protocol Depot drives.
* One game at a time, across both engines. No telemetry, no session tracking.
* The settings of OMSI 2 mode are Depot's own (`~/.depot/omsi2/settings.json`) and are sent
  at every start; whatever is changed inside OMSI 2 is ignored (OmsiLaunch restores its files).

## Where the code compiles

The module (`omsi-launcher-core/src/original_engine`) is compiled on every desktop platform,
so that its tests (a fake `OmsiLaunch.exe`, synthetic folders and packages) run on a
developer's machine of any system, not only on Windows. Only what needs Windows is Windows-only:

* the user interface exposes the mode only when `cfg!(windows)` (and the rollout flag, or the
  development variable `DEPOT_ORIGINAL_ENGINE=1`);
* the check for links also treats Windows reparse points (junctions) as links;
* the HTTPS client is not built for Android (the mode does not exist there).

CI: only the `windows-workspace` job runs the module's tests (it runs the whole workspace
tests); the Linux jobs check `omsi-app` and clippy, which compile the module but do not run
its tests. There is no macOS or Android CI job; the module uses only the standard
library, `serde_json`, `sha2`, `zip` and (outside Android) `ureq`.

## Install and removal

1. "Install OmsiLaunch…" asks the official repository (`OmsiModsBR/OmsiLaunch`, GitHub
   releases) for the newest release and its `.sha256`. Nothing is downloaded yet; the window
   shows version, license and the SHA-256.
2. "Download" fetches the package and compares it with that SHA-256.
3. The window lists the files the package would add, which of OmsiLaunch's own files are
   replaced, and refuses when a file exists that is not OmsiLaunch's.
4. "Install" extracts into `~/.depot/omsi2/staging`, checks every file against the package's
   own manifest, then moves them into the OMSI 2 folder (backups first, all or nothing).
5. "Remove OmsiLaunch…" removes the files Depot installed (by the record in `~/.depot/omsi2`),
   keeps the player's session profiles, other plug-ins and OMSI 2's own files.

Rules enforced in code (and tested): HTTPS only, official hosts only (every redirect hop
checked), the first addresses must be files of the official repository's releases, size and
time limits, no `..`, absolute, drive, backslash, link, device-name or duplicate paths,
nothing that is not OmsiLaunch's is overwritten or deleted, the OmsiLaunch programs are
checked against the manifest before they are run. The checksum comes from the same release
as the package, so it detects damage and tampering in transit, not a replaced release; the
window shows the SHA-256 and the release page so the player can compare.

## The settings table

OmsiLaunch applies the settings of its own catalog (`/set:<key>=<value>`). Depot keeps one
saved value per key and sends **all** of them at every start, so the original game always
runs with the settings chosen in Depot. A setting that has no key in this table is "Not in
OMSI 2" in the interface and is not sent. A value outside its range, or an unknown key,
stops the start with a plain message. "Import from OMSI 2" reads the player's `options.cfg`
once (read only) using the last column.

The table is generated from `catalog.rs`; a test fails when it differs.
Regenerate with `cargo test -p omsi-launcher-core print_docs_table -- --ignored --nocapture`.

<!-- catalog:start -->
| OmsiLaunch key | Depot setting | Group | Values | Default | Import from `options.cfg` |
|---|---|---|---|---|---|
| `general.alternateView` | - | General | on / off | `false` | `altView` |
| `general.showOwnDriver` | `driver` | General | on / off | `true` | `see_own_driver` |
| `general.showErrorMessages` | - | General | on / off | `false` | `showerrormessages` |
| `general.autoSave` | - | General | on / off | `true` | `noAutoSave` (inverted) |
| `general.currentTime` | - | General | on / off | `false` | `useActTime` |
| `general.currentDate` | - | General | on / off | `false` | `useActDate` |
| `general.currentYear` | - | General | on / off | `false` | `useActYear` |
| `graphics.maxFPS` | `max_fps` | Graphics | whole number 10 to 200 | `60` | `[maxFPS]` |
| `graphics.tileDistance` | - | Graphics | whole number 1 to 20 | `6` | `[performance_tiledistmax]` |
| `graphics.maxObjectDistanceMeters` | `max_obj_dist` | Graphics | whole number 20 to 5000 | `1000` | `[performance_maxObjDist]` |
| `graphics.minObjectScreenPercent` | - | Graphics | number 0 to 10 | `1` | `[performance_minObjSize]` |
| `graphics.minReflectionObjectScreenPercent` | - | Graphics | number 0 to 50 | `5` | `[performance_minObjSizeRefl]` |
| `graphics.maxObjectComplexity` | - | Graphics | whole number 0 to 3 | `3` | `[maxcomplexity]` |
| `graphics.maxMapComplexity` | - | Graphics | whole number 0 to 2 | `2` | `[maxcomplexity_map]` |
| `graphics.sunGlow` | - | Graphics | on / off | `true` | `sunglow` |
| `graphics.loadAllTiles` | - | Graphics | on / off | `false` | `loadAllTiles` |
| `graphics.stencilBuffer` | - | Graphics | on / off | `true` | `no_stencilbuffer` (inverted) |
| `graphics.stencilShadows` | - | Graphics | on / off | `false` | - |
| `graphics.rainReflections` | - | Graphics | on / off | `true` | `no_rain_refl` (inverted) |
| `graphics.humansInRainReflections` | - | Graphics | on / off | `true` | `no_humans_on_rain_refl` (inverted) |
| `graphics.realTimeReflections` | `mirror_refresh` | Graphics | one of economy, full | `economy` | - |
| `graphics.particles` | - | Graphics | enabled, per-emitter maximum, player vehicle only, in reflections | `true,50,false,true` | - |
| `simulation.collision` | `collision_objects` | Simulation | on / off | `true` | `no_collision` (inverted) |
| `simulation.collisionTerrain` | - | Simulation | on / off | `true` | `no_collision_terrain` (inverted) |
| `simulation.collisionVehicles` | `collision_vehicles` | Simulation | on / off | `true` | `no_collision_vehToVeh` (inverted) |
| `simulation.collisionPedestrians` | - | Simulation | on / off | `true` | `no_collision_pedastrians` (inverted) |
| `simulation.ticketSelling` | `boarding` | Simulation | whole number 0 to 2 | `1` | `[ticketselling]` |
| `simulation.maintenance` | `maintenance` | Simulation | whole number 0 to 4 | `0` | `[wear_lifespan]` |
| `simulation.disableAutomaticScheduleAnalysisPopup` | - | Simulation | on / off | `false` | `no_schedAnaPopUp` |
| `simulation.ticketInfo` | - | Simulation | on / off | `true` | `no_ticketinfo_visible` (inverted) |
| `simulation.automaticClutch` | - | Simulation | on / off | `true` | `no_automaticClutch` (inverted) |
| `traffic.randomVehicles` | `ai_max_random` | Traffic | whole number 0 to 1000 | `150` | `[AIMaxCountRandom]` line 1 |
| `traffic.humans` | `ai_max_humans` | Traffic | whole number 0 to 1000 | `150` | `[AIMaxCountRandom]` line 2 |
| `traffic.factorPercent` | `ai_unsched_factor` | Traffic | whole number 1 to 300 | `100` | `[AIUnschedFactor]` |
| `traffic.parkedVehiclesPercent` | `ai_max_parked` | Traffic | whole number 0 to 100 | `50` | `[AIMaxCountParked]` |
| `traffic.scheduledVehicles` | `ai_max_scheduled` | Traffic | whole number 0 to 1000 | `150` | `[AIMaxCountScheduled]` |
| `traffic.scheduledLinePriority` | `ai_priority_scheduled` | Traffic | whole number 1 to 4 | `4` | `[AIPriorityScheduled]` |
| `traffic.passengerFactorPercent` | `pax_density` | Traffic | whole number 0 to 200 | `100` | `[AIPassFactor]` |
| `sound.stereo` | - | Sound | whole number 0 to 100 | `50` | `[sound_stereo]` |
| `sound.maxSimultaneousSounds` | - | Sound | whole number 5 to 1000 | `128` | `[sound_maxcount]` |
| `sound.masterVolume` | `volume` | Sound | number 0 to 1 | `1` | `[sound_vol_master]` |
| `view.driverSmooth` | `driverview_smooth` | View | on / off | `true` | `driverview_smooth` |
| `view.driverMoving` | `head_movement` | View | on / off | `true` | `driverview_moving` |
| `controls.autoCenter` | - | Controls | on / off | `true` | `autoCenter` |
| `controls.reducedSteeringSpeed` | - | Controls | on / off | `false` | `redSteerSpd` |
| `advanced.reducedMultithreading` | - | Advanced | on / off | `false` | - |
<!-- catalog:end -->

## Files Depot writes

| Where | What |
|---|---|
| `~/.depot/launcher.json` | the `original_engine` block (`enabled`, `mode`, `last_mode`) |
| `~/.depot/omsi2/settings.json`, `modules.json` | the saved settings and module choices |
| `~/.depot/omsi2/install-record.json` | what Depot installed (files, folders it made, version, SHA-256) |
| `~/.depot/omsi2/launch/<id>.json` | the launch description of one run, removed when the run ends |
| `~/.depot/omsi2/staging/` | work area of an install or removal, emptied afterwards |

Nothing is written into the OMSI 2 folder except OmsiLaunch's own files at install, and
their removal.
