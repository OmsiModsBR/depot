# Research: Steerable axles in Project Depot Physics

Decisions taken while planning spec 004. Each gives the decision, why, and what was rejected. Code
read at `e7d6830` (2026-11-21). Equations are written in the body frame: `y` forward along the body
(m, from the model origin), `κ` the curvature (1/m, positive to the right).

## R-1. How the game steers today

- `train.rs` turns the driver's steering `s ∈ [−1, 1]` into `κ = s · [inv_min_turnradius]` for the
  leading section, and gives each section behind a joint the joint's curvature
  (`Joint::follower_kappa`, Depot: `sin α / arm`).
- `RigidBody::steer_to` points **every** wheel of the leading section at the centre of the turn on
  the `[rot_pnt_long]` line (`tyre = atan((y − rot) · κ / (1 − x · κ))`), and keeps every tyre of a
  section behind square to the body (`steered = false`).
- `RigidBody::axle_steer` hands the scripts `atan((y − rot) · κ)` per axle (`Axle_Steering`),
  including for sections behind (their axles show the joint's bend).
- `lateral_start` holds the leading section's tyres at the tyre with unlimited reach while holding
  (the body turns about the line at once, `C-7`); a section behind is held at its turning line,
  level with each tyre, with the road's reach (`C-9`). `D-3` checks the hold axle by axle.
- Axle numbers seen by the scripts run over the whole train (`first_axle + i` in
  `vehicle/coupled/articulation.rs`, `trailer.rs`).

**Consequence for the plan**: the change is confined to how the angles and the turning line are
chosen (`steer_to`, `axle_steer`, the line used by `lateral_start`) and to the reach of tyres that
do not point at the centre; the rest of the step is untouched.

## R-2. OMSI 2 has no per-axle steering setting

The `.bus` keywords of an axle are `achse_long`, `achse_maxwidth`, `achse_minwidth`,
`achse_raddurchmesser`, `achse_feder`, `achse_maxforce`, `achse_daempfer`, `achse_antrieb`,
`achse_inertia_inv`; none says whether the axle steers. Steering comes from `[rot_pnt_long]` and
`[inv_min_turnradius]` for the whole body; `Axle_Steering_<n>_L/R` are outputs the scripts read.
A steered tag axle that locks at speed is an animation of the scripts.

**Decision**: the feature is Project Depot Physics' own; no fidelity claim is made (Principle II).
Its only OMSI 2 requirement is that the new key is ignored there (R-3).

## R-3. The `.bus` key

**Decision** (maintainer, 2026-11-21): two new axle parameters inside the axle's own
`[newachse]`, each with one value line like `achse_long`: `achse_steer_angle` (lock angle in
degrees) and `achse_steer_centring` (centring rate in degrees per second). No lock speed: when
the axle locks is each vehicle's script's business (R-6, R-9) ([contracts/bus-key.md](contracts/bus-key.md)). One
parameter per value keeps the pattern of the other axle parameters.

**Rationale**: the declaration stays with the axle it describes, so no index can point at the
wrong axle. In the keyword form the axle parameters are named lines that apply to the axle opened
by the last `[newachse]`, in any order, and a line the reader does not know is skipped; the values
are bare numbers, which name no parameter. Depot's own reader (`parse_axle`) already skips
unknown lines inside an axle. That OMSI 2 ignores the new parameters was checked in the game by the
maintainer on 2026-11-21 (T001), as `[depot_physics]` was (spec 002, Q1).

**Alternatives rejected**: a section of its own with an axle index (the maintainer chose the
parameters; an index can drift when axles are added or reordered); one keyword followed by all its
values (does not follow the pattern of the other axle parameters, maintainer 2026-11-21); a side file `<name>.depot.cfg` (not needed:
T001 passed); an optional centring rate with a default (the game would invent a
value, Principle III). The old keyword-less axle form (values only) cannot carry it.

## R-4. The turning line (maintainer decision Q1)

**Decision**: per section and per substep,

```
w_i     = F_i · r_i                          F_i: the axle's static load (D-1 rest load)
r_i     = 1                                  rigid axle
r_i     = 1 − e_i                            steerable axle, e_i ∈ [0, 1] its engagement (R-6)
y_turn  = Σ w_i · y_i / Σ w_i                if Σ w_i > 0
y_turn  = the C-6 line ([rot_pnt_long])      otherwise (every axle steering)
```

The leading section's front axle is excluded (the driver steers it). For a section behind a joint,
its own axles and its own loads.

**Rationale**: with one rigid axle the line sits on it, which is the textbook kinematics of a bus
with a steered tag axle; with several, a load-weighted middle is the steady-state zero-slip point
of axles with cornering stiffness in proportion to load, the usual first approximation of a tandem.
Weighting by `1 − e_i` lets an axle on its way to locked move the line continuously (FR-009).

**Alternatives rejected**: plain mean of rigid axles (ignores that the heavier axle dominates);
keeping `[rot_pnt_long]` (rejected by the maintainer, Q1); solving the tyre forces for the yaw
balance every substep (exact but needs cornering stiffness the `.bus` does not give).

## R-5. The full lock

**Decision**: the front wheels' lock angle stays the `C-6` one,
`lock = atan([inv_min_turnradius] · |y_front − y_rot|)` with `y_rot` the `C-6` line from the file;
the full-lock curvature follows the current line, `κ_max = tan(lock) / |y_front − y_turn|`, and the
driver's `κ = s · κ_max`.

**Rationale**: the steering box limits the wheels' angle, not the radius; the bus then turns tighter
with its tag axle steering and wider with it locked, which is what the maintainer asked for. The
file's tuning (`[inv_min_turnradius]` with `[rot_pnt_long]`) still sets the wheels' angle.

**Alternatives rejected**: keeping `κ_max = [inv_min_turnradius]` (locking a tag axle would not
change the radius).

## R-6. The axle's law, engagement and centring

**Decision** (maintainer, 2026-11-21): the game has no locking rule of its own; the script picks the
mode. Each steerable axle keeps an angle `δ_i` and an engagement `e_i`:

```
s       = δ_front / lock_front                the driver's steering, −1..1 (= κ / κ_max)
target  = σ_i · s · lock_i                     mode 0, basic: in proportion to the axle's own lock;
σ_i     = −1 if y_i < y_turn, +1 if y_i > y_turn, 0 on it
                                               (behind the line opposite, as a 6x2-4 tag axle;
                                               ahead the same way, as a twin-steer second axle)
target  = 0                                    mode 1, locked straight
target  = clamp(δ_script, ±lock_i)             mode 2, script angle
e_i     → 1 in mode 0, → 0 in modes 1 and 2, at rate  centring_i / lock_i  (1/s)
δ_i     moves to its target at most centring_i (deg/s) per second
```

In mode 2 the axle counts as held for the turning line (R-4); in mode 0 as steering. A section behind a joint in mode 0
follows its joint instead of the steering wheel (clarify Q6): `s_f = clamp(κ_f / κ_max, −1, 1)`,
with `κ_f` the joint's curvature (already computed each substep for `C-9`) and `κ_max` the leading
section's current full-lock curvature (R-5), so in a steady turn `s_f` is close to the driver's `s`
and it lags the wheel as the real section does. Rejected: the joint angle over its
`[coupling_front_character]` limit (most files have none, spec 002 P11), the driver's `s` (the axle
would steer before the section enters the bend). The side `σ_i` (clarify Q5) costs one
comparison per axle against `y_turn`, which is computed anyway; it was chosen over "always
opposite" so that a twin-steer axle needs no script. The tyres of an axle keep their own Ackermann angles about
`δ_i`'s centre as today. Uses `y_turn` of the previous substep (one substep, 1/240 s, of lag) to
break the loop between the line and the angles.

