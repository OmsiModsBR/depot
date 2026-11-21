# Feature Specification: Steerable axles in Project Depot Physics

**Feature Branch**: one per delivery phase (see `plan.md`)

**Created**: 2026-11-21

**Status**: Clarified (2026-11-21)

**Input**: User description (maintainer, 2026-11-21): "Plan a change to the Depot physics: steerable
axles configurable in the physics, with deactivation and steering logic that scripts can change;
steerable axles take part in the bus's turning physics."

## Context

Today both engines steer a road vehicle the same way (`C-6` in `docs/physics/common-model.md`): the
driver sets a curvature `κ`, and **every** axle points at the centre of the turn on the
`[rot_pnt_long]` line, `δ_i = atan((y_i − y_turn) · κ)`. No axle is rigid: a tag axle behind the
line turns the other way by geometry, whether the real bus has a steered tag axle or not, and nothing
can lock it. The scripts only read the result (`Axle_Steering_<n>_L/R`); a steered tag axle that
locks at speed exists only as an animation. The `.bus` format of OMSI 2 has no per-axle steering
setting.

This feature adds, for **Project Depot Physics only** (spec 002, `D-*` rules), axles that are either
rigid or steerable, declared in the `.bus` file, and makes the turning of the bus follow from which
axles steer at each moment. The declaration is generic: any axle of any section of a vehicle (a
rigid bus, the front, middle or rear section of an articulated bus, each section with its own
`.bus` file) can be declared steerable. OMSI Classic keeps `C-6` and `C-9` unchanged.

## Clarifications

### Session 2026-11-21

- Q1: Where is the turning line when a bus declares steerable axles? → A (maintainer): it follows
  from the axles that do not steer at that moment (one rigid axle: on it; several: their
  load-weighted middle), and `[rot_pnt_long]` is no longer the turning line for that bus; locking a
  tag axle widens the turning circle (FR-006 to FR-009). Rejected: keeping `[rot_pnt_long]` as the
  line, where locking an axle would not change the turning circle.

- Q2: Are steerable axles on the rear section of an articulated bus in scope? → A (maintainer): yes,
  and generically: any axle of any section of the vehicle can be declared steerable. A section
  behind a joint turns on the curvature its joint gives it (`C-9`); its steerable axles point at the
  centre of that turn and its turning line follows from its own non-steering axles, as in the leading
  section (FR-015). Reading taken: the leading section's front axle is already steered by the driver,
  so it needs no declaration (a declaration for it is reported and ignored); every other axle,
  including the front axle of a section behind a joint, can be declared.
- Q3: Where does the declaration go in the `.bus` file? → A (maintainer): as new parameters of
  each `[newachse]`, tied to that axle, not as a section of its own, one parameter per value as the
  other axle parameters are (FR-001).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A steered tag axle makes the bus turn tighter (Priority: P1)

A mod author declares the third axle of a 15 m 6x2 bus as steerable. Driving it in Project Depot
Physics, the player turns at full lock in a smaller circle than with the same bus whose tag axle is
rigid, the tag axle turns against the front wheels as far as its own lock lets it, and the scripts
receive its real angle.

**Why this priority**: it is the feature itself: an axle that steers changes how the bus turns.

**Independent Test**: on test vehicle V2 (6x2, steered third axle, spec 002), drive a steady full-lock
circle at 5 km/h with the tag axle declared steerable and again declared rigid; compare the measured
turning radius and each axle's angle with the closed forms of the model document.

**Acceptance Scenarios**:

1. **Given** a 6x2 bus whose third axle is declared steerable with a 15° lock, **When** it turns at
   full lock at walking speed, **Then** it turns about the line of its rigid middle axle, its turning
   circle is the one the model gives for that line, and the tag axle stands at `atan((y_3 − y_2) · κ)`
   or at its 15° lock, whichever is smaller.
2. **Given** the same bus with no axle declared steerable apart from the front one, **When** it turns
   at full lock, **Then** its two rear axles both stay straight, the bus turns about their
   load-weighted middle, and both scrub (FR-010).
3. **Given** a bus with the key, **When** a script reads `Axle_Steering_<n>_L/R` of a rigid axle,
   **Then** it reads 0; of a steerable axle, its actual angle.
4. **Given** an 8x2 front section whose second axle is declared steerable (twin steer), **When** it
   turns, **Then** both front axles point at the centre of the turn within their own locks.

---

