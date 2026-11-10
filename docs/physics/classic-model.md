# OMSI Classic: the player's weight sharing (`K-*`) and the AI (`KA-*`)

OMSI Classic is the default engine. For the player's road vehicle it runs every shared rule of
[common-model.md](common-model.md) with one rule of its own, `K-1`. Its AI road vehicles run a body
brought in line with OMSI 2's AI, `KA-1` to `KA-11`.

How to read an entry (inputs, outputs, module, basis, status) is explained in the [README](README.md).
Module paths are under `crates/omsi-sim/src/`.

Nothing below says that OMSI 2 does something unless the status says it was **validated**. A row marked
**to validate** is built that way on purpose and corrected by a `fix/` pull request once its check has
been run (the plan's clarification of 2026-11-09).

## Index

| Id | Concept | Status |
|---|---|---|
| `C-1` to `C-11` | everything in [common-model.md](common-model.md), as it is there | see each entry |
| [K-1](#k-1-static-load-share-about-the-model-origin) | the weight is shared about the model origin | validated |
| [KA-1](#ka-1-route-and-heading) | route following by a kinematic bicycle model | to validate |
| [KA-2](#ka-2-speed-near-the-camera) | speed near the camera from the scripts' drive and brakes | to validate |
| [KA-3](#ka-3-grip) | one grip value per vehicle | to validate |
| [KA-4](#ka-4-no-body-dynamics-on-the-route) | no body dynamics on the route | to validate |
| [KA-5](#ka-5-rear-sections-as-kinematic-trailers) | rear sections as kinematic trailers | to validate |
| [KA-6](#ka-6-release-by-an-active-body) | release by any active body, and return | validated in part |
| [KA-6b](#ka-6b-release-and-return-of-the-whole-vehicle) | release and return act on the whole vehicle | to validate |
| [KA-7](#ka-7-weight-sharing-only-while-loose) | weight shared only while loose | to validate |
| [KA-8](#ka-8-contacts) | no contacts between vehicles on their routes | to validate |
| [KA-9](#ka-9-speed-beyond-the-detail-radius) | speed beyond the detail radius | decided; details to validate |
| [KA-10](#ka-10-vehicle-serving-a-stop) | vehicle serving a stop | to validate |
| [KA-11](#ka-11-automatic-clearing) | automatic clearing of a loose vehicle | to validate |

The AI body is delivered in plan phase 8 (`feat/classic-ai`). Until then the AI of the code is the
Depot AI of [depot-model.md](depot-model.md) in both engines.

## K-1. Static load share about the model origin

- **Inputs**: `[mass]`, `achse_long` (`y_i`), `achse_feder` (`k_i`), `achse_maxforce` (`F_max,i`).
  **Outputs**: the rest compression of every strut (the spring travel the scripts read in
  `Axle_Suspension`) and the body's rest height and pitch. **Module**:
  `physics/implementation/suspension.rs` (the `cog x F` term of the load reference, `C-3`), `physics/implementation/spec.rs` (`wheel_rest_loads`, the
  centre `0`).
- **Basis**: *OMSI*. **Status**: **validated**: the reasoning and the check of #675 (merged
  2026-11-04), in which the Mi5 articulated bus stands level in OMSI 2, with the sag 9.1 / 9.8 / 9.1 cm per
  axle. The maintainer also compared the Mi5 at rest in OMSI 2 and in the game, which is how the
  difference in where `[cog]` acts was found, before spec 002 (quickstart Q3).

Classic adds the term `(cog_y · N, −cog_x · N, 0)` of `C-3` to the torque about the centre of gravity,
so the struts turn the body as if they acted about the model origin: the load is shared about the
origin, wherever `[cog]` is. At rest, with `W = m g`:

```
Σ F_i       = W
Σ F_i · y_i = 0
```

With springs, `F_i = k_i · s_i` and the compression `s_i = z - θ · y_i` for a body sunk by `z` and
pitched by `θ` (nose down is positive). Let `S0 = Σ k_i`, `S1 = Σ k_i · y_i`, `S2 = Σ k_i · y_i²`.
Then:

```
z = W · S2 / (S0 · S2 - S1²)
θ = S1 · W / (S0 · S2 - S1²)
s_i = z - θ · y_i
```

This holds while every spring is pressed and below its `F_max,i`. For three or more axles `θ` and
`z` are the least-squares share of the spring rates, the same one used to place the vehicle at rest.
A vehicle whose springs and geometry are symmetric about the origin stands level whatever `[cog]` says,
and the compression of axle `i` does not depend on `[cog]`. `[cog]` still sets the turning line
(`C-6`), the centre the body turns about (`C-8`) and the lever of the load shifts in braking and
cornering.

The Depot engine replaces only this rule, with `D-1`.

## The AI road vehicle

An AI road vehicle has two physical states, shared by all its sections: **on its route** and
**loose** (after a crash). On its route it is not a rigid body. Loose, it is the player's rigid body
(`K-1` and the `C-*`), without a driver, until it is cleared (`KA-6`, `KA-11`).

The traffic planner, the AI driver's target-speed decisions and the rail and air followers are shared
and unchanged. In the rules below, `L` is the length of the kinematic bicycle (from the point at
`[rot_pnt_long]` to the front of the vehicle), `κ` the curvature of the lane path, `κ_v` the
curvature of the vehicle, `y_i` an axle's place along the body and `r` is `[rot_pnt_long]`.

Module for all the `KA-*` rules: `ai/classic/` (new, phase 8), using `physics/` for the loose state.

## KA-1. Route and heading

- **Inputs**: the lane path (planner), `[rot_pnt_long]`, `achse_long`, the lane change's sideways shift.
  **Outputs**: the pose of the leading section and each axle's steering angle. **Module**:
  `ai/classic/` (route and pose, phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: film an AI bus from above through a tight
  junction turn and a lane change; the rotation point stays on the lane while the front swings out.

The point at `[rot_pnt_long]` moves along the lane path, shifted sideways during a lane change. The
heading relative to the path, `ψ_r`, is a state of its own, integrated by a kinematic bicycle model:

```
sin ψ_r = L · κ                      (steady bend: the axis is turned in, not on the tangent)
δ_i     = atan((y_i - r) · κ_v)      (each axle)
```

## KA-2. Speed near the camera

- **Inputs**: the scripts' `M_Wheel` at the first axle's radius of each section, `[rollwiderstand]`,
  every wheel's brake forces, the masses of all sections with their passengers, the target speed of
  the driver. **Outputs**: the leading section's speed; the throttle and brake the scripts read.
  **Module**: `ai/classic/` (speed, phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: halve the engine torque of an AI-only
  copy of a bus and time its pull-away next to the camera.

Within the detail radius (`KA-9`) the leading section integrates its speed from the drive force
`Σ M_Wheel / r`, the rolling resistance and the brake forces over the mass of all sections. The AI
driver turns the ratio of speed to target speed into throttle and brake for the scripts. The other
sections take the leading section's speed.

## KA-3. Grip

- **Inputs**: a fixed road surface and the weather (wet, snow, ice). **Outputs**: one `μ` per
  vehicle, bounding the AI's acceleration, braking and comfortable braking. **Module**: `ai/classic/`
  (phase 8), the values of `C-5`.
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: an AI bus pulling away and stopping on a
  wet and a dry road, and on a gravel patch.

The grip is not read from the surface under each wheel, as in `C-5` for the player's vehicle.

## KA-4. No body dynamics on the route

- **Inputs**: the path's geometry; the static sag of `K-1`. **Outputs**: the vehicle's height and
  attitude, and the `Axle_Suspension` the scripts read. **Module**: `ai/classic/` (phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: watch an AI bus braking hard and in a
  bend; the body neither pitches nor rolls.

On its route the vehicle has no springs, no settling and no dynamic pitch or roll: its height and
attitude come from the path. The spring travel the scripts read is the static sag of `K-1`
(about the model origin) computed **once at the vehicle's spawn** (maintainer's choice, 2026-11-09).

## KA-5. Rear sections as kinematic trailers

- **Inputs**: the front section's curvature, the joint and rotation-line distances. **Outputs**: the
  articulation angle and the pose of each rear section. **Module**: `ai/classic/` (rear sections,
  phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: film an articulated AI bus through a
  junction turn; the rear section cuts inside the lane.

The articulation angle is integrated every frame from the front section's curvature and the
distances of the joint and of the rotation line, so a rear section cuts a bend as a real trailer does.
Beyond the detail radius the rear sections' poses are not recalculated (`KA-9`).

## KA-6. Release by an active body

- **Inputs**: contacts of any section of the AI vehicle with any active body (the player's vehicle,
  an AI vehicle already loose); the vehicle type. **Outputs**: the vehicle's state (on its route or
  loose). **Module**: `ai/classic/` (release, phase 8); the player's body for the loose state.
- **Basis**: *OMSI*. **Status**: **validated in part**: the maintainer's in-game observation
  (2026-11-08) that an AI vehicle hit by the player becomes a free rigid body, without a driver, until
  it is cleared. The rest is **to validate**. **Check**: nudge an AI car at walking pace with the
  player's bus, then push it into a second AI car.

Contact with any active body on any section releases the AI vehicle from its route, unless its type
is `[fixed]`. A loose vehicle can release another (a chain). Loose, it runs the player's rigid body
without a driver, fully braked with its hazard lights on, and never falls asleep for standing still.

It goes back to its route, asking for a new path, when the player clears the crash from the Alt menu
("Clear the accident") or by `KA-11`. An AI bus goes back empty and starts the next trip of its
timetable (maintainer, 2026-11-09). Crashes have no effect on the career.

## KA-6b. Release and return of the whole vehicle

- **Inputs / outputs**: the poses and articulation angles of all sections. **Module**: `ai/classic/`
  (phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: release an articulated AI bus in a bend;
  no section jumps at the moment of the hit.

Release and return act on all sections at once. Released, the sections become the player's multibody
train (rigid bodies on joints), starting from their poses and articulation angles on the route with
no jump. Back on the route, every section is put back on it.

## KA-7. Weight sharing only while loose

- **Inputs / outputs**: as `K-1`. **Module**: `ai/classic/` (phase 8), `physics/` for the loose state.
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: an AI copy of the Mi5 at a stop next to
  the player's Mi5; its height follows the path, not its springs.

On its route no weight is shared between axles in the motion (`KA-4`), only the static sag the
scripts read. The weight is shared, about the model origin (`K-1`), only while the vehicle is loose.

## KA-8. Contacts

- **Inputs**: the bodies' boxes, the scenery. **Outputs**: release (`KA-6`) or an ordinary hit
  (`C-10`). **Module**: `ai/classic/` (contacts, phase 8), `physics/implementation/collide.rs`.
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: two AI cars forced into the same spot
  (a blocked junction) overlap rather than push; a loose AI car pushed against a wall stops at it.

On their routes, AI vehicles collide neither with each other nor with static scenery (walls, posts,
buildings): they only avoid obstacles by the driver's checks. An AI vehicle on its route collides
only with active bodies (`KA-6`). A loose AI vehicle is an ordinary body: it collides with everything
and releases any AI vehicle it touches.

## KA-9. Speed beyond the detail radius

- **Inputs**: the tile of the vehicle, the camera's tile, `[noDistanceCheck]`, the driver's target
  speed. **Outputs**: the speed; the rear sections' poses are left as they are. **Module**:
  `ai/classic/` (speed, phase 8).
- **Basis**: *OMSI*. **Status**: **decided** by the maintainer (2026-11-08): the Classic AI far from
  the camera behaves as OMSI 2's; the details are **to validate**. **Check**: watch an AI bus leave
  and re-enter the detail radius; far away it keeps its target speed exactly.

The detail radius is the camera's tile and its eight neighbours (centred on the camera, never on the
player's vehicle). A vehicle is near while its tile is in it. Beyond it the vehicle's speed equals the
driver's target speed, with no force integration. The model key `[noDistanceCheck]` makes a vehicle
count as near in both engines.

## KA-10. Vehicle serving a stop

- **Inputs / outputs**: as `KA-2`. **Module**: `ai/classic/` (speed, phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: an AI bus at a stop far from the camera
  on a grade; the scripts' brakes hold it.

While the vehicle serves a stop (doors open until the scripts close them), its speed always comes
from its drive and brakes (`KA-2`), even beyond the detail radius.

## KA-11. Automatic clearing

- **Inputs**: the tile of a loose vehicle. **Outputs**: the state of every loose vehicle. **Module**:
  `ai/classic/` (release, phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: knock an AI car loose, drive away until it
  is out of the detail radius, come back; it drives on its route again.

A loose vehicle is cleared automatically once its tile is beyond the detail radius; every loose
vehicle then goes back to its route (`KA-6`). There is no limit on the number of loose vehicles.

## Open items

Behaviours of the player's body that may differ from OMSI 2 and are kept shared until a check says
otherwise. Each is marked **to validate** and decided later (the plan's decision D2):

| Item | Behaviour | Check in OMSI 2 |
|---|---|---|
| V3 | air drag in `C-8` | coast down a long 6-8 % grade without brakes: does the speed level off, and where? |
| V4 | tyre envelope and kerb climbing in `C-3` | drive over a 15 cm kerb at 10 km/h at 45° and square on |
| V5 | wheels stopped at wall faces in `C-3` | drive a wheel slowly into a wall face |
| — | the articulated stop wave of `C-9` | a known difference, left as it is (maintainer, 2026-10-23) |
| — | the steady articulation angle of a section behind a joint with two axles (V5's rear section): the engine settles at about 85 % of the closed form of `C-9` (18.5° against 21.7° at half lock), while one-axle sections match it within 0.05° | the same section with only its first axle matches the closed form, so the second axle's tyres are what hold it back. The maintainer notes that in OMSI 2 the steering of such a rear axle is only animated by the vehicle's scripts and does not act on the turn (2026-11-10, not yet checked in the game); the fix comes with a test that fails first (T017) |

## Known divergences

Rules the Classic engine runs that are believed to differ from OMSI 2. They stay as they are until
the engines are split (plan phases 5 and 6); each is then resolved by a `fix/` pull request, with
the OMSI 2 check below settling what the Classic rule must be.

| Id | What the engine does | What is believed of OMSI 2 | Check that settles it |
|---|---|---|---|
| X-1 | Articulated vehicles: the joint carries part of a section's weight to its neighbour, and a joint's load turns a section about `[cog]` while its struts turn it about the origin (`K-1`, `C-9`). Found by the conformance tests of the articulated vehicles | Each section's springs may carry that section's own weight alone, with no weight passed through the joint. Not confirmed: how OMSI 2 poses a section behind a joint is still to be found | an articulated bus whose rear section has its axle off the model origin and a very soft rear spring: does the front section sink when the rear is coupled? |