The rate limit applies in every mode, also while following the steering in mode 0: a real
actuator has a finite speed, and the angle never jumps. A steering wheel turned faster than the rate
(a mouse, for example) makes the axle lag; the mod author sets `achse_steer_centring` high enough
to keep up (maintainer, 2026-11-21, analysis U1).

**Rationale**: the proportional law is how the steered tag axle of a 6x2-4 is linked to the
steering; rate limits on both the angle and the engagement make every mode change continuous
(FR-009, SC-003); one engagement value serves the line and the locked flag.

**Alternatives rejected**: a lock speed in the `.bus` file (removed by the maintainer: each make
locks differently, so the script decides); a geometric law pointing at the centre of the turn (not
what the maintainer asked for mode 0; a script can compute it and use mode 2); snapping to straight
on a mode change (a yaw kick at speed); a separate "locked" boolean (the line would jump).

## R-7. Scrub of an axle that does not point at the centre

**Decision**: while holding, a tyre whose angle differs from the angle that points at the centre of
the turn by more than a small tolerance (`SCRUB_TOLERANCE`, a *tuned* constant in
`domain::params`) is held with the road's reach (`μ · grip · h` per substep) instead of the
unlimited reach of the leading section's tyres; its force across enters `D-3` with the others.
Rigid axles of a section with steerable axles behind a joint are held at the tyre with the road's reach (not
projected on the line), so they scrub as well.