### User Story 2 - The steerable axle locks by itself at speed (Priority: P1)

A real steered tag axle is centred and locked above a speed. The mod author gives that speed in the
declaration; with no script involved, the axle returns smoothly to straight when the bus goes faster
than it, and the bus then turns as a bus with two rigid rear axles.

**Why this priority**: without it a steered tag axle steers at motorway speed, which is the main
thing real systems prevent; it is the default "deactivation logic" scripts can later replace.

**Independent Test**: on V2 with a lock speed of 30 km/h, run a slow S-bend (tag steers), accelerate
past 30 km/h (tag centres at its centring rate, then locks), slow down below it (tag steers again);
check the axle angle and the turning line over time, and that the yaw rate shows no step.

**Acceptance Scenarios**:

1. **Given** a steerable axle with lock speed 30 km/h, **When** the bus passes 30 km/h, **Then** the
   axle returns to straight at its centring rate and stays locked while the speed is above it.
2. **Given** the axle locked, **When** the speed falls below the lock speed, **Then** the axle
   steers again from its current angle at the same rate.
3. **Given** the axle locking or unlocking during a turn, **When** the turning line moves, **Then**
   it moves without a jump: no step in yaw rate or lateral speed larger than the steering itself
   would cause (SC-003).
4. **Given** a lock speed of 0, **Then** the axle never locks by speed.

---

### User Story 3 - Scripts take over the steering logic (Priority: P2)

A script author makes the tag axle lock in reverse, lock on a fault or a dashboard switch, or steer
by a law of its own (for example proportional to the front wheels). The script writes a mode and,
for its own law, an angle; the physics applies them in the same frame and the bus's turning follows
what the axle really does.

**Why this priority**: the maintainer asked for deactivation and steering logic that scripts can
change; it builds on stories 1 and 2.

**Independent Test**: a test script cycles the mode of V2's tag axle through every value at walking
speed in a steady turn; each mode is checked against the expected axle angle and turning line.

**Acceptance Scenarios**:

1. **Given** mode 0 (or a script that never declares the variable), **Then** the built-in logic of
   story 2 runs.
2. **Given** mode 1, **Then** the axle centres at its centring rate and stays locked, whatever the
   speed.
3. **Given** mode 2, **Then** the axle steers by the built-in geometric law and ignores the lock
   speed.
4. **Given** mode 3 and an angle written by the script, **Then** the axle moves to that angle
   (clamped to its lock) at its centring rate, and the turning line and scrub follow from that angle.
5. **Given** any mode, **Then** the script can read whether the axle is locked straight, for a
   dashboard lamp or a sound.

---

### User Story 4 - A rear section with a steered axle follows tighter (Priority: P2)

A mod author declares the last axle of the rear section of a 23 m 8x2 articulated bus (test vehicle
V5) as steerable. In a turn the rear section turns about the line of its rigid driven axle, its last
axle points at the centre of the rear section's turn, and the rear section cuts the corner less
than with that axle rigid. The same works for the middle and rear sections of a bi-articulated bus.

**Why this priority**: the maintainer wants the declaration generic, for any axle of any section;
articulated buses with a steered last axle are common in Brazil.

**Independent Test**: on V5, a steady turn at walking speed with the fourth axle declared
steerable and again rigid; compare the articulation angle and the rear section's swept path with the
closed forms of the model document.

**Acceptance Scenarios**:

1. **Given** V5 with its fourth axle declared steerable, **When** it turns steadily, **Then** the
   rear section turns about its third axle's line, the fourth axle stands at
   `atan((y_4 − y_3) · κ_rear)` or its lock, and the articulation angle is the closed form's for
   that turning line.
2. **Given** the same axle rigid, **Then** the rear section turns about the load-weighted middle of
   its two axles and both scrub (FR-010).
3. **Given** a rear section without any declaration, **Then** it behaves exactly as today (`C-9`).
4. **Given** the rear section's steerable axle locked by speed or by its script, **Then** the rear
   section's turning line moves continuously, as in story 2.

---

### Edge Cases

- The declaration sits on the leading section's front axle (steered by the driver already), or is
  given twice in the same axle: that declaration is ignored with a log line naming the file and the
  reason; the bus loads and the rest of it works (Principle IV).
- Only one or two of the three parameters given on an axle: the declaration is ignored with a log
  line naming the missing ones.
- An axle written in the old form without keywords (values only, one per line) cannot carry the
  declaration; it stays rigid or `C-6` as the rest of its section decides.
