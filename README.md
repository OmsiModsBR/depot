<p align="center">
  <img src="assets/logos/pd-black.png" alt="Project Depot" width="640">
</p>

# Project Depot

Independent repository: https://github.com/OmsiModsBR/depot

Current Depot release: **[0.3.0](https://github.com/OmsiModsBR/depot/releases/tag/v0.3.0)** (2026-10-30), based on openOMSI 0.2.16. Previous releases: [0.2.52](https://github.com/OmsiModsBR/depot/releases/tag/v0.2.52) and [0.2.17](https://github.com/OmsiModsBR/depot/releases/tag/v0.2.17) (rebranding only).

Historical source baseline from [openOMSI](https://github.com/openOMSI-org/openOMSI): `v0.2.16` (`2026-10-07`), commit
`c85b2e0dda13d219fd12d0496c653d20cdb54145`.

Maintenance policy: **stable base + selective upstream integration**.
See [baseline provenance and integration policy](DEPOT_BASELINE.md).
Build/test evidence and remaining limits are recorded in
[baseline validation](docs/BASELINE-VALIDATION.md).

> [!WARNING]
> **Early release. Expect bugs.** Project Depot is in an early stage of development: things may be
> missing, broken or change between versions. Please report problems in
> [Issues](https://github.com/OmsiModsBR/depot/issues).

**Project Depot** began as an independent project derived from
[openOMSI](https://github.com/openOMSI-org/openOMSI), a Rust recreation of the bus
simulator **OMSI 2**. It retains that technical foundation: 64-bit, multithreaded,
with a modern renderer (Vulkan / DirectX 12 through wgpu) and an optional DirectX 9 renderer on Windows.

The project's explicit goal is to **achieve 1:1 compatibility with OMSI 2**,
including its existing maps, buses, scenery, scripts and mods. This is a development
target, not a claim that complete compatibility has already been achieved.
Alongside that goal, **evolutions to the simulator's physics and simulation are
a development priority**, while preserving compatibility and a stable base through
selective upstream integration.

## What's new in 0.3.0

**Depot 0.3.0** (2026-10-30) is the first release numbered by hand. The full list, with the pull
request behind every item, is in the [changelog](CHANGELOG.md#030---2026-10-30); the
[release notes](docs/releases/0.3.0.md) summarise it. Highlights:

* **DirectX 9 renderer (D3D9Ex), off by default.** Settings → Graphics API offers "DirectX9 (D3D9Ex)" on
  Windows ("Automatic" never picks it). It draws the scene and its materials, lamps and light maps, the
  sky with clouds and stars, mirrors, light coronas, smoke and particles, the interface and the maps, with
  an optional snowfall. Graphics stays at Vanilla under it. It is new: its look is still to be compared
  with the original in the game.
* **New in-game interface.** A new Esc menu and the pages behind it, the Alt bar (a tap of Alt, never
  pauses the game) with *Add vehicle* and the rest of its buttons, a new loading screen, an end-of-trip
  report, and windows in the Windows 11 look.
* **Information bar (Shift+Y)** redesigned, with Full, Two-tier, Split and Custom layouts, a position and
  a frame-rate part.
* **Navigator and city map.** Compass, terrain view with contour lines, street names under the pointer,
  lines panel with timetables, a top-view minimap option and new vehicle pictograms.
* **Launcher redesign.** Windows 11 look with Maps as the start page, Vehicles grouped by manufacturer,
  Profile and Sessions in one page, a Timetable planning page, Setup and Settings rebuilt, accent colours, a sidebar or
  a top bar, tooltips on every button, and a launcher that opens at once.
* **Diagnostics.** Integrity check of maps and vehicles (also against a baseline folder), the AI traffic tab
  with editing of a map's AI lists and parking lists, and a detailed log switch.
* **Vehicle physics and passengers.** The vehicle physics refactor is complete; the weight of the
  passengers aboard is added; steering linearity is on by default; articulated buses no longer fold into an L
  at speed; AI vehicles steer by their own `.bus` files; passengers use every door of an articulated bus,
  keep the height of a high platform and complain again when the saloon is too dark.
* **Performance and safety.** No more drop to a few frames a second while paused in traffic; a content file or
  map package can no longer point the game at another drive or computer; mods install from a `.zip` only.
* **Removed.** LAN play, the dedicated server, voice chat and plugins, switched off since the first Depot
  release, are gone from the repository.
* **Hidden for now:** an original OMSI 2 mode in the launcher (off by default, Windows only).

Earlier releases: **0.2.17** was only the rebranding (no gameplay, renderer, physics, simulation, scripting
or multiplayer changes; [release notes](docs/releases/0.2.17.md)), and **0.2.52** was the first with behaviour
changes: articulated buses and trailers as separate bodies, refactored vehicle physics, an independent
outside-view field of view, navigator map packages, a pusher joint, timetable and sound fixes
([release notes](docs/releases/0.2.52.md)). Several of these are validated only by automated tests, and the
physics changes are not shown to match OMSI 2 yet: 1:1 compatibility remains a development goal, not an
achieved result.

<!-- screenshots: see "Screenshots" below -->

> [!IMPORTANT]
> **Project Depot needs an original copy of OMSI 2.** It contains no game content of its own: it
> plays on the maps, vehicles and other files of an installed OMSI 2 and **will not start without one**.


## Feature highlights

How to read the labels below. **1:1 goal**: the feature is meant to behave like OMSI 2; it is checked by
automated tests and traces, and is not claimed identical until it has been compared in the game.
**Deliberate improvement**: it differs from the original on purpose and is recorded as such.
**Depot addition**: OMSI 2 has nothing equivalent. **Experimental**: new, off by default or not yet
compared with the original in the game. Every item below is in the 0.3.0 release; the pull request behind
each line is in the [changelog](CHANGELOG.md#030---2026-10-30).

### Vehicle physics and simulation

*Project Depot has two aims side by side: configuration compatibility and 1:1 behaviour with OMSI 2, and a
better simulation. It does not chase purely visual extras or features for their own sake; the improvements it
makes are real increments in the simulation, recorded as such. Labels: **1:1 goal** for corrections towards the
original, **deliberate improvement** where the Depot goes beyond it on purpose.*

Each item gives what you can feel at the wheel first, then the technical side.

* **Articulated buses and trailers are real, separate bodies** *(deliberate improvement; since 0.2.52,
  refined in 0.3.0)*.
  *You feel:* the rear section has weight of its own and follows the front one through the joint instead of
  being glued to it: it trails in a bend, settles over a crest or a dip, and the bellows stay joined to the rear
  body instead of opening a gap on a grade. A pusher articulated bus no longer folds into an "L" when you turn
  sharply at 30-50 km/h on uneven ground: the rear tyres grip only as much as the road gives and slide beyond it.
  *Technically:* every section has its own mass, inertia, centre of gravity, axles, springs and brakes, held by a
  joint that follows `[coupling_front_character]`; a coupled section with no such key is hitched by a free ball.
* **Passengers weigh on the player's bus** *(deliberate improvement)*.
  *You feel:* a full bus sits lower on its springs, gathers speed more slowly, takes longer to stop under the same
  brakes and moves in pitch and roll like a heavier body; an empty one is livelier.
  *Technically:* each person's `.hum` `[mass]` adds to the mass of the section they stand in, and the
  damping follows the heavier body.
* **Steering you can predict.** *Steering linearity* is on by default *(deliberate improvement)*.
  *You feel:* the steering keys turn the wheel at the same steady pace whatever your speed, so a tap at 60 km/h
  is as controllable as a tap at 10; with the switch off (Driving help) the keys turn it more slowly the faster
  you go, as before. *Mouse steering* stops at the window's edge and keeps the wheel where the mouse left it
  *(1:1 goal)*: moving the mouse on past the edge used to throw a bus into a slide and a spin at 30-50 km/h.
  A bus whose file asks for more than 60° of lock now gets it and turns as tightly as its own file says.
* **Inertia that matches the file.** *(1:1 goal)* `[momentofintertia]` is read in OMSI's y-up order for the
  player's bus again.
  *You feel:* a bus with a larger roll value no longer turns in too slowly and leans too far in a curve.
* **AI vehicles drive like their own vehicles.** *(1:1 goal)*
  *You feel:* traffic turns about the pivot, with the lock and the axle behaviour its `.bus` file gives, so a
  long vehicle swings like a long vehicle and a tight-steering one turns tight, and at speed none of them
  turns harder than 20 m/s² sideways; at a junction they still turn as far as the bend needs to keep to their lane.
* **Ground under the lift and ramp.** A bus stopped under a canopy or a bridge measures the ground under its lift
  and ramp, not the roof over them, so the lift can be lowered. **Saved situations** bring back destination
  signs, IBIS screens and ticket-printer output.
* **Sound.** *You hear:* door and window claps at the volume their curves gave them instead of falling silent the
  frame after they start; no stray engine-shutdown sound when a bus arrives. A *Muffle outside sounds* low-pass
  for the cab is available *(Depot addition, off by default)*. The `sound_levels` lint helps modders find what
  is likely to sound wrong.
* **No stutter when paused in traffic.** *You feel:* nothing, which is the point: the game no longer drops to a
  few frames a second while paused with traffic around, because silent voices now end at zero.
* **The refactor underneath.** The vehicle physics is one rigid-body model with a single bridge to the scripts'
  variables; the older A/B switches and models are gone (and `--physics simple`). Bit-exact physics traces guard
  it, so a change that moves a vehicle by a hair shows up in review.
* **Other corrections:** `[mouseevent]` meshes behind another mesh can no longer be clicked through it;
  `[animparent]` is no longer guessed (a wrong name is logged so the model can be fixed).

*Not claimed:* these changes are supported by automated tests and traces. The ones marked 1:1 goal have not yet
been shown identical to OMSI 2 in a recorded comparison in the game, and the deliberate improvements are by
definition not meant to be. Describe them as such.

### Passengers and people

* Passengers use **every door of an articulated bus**: they no longer all wait at the last door with a button.
* At a stop on the left of a bus with doors on both sides they go to the nearest door on their own side while
  the doors are shut, and no longer walk across the bus.
* Passengers who get off at a **high platform (BRT)** keep the height of the platform's path instead of sinking
  to the road.
* At night they **complain again that the saloon is too dark** when its light is off; darkness is the sky's light
  alone, so lit stops no longer hide it.

### DirectX 9 renderer *(experimental, off by default, Windows only)*

* **What it is for.** Compatibility and performance on older hardware, drawing the picture as close as possible
  to OMSI 2 within the graphics options OMSI 2 itself has: no extras. Settings → Graphics API → "DirectX9
  (D3D9Ex)"; "Automatic" never picks it, and VR is not available under it.
* **How it was built.** A separate crate, `omsi-render-d3d9`, with a GPU-less test backend, Direct3D 9Ex device
  bring-up with loss handling and Shader Model 3.0 shaders compiled at run time; the game's renderer keeps its
  own books and the DirectX 9 device draws and presents the window.
* **What it draws:** meshes and textures (RGBA with mip chains, DXT1/3/5), vanilla shading with fog and the alpha
  kinds, the materials' maps (light maps, night maps, environment reflections with masks, bump maps, specular),
  the lamps and spots of the map and of vehicles, the tile light maps, the sky dome by time of day with the
  weather's **clouds and the night's stars**, **mirrors**, **light coronas** (headlights, beacons, street lamps,
  fog cones), **smoke and particles**, the vehicle shadow, the new interface, the minimap and the city map, and
  an optional **snowfall**. Multisampling, anisotropic filtering and alpha-to-coverage are handled so that
  cut-out lettering and fences keep clean edges.
* **Options follow what OMSI 2 has.** Graphics stays at Vanilla and fixed; quality presets and improvements
  stay hidden; settings the renderer cannot honour show as fixed lines with the reason.
* **Honest status.** New in 0.3.0. Its look is still to be compared with the original in the game; the clouds,
  stars, snow and rain streaks in particular are not yet confirmed. Nothing here widens the supported hardware.

### The launcher, rebuilt in the Windows 11 look

* **How it was built.** A light program of its own (`omsi-launcher-ui`), no web view, that opens at once: it no
  longer builds the game's 3D renderer before showing its window (that used to take most of a minute on a slow
  driver). Previews are drawn by the game's own renderer in a helper process, so a bus looks in the launcher
  exactly as it does in the game.
* **Look and refinement.** Fluent greys, thin outline icons, rounded cards with hairline strokes, an optional
  accent (Amber by default, plus eight more), sidebar or top bar, plain-dark or scene background, rounded or square
  corners, light motion in Windows 11's timings (nothing bounces, nothing moves at rest), a tooltip after one
  second on every button, no text under 12 physical pixels, and its own minimize, maximize and close.
* **Pages.** *Maps* (start page: the library, the picture and description, the start card with date, start
  point, weather, line and tour), *Vehicles* (grouped by manufacturer with rendered previews, paints, depot files
  and "Use for the next start"), *Timetable* (planning with a map of the real lanes the trips drive, a schematic
  and the trips), *Profile and Sessions*, *Mods*, *Controls*, *Diagnostics*, *Settings* and *Setup*.
* **First run and status.** With no OMSI 2 folder saved, Setup opens with an InfoBar that says what to do; Start
  is dimmed (with the reason in its tooltip) until the folder is complete.
* **Mods** install from a `.zip` only and are used in place from `Archives`, never unpacked.
* **One game at a time.** The launcher steps aside to the system tray while a game runs.

### Timetables: planning in the launcher, Skip stop in the game

* **Timetable page (launcher).** A planning page with three tabs:
  * **Map** draws the map's streets with its lines on the very lanes their trips drive, in both directions and in
    the lines' colours (lines that share a road run side by side). A stretch whose road the map or its timetable
    does not have is **left out, never drawn as a straight line from stop to stop**, and the map says so. Each
    line has a switch (or all at once); you can search a stop by name (even on a map that does not say where its
    stops stand) or a street by the map's street-name signs, and for a stop you see the lines that serve it and
    their next departures on the start's date and time, *Show only these lines* and *Edit this line's trips*.
  * **Schematic** draws the chosen line as a diagram in either direction: the stops it calls at in order, the
    minutes between them from its trip profile, the other lines at each stop, the whole route's time and a table
    of interchanges with those lines' next departures.
  * **Trips** is the editor: the line's tours with new, copy and delete, and each tour's trips with departure,
    route, first and last stop, profile and **AI group** (chosen from the map's own `ailists.cfg`).
  Stops, streets and routes are read from the map; only departures change, saved in the content folder, and the
  original OMSI 2 files are never touched. The page always follows the map chosen on Maps.
* **Honest times.** A trip's stop times in the line lists (Maps, Drive, Timetable and `--cli lines`) now follow the
  trip's profile as the game times it, manual arrival and departure times included.
* **In the game.** The running timetable marks served stops and stops to come with outline icons and a stop
  skipped or missed with a muted sign of its own, and offers **Skip stop** *(deliberate improvement)*. The end-of-trip report lists every stop with planned and actual times.

| | Before 0.3.0 | Now |
| --- | --- | --- |
| Planning a line | A list editor of lines, tours and trips (*Save all*, *New line*, *Repeat*) | Map, Schematic and Trips tabs, the map drawn on the lanes the trips really drive |
| A new tour's buses | The fixed group name "Busses" | The map's own first depot group, or any AI group of its `ailists.cfg` |
| In the game | Skip the duty's next stop from the game menu | A running timetable window with served, skipped and missed stops marked and a Skip stop button |

### Setup: free disk space with NTFS compression *(Depot addition)*

* Setup's *Free up space* card can compress the content with NTFS (LZX, Windows' "compact") and undo it, for the
  Depot's content folder (ticked) and, apart, the OMSI 2 folder (not ticked). It estimates what each would free
  from a sample of files (marked as an estimate), works in the background with progress and Cancel, and leaves
  files the game writes, `.zip` archives and already-compressed media alone.
* It is offered only on a fixed NTFS disk (never in a cloud-sync folder, on a network or removable disk) and says
  why when it is not. On a hard disk it warns that loading can be much slower and asks to confirm; for the OMSI 2
  folder it recommends playing through OmsiLaunch with the Omsi Prefetch module. Undo checks the free space first.
* *Not yet measured:* the effect of compressed files on loading times on real hardware.

### Opening the original OMSI 2 from the launcher *(hidden, off by default, Windows only)*

* **What it is.** A Depot | OMSI 2 switch that starts the original game through the OmsiLaunch tool when it is
  needed, from the same Maps page, with the OMSI 2 settings the original has (frame-rate limit, object distance,
  reflections, collisions, ticket selling in its three positions, traffic counts, passenger density, volume and
  more), kept in a file of their own and shown with OMSI 2's defaults and limits; Controls is read-only there.
* **How it was built safely.** The OmsiLaunch install takes only the newest official release of its own
  repository, checks it against the published SHA-256 and its manifest, lists the files for the player to confirm
  and installs in a staging area; what Depot may overwrite or delete is decided by a record it keeps outside the
  OMSI 2 folder; the launch is built outside the OMSI 2 folder, planned first, and nothing starts unless it can
  run (each reason is shown as a sentence). It passed an independent security review. Optional modules (Omsi
  Prefetch, openHUD) install by the same rules. Texts exist in all 26 languages.
* **Status.** Hidden unless the maintainers' development switch is set; documented in `docs/ORIGINAL_ENGINE.md`.

### A new in-game interface

* **Esc menu and its pages** drawn by the Depot's own toolkit over the paused picture: Options, Controls, Vehicle
  options, World options, line and tour with the timetable, and the lists of drivers, fleet numbers, destinations,
  depot files and more, in the same look, with rounded rows, switches, keycaps and an accent pill.
* **Alt bar** (a tap of Alt; it never pauses the game): the original's fifteen buttons in their order, five by
  three, with tinted tiles and outline icons in the accent, a name above the tile under the mouse, a fold-away
  tab, and keyboard use (Alt+Down, arrows, Enter). Everything on it works from the bar.
* **Add vehicle** is laid out like the original's vehicle choice, with a live 3D preview of the vehicle in the
  light of the game's hour, liveries, depot files, fleet number and plate.
* **Loading screen** with the logo on a photo, a darkened edge so the text reads on bright screenshots, and a
  progress bar; **end-of-trip report** with the stops served, late arrivals and early departures (it pauses the
  game); a running timetable with served and skipped stops marked.
* **Accent colour** for the whole interface, shared by the launcher and every game window.

### Information bar (Shift+Y) *(Depot addition)*

* A dark plate in the interface's look with the time, the speed with the **street's speed limit as a road sign**,
  the line with its destination, the next stop with the delay, trip distance, odometer, outside and cabin
  temperature, passengers and the tank, plus a frame-rate part.
* **Layouts:** Full (default), Two-tier, Split and **Custom**, where each item is switched off one by one; a
  position (top or bottom) and the frame-rate chip are options in the launcher and in the game's World options.

### Navigator and city map

* **City map (Shift+M):** a compass (Map, North up or Heading up), a **terrain view with contour lines** and their
  heights (computed on a worker thread so the frame never waits), the street under the pointer and the speed limit
  of the street the player is on, and a **lines panel** with each line's departures by direction and day and the
  time at every stop for a chosen departure, from the map's own timetable. It reads map packages
  (`docs/MAP_PACKAGE.md`) and nothing is added to the map.
* **Minimap:** an optional **top view** (heading up), a mode that follows the walker or the free camera, and new
  **rounded vehicle pictograms** (a trolleybus is shown with its two poles).
* The lines panel is translated into every interface language, and the new look applies to both maps.

### Diagnostics: integrity checks and AI traffic *(Depot addition)*

* **Integrity check** of a map or a vehicle (also from `omsi-check --integrity`): it follows what the content
  names (tiles, objects, splines, models, meshes, textures, sounds, scripts, paths, passenger cabin) and lists
  errors, warnings and "not optimized" findings, split in tabs by kind with a dot for the worst one, with why each
  matters; it can be **compared against an untouched baseline folder** so that what an optimization broke stands
  out from what was already wrong. Results export to a text report with no personal paths; Cancel really stops it.
* **AI traffic tab:** shows a map's AI lists as the game takes them on a chosen day, with the Chrono scenarios in
  force, the chance of each vehicle, and a 3D preview. **Edit lists** changes car groups, weights, parking lists
  and bus fleets and which AI group a tour takes its buses from; nothing is written until *Save*, which keeps
  a copy of every changed file, and a backup can be put back safely.
* A *Detailed log* switch writes every file the game reads, every probe that found nothing and every archive it
  mounts, to find why content does not load.

| | Before 0.3.0 | Now |
| --- | --- | --- |
| Diagnostics page | An overview page | **Overview**, **Integrity** and **AI traffic** tabs |
| Checking content | `omsi-check` on the command line | The same integrity check from the launcher, by kind, with why each finding matters, and a baseline comparison |

### Controls: mapping, import and conflicts

* **Keyboard page.** Actions in groups on the left (All, Driving, Cab and lights, Doors and tickets, Cameras and
  views, Game and menus, VR, Advanced; plus *Conflicts*, *Changed by me* and *Not assigned*, each with its count),
  the group's actions in the middle with a key and a second key (click a key and press the new one; "changed" and
  "shared key" are marked; the search covers every action) and the chosen action on the right: its key, its key
  in your OMSI 2 controls file, OMSI 2's key as it comes, *Back to original* and *Clear*. Keys are named in the
  game's language.
* **Conflicts are highlighted but saveable.** A key used twice is flagged, never refused, and only when OMSI 2's
  own layout does not share it that way on purpose (it puts the manual and automatic gears on one key).
* **Import from OMSI 2.** *Preview changes / Reload from OMSI 2* lists what would change, what is already the same
  and what only the Depot has (kept as it is), makes a dated backup first and can read the game-controller
  assignments too, device by device by name. The original's files are only ever read.
* **Add a control** lists the actions the game knows that have no key yet, waits for the key and warns when
  another action has it; a bus's own trigger is added when its whole name is typed.
* **In the game** the Esc menu's Controls pages use the same look, with options that apply at once as switches.

| | Before 0.3.0 | Now |
| --- | --- | --- |
| The key list | Click a key, press the new one; clashes shown in red | The same, in groups with counts, a second key, search, a detail panel and *Back to original* |
| Clashes | Red | Flagged and never refused; keys the original shares on purpose are recognised |

### Content safety, translations and performance

* **Safer content loading.** A content file or map package can no longer point the game at another drive or
  computer (drives, streams, shares); a map package's icon path stays inside the package; the `zip` library
  was updated for fixes to unsafe paths and links.
* **26 languages.** Every interface text the code uses exists in all 26 languages, and a check keeps it so.
* **No more drop to a few frames a second while paused in traffic**, and a graphics device the driver gives up
  in the middle of a frame no longer closes the game.
* **Removed:** LAN play, the dedicated server, voice chat and plugins, switched off since the first Depot release.

## The launcher and the interface at a glance

Project Depot opens a light launcher of its own, in the Windows 11 look (optionally a sidebar or a top bar,
a plain dark or a scene background, rounded or square corners, and a choice of accent colours). The game
starts only when you drive, and the launcher steps aside to the tray while it runs.

| Page | What it is for |
| --- | --- |
| **Maps** | The start page: pick a map, a start point, a line and a tour, the date and the weather, and press **Start driving**. |
| **Vehicles** | The library grouped by manufacturer and model, with a rendered preview, paints and details. |
| **Timetable** | Planning a map's lines, tours and trips, with a map and a schematic view. |
| **Profile** | Your driver, the hours and level, and the record of past drives. |
| **Mods** | Installing `.zip` mods. |
| **Controls** | Keyboard and game-controller bindings, with conflicts highlighted. |
| **Diagnostics** | The OMSI 2 folder card, the **Integrity** check of maps and vehicles and the **AI traffic** tab. |
| **Settings** and **Setup** | Graphics (including the optional DirectX 9 renderer), simulation, traffic and people, on-screen items, interface; and where OMSI 2 is. |

In the game, the Esc menu, the Alt bar (a tap of Alt), the *Add vehicle* window, the end-of-trip report, the
information bar (Shift+Y), the navigator and the city map (Shift+M) share the same look.

<!-- screenshots to add here, from the 0.3.0 release (docs/images/): launcher-maps, launcher-vehicles,
     launcher-timetable, diagnostics-integrity, settings-graphics-dx9, ingame-esc-menu, ingame-alt-bar,
     ingame-info-bar, ingame-city-map -->

## Download

Project Depot is built and published for **Windows x64 only**. The current release is on the
[**0.3.0 release page**](https://github.com/OmsiModsBR/depot/releases/tag/v0.3.0):

| Platform | File |
| --- | --- |
| Windows x64 | `depot-0.3.0-windows-x64.zip` - run `depot.exe`; also includes `depot-launcher.exe` |

No ARM64, Linux, macOS or Android package is published, and none is promised: the supported hardware
and system are a deliberate product decision, not a consequence of what the code can compile for.

The program does not update itself: new versions are published on the
[Releases](https://github.com/OmsiModsBR/depot/releases) page, and you download them yourself
(in-app updating is disabled, see [docs/removed/UPDATER_DISABLED.md](docs/removed/UPDATER_DISABLED.md)).
Settings → General → About shows the version you run and links to that page.

## Installation

**You need an installed OMSI 2** (Steam or retail, any version) with its stock content -
the maps Grundorf and Berlin-Spandau and the stock buses (MAN SD200/SD202, NL). Project Depot
brings no game content of its own; it plays the original's maps, buses and mods.

1. **Download** the Windows x64 zip from
   [Releases](https://github.com/OmsiModsBR/depot/releases) and unpack it into a folder of its own that
   you can write to - your Documents, a games folder, or the OMSI 2 folder itself. Not `Program Files`:
   the game writes its settings and logs next to itself.
2. **Start `depot.exe`.** Windows SmartScreen may warn about an unknown app: *More info* → *Run anyway*.
3. **Point it at OMSI 2.** The launcher usually finds the installation by itself (Steam
   libraries, the usual folders). If not, open **Setup** and choose the OMSI 2 folder - the
   one with `Omsi.exe`, `maps` and `Vehicles` in it (the folder, or `Omsi.exe` itself) - and
   press **Save**. The Steam version is under
   `…\Steam\steamapps\common\OMSI 2`.
4. **Drive:** pick a map, a bus and a duty on the **Maps** page and press **Start driving**.

**Mods** are installed on the **Mods** page: only a `.zip` is installed (chosen there, or dropped on the
window), and it is used in place from `Archives`, never unpacked. The launcher never installs a loose folder; the
Mods page tells where to copy one by hand.

### When something goes wrong

* **"The original OMSI 2 was not found"** - choose the folder under Setup (step 3); the
  message says what the chosen folder lacks.
* **The game closes after a few seconds, or "the graphics device was lost"** - update the
  graphics driver (NVIDIA, AMD or Intel's own, not the one Windows installs). On Windows you
  can also switch to DirectX 12: Settings → Graphics → Graphics API (the launcher offers it after such a
  crash).
* **An older graphics card** (no Vulkan): Project Depot falls back to DirectX 12 and then OpenGL by
  itself; Settings → Graphics → Graphics API chooses one.
* **Stuck at a bridge or an invisible wall** on a mod map: Esc → Options → *Collisions with
  objects* switches collisions with the map's objects off (Settings has it too).
* **Keys do not do what you set:** Controls - the page shows which driving keys are in use;
  a key you change there takes effect at once.
* **Anything else:** when the game ends on an error, the launcher shows it with *Copy report*
  and *Report on GitHub*. The logs are in `~/.depot` (Windows: `C:\Users\<you>\.depot`),
  `game.log` for the last game.

## Goals

1. **1:1 behaviour.** Every content format of the original - maps, splines, scenery objects,
   vehicles, scripts, timetables, HOF files, fonts, weather, tickets, situations, plugins -
   loads and behaves exactly as in OMSI 2. Existing maps and mods work unchanged.
2. **No original code or assets.** Nothing from the original is copied; the formats are
   described in [docs/FORMATS.md](docs/FORMATS.md).
3. **A better engine.** 64-bit address space, streaming and texture loading on worker threads,
   no 2 GB limit, no single-thread stalls.

## Documentation

The pages live in [`docs/`](docs) and are also published as the website
(https://omsimodsbr.github.io/depot/, a temporary address):

| Document | What is in it |
| --- | --- |
| [User guide](docs/USER_GUIDE.md) | running, controls, launcher, settings, mods, debug switches |
| [Virtual reality](docs/VR.md) | OpenXR setup, VR settings and controls on Windows |
| [Modding](docs/MODDING.md) | limits lifted for modders: more interior lights, larger textures, additions OMSI 2 ignores |
| [PBR materials](docs/PBR.md) | normal, roughness, metalness and occlusion maps for mods |
| [Building](docs/BUILDING.md) | building from source on Windows |
| [Content formats](docs/FORMATS.md) | every OMSI 2 file format |
| [Architecture](docs/ARCHITECTURE.md) | crates, threading, renderer, roadmap |
| [Routes](docs/ROUTES.md) | how the original runs timetables, chrono, HOF, IBIS |
| [Versioning & releases](docs/VERSIONING.md) | the `MAJOR.MINOR.PATCH` scheme and how a release is made |
| [Release notes](docs/releases) | one page per release: [0.3.0](docs/releases/0.3.0.md), [0.2.52](docs/releases/0.2.52.md), [0.2.17](docs/releases/0.2.17.md) |
| [Changelog](CHANGELOG.md) | what changed in Project Depot ([upstream history](CHANGELOG_UPSTREAM.md), frozen) |
| [Where the code comes from](UPSTREAM.md) | the openOMSI baseline, what was taken in, and under which licence |

## Building from source

```bat
git clone https://github.com/OmsiModsBR/depot.git && cd depot
scripts\build-windows.cmd     :: Windows x64 → dist\windows\depot.exe
```

Needs the latest [Rust stable](https://rustup.rs) with the MSVC toolchain and the Visual Studio Build
Tools; details in [docs/BUILDING.md](docs/BUILDING.md). Other systems are not supported.

## Repository layout

```
depot/
├── VERSION            MAJOR.MINOR of the next release (edited by hand)
├── crates/            the engine, one crate per subsystem of the original
│   ├── omsi-app/        the game binary `depot` (window, launcher, HUD)
│   ├── omsi-launcher-core/  launcher data side + `depot-launcher` terminal tool
│   ├── omsi-launcher-ui/  the launcher's window and pages
│   ├── omsi-integrity/  integrity checks of maps, vehicles and AI lists
│   ├── omsi-html/       the HTML engine behind in-vehicle screens
│   ├── omsi-devtools-proto/  messages for developer tools (not yet used by the game)
│   ├── omsi-cfg/        text files, code pages, virtual file system, content roots
│   ├── omsi-script/     the OMSI script language (compiler + VM)
│   ├── omsi-o3d/ omsi-model/ omsi-texture/ omsi-geometry/   meshes, models, textures, splines
│   ├── omsi-map/ omsi-scenery/ omsi-timetable/ omsi-vehicle/ omsi-content/   content formats
│   ├── omsi-sim/        vehicles, AI traffic, people, physics
│   ├── omsi-render/ omsi-render-d3d9/   the wgpu renderer; the DirectX 9 (D3D9Ex) renderer
│   ├── omsi-audio/ omsi-ui/   sound, UI toolkit
├── tools/             developer tools: omsi-check (format coverage)
├── scripts/           the Windows build script, version.sh, packaging files
├── assets/            fonts, Material icons, app icons (assets/icons/app), Project Depot logos (assets/logos)
├── docs/              documentation (also published as the website)
├── site/              the GitHub Pages website
└── docs/upstream-automation/ archived upstream CI recipes (inactive)
```

## Contributing

Issues and pull requests are welcome - see [CONTRIBUTING.md](CONTRIBUTING.md).
Branches are named `feat/`, `fix/` or `docs/` by what they contain - see
[CONTRIBUTING.md](CONTRIBUTING.md#branches).

## License

Project Depot is released under the [MIT License](LICENSE). OMSI and OMSI 2 are trademarks of their
respective owners. Project Depot is an independent project and is not affiliated with them.
