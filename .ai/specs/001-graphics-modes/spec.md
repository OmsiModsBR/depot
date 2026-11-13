# Feature Specification: Three graphics modes, each with its own code and its own options

**Feature Branch**: one per delivery phase (see [plan.md](plan.md), to be regenerated from this spec)

**Created**: 2026-11-13

**Status**: Draft

**Input**: User description: issue #703, the maintainer's direction of 2026-11-08 and the request of
2026-11-13: "Revise spec 001 (simplification of the graphics modes). The specification must cover a
refactor that makes the difference between the Vanilla+ and Enhanced modes clear, besides removing
Enhanced+ and Vanilla, with the DX9 mode taking Vanilla's place. Structure this refactor together
with the simplification, so that there are directories for each graphics mode and for their shared
interfaces." Follow-up of the same day, after the directory structure was approved: "Include in the
scope the clean-up of the configuration options, in the launcher and in the in-game menu, because
there are settings that do not apply to one or more modes." And: "In the launcher's graphics
settings page, the 'Graphics API' section is to be reworked to hold the choice of the graphics mode.
The choice of the graphics API, available only in Vanilla+ and Enhanced, goes where 'Graphics' is
today, inside the 'Image' section." And: "Graphics options whose change only shows after the game
is restarted must not be displayed in the in-game menu."

## Context

Depot offers five graphics choices today: Vanilla, Vanilla+, Enhanced, Enhanced+ and "DirectX9
(D3D9Ex)". Two of them draw the same thing by different means (Vanilla and DirectX9), one is
discontinued (Enhanced+), and the code of the remaining two (Vanilla+ and Enhanced) is interleaved,
so a change meant for one can alter the other. The options the player sees do not follow the mode
either: some options are offered in a mode in which they change nothing.

After this feature there are **three modes**, each with one role:

| Mode | Role |
|---|---|
| **Vanilla** | The OMSI 2 picture with nothing added. Drawn by what is today the "DirectX9 (D3D9Ex)" renderer |
| **Vanilla+** | The OMSI 2 picture with conservative improvements, on modern graphics APIs. Its picture does not change with this feature |
| **Enhanced** | Where the advanced graphics work goes. No ray tracing and no path tracing |

## Clarifications

### Session 2026-11-13

- Q: Does making graphics options take effect at once (without a restart) belong to this feature? →
  A: Yes, for a small set. The plan lists a few options that are cheap to apply in the running
  session; every other option that needs a restart stays in the launcher only.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Three modes to choose from, and no player left in a mode that is gone (Priority: P1)

A player opens the graphics settings and finds three modes: Vanilla, Vanilla+ and Enhanced. Enhanced+
and the separate "DirectX9 (D3D9Ex)" entry in the list of graphics APIs are gone. A player who had
any of the five old choices saved starts the game in the mode that replaces it, without editing a
file and without an error.

**Why this priority**: it is the simplification itself. Everything else in this feature organises
what is left after it.

**Independent Test**: start the game once with each old saved choice (Vanilla, Vanilla+, Enhanced,
Enhanced+, and any of them with the DirectX9 API) and check which mode opens and what the settings
show.

**Acceptance Scenarios**:

1. **Given** a fresh installation, **When** the player opens the graphics settings in the
   launcher, **Then** the mode list has exactly Vanilla, Vanilla+ and Enhanced.
2. **Given** saved settings with Enhanced+, **When** the game starts, **Then** it runs in Enhanced
   and the log says the saved mode was replaced.
3. **Given** saved settings with the old Vanilla on any modern API, **When** the game starts,
   **Then** it runs in Vanilla, drawn by the renderer that was "DirectX9 (D3D9Ex)".
4. **Given** saved settings with the DirectX9 API and any mode, **When** the game starts, **Then**
   it runs in Vanilla.
5. **Given** saved settings with an unknown or misspelt mode, **When** the game starts, **Then** it
   runs in Vanilla+ (the default) and the log names the value it could not read.
