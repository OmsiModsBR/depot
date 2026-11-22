# Logs

Where the logs are, what each one contains, how to get more detail and what to attach to a problem report. This page describes what the program does today; it is updated in the same change as each logging change.

## Where they are

All logs are in the `.depot` folder in your user folder: `~/.depot` on Linux and macOS, `C:\Users\<you>\.depot` on Windows. (The launcher's Setup page shows the folder and opens it.) Android has no launcher; its game log is written by the game itself and is not covered here.

| File | Written by | Contains |
|---|---|---|
| `game.log` | the game, started from the launcher | the machine and the settings at the start, what the game loads and what fails, on-screen messages, a status line every minute, warnings and errors, and a panic with a backtrace if the game stops on an error |
| `game-2.log`, `game-3.log` … | a second, third … game running at the same time | the same, one file per running game |
| `launcher.log` | the launcher and the helper process that draws the bus pictures in the launcher | the launcher's own lines, such as starting a game, installs and clean-up, and the preview helper's lines. Every line says which of the two wrote it: `[launcher]` or `[preview-helper]` |

Each run replaces the previous `game.log`, so only the current session is kept. `launcher.log` is replaced when the launcher starts and grows while it is open; a preview helper that is started again adds to it. The first line of each program says the build it was made from. There is no `preview-helper.log` any more; one left by an older version can be deleted.

## Size

Each log file holds 30 MB at most. A file that reaches the limit stops taking ordinary lines, and a line says so:

```
log truncated at 30 MB: of the lines after this one, only errors and the end of the game were written
```

- The last part of the 30 MB (1/64 of it, about half a megabyte) is kept for what must not be lost: errors, a panic with its backtrace, a lost graphics device and the line that says the game ended. Ordinary lines stop before it; those lines are still written after the truncation line, until the 30 MB are full. So a game that stops on an error after its log was cut still shows the crash dialog and has the error in its report.
- The room for the truncation line is kept free too, so it is always there when a log was cut. The game goes on as before.
- `launcher.log` is written by more than one program (the launcher and each preview helper), so it keeps a further slack of the same size free, and each program writes that line when it stops. With more than two programs writing at the same instant the file can pass the limit by a few lines.
- A game started by hand in a terminal writes to the terminal and has no limit; a game whose error output is a pipe or a file is limited.
- Not counted: what a graphics driver or the system writes to the game's error output on its own.

## Script errors

There is no separate script log yet. Script read and compile problems appear in `game.log` as warnings in the form `file:line: message`, mixed with the other lines. If a script block panics, `game.log` lists the block and every macro it was called from.

## How to get more detail

The log has four levels. Each level writes its own lines and those of the levels before it:

| Level | What is written |
|---|---|
| `error` | errors only |
| `warn` | errors and warnings |
| `info` | what the program does, its warnings and errors. This is the default |
| `debug` | also the debug lines of the game's own crates: every file it loads (`loaded <path> (<n> bytes)`), files it looked for and did not find, and a warning with path and reason for a file that exists but cannot be read. The graphics libraries stay at `info` |

Every line says its level in its header: `[<time> WARN  <where>] <text>`.

- **Log level** (launcher, Diagnostics page): Error, Warning, Info (the default) or Debug. It applies to the next games started from the launcher. A settings file from before the four levels, with the *Detailed log* switch on, is read as Debug.
- **`--log-level <level>`**: the same choice on the game's command line (`depot --log-level debug ...`); the launcher passes its setting this way. A value that is none of the four is `info`, and the log starts with a warning that says so.
- **`RUST_LOG`**: the specialist's override. When it is set, it wins over the level, and the log starts with a warning that says so. It takes any filter the logger reads, for instance `warn,omsi_script=debug`.
- **`OMSI_PROFILE=1`**: with it, `game.log` gets one `profile window:` line every 30 seconds with the frame time and the cost of each stage, and the stage table when the game ends. Every profiling line carries the target `profile` in its header (`[<time> INFO  profile] profile window: ...`), so the profiling lines can be picked out of the file by that word. See [performance measurement](performance-measurement.md).
- Other `OMSI_DEBUG_*` switches for single subjects are listed in [ENVIRONMENT](ENVIRONMENT.md).

## What to attach to a report

Attach `game.log` (the part around the problem is enough). For a problem in the launcher or in a bus preview, attach `launcher.log`.

## Privacy

Logs are written on your computer and nothing sends them. They can contain paths on your computer (including your user name), the graphics card and the maps and vehicles you loaded: look through them before posting. See [PRIVACY](PRIVACY.md).
