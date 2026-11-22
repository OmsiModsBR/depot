# Developer mode

Status: **first step only**. The switch exists; the tools do not yet. The design is in
[#500](https://github.com/OmsiModsBR/depot/issues/500).

Developer mode is for people who make or test vehicles and maps: a live view of a running
vehicle (script variables, physics, world interactions) in a window of the launcher, with
commands from that window applied by the game. These tools are the Depot's own. They carry no
claim of matching OMSI 2's internals, and a run in which a command was applied is not a 1:1
comparison run.

## The switch

Launcher → Settings → Developer → *Enable developer mode*. It is **off by default**.

- It is the launcher's own setting. It is stored as `developer_mode` in `settings.cfg`, as
  the *Log level* is, but the game never reads it. `omsi_launcher_lib::developer_mode_on()` is
  what the launcher asks.
- Off means off: no window, no local connection, no extra work in the game. A game started
  with the switch off is the game started before this setting existed.
- The section is listed with the Depot engine only; in OMSI 2 mode it is not shown.
- Nothing is sent anywhere. There is no telemetry and no network listener.

## What is planned (not in this version)

In order, one change each (see #500):

1. The launcher-owned local socket and the game's side of it, behind a trait. The message types
   and framing already exist in the `omsi-devtools-proto` crate.
2. Variable viewer with editing, macro/toggle status bar.
3. Physics view and editor, save to `.bus` with a backup of the original.
4. Mirror editor, 5. camera editor, 6. hot reload of scripts and sounds.
7. In-game overlays (bounding box, axles, centre of gravity, pivots), drawn only when the tool
   asks for them.
8. The run record written by the launcher and the recorded-session viewer. The format and the
   writer exist already (below); nothing writes a record until the socket of step 1 exists.

The game starts a connection only when the launcher gives it the per-run socket name, which the
launcher does only with this switch on.

## The run record (format)

A run record is what the launcher receives from the game during one session, kept as a file to
open later as graphs. The `omsi-devtools-record` crate writes it; **nothing calls it yet**, so
no record is written by this version.

- One file per session, next to `game.log`: `run-<date>-<time>.jsonl` (UTC), for instance
  `run-20261122-101500.jsonl`.
- **JSON Lines**: one JSON object a line, its kind in `t`, which comes first.
- **The last 5 sessions are kept.** The oldest record is removed when a new session starts,
  never while one is written. Other files of the folder are not touched.
- **50 MB at most per file.** A record that reaches the limit ends with the line
  `{"t":"truncated"}`, whose room is always kept free, and takes no more lines. (The text logs
  have their own limit of 30 MB, see [LOGS](LOGS.md).)
- Only the launcher writes it, and only with developer mode on. Nothing is sent anywhere.

| Line | Fields | Meaning |
|---|---|---|
| `{"t":"run",...}` | `protocol`, `build`, `started` | the first line: the protocol version and the build the game named in its hello, and when the record started (RFC 3339, UTC) |
| `{"t":"snap",...}` | `frame`, `sim_s`, `vars`, `phys` | a snapshot: the variables that changed since the previous one, by name (a number or a text; a number that is not finite is `null`), and the physics after the last substep of the frame (`position`, `velocity`, `wheels` with `load_n`, `slip`, `suspension_travel_m`). `vars` and `phys` are left out when the snapshot has none; a snapshot with neither writes no line |
| `{"t":"ev",...}` | `frame`, `kind`, `name` | a `macro`, `trigger` or `toggle` that fired, with the frame it fired in |
| `{"t":"cmd",...}` | `frame`, `cmd` | a command the game applied at that frame: `{"set":<name>,"value":<value>}`, `{"reload":<file index>}` or `{"subscribe":[<names>]}` |
| `{"t":"truncated"}` | | the last line of a record that reached the limit |

A record with a `cmd` line is of a run the tool changed. Such a run is not a 1:1 comparison
run. CSV for graphs is an export made from a record later; the record is the only source.