- A lock angle of 0 or below, or not a number; a negative lock speed; a centring rate of 0 or below;
  a missing value: the
  declaration is ignored with a log line. Nothing is corrected silently (Principle III).
- Every axle of a section steerable (no rigid axle at all): there is no geometric turning line, so
  that section turns about its `[rot_pnt_long]` line as in `C-6` (leading section) or `C-9` (a
  section behind a joint), and the log says so once.
- A section behind a joint with steerable axles while its joint is straight: its axles stand
  straight, as the curvature is 0.
- The bus runs OMSI Classic (no `[depot_physics]` key and the setting is Classic): the declaration
  is read, ignored, and the log says it applies to Project Depot Physics only; `C-6` runs unchanged.
- An AI copy of a bus with the declaration: the AI keeps its own steering (spec 002 D6: the key
  applies to the player's vehicle only); `Axle_Steering` of AI copies stays as today.
- The script writes an unknown mode or an angle that is not a number: the axle behaves as mode 0,
  and the log says so once per vehicle.
- A steerable axle clamped at its lock does not point at the centre of the turn: it scrubs like a
  rigid axle off the line, in proportion to its error (FR-010).
- Reversing: the built-in logic does not lock the axle in reverse; scripts that want it use mode 1.
- A saved situation restores each steerable axle's angle and locked state.

## Requirements *(mandatory)*

### Functional Requirements

**Declaration**

- **FR-001**: In the `.bus` file of any section of a vehicle, an axle MUST be declarable steerable
  by new Project Depot parameters inside its own `[newachse]`, one parameter per value like
  `achse_long` and the others, which OMSI 2 and its derivatives ignore: the axle's lock angle
  (degrees, each side), lock speed (km/h, 0 = never) and centring rate (degrees per second). All
  three are required together, so the game invents none (Principle III).
- **FR-002**: Before any implementation, the parameter MUST be checked in OMSI 2 (black box, as
  `Q1` of spec 002): a stock bus with the parameter added to an axle loads and drives in OMSI 2
  exactly as without it.
- **FR-003**: In a section with at least one valid declaration, every axle not declared steerable is
  rigid (always straight), except the leading section's front axle, which the driver steers as
  today. A section without any declaration runs exactly as today in both engines.
- **FR-004**: The declaration MUST take effect only in Project Depot Physics; OMSI Classic ignores it
  (`C-6` unchanged) and logs that once.
- **FR-005**: Invalid declarations MUST be reported in the log and ignored without crashing and
  without being corrected (edge cases above).

**Turning**

- **FR-006**: The turning line MUST follow from the axles that do not steer at that moment (rigid,
  locked, or held at an angle by a script; an axle on its way to or from locked counts by how far it
  has gone):
  on the single such axle, or the static-load-weighted middle of several; with none, the
  `[rot_pnt_long]` line of `C-6`.
- **FR-007**: The front wheels' lock angle MUST stay the one `C-6` gives from `[inv_min_turnradius]`
  and `[rot_pnt_long]` as the file has them; the full-lock curvature then follows from the current
  turning line, so the turning circle changes with what the steerable axles do.
- **FR-008**: Each steerable axle following the geometric law MUST point at the centre of the turn,
  `atan((y_i − y_turn) · κ)`, clamped to its lock angle; tyres keep their own Ackermann angles as in
  `C-6`.
- **FR-009**: The turning line MUST move continuously when an axle locks, unlocks or changes angle:
  no step in yaw rate or lateral speed beyond what the steering itself causes.
- **FR-010**: An axle that does not point at the centre of the turn (rigid off the line, clamped at
  its lock, held by a script) MUST scrub: it takes force across in the tyres' hold of `D-3` (so the
  bus lets go sooner on a tight bend at speed) and resists the motion (the bus needs more drive to
  keep its speed at full lock).
- **FR-011**: While the tyres slide (`C-7`), each tyre's force MUST use its real angle (rigid axles
  at 0), so a steered tag axle changes how a sliding bus yaws.

**Logic and scripts**

- **FR-012**: The built-in logic of a steerable axle MUST follow the geometric law (FR-008) below its
  lock speed, and centre and lock above it, moving at its centring rate in both directions.
- **FR-013**: Scripts MUST be able to choose, per steerable axle and each frame, one of: built-in
  logic (default, also when the script never declares the variable), locked straight, steering
  without the speed lock, and an angle written by the script (clamped to the lock, reached at the
  centring rate). Variable names are Project Depot's own and carry no meaning in OMSI 2.