6. **Given** the player chooses Vanilla, **When** they look at the graphics API option, **Then** it
   is not offered as a choice: Vanilla has one renderer.
7. **Given** a computer that cannot open Vanilla's renderer, **When** the player starts the game in
   Vanilla, **Then** the game shows a clear message telling them to choose Vanilla+ and does not
   switch mode on its own.

---

### User Story 2 - Only the options that do something in the chosen mode (Priority: P2)

A player chooses a mode and sees, in the launcher and in the in-game menu, only the options that
change the picture or the performance in that mode. An option that does nothing there is not
offered; an option whose value the mode fixes is shown as a fixed line with the reason. The in-game
menu goes one step further: it offers only the graphics options whose change shows at once, so
nothing the player moves there waits for a restart. Both places agree, because they read the same
description of what applies to each mode and of when a change takes effect.

**Why this priority**: today a player can move a slider that does nothing in their mode (for example
the night brightness and LED glow options, which act only in Enhanced, are offered in Vanilla+), and
cannot tell which mode an option belongs to. It is the part of the simplification the player touches
most often.

**Independent Test**: for each of the three modes, open the launcher's settings and the in-game
menu, list the graphics options offered, and compare both lists with the applicability table this
feature produces; then change every offered option and confirm each has an observable effect in that
mode, and that every option offered in the in-game menu shows its effect without a restart.

**Acceptance Scenarios**:

1. **Given** the mode is Vanilla+, **When** the player opens the graphics options, **Then** no
   option that acts only in Enhanced or only in Vanilla is offered.
2. **Given** the mode is Enhanced, **When** the player opens the graphics options, **Then** no
   option that acts only in Vanilla+ or only in Vanilla is offered.
3. **Given** the mode is Vanilla, **When** the player opens the graphics options, **Then** the
   improvements the mode does not have are not offered, and Vanilla's own options are.
4. **Given** any mode, **When** the player compares the launcher with the in-game menu, **Then**
   the in-game menu offers exactly the launcher's graphics options for that mode that take effect
   without a restart, and no other.
5. **Given** the player changes the mode in the launcher, **When** the list redraws, **Then** it
   shows the new mode's options at once, before the game is started.
6. **Given** the player set an Enhanced-only option, switched to Vanilla+ and later back to
   Enhanced, **When** they open the options, **Then** the value they had set is still there.
7. **Given** the mode is Vanilla, **When** the player applies a quality preset, **Then** the preset
   changes only options that apply to Vanilla, or presets are not offered there.
8. **Given** an option is fixed by the mode, **When** the player reads its line, **Then** it shows
   the fixed value and why, and it cannot be changed.
9. **Given** the launcher's graphics settings page, **When** the player opens it, **Then** the
   highlighted section at the top holds the choice of the graphics mode, with one line per mode
   saying what it is, and says that the mode decides which other graphics options are offered.
10. **Given** the mode is Vanilla+ or Enhanced, **When** the player looks at the "Image" section,
    **Then** the graphics API choice is in the place the mode choice had, with Automatic, Vulkan,
    DirectX 12 and OpenGL.
11. **Given** the mode is Vanilla, **When** the player looks at the "Image" section, **Then** the
    graphics API choice is not there.
12. **Given** the game is running, **When** the player opens the in-game graphics options, **Then**
    the graphics mode, the graphics API and every other option that needs a restart are not
    offered.
13. **Given** the game is running, **When** the player changes any graphics option the in-game menu
    offers, **Then** the picture changes in the same session.
14. **Given** an option that needs a restart, **When** the player looks for it, **Then** it is in
    the launcher, with the line saying the change takes effect on the next start.

---

### User Story 3 - Each mode's code in its own place (Priority: P3)

A contributor who wants to improve Enhanced opens the Enhanced directory and finds what only
Enhanced draws: its shading, its sky, its post-processing, its exposure. They change it knowing that
Vanilla+ cannot be affected, because Vanilla+ lives in another directory and the two never refer to
each other. What both modes share sits in the shared core, behind interfaces that say what a mode
must provide.

