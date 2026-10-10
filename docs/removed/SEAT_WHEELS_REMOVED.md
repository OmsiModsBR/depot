# Wheel seating (`seat_wheels`): removed

Status: **removed** (decision by the maintainer, 2026-10-10).

## What was removed

`VehicleInstance::seat_wheels` in `crates/omsi-sim/src/vehicle.rs`, its cache field `wheel_seats`
and its call in `update_visuals`, together with the debug variable `OMSI_DEBUG_SEAT`.

For a vehicle on the rigid-body physics, every frame it compared where the model's own
`Axle_Suspension_*` animation drew each tyre's hub with where the physics had the hub (the spring's
compression), and moved the tyre mesh up or down by the difference when it was under 8 cm. It
corrected models whose suspension animation lifts the wheels by more or less than the spring is
compressed (the LiAZ 5292, whose tyres stood 4 cm in the road). It ran on the front section only:
the rear sections of an articulated bus never had it.

## What stays

The physics still takes its unloaded hub heights from the model's wheel meshes
(`VehicleType::hub_heights`), and the scripts still receive `Axle_Suspension_*` as before. Only the
correction of the drawn tyres is gone: each tyre is drawn exactly where the model's animation puts it.

## Why

The game does not correct 3D animations that a bus model has configured wrongly: a model whose
suspension animation does not follow its springs shows that, as it would in OMSI 2. It also
removes a difference between the front section (corrected) and the rear sections (not corrected)
of an articulated bus.

## How to revert

`seat-wheels-removal.patch` in this directory is the code change of the commit that removed it:

    git apply -R docs/removed/seat-wheels-removal.patch
