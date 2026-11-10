# Contract: the `[depot_physics]` key in a `.bus` file

Status: name and format **decided** (D1, 2026-11-08); waiting on the black-box check (quickstart
Q1). Public text for `docs/FORMATS.md` and `docs/MODDING.md` is written from this contract in phase 9.

**If Q1 fails** (OMSI 2 or a named fork does not accept the line; maintainer, 2026-11-09): the key
moves to a separate file next to the `.bus` file, `<name>.depot.cfg`, holding the `[depot_physics]`
line, which OMSI 2 never reads. Its presence, format and rules are then the same as below, read from
that file instead of the `.bus` file; the `.bus` key is not introduced.

## Format

```text
[depot_physics]
```

A section with no value lines. Its presence makes the vehicle use the Depot physics engine when the
player drives it, whatever the player's "Physics" setting says. AI vehicles ignore it and follow the
setting (decision D6), so keyed mods in traffic add no processing load. There is no value that forces OMSI Classic.

## Reading rules (Depot)

| File content | Result |
|---|---|
| no `[depot_physics]` | the vehicle follows the setting |
| `[depot_physics]` once | Depot |
| `[depot_physics]` more than once | Depot; one log line |
| lines under `[depot_physics]` before the next section | Depot; the lines are ignored with one log line (they are not read as another section) |
| `[depot_physics]` in a rear section's file only (`[couple_back]` target) | no effect; one log line naming the file. The front section's key applies to every following section (decision D4) |
| any vehicle with `[depot_physics]` spawned as AI traffic (including `*_KI.bus` copies) | the key is ignored: the AI vehicle follows the setting (decision D6); no log line per spawn |

Nothing about the key can stop the load (Principle IV).

## Other readers

The key is written so that a reader that does not know it skips it:

- OMSI 2: **validated** (Q1, maintainer's test, 2026-11-09): OMSI 2 ignores `[depot_physics]`; the
  vehicle loads and drives as without it.
- openOMSI and other forks: the maintainer's test of 2026-11-09 found the same on the derivatives
  tested; the key stays `[depot_physics]` and the `<name>.depot.cfg` fallback (FR-009a) is not needed.

## What a modder is told (phase 9)

- What Depot physics changes (the list of `D-*` items in `docs/physics/depot-model.md`), so a mod can
  be tuned for it, for example the springs for the weight shared about `[cog]`.
- That OMSI 2 and other games ignore the key, so the same file works there with OMSI 2's physics.