**Why this priority**: it is what keeps the three modes apart after the removals, and what lets the
advanced work on Enhanced go on without risk to the conservative mode. The player does not see it,
but every later graphics change depends on it.

**Independent Test**: delete (in a scratch copy) one modern-API mode's directory and build the other mode:
it builds and draws without it. Run the automated dependency check: no reference from one mode to
the other, and none from the shared core to a mode.

**Acceptance Scenarios**:

1. **Given** the finished structure, **When** a contributor looks for the code that only one mode
   runs, **Then** it is under that mode's directory and nowhere else.
2. **Given** a change that touches only files under the Enhanced directory, **When** the reference
   pictures of Vanilla+ and Vanilla are compared before and after, **Then** they are identical.
3. **Given** a change that makes one mode refer to the other, or the shared core refer to a mode,
   **When** the continuous-integration checks run, **Then** they fail and name the reference.
4. **Given** the game started in Vanilla+, **When** the renderer opens, **Then** nothing that only
   Enhanced uses is prepared, and the other way round.
5. **Given** any refactor step of this feature, **When** it is merged, **Then** the reference
   pictures, the state traces and the frame time of the modes that remain are unchanged.

---

### User Story 4 - Enhanced keeps what Enhanced+ did better without tracing rays (Priority: P4)

A player who used Enhanced+ moves to Enhanced and does not lose the one visible thing Enhanced+ got
right and Enhanced got wrong: sunlight coming through a vehicle's windows is dimmed by the glass
instead of passing in full.

**Why this priority**: it is the only behaviour of the removed mode worth carrying over, and it is a
visible change with its own cost, so it is separate from the removal.

**Independent Test**: the same camera inside a bus in direct sun, before and after: the cab and the
floor lit through the windows are darker than in the open, and the frame time is measured on both.

**Acceptance Scenarios**:

1. **Given** Enhanced and a bus in direct sunlight, **When** the player looks at the floor under a
   window, **Then** it receives less sunlight than the ground outside and more than a spot shaded by
   the body.
2. **Given** the same scene, **When** the frame time is measured before and after, **Then** the cost
   is reported with the change and justified.

---

### Edge Cases

- A saved settings file from before the mode key existed (only the old "enhanced on/off" key): it is
  read as Enhanced or Vanilla+ as today.
- Vanilla chosen together with VR or triple screen, which Vanilla's renderer does not have: the
  launcher does not offer VR and triple screen in Vanilla; a settings file that asks for both by
  hand opens Vanilla without them and logs why.
- The mode forced from outside the settings (command line or environment): the options shown follow
  the mode the game is actually running.
- The player wants to change the mode while the game is running: the in-game menu does not offer
  it (it needs a restart); the mode is changed in the launcher.
- An option that takes effect at once in one mode and only after a restart in another: the in-game
  menu offers it only in the mode in which it takes effect at once.
- A graphics section of the in-game menu left with no option in the current mode: the section is
  not shown.
- A value saved for an option that the current mode fixes or hides: the saved value is kept in the
  file and not applied; it comes back when the mode changes.
- The launcher's vehicle preview while the game is set to Vanilla: it is drawn as Vanilla+ (the
  preview has one renderer); this is stated in the user guide.
- A command-line argument or environment variable of a removed mode (for example the one that asked
  for Enhanced+): it is ignored with a log line, never an error.
- A graphics option that turns out to act in no mode at all: it is removed with its code.

## Requirements *(mandatory)*

### Functional Requirements

**The three modes**

- **FR-001**: The game MUST offer exactly three graphics modes: Vanilla, Vanilla+ and Enhanced.
- **FR-002**: Vanilla MUST be drawn by the renderer that is today "DirectX9 (D3D9Ex)". The
  implementation of Vanilla on modern graphics APIs MUST be removed with all code only it uses.
- **FR-003**: Enhanced+ MUST be removed with all its code: the ray-traced shadows, occlusion and
  reflections, their options, command-line arguments and environment variables. Nothing of it stays
  behind a build switch or a hidden option.
