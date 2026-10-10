# Project Depot

> Experimental branch `feat/outside-fov-ultrawide`: contains the selective
> [outside-FOV integration](docs/integrations/OUTSIDE-FOV-ULTRAWIDE.md),
> not included in main or the published Depot 0.2.17 packages.

Independent repository: https://github.com/OmsiModsBR/depot

Current Depot release: **[0.2.17](https://github.com/OmsiModsBR/depot/releases/tag/v0.2.17)** — rebranding only, based on openOMSI 0.2.16.

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
with a modern renderer (Metal / Vulkan / DirectX 12 through wgpu).

The project's explicit goal is to **achieve 1:1 compatibility with OMSI 2**,
including its existing maps, buses, scenery, scripts and mods. This is a development
target, not a claim that complete compatibility has already been achieved.
Alongside that goal, **evolutions to the simulator's physics and simulation are
a development priority**, while preserving compatibility and a stable base through
selective upstream integration.

**Depot 0.2.17 is simply the project's rebranding release.** It changes branding,
distribution names and Depot-specific repository/update configuration; it adds
no gameplay, renderer, physics, simulation, scripting or multiplayer features.
Physics/simulation evolutions and 1:1 compatibility remain development goals,
not improvements delivered by this release. See the [release notes](docs/releases/0.2.17.md).

> [!IMPORTANT]
> **Project Depot needs an original copy of OMSI 2.** It contains no game content of its own: it
> plays on the maps, vehicles and other files of an installed OMSI 2 and **will not start without one**.


## Download

The first rebranding release is available on the
[**0.2.17 release page**](https://github.com/OmsiModsBR/depot/releases/tag/v0.2.17):

| Platform | File |
| --- | --- |
| Windows x64 | `depot-0.2.17-windows-x64.zip` - run `depot.exe`; also includes `depot-launcher.exe` |
| Dedicated server, Windows x64 | `depot-0.2.17-server-windows-x64.zip` - see [docs/SERVER.md](docs/SERVER.md) |

ARM64, Linux, macOS and Android packages are not included in Depot 0.2.17.
Inherited platform support and build instructions do not imply published Depot packages.

Start the game, point the launcher to your OMSI 2 folder once, pick a map, a bus and a duty,
and drive. Mods go into the folder next to the game (or through the launcher's **Mods**
page); the original installation is never written to.

From 0.1.7 on the launcher updates itself: when a newer release is out it asks at the start
and, with your yes, downloads it, replaces the program and starts again (on Android through
the system's installer). Settings → General → Updates switches the check off or installs without
asking.

## Installation

**You need an installed OMSI 2** (Steam or retail, any version) with its stock content -
the maps Grundorf and Berlin-Spandau and the stock buses (MAN SD200/SD202, NL). Project Depot
brings no game content of its own; it plays the original's maps, buses and mods.

1. **Download** the file for your system from
   [Releases](https://github.com/OmsiModsBR/depot/releases) (table above) and unpack it
   into a folder of its own that you can write to - your Documents, a games folder, or the
   OMSI 2 folder itself. Not `Program Files`: the launcher could not update itself there.
2. **Start it.**
   * **Windows:** `depot.exe`. Windows SmartScreen may warn about an unknown app: *More
     info* → *Run anyway*.
   * **macOS:** open `depot.app`. The first time, macOS may refuse an app from the
     internet: right-click → *Open* → *Open*, or run
     `xattr -dr com.apple.quarantine /path/to/depot.app` once.
   * **Linux:** `./depot` (run `chmod +x depot` if it does not start). It needs a
     Vulkan or OpenGL driver (Mesa: `mesa-vulkan-drivers`, or your GPU vendor's driver).
   * **Android:** see [docs/ANDROID.md](docs/ANDROID.md) - the OMSI 2 folder is copied onto the
     phone first.
3. **Point it at OMSI 2.** The launcher usually finds the installation by itself (Steam
   libraries, the usual folders). If not, open **Setup** and choose the OMSI 2 folder - the
   one with `Omsi.exe`, `maps` and `Vehicles` in it (the folder, or `Omsi.exe` itself) - and
   press **Save**. The Steam version is under
   `…\Steam\steamapps\common\OMSI 2`.
4. **Drive:** pick a bus, a map and a duty on the **Drive** page and press **Start the duty**.

**Mods** are installed on the **Mods** page (a folder or a `.zip`, or dropped on the window) or
by putting them into the `Mods` folder next to the game; the OMSI 2 folder is never written
to.

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
* **Multiplayer: you do not meet the others** - both players need the host's map (a map in
  the OMSI 2 folder is not passed on; one from the Mods page is). The joining game switches
  to the host's map by itself and says in the HUD when it is not installed.
* **Keys do not do what you set:** Controls - the page shows which driving keys are in use;
  a key you change there takes effect at once.
* **Anything else:** when the game ends on an error, the launcher shows it with *Copy report*
  and *Report on GitHub*. The logs are in `~/.depot` (Windows: `C:\Users\<you>\.depot`),
  `game.log` for the last game.

## Goals

1. **1:1 behaviour.** Every content format of the original - maps, splines, scenery objects,
   vehicles, scripts, timetables, HOF files, fonts, weather, tickets, situations, plugins -
   loads and behaves exactly as in OMSI 2.2.032. Existing maps and mods work unchanged.
2. **No original code or assets.** Nothing from the original is copied; the formats are
   described in [docs/FORMATS.md](docs/FORMATS.md).
3. **A better engine.** 64-bit address space, streaming and texture loading on worker threads,
   no 2 GB limit, no single-thread stalls, LAN multiplayer and a dedicated server.

## Documentation

Documentation is available locally. The pages live in [`docs/`](docs):

| Document | What is in it |
| --- | --- |
| [User guide](docs/USER_GUIDE.md) | running, controls, launcher, settings, mods, LAN play, debug switches |
| [Virtual reality](docs/VR.md) | OpenXR setup, VR settings and controls on Windows |
| [Android](docs/ANDROID.md) | the mobile version: install, touch controls, building the APK |
| [Modding](docs/MODDING.md) | limits lifted for modders: more interior lights, larger textures, additions OMSI 2 ignores |
| [PBR materials](docs/PBR.md) | normal, roughness, metalness and occlusion maps for mods |
| [Building](docs/BUILDING.md) | building from source on macOS, Windows, Linux and Android |
| [Content formats](docs/FORMATS.md) | every OMSI 2 file format |
| [Architecture](docs/ARCHITECTURE.md) | crates, threading, renderer, roadmap |
| [Routes](docs/ROUTES.md) | how the original runs timetables, chrono, HOF, IBIS |
| [Plugins](docs/PLUGINS.md) | Lua plugins (API and examples), OMSI plugin DLLs and the 32-bit plugin host |
| [Dedicated server](docs/SERVER.md) | hosting a session without a window |
| [Versioning & releases](docs/VERSIONING.md) | the `MAJOR.MINOR.COMMIT` scheme and the CI |
| [Changelog](CHANGELOG.md) | what changed in each version |

## Building from source

```sh
git clone https://github.com/OmsiModsBR/depot.git && cd depot
scripts/build-macos.sh        # macOS   → dist/macos/depot.app
scripts\build-windows.cmd     # Windows → dist\windows\depot.exe
scripts/build-linux.sh        # Linux   → dist/linux/depot
scripts/build-android.sh      # Android → dist/android/depot-<version>.apk
scripts/build-server.sh       # server  → dist/server
```

Needs the latest [Rust stable](https://rustup.rs) and the platform's C toolchain; details in
[docs/BUILDING.md](docs/BUILDING.md).

## Repository layout

```
depot/
├── VERSION            MAJOR.MINOR of the next release (edited by hand)
├── crates/            the engine, one crate per subsystem of the original
│   ├── omsi-app/        the game binary `depot` (window, launcher, HUD, server mode)
│   ├── omsi-launcher-core/  launcher data side + `depot-launcher` terminal tool
│   ├── omsi-cfg/        text files, code pages, virtual file system, content roots
│   ├── omsi-script/     the OMSI script language (compiler + VM)
│   ├── omsi-o3d/ omsi-model/ omsi-texture/ omsi-geometry/   meshes, models, textures, splines
│   ├── omsi-map/ omsi-scenery/ omsi-timetable/ omsi-vehicle/ omsi-content/   content formats
│   ├── omsi-sim/        vehicles, AI traffic, people, physics
│   ├── omsi-render/     the wgpu renderer
│   ├── omsi-audio/ omsi-net/ omsi-plugin/ omsi-ui/   sound, multiplayer, plugins, UI toolkit
├── tools/             developer tools: omsi-check (format coverage)
├── scripts/           build scripts for every platform, version.sh, packaging files
├── assets/            fonts, Material icons, app icons (assets/icons/app), logos (assets/logos)
├── docs/              documentation (also published as the website)
├── site/              the GitHub Pages website
└── docs/upstream-automation/ archived upstream CI recipes (inactive)
```

## Contributing

Issues and pull requests are welcome - see [CONTRIBUTING.md](CONTRIBUTING.md).

## License

Project Depot is released under the [MIT License](LICENSE). OMSI and OMSI 2 are trademarks of their
respective owners. Project Depot is an independent project and is not affiliated with them.
