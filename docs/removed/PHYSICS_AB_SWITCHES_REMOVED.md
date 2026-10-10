# Vehicle physics A/B switches: removed

Status: **removed** (decision by the maintainer, 2026-10-10).

## What was removed

`crates/omsi-sim/src/rigid/toggles.rs` and the three environment variables it read, each of which
brought an older model of the rigid-body physics back for comparison:

* `OMSI_TYRE_SUSPENSION=1`: the suspension with a wheel of its own mass between a tyre and the
  strut, instead of OMSI's spring on the ground under the wheel. With it went its digressive damper,
  its bump and rebound stops, the limit on how fast the ground under a tyre may rise, the damper's
  roll term while the tyre carried, and the hub lift of a kerb under part of the tread
  (`ground::tread_step`, which only that model read). Removed with it: the 16 constants of its
  section in `rigid/params.rs` (`UNSPRUNG` to `BUMP_TRAVEL`), `TREAD_STEP_MIN`, `TREAD_HALVINGS`, and
  the fields `tyre_k` and `tyre_c` of `RigidWheel`. That model also kept OMSI's holding state across
  the tyres with a wheel in the air.
* `OMSI_NO_WHEEL_SLIP=1`: every wheel gripping, as before the wheels had a turning of their own
  (its branch in `RigidWheel::advance_spin` and for a wheel off the ground).
* `OMSI_KINEMATIC_TRAILERS=1`: the coupled parts behind the player's vehicle following
  kinematically, as before every section was a body of its own (its condition in
  `VehicleInstance::part_bodies_wanted`).

`scripts/physics-trace-diff.sh` no longer runs the scenarios once with each switch on: it runs
them once a side.

## What stays

Everything the game does without the switches, which is what it did by default: OMSI's
suspension, the wheels' own turning (gripping, slipping, locked) and every section of the player's
articulated bus or lorry as a body. The physics traces of the change are identical to those of its
parent, bit for bit. The AI's vehicles, other players' over the LAN and trains on rails still
follow kinematically; that never depended on a switch. `BUMP` stays: it is the most a spring is
pressed when its rate does not say, and the cap of the ride height's sag.

## Why

The switches were for comparing the new physics with the old while it was reworked. The physics
refactor kept them until its second phase was done (the vehicle's physics held in `enum Dynamics`),
and then the maintainer decided to remove all three. No player sets them, each one cost a run of
the trace scenarios on every pull request, and the tyre model kept a second suspension, with its
constants and tests, beside the one the game uses.

## How to revert

`physics-ab-switches-removal.patch` in this directory is the code change of the commit that
removed them (the script's runs with each switch on included):

    git apply -R docs/removed/physics-ab-switches-removal.patch

The documentation lines that named the switches (`docs/MODDING.md`, `docs/FORMATS.md`,
`docs/QUALITY.md`) are not in the patch. Edits made since that commit to the touched lines need a
manual merge.
