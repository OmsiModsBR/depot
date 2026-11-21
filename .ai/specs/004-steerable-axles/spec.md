# Feature Specification: Steerable axles in Project Depot Physics

**Feature Branch**: one per delivery phase (see `plan.md`)

**Created**: 2026-11-21

**Status**: Clarified (2026-11-21, including a `/speckit-clarify` session)

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
- Q4: What logic does the game run by itself? → A (maintainer): none for locking: the speed lock
  is removed, and each vehicle's script locks or steers the axle through
  `Axle_Steer_Mode_<n>`. Mode 0 is the basic mode of a 6x2-4: the axle turns opposite to the
  steering axle, in proportion to its configured lock (FR-008, FR-012, FR-013).
- Q5 (`/speckit-clarify`): In mode 0, which way does an axle ahead of the turning line turn? → A:
  by its side of the turning line: an axle behind the line turns opposite to the steering axle (tag
  axle), an axle ahead of it turns the same way (twin steer second front axle); no script needed
  for either (FR-008).
- Q6 (`/speckit-clarify`): In mode 0, what does a steerable axle of a section behind a joint follow?
  → A: the joint's bend, not the driver's steering wheel: its steering fraction is the curvature
  its joint gives it over the leading section's full-lock curvature, so it follows the real bend of
  the section even while the driver is already straightening (FR-008, FR-015).
- Q7 (`/speckit-clarify`): Do AI copies of a bus with steerable axles show them steering? → A: no,
  the AI stays as today: it ignores the parameters and every axle points at the centre as in `C-6`,
  at no cost, even though it looks different from the player's bus (FR-016).
- Q8 (`/speckit-clarify`): Does an axle in mode 2 (script angle) count as rigid for the turning
  line? → A: yes: an axle held by the script at an angle weighs on the turning line like a locked
  one (mode 1) and scrubs by how far it is from pointing at the centre (FR-006, FR-010).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A steered tag axle makes the bus turn tighter (Priority: P1)

A mod author declares the third axle of a 15 m 6x2 bus as steerable (a 6x2-4). Driving it in
Project Depot Physics, the tag axle turns against the front wheels in proportion to the steering, up
to its own lock at full lock; the player turns in a smaller circle than with the same bus whose tag
axle is rigid, and the scripts receive the tag axle's real angle.

**Why this priority**: it is the feature itself: an axle that steers changes how the bus turns.

**Independent Test**: on test vehicle V2 (6x2, steered third axle, spec 002), drive a steady full-lock
circle at 5 km/h with the tag axle declared steerable and again declared rigid; compare the measured
turning radius and each axle's angle with the closed forms of the model document.

**Acceptance Scenarios**:

1. **Given** a 6x2 bus whose third axle is declared steerable with a 15° lock and its script never
   writes the mode, **When** the driver steers half-way to the right, **Then** the tag axle stands at
   7.5° to the left; at full lock, at 15° to the left.
2. **Given** the same bus at full lock at walking speed, **Then** it turns about the line of its
   rigid middle axle, its turning circle is the one the model gives for that line, and the tag axle
   scrubs by the difference between its angle and the one that points at the centre (FR-010).
3. **Given** the same bus with no axle declared steerable apart from the front one, **When** it turns
   at full lock, **Then** its two rear axles both stay straight, the bus turns about their
   load-weighted middle, and both scrub.
4. **Given** an 8x2 front section whose second axle is declared steerable and stands ahead of the
   turning line, **When** the driver steers right, **Then** that axle turns right too, in proportion
   to its own lock, with no script.
5. **Given** a keyed bus, **When** a script reads `Axle_Steering_<n>_L/R` of a rigid axle, **Then** it
   reads 0; of a steerable axle, its actual angle.

---

### User Story 2 - Each vehicle's script locks and steers the axle (Priority: P1)

Real steered tag axles lock straight above a speed, in reverse, on a fault or from a dashboard
switch, and each make does it its own way. The game has no built-in rule for that: the vehicle's own
script decides, by writing the axle's mode (basic, locked, or an angle of its own); the physics
applies it in the next step, the axle moves to and from straight at its centring rate, and the bus's
turning follows what the axle really does.

**Why this priority**: the maintainer wants the deactivation logic to live in each vehicle's script;
without it a steered tag axle would steer at any speed.

**Independent Test**: a test script on V2 locks the tag axle above 30 km/h and unlocks it below; run
a slow S-bend, accelerate past 30 km/h, slow down again; check the axle angle, the turning line and
the yaw rate over time; then cycle the mode through every value in a steady turn.

**Acceptance Scenarios**:

1. **Given** mode 0 (or a script that never declares the variable), **Then** the axle follows the
   basic law of story 1.
2. **Given** mode 1, **Then** the axle returns to straight at its centring rate and stays there.
3. **Given** mode 2 and an angle written by the script, **Then** the axle moves to that angle
   (clamped to its lock) at its centring rate, and the turning line and scrub follow from it.
