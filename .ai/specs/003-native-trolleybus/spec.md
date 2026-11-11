# Feature Specification: Native trolleybus support

**Feature Branch**: one per delivery phase (see the delivery order in issue #525; the plan will
name each branch)

**Created**: 2026-11-11

**Status**: Draft

**Input**: User description: issue #525 ("Proposal: native trolleybus support (two content
conventions, one internal model)") and the maintainer's decisions of 2026-11-11 recorded under
Clarifications. In short: Depot runs trolleybus content without plugins. The overhead wire is read
from the map, the poles of a declared trolleybus are simulated by the game, and the result reaches
the vehicle through script variables, so existing trolleybus content works without being rewritten.
Vehicles and maps without trolleybus content behave exactly as before.

## Clarifications

### Session 2026-11-11

- Q: Which content convention is Depot's native one? → A: The plugin-based convention (vehicle
  registry list, global parameter file, wire splines and switch objects in the map) is Depot's native
  trolleybus format. The native-script convention is read later through an adapter.
- Q: AI trolleybuses in the first release? → A: Player vehicle only. AI trolleybuses are a later,
  separate feature.
- Q: When does the native-script-convention adapter come? → A: Last, after the plugin-based path is
  proven in the game.
- Q: Minimal traction? → A: Without contact the game reports it to the vehicle (no wire present, the
  warning variables) and the vehicle's own script decides on power, battery mode and the rest.
  No voltage model in the foundation.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Poles stand on the wire of a plain line (Priority: P1)

A player starts a declared trolleybus (rigid or articulated, poles on the rear section included) on a
map with overhead wire. The poles are drawn, they can be raised onto the wire, and the shoes follow
both wires along straights and curves while the body rolls and pitches. The vehicle's script sees
the same variables it saw with the plugin (pole angles, shoe position, contact, wire present).

**Why this priority**: Today the poles are not even drawn in Depot, because their visibility comes
from a variable nobody writes. This story makes existing trolleybus content usable at all.

**Independent Test**: Spawn the trolleybus on a straight and on a curved wired street, raise the
poles, drive a lap; the shoes stay on the wires, the script reports contact, and a diesel bus on the
same map behaves as before.

**Acceptance Scenarios**:

1. **Given** a declared trolleybus at a wired street with poles on the hooks, **When** the player
   raises them, **Then** each pole rises and its shoe settles on the wire of its side, and the
   script variable for contact turns on.
2. **Given** the poles on the wire, **When** the bus corners, brakes hard or crosses a bump,
   **Then** the shoes stay on the wire while the body rolls and pitches under them.
3. **Given** a session started far from where the last one ended, poles down, **When** the poles
   are raised under a wire, **Then** the wire is found without any position guess.
4. **Given** a map whose wire has reversed splices, loose joints up to about a metre, or a very
   short span, **When** the bus drives through them, **Then** the shoes pass without losing
   contact.
5. **Given** a declared trolleybus on a map with no wire, **When** the player drives, **Then** the
   poles stay on the hooks, the script sees no wire present, and nothing else changes.

---

### User Story 2 - Dewirement, retriever and lowering (Priority: P2)

When a pole cannot follow the wire (too far sideways, too much force, too fast through a special
piece), it leaves the wire, rises freely, pauses and is pulled down by the retriever to its hook.
The player can lower the poles on purpose with the vehicle's retract control, in phases.

**Why this priority**: Without dewirement the poles would follow the wire through anything, which is
neither believable nor what trolleybus content expects (warning sounds and lamps hang on it).

**Independent Test**: Steer far from the wire until a pole leaves; watch it rise, pause and come
down to the hook; the script sees the dewirement and its side. Use the retract control on the wire.

**Acceptance Scenarios**:

1. **Given** poles on the wire, **When** the bus moves sideways beyond the pole's reach, **Then**
   that pole dewires, rises, pauses and is pulled down to its hook, and the script sees which side
   dewired.
