# Project Depot Physics: the player's weight sharing (`D-*`) and the AI (`DA-*`)

Project Depot Physics runs every shared rule of [common-model.md](common-model.md) for the player's
road vehicle, with the overrides listed here. It holds only changes that **override** a behaviour of
OMSI 2; behaviour that only adds to OMSI 2 stays on in both engines. There are seven overrides: `D-1`
(the plan's decision D2), `D-2`, `D-3`, `D-4`, and `D-5` to `D-7` for steerable axles. The AI road vehicle is today's Depot AI, `DA-1` to `DA-6`.

How to read an entry (inputs, outputs, module, basis, status) is explained in the [README](README.md).
Module paths are under `crates/omsi-sim/src/`. Every rule that differs between the engines is chosen
in one place, `physics/implementation/engine.rs` (one method of `Engine` per rule); the Depot rules
themselves are in `physics/implementation/depot/`.

## Index

| Id | Concept | Basis |
|---|---|---|
| `C-1` to `C-12` | everything in [common-model.md](common-model.md), as it is there | see each entry |
| [D-1](#d-1-static-load-share-about-cog) | the weight is shared about `[cog]` | physics |
| [D-2](#d-2-the-people-aboard-move-cog-and-add-to-the-inertia) | the people aboard move `[cog]` and add to the inertia | physics |
| [D-3](#d-3-the-tyres-hold-axle-by-axle-with-the-turning-they-must-start) | the tyres hold axle by axle, with the turning they must start | physics, tuned |
| [D-4](#d-4-the-payload-slows-the-body-damping) | the payload slows the body's damping | physics |
| [D-5](#d-5-steerable-axles-and-the-turning-line) | steerable axles and the turning line | physics |
| [D-6](#d-6-modes-of-a-steerable-axle) | modes of a steerable axle | physics |
| [D-7](#d-7-scrub) | scrub of tyres that do not point at the centre of the turn | physics, tuned |
| [DA-1](#da-1-path-follower) | path follower | tuned |
| [DA-2](#da-2-pure-pursuit) | pure pursuit steering | tuned |
| [DA-3](#da-3-heading) | heading | physics |
| [DA-4](#da-4-ground-plane) | ground plane | tuned |
| [DA-5](#da-5-sprung-body) | sprung body | tuned |
| [DA-6](#da-6-static-pose) | static pose | physics |

The Depot engine and the Depot AI are two separate choices (D6 of the plan): a vehicle's engine is
chosen when it appears and never changes, the `[depot_physics]` key applies to the player's vehicle
only, and AI copies follow the launcher's setting.

## D-1. Static load share about `[cog]`

- **Inputs**: `[mass]`, `[cog]` (`cog_y`), `achse_long` (`y_i`), `achse_feder` (`k_i`),
  `achse_maxforce` (`F_max,i`). **Outputs**: the rest compression of every strut and the body's rest
  height and pitch. **Module**: `physics/implementation/suspension.rs` (the load reference of `C-3`
  adds nothing), `physics/implementation/spec.rs` (`kinematic_wheel_rest_loads`, the centre `cog_y`).
- **Basis**: *physics*: the weight acts at the centre of gravity, so the struts must balance it
  there. **Status**: the equations are checked by the conformance tests of phase 7; no claim is made
  here about OMSI 2 (the rule it replaces, `K-1`, is the one Classic keeps).

The weight is shared about the centre of gravity: with `W = m g`,

```
Σ F_i                 = W
Σ F_i · (y_i - cog_y) = 0
```

With springs the compression is `s_i = z - θ · y_i` (nose down positive). Let `S0 = Σ k_i`,
`S1 = Σ k_i · y_i`, `S2 = Σ k_i · y_i²` and `M = W · cog_y` (the moment of the weight about the
origin). Then:

```
z = (W · S2 - S1 · M) / (S0 · S2 - S1²)
θ = (S1 · W - S0 · M) / (S0 · S2 - S1²)
s_i = z - θ · y_i
```

For `cog_y = 0` this is the Classic share, `K-1`. A centre of gravity ahead of the middle presses the
front springs more (the nose stands lower); behind it, the rear. The rule is valid while every spring
is pressed and below `F_max,i`. For three or more axles `rest_loads` is the least-squares linear
share, used to place the vehicle at rest, with a floor of 0.2 of an even split on a wheel's load.

**What it means for a mod.** A fuel tank or the engine's place moves the centre of gravity without
changing the axles. Under Depot the axle loads follow it. A mod whose springs were sized for the
origin rule stands differently here: this is the one visible difference between the engines on a
vehicle at rest.

## D-2. The people aboard move `[cog]` and add to the inertia

- **Inputs**: `[mass]`, `[cog]` (`cog_x`, `cog_y`), `[schwerpunkt]` (`cog_z`), `[momentofintertia]`
  (`I_0`), and for each section the people in it: each one's `.hum` `[mass]` (`m_j`) and the centre
  of their own body in the section's frame (`r_j`; see "Each person's centre of mass" below).
  **Outputs**: the body's centre of gravity, so the load on every axle and every side follows it
  through `D-1` and the struts of `C-3` and the bend's pull rolls it by its height (`C-7`), and the
  body's inertia (`C-8`). **Module**: `physics/implementation/depot/load_share.rs` (`loaded_body`),
  `physics/implementation/body.rs` (`set_payload`), `people/aboard.rs` (`payload_by_section`),
  `people/rig/com.rs` (`BodyCom`).
- **Basis**: *physics*: the people and the bus make one rigid body; their weight acts where they are.
  **Status**: the equations are checked by unit tests; no claim is made here about OMSI 2. Classic
  keeps today's rule: the people add their mass at `[cog]`, wherever they are, and the inertia stays
  the file's.

With `M` the body's own mass, `c_0 = (cog_x, cog_y, cog_z)` and `P = Σ m_j` the people's mass:

```
c'  = (M · c_0 + Σ m_j · r_j) / (M + P)
I'  = I_0 + A(M · (c_0 - c')²) + A(Σ m_j · (r_j - c')²)
A(v) = (v_y + v_z,  v_x + v_z,  v_x + v_y)        about the pitch, roll and yaw axes
```

(`v²` taken along each axis.) The products of inertia are left out: the body's inertia is diagonal
(`C-8`). The model origin stays where it is when the centre moves, and so does the motion of every
point of the body; the new inertia applies from the next step. Each section of an articulated bus
takes the people in it, in its own frame. With nobody aboard the centre is `[cog]` and the inertia
`[momentofintertia]`, exactly.

**Each person's centre of mass.** Worked out once for each human type when it is loaded, from the
joints of its rig in two still postures, with the segment shares of the anthropometric tables (D. A.
Winter, *Biomechanics and Motor Control of Human Movement*, table 4.1: head 8.1 %, trunk 49.7 %, each
thigh 10 %, shank 4.65 %, foot 1.45 %, upper arm 2.8 %, forearm 1.6 %, hand 0.6 %). Standing upright
with the arms down, it lies over the feet at about 0.57 of the body's height (1.02 m for a 1.77 m
man); seated upright with the thighs level and the hands on the lap, about 0.07 m ahead of the hips
and 0.19 m over them. A person's centre is that point from their place in the cabin, turned the way
they face. A rig made from broken `[links]` gives the centre of the body as broken as it is drawn.

**What the player notices.** A crowd at the front presses the front axle more and the bus stands a
little nose down; people bunched on one side lean it to that side; a full rear raises the nose and
moves grip to the driven rear axle. A full bus leans and pitches a little more under the brakes and
in bends (its centre of gravity higher) and, once its tyres slide, turns and stops turning more
slowly (its inertia larger). The AI's buses are not affected (`DA-*`).

## The Depot AI

An AI road vehicle on its route is a sprung body (`AiBody`) that follows the planner's path. Its
pieces are `DA-1` to `DA-6`. Module for all: `ai/road.rs`. Traffic planning and the target speed are
shared with the Classic AI. When a vehicle is hit by the player it becomes the player's rigid body, as
in `KA-6` of [classic-model.md](classic-model.md) (decision D3 of the plan), under the Depot rules of
`D-1`.

Notation: `wb` is the wheelbase, `v` the speed, `ψ` the heading, `δ` the steering angle.

## DA-1. Path follower

- **Inputs**: the planner's position and speed `v_plan`, the vehicle's position along the path
  (`along` is the signed distance the vehicle is behind its planned point). **Outputs**: the speed
  `v`. **Module**: `ai/road.rs`.
- **Basis**: *tuned*.

The rotation point is led by the planner's position, and the speed catches up with it:

```
v = clamp( v_plan + 1.5 · along , 0 , v_plan + 3 )
v = 0   when  v_plan < 0.02  and  |along| < 0.05
```

## DA-2. Pure pursuit

- **Inputs**: the path ahead, `v`, `wb`, the model's steering lock, the path's curvature `κ_bend`.
  **Outputs**: the steering angle `δ`. **Module**: `ai/road.rs`. **Basis**: *tuned*.

```
L      = max(1.2 · wb, 3.5) + 0.6 · min(v, 20)               look-ahead distance
δ_want = atan(2 · wb · sin α / reach)                        α: angle to the look-ahead point
lock   = max(lock_model, min(1.15 · atan(κ_bend · wb), 60°)) unless the model's lock is forced
limit  = atan(20 · wb / v²)                                  lateral-acceleration limit
```

The steering angle moves toward `clamp(δ_want, −lock, lock)` and the limit at 25°/s for a vehicle over
6000 kg, 40°/s otherwise (1.5 times as fast while above the model's lock), smoothed with the time
constant 0.08 s (`dt / 0.08` per frame). The lock beyond the model's keeps the vehicle on a junction's
path (#249).

## DA-3. Heading

- **Inputs**: `v`, `δ`, `wb`. **Outputs**: the heading and the rotation point's position.
  **Module**: `ai/road.rs`. **Basis**: *physics* (the kinematic bicycle model).

```
dψ = v · dt · tan δ / wb
```

The rotation point is integrated at the midpoint of the heading step.

## DA-4. Ground plane

- **Inputs**: the road surface under each wheel. **Outputs**: the plane `h = a + b · long + c · lat`.
  **Module**: `ai/road.rs`. **Basis**: *tuned*.

The plane is the least-squares fit through the wheels' contact heights, each wheel read on its own
face of the ground (a ray from 3 m above, with the 0.3 m drop rule).

## DA-5. Sprung body

- **Inputs**: the plane of `DA-4`, the acceleration along and across, `m`, `h_cog`
  (`[schwerpunkt]`), the axles' `achse_feder` and `achse_daempfer`. **Outputs**: heave, pitch and roll
  about the plane, and the `Axle_Suspension` the scripts read. **Module**: `ai/road.rs`.
  **Basis**: *tuned*.

Heave, pitch and roll are three spring-damper systems, run in 4 substeps per frame. Pitch is driven by
`m · a_long · h_cog` and roll by `−m · a_lat · h_cog`. They are clamped to ±0.15 m, ±0.07 rad and
±0.09 rad.

- The spring and damper of each mode come from the wheels' springs and dampers. A file without springs
  gets 1.5 Hz at 0.35 of the critical damping.
- The damping is raised to at least 0.8 of the critical damping.
- The inertia is raised so that no mode passes about 6 Hz (38 rad/s), which keeps the 4 substeps stable.

## DA-6. Static pose

- **Inputs**: `[ai_deltaheight]`, the axle loads of `D-1`, `achse_feder`. **Outputs**: the vehicle's
  height at rest and its static `Axle_Suspension`. **Module**: `ai/road.rs`. **Basis**: *physics*.

The pose at rest is the `[ai_deltaheight]` offset plus the sag of each wheel, `min(F_i / k_sag, 0.24)`
m, with `F_i` the load of the weight shared about `[cog]` (`D-1`).

## D-3. The tyres hold axle by axle, with the turning they must start

- **Inputs**: each tyre's grip (`μ · N_i`, its load from its strut as in `C-7`) and lever `y_i` (m
  forward of the centre of gravity), the bend's pull `F = m · v² · κ` less a joint's push (as in
  `C-7`), the body's yaw inertia `I_z` (`C-8`, with `D-2`) and its yaw rate. **Outputs**: whether the
  tyres still hold (`C-7`'s holding state). **Module**: `physics/implementation/depot/grip.rs`
  (`holds_across`, `yaw_accel`), `physics/implementation/tyre.rs` (`lateral_start`).
- **Basis**: *physics* for the split of the force between the axles; *tuned* for the tyres'
  relaxation length (`RELAXATION_LENGTH`, 0.6 m). **Status**: checked by unit tests; no claim is made
  here about OMSI 2. Classic keeps `C-7`: the bend's pull against the grip of all tyres together,
  with no turning to start.

While holding, the body's yaw rate is set to `v · κ` (`C-7`), so the tyres must also give the moment
that starts or stops that turning. With the tyres ahead of the centre of gravity at their
grip-weighted lever `y_f` and those behind at `y_r`:

```
F_f + F_r         = F
y_f F_f + y_r F_r = - I_z · ω̇
|F_f| ≤ μ N_f  and  |F_r| ≤ μ N_r        the tyres hold
```

`ω̇` is the change of the yaw rate over a substep, followed with the lag of the tyres' relaxation,
`τ = 0.6 m / max(v, 1 m/s)`: a steering key that sets the full bend in one step does not ask an
endless force. A body with tyres on one side of its centre of gravity only (a rear section on one
axle) is held by all of them together, as in `C-7`.

**What it means.** In a steady bend nothing changes: the weight is shared about `[cog]` (`D-1`), so
`N_f · y_f = - N_r · y_r` and each axle is asked the same share of its grip as all the tyres
together. A quick swing of the steering (a lane change, a swerve) now asks more of the tyres, the
more so the larger `[momentofintertia]`'s yaw value (and the passengers' share, `D-2`): on the test
bus at 50 km/h, a swing to and back from the steering's position in one second lets the tyres go at
0.24 of the lock with a yaw inertia of 130 t m² (Classic: 0.34), 0.14 with 400 t m². Braking or
accelerating in a bend moves load between the axles and lets the lighter one go first. A
`[momentofintertia]` far larger than the bus's real one (about `m · (L² + W²) / 12`) now shows as a
bus that lets go too easily in quick manoeuvres; the value belongs fixed in the file (Principle III).

## D-4. The payload slows the body damping

- **Inputs**: `f_body` of the empty body (`C-8`), `[mass]`, the people aboard. **Outputs**: `f_body`
  with the payload. **Module**: `physics/implementation/depot/load_share.rs` (`loaded_body_freq`),
  through `set_payload`.
- **Basis**: *physics*. **Status**: checked by unit tests; no claim is made here about OMSI 2.
  Classic keeps the empty body's (`K-5`).

The springs stay and the mass grows, so the body's pitch and roll follow the heavier body:

```
f_body = f_body,empty · sqrt( m_empty / (m_empty + payload) )
```

## D-5. Steerable axles and the turning line

> In the game for every section, leading or behind a joint, with the modes the scripts pick
> (spec 004, phases 3 to 5).

- **Inputs**: `achse_steer_angle` (`lock_i`, the axle's own lock) and `achse_steer_centring` of each
  steerable axle (`docs/FORMATS.md`), `achse_long` (`y_i`), each axle's load at rest `F_i` (`D-1`),
  each steerable axle's angle `δ_i` (`D-6`), `[inv_min_turnradius]`, `[rot_pnt_long]` (`C-6`). **Outputs**: the section's turning line `y_turn`, the full-lock curvature `κ_max` and the
  driver's curvature `κ`. **Module**: `physics/implementation/depot/steering.rs` (`turning_line`,
  `full_lock`), chosen in `physics/implementation/engine.rs`; the values in
  `physics/domain/spec.rs` (`SteerAxle`).
- **Basis**: *physics*: the body turns about the point where the side forces of its tyres
  balance, each axle's in proportion to its load (cornering stiffness in proportion to load) and to
  its slip; one rigid axle turns about its own line, several about their load-weighted middle.
  **Status**: checked by the conformance tests; no claim is made here about OMSI 2, which has no
  steerable axle of its own (the scripts only animate one). Classic, and a section with no steerable
  axle under Depot, keep `C-6` and `C-9` exactly.

An axle is steerable when its `[newachse]` carries both parameters; in a section with at least one,
every other axle is rigid, except the leading section's front axle, which the driver steers. Per
section and per substep:

```
p_i     = y_i − tan(δ_i) / κ            where axle i points on the body's middle (y_i for a rigid one)
y_turn  = Σ F_i · p_i / Σ F_i           over the section's axles but the leading front axle,
                                        F_i the axle's load at rest (D-1); then kept between the
                                        rearmost axle and 0.5 m behind the front one
y_turn  = the C-6 line                  when every axle but the front one steers
```

`κ` and `δ_i` are the substep before's (`D-6`); `|κ|` is taken as at least `10⁻³` 1/m, as straight
on a steered axle points at no finite place (and the line then hardly matters). An axle that points
at the centre of the turn about the line (`p_i = y_turn`) does not move it; one that turns less than
that pulls the line its way, so that the rear axles' tyres share the slip and none fights the others
alone.

The front wheels keep the lock `C-6` gives them, from the file's `[rot_pnt_long]` line (`y_rot`);
the full lock follows the current line:

```
lock_front = atan( [inv_min_turnradius] · |y_front − y_rot| )
κ_max      = tan(lock_front) / |y_front − y_turn|
κ          = s · κ_max                  s ∈ [−1, 1] the driver's steering
```

The tyres of each axle still point at the centre of the turn (Ackermann, `C-6`) about their axle's
angle. A section behind a joint takes its curvature from the joint (`C-9`), with its own axles,
loads and line.

**Closed forms** (the conformance vehicles of `crates/omsi-sim/tests/support/vehicles.rs`, every
`[inv_min_turnradius]` 0.12 1/m):

- **V2**, 6x2 rigid, axles at `4.4`, `−1.9`, `−5.2` m, `[cog]` y `0.4`, `y_rot = −1.9`; the loads at
  rest share `0.478 : 0.306 : 0.216`, and `lock_front = atan(0.12 · 6.3) = 37.09°`. Tag axle pointing
  at the centre of a turn about the middle axle: `y_turn = −1.9`, `κ_max = 0.120` 1/m, radius 8.33 m
  at the line. Tag axle at a 15° lock, which that full lock would ask 18.2° of: `y_turn = −2.285`,
  `κ_max = 0.1131` 1/m, radius 8.84 m (solved together, the line and the full lock). Tag axle locked:
  `y_turn = (0.306 · −1.9 + 0.216 · −5.2) / 0.522 = −3.265`, `κ_max = 0.0986` 1/m, radius 10.14 m.
  Locking the tag axle widens the full-lock circle by 1.3 m (1.8 m from one that points at the
  centre).
- **V5**, articulated 8x2: the rear section has axles at `1.2` and `−4.2` m, `[cog]` y `0.6`, its
  ball at `5.6`; the front section's line is at `−2.5`, its ball at `−4.4`, `1.9` m behind it
  (`D = −1.9` m, counted forward). The joint carries part of the rear section, and its axles' loads
  at rest share `0.571 : 0.429`. Fourth axle steerable by `34.85°`, which points it at the centre
  of the turn at half steering: `y_turn = 1.2`, `d = 4.4` m, the axle at `−s_f · lock` with
  `s_f = 0.70`; locked: `y_turn = 0.571 · 1.2 + 0.429 · −4.2 = −1.12`, `d = 6.72` m. At half
  steering (`R = 16.67` m) the steady articulation angle of `C-9`, `R · sin α + D · cos α = d`, is
  21.7° steering and 30.1° locked, and the rear section's line runs on a circle of 16.19 m and 15.37 m:
  the rear section cuts the corner less with its last axle steering. Without the declaration it
  turns about its file's line, here `1.2`, as `C-9` has it.

## D-6. Modes of a steerable axle

> As `D-5`: in the game for every section.

- **Inputs**: the axle's mode `Axle_Steer_Mode_<n>` and angle `Axle_Steer_Angle_<n>` from the
  scripts (`docs/FORMATS.md`), `lock_i` and the centring rate `c_i` (`achse_steer_centring`, °/s),
  the driver's steering `s` (`D-5`) or, behind a joint, the joint's curvature `κ_joint` (`C-9`), the
  turning line `y_turn` (`D-5`). **Outputs**: the axle's angle `δ_i` (the scripts'
  `Axle_Steering_<n>_L/R`), the locked flag `Axle_Steer_Locked_<n>`.
  **Module**: `physics/implementation/depot/steering.rs` (`target_angle`, `advance`), the state in
  `physics/domain/steering.rs` (`SteerState`).
- **Basis**: *physics*: the proportional law is how the steered tag axle of a 6x2-4 is linked to
  the steering; an actuator moves at a finite speed. **Status**: checked by unit tests; no claim is
  made here about OMSI 2.

The game never locks a steerable axle by itself: speed, gear, fault or dashboard logic is the
vehicle's script's, through the mode (0 for a script that does not know the variable).

```
mode 0 (basic)   target = σ_i · s · lock_i       σ_i = −1 if y_i < y_turn  (behind the line: the other way, a tag axle)
                                                 σ_i = +1 if y_i > y_turn  (ahead of it: the same way, a twin-steer axle)
                                                 σ_i =  0 on the line
                 s   = κ / κ_max                 the leading section
                 s_f = clamp( κ_joint / κ_max, −1, 1 )      a section behind a joint, κ_max of the leading section
mode 1 (locked)  target = 0
mode 2 (script)  target = clamp( Axle_Steer_Angle_<n>, −lock_i, lock_i )
any other mode   mode 0, one log line per vehicle; a script angle that is not a number: mode 0

δ_i  → target, at most  c_i  degrees per second, in every mode
Axle_Steer_Locked_<n> = 1  while mode 1 has brought δ_i to 0, else 0
```

`σ_i` and `s` use the turning line and the full lock of the substep before (1/240 s of lag), which
breaks the loop between the line and the angles. Behind a joint the axle follows the joint's bend
rather than the steering wheel, so it steers as the section enters the bend, not before. In a mode
change the angle moves at the centring rate, and the line (`D-5`), which follows the angle, and the
full lock move without a jump, and so does the body. The rate limits the angle while following the
steering too: a steering wheel turned faster than `c_i` (with a mouse, for example) makes the axle
lag, so a mod sets `achse_steer_centring` high enough to keep up. A rigid axle of a section with
steerable axles shows `Axle_Steering_<n>` = 0.

**What it means for a mod.** A 6x2-4 with a steered tag axle needs only the two parameters: in
mode 0 the tag axle turns against the front wheels. Locking it above a speed, in reverse or on a
fault is a line of the bus's script (`Axle_Steer_Mode_<n>` = 1), and the tag axle centres smoothly at
its centring rate; a dashboard lamp can read `Axle_Steer_Locked_<n>`. Mode 2 lets a script compute
any other law (pointing at the centre of the turn, a speed-dependent ratio) and hand the angle over.

## D-7. Scrub

> As `D-5`: in the game for every section.

- **Inputs**: each tyre's angle and the angle that points it at the centre of the turn (`C-6`), the
  road's reach `μ · grip_i · h` (`C-7`). **Outputs**: how much of the road's grip each tyre takes
  across while the tyres hold, and its force across, which enters `D-3`. **Module**:
  `physics/implementation/depot/steering.rs` (`scrub_reach`), `physics/implementation/tyre.rs`
  (`lateral_start`), the angle `SCRUB_FULL_ANGLE` in `physics/domain/params.rs`.
- **Basis**: *physics* for the friction; *tuned* for the angle. **Status**: checked by the
  conformance tests; no claim is made here about OMSI 2.

Under `C-7` the leading section's tyres hold with unlimited reach, which is right only while every
tyre points at the centre of the turn. In a section with steerable axles, while the tyres hold, only
the front axle's tyres keep the unlimited reach; every other tyre is held with a share of the road's
reach that grows with how far it points off the centre of the turn, `δ_off`:

```
reach_i = min( |δ_off,i| / SCRUB_FULL_ANGLE , 1 ) · μ · grip_i · h        SCRUB_FULL_ANGLE = 5°
```

as a tyre's side force grows with its slip angle up to the road's grip. A tyre that points at the
centre takes nothing across and slides nowhere; one far off it scrubs with the road's whole grip. A
section with steerable axles behind a joint is held at each tyre rather than at its turning line
(`C-9`), so its rigid axles scrub as well, and its driver's axle is none: every tyre's reach follows
the rule above. These forces across enter `D-3`'s balance with the others.

While holding, the section is also held at its turning line (`D-5`): no speed across at the point
of the line in the body's middle, with unlimited reach, as `C-9` holds a section behind a joint.
Behind a joint that hold takes at most the road's reach of all the section's tyres, `Σ μ · grip_i ·
h`, which is what `C-9` lets it take at its line, and the section holds again (`C-7`) only once the
speed across at its line is under 0.1 m/s as well, since every tyre of it may scrub.
Without it the scrubbing tyres alone would set where the body turns: with a locked tag axle, about
the lighter axle instead of the load-weighted middle. A scrubbing tyre slides across by design, so it
is left out of the speed check by which the tyres hold again (`C-7`).

With steerable axles, while holding, the tyres take no roll across and the bend's pull rolls the
body whatever the body's speed along and across (`C-7` does so only while the body follows its
wheels: over 1 m/s along and under 0.5 m/s across at `[cog]`). Below 1 m/s the scrub rolled the body
away from the turn, and it rocked back as it reached that speed; with the turning line well behind
`[cog]`, as on a 6x2-4 whose line is on its driven axle, the speed across at `[cog]` passes 0.5 m/s in
a turn at town speeds, and the roll swapped between the tyres and the bend's pull in one step.

So a tandem's tyres slide across in a tight turn at walking speed, which costs drive and grip, and a
tag axle locked in a tight turn drags; a tag axle that turns less than the centre asks shares the
slip with the other rear axles (`D-5`) and costs about what a rigid tandem does.

**Why not the road's whole grip at any angle.** With Coulomb friction for every tyre off the centre
by more than a tolerance, a tag axle a fraction of a degree off it at a slight steer (a mouse's small
corrections) either fought the other axles with unlimited reach below the tolerance or dragged with
the whole grip above it: the bus needed far more drive to speed up, and crossing the tolerance gave
it a jolt. The share that grows with the angle has neither. With the turning line of the rigid axles
alone, a tag axle that turns less than the centre asks (as a linked one does) scrubbed alone against
the line's unlimited hold, with the road's whole grip at a few degrees: in a turn the bus needed far
more drive and spun its driven wheels. The line where the slip balances (`D-5`) shares it.

## Further `D-*`

A later override that its check in OMSI 2 shows is a difference adds its own id here.
