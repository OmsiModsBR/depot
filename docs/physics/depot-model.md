# Project Depot Physics: the player's weight sharing (`D-*`) and the AI (`DA-*`)

Project Depot Physics runs every shared rule of [common-model.md](common-model.md) for the player's
road vehicle, with the overrides listed here. It holds only changes that **override** a behaviour of
OMSI 2; behaviour that only adds to OMSI 2 stays on in both engines. There are four overrides: `D-1`
(the plan's decision D2), `D-2`, `D-3` and `D-4`. The AI road vehicle is today's Depot AI, `DA-1` to `DA-6`.

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

## Further `D-*`

A later override that its check in OMSI 2 shows is a difference adds its own id here.