**Rationale**: a tandem's tyres slide across in a tight turn at walking speed, which costs drive and
grip; the existing solver already caps impulses at the road's reach, so no new force model is
needed. It also avoids the fight between unlimited constraints that ate the drive before Ackermann
was added (comment in `steer_to`).

**Alternatives rejected**: a slip-angle tyre model with cornering stiffness (the `.bus` has no
data for it; a larger change of the Depot model, possible later as its own `D-*`); no scrub at all
(FR-010, SC-006 would fail).

**Risk**: Coulomb-like friction makes the drag independent of how small the error is above the
tolerance. The tolerance and the closed-form test of SC-006 keep it in check; if the in-game
check (SC-008) shows a kick at the threshold, a linear ramp of reach with the error is the fallback.

**Taken (2026-11-21, maintainer's first drive)**: the fallback is now the rule. Below the tolerance a
tyre slightly off the centre fought the other axles with unlimited reach (a slight mouse steer made
the bus need far more drive), above it Coulomb drag jumped in. The reach grows linearly with the
angle off the centre up to `SCRUB_FULL_ANGLE` (5°); only the front axle keeps unlimited reach, and
the section is held at its turning line while holding (`D-7`).

## R-8. Sections behind a joint (maintainer decision Q2)

**Decision**: a section behind a joint with at least one declaration takes its curvature from the
joint as today, points its steerable axles at the centre of that turn (R-6, with its own line R-4),
lets those tyres steer, and is held at each tyre (R-7) rather than at the projected line. A section
without declarations keeps `C-9` exactly.

**Rationale**: generic per section, as the maintainer asked; the joint's curvature is what a real
articulation-angle-controlled rear axle follows.

## R-9. Script interface

**Decision**: three variables per steerable axle, numbered like `Axle_Steering` (axle number over
the whole train): `Axle_Steer_Mode_<n>` and `Axle_Steer_Angle_<n>` written by the
script, `Axle_Steer_Locked_<n>` written by the physics
([contracts/script-vars.md](contracts/script-vars.md)). Read once per frame before the physics
step, as `Axle_Springfactor` is. `Axle_Steering_<n>_L/R` carries `δ_i` (0 for a rigid axle of a
section with steerable axles).

Names chosen by the maintainer (2026-11-21), without a `Depot_` prefix, next to OMSI 2's
`Axle_Steering_<n>`. The implementation must match them whole: `Axle_Steer_` is a prefix of
`Axle_Steering_`, so no lookup by prefix (as in `vehicle/builtins.rs`) may mix the two.

**Rationale**: 0, the value of an undeclared variable, is the basic mode, so a script that knows
nothing of the feature still gets a steering tag axle; OMSI 2 treats the names as ordinary user variables.

**Alternatives rejected**: an `{event}`/macro call (no such mechanism for physics inputs); separate
"lock" and "override" booleans (two variables for one choice).

## R-10. Where it lives (Principle V, X)

- `omsi-vehicle`: parse the two `achse_steer_*` parameters in `parse_axle` into
  `Axle::steer: Option<AxleSteer>` (both or none), validate, report.
- `omsi-sim/src/physics/domain/`: `SteerAxle` (spec values in SI) in `spec.rs`, `SteerState` (angle,
  engagement, mode) in `wheel.rs` or a new `steering.rs`.
- `omsi-sim/src/physics/implementation/depot/steering.rs`: the turning line (R-4), the lock (R-5),
  the law and ramps (R-6), as pure functions with unit tests from `tests/`.
- `physics/implementation/engine.rs`: one rule method choosing `C-6` (Classic, and Depot without
  declarations) or the Depot steering; `steer_to`, `axle_steer` and `lateral_start` call it.
- `vehicle/physics_bridge.rs`: the script variables (R-9); the situation file keeps `δ_i`, `e_i`.

No trait: one Depot implementation and the existing `C-6` code are chosen by the engine's `match`,
as every other `D-*` rule.

## R-11. Performance

Per substep and section: one weighted mean over at most 4-5 axles and one `atan` per steerable
axle; the scrub test is one comparison per tyre. Negligible against the solver's passes. Measured
anyway (SC-007): frame time of V2 with and without the parameters in `examples/handling.rs` and in game.