- **FR-004**: Vanilla+ MUST draw the same picture after this feature as before it.
- **FR-005**: Enhanced MUST draw the same picture after this feature as before it, except for the
  sunlight through glass of User Story 4, which is delivered as its own change with before/after
  pictures and measurements.
- **FR-006**: The graphics API choice MUST apply to Vanilla+ and Enhanced only, and MUST no longer
  list DirectX9.
- **FR-007**: Before Vanilla changes renderer, the differences between today's two Vanilla
  implementations MUST be listed, and for each one the maintainer decides which side is kept. A
  difference in which the removed implementation is the correct one MUST be fixed in the remaining
  renderer first.
- **FR-008**: Before Vanilla changes renderer, the remaining renderer MUST be measured against the
  removed one on the same scene, and MUST NOT be slower.
- **FR-009**: The mode MUST be resolved once at start-up into a single value that the game, the
  launcher and the in-game menu all read. No second copy of the rule that reads a saved mode may
  exist.

**Saved settings**

- **FR-010**: Saved settings naming a removed mode or the DirectX9 API MUST open the replacing mode
  as in User Story 1, with one log line saying what was replaced.
- **FR-011**: An unknown mode value MUST open Vanilla+ and be reported in the log; it MUST NOT stop
  the game.
- **FR-012**: The settings file MUST NOT be rewritten just because it was read: the migrated value
  is written only when the player saves.

**Options per mode**

- **FR-013**: Every graphics option MUST be audited and classified, for each of the three modes, as:
  applies (offered), fixed by the mode (shown as a fixed line with its value and the reason), or
  does not apply (not offered). For each mode in which it applies, the audit also records when a
  change takes effect: at once, or on the next start. The result is one applicability table, kept
  with this feature's documents and in the user guide.
- **FR-014**: The launcher and the in-game menu MUST decide what to offer from that one table. No
  screen may carry its own rule for a mode.
- **FR-015**: An option that does not apply to the current mode MUST NOT be offered in either place.
- **FR-016**: A value saved for an option the current mode hides or fixes MUST be kept and MUST come
  back when a mode in which it applies is chosen.
- **FR-017**: Quality presets MUST change only options that apply to the current mode; where a mode
  has no option a preset would change, presets are not offered in it.
- **FR-018**: The options that exist only for Vanilla (today named after DirectX9) MUST be offered
  in Vanilla only, and described by what they do, not by the API.
- **FR-019**: An option that the audit shows to act in none of the three modes MUST be removed
  together with the code that reads it.
- **FR-020a**: In the launcher's graphics settings page, the highlighted section at the top (today
  "Graphics API") MUST hold the choice of the graphics mode, named after it, with a short
  description of each of the three modes and the statement that the mode decides what the other
  graphics options can do. Its text MUST no longer say that the API limits the options.
- **FR-020b**: In the same page, the choice of the graphics API MUST sit inside the "Image" section,
  in the place the mode choice has today, and MUST be offered only in Vanilla+ and Enhanced.
- **FR-020c**: The in-game menu MUST NOT offer a graphics option whose change takes effect only
  after the game is restarted. This includes the graphics mode and the graphics API. Such options
  are offered in the launcher only, where their line says the change takes effect on the next
  start.
- **FR-020d**: Every graphics option the in-game menu offers MUST show its effect in the running
  session.
