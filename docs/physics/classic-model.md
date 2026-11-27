# OMSI Classic: the player's weight sharing (`K-*`) and the AI (`KA-*`)

OMSI Classic is the default engine. For the player's road vehicle it runs every shared rule of
[common-model.md](common-model.md) with rules of its own: `K-1` for the weight, `K-2` to `K-4`
for the joints of articulated vehicles. Its AI road vehicles run a body brought in line with OMSI 2's AI, `KA-1` to `KA-11`.

How to read an entry (inputs, outputs, module, basis, status) is explained in the [README](README.md).
Module paths are under `crates/omsi-sim/src/`.

Nothing below says that OMSI 2 does something unless the status says it was **validated** or
**confirmed**. A row marked **to validate** is built that way on purpose and corrected by a `fix/`
pull request once its check has been run (the plan's clarification of 2026-11-09).

## Index

| Id | Concept | Status |
|---|---|---|
| `C-1` to `C-12` | everything in [common-model.md](common-model.md), as it is there | see each entry |
| [K-1](#k-1-static-load-share-about-the-model-origin) | the weight is shared about the model origin | validated |
| [K-2](#k-2-a-section-behind-a-joint-put-back-on-it) | a section behind a joint is put back on it every frame | to validate |
| [K-3](#k-3-a-turntable-at-its-limit-lets-the-tyres-go) | a turntable at its limit lets the tyres of both sections go | to validate |
| [K-4](#k-4-the-curvature-of-a-section-behind-a-joint) | the curvature of a section behind a joint | to validate |
| [K-5](#k-5-the-payload-does-not-change-the-body-damping) | the payload does not change the body's damping | to validate |
| [KA-1](#ka-1-route-and-heading) | route following by a kinematic bicycle model | confirmed |
| [KA-2](#ka-2-speed-near-the-camera) | speed near the camera from the scripts' drive and brakes | confirmed |
| [KA-3](#ka-3-grip) | one grip value per vehicle | confirmed |
| [KA-4](#ka-4-no-body-dynamics-on-the-route) | no body dynamics on the route | confirmed |
| [KA-5](#ka-5-rear-sections-as-kinematic-trailers) | rear sections as kinematic trailers | confirmed |
| [KA-6](#ka-6-release-by-the-players-vehicle) | release by the player's vehicle, and return | confirmed (corrected) |
| [KA-6b](#ka-6b-release-and-return-of-the-whole-vehicle) | release and return act on the whole vehicle | confirmed |
| [KA-7](#ka-7-weight-sharing-only-while-loose) | weight shared only while loose | confirmed |
| [KA-8](#ka-8-contacts) | no contacts between vehicles on their routes | confirmed (corrected) |
| [KA-9](#ka-9-speed-beyond-the-detail-radius) | speed beyond the detail radius | confirmed |
| [KA-10](#ka-10-vehicle-serving-a-stop) | vehicle serving a stop | confirmed |
| [KA-11](#ka-11-a-loose-vehicle-goes-with-its-tile) | a loose vehicle goes with its tile | confirmed (corrected) |

The AI body (`ai/classic/`) runs every road vehicle of the AI traffic in OMSI Classic. Rail
vehicles and aircraft keep the Depot AI of [depot-model.md](depot-model.md) in both engines.

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

The Depot engine replaces this rule with `D-1`.

## K-2. A section behind a joint put back on it

- **Inputs**: the joint (`C-9`): its points, its type, `α_max`, `β_min`, `β_max`; the poses of the
  two sections after the frame. **Outputs**: the pose of the section behind. **Module**:
  `physics/implementation/classic/joint.rs` (`seat_followers`), called at the end of the step in
  `train.rs` through `Engine::after_step` (`physics/implementation/engine.rs`).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: drive an articulated bus over
  uneven ground and through a hard stop, in OMSI 2 and in the game, and watch the joint and the
  bellows: no gap opens at the joint and the sections do not lean against each other.

Besides the joint's constraint of `C-9`, at the end of every frame each section behind a joint is
put back on it, in order from the front, keeping its velocities:

- a turntable (any type but 0): the section takes the attitude of the section ahead turned by its
  own yaw and pitch against it, each held to the joint's range (`|α| ≤ α_max`,
  `β_min ≤ β ≤ β_max`), and no roll against it;
- a hitch (type 0): the section is swung about its `[rot_pnt_long]` point until its joint point
  lies on the line to the joint point of the section ahead;

then it is moved until the two joint points meet. The joint is closed after every frame.

## K-3. A turntable at its limit lets the tyres go

- **Inputs**: `α` and `α_max` of a turntable. **Outputs**: whether the tyres of the two sections
  hold (`C-7`). **Module**: `physics/implementation/classic/joint.rs` (`at_alpha_limit`), in
  `train.rs` before the tyres' hold.
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: an articulated bus at full lock at
  walking pace until the joint reaches its limit, in OMSI 2 and in the game: does the front
  section slide on instead of turning on?

While a turntable stands at `α_max` the tyres of both its sections let go (`C-7`), and hold again
by the rule of `C-7` once the joint has left the limit. A hitch has no limit.

## K-4. The curvature of a section behind a joint

- **Inputs**: `α`; the curvature `κ_lead` of the section ahead; `[rot_pnt_long]` of both
  sections (`r_lead`, `r_follow`, each the file's value) and the joint's place in each
  (`y_lead`, `y_follow`). **Outputs**: the curvature `κ_follow` the section behind turns on: its
  axles' angles `δ_i` for the scripts (`Axle_Steering`) and the bend's pull of its hold (`C-7`).
  **Module**: `physics/implementation/classic/joint.rs` (`follower_kappa`).
- **Basis**: *OMSI*. **Status**: **to validate**. **Check**: compare `Axle_Steering` of the
  rear section's axle of an articulated bus in a steady turn, in OMSI 2 and in the game.

The joint point moves the way the section ahead carries it, `atan((y_lead − r_lead) · κ_lead)`
from that section's axis, which is `α` more from the axis of the section behind. That section
turns about its own `[rot_pnt_long]` line so that its joint point moves that way:

```
κ_follow = tan( atan((y_lead − r_lead) · κ_lead) + α ) / (y_follow − r_follow)
```

The arm of the section ahead is counted from its file's `[rot_pnt_long]`, not from its turning line
(`C-6`, which adds `[cog]`): for a section ahead with a `[cog]` off the origin the curvature differs
from the one the section turns on by that much. The Depot engine keeps `sin α / |y_follow − r_follow|`
of `C-9`.

## K-5. The payload does not change the body damping

- **Inputs**: `[mass]`, `achse_feder` (`k_i`), the people aboard. **Outputs**: `f_body`, the
  frequency in OMSI's damping of the body's pitch and roll (`C-8`). **Module**:
  `physics/implementation/classic/load_share.rs` (`loaded_body_freq`), through `set_payload`.
- **Basis**: *OMSI*. **Status**: **to validate**: the decay of the body's pitch after a kerb drop,
  empty and full, in OMSI 2 and in the game.

Classic takes `f_body` of `C-8` with the empty `[mass]` whatever the people aboard weigh: a full bus
is damped as the empty one is. The people still weigh in the gravity, the springs, the tyres and the
acceleration. Depot follows the heavier body instead (`D-4`).

## The AI road vehicle

An AI road vehicle has two physical states, shared by all its sections: **on its route** and
**loose** (after a crash). On its route it is not a rigid body. Loose, it is the player's rigid body
(`K-1` and the `C-*`), without a driver, until it is cleared (`KA-6`) or goes with its tile (`KA-11`).

The traffic planner, the AI driver's target-speed decisions and the rail and air followers are shared
and unchanged. In the rules below, `L` is the length of the kinematic bicycle (from the point at
`[rot_pnt_long]` to the front axle), `κ` the curvature of the lane path, `κ_v` the
curvature of the vehicle, `y_i` an axle's place along the body and `r` is `[rot_pnt_long]`.

Module for all the `KA-*` rules: `ai/classic/` (new, phase 8), using `physics/` for the loose state.

## KA-1. Route and heading

- **Inputs**: the lane path (planner), `[rot_pnt_long]`, `achse_long`, the lane change's sideways shift.
  **Outputs**: the pose of the leading section and each axle's steering angle. **Module**:
  `ai/classic/` (route and pose, phase 8).
- **Basis**: *OMSI*. **Status**: **to validate**. The maintainer's check of 2026-11-11 confirmed the
  heading equation; on 2026-11-26 the maintainer saw long AI buses swing their rear out of bends as
  if on a rail, with the rotation point held on the lane. The equation is the one of a trailer, so
  the point on the lane is now the front axle and the rotation point trails it.
  **Check**: film an AI bus from above through a tight junction turn and a lane change; the front
  axle stays on the lane while the rear cuts the inside of the bend.

The front axle moves along the lane path, shifted sideways during a lane change. The body's axis
lags the path's tangent there by `ψ_r`, a state of its own: the axis follows the path as a trailer
of length `L` pulled by the front axle, the point at `[rot_pnt_long]` not slipping sideways. Over a
step `Δs = v · dt` along the path, with `κ` taken at the front axle:

```
Δθ   = Δs · κ                                     (the path's turn)
ψ_r ← ψ_r + Δθ - (Δs / L) · sin(ψ_r + Δθ / 2)
κ_v  = sin ψ_r / L
δ_i  = atan((y_i - r) · κ_v)                      (each axle, with κ_v of the frame before)
```

In a steady bend `sin ψ_r = L · κ`: the front axle stays on the lane and the rotation point runs
on the inner circle of radius `sqrt(R² − L²)`, as a real vehicle's rear axle does. A vehicle put down
on a bend starts in that steady state. In a bend tighter than `L` there is no steady state, and
`ψ_r` stops at 90°.

## KA-2. Speed near the camera

- **Inputs**: the scripts' `M_Wheel` at the first axle's radius of each section, `[rollwiderstand]`,
  every wheel's brake forces, the masses of all sections with their passengers, the target speed of
  the driver. **Outputs**: the leading section's speed; the throttle and brake the scripts read.
  **Module**: `ai/classic/` (speed, phase 8).
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11): the
  vehicle's scripts give the drive force. **Check**: halve the engine torque of an AI-only copy of a
  bus and time its pull-away next to the camera.

Within the detail radius (`KA-9`) the leading section integrates its speed from the drive force
`F = 2·M_Wheel / r` (`r`: the first axle's radius), the resistance `R` (every section's
`[rollwiderstand]` and every wheel's brake forces) and the mass `M` of all sections with their
passengers:

- standing, `F_n = F − sgn(F)·R`, and `F_n = 0` when that turns its sign (the brakes hold);
- rolling, `F_n = F − sgn(v)·R`;
- `Δv = dt·F_n / M`, bounded by `±g·μ` per frame (`KA-3`), and `v = 0` when `v + Δv` would change
  its sign.

The other sections take the leading section's speed.

The AI driver turns the ratio `r = v / v_t` of the speed to the target speed into throttle and
brake for the scripts' next frame: brake `clamp(2·(r − 1.05), 0, 0.7)`, and 1 when `r > 2`; with the
brake off, the throttle goes towards `clamp(10·(1 − r), 0, 1)`, rising by at most 1 per second;
asked to stop (`v_t = 0`), throttle 0 and brake 1. The traffic planner shared by both engines decides
an acceleration `a`, not a speed: the target speed is `v_t = max(0, v + a·1 s)` (a choice of this
code, **to validate** with the rest of the row).

## KA-3. Grip

- **Inputs**: a fixed road surface and the weather (wet, snow, ice). **Outputs**: one `μ` per
  vehicle, bounding the AI's acceleration, braking and comfortable braking. **Module**: `ai/classic/`
  (phase 8); the value from `vehicle/weather.rs` (`route_grip`).
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11): the
  grip does not change with the surface under the wheels. **Check**: an AI bus pulling away and
  stopping on a wet and a dry road, and on a gravel patch.

The grip is not read from the surface under each wheel, as in `C-5` for the player's vehicle: it is
`C-5`'s grip of a plain asphalt road that gets wet (ground byte `0x20`, a choice of this code) in
the current weather. It bounds the change of speed of a
frame (`KA-2`); the comfortable braking stays the shared traffic planner's.

## KA-4. No body dynamics on the route

- **Inputs**: the path's geometry; the static sag of `K-1`. **Outputs**: the vehicle's height and
  attitude, and the `Axle_Suspension` the scripts read. **Module**: `ai/classic/` (phase 8).
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11): no
  inertia, no suspension dynamics. **Check**: watch an AI bus braking hard and in a bend; the body
  neither pitches nor rolls.

On its route the vehicle has no springs, no settling and no dynamic pitch or roll: its height and
attitude come from the path. The spring travel the scripts read is the static sag of `K-1`
(about the model origin) computed **once at the vehicle's spawn** (maintainer's choice, 2026-11-09).

## KA-5. Rear sections as kinematic trailers

- **Inputs**: the front section's curvature, the joint and rotation-line distances. **Outputs**: the
  articulation angle and the pose of each rear section. **Module**: `ai/classic/` (rear sections,
  phase 8).
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11).
  **Check**: film an articulated AI bus through a junction turn; the rear section cuts inside the
  lane.

The articulation angle is integrated every frame from the front section's curvature and the
distances of the joint and of the rotation line, so a rear section cuts a bend as a real trailer does.
Beyond the detail radius the rear sections' poses are not recalculated (`KA-9`).

## KA-6. Release by the player's vehicle

- **Inputs**: contacts of any section of the AI vehicle with the player's vehicle; the vehicle
  type. **Outputs**: the vehicle's state (on its route or
  loose). **Module**: `ai/classic/` (release, phase 8); the player's body for the loose state.
- **Basis**: *OMSI*. **Status**: **confirmed** as written here by the maintainer's in-game check
  (2026-11-11), which found that an AI vehicle the player pushes into another one does not touch it
  (the row first said any active body released it, in a chain). **Check**: nudge an AI car at walking pace with
  the player's bus, then push it into a second AI car.

Contact with the player's vehicle on any section releases the AI vehicle from its route, unless its
type is `[fixed]`. A loose vehicle releases no other: it does not touch the AI vehicles on their
routes (`KA-8`). Loose, it runs the player's rigid body
without a driver, fully braked with its hazard lights on, and never falls asleep for standing still.

It goes back to its route when the player clears the crash from the Alt menu ("Clear the accident",
enabled only while a vehicle is loose); left loose, it goes with its tile (`KA-11`). In this code it is put back, standing, where
its driver left the route, and drives on from there. An AI bus goes back empty and starts the next trip of its
timetable (maintainer, 2026-11-09). Crashes have no effect on the career.

## KA-6b. Release and return of the whole vehicle

- **Inputs / outputs**: the poses and articulation angles of all sections. **Module**: `ai/classic/`
  (phase 8).
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11).
  **Check**: release an articulated AI bus in a bend; no section jumps at the moment of the hit.

Release and return act on all sections at once. Released, the sections become the player's multibody
train (rigid bodies on joints), starting from their poses and articulation angles on the route with
no jump. Back on the route, every section is put back on it.

## KA-7. Weight sharing only while loose

- **Inputs / outputs**: as `K-1`. **Module**: `ai/classic/` (phase 8), `physics/` for the loose state.
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11).
  **Check**: an AI copy of the Mi5 at a stop next to the player's Mi5; its height follows the path,
  not its springs.

On its route no weight is shared between axles in the motion (`KA-4`), only the static sag the
scripts read. The weight is shared, about the model origin (`K-1`), only while the vehicle is loose.

## KA-8. Contacts

- **Inputs**: the bodies' boxes, the scenery. **Outputs**: release by the player's vehicle (`KA-6`)
  or an ordinary hit (`C-10`). **Module**: `ai/classic/` (contacts, phase 8), `physics/implementation/collide.rs`.
- **Basis**: *OMSI*. **Status**: **confirmed** as written here by the maintainer's in-game check
  (2026-11-11), which found that a loose AI vehicle does not touch the AI vehicles on their routes
  (the row first said it collided with everything). **Check**: two AI cars forced into the same spot (a blocked junction) overlap rather
  than push; a loose AI car pushed against a wall stops at it.

On their routes, AI vehicles collide neither with each other nor with static scenery (walls, posts,
buildings): they only avoid obstacles by the driver's checks. An AI vehicle on its route collides
only with the player's vehicle (`KA-6`). A loose AI vehicle collides with the scenery, the player's
vehicle and the other loose vehicles, and passes through the AI vehicles on their routes without
releasing them.

In this code the player's vehicle meets the AI vehicles' boxes as moving obstacles, and an AI vehicle
it touched is released at the next frame. A loose vehicle meets the scenery's solids, the player's
vehicle and the boxes of the other loose vehicles within 80 m.

## KA-9. Speed beyond the detail radius

- **Inputs**: the tile of the vehicle, the camera's tile, `[noDistanceCheck]`, the driver's target
  speed. **Outputs**: the speed; the rear sections' poses are left as they are. **Module**:
  `ai/classic/` (speed, phase 8).
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11).
  **Check**: watch an AI bus leave and re-enter the detail radius; far away it keeps its target
  speed exactly.

The detail radius is the camera's tile and its eight neighbours (centred on the camera, never on the
player's vehicle). A vehicle is near while its tile is in it. Beyond it the vehicle's speed equals the
driver's target speed, with no force integration. The model key `[noDistanceCheck]` makes a vehicle
count as near in both engines.

## KA-10. Vehicle serving a stop

- **Inputs / outputs**: as `KA-2`. **Module**: `ai/classic/` (speed, phase 8).
- **Basis**: *OMSI*. **Status**: **confirmed** by the maintainer's in-game check (2026-11-11).
  **Check**: an AI bus at a stop far from the camera on a grade; the scripts' brakes hold it.

While the vehicle serves a stop (doors open until the scripts close them), its speed always comes
from its drive and brakes (`KA-2`), even beyond the detail radius.

## KA-11. A loose vehicle goes with its tile

- **Inputs**: the tile of a loose vehicle. **Outputs**: the vehicle removed; a warning to the
  player. **Module**: the traffic's removal of vehicles over unloaded tiles (`omsi-app`).
- **Basis**: *OMSI*. **Status**: **confirmed** as written here by the maintainer's in-game check
  (2026-11-11), which found that the loose vehicle disappears when its tile is unloaded, with a
  warning that the player left the accident without calling the police (the row first said it went
  back to its route once beyond the detail radius). **Check**: knock an AI car loose and drive away
  until its tile is unloaded; it disappears and the warning shows.

A loose vehicle is never put back on its route by distance: it stays where it lies, still simulated,
beyond the detail radius too, until the player clears the accident (`KA-6`) or its tile is unloaded.
Then it disappears with the tile, as every vehicle over unloaded ground does, and the player is told
"You left the scene of the accident without calling the police". A loose vehicle never goes to sleep
out of range as a random car on its way does. There is no limit on the number of loose vehicles.

## Open items

Behaviours of the player's body that may differ from OMSI 2 and are kept shared until a check says
otherwise. Each is marked **to validate** and decided later (the plan's decision D2):

| Item | Behaviour | Check in OMSI 2 |
|---|---|---|
| V3 | air drag in `C-8` | coast down a long 6-8 % grade without brakes: does the speed level off, and where? |
| V4 | tyre envelope and kerb climbing in `C-3` | drive over a 15 cm kerb at 10 km/h at 45° and square on |
| V5 | wheels stopped at wall faces in `C-3` | drive a wheel slowly into a wall face |
| — | the articulated stop wave of `C-9` | a known difference, left as it is (maintainer, 2026-10-23) |

## Known divergences

Rules the Classic engine runs that are believed to differ from OMSI 2. They stay as they are until
the engines are split (plan phases 5 and 6); each is then resolved by a `fix/` pull request, with
the OMSI 2 check below settling what the Classic rule must be.

| Id | What the engine does | What is believed of OMSI 2 | Check that settles it |
|---|---|---|---|
| X-1 | Articulated vehicles: the joint carries part of a section's weight to its neighbour, and a joint's load turns a section about `[cog]` while its struts turn it about the origin (`K-1`, `C-9`). Found by the conformance tests of the articulated vehicles | Each section's springs may carry that section's own weight alone, with no weight passed through the joint. Not confirmed: how OMSI 2 poses a section behind a joint is still to be found | an articulated bus whose rear section has its axle off the model origin and a very soft rear spring: does the front section sink when the rear is coupled? |
