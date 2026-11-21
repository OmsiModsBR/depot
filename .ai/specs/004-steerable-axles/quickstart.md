# Quickstart: validating steerable axles

## Automated (every phase from 2 on)

```sh
cargo test -p omsi-vehicle --test steer_axle_key           # key parsing and validation (SC-005)
cargo test -p omsi-sim --test physics_model_depot          # V2/V5 keyed closed forms (SC-001, SC-006a)
cargo test -p omsi-sim --test steer_axles                  # ramps, continuity, modes, scrub (SC-003..SC-006)
scripts/physics-trace-diff.sh origin/main                  # existing traces identical (SC-002)
```

Expected: all green; the trace diff lists only the new keyed scenarios.

## Handling numbers

```sh
cargo run -p omsi-sim --example handling -- <bus with the key>
```

Compare the full-lock turning radius with the tag axle steering (mode 2) and locked (mode 1): the
locked radius is larger by the amount `depot-model.md` D-5 gives for that bus.

## In OMSI 2 (phase 0, maintainer)

1. Copy a stock 3-axle bus and add the three `achse_depot_steer_*` parameters inside the third
   `[newachse]` ([contracts/bus-key.md](contracts/bus-key.md)); also try them as the first and as
   the last parameters of an axle.
2. Load and drive it in OMSI 2: it must behave exactly like the unmodified copy, with no error in
   `logfile.txt`.

## In Project Depot (phase 7, maintainer)

1. Settings: Project Depot Physics (or `[depot_physics]` in the file).
2. Keyed 6x2 at a tight junction at walking speed: tighter turn than the unkeyed copy; the tag axle
   turns against the front wheels.
3. Accelerate past the lock speed: the tag axle centres smoothly, no jerk; the locked flag lights.
4. A script writing mode 1 locks it at any speed; mode 3 holds the script's angle.
5. Keyed articulated bus with a steered last axle: the rear section cuts the corner less.
6. Frame time with and without the key, same scenario (SC-007), recorded in the phase 6 PR.