- **FR-020e**: A small set of graphics options that need a restart today MUST be made to take
  effect in the running session, so that the in-game menu keeps them. The plan names the set,
  chosen among the options that are cheap and safe to apply at once (switching an effect on or off
  is a candidate; anything that rebuilds the renderer's resources is not). Each one is delivered as
  its own change, with a test that the picture after the in-session change equals the picture of a
  fresh start with the same value, and with no frame-time cost while the option is not being
  changed.
- **FR-020**: An automated test MUST fail when a graphics option exists without a row in the
  applicability table, when the in-game menu offers an option the table marks as needing a restart
  or as not applying to the running mode, or when it leaves out one the table marks as applying at
  once.

**Code structure**

- **FR-021**: The code that only one mode runs MUST live in that mode's directory, one directory
  per mode, following the structure in "Code structure" below.
- **FR-022**: What Vanilla+ and Enhanced share MUST live in a shared core, and the core MUST reach a
  mode only through a shared interface that states what a mode provides to a frame.
- **FR-023**: What the shared core hands to Vanilla's renderer (the plan of a frame: what is
  visible, in which order, with which lights) MUST be a shared interface of its own, with a test
  that the same plan always gives Vanilla the same draws.
- **FR-024**: Vanilla+ and Enhanced MUST NOT refer to each other, and the shared core MUST NOT refer
  to a mode. An automated check in continuous integration MUST fail on any such reference.
- **FR-025**: A source file that holds code of two modes MUST be split so that each part lives with
  its mode, and the common part lives in the core.
- **FR-026**: Starting the game in a mode MUST prepare only what that mode and the shared core use.
- **FR-027**: Each directory of this structure MUST follow the project's package layout: domain
  objects, interfaces (what others call, what it asks of the outside) and implementation, kept
  apart.
- **FR-028**: Tests of the affected crates MUST live outside the source directories.
- **FR-029**: The removals (FR-002, FR-003) MUST come before the restructuring, so that only code
  that stays is moved.
- **FR-030**: A step that only moves or reorganises code MUST NOT change any picture, any state
  trace, or the frame time beyond measurement noise, and MUST leave the code smaller or less coupled
  than before. Long routines that mix both modes' decisions are split as part of the move, not kept
  whole in a new place.

**Documents and public text**

- **FR-031**: The user guide, the environment-variable reference, the architecture documents of the
  rendering crates and the changelog MUST describe the three modes, the applicability table and the
  new structure, and MUST no longer mention the removed modes except as migration notes.
- **FR-032**: No public text may say that Vanilla matches OMSI 2 in a point the maintainer has not
  checked in the game; unchecked points are marked "to validate".

### Code structure

Approved by the maintainer on 2026-11-13. Names of files inside each directory are decided in the
plan; the directories and what each holds are part of this specification.

```text
crates/
├── omsi-cfg/                       the mode as one shared type (Vanilla, Vanilla+, Enhanced)
│
├── omsi-render/src/                shared core + the two modes on modern APIs
│   ├── api/                        the public entry, as today
│   ├── interface/                  SHARED INTERFACES
│   │   ├── mode                    what a mode provides to a frame (Vanilla+ and Enhanced implement it)
│   │   └── frame plan              what the core hands to Vanilla's renderer
│   ├── modes/
│   │   ├── vanilla_plus/           VANILLA+: its pipelines, its frame decisions, its sky, its shaders
│   │   └── enhanced/               ENHANCED: its pipelines, its frame decisions, post-processing,
│   │                               exposure, atmosphere, lamp light in fog, high-range targets, its shaders
│   ├── gpu/ assets/ scene/ lighting/ sky/ targets/ passes/ effects/ shaders/ output/
│   │                               SHARED CORE: only what both modes use
│   └── renderer/                   runs the frame; knows the mode only through the interface
│
├── omsi-render-vanilla/            VANILLA (today `omsi-render-d3d9`, renamed after the mode)
│   └── src/
│       ├── domain/                 the scene and interface frames it takes, device capabilities
│       ├── interface/              what the game calls
│       └── implementation/         the Direct3D 9Ex drawing, and the recording stand-in used by tests
│
└── omsi-app/src/output/            the mode chooses the renderer; the frame plan is translated for Vanilla
```

Rules of the structure:

1. `modes/vanilla_plus` and `modes/enhanced` never import from each other.
2. The shared core never imports from `modes/`.
3. A mode imports from the core and from `interface/` only.
4. Vanilla stays a separate crate: it does not depend on the modern-API renderer and runs on
   Windows only.
5. The settings keys of Vanilla's own options keep their saved names, so no player's file breaks.

