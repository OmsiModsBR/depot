# Research: Two physics modes (OMSI Classic and Depot)

Phase 0 of the [plan](plan.md). Code read at `0f4b334` (2026-11-08). "OMSI 2" on this page is what
the original game is expected to do and **still has to be checked in the game** wherever a row says
"to validate" (Principle II, #49).

## R-V. Which of today's behaviours are overrides (Depot mode) and which stay shared

Rule from #677: the Depot mode holds only changes that *override* an OMSI 2 behaviour; additions stay
on in both modes. Each candidate below is a behaviour of today's player physics that may differ from
OMSI 2. A candidate moves to Depot only when its check shows that OMSI 2 behaves differently
(decision D2).

| # | Behaviour today | Where | Check in OMSI 2 (quickstart Q4) | Proposal |
|---|---|---|---|---|
| V1 | Weight shared about the model origin (#675); before #675 about `[cog]` | `physics/implementation/suspension.rs` (the `cog x F` torque term), `spec.rs::wheel_rest_loads` | Done by #675's own reasoning; in-game check of the Mi5 still pending (#677 item d, Q3) | **Classic = origin, Depot = `[cog]`** (the first Depot item) |
| V2 | Fixed 1/240 s substeps, the same motion at any frame rate | `domain/params.rs` `SUBSTEP_DT`, `body.rs` | Does the same manoeuvre (full lock at 30 km/h, brake from 50) come out differently at a 30 and a 60 fps cap? | **Shared**: it is how the equations are integrated, not a rule a mod is tuned to. The Classic document states the integration and notes any frame-rate effect observed in OMSI 2 |
| V3 | Air drag ½ ρ c_w A v², with A from the mass | `tyre.rs:108`, `params.rs` (`*physics*`, `*tuned*`) | Coast down a long 6-8 % grade without brakes: does OMSI 2's speed level off, and where? | Depot only if OMSI 2 has no drag; otherwise Classic keeps it with the value the check gives |
| V4 | Tyre as a flattened envelope over the ground (13 samples), climbing kerbs up to 0.8 r | `ground.rs`, `params.rs` `ENVELOPE`, `CLIMB` | Drive over a 15 cm kerb at 10 km/h at 45° and square on: does OMSI 2 climb it, stop, or bounce? | Depot only if OMSI 2's wheel cannot climb it |
| V5 | Wheels pushed out of wall faces (`WALL_K`, `WALL_C`) | `suspension.rs`, `collide.rs` | Drive a wheel slowly into a wall face | Shared if OMSI 2 also stops the wheel at the face |
| V6 | The body set back on its wheels past 72.5° of lean | `body.rs` `TIP_OVER` | — | Shared: a safety net against bad ground, not a model rule |
| V7 | AI road body: `AiBody` (pure pursuit, planner speed, sprung body, `[cog]` sag) | `ai/`, `omsi-app` `traffic.rs` | — (decided by the request) | **Depot AI**; Classic gets a new body (R-A) |
| V8 | AI may steer beyond the model's lock to keep to a junction's path (#249) | `ai/road.rs` | — | Part of the Depot AI. The Classic AI keeps to its path by construction (KA-1), so the question does not arise |

**Decision** (D2, maintainer, 2026-11-08): V1 and V7 go into the engines now; for now the Depot mode
overrides nothing else. V2-V6 stay shared. The V3 and V4 checks are kept here for a later decision;
until one is made the Classic document marks them "to validate".

**Rationale**: moving a behaviour to Depot on a guess would make Classic *less* faithful if the
guess is wrong; Principle II asks for the check first.

**Alternatives considered**: put every `*tuned*` constant of `params.rs` into Depot and give Classic
"OMSI values": rejected, most of them have no OMSI counterpart (they are numerical floors, probe
geometry, solver passes), and inventing Classic values would break Principle III's "only OMSI 2's own
defaults".

## R-A. The Classic AI body: what OMSI 2's AI is expected to do

The request asks for a new AI implementation brought in line with OMSI 2's. Today's `AiBody` is a
Depot design (it was built to fix lane-placement artefacts, see the `ai/mod.rs` header). What OMSI 2's
AI does is written below as **observable behaviour to validate**, one check each (quickstart Q2); the
Classic AI implements only what the checks confirm. The existing `ai/road.rs` comment and these
expectations disagree on KA-4, which is why the check comes before the code.

| # | Expected behaviour (to validate) | How it is checked in OMSI 2 | Phase |
|---|---|---|---|
| KA-1 | The leading section stays on its lane path: the point at `[rot_pnt_long]` lies on the path and the vehicle's heading is the path's direction there, so the rear axle tracks the path through bends without running wide | Film an AI bus through a tight junction turn from above; compare the rear wheels' track with the lane | 8 |
| KA-2 | The AI vehicle's speed comes from its own drive and brakes: the AI driver sets throttle and brake towards the target speed, and the scripts' `M_Wheel` and brake forces accelerate the mass | Halve the engine torque of an AI-only copy of a bus (its scripts' constant file) and time its pull-away from a stop | 9 |
| KA-3 | One grip value per vehicle (the road surface under it) limits the AI's acceleration and braking | An AI bus pulling away and stopping on a wet and on a dry road | 9 |
| KA-4 | On its path the AI body has no sprung pitch or roll of its own: it stands on the ground under its wheels with its static sag | Watch an AI bus braking hard and in a bend: does the body pitch/roll? | 10 |
| KA-5 | The rear sections of an articulated AI vehicle follow the path behind their joint | Film an articulated AI bus through a junction turn | 10 |
| KA-6 | A hit by the player's vehicle releases the AI vehicle to the free rigid body, without a driver, until it is cleared from the scene | Validated: the maintainer's in-game observation (2026-11-08): the AI vehicle becomes a free rigid body until it is cleared from the scene | 10, decision D3 |
| KA-7 | The AI vehicle's weight is shared about the model origin, as the player's in Classic | An AI copy of the Mi5 standing at a stop: is it level, as the player's? | 10 |

**Decision**: the Classic AI is a new body in `ai/classic/`, built in three steps (path, drive,
pose), each from its confirmed rows. The traffic planner, the AI driver's decisions and the rail and
air followers stay shared.

**Rationale**: the request makes the current AI Depot's; a Classic AI that copied it would leave the
default mode without an OMSI-faithful AI. Building from observed rows keeps Principle II.

**Alternatives considered**: (a) the Classic AI runs the player's full rigid body: the most faithful
if KA-4 shows sprung motion, but costs about five times `AiBody` per vehicle (study of 2026-10-10:
rigid step about 10 µs plus 336 ground probes against 1.1 µs and 4 probes); only considered if Q2
contradicts KA-4. (b) keep `AiBody` in both modes: rejected by the request.

## R-K. The `.bus` key

**Decision** (D1, maintainer, 2026-11-08): `[depot_physics]`, a section with no value lines; its
presence makes the vehicle use the Depot engine ( [contracts/bus-key.md](contracts/bus-key.md)).

**Rationale**: Depot's own parser already collects keys it does not know into
`Vehicle::unknown_keywords` (`omsi-vehicle/src/vehicle.rs:484`) and goes on, which is the behaviour
the key needs from other readers. A key with no value lines leaves nothing a reader that does not know
it could take for another section's values. Only "force Depot" exists (#677: no "force Classic"), so
a value adds nothing.

**To validate before phase 3** (#677 item a, quickstart Q1): OMSI 2 loads a stock bus with
`[depot_physics]` added, with no line in `logfile.txt` beyond what it writes without it, and drives
the same. The same check on openOMSI and on any fork the maintainers name.

**Alternatives considered**: a `.bus` value under an existing section: rejected, an older reader would
take it as that section's data. A separate file next to the `.bus`: rejected, mods are shared as
folders and a loose file is easily lost.

## R-D. How the code switches between engines

**Decision**: the vehicle's `PhysicsMode` is resolved once at spawn and handed to its bodies. The
player's `RigidBody` holds an `Engine` enum and calls the engine's module only at the stages that
differ (today only the load share); the AI holds `AiMotion { Classic(ClassicAiBody), Depot(AiBody) }`
for road vehicles. Rail and air followers and the kinematic model take no engine.

**Rationale**: Principle V allows a dispatch point where two real implementations exist; an enum is
the cheapest form for a closed set and keeps the shared stages free of `if depot`.

**Alternatives considered**: a global mode read inside the stages: rejected, a session mixes
vehicles of both engines (#677 decision 4). Two complete physics packages: rejected, they would copy
every shared stage (Principle X).

## R-T. Validating the implementation against the model

**Decision**: every equation of the model documents has an id (`C-n`, `K-n`, `KA-n`, `D-n`, `DA-n`,
[contracts/model-docs.md](contracts/model-docs.md)). A conformance test names the ids it checks and
compares what the simulation does with what the equation gives in closed form, on a case where the
closed form holds (standing still, a constant drive below the grip, a steady low-speed turn). The
matrix of the five vehicles against the equations is in
[contracts/test-vehicles.md](contracts/test-vehicles.md).

Examples of closed forms the tests use:

- **Static share** (K-1/D-1): with `n` axles of spring rate `k_i` (both sides) at `y_i`, the body
  sinks by `z` and pitches by `θ` so that `F_i = k_i (z - θ y_i)`, `Σ F_i = m g` and
  `Σ F_i (y_i - y_ref) = 0`, where `y_ref = 0` in Classic and `y_ref = cog_y` in Depot. Each axle's
  `Axle_Suspension` must match `F_i / k_i` within 1 mm.
- **Drive** (C-4, C-8): on the flat, below the grip, `a = (M_Wheel / r - F_roll - F_air(v)) / m`
  within 1 % over the first seconds.
- **Turning** (C-6): at walking speed on full lock, the radius of the `[rot_pnt_long]` point is
  `1 / inv_min_turnradius` within 1 %, and each steered axle stands at `atan(arm_i · κ)`, `arm_i`
  measured from that line (negative for a steered tag axle).
- **Articulation** (C-9): in a steady low-speed turn the rear section's angle is the trailer's
  kinematic angle, `γ = atan(a / R) + asin(b / sqrt(R² + a²))` with `R` the radius of the front
  section's rotation point, `a` the joint behind it and `b` the rear section's rotation line behind
  the joint (each further section in turn).
- **Classic AI** (KA-1): the lateral distance of the `[rot_pnt_long]` point from the path stays under
  1 cm through the test bends.

Tolerances come from the numerical method (1/240 s substeps) and are written next to each test.

**Rationale**: traces prove that a refactor changes nothing; they cannot prove that the behaviour is
the one the document describes. Closed forms can.

**Alternatives considered**: golden traces only: rejected (they encode today's behaviour, right or
wrong). Comparing with OMSI 2 recordings in CI: not possible (no OMSI 2 in CI, Principle VIII); the
in-game checks feed the document instead.

## R-M. Shape of the model documents

**Decision**: `docs/physics/common-model.md` holds the equations both engines use;
`classic-model.md` and `depot-model.md` each list *every* equation their engine runs, by id, and write
out in full only their own (K-*, KA-*, D-*, DA-*). Each Classic rule carries "validated (how)" or "to
validate (check)".

**Rationale**: the request asks for one document per engine with all the equations and concepts it
uses; writing the shared ones once keeps the two from drifting apart, and the per-engine index still
lets a reader see the whole engine in one place.

**Alternatives considered**: two fully self-contained documents: rejected, the strut, tyre, steering
and body equations would be written twice and fixed in one copy only.

## R-N. Telling the player and the modder

**Decision**: the launcher's "Physics" list says in one line that mods tuned for OMSI 2 may stand or
handle differently in Depot physics; the log names each vehicle's engine at spawn and why ("setting"
or "`[depot_physics]` in <file>"). Nothing is corrected (Principle III).

**Alternatives considered**: a warning when a Classic-tuned bus stands unlevel in Depot: rejected,
"tuned for Classic" cannot be told from the file, and the warning would be a guess.

## R-S. Saved situations

**Decision**: the engine is not saved with a situation; it is resolved again at load from the setting
and the key, like any other setting. The log line of R-N shows which one applied.

## R-P. Cost

**Decision**: phases 3-7 must leave the frame time unchanged (Principle VII, no exception for
refactors): the AI and player steps measured before/after in the existing physics benchmark scenes.
The Classic AI phases (8-10) measure the AI step per vehicle against `AiBody` (1.1 µs and 4 ground
probes per vehicle and frame in the 2026-10-10 study) on 40 AI vehicles; Classic must not be slower.
If KA-4 forces sprung motion, that is a new feature cost and needs the justification of Principle VII.
