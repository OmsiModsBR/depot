# Contract: the logical-mathematical model documents

Delivered in phase 1 under `docs/physics/` (public, English, Principle VIII). Each equation has an id
that conformance tests name in their doc comment (`/// Checks: C-3, K-1`). An id is never reused;
an equation that changes keeps its id and the document notes the change and its pull request.

## Common rules for every document

- Frames: body x right, y forward, z up; model origin as in the `.bus` file; world x east, y north.
- Units: SI inside the model; each `.bus` value is listed with its unit in the file and the
  conversion (`[mass]` t → kg, `achse_feder` kN/m → N/m, `achse_maxforce` kN → N, `[momentofintertia]`
  t·m² → kg·m²).
- Every equation states its inputs (which `.bus` key or script variable), its output (which state or
  script variable) and the code module that implements it.
- Classic rules carry one of: **validated** (the black-box test or in-game check, with its date and
  issue) or **to validate** (the check that will settle it). No text says "as OMSI 2 does" without the
  first.
- No addresses, internal names or thresholds of the reference executable.

## `common-model.md`: shared by both engines (C-*)

| Id | Concept | Content |
|---|---|---|
| C-1 | `.bus` to model | every physics key read, unit, default when missing (OMSI 2's own defaults only, Principle III) |
| C-2 | Time integration | fixed substep `h = 1/240 s` with the leftover carried to the next frame; slices of at most 0.05 s answering obstacles; frames longer than 0.25 s cut; semi-implicit Euler per substep |
| C-3 | Strut | spring point on the origin plane under the wheel; `t` = ground over that point; `N = min(k·s_f·t - c·v_up, F_max)` when `k·t ≥ 0`, else 0; `Axle_Suspension = clamp(t, -droop, F_max/k)` |
| C-4 | Drive and brakes | `F_drive = M_Wheel / (r · n_driven)` per driven wheel; brake forces per wheel from `Brakeforce` and `Axle_Brakeforce_*`, cleared each frame; rolling resistance `[rollwiderstand]` shared over the wheels |
| C-5 | Grip | `μ` of the road under the vehicle; a tyre carries at most `μ · min(N, F_max)` |
| C-6 | Steering geometry | turning line at `y_turn = cog_y + [rot_pnt_long]`; curvature `κ`, `|κ| ≤ [inv_min_turnradius]`; each axle `δ_i = atan((y_i - y_turn) · κ)` |
| C-7 | Adhering and sliding | adhering: the body turns about the turning line, `ω_z = v · κ`; released when `m v² |κ| > Σ μ N` or a side knock; sliding: the tyres' forces at `μ N` |
| C-8 | Body | Newton-Euler with mass `[mass]`, inertia `[momentofintertia]` (first, second, third value = pitch, roll, yaw), centre `([cog] x, [cog] y, [schwerpunkt])`; gravity; pitch and roll damped by `(sin x / x)^(dt / (1/30 s))`, `x = 1.5 · f_body / 30`; air drag while shared (R-V V3) |
| C-9 | Joints | ball joint at `[coupling_front]`/`[coupling_back]`; limits from `[coupling_front_character]` when present, free otherwise; the steady-turn articulation angle used by the tests |
| C-10 | Hits | impulse with restitution 0.2 and scrape friction 0.4 against obstacles; a hit releases adhesion |
| C-11 | Kinematic model and rail/air followers | unchanged by the engine; written here for completeness |

## `classic-model.md`: OMSI Classic (K-* player, KA-* AI)

- **Index**: every C-* the engine runs, then its own ids.
- **K-1 Static load share about the model origin**: the strut force turns the body about the origin,
  so at rest `Σ F_i = m g` and `Σ F_i · y_i = 0`; with springs, `F_i = k_i (z - θ y_i)` gives one
  `(z, θ)` and the sag per axle. Validated: #675 reasoning; in-game check of the Mi5 (Q3) pending.
- **KA-1 .. KA-7**: the Classic AI rows of [research.md](../research.md) R-A, each written as an
  equation once its check confirms it (path following, speed from drive and brakes, grip, pose,
  rear sections, release after a hit, static share about the origin). Until then the row stays in
  the document as "to validate" with no equation.
- **KA-6** is validated by the maintainer's in-game observation (2026-11-08): a hit by the player makes
  the AI vehicle a free rigid body, without a driver, until it is cleared from the scene.
- **Open items**: the R-V candidates V3 and V4 while unchecked.

## `depot-model.md`: Project Depot Physics (D-* player, DA-* AI)

- **Index**: every C-* the engine runs, then its own ids.
- **D-1 Static load share about `[cog]`**: `Σ F_i = m g`, `Σ F_i · (y_i - cog_y) = 0`; why (a fuel
  tank or the engine's place changes each axle's load without changing the geometry) and what it
  means for a mod tuned for Classic (springs sized for the origin rule stand differently).
- **DA-1 Path follower**: rotation point led by the planner's position, `v = clamp(v_plan + 1.5 ·
  along, 0, v_plan + 3)`.
- **DA-2 Pure pursuit**: look-ahead `L = max(1.2 · wb, 3.5) + 0.6 · min(v, 20)`; `δ_want =
  atan(2 · wb · sin α / reach)`; lock `max(lock_model, min(1.15 · atan(κ_bend · wb), 60°))` (#249);
  lateral limit `atan(20 · wb / v²)`; rate limit (25 or 40 °/s by mass) and 0.08 s smoothing.
- **DA-3 Heading**: `dψ = v · dt · tan δ / wb`, midpoint integration of the rotation point.
- **DA-4 Ground plane**: least-squares plane `h = a + b · long + c · lat` through the wheels' contact
  heights (each wheel's own face, the 3 m ray and the 0.3 m drop rule).
- **DA-5 Sprung body**: heave, pitch and roll springs and dampers about that plane, 4 substeps,
  pitch driven by `m · a_long · h_cog`, roll by `-m · a_lat · h_cog`; clamps ±0.15 m, ±0.07 rad,
  ±0.09 rad.
- **DA-6 Static pose**: `[ai_deltaheight]` plus the sag of the weight shared about `[cog]`.
- **Further D-***: none for now (D2); a later override adds its id here.

## `README.md`

What the two engines are, how a vehicle gets one (setting, `[depot_physics]`, the log line), the id
scheme, and how to run the conformance tests.