2. **Given** poles on the wire, **When** the player uses the retract control, **Then** both poles
   leave the wire and go down to the hooks in visible phases.
3. **Given** the game paused or the window in the background for a short moment, **When** play
   resumes, **Then** the poles are still on the wire; only a clearly long stall resets them.

---

### User Story 3 - Switches and crossings (Priority: P3)

At a wire switch the poles follow the branch the vehicle asks for (by the vehicle's own demand, as
the content convention defines), through Y switches and left/right 20° branch switches, two
switches in a row included. At crossings the shoes pass the crossing piece.

**Why this priority**: Real lines have switches; without them only a single plain line can be
driven.

**Independent Test**: Drive a route with a Y switch, a 20° branch and two consecutive switches,
asking for each branch in turn; the poles take the asked branch every time.

**Acceptance Scenarios**:

1. **Given** a switch ahead, **When** the vehicle asks for a branch, **Then** both shoes take that
   branch and continue on its wire.
2. **Given** a neighbouring wire about 1.9 m away beside a switch, **When** the bus passes, **Then**
   that neighbouring wire never captures a shoe.
3. **Given** two switches in a row, **When** the bus passes both, **Then** each exit wire is
   captured.
4. **Given** a merging switch, **When** the bus enters from either side, **Then** the shoes join the
   common wire.

---

### User Story 4 - Native-script convention (Priority: P4)

Trolleybus content written for the native-script convention (wire network described in text next to
the map, poles declared in the vehicle file) runs through the same simulation.

**Why this priority**: Decided last: only after the plugin-based path is proven in the game.

**Independent Test**: One vehicle and one map of that convention run with poles on the wire, without
any change to the simulation.

**Acceptance Scenarios**:

1. **Given** content of the native-script convention, **When** it is loaded, **Then** it behaves
   like the equivalent plugin-based content.

---

### Edge Cases

- A vehicle without a declaration is never a trolleybus, even if it has pole-like meshes or long
  thin lights.
- A declared vehicle whose declaration is broken (missing file, unreadable value): the error is
  logged, the vehicle runs as an ordinary bus, and the game does not stop.
- Wire with a gap larger than the join tolerance: the shoe leaves the wire there (dewirement), it
  is not bridged.
- A branch starting in the middle of a span.
- Low and high frame rates: tolerances are measured in time, so the same drive gives the same
  result at 20 and at 200 frames per second.
- Maps with world coordinates (tiles larger than 300 m) and wire spanning tile borders.
- Tiles loaded and unloaded around the bus while the poles are on the wire.
- Switching vehicles, respawning, or leaving the session: no state is left behind.
- The DirectX 9 renderer: the feature works, with no sparks or arcs.

## Requirements *(mandatory)*

### Functional Requirements

**Activation and compatibility**

- **FR-001**: A vehicle MUST be treated as a trolleybus only when it is declared in the trolleybus
  vehicle registry of the plugin-based convention (or, later, by the native-script convention).
- **FR-002**: Vehicles and maps without trolleybus content MUST give identical results before and
  after the feature (motion, lights, meshes, script variables), and a map without wire MUST cost
  nothing measurable.
- **FR-003**: A setting MUST allow turning trolleybus support off entirely, for comparison.

**Content reading (plugin-based convention, native)**

- **FR-004**: The game MUST read the vehicle registry and the global parameter file of the
  plugin-based convention, first from Depot's content folder and, when absent there, from the OMSI 2
  folder; the OMSI 2 folder is never written.
- **FR-005**: The game MUST recognise wire splines by the convention's list of spline name keywords
  and switch, crossing and marker objects by their file names, as the convention defines them.
- **FR-006**: Every parameter the content provides (pole base and contact positions, lengths, angle
  limits, spring and damping, dewirement limits, raise and lower durations, speed limit through
  special pieces) MUST come from the content files; where the content gives no value, the
  convention's own default applies.
