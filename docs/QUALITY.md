# Code quality checks

The `quality` job in `.github/workflows/depot-ci.yml` runs `cargo fmt --check`, `cargo clippy`,
`cargo audit` and `cargo deny check` (policy in `deny.toml`) on every push and pull request.
They are **informational**: each step is `continue-on-error`, so they never fail the workflow
that guards `main` (build and tests). The exception is `rustfmt (formatted crates)`, which blocks
for the crates already formatted (below). Do not add `-D warnings` until the backlog below is cleared.

The release profile also builds with `overflow-checks = true` (integer overflow panics instead
of wrapping), because the parsers read untrusted content.

## Status when this was introduced (toolchain 1.99.0, Linux)

| Check | Result | What is pending |
| --- | --- | --- |
| `cargo build --locked --workspace` | passes | - |
| `cargo test --locked --workspace` | passes (1317 passed, 0 failed, 35 ignored) | - |
| `cargo fmt --all -- --check` | fails: about 8000 diff hunks; the baseline was never formatted | Format crate by crate (macro-refactor plan, track 1), never in one commit: one would break every open branch. Each formatted crate joins the blocking `rustfmt (formatted crates)` step. Formatted so far: `omsi-cfg`, `omsi-o3d`, `omsi-model`, `omsi-map`, `omsi-vehicle`, `omsi-scenery`, `omsi-texture`, `omsi-timetable`, `omsi-content`, `omsi-script` (all the format crates), `omsi-audio`, `omsi-ui`, `omsi-geometry` and `tools/omsi-check`. |
| `cargo clippy --workspace --all-targets` | passes with about 550 warnings | The most frequent: `chunks_exact` with constant size, very complex types, `is_multiple_of`, needless borrows/clones, too many arguments. Fix per crate, then use `-D warnings`. The ten format crates (`omsi-cfg` to `omsi-script`) have none since T1.3 of the macro-refactor plan. |
| `cargo audit` | 2 warnings (unmaintained, no known vulnerability): `paste` (RUSTSEC-2024-0436), `ttf-parser` (RUSTSEC-2026-0192) | Find the dependency that pulls them in and replace or accept the risk in writing. |
| `cargo deny check` | advisories: same two crates; licenses: `omsi-net` has no `license` field, and `webpki-roots` is CDLA-Permissive-2.0 (allowed in `deny.toml` pending maintainer confirmation) | Add `license` to the `omsi-net` manifest; confirm the CDLA decision. Sources and bans pass. |

When a row is cleared, switch its CI step from informational to blocking and update this table.

## Offscreen golden

`crates/omsi-app/tests/offscreen_golden.rs` renders a synthetic root (written by the test, no OMSI 2 content)
with the real `depot --offscreen`. The picture checks run only with `OMSI_GOLDEN_RENDER=1` (or `OMSI_GOLDEN_BLESS=1`)
and a graphics adapter (a software one is enough), and print `SKIP` and pass otherwise; the root read-back and the
window/offscreen `signal_aspects` check always run. Pictures differ between drivers, so the thumbnail is compared with
the golden of the running adapter only: one file per adapter in `crates/omsi-app/tests/golden/offscreen_synthetic/`,
named after its backend and name. Write the running adapter's file with
`OMSI_GOLDEN_BLESS=1 cargo test -p omsi-app --test offscreen_golden -- --nocapture`.

The `offscreen-golden` job (informational for now) runs it on every push and pull request on the software Vulkan
adapter (lavapipe, `mesa-vulkan-drivers` on `ubuntu-24.04`) with `OMSI_GOLDEN_REQUIRE=1`: a missing adapter or a
missing golden for lavapipe fails the job instead of skipping, and the failure (also in the job summary) prints the
file to commit. That is what happens when the runner image moves to another mesa or LLVM, since the adapter's name
carries the LLVM version. The job becomes blocking after three passing runs in a row on the same commit.

## Offscreen state traces

`crates/omsi-app/tests/offscreen_state_trace.rs` runs `depot --offscreen` on a synthetic street the test writes
itself (two lanes, a crossing whose two lights run a program, six AI cars, a 12 m bus rolling north) with
`OMSI_STATE_TRACE=<file>`, which writes for every frame where the bus is and how fast it goes, every AI car, the
lights, the rail signals and the player's duty (`crates/omsi-app/src/state_trace.rs`). The test runs only when
`OMSI_STATE_TRACE_DIR` is set (the run renders, so it needs a graphics adapter; a software one is enough), runs the
street twice, fails when the two runs differ and writes the trace to `<dir>/synthetic.trace`. The street has no
timetable yet, so the duty line reads `-`.

The `offscreen-traces` job (informational for now) runs `scripts/offscreen-trace-diff.sh` on pull requests, on
lavapipe: the scenario on the pull request's base and on the change, compared bit for bit, in the same way as the
physics traces below. A change meant to alter the behaviour carries the `offscreen-delta` label (re-run the job
after adding it). The job becomes blocking once two parity pull requests in a row passed it without a false alarm.
Locally: `scripts/offscreen-trace-diff.sh [base-ref]` (default `origin/main`); the traces are kept in
`target/offscreen-traces/`.

## Vehicle physics traces

The refactor of the vehicle physics moves code in steps that must not change what the
physics computes. Two jobs of the workflow guard it on pull requests:

- `physics-traces` (blocking) runs `scripts/physics-trace-diff.sh` against the pull
  request's base. The scenarios in `crates/omsi-sim/tests/physics_traces.rs` drive the rigid
  body, a train of articulated sections and the vehicle with its script variables (standing,
  on a grade, spinning on ice, locking the brakes, bends up to the full lock, humps, a kerb,
  a wall face, crashes, frame times from 1/144 s to 0.3 s, inputs that are no number, air
  springs) and write the whole state of every frame. The script runs them on both commits
  and compares the traces bit for bit; it also runs the change twice and fails if two runs of
  the same commit differ. Both sides are built and run on the same machine, so the
  comparison does not depend on how a platform rounds `sin` or `sqrt`. A change meant to alter the physics carries the
  `physics-delta` label (re-run the job after adding it): the job then lists the first
  difference of each trace without failing, for the review to check against the predicted
  change.
- `physics-coverage` (informational) reports the lines, regions and branches of
  the rigid body (`crates/omsi-sim/src/rigid/`, its tests aside) that those scenarios and
  `physics_invariants.rs` reach.

Locally: `scripts/physics-trace-diff.sh [base-ref]` (default `origin/main`); the traces are
kept in `target/physics-traces/`. Without `OMSI_PHYSICS_TRACE_DIR`, `cargo test` runs each
scenario twice and checks that both runs agree.

## settings.cfg between the game and the launcher

The launcher's settings page writes `settings.cfg` and the game reads it. The unit tests in
`crates/omsi-app/src/settings/round_trip.rs` (run by `cargo test --workspace`) write the file
from one side and read it from the other, in both directions:

- `game>launcher`: the file as the game writes it (`Settings::to_text`, test only), read and
  saved by the launcher as its page's Save does, then read by the game again.
- `launcher>game`: the file as the launcher writes it, read by the game, written by it, then
  read by the launcher again.

They try the defaults, one value per key inside its range (`VARIED`, which must reach every
field the game has) and, on every key, values at and past the ends of the ranges and ones
that are no number (`EDGES`). Inside the ranges and on the defaults both sides agree. The
differences at the edges are listed per key and direction, with what goes wrong, in `KNOWN`;
they are not fixed there. A change that fixes one takes its line out of `KNOWN` (the test
fails while a listed difference no longer happens), and a new difference fails the test.
