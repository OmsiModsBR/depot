# Feature Specification: Two physics modes (OMSI Classic and Project Depot Physics)

**Feature Branch**: one per delivery phase (see [plan.md](plan.md))

**Created**: 2026-11-08

**Status**: Draft

**Input**: User description: issue #677, its decision comment, and the maintainer's request of
2026-11-08: "Plan the two physics modes. The code of each physics is kept apart, so that the switch
changes from one module to the other where they differ. Depot physics is switched on in two ways: a
setting in the launcher, and a keyword in the `.bus` file that forces Depot physics even when the
setting is OMSI 2 Classic. The switch applies to the player's vehicle and to the AI vehicles. A new AI
implementation brings the AI in line with OMSI 2's; the current Depot AI physics becomes part of the
new Depot physics. One logical-mathematical model document for each physics engine. Tests validate the
implementation against the model with at least five kinds of vehicle: a 4x2 bus, a 6x2 bus with a
steered third axle, a 6x2 articulated bus, a bi-articulated bus and a 23 m articulated 8x2 bus."
Refined with the maintainer's decisions D1-D5 (2026-11-08) recorded in [plan.md](plan.md).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Mods behave as they were tuned for OMSI 2 (Priority: P1)

A player who has never touched the new setting starts Depot with a bus mod made for OMSI 2. The bus
stands, drives and leans as the mod's author tuned it in OMSI 2: the springs share the weight between
the axles as OMSI 2 does, so a bus that stands level in OMSI 2 stands level in Depot.

**Why this priority**: almost every bus a player owns was tuned for OMSI 2. If the default mode does
not keep them as tuned, every other part of this feature only adds options to a broken default.

**Independent Test**: with the default setting, put the Caio Millennium V articulated (Mi5) at a flat
stop and compare each axle's spring travel and the body's attitude with OMSI 2 from the same camera.

**Acceptance Scenarios**:

1. **Given** a fresh installation (no `physics` entry saved), **When** the player starts a session,
   **Then** every vehicle runs OMSI Classic physics.
2. **Given** a bus whose centre of gravity lies ahead of its model origin, **When** it stands at rest
   in OMSI Classic, **Then** each axle carries the share it carries in OMSI 2 (the Mi5: front, middle
   and rear springs sag 9.1, 9.8 and 9.1 cm) and the body stands level.
3. **Given** the default setting, **When** the player drives the existing physics scenarios, **Then**
   the player's vehicle moves exactly as before this feature.

---

### User Story 2 - The player chooses Depot physics in the launcher (Priority: P2)

A player who prefers Depot's deliberate improvements opens the launcher, picks "Project Depot Physics"
in the "Physics" list, and starts a session. Every vehicle, the player's and the AI's, now uses Depot
physics. The launcher says in one line that mods tuned for OMSI 2 may stand or handle differently.

**Why this priority**: it is the main way into the Depot mode and the reason the mode exists, but it
only makes sense once the default (Story 1) is safe.

**Independent Test**: choose the option, start a session, and check that the session log names Depot
physics for every spawned vehicle and that the Mi5 now stands with its weight shared by its centre of
gravity (front springs sag more than the others).

**Acceptance Scenarios**:

1. **Given** the launcher, **When** the player opens the gameplay settings, **Then** a "Physics" list
   offers "OMSI Classic" (selected by default) and "Project Depot Physics", with the one-line notice.
2. **Given** "Project Depot Physics" saved, **When** a session starts, **Then** the player's vehicle and
   every AI road vehicle use Depot physics.
3. **Given** "Project Depot Physics", **When** a bus with its centre of gravity ahead of its origin
   stands at rest, **Then** the axle nearer the centre of gravity carries more weight than in OMSI
   Classic, by the amount the Depot model document gives.
4. **Given** the player switches back to "OMSI Classic", **When** the next session starts, **Then**
   every vehicle without the `.bus` keyword is back to OMSI Classic.

---

