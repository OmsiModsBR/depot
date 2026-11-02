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
  *Detailed log* is, but the game never reads it. `omsi_launcher_lib::developer_mode_on()` is
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

The game starts a connection only when the launcher gives it the per-run socket name, which the
launcher does only with this switch on.
