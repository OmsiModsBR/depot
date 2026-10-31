# Upstream provenance

Project Depot was established from the following openOMSI revision:

- Repository: https://github.com/openOMSI-org/openOMSI
- Version: `0.2.16`
- Date: `2026-10-07`
- Commit: `c85b2e0dda13d219fd12d0496c653d20cdb54145`
- Immutable Depot baseline tag: `depot-baseline-openomsi-0.2.16`

The tag points exactly to the original upstream commit, not a Depot
administrative or release commit. It must never be moved.
The original MIT license, copyright notices, attribution, authorship and
ancestral Git history are preserved. Depot does not claim authorship of
upstream code.

The upstream repository is consulted for research. Project Depot does not automatically merge or rebase
onto upstream `main`, and since its licence changed takes no code from it (see *What came from where* below).

Maintenance remotes:

- `origin`: https://github.com/OmsiModsBR/depot.git
- `upstream`: https://github.com/openOMSI-org/openOMSI.git (consultation only)

The maintenance environment disables pushing to `upstream`, disables automatic
tag following for that remote, and restricts its branch fetch mapping to main.
No Depot branch tracks upstream/main. Remote configuration is local Git metadata,
not a configuration imposed on users cloning the repository.

## What came from where

1. **The base.** Project Depot is built on openOMSI `0.2.16` (MIT). All of that code, with its Git
   history and authorship, is inherited as it was; Depot does not claim it. It is the large majority of
   the engine's older parts (content formats, scripts, simulation, renderer, audio).
2. **Changes taken in while openOMSI was MIT (up to `0.2.50`).** A few changes were taken in one at a
   time after a licence and provenance review, either as an adapted cherry-pick of a single commit, as code
   from the maintainers' own forks, or written again in Depot from the behaviour a commit described. Each
   is listed below with its source, author, licence and method, and recorded in the
   [changelog](CHANGELOG.md) and, for the larger ones, in `docs/integrations/`.
3. **Since openOMSI changed licence (from `0.2.51` on), no code has come from it.** Anything learnt
   from later work was reimplemented from scratch, from described behaviour and Depot's own tests,
   in a clean-room way: nobody copied or adapted that code. That is very little compared with what the
   Depot has built on its own (the DirectX 9 renderer, the new interface and launcher, the integrity
   checks, the physics refactor and much more, see the [changelog](CHANGELOG.md)).

| Change | Depot commit(s) | Source and licence | Method |
| --- | --- | --- | --- |
| Outside view (F3) with its own field of view, fitted to ultrawide screens | `0cc0a03d` | openOMSI `ea47e092`, MIT, author zane_thruston (taken through the maintainer's fork) | Cherry-pick, adapted to the baseline's files |
| Pusher articulation joint (`[coupling_pitch_offset]`) | `d265fbab`, `38777fcc` | openOMSI `a97595b1`, MIT, author zane_thruston (taken through the maintainer's fork) | Cherry-pick |
| Articulated buses and trailers: every section a body of its own | `f62eac79` and the commits after it | Fork branch `feat/multibody-articulation` of the maintainer, MIT, author zane_thruston | Code from the fork |
| Navigator map packages (surface, icons, regions, street names, lines, orientation, terrain) | `218256ab` and the commits after it | Fork branches `feat/map-package` and `local/lines-timetable` of the maintainer, MIT, author lmonteirotech | Code from the maintainer's fork |
| A sound entry that starts playing starts at full gain | `0a1314f7`, `cb92f185` | Behaviour of openOMSI `ea26060`, MIT, author turbo-devv | Reimplemented in Depot, with new tests |
| Timetable delay on the way follows where the bus is | `0e749e3f` | Behaviour of openOMSI `3c75499`, MIT, author turbo-devv | Reimplemented in Depot |
| The game opens on the monitor the launcher is on | pull request #13 | An issue reported to openOMSI | Independent implementation |
| An open door wins over a shut button door when passengers board | `3ec96874` | Same change as openOMSI `3bebe136`, MIT | Written in Depot, with a new test |

Verification:

```sh
git rev-parse depot-baseline-openomsi-0.2.16
git merge-base depot-baseline-openomsi-0.2.16 main
```

Both commands must return `c85b2e0dda13d219fd12d0496c653d20cdb54145`.
The Depot release numbers (0.2.17, 0.2.52, 0.3.0) are Depot's own and do not mean the openOMSI release of the same number was imported.
