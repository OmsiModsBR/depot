# Common model: what both engines share (`C-*`)

The equations of the rigid body that the player's road vehicle runs in **both** engines. Where an
engine overrides one, its own document says so and keeps the id of the rule it replaces
(`K-1` and `D-1` replace the load reference of `C-3`). How to read an entry (inputs, outputs, basis,
status) is explained in the [README](README.md).

Where a module path says `physics/…` it is under `crates/omsi-sim/src/`.

| Id | Concept |
|---|---|
| [C-1](#c-1-bus-file-to-model) | `.bus` file to model: keys, units, defaults |
| [C-2](#c-2-time-integration) | time integration |
| [C-3](#c-3-strut-and-ground) | strut and ground |
| [C-4](#c-4-drive-brakes-and-rolling-resistance) | drive, brakes and rolling resistance |
| [C-5](#c-5-grip) | grip |
| [C-6](#c-6-steering-geometry) | steering geometry |
| [C-7](#c-7-adhering-and-sliding) | adhering and sliding |
| [C-8](#c-8-body) | the body |
| [C-9](#c-9-joints) | joints of articulated vehicles |
| [C-10](#c-10-hits) | hits against obstacles |
| [C-11](#c-11-kinematic-model-and-followers) | kinematic model, rail and air followers |

## C-1. `.bus` file to model

A `.bus` file is read in one place, so every model reads the same values.

- **Inputs**: the keys below. **Outputs**: the physics specification of the vehicle (mass, inertia,
  centre of gravity, turning line, axles, rest loads). **Module**: `physics/implementation/spec.rs`
  (types in `physics/domain/spec.rs`; the file is parsed in `crates/omsi-vehicle/src/vehicle.rs`).
- **Basis**: *OMSI*. The player's vehicle reads its file as the game does: no guess at units, no
  minimum, nothing invented for a missing value (OMSI's own defaults aside). A model configured wrongly
  behaves wrongly.

| Key | Unit in the file | Unit in the model | When missing |
|---|---|---|---|
| `[mass]` | t | `m` in kg (× 1000) | 1 t |
| `[momentofintertia]` | 3 values, t·m² | `I_x, I_y, I_z` in kg·m² (× 1000): pitch (about the lateral axis), roll (about the long axis), yaw (about the vertical axis) | 100 t·m² about each axis |
| `[cog]` | x, y, z in m | centre of gravity `x`, `y`; the third value is not used here | `(0, 0)` |
| `[schwerpunkt]` | m | `h_cog`: height of the centre of gravity, the lever the weight shifts by | 0: the body then leans into no acceleration |
| `[rot_pnt_long]` | m | `r`, counted from the body's centre (see `C-6`) | 0 |
| `[inv_min_turnradius]` | 1/m | `κ_max`, the curvature of full lock | 0: the vehicle does not steer |
| `[rollwiderstand]` | N, as given | `F_roll`, a force shared over the wheels | none |
| `[boundingbox]` | sizes and the offset of its centre, m | the body's footprint for `C-10`; its centre `y` when `[cog]` is missing (`C-6`) | none |
| `achse_long` | m | `y_i`: the axle's place along the body | 0 |
| `achse_raddurchmesser` | m | `r_i` = half | — |
| `achse_minwidth`, `achse_maxwidth` | m | the strut acts at `maxwidth / 2`; the tyres stand halfway between the two widths | — |
| `achse_feder` | kN/m | `k_i` per wheel, N/m (× 1000) | no spring carries nothing |
| `achse_daempfer` | kN·s/m | `c_i` per wheel, N·s/m (× 1000) | none |
| `achse_maxforce` | kN | `F_max,i` per wheel, N (× 1000) | no largest force: the strut pushes nothing |
| `achse_antrieb` | share, 0 = none | the axle is driven when not 0 | not driven |
| `achse_inertia_inv` | 1/(kg·m²) | the wheel's inverse rotational inertia | 0.002 |

**Status**: the order of `[momentofintertia]` (first, second, third value = pitch, roll, yaw) is
**validated**: on a copy of a stock bus given `100 / 1 / 100`, OMSI 2 sways from side to side at every
lateral load shift, so the second value is the roll (#655, black-box test 2). The units and the
defaults are **to validate**: check each with a file that leaves the key out and compare the weight
and the sway in OMSI 2 and in the game.

## C-2. Time integration

- **Inputs**: the frame time `Δt`. **Outputs**: the body state after the frame. **Module**:
  `physics/implementation/train.rs` (`step_train`), constants in `physics/domain/params.rs`.
- **Basis**: *tuned*.

A frame longer than 0.25 s is cut to 0.25 s. The frame's time, plus what the frames before it left
over, is run in fixed substeps of `h = 1/240 s`; the remainder is carried to the next frame, so the
motion does not depend on the frame rate. Substeps are run in slices of at most 0.05 s, and each slice
answers the obstacles once (`C-10`). Each substep integrates semi-implicitly: velocities from the
forces first, then positions from the new velocities. The tyres' constraints are solved in 8 passes
(16 with joints).

**Status**: the integration scheme is Depot's own and shared. That the same manoeuvre comes out the
same at a 30 and a 60 fps cap in OMSI 2 is **to validate** (candidate V1 of the research of the plan).

## C-3. Strut and ground

- **Inputs**: `achse_feder` (`k`), `achse_daempfer` (`c`), `achse_maxforce` (`F_max`), `achse_long`,
  `achse_minwidth`, `achse_maxwidth`; the script variable `Axle_Springfactor` (`s_f`, 1 when the
  scripts do not set it); the ground under each wheel. **Outputs**: the force each strut puts on the
  body, the wheel's load, and `Axle_Suspension` (the wheel's travel) for the scripts. **Module**:
  `physics/implementation/suspension.rs` (`wheel_contacts`), `ground.rs` for the ground, `spec.rs` for
  the rest loads.
- **Basis**: *OMSI* for the strut; *tuned* for the tyre envelope.

The body hangs straight on a spring under each wheel, with no wheel mass, no tyre spring and no bump
stop between the road and the body:

```
t   = height of the ground over the spring's point, measured straight up
      (the point is on the model's origin plane, z = 0, under the wheel)
N   = min( k · s_f · t − c · v_up , F_max )     when k · s_f · t ≥ 0
N   = 0                                          otherwise (the wheel is in the air)
travel = clamp( t , −droop , F_max / k )         droop = 0.15 m
```

`v_up` is the body's upward speed at the point where the strut acts (`achse_maxwidth / 2` across, the
axle's place along). The ground is looked for from just over the wheel's climb, and, when nothing is
found, again from 3 m over the origin plane. A wheel in the air pushes nothing and its tyres stop
holding (`C-7`). The travel is handed to the scripts as `Axle_Suspension` (sign and unit conversion in
`vehicle/physics_bridge.rs`).

**The load reference.** The strut's force `N`, at the wheel, turns the body about its centre of
gravity. Which point the weight is shared about is the one thing the two engines differ in:

```
torque about the centre of gravity (body frame):  ( −lever·N , … )   the strut's own arm
Classic  adds   ( cog_y · N , −cog_x · N , 0 )     → the weight is shared about the model origin  (K-1)
Depot    adds   nothing                            → the weight is shared about [cog]             (D-1)
```

**Rest height.** At rest each wheel's compression is `rest_load / (k · s_f)`, at most the bump stop
(0.24 m), and the model origin stands above the ground by the mean over the wheels of
`r − hub height − compression`.

**Tyre on the ground.** A tyre is a flattened envelope over the ground (13 samples over two radii ahead
and behind); it climbs a step up to 0.8 of its radius and a higher face is a wall. A wall pushes the
tyre back with stiffness `1.5·10⁶ N/m` and damping `6·10⁴ N·s/m`, and stops the hub at once by an
impulse.

**Status**: the strut is **to validate**: the Mi5 at rest on a flat stop, in OMSI 2 and in the game,
compared axle by axle (quickstart Q3 of the plan); the spring's point on the origin plane and the 3 m
ground query have no recorded check yet. The tyre envelope, the climb of 0.8 and the walls are *tuned*;
that OMSI 2's wheel climbs a 15 cm kerb as the model does is **to validate** (candidate V4).

## C-4. Drive, brakes and rolling resistance

- **Inputs**: the script variables `M_Wheel` (the drive torque at the driven wheels, N·m),
  `Brakeforce` and `Axle_Brakeforce_<axle>_<side>` (per wheel, N, cleared each frame once read);
  `[rollwiderstand]`; `achse_raddurchmesser`; `achse_antrieb`. **Outputs**: the forces along each tyre,
  each wheel's spin (`Wheel_Rotation`, `Wheel_RotationSpeed`). **Module**:
  `physics/implementation/tyre.rs` (`prepare`), `wheel.rs`; the scripts' side in
  `vehicle/physics_bridge.rs`.
- **Basis**: *OMSI* for the inputs and their units; *tuned* for the thresholds.

```
F_drive,i = M_Wheel / ( r_i · n_driven )          for each driven wheel; n_driven counts the driven
                                                  wheels of the whole train
F_brake,i = Brakeforce_i + F_roll / n_wheels      what the wheel's brake (and the rolling resistance) bears
rolling tyre (|v| > 0.15 m/s):   F_i = F_drive,i − F_brake,i · sign(v)
slipping tyre:                   F_i = μ · N_i · sign(slide)          slide = ω·r − v
standing tyres (|v| ≤ 0.15 m/s): hold together: each takes
                                 clamp( need · F_brake,i / Σ F_brake , ± F_brake,i )
                                 need = −(everything else along the body) − m·v/h
every F_i is limited to  ± μ · min(N_i, F_max,i)              (C-5)
```

Driving and braking pitch the body about a lever longer than the centre of gravity's height: the
tyres' forces act at the hubs, so the body is also turned by `Σ F_i · r_1` (`r_1` the radius of the
first axle's wheels), unless every tyre stands and the brakes hold.

**Status**: **to validate**: a pull-away with a known `M_Wheel`, a brake stop, and a coast on a flat
road without the brakes, compared in OMSI 2 and in the game (candidate V3 for the air drag, `C-8`).

## C-5. Grip

- **Inputs**: the road under the vehicle (its condition and the temperature); the load `N` of each
  strut. **Outputs**: `μ`, and the most a tyre can pass on. **Module**: `vehicle/weather.rs`
  (`road_grip`) sets `μ` on the body (`RigidBody::friction`, 0.85 on a dry road until set).
- **Basis**: *tuned* for the values of `μ`; *OMSI* for the rule below.

```
μ        = grip of the road: dry 0.85, wet 0.6, snow 0.3, ice 0.12, blended by the road's condition
grip_i   = min( N_i , F_max,i )                 a tyre works with at most its rated load
F_i max  = μ · grip_i
```

**Status**: that a tyre carries at most `μ · min(N, F_max)` is **to validate** (a brake stop at a
rising load on a dry and a wet road); the values of `μ` are Depot's.

## C-6. Steering geometry

- **Inputs**: `[inv_min_turnradius]`, `[rot_pnt_long]`, `[cog]` (or `[boundingbox]`), `achse_long`; the
  steering `s ∈ [−1, 1]` of the driver. **Outputs**: the curvature `κ`, each axle's angle
  (`Axle_Steering` for the scripts) and each tyre's own angle. **Module**:
  `physics/implementation/body.rs` (`steer_to`, `axle_steer`), `spec.rs` (the turning line).
- **Basis**: *OMSI*.

```
y_turn  = centre + [rot_pnt_long]               centre = [cog] y, else the middle of [boundingbox], else 0
κ       = s · [inv_min_turnradius]              |κ| ≤ [inv_min_turnradius]; s = 1 is the full lock
δ_i     = atan( (y_i − y_turn) · κ )            the axle's angle, the same for its left and right wheel
tyre    = atan( (y_i − y_turn) · κ / (1 − x · κ) )     each tyre square to the line from the centre of
                                                       the turn to itself (Ackermann)
lock    = atan( [inv_min_turnradius] · |y_front − y_turn| )
```

An axle behind the line (`y_i < y_turn`) points the other way, and so does a tag axle. A section behind
a joint has no steering of its own: its curvature comes from the joint (`C-9`).

**Status**: the turning line counted from `[cog]` is **validated**: with a `[cog]` of `0 / 3 / 1.2` on a
copy of a stock bus, driven at about 5 km/h at full lock, OMSI 2's pivot moves forward and the rear
axle drags sideways (#655, black-box test 1). The per-axle angles `δ_i` are **to validate**: compare
the `Axle_Steering` variables of a 6x2 bus with a steered third axle in OMSI 2 and in the game.

## C-7. Adhering and sliding

- **Inputs**: `κ` (`C-6`), the speed, the loads and `μ`. **Outputs**: whether the tyres hold, and the
  forces across each tyre. **Module**: `physics/implementation/tyre.rs` (`lateral_start`,
  `lateral_pass`, `lateral_end`).
- **Basis**: *OMSI* for the two states and the re-hold speed; *tuned* for the other thresholds.

While the tyres **hold**, the body follows its wheels: it turns about the turning line at once,

```
ω_z = v · κ          the direction of the velocity follows the wheels
```

and the tyres take whatever keeps them rolling where they point, each at most `μ · grip_i · h` of
impulse a substep. They **let go** when

```
m · v² · |κ| − (what a joint pushes across)  >  Σ μ · grip_i        the bend asks more than the road gives
or a wheel slips, or a wheel is in the air,
or the speed across at the turning line is over 0.5 m/s              a knock from outside
```

or when a wheel is struck by a face (`C-3`). Sliding, each tyre passes on at most `μ N` across. The
tyres **hold again** once the sideways speed at every tyre is under 0.1 m/s and no wheel slips. While
holding, the bend's pull rolls the body by `m · h_cog · v · ω_z` (about the longitudinal axis).

**Status**: **to validate**: full lock at rising speed on a dry and a wet road, noting the speed at
which the bus lets go and the speed at which it holds again, in OMSI 2 and in the game.

## C-8. Body

- **Inputs**: all the forces and torques above, the mass, the inertia, the centre of gravity, the
  people aboard (`payload`). **Outputs**: position, orientation, velocity, angular velocity, and the
  accelerations the scripts read. **Module**: `physics/implementation/body.rs` (`integrate`,
  `body_damping`), the air in `tyre.rs` (`prepare`).
- **Basis**: *physics* for Newton–Euler, gravity and the air; *OMSI* for the damping of pitch and roll;
  *tuned* for the rest.

```
m · dv/dt  = Σ F  + m · g · (−z)  + F_air              g = 9.81 m/s²
I · dω/dt  = Σ τ  − ω × (I · ω)                        I = (I_x, I_y, I_z) = pitch, roll, yaw (C-1)
F_air      = − ½ · ρ · c_w · A · v · |v|               ρ = 1.2, c_w = 0.6, A = clamp(m_empty / 2200, 2, 8.5) m²
                                                       only on the leading body of a train
```

The body's centre is `([cog] x, [cog] y, [schwerpunkt])`. The people aboard add to the mass in the
gravity, the tyres and the acceleration; the inertia stays the file's.

OMSI's per-frame damping of the body's pitch and roll rates, as it is applied over a time `dt`:

```
f_body   = sqrt( Σ 2 · k_i  /  m )        k_i in kN/m, m in t, over the axles
x        = 1.5 · f_body / 30
rate    ← rate · ( sin x / x )^( dt / (1/30 s) )      (clamped to 0.1 … 1 per frame)
```

A body whose up axis leans past 72.5° is set back on its wheels (a safety net on bad ground, *tuned*,
not a model rule).

**Status**: the order of the inertia is **validated** (`C-1`). The damping of pitch and roll is
**to validate**: the decay of the body's pitch after a kerb drop, in OMSI 2 and in the game. The air
drag is **to validate** (candidate V3: coast down a 6–8 % grade without the brakes; if OMSI 2 has no
drag the term moves to the Depot engine).

## C-9. Joints

- **Inputs**: `[coupling_back]` of the section in front and `[coupling_front]` of the one behind
  (positions in each file's frame), `[coupling_front_character]` (the joint's type, the most it turns
  about the vertical axis `α_max`, and its range `β_min … β_max` about the transverse axis),
  `[coupling_pitch_offset]` (Depot's own, a pusher's hinge). **Outputs**: the articulation angles `α`
  (vertical) and `β` (transverse) the scripts read, and the curvature the section behind turns on.
  **Module**: `physics/implementation/train.rs` (`Joint`, `step_train`), types in
  `physics/domain/train.rs`; the angles for the scripts in `vehicle/articulation.rs`.
- **Basis**: *tuned* for the solver; the angles' definition follows what OMSI 2's scripts read
  (#384, #407).

The sections are rigid bodies joined by a ball at the two coupling points: a velocity constraint
solved with the tyres' (16 passes), with a correction of 20 % of the position error per substep, at
most 1 m/s or 1 rad/s. A hitch (no `[coupling_front_character]`, or type 0) is free about every axis;
a turntable (any other type) lets the sections not roll against each other and stops at `α_max`,
`β_min` and `β_max`. The section behind turns on the curvature its joint gives it:

```
κ_follow = sin α / max( | y_ball − y_turn,follow | , 0.5 m )       y_turn,follow: its own turning line
```

**Status**: **to validate**: the steady articulation angle of an articulated bus in a steady turn
(radius and `α`), in OMSI 2 and in the game. The pitch about the hinge of a pusher is Depot's own. Known
and left as it is (maintainer, 2026-10-23): after a hard stop the sections of an articulated bus rock in
a pitch wave in the game, which OMSI 2 does not show.

## C-10. Hits

- **Inputs**: the body's `[boundingbox]`, the obstacles (boxes with a velocity, `[crashmode_pole]`
  ones that break off), the wheels' wall faces. **Outputs**: impulses on the body, `Impact` records
  for the scripts (point, speed, energy). **Module**: `physics/implementation/collide.rs`,
  `physics/domain/impact.rs`; the wheels' faces in `tyre.rs` and `suspension.rs`.
- **Basis**: *tuned*.

```
closing speed along the normal is removed; above 1 m/s a fifth of it comes back   (restitution 0.2)
sliding along the obstacle is braked by friction                                  (0.4)
a touch under 0.4 m/s closing is a push, not a crash
the body is put 1 mm clear of the obstacle
```

At most four obstacles are answered per call, each once, deepest first. An obstacle the body overlaps by
more than 0.5 m at spawn (plus what it could have moved in two frames) is one the vehicle has been
standing in all along (a depot hall's box) and never pushes. A hit releases the tyres' hold (`C-7`).
Only the player's body tests obstacles today; AI vehicles are neither pushed nor released by it (the
Classic AI adds that, `KA-6`).

**Status**: the coefficients are Depot's; that OMSI 2's vehicle bounces and scrapes alike is
**to validate** (a bump against a wall at 5 and 20 km/h).

## C-11. Kinematic model and followers

The kinematic model (longitudinal forces from `M_Wheel`, the brakes and `[rollwiderstand]`, kinematic
steering, the ground followed under the wheels) moves the player's rail vehicles, the parts of AI
vehicles in the Depot AI, and every vehicle of the AI that is not a road one (rail and air followers).
No engine overrides it; it is written here for completeness. **Module**:
`physics/implementation/kinematic.rs`, `ai/rail.rs`, `ai/air.rs`.

```
a      = ( M_Wheel / r_driven − (ΣBrakeforce + F_roll) · sign(v) ) / m      resistances never reverse v
steer  → s · lock, at 2.5 · lock per second                                 lock held to 10° … 60°
dψ     = v · dt · tan(steer) / wheelbase                                     dt ≤ 0.1 s
```

**Status**: not a rule of OMSI 2's: Depot's own, shared by both engines.
