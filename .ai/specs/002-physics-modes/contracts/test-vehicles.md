# Contract: the five test vehicles and the conformance matrix

Synthetic vehicles built in `omsi-sim/tests/support/vehicles.rs` (small, made for the test,
Principle VI and VIII). Values are plausible for the class, not copied from any mod. The axle layout
and `[rot_pnt_long]` follow the usual conversion rules for each layout (the `omsi-bus-files`
reference used by the project): rear axle for a 4x2; the middle axle when the third axle steers; the
front section's rear axle for an articulated front section; a rear section's first axle.

| # | Vehicle | Sections | Axles (front to rear) | `[rot_pnt_long]` |
|---|---|---|---|---|
| V1 | 4x2 solo bus, 12 m | 1 | steered front; driven rear | rear axle (today's `bus()`) |
| V2 | 6x2 rigid bus, 15 m, steered third axle | 1 | steered front; driven middle; **steered** tag | middle axle |
| V3 | 6x2 articulated bus, 18 m (pusher) | 2 | front: steered front, middle; rear: driven | front: its rear axle; rear: its axle |
| V4 | Bi-articulated bus, 27 m (8x2, puller) | 3 | front: steered front, **driven** second; middle: one axle; rear: one axle | each section per the rules above |
| V5 | Articulated bus, 23 m, 8x2 (pusher) | 2 | front: steered front, second; rear: **driven** first (third axle), **steered** fourth | front: its rear axle; rear: its first axle |

Decision D5 (2026-11-08): V5 is one vehicle; five vehicles in total. Driven axles as the maintainer
gave them (2026-11-08): V4 drives the front section's second axle, V5 the rear section's first.

Each vehicle is also given a `[cog]` y off the origin (between 0.3 and 0.8 m forward) so that the
Classic and Depot static shares differ measurably (K-1 against D-1).

## Matrix (player body; the AI columns follow in phases 8-10)

| Check | Ids | V1 | V2 | V3 | V4 | V5 | Engines |
|---|---|---|---|---|---|---|---|
| Static sag per axle and pitch at rest | C-3, K-1 / D-1 | ✓ | ✓ | ✓ | ✓ | ✓ | both (they must differ by the closed form) |
| Pull-away below the grip | C-4, C-5, C-8 | ✓ | ✓ | ✓ | ✓ | ✓ | both |
| Braking below the grip, stop distance | C-4, C-5 | ✓ | ✓ | ✓ | ✓ | ✓ | both |
| Steady low-speed turn at full lock: radius of the turning line | C-6, C-7 | ✓ | ✓ | ✓ | ✓ | ✓ | both |
| Each steered axle's angle, tag axle opposite | C-6 | ✓ | ✓ (tag) | ✓ | ✓ | ✓ (fourth) | both |
| Steady articulation angle(s) | C-9 | — | — | ✓ | ✓ (two joints) | ✓ | both |
| Release when the bend asks more than the grip | C-7 | ✓ | ✓ | ✓ | ✓ | ✓ | both |

## Matrix (AI body)

| Check | Classic ids | Depot ids | V1-V5 |
|---|---|---|---|
| Rotation point on the path through test bends (90° junction, roundabout, lane change) | KA-1 | DA-1..DA-3 (bounded error, today's behaviour) | all |
| Pull-away and stop from the scripts' drive and brakes | KA-2, KA-3 | DA-1 (planner speed) | all |
| Pose at rest and in a bend | KA-4, KA-7 | DA-4..DA-6 | all |
| Rear sections on the path | KA-5 | today's coupled followers | V3, V4, V5 |
| After a hit | KA-6 | today's impulse | V1 |

The Depot AI checks start from today's `AiBody` behaviour and the existing `ai_traces.rs`: they make
the DA-* equations explicit without changing a bit.

## Where the tests live

- `omsi-sim/tests/physics_model_classic.rs`, `omsi-sim/tests/physics_model_depot.rs`: one test per
  row and vehicle, named `<check>_<vehicle>` (for example `static_sag_v4_biarticulated`), each naming
  its ids.
- Optional, with `OMSI_ROOT`: the same static-sag check on real installed buses the maintainer lists
  (the Mi5 first), skipped without it.