4. **Given** the axle locking or unlocking during a turn, **When** the turning line moves, **Then**
   it moves without a jump: no step in yaw rate or lateral speed larger than the steering itself
   would cause (SC-003).
5. **Given** any mode, **Then** the script can read whether the axle is locked straight, for a
   dashboard lamp or a sound.

---

### User Story 3 - A rear section with a steered axle follows tighter (Priority: P2)

A mod author declares the last axle of the rear section of a 23 m 8x2 articulated bus (test vehicle
V5) as steerable. In a turn the rear section turns about the line of its rigid driven axle, its last
axle turns against the bend as its joint bends (mode 0 follows the joint), and the rear section cuts the corner less than with
that axle rigid. The same works for the middle and rear sections of a bi-articulated bus, and the
script can take over as in story 2 (for example to steer by the joint's bend).

**Why this priority**: the maintainer wants the declaration generic, for any axle of any section;
articulated buses with a steered last axle are common in Brazil.

**Independent Test**: on V5, a steady turn at walking speed with the fourth axle declared
steerable and again rigid; compare the articulation angle and the rear section's swept path with the
closed forms of the model document.

**Acceptance Scenarios**:

1. **Given** V5 with its fourth axle declared steerable, **When** it turns steadily, **Then** the
   rear section turns about its third axle's line, the fourth axle stands at `−s_f · lock` (`s_f`
   the joint's steering fraction, FR-008), and the articulation angle is the closed form's for that turning line.
2. **Given** the same axle rigid, **Then** the rear section turns about the load-weighted middle of
   its two axles and both scrub (FR-010).
3. **Given** a rear section without any declaration, **Then** it behaves exactly as today (`C-9`).
4. **Given** the rear section's steerable axle locked by its script, **Then** the rear section's
   turning line moves continuously, as in story 2.

---

### Edge Cases

- The declaration sits on the leading section's front axle (steered by the driver already), or is
  given twice in the same axle: that declaration is ignored with a log line naming the file and the
  reason; the bus loads and the rest of it works (Principle IV).
- Only one of the two parameters given on an axle: the declaration is ignored with a log line naming
  the missing one.
- An axle written in the old form without keywords (values only, one per line) cannot carry the
  declaration; it stays rigid or `C-6` as the rest of its section decides.
- A lock angle of 0 or below, or not a number; a centring rate of 0 or below; a missing value: the
  declaration is ignored with a log line. Nothing is corrected silently (Principle III).
- Every axle of a section steerable (no rigid axle at all): there is no geometric turning line, so
  that section turns about its `[rot_pnt_long]` line as in `C-6` (leading section) or `C-9` (a
  section behind a joint), and the log says so once.
- A section behind a joint in mode 0 follows its joint: with the joint straight its axles stand
  straight whatever the steering wheel does, and with the joint bent they keep steering after the
  wheel is centred, until the section is straight again.
- A steerable axle that stands on the turning line in mode 0 stays straight; one that crosses the
  line (the line moves when another axle locks) changes direction at its centring rate.
- The bus runs OMSI Classic (no `[depot_physics]` key and the setting is Classic): the declaration
  is read, ignored, and the log says it applies to Project Depot Physics only; `C-6` runs unchanged.
- An AI copy of a bus with the declaration: the AI keeps its own steering (spec 002 D6: the key
  applies to the player's vehicle only); `Axle_Steering` of AI copies stays as today.
- The script writes an unknown mode or an angle that is not a number: the axle behaves as mode 0,
  and the log says so once per vehicle.
- A steerable axle that does not point at the centre of the turn (the basic law is proportional, not
  geometric) scrubs like a rigid axle off the line, in proportion to its error (FR-010).
- No speed, gear or fault locks the axle unless the vehicle's script does it (mode 1).
- A saved situation restores each steerable axle's angle and locked state.

## Requirements *(mandatory)*

### Functional Requirements

**Declaration**

- **FR-001**: In the `.bus` file of any section of a vehicle, an axle MUST be declarable steerable
  by new Project Depot parameters inside its own `[newachse]`, one parameter per value like
  `achse_long` and the others, which OMSI 2 and its derivatives ignore: the axle's lock angle
  (degrees, each side) and centring rate (degrees per second). Both are required together, so the
  game invents none (Principle III).
- **FR-002**: Before any implementation, the parameter MUST be checked in OMSI 2 (black box, as
  `Q1` of spec 002): a stock bus with the parameter added to an axle loads and drives in OMSI 2
  exactly as without it. **Done 2026-11-21**: the maintainer checked it; OMSI 2 ignores the new parameters.
- **FR-003**: In a section with at least one valid declaration, every axle not declared steerable is
  rigid (always straight), except the leading section's front axle, which the driver steers as
  today. A section without any declaration runs exactly as today in both engines.
- **FR-004**: The declaration MUST take effect only in Project Depot Physics; OMSI Classic ignores it
  (`C-6` unchanged) and logs that once.
- **FR-005**: Invalid declarations MUST be reported in the log and ignored without crashing and
  without being corrected (edge cases above).

**Turning**

- **FR-006**: The turning line MUST follow from the axles that do not steer at that moment (rigid,
  locked in mode 1, or held at an angle by a script in mode 2, which counts as rigid; an axle on its way to or from locked counts by how far it
  has gone):
  on the single such axle, or the static-load-weighted middle of several; with none, the
  `[rot_pnt_long]` line of `C-6`.
- **FR-007**: The front wheels' lock angle MUST stay the one `C-6` gives from `[inv_min_turnradius]`
  and `[rot_pnt_long]` as the file has them; the full-lock curvature then follows from the current
  turning line, so the turning circle changes with what the steerable axles do.
- **FR-008**: In mode 0 (basic), a steerable axle MUST turn in proportion to the driver's steering
  axle and its own lock, on the side its position gives: `δ_i = σ_i · (δ_front / lock_front) ·
  lock_i = σ_i · s · lock_i`, with `s` the driver's steering from −1 to 1 and `σ_i` = −1 behind the
  turning line (a 6x2-4 tag axle turns opposite), +1 ahead of it (a twin-steer second front axle
  turns the same way), 0 on it. In a section behind a joint the steering fraction is the joint's:
  `s_f = clamp(κ_f / κ_max, −1, 1)`, `κ_f` the curvature the joint gives the section (`C-9`) and
  `κ_max` the leading section's current full-lock curvature (FR-007). Its tyres keep Ackermann angles about
  the axle's angle as in `C-6`.
- **FR-009**: The turning line MUST move continuously when an axle locks, unlocks or changes angle:
  no step in yaw rate or lateral speed beyond what the steering itself causes.
- **FR-010**: An axle that does not point at the centre of the turn (rigid off the line, steered by
  the proportional law, held by a script) MUST scrub: it takes force across in the tyres' hold of `D-3` (so the
  bus lets go sooner on a tight bend at speed) and resists the motion (the bus needs more drive to
  keep its speed at full lock).
- **FR-011**: While the tyres slide (`C-7`), each tyre's force MUST use its real angle (rigid axles
  at 0), so a steered tag axle changes how a sliding bus yaws.

