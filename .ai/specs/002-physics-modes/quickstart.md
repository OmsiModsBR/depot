# Quickstart: how each phase is validated

See the [plan](plan.md) (phases), [research](research.md) (R-V, R-A) and [contracts](contracts/).

## Prerequisites

- Windows x64 with OMSI 2 installed (`OMSI_ROOT`), for the in-game checks Q1-Q4.
- `cargo test --workspace` and `scripts/physics-trace-diff.sh <base>` for every phase.

## Q1. Phase 0: OMSI 2 ignores `[depot_physics]` (#677 item a)

1. Copy a stock bus folder (for example the SD202) to a test folder; add a line `[depot_physics]`
   after `[friendlyname]` in its `.bus` file, and once more with a stray line `1` under it.
2. Start OMSI 2, put the bus on a stock map, drive one stop.
3. Expected: the bus loads, drives and stands as the unmodified copy; `logfile.txt` has no line the
   unmodified copy does not produce.
4. Repeat with openOMSI and each fork the maintainers name.

The result is written in #677 and in `contracts/bus-key.md` before phase 3.

## Q2. Phase 0: the Classic AI observations (R-A, KA-1..KA-7)

For each row of R-A, the check in its "How it is checked" column, on a stock map with the AI density
low enough to follow one vehicle. Record a short video or a screenshot sequence and fill the row with
"confirmed", "not so" and what was seen. Phases 8-10 implement only confirmed rows.

## Q3. Phase 0: the Mi5 levels as in OMSI 2 with #675 (#677 item d)

The Caio Millennium V articulated at rest on a flat stop, in OMSI 2 and in Depot (Classic): the three
axles' `Axle_Suspension` (Depot's vehicle info) and a side screenshot from the same camera. Expected:
9.1 / 9.8 / 9.1 cm in Depot and a level body in both.

## Q4. Phase 0 and D2: override candidates (R-V)

The checks of R-V for V3 (coasting on a grade) and V4 (kerb), the same in OMSI 2 and in Depot. A row
whose result differs moves to Depot (phase 12) or becomes a `fix/` for Classic, by decision D2.

## Q5. Phase 1: model documents

- Every equation has an id, inputs, outputs and the module that implements it.
- `grep -nE "0x[0-9a-fA-F]{5,}" docs/physics` is empty, and no name or threshold read from the reference executable appears (Principle VIII).
- Every Classic rule says "validated" or "to validate".

## Q6. Phase 2: conformance of today's physics

```text
cargo test -p omsi-sim --test physics_model_classic
```

Expected: every player row of [contracts/test-vehicles.md](contracts/test-vehicles.md) passes for
V1-V5 in Classic. A failing row is either a defect (a `fix/` PR with that test failing before it) or a
document error (corrected in `docs/physics/`), decided in the pull request.

## Q7. Phases 3-6: nothing changes

- `scripts/physics-trace-diff.sh origin/main`: every player, AI and traffic trace identical, in both
  engines (phase 4 on); no `physics-delta` label.
- `omsi-app` settings tests: every row of [contracts/settings-cfg.md](contracts/settings-cfg.md).
- `omsi-vehicle` tests: every row of [contracts/bus-key.md](contracts/bus-key.md); a file with the key
  has no `unknown_keywords` entry for it.
- A session log shows, for each spawned vehicle, its engine and the reason.
- Frame time: the existing physics benchmark scenes before and after, within noise.

## Q8. Phase 7: Depot shares the weight about `[cog]`

- `physics_model_depot`: static sag rows (D-1) pass on V1-V5; the Classic rows still pass.
- Trace diff: only the `.depot` player traces change (`physics-delta`, predicted in the PR from D-1);
  every Classic trace and every AI trace identical.
- In the game: the Mi5 with `[depot_physics]` added stands nose down as before #675 (front 12.7 cm),
  without the key level as in Q3.

## Q9. Phases 8-10: Classic AI

- `physics_model_classic`: the AI rows of the confirmed KA-* on V1-V5.
- Trace diff: Depot AI traces identical; Classic AI traces change (`physics-delta`).
- Cost: the AI step per vehicle and frame on 40 AI vehicles, Classic against `AiBody`; Classic not
  slower (R-P).
- In the game: the same junction filmed in Q2, in OMSI 2 and in Depot Classic, side by side.

## Q10. Phase 11: the setting

- Launcher: the "Physics" list saves `physics=classic|depot` and reads it back.
- A session with `physics=depot`: every vehicle logs Depot; with `classic`, only vehicles with the key
  log Depot.
- `docs/FORMATS.md`, `docs/MODDING.md` and `docs/USER_GUIDE.md` describe the key and the setting;
  `CHANGELOG.md` has the line under **Unreleased**.
