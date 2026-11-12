# Data model: Native trolleybus support

All positions are in Depot's global frame (metres, `DVec3`) unless marked "body frame" (the carrying
section's model frame: x right, y forward, z up).

## Wire network

**WireSpan** — one wire spline.
- `id` (tile, spline index), `wires: [Polyline; 2]` (left, right; points with height), `length`
- `ends: [SpanEnd; 2]` (position and direction at start and end)
- Rule: built only from splines whose `.sli` matches a `spline_keywords` entry.

**WireJoint** — two span ends that meet.
- `a: (span, end)`, `b: (span, end)`, `reversed: bool` (the spans run in opposite senses)
- Rule: ends closer than the join tolerance and with directions parallel or anti-parallel;
  rebuilt for the affected ends when a tile loads or unloads.

**WireSwitch** — a switch object.
- `kind: Y | BranchLeft | BranchRight`, `pose` (position, heading), `trigger` point
- `entry`, `branches: [span; 2]` resolved from the joints around it (phase 4).

**Crossing**, **Marker** — pose and kind; the marker's sound is not played in this feature.

**TileWires** — everything above for one tile, plus its grid cells; dropped with the tile.

**NetworkView** — the read-only view of the loaded tiles' wires that the pole step queries
(`segments_near(point, radius)`, `next_span(span, end)`).

## Declaration

**TrolleyDeclaration** — one registry entry that matched.
- `entry_name`, `bus_file`, `carrier_bus_file: Option`, `carrier_part: usize` (0 = lead)
- `poles: [PoleGeometry; 2]`, `params: PoleParams`
- Rule: required keys missing or unreadable → the entry is rejected with the key named.

**PoleGeometry** (body frame) — `base` (x, forward, height), `contact_rest` (x, forward, height),
`length` (from base to contact), `shoe_height_offset`.

**PoleParams** — the global parameters overridden by the entry's own keys: angle limits, reach,
spring and damping, contact force limits, dewirement limits, raise/lower durations, speed limit
through special pieces.

## Pole state (per pole)

**PoleState**
- `phase: Hooked | Rising | OnWire | Dewired | Retracting | Lowering`
- `track: Option<(span, wire_side, s)>` — where the shoe is on the network
- `yaw`, `pitch` (degrees, body frame), `shoe` (body frame), `shoe_rotation`
- `contact_force`, `dewire: Option<DewireReason>`, `timer` (seconds in the current phase)

**DewireReason** — `Reach | Force | SideAngle | Gap | SwitchMiss | Retract | Stall`.

**Transitions** (phase 2 implements the first three; 3 the rest):

```text
Hooked --raise input--> Rising --shoe within capture of a wire--> OnWire
Rising --raise time spent, no wire--> Lowering --> Hooked
OnWire --lower input--> Lowering --> Hooked
OnWire --limit exceeded / gap--> Dewired --free rise, pause--> Retracting --> Hooked
OnWire --retract input--> Retracting --> Hooked
any --stall longer than reset time--> Hooked
```

**PoleInputs** (read from variables each frame): raise/lower command per side, manual grab per
side, retract command, branch demand.

**PoleOutputs** (written each frame): see [contracts/script-variables.md](contracts/script-variables.md).
