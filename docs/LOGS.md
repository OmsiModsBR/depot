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

Each log file holds 30 MB at most. A file that reaches the limit stops growing, and its last line says so:

```
log truncated at 30 MB: the lines after this one were not written
```

The line that did not fit and everything after it are not written; the room for that last line is kept free, so it is always there when a log was cut. The game goes on as before. `launcher.log` is written by two programs, so it keeps a small slack free as well (about half a megabyte) and each of the two programs ends its own lines with that line. A game started by hand in a terminal writes to the terminal and has no limit. What a graphics driver or the system writes to the game's error output on its own is not counted.

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

### Channels

Parts of the program can be given a level of their own, over the level above, with `--log-channels` on the game's command line: `channel=level` entries separated by commas.

```
depot --log-channels physics=debug,render=warn ...
```

| Channel | Lines of |
|---|---|
| `script` | the script compiler and the script machine |
| `content` | reading content: configuration files, maps, models, textures, timetables, vehicles, scenery, and the loading of the tiles |
| `audio` | sound and the bus radio |
| `physics` | vehicle physics, collisions, the trolleybus wires |
| `ai` | AI vehicles, traffic and people |
| `network` | the Steam status and the developer-mode link |
| `render` | the renderers and the cameras |
| `input` | keyboard, mouse, wheels and gamepads |
| `app` | everything else of the game and the launcher |

- A channel that is not listed follows the level (`--log-level`, or the launcher's setting).
- A channel can be louder than the level (`physics=debug` at `info`) or quieter (`render=warn` at `debug`).
- An entry whose channel or level is not known is left out, and the log starts with a warning that names it; the other entries still count.
- `RUST_LOG`, when set, wins over the channels as well.
- There is no setting for channels in the launcher: they are for a game started by hand. The graphics libraries are no channel and never write more than `info`.

## What to attach to a report

Attach `game.log` (the part around the problem is enough). For a problem in the launcher or in a bus preview, attach `launcher.log`.

## Privacy

Logs are written on your computer and nothing sends them.

Your user folder is not written to them. Wherever a line would hold it (`C:\Users\<you>` on Windows, `/home/<you>` on Linux, `/Users/<you>` on macOS), the log holds the word `USER_FOLDER`, and the rest of the path stays, so it still says which file is meant:

```
loaded USER_FOLDER\.depot\settings.cfg (912 bytes)
loaded D:\OMSI 2\Vehicles\MAN_SD200\MAN_SD80.bus (5120 bytes)
```

- Both slash styles and any letter case of the folder are recognised (`c:/users/<you>/...` too), and so are paths written with doubled backslashes.
- A path that is not under your user folder is left as it is, the OMSI 2 folder and your content folder included.
- Where your account name is a piece of another path (`/run/media/<you>/...`, `D:\Profiles\<you>\...`), it is written as `USER_NAME`; the computer's name in a network path (`\\<computer>\share`) is written as `COMPUTER_NAME`. Names of fewer than three characters are only replaced as part of the user folder. The program never writes your account name or your computer's name on its own.
- The same replacement is made in the text the program hands out for a report: the crash dialog's "Copy report" and "Report on GitHub", and the integrity report (copied or saved).
- It applies to `game.log` and `launcher.log`. A game started by hand in a terminal prints to the terminal as it is. What the system or a graphics driver writes to the game's error output on its own does not pass through it.

Logs still contain the graphics card, the maps and vehicles you loaded, and paths outside your user folder: look through them before posting. See [PRIVACY](PRIVACY.md).
