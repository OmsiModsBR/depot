# Building from source

Project Depot is built and published for **Windows x64 only**. Releases are made by hand after
review and are on the [Releases](https://github.com/OmsiModsBR/depot/releases) page; build from
source only to work on Project Depot itself. Other systems and processors are not supported and
no package for them is published (see [versioning](VERSIONING.md)).

`scripts\build-windows.cmd` runs from any folder (paths with spaces are fine) and puts the
result into `dist\windows\`. That folder is also the game's **content folder** (mods go
beside the binary), so the script replaces only the binaries and never deletes anything else
there.

## Requirements

* The latest [Rust stable](https://rustup.rs) (`rustup update stable`) with the
  *x86_64 MSVC* toolchain: the builds use it, and the dependencies raise the oldest Rust they
  accept as they are updated.
* Visual Studio Build Tools with *Desktop development with C++* and the Windows SDK.
* CMake, to build the OpenXR dependency; it must be on `PATH`.
* A graphics card with Vulkan or DirectX 12 to play; the optional DirectX 9 renderer
  (Settings → Graphics API) needs Windows.

## Build

```bat
scripts\build-windows.cmd
```

The result is `dist\windows\depot.exe` (the game; started with no arguments it opens the
launcher) and `dist\windows\depot-launcher.exe` (the launcher's terminal tool).

Plain cargo works too: `cargo build --release -p omsi-app` builds `target\release\depot.exe`.

A release package is built with the version passed explicitly, and `depot --version` is checked
before it is packed: see [Versioning and releases](VERSIONING.md).

## The programs

* `depot` (`crates/omsi-app`) - the game. Started with no arguments it opens the launcher
  window; with arguments it starts a session directly (see [USER_GUIDE.md](USER_GUIDE.md)).
* `depot-launcher` (`crates/omsi-launcher-core`) - the launcher's commands for a terminal
  (`depot-launcher --cli maps`, `--cli install '{"path":"mod.zip"}'` …).
* `omsi-check` (`tools/omsi-check`) - loads every content file of an installation and reports
  what failed: `cargo run --release -p omsi-check -- "C:\path\to\OMSI 2"`.

## Icons

The application icon is made from the Project Depot logos in `assets/logos` (`pd-short-black.png` is
the source of the icon): `assets/icons/app/depot.ico` is embedded into the Windows executables at
build time (`build.rs`, `winresource`) and `depot-256.png` is the window icon. To regenerate them
after changing the source PNG (needs Python 3 with Pillow):

```sh
scripts/make-icons.sh
```

## Tests

```sh
cargo test --workspace
```

Some tests read an OMSI 2 installation (`OMSI_ROOT=C:\path\to\OMSI 2`) and skip themselves
when there is none.