- **FR-007**: The wire network MUST be built off the main thread at map load and with the tiles, so
  spawning never waits on it.

**Simulation (player vehicle)**

- **FR-008**: Each pole MUST be simulated in the frame of the drawn body of the section that carries
  it (the rear section on articulated vehicles), so it follows suspension roll and pitch.
- **FR-009**: The game MUST write, before the vehicle scripts run each frame, the script variables of
  the plugin-based convention for pole angles, shoe position and rotation, contact force, contact,
  wire present, dewirement per side, speed limit and engine trip; and read its inputs (raise, lower,
  manual grab, retract, branch demand).
- **FR-010**: Variables the model only reads in visibility or animation MUST exist even when no
  script declares them.
- **FR-011**: Joints within the join tolerance, reversed splices and very short spans MUST be
  followed without contact loss.
- **FR-012**: Dewirement MUST follow the content's limits (reach, force, sideways angle) with
  time-based tolerances; after it the pole rises freely, pauses and is lowered to the hook by the
  retriever.
- **FR-013**: A switch MUST be entered only when the shoes are close to it and aligned with it
  sideways; the exit branch follows the vehicle's demand; consecutive switches and 20° branches MUST
  work.
- **FR-014**: Without contact the game MUST report it through the variables (no wire present, the
  warning); it MUST NOT cut or change motive power on its own beyond what the vehicle's script does
  with those variables.
- **FR-015**: A short stall (pause, window in the background, a loading hitch) MUST NOT drop the
  poles; only a clearly long one resets them.
- **FR-016**: Each relevant decision (capture, joint, switch entry and exit, dewirement and its
  reason) MUST be written as one line of the detailed log, with the frame number.

**Out of scope for this feature**

- **FR-017**: AI trolleybuses, a voltage or electrical model, sparks and arcs, and contact sounds are
  NOT part of this feature; each comes later as its own opt-in feature.

### Key Entities

- **Wire network**: the overhead wires of the loaded map: spans (lines with height, two wires a
  fixed distance apart), the joints between spans, switches (three spans meeting, with their kind and
  trigger point), crossings and sound markers.
- **Trolleybus declaration**: one registry entry: which vehicle files it applies to, which section
  carries the poles, pole base and contact positions on each side, and the vehicle's own overrides of
  the global parameters.
- **Pole state**: per pole: on the hook, rising, on the wire (which span, where along it), dewired
  and why, retracting; its angles, shoe position and contact force.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On the maintainer's test maps (Brigadeiro Sampaio and Projeto Metropole Editado V2),
  the four declared trolleybuses in use (Caio Millennium II O500UA, Trollino 18, Busscar B10M, Busscar
  Urbanuss) show their poles and keep contact over a full route without switches, with no unwanted
  dewirement.
- **SC-002**: The same routes with switches are driven with every asked branch taken, at 30 and at
  144 frames per second.
- **SC-003**: The non-trolleybus regression list (stock maps and buses through `omsi-check`, and the
  physics and state traces) gives identical results with the feature on and off.
- **SC-004**: Frame time on a map without wire does not change; on a wired map the cost stays within
  what the pull request measures and justifies (Principle VII).
- **SC-005**: Spawning a trolleybus on a wired map takes no longer than spawning the same map with a
  diesel bus.

## Assumptions

- The plugin-based convention is described from its public content (registry keys, parameter file,
  map object and spline names) and from observed behaviour in the game; no third-party code or
  binary analysis is used or quoted (Principle VIII).
- Depot reads its own copy of the registry and parameter file from its content folder, so a player
  can keep the OMSI 2 plugin's files untouched for OMSI 2.
- Only the player vehicle is simulated; an AI trolleybus spawned by the timetable shows its poles as
  its content draws them without simulation.
- Sounds and sparks of the convention's parameter file are read but not played in this feature.
