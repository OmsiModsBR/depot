# Pusher articulation — experimental integration

Branch: `feat/pusher-articulation`, based on Depot main
`b4d50a878156ab436b375b07a0c58d0ef8461439`.

Source branch: https://github.com/lmonteirotech/openOMSI/tree/feat/pusher-articulation.
Observed source tip: `cf8fd061edcb7fb0c0ceb4eaeeed13137a5faa54`.
Selected feature commit: `a97595b1b254cf94abc27bb67844a8a1dfcee2ab`,
authored by zane_thruston; original authorship and cherry-pick provenance retained.

The source branch had 190 commits beyond the historical baseline. Its main
merge and unrelated sound/compilation cleanup were not imported. Only the
pusher joint implementation, tests and related documentation were cherry-picked.
The source patch applied to the 0.2.16 structures without runtime conflict.
Its Unreleased entry was moved out of the historical upstream changelog.

The rear `.bus` section may declare `[coupling_pitch_offset]`, in metres.
Positive finite offsets move the pitch hinge ahead of the ball on the front
section; yaw continues to turn about the ball. Missing, zero, negative or
nonfinite offsets preserve the baseline puller joint. Existing articulation
script variables retain their sign and meaning. See [modding instructions](../MODDING.md).

Synthetic tests cover parsing, legacy defaults, hinge/link geometry, offset
scaling and yaw equivalence. Validation on Windows x64 / Rust 1.99.0 passed
298 tests across `omsi-sim` and `omsi-vehicle`; one existing benchmark remained
ignored. `cargo check --locked --workspace --all-targets -j 1` also passed.
Tests used the inherited development optimization level 1, debug=0 and a
dedicated target directory; dependencies remained locked. These checks
do not certify a real bus model, bellows animation, all road
profiles or a full physical multibody implementation.

This branch was independent of `feat/outside-fov-ultrawide` and the multibody branch. It is merged into
main and part of released Depot 0.2.52 (not of 0.2.17). MIT, internal identifiers and Depot branding are retained.