### Key Entities

- **Graphics mode**: one of Vanilla, Vanilla+, Enhanced. Decides the renderer, what is prepared at
  start-up and which options are offered. Resolved once per start.
- **Applicability table**: for every graphics option and every mode, whether the option applies, is
  fixed (with value and reason) or does not apply, and whether a change takes effect at once or
  on the next start. The single source for the launcher and the in-game menu.
- **Mode interface**: the contract a modern-API mode fulfils for the shared core (what it prepares,
  what it decides for a frame, what it draws).
- **Frame plan**: the description of a frame the shared core produces once (visible objects, draw
  order, lights), consumed by Vanilla's renderer.
- **Saved settings**: the player's file. Holds the mode and every option value, including values of
  options the current mode does not offer.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The mode list shows exactly three entries in the launcher.
- **SC-002**: 100 % of the old saved choices (five modes, with and without the DirectX9 API, and
  the old spellings) open a valid mode on first start with no player action and no error.
- **SC-003**: In each mode, 100 % of the graphics options offered have an observable effect in that
  mode. The in-game menu offers exactly the subset that takes effect without a restart: zero
  options there need one.
- **SC-004**: Zero references between the two modern-API modes and zero from the shared core to a
  mode, checked automatically on every pull request.
- **SC-005**: The reference pictures of Vanilla+ and Enhanced are identical before and after every
  removal and restructuring step (the sunlight-through-glass change excepted, which ships with its
  own before/after pictures).
- **SC-006**: No mode is slower after the feature than before it, measured on the same machine and
  scene; Vanilla on its remaining renderer is not slower than the removed implementation.
- **SC-007**: Opening the renderer in Vanilla+ takes less time than before, because nothing of
  Enhanced is prepared.
- **SC-008**: A contributor can name, from the directory listing alone, where the code of each mode
  and the shared interfaces live; no source file contains code of two modes.
- **SC-009**: The rendering code is smaller after the feature than before it (the removed modes
  outweigh what the restructuring adds).
- **SC-010**: No test of the affected crates remains inside a source directory.

## Assumptions

- Changing the mode needs a restart, as today.
- Windows x64 is the only supported platform; code kept only for other platforms' graphics quirks is
  removed where this feature touches it.
- Vanilla+ stays the default mode.
- The findings of the 2026-11-07 survey and of [research.md](research.md) (differences between the
  two Vanilla implementations, and between Enhanced and Enhanced+) are the starting point; they are
  re-checked against the current code when the plan is regenerated.
- Known starting point for the options audit (FR-013), to be completed in the plan: night
  brightness and LED glow act only in Enhanced but are offered in Vanilla+ today; shadows, ambient
  occlusion and detail textures are improvements Vanilla does not have; the "DirectX9" effort-saving
  options act only in Vanilla; the anti-aliasing after the picture differs per mode.
- Decisions still open from the earlier plan, taken here as their recommended answers until the
  maintainer says otherwise (`/speckit-clarify`):
  - Vanilla with VR or triple screen: not offered in the launcher (no silent switch to another
    mode).
  - A computer that cannot open Vanilla's renderer: a clear error pointing to Vanilla+ (no silent
    switch).
  - Enhanced+'s colour grade, vignette and sharpening are an aesthetic choice and are not carried
    over to Enhanced; only the sunlight through glass is.
  - Which side of each difference between the two Vanilla implementations matches OMSI 2 is decided
    by the maintainer in the game (FR-007); the feature does not guess.
- Making every graphics option take effect without a restart is out of scope: only the small set of
  FR-020e is.
- New advanced features for Enhanced (for example screen-space reflections or a new ambient
  occlusion) are out of scope: this feature prepares the place for them.
- The existing [plan.md](plan.md), [research.md](research.md), [data-model.md](data-model.md),
  [quickstart.md](quickstart.md) and contracts predate this spec. They stay as reference and are
  regenerated with `/speckit-plan`; where they differ from this spec, the spec wins.
