# Performance plan: cursor ray and traffic spawn spikes

Plan of 24/10/2026, revised on 25/10/2026, based on the measurements for issue #43. Code and log reading only; no code has changed yet. The measurements follow the [measurement protocol](performance-measurement.md).

## Summary

The cursor ray accounts for almost all of the stutter: 5.8 s of the 6.2 s measured in the 147 frames over 50 ms of the 24/10 session (19:25), with peaks of 40 to 82 ms. Traffic stutters when the game creates many cars at once: 46 ms at the first population of that session, and frames of 250 to 500 ms in a 25/10 session when entering an area with many lanes.

The plan has two PRs for the cursor and two for traffic, in that order. None of them changes what the player sees or which car appears: only when and how the work is done.

| stage | slow frames containing it | total (ms) | peak (ms) |
| --- | --- | --- | --- |
| `player.hover` (cursor) | 143 of 147 | 5802 | 82 |
| `traffic` | 72 of 147 | 223 | 55 |
| `traffic.populate` | 4 of 147 | 54 | 46 |

Source: `game.log` of 24/10/2026, 19:25–19:33, build `3d3143ad`, `OMSI_PROFILE=1`, MSAA 8x, shadows 4096, SSAO, 3440x1440, articulated TopbusPB, traffic 120.

## Cursor ray

The cost comes from testing the ray against every triangle of the cab's large meshes, several times per query and more than one query per frame. The camera is inside the bus, so the bounding sphere of each large mesh (body, interior) contains the eye and the sphere filter (`ray_may_hit`) discards nothing.

### What a query does today

`update_hover` (`crates/omsi-app/src/input/mouse.rs`) calls `hovered_part` (`crates/omsi-app/src/player.rs`), which in sequence:

1. `pick_in`: up to 25 rays (the central one and two rings of 8 and 16) against the meshes with a `[mouseevent]`.
2. For each ray with a hit, `nearest_blocker`: one ray against every mesh without an event of the bus and its trailers (368 meshes on the TopbusPB), triangle by triangle. If the switch is covered, it tries the next ray and repeats.
3. No switch found: `pick_trailer_in` repeats step 1 on the trailers.
4. Still nothing: `hovered_body_part` walks every visible mesh again to name the part.

On every test, `ray_mesh` (`crates/omsi-geometry/src/lib.rs`) recomputes and inverts the mesh's matrix. Steps 2 and 4 repeat the same central ray over the same meshes.

### Queries per frame

- Every mouse-move event calls `update_hover` immediately (`on_cursor`), outside the profiler stages. A 1000 Hz mouse produces several events per frame.
- The frame calls it again in the `player.hover` stage, because the event does not update `hover_key`. That is the part the log shows; the event part may explain some of the time "outside the stages".

### Plan

1. **Measure the parts** (in the same PR as step 2): sub-stages `player.hover.pick`, `.blocker`, `.trailer`, `.body` and a count of triangles tested, in the stutter log. Plus a performance test with a synthetic cab mesh.
2. **One query per frame** (`fix/cursor-uma-consulta`): the event only marks that the cursor moved, and the frame runs the query. Touch input and `OMSI_INPUT` scripts, which read `hover` right after the event, keep querying immediately. Compute each mesh's matrix and inverse once per query, and reuse step 2's central ray in step 4.
3. **Bounding-volume tree per mesh** (`fix/cursor-arvore-malha`): a BVH in mesh space in `omsi-geometry`, built once per vehicle type (on the first query, so it does not add to loading) and shared by its copies. `ray_mesh`, `ray_mesh_hit`, `cone_triangles` and `ray_triangles` use it. The test drops from every triangle to a few dozen. Skinned meshes (bellows, curtain, sun blind) deform: they keep the full test.

Expected result (an estimate, to be confirmed by measurement): peaks of 40–82 ms below 2 ms, with the same part picked. The same gain applies to the click, the mouse wheel and the VR pointer, which use the same functions.

### Risks

- The switch picked must not change (Aachen ibox, VDV panels). The existing `best_mouseevent_hit` tests stay, and a test is added that compares the BVH with the full test on real meshes.
- BVH memory: about twice the triangle index, only for the types that get queried (the player's bus).

## Traffic spawn spikes

The game creates every car missing from the target at once, and creating cars is what costs, not having them.

### What the logs show

- **24/10, 19:25 (with the profiler):** a single spike, at frame 81 (71 ms), with `traffic.populate` 46 ms, `traffic.schedule` 5 ms and `traffic.sync` 3 ms. It was the first population, right after loading. For the rest of the session traffic stayed between 2 and 8 ms in the slow frames.
- **25/10, 02:51–03:25 (no profiler, only the once-a-minute status line):** between 03:19 and 03:21 the car count went from 122 to 262 and then 303, and the worst frame of each minute dropped to 2–4 fps (frames of 250–500 ms), preceded by smaller stutters. With 200 cars standing at 03:02 the game ran at 63 fps.

### Likely cause

The car target (`street_target` in `populate_seen`, `crates/omsi-app/src/traffic.rs`) is the requested count times the hour's density times `road_scale`, which grows with the number and density of the lanes nearby (up to 8 times). On reaching a dense area the target rises, and `populate_kind` and `wake_dormant` create the whole deficit in a single call. The smaller stutters before it are likely the target rising step by step during the approach.

The `[AIMaxCountRandom]` limit (`max_random`, default 1000) landed after these sessions and caps the target, but it does not change the fact that the deficit is created at once.

The cause of the 25/10 session is not measured yet: the same stretch still has to be driven again with `OMSI_PROFILE=1` and `OMSI_DEBUG_POPULATION=1`.

### Plan

1. **Measure the parts** (`feat/profile-povoacao`, small): sub-stages `traffic.populate.despawn`, `.wake`, `.parked`, `.spawn`, plus the count of cars created and removed per call, in the stutter log. One run on the same stretch shows where the time goes.
2. **Budget per call** (`fix/transito-orcamento-povoacao`): create at most a few cars per call (by time, for example 3 ms, or by count) in `populate_kind` and `wake_dormant`, and come back in 0.1 s while cars are missing, as the code already does for timetable departures. The target is reached within a second or two without stalling a frame.
3. **First population on the loading screen**: a complement to item 2 for the start only. Run the first call before the first frame, where its cost is not seen.

### Risks

- The budget changes when each car appears, not where or which one. Cars still only spawn out of sight, so nothing pops into view.
- Traffic tests that count cars right after `populate` need to call it until the target is reached.

## PR order and how to measure

One PR at a time, each with a before and after measurement in the same saved situation (TopbusPB, MSAA 8x, traffic 120, `OMSI_PROFILE=1`).

| order | branch | what changes | how to prove it |
| --- | --- | --- | --- |
| 1 | `fix/cursor-uma-consulta` | one query per frame, reused matrices, cursor sub-stages | test that touch input and `OMSI_INPUT` still read `hover` immediately; log with the sub-stages |
| 2 | `fix/cursor-arvore-malha` | per-mesh BVH in `omsi-geometry` | test BVH = full test on real meshes; performance test; `player.hover` in the log |
| 3 | `feat/profile-povoacao` | population sub-stages | one run on the dense stretch of 25/10 |
| 4 | `fix/transito-orcamento-povoacao` | per-call budget and first population during loading | traffic tests; no frame over 50 ms with `traffic.populate` |

Done when: no frame over 50 ms caused by `player.hover` or `traffic.populate` in a 5-minute session of driving, operating switches and passing through an area with many lanes.