**Logic and scripts**

- **FR-012**: The game MUST NOT lock a steerable axle by itself (no speed, gear or fault rule): the
  vehicle's script decides, through the mode (FR-013).
- **FR-013**: Scripts MUST be able to choose, per steerable axle and each frame, through
  `Axle_Steer_Mode_<n>`: 0 basic (FR-008; also when the script never declares the variable),
  1 locked straight, 2 an angle written by the script (clamped to the lock). The axle moves between
  modes at its centring rate. Variable names are Project Depot's own and carry no meaning in OMSI 2.
- **FR-014**: The physics MUST hand the scripts each axle's actual angle through the existing
  `Axle_Steering_<n>_L/R` (0 for rigid axles of a keyed bus) and, per steerable axle, whether it is
  locked straight.

**Scope**

- **FR-015**: The feature MUST work on any section. In a section behind a joint, the curvature comes
  from the joint (`C-9`, Depot's `follower_kappa`); its steerable axles follow their mode (FR-008,
  FR-013) and their tyres steer, its rigid axles' tyres stay square to the body, and its
  turning line follows FR-006 from its own non-steering axles. A section behind a joint without any
  declaration keeps `C-9` unchanged.
- **FR-016**: AI vehicles are not affected (spec 002 D6), neither in physics nor in the `Axle_Steering`
  they hand their scripts: they ignore the steerable-axle parameters (clarify Q7).
- **FR-017**: The rules MUST be written in `docs/physics/depot-model.md` as new `D-*` entries with
  their equations, inputs, outputs, module and status, and `common-model.md` `C-6` MUST point to
  them.
- **FR-018**: The new script variables and the `.bus` key MUST be documented in `docs/FORMATS.md`.
- **FR-019**: A change in Project Depot Physics for keyed buses carries the `physics-delta` label;
  the physics traces of every bus without the key MUST stay identical.

### Key Entities

- **Steerable axle declaration**: lock angle, centring rate; belongs to one
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
- **SC-003**: Locking or unlocking the tag axle by script in a steady turn at 25 km/h never makes the yaw rate
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
- **SC-008**: The maintainer drives a keyed 6x2 bus in the game and confirms the tighter turn, a script
  that locks the axle at speed, and the script angle mode by eye.

## Assumptions

- There is no OMSI 2 behaviour to match: OMSI 2 has no rigid or steerable axle setting, and every
  axle follows `C-6`. The rules are Project Depot Physics' own and are described as such (Principle
  II); only the key's harmlessness in OMSI 2 needs an in-game check (FR-002).
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
