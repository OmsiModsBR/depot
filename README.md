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
