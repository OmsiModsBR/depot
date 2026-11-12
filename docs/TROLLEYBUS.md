# Trolleybuses

Project Depot runs trolleybuses on its own: poles, overhead wire, switches, crossings, wire
sounds and sparks. No plugin is needed. That matters because the 64-bit engine cannot load the
32-bit trolleybus plugins of OMSI 2.

Depot does not bring the trolleybus content, though. It reads the content of the existing
plugin-based convention, from the player's OMSI 2 folder: the same parameter file, vehicle
registry, wire splines, switch objects and recordings the plugin used. None of these files ship
with Depot, because nothing from an OMSI 2 installation goes into the project. Installing them is
up to the player, as for any map or bus.

The setting **Simulation > Trolleybus** (on by default) turns all of it off.

## What goes where

| File or folder | Where Depot looks for it | What it holds |
|---|---|---|
| `TrolleyWirePlugin.cfg` | first Depot's content folder `Plugins\`, then the OMSI 2 folder `Plugins\` | where the wire hangs, how the poles move, when they leave the wire, sounds and sparks |
| `TrolleyWireVehicles.ini` | the same | which buses are trolleybuses, and where their poles stand |
| `Sounds\AckerTroll\` | the OMSI 2 folder (and Depot's content folder) | the recordings of switches, crossings and sparks |
| wire splines and switch objects | the map, as usual | the overhead wire itself |

Both `Plugins\` files are needed. If either one is missing, there are no trolleybuses in that
session, and the log says so once.

Depot's content folder comes first, so you can tune a copy of the two files there without
touching your OMSI 2 folder. The plugin DLLs (`TrolleyWirePlugin.dll` and its companions) are not
read. You can leave them, since OMSI 2 still uses them, but Depot does not need them.

## Making a bus a trolleybus

A bus is a trolleybus when `TrolleyWireVehicles.ini` has an enabled `[vehicle:<name>]` section
whose `bus_file` is that bus's `.bus` file. The name after `vehicle:` is free: the log uses it.

Coordinates use the bus's own frame, in metres:
- `x` is to the right, so the left side is negative;
- `forward` is along the bus;
- `height` is up.

An example (the numbers are only an illustration; measure your own bus):

```ini
[vehicle:my_trolleybus]
enabled=1
bus_file=Vehicles\My_Trolleybus\My_Trolleybus.bus

; pole pivots on the roof
pole_base_left_x=-0.30
pole_base_right_x=0.30
pole_base_forward=-1.80
pole_base_height=3.40

; shoe at rest on the hook, poles down
contact_left_x=-0.30
contact_right_x=0.30
contact_forward=-7.60
contact_height=3.45
```

### Keys that must be there

- `bus_file`
- `pole_base_left_x`, `pole_base_right_x`, `pole_base_forward`, `pole_base_height`
- `contact_left_x`, `contact_right_x`, `contact_forward`, `contact_height`

### Keys that may be left out

| Key | Use |
|---|---|
| `enabled` | `0` skips the entry |
| `pole_carrier_bus_file` | for an articulated bus: the `.bus` of the section that carries the poles |
| `shoe_contact_height_offset` | raises (`+`) or lowers (`-`) the drawn shoe on the wire, in metres; without it, the value from `[global]` of the parameter file, or 0 |
| `manual_capture_distance` | how far a raised pole looks for the wire; without it, the value from `[global]` |
| `yaw_factor_left`, `yaw_factor_right`, `pitch_factor`, `shoe_factor_left`, `shoe_factor_right` | scale the angles written to the model's script variables (1 when left out) |
| `contact_reach_release_error_m`, `contact_reach_release_time` | how far, and for how long, the wire may be out of reach before the pole leaves it; without them, the values from `[physics]` |

A section that lacks a key it must have, or has a value that is not a number, is skipped. The
log names the key, and the bus runs as an ordinary bus.

The bus's model draws the poles, through the usual `shtanga_*` script variables of the
convention. Depot writes these variables, so the bus's own scripts and model work unchanged.

## The parameter file

`TrolleyWirePlugin.cfg` is read as the plugin wrote it.

Keys Depot reads but does not use:
- `[global] active_map`, `debug_level`, `tile_lock_radius`: Depot knows which map is loaded and
  where the bus is.
- the `*_chance` keys of `[sound]`: every switch and crossing passed is heard.
- `spark_sample_gain`: the spark recording is brought up to full scale anyway.

The `[sound]` and `[sparks]` sections may be left out. The poles still work without them, but
the wire makes no sound and does not spark.

## The map

The map needs nothing new:
- **Wire.** A spline is wire when its `.sli` path contains one of the `spline_keywords` of the
  parameter file.
- **Switches and crossings** are recognised by their object's path:

| Object | Path contains |
|---|---|
| Y switch | `Troll_strelka_out_02` or `Troll_strelka_out_03` |
| left or right branch switch | `chave_segmento_esquerda_depot` or `chave_segmento_direita_depot` |
| crossing | `OnKrest` or `TwoKrest` |
| wire sound marker | `Troll_podves_02` |

## Driving

- **Raise and lower.** Click a pole to raise it onto the wire on its side. Click it again to
  lower it. The vehicle's retract control takes both poles down.
- **Switches.** The throttle chooses the way. Pressed through the switch, the shoes take its left
  way; coasting, they take the right way. A crossing is always passed straight on.
- **Leaving the wire.** Too fast through a switch or crossing, too far from the wire's line, or
  too hard a pull sideways, and a pole leaves the wire. The retriever then pulls it down to its
  hook.

## When something does not work

The log is `%USERPROFILE%\.depot\game.log`. Look for lines that start with `trolleybus:`.

| What you see | What to check |
|---|---|
| The log does not mention trolleybuses at all | Both `Plugins\` files are in place, and **Simulation > Trolleybus** is on. |
| The bus has no poles to click | The entry's `bus_file` is the bus you are driving. The log names any key the entry lacks. |
| A pole finds no wire | The log says where the nearest wire is. If it says there is none, the map's wire splines do not match `spline_keywords`. |
| The shoe floats above the wire or sinks into it | Adjust `shoe_contact_height_offset` in the bus's entry. |
| Switches are silent | `Sounds\AckerTroll\` is in place, and the parameter file has a `[sound]` section. |
