# Vehicle physics models

Project Depot is getting two vehicle physics engines, **OMSI Classic** (the default) and **Project
Depot Physics**. This folder holds the logical-mathematical model of each: every equation the engine
uses, what it reads, what it produces, and which module of the code implements it. The equations are
what the automated tests check the code against, so a rule is never described only in a comment.

| Document | What is in it |
|---|---|
| [common-model.md](common-model.md) | the equations both engines share (`C-*`): units, time step, struts, drive and brakes, grip, steering, the body, joints, hits |
| [classic-model.md](classic-model.md) | OMSI Classic's own rules: the player's weight sharing (`K-*`) and the AI (`KA-*`) |
| [depot-model.md](depot-model.md) | Project Depot Physics' own rules: the player's weight sharing (`D-*`) and the AI (`DA-*`) |

## Where things stand

The code still has **one** rigid-body physics for the player's road vehicle, and its rules are the
Classic ones (the weight is shared about the model origin since #675), and **one** AI body, which is
the Depot AI. The two engines, the launcher setting and the `.bus` key described below arrive in the
steps of the plan in [`.ai/specs/002-physics-modes`](../../.ai/specs/002-physics-modes/plan.md); until
then these documents describe today's code under the engine it will belong to, and the module column
names today's files.

## What the two engines are

| Engine | Player's road vehicle | AI road vehicle |
|---|---|---|
| **OMSI Classic** (default) | the rigid body on spring struts, its weight shared about the model origin | a new AI body brought in line with OMSI 2's AI (`KA-*`) |
| **Project Depot Physics** | the same rigid body, with Depot's overrides; the first is the weight shared about `[cog]` (`D-1`) | today's AI body, unchanged (`DA-*`) |

Everything the engines do not override is shared (`C-*`). Rail vehicles and aircraft move the same in
both. Behaviour that only *adds* to OMSI 2 (articulated bus work, standing passengers, skip stop) stays
on in both engines.

## How a vehicle gets its engine

The engine of a vehicle is chosen once, when the vehicle appears, and never changes:

1. The launcher's **Physics** list (Settings, Simulation) holds the game's engine: `classic` (the
   default) or `depot`, saved as `physics=` in `settings.cfg`. A missing or unknown value is Classic.
2. A bus whose `.bus` file contains the line `[depot_physics]` (no value lines) runs Depot physics when
   **the player drives it**, whatever the setting says. AI copies of the same bus follow the setting.
3. There is no way to force Classic on a vehicle whose setting or file selects Depot, and no
   "automatic" mode.
4. In an articulated vehicle only the key in the front section's file counts, and it applies to every
   section.
5. The session log names the engine of each spawned vehicle and the reason.

The format of the key and of the setting is in the
[contracts of the plan](../../.ai/specs/002-physics-modes/contracts/).

## How to read an equation

Every equation has an **id** that never changes meaning. A rule that is corrected keeps its id, and the
section notes the change and the pull request.

| Prefix | Where | What |
|---|---|---|
| `C-n` | [common-model.md](common-model.md) | shared by both engines |
| `K-n` | [classic-model.md](classic-model.md) | OMSI Classic, the player's vehicle |
| `KA-n` | [classic-model.md](classic-model.md) | OMSI Classic, AI vehicles |
| `D-n` | [depot-model.md](depot-model.md) | Project Depot Physics, the player's vehicle |
| `DA-n` | [depot-model.md](depot-model.md) | Project Depot Physics, AI vehicles |

Each equation lists:

- **Inputs**: the `.bus` key (with its unit in the file) or the script variable it reads;
- **Outputs**: the state or script variable it produces;
- **Module**: the code that implements it;
- **Basis**: *OMSI* (the code takes it as OMSI 2's rule), *physics* (a physical constant or a
  textbook formula), or *tuned* (Depot's own numerical choice, shared by both engines and not claimed
  to be OMSI 2's);
- **Status**, for what the code takes as OMSI 2's: **validated** (the black-box test or in-game check,
  with its pull request) or **to validate** (the check that will settle it). Nothing here says that OMSI
  2 does something unless the status says it was validated.

Frames: body `x` right, `y` forward, `z` up, the model origin as in the `.bus` file; world `x` east,
`y` north. Units inside the model are SI. The unit of each `.bus` value is listed in `C-1`.

## How the tests use the ids

Each conformance test names the equations it checks in its doc comment, for example
`/// Checks: C-3, K-1`, and is named `<check>_<vehicle>`. The tests build five synthetic vehicles (a 4x2
bus, a 6x2 bus with a steered third axle, a 6x2 articulated bus, a bi-articulated bus and a 23 m
articulated 8x2 bus) and compare the engine with the closed forms of these documents. They are added
in the phases 2, 7 and 8 of the plan, in `crates/omsi-sim/tests/physics_model_classic.rs` and
`physics_model_depot.rs`. A check that every equation id is named by some test comes with them.
