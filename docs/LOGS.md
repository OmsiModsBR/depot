# Logs

Where the logs are, what each one contains, how to get more detail and what to attach to a problem report. This page describes what the program does today; it is updated in the same change as each logging change.

## Where they are

All logs are in the `.depot` folder in your user folder: `~/.depot` on Linux and macOS, `C:\Users\<you>\.depot` on Windows. (The launcher's Setup page shows the folder and opens it.) Android has no launcher; its game log is written by the game itself and is not covered here.

| File | Written by | Contains |
|---|---|---|
| `game.log` | the game, started from the launcher | the machine and the settings at the start, what the game loads and what fails, on-screen messages, a status line every minute, warnings and errors, and a panic with a backtrace if the game stops on an error |
| `game-2.log`, `game-3.log` … | a second, third … game running at the same time | the same, one file per running game |
| `launcher.log` | the launcher | the launcher's own lines, such as starting a game, installs and clean-up |
| `preview-helper.log` | the helper process that draws the bus pictures in the launcher | its lines, rewritten for each helper |

Each run replaces the previous `game.log`, so only the current session is kept. The launcher log may grow during a run. The first line of both says the build they were made from.

## Script errors

There is no separate script log yet. Script read and compile problems appear in `game.log` as warnings in the form `file:line: message`, mixed with the other lines. If a script block panics, `game.log` lists the block and every macro it was called from.

## How to get more detail

- **Detailed log** (launcher, Diagnostics page): off by default. When on, a game started from the launcher also writes debug lines from the game's own crates: every file it loads (`loaded <path> (<n> bytes)`), files it looked for and did not find, and a warning with path and reason for a file that exists but cannot be read. It does not turn on the graphics libraries' debug output.
- **`RUST_LOG`**: for a game started by hand, or to override the setting for one launch (the environment variable wins over the Detailed log setting, and the log says so). The default is `info`.
- **`OMSI_PROFILE=1`**: with it, `game.log` gets one `profile window:` line every 30 seconds with the frame time and the cost of each stage. See [performance measurement](performance-measurement.md).
- Other `OMSI_DEBUG_*` switches for single subjects are listed in [ENVIRONMENT](ENVIRONMENT.md).

## What to attach to a report

Attach `game.log` (the part around the problem is enough). For a launcher problem, attach `launcher.log`. If the problem is in a bus preview, attach `preview-helper.log` too.

## Privacy

Logs are written on your computer and nothing sends them. They can contain paths on your computer (including your user name), the graphics card and the maps and vehicles you loaded: look through them before posting. See [PRIVACY](PRIVACY.md).