- **FR-014**: The physics MUST hand the scripts each axle's actual angle through the existing
  `Axle_Steering_<n>_L/R` (0 for rigid axles of a keyed bus) and, per steerable axle, whether it is
  locked straight.

**Scope**

- **FR-015**: The feature MUST work on any section. In a section behind a joint, the curvature comes
  from the joint (`C-9`, Depot's `follower_kappa`); its steerable axles point at the centre of that
  turn (FR-008) and their tyres steer, its rigid axles' tyres stay square to the body, and its
  turning line follows FR-006 from its own non-steering axles. A section behind a joint without any
  declaration keeps `C-9` unchanged.
- **FR-016**: AI vehicles are not affected (spec 002 D6).
- **FR-017**: The rules MUST be written in `docs/physics/depot-model.md` as new `D-*` entries with
  their equations, inputs, outputs, module and status, and `common-model.md` `C-6` MUST point to
  them.
- **FR-018**: The new script variables and the `.bus` key MUST be documented in `docs/FORMATS.md`.
- **FR-019**: A change in Project Depot Physics for keyed buses carries the `physics-delta` label;
  the physics traces of every bus without the key MUST stay identical.

### Key Entities

- **Steerable axle declaration**: lock angle, lock speed, centring rate; belongs to one
  `[newachse]` of a section's `.bus` file; valid or reported as invalid.
- **Steerable axle state**: per steerable axle, its current angle, target angle, mode chosen by the
  script, and whether it is locked straight; saved with the situation.
- **Turning line**: per section, the longitudinal position the body turns about this step, derived
  from that section's non-steering axles and their loads (FR-006).
- **Script steering controls**: per steerable axle, the mode and angle the script writes and the
  locked flag it reads.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On V2 at full lock and 5 km/h, the turning radius with the tag axle steering is
  smaller than with it locked, and each matches the model's closed form within 2 % (automated test).
- **SC-002**: The physics traces of every existing scenario (buses without the key) stay
  bit-identical, in both engines.
- **SC-003**: Locking or unlocking the tag axle in a steady turn at 25 km/h never makes the yaw rate
  jump by more than 2 % between two frames (automated test).
- **SC-004**: Each script mode takes effect in the first physics step after the script writes it, in
  100 % of the test cases.
- **SC-005**: No invalid declaration or script value in the test set (missing values, front axle,
  duplicates, zero or negative values, not-a-number, unknown modes) crashes the game; each one
  produces its log line.
- **SC-006**: At full lock and constant speed, V2 with both rear axles rigid needs measurably more
  drive than with the tag axle steering (automated test).
- **SC-006a**: On V5 in a steady turn, the articulation angle with the fourth axle steering and with
  it rigid each match the model's closed form within 2 % (automated test); the rear section's swept
  path is narrower with the axle steering.
- **SC-007**: Frame time of a keyed bus versus the same bus without the key, same machine and
  scenario, differs by no more than measurement noise; the measurements go in the pull request
  (Principle VII).
- **SC-008**: The maintainer drives a keyed 6x2 bus in the game and confirms the tighter turn, the
  lock at speed and a script mode by eye.

## Assumptions

- There is no OMSI 2 behaviour to match: OMSI 2 has no rigid or steerable axle setting, and every
  axle follows `C-6`. The rules are Project Depot Physics' own and are described as such (Principle
  II); only the key's harmlessness in OMSI 2 needs an in-game check (FR-002).
- The built-in logic does not lock in reverse; real systems differ, and scripts cover it (mode 1).
- The driver's steering input and its rate stay as they are; only what the axles do with the
  curvature changes.
- Players and modders who do not use the key see no change.

## Dependencies

- Spec 002 (two physics modes) is implemented: `[depot_physics]`, the `Engine` split and the `D-*`
  rules exist.
- The physics trace harness and test vehicles V1-V5 (`crates/omsi-sim/tests/`).
- A train runs one engine, chosen by its leading section's file (spec 002): the declarations of the
  sections behind take effect only when the leading section runs Project Depot Physics.
- `common-model.md` (`C-7`) says the steering of a rear axle's tyres is modelled in neither engine
  (maintainer, 2026-11-12); this feature lifts that for Project Depot Physics only, and the note is
  updated to say so. OMSI Classic keeps it.
