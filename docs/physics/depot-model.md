# Project Depot Physics: the player's weight sharing (`D-*`) and the AI (`DA-*`)

Project Depot Physics runs every shared rule of [common-model.md](common-model.md) for the player's
road vehicle, with the overrides listed here. It holds only changes that **override** a behaviour of
OMSI 2; behaviour that only adds to OMSI 2 stays on in both engines. There are two overrides,
`D-1` (the plan's decision D2) and `D-2`. The AI road vehicle is today's Depot AI, `DA-1` to `DA-6`.

How to read an entry (inputs, outputs, module, basis, status) is explained in the [README](README.md).
Module paths are under `crates/omsi-sim/src/`.

## Index

| Id | Concept | Basis |
|---|---|---|
| `C-1` to `C-11` | everything in [common-model.md](common-model.md), as it is there | see each entry |
| [D-1](#d-1-static-load-share-about-cog) | the weight is shared about `[cog]` | physics |
| [D-2](#d-2-the-tyres-of-a-section-behind-a-joint-steer) | the tyres of a section behind a joint steer | physics |
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

## D-2. The tyres of a section behind a joint steer

- **Inputs**: the section's `[rot_pnt_long]` (its turning line `y_turn`, `C-6`), `achse_long` (`y_i`),
  the curvature its joint gives it (`κ_follow`, `C-9`). **Outputs**: each axle's angle for the scripts
  and each tyre's own angle. **Module**: `physics/implementation/depot/follower.rs`, used by `body.rs`
  (`steer_to`) and `tyre.rs` (`lateral_start`).
- **Basis**: *physics*: an axle that steers towards the turning line, like the rear axle of an
  articulated bus with a steered last axle, rolls without scrubbing. **Status**: the rule is checked
  by the conformance tests; no claim is made here about OMSI 2 (the rule it replaces, `K-2`, is the
  one Classic keeps).

The tyres of a section behind a joint steer as its axles show it, each by the tyre angle of `C-6` with
`κ = κ_follow`, and each tyre is held across where it touches the road. A section with two axles then
settles short of the closed form of `C-9` in a steady turn (V5 of the conformance tests: 18.5° against
21.7° at half lock), as its tyres turn on `κ_follow`, which does not count the ball's own sideways
speed. A section with one axle on its turning line turns the same in both engines.

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

## Further `D-*`

None for now (decision D2 of the plan). A later override that its check in OMSI 2 shows is a
difference adds its own id here.
