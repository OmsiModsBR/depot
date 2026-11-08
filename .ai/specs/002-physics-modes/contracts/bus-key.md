# Contract: the `[depot_physics]` key in a `.bus` file

Status: name and format **decided** (D1, 2026-11-08); waiting on the black-box check (quickstart
Q1). Public text for `docs/FORMATS.md` and `docs/MODDING.md` is written from this contract in phase 11.

## Format

```text
[depot_physics]
```

A section with no value lines. Its presence makes the vehicle use the Depot physics engine, whatever
the player's "Physics" setting says. There is no value that forces OMSI Classic.

## Reading rules (Depot)

| File content | Result |
|---|---|
| no `[depot_physics]` | the vehicle follows the setting |
| `[depot_physics]` once | Depot |
| `[depot_physics]` more than once | Depot; one log line |
| lines under `[depot_physics]` before the next section | Depot; the lines are ignored with one log line (they are not read as another section) |
| `[depot_physics]` in a rear section's file only (`[couple_back]` target) | no effect; one log line naming the file. The front section's key applies to every following section (decision D4) |
| `[depot_physics]` in an AI-only copy (`*_KI.bus` or the file an AI list names) | that copy spawns with Depot physics: the spawned file decides |

Nothing about the key can stop the load (Principle IV).

## Other readers

The key is written so that a reader that does not know it skips it:

- OMSI 2: to validate (Q1). Expected: the line is not a section OMSI 2 knows, so it reads past it; the
  vehicle loads and drives as without it, and `logfile.txt` shows nothing new.
- openOMSI and other forks: the same check on each one the maintainers name.

## What a modder is told (phase 11)

- What Depot physics changes (the list of `D-*` items in `docs/physics/depot-model.md`), so a mod can
  be tuned for it, for example the springs for the weight shared about `[cog]`.
- That OMSI 2 and other games ignore the key, so the same file works there with OMSI 2's physics.
