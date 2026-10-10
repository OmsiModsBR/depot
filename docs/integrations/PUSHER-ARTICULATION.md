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
scaling and yaw equivalence. Validation results will be recorded after
execution. They do not certify a real bus model, bellows animation, all road
profiles or a full physical multibody implementation.

This branch is independent of `feat/outside-fov-ultrawide` and the existing
experimental multibody branch. It is not merged into main and is not part of
released Depot 0.2.17. MIT, internal identifiers and Depot branding are retained.