### User Story 3 - A modder ships a bus made for Depot physics (Priority: P2)

A modder tunes a bus for Depot physics (for example, springs sized for the weight shared by the centre
of gravity) and adds the `[depot_physics]` line to its `.bus` file. Players get the bus as tuned, even
when their setting is OMSI Classic. The same file still loads in OMSI 2, openOMSI and other forks,
which ignore the line.

**Why this priority**: it lets optimised and ordinary mods share a session without the player having
to switch modes per bus (#677 decision 4).

**Independent Test**: add `[depot_physics]` to a test bus, keep the setting at OMSI Classic, spawn it
as the player's vehicle and as an AI vehicle; both run Depot physics while other vehicles stay Classic.
Load the same file in OMSI 2: it loads and drives as without the line.

**Acceptance Scenarios**:

1. **Given** the setting OMSI Classic and a bus with `[depot_physics]`, **When** it spawns, **Then** it
   uses Depot physics and the log says why (the key, and the file it is in).
2. **Given** an articulated bus whose front section's file has the key, **When** it spawns, **Then**
   every section of that vehicle uses Depot physics.
3. **Given** an articulated bus whose key is only in a rear section's file, **When** it spawns, **Then**
   the vehicle follows the setting and the log reports that the key has no effect there.
4. **Given** a `.bus` file with the key, **When** OMSI 2 loads it, **Then** the vehicle loads, drives
   and stands as without the key, and OMSI 2's log shows nothing new.
5. **Given** the setting "Project Depot Physics", **When** any vehicle spawns, **Then** it uses Depot
   physics whether or not it has the key (no file can force OMSI Classic).

---

### User Story 4 - AI traffic behaves as in OMSI 2 (Priority: P2)

In OMSI Classic the AI traffic moves as OMSI 2's AI does: buses and cars keep to their lane paths
through bends, pull away and stop according to their own engines and brakes, and stand on their wheels
as their files set them. When the player hits an AI vehicle, it becomes a free body that rolls and
reacts like any physical object, with no driver, until it is cleared from the scene.

**Why this priority**: the request makes today's AI part of the Depot mode, so the default mode needs
an AI of its own that matches OMSI 2.

**Independent Test**: on a stock map, film the same AI bus through the same junction in OMSI 2 and in
Depot (OMSI Classic); then nudge an AI car with the player's bus in both.

**Acceptance Scenarios**:

1. **Given** OMSI Classic, **When** an AI bus turns through a junction, **Then** its rear axle tracks
   its lane path and does not run wide or cut the corner beyond what OMSI 2 shows.
2. **Given** OMSI Classic, **When** the player's vehicle hits an AI vehicle, **Then** the AI vehicle
   becomes a free body without a driver and stays so until it is cleared from the scene.
3. **Given** OMSI Classic, **When** an AI articulated bus stands at a stop, **Then** it stands with the
   weight shared as the player's bus is in OMSI Classic.
4. **Given** "Project Depot Physics", **When** AI vehicles drive, **Then** they move exactly as AI
   vehicles move in Depot before this feature.

---

### User Story 5 - Maintainers and modders can read and check each physics (Priority: P3)

A maintainer or a modder opens the model document of either engine and finds every equation it uses,
with its inputs (which `.bus` value, which script variable), its outputs and whether each OMSI Classic
rule has been validated against OMSI 2. Every equation is checked by an automated test on five kinds
of bus.

**Why this priority**: it is what keeps the two engines honest over time, but players do not see it
directly.

**Independent Test**: pick any equation in either document and find the tests that name it; run them.

**Acceptance Scenarios**:

1. **Given** the OMSI Classic and Depot model documents, **When** a reader looks up an equation,
   **Then** it has an identifier, its inputs and outputs, and (for OMSI Classic) "validated" with how,
   or "to validate" with the check that will settle it.
2. **Given** the test suite, **When** it runs, **Then** each of the five test buses (4x2; 6x2 with a
   steered third axle; 6x2 articulated; bi-articulated 8x2 driven on the front section's second axle;
   23 m articulated 8x2 driven on the rear section's first axle, with a steered fourth axle) is
   checked in both engines, for the player's vehicle and for the AI, against the documents'
   equations.

### Edge Cases

- The saved `physics` value is misspelt or unknown: the game uses OMSI Classic and writes one warning
  to the log.
- The `[depot_physics]` line appears twice, or has stray lines under it: the vehicle uses Depot
  physics, the stray lines are ignored and reported once; nothing else in the file is affected.
- A saved situation is loaded after the player changed the setting: each vehicle takes the engine the
  current setting and its file give; the log names it.
- A session mixes vehicles of both engines (setting Classic, one bus with the key): each keeps its own
  engine, and they can still collide with each other.
- An AI vehicle released after a hit is cleared from the scene: the free body disappears with it; a
  new AI vehicle spawned in its place follows its own engine again.
- A rail vehicle (tram, train) or an aircraft: it moves the same in both modes; the key and the setting
  change nothing for it.
- A Classic-tuned bus driven in Depot stands unlevel: shown as it is; nothing corrects it (Principle
  III).

## Requirements *(mandatory)*

### Functional Requirements

**Selecting the engine**

- **FR-001**: The game MUST offer two physics engines: OMSI Classic and Project Depot Physics.
- **FR-002**: OMSI Classic MUST be the default whenever the player has not chosen otherwise.
- **FR-003**: The launcher MUST let the player choose the engine in a "Physics" list, saved with the
  other settings, and MUST show a one-line notice that mods tuned for OMSI 2 may stand or handle
  differently in Depot physics.
- **FR-004**: A vehicle whose `.bus` file contains the `[depot_physics]` line (no value lines) MUST
  use Depot physics whatever the setting says.
- **FR-005**: There MUST be no way for a file or a setting to force OMSI Classic on a vehicle whose
  setting or file selects Depot, and no "automatic" mode.
- **FR-006**: In an articulated or bi-articulated vehicle, only the key in the front section's file
  MUST count, and it MUST apply to every following section; a key found only in a rear section's
  file MUST have no effect and MUST be reported in the log.
- **FR-007**: The engine MUST apply equally to the player's vehicle and to AI road vehicles.
- **FR-008**: The engine of each vehicle MUST be fixed when the vehicle appears and written to the log
  with the reason (the setting, or the key and its file).
- **FR-009**: The `[depot_physics]` line MUST NOT change how OMSI 2, openOMSI or another fork the
  maintainers name loads or drives the vehicle (verified before the key is introduced).
- **FR-010**: A malformed setting value or key MUST NOT stop the game or the vehicle from loading; it
  is reported and a valid engine is used (Principle IV).

**What each engine does**

- **FR-011**: OMSI Classic MUST share a vehicle's weight between its axles about the model's origin,
  as OMSI 2 does; Project Depot Physics MUST share it about the vehicle's centre of gravity (`[cog]`).
- **FR-012**: For now, the weight sharing of FR-011 MUST be the only difference between the two engines
  for the player's vehicle; every other behaviour MUST be the same in both (decision D2). Changes
  that only add behaviour OMSI 2 lacks (articulated bus work, standing passengers, skip stop) MUST
  stay on in both engines.
- **FR-013**: With OMSI Classic and no key, the player's vehicle MUST move exactly as it does before
  this feature.
- **FR-014**: In Project Depot Physics, AI road vehicles MUST move exactly as AI road vehicles move
  before this feature.
- **FR-015**: In OMSI Classic, AI road vehicles MUST use a new AI behaviour that matches OMSI 2's AI in
  each point confirmed by an in-game comparison: keeping to the lane path, pulling away and stopping
  by their own drive and brakes, standing on their wheels, rear sections following the path, and
  weight shared about the model origin.
- **FR-016**: In OMSI Classic, an AI vehicle hit by the player's vehicle MUST become a free body without
  a driver until it is cleared from the scene (decision D3).
- **FR-017**: The code of each engine MUST be kept apart wherever the engines differ, and shared
  wherever they do not, so that the switch selects one module or the other.
- **FR-018**: Rail vehicles and aircraft MUST behave the same in both engines.

**Documents and tests**

- **FR-019**: There MUST be one logical-mathematical model document for OMSI Classic and one for
  Project Depot Physics, with the equations, calculations and concepts each uses; equations both share
  MAY be written once in a common part that both documents index.
- **FR-020**: Every equation MUST carry an identifier, its inputs (`.bus` values, script variables),
  its outputs, and, for OMSI Classic, whether it is validated against OMSI 2 (and how) or still to
  validate (and by which check).
- **FR-021**: Automated tests MUST check the implementation against the documents' equations, each test
  naming the equations it checks, for five buses: a 4x2 bus; a 6x2 bus with a steered third axle; a
  6x2 articulated bus; a bi-articulated 8x2 bus driven on the front section's second axle; and a 23 m
  articulated 8x2 bus driven on the rear section's first axle with a steered fourth axle.
- **FR-022**: Those tests MUST cover both engines, the player's vehicle and AI vehicles.
- **FR-023**: The `.bus` key and the launcher setting MUST be documented publicly (formats, modding and
  user guides) before players can choose the Depot engine.

### Key Entities

- **Physics engine**: OMSI Classic or Project Depot Physics; one per vehicle, fixed when it appears.
- **Physics setting**: the player's saved choice of engine; default OMSI Classic.
- **Depot physics key**: the `[depot_physics]` line in a vehicle's `.bus` file; forces Depot for that
  vehicle and all its sections when in the front section's file.
- **Model document**: the description of one engine's equations, each with an identifier, inputs,
  outputs and validation status.
- **Test vehicle**: one of the five synthetic buses the conformance tests use.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: With the default setting, the player's vehicle behaves identically to the release before
  this feature in 100% of the existing physics scenarios.
- **SC-002**: With "Project Depot Physics", AI vehicles behave identically to the release before this
  feature in 100% of the existing AI scenarios.
- **SC-003**: The Mi5 articulated stands with its three axles within 1 cm of the sag OMSI 2 shows, in
  OMSI Classic.
- **SC-004**: Every equation in both model documents is checked by at least one automated test, and
  every test bus is checked in both engines for the player and the AI (100% coverage of the matrix in
  [contracts/test-vehicles.md](contracts/test-vehicles.md)).
- **SC-005**: A bus file with `[depot_physics]` loads and drives in OMSI 2 with no visible or logged
  difference from the same file without it.
- **SC-006**: In OMSI Classic, an AI bus turning through the reference junction keeps its rotation
  point within 1 cm of its lane path.
- **SC-007**: The game's frame rate with 40 AI vehicles is not lower in OMSI Classic than before this
  feature, measured on the same machine and scene.
- **SC-008**: A player can switch engine in the launcher and see the result in the next session in
  under one minute, without editing any file.

## Assumptions

- The engine is chosen in the launcher before a session; the game's own menu does not change it
  mid-session (the engine is fixed when each vehicle appears).
- Rail vehicles and aircraft are out of scope: neither engine overrides them.
- The Classic AI implements only the points an in-game comparison with OMSI 2 confirms (KA-1..KA-7 in
  [research.md](research.md)); KA-6 (free body after a hit) is already confirmed by the maintainer.
- Saved situations do not store the engine; it is resolved again when the situation loads.
- Mods' scripts read and write the same variables in both engines; nothing in a script needs to change.
- The five test buses are synthetic and built for the tests; no OMSI 2 content enters the repository.
  Checks on real mods (the Mi5 first) run only on a machine with OMSI 2 installed.
- Further overrides beyond the weight sharing are out of scope until the maintainer decides them one
  by one.
