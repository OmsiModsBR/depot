# Parity: AI traffic and people density

Status: **implemented, not yet measured against OMSI 2 in the game.** Tracks issue #613.

This page records observable behaviour only: what the player sees and what can be counted
while playing. It carries no code, addresses or internal names of any reference
program. The numbers in the "Calibration" section are the Depot's own and are the part still
to be settled by measurement.

## Observable behaviour (the Depot now follows)

| # | Behaviour | Confidence |
|---|---|---|
| 1 | There is no target number of vehicles. Each street lane brings vehicles on by itself, at random intervals; a shorter interval means a denser road. | high |
| 2 | The only global limit is the "vehicles without a timetable" cap (all of them in circulation, wherever they are). At the cap nobody appears until the count falls. | high |
| 3 | There is no distance around the player and no "out of sight" rule: vehicles may appear anywhere on the loaded map, in view too. | high |
| 4 | A lane's rate is the product of: its configured density (none = never), the day curve for the hour, the Road Traffic Factor, divided between the lanes of the road running the same way. | high |
| 5 | The factor multiplies the curve linearly (twice the factor, twice the rate, below the cap). The curve is used as it is: no ceiling is applied inside the calculation. The factor control runs from 1 % to 300 %. | high / medium (curve ceiling) |
| 6 | Arrivals are at the start of the lane. Refused only when the entry is occupied or the cap is reached. At most one per lane per pass; late timers do not catch up. | high |
| 7 | When a scenario (or a map tile) loads, every lane without vehicles gets a random number of them, proportional to its rate and length, at random places along it. | high |
| 8 | Timetable vehicles do not use the Road Traffic Factor. | high |
| 9 | People walking the pavements follow the map's people curve and the pavements nearby, never exceed half of the people pool, and are **not** scaled by the passengers factor. That factor only scales people waiting at stops. | high |

## Not established (stay as before until measured)

Default value of the Road Traffic Factor; the exact form of the two per-group factors;
quality presets; when and where vehicles are removed; whether pedestrians stop at exactly half
of the pool; the absolute scale of the rate (below).

## Calibration (the Depot's own, to be tuned)

`CARS_PER_METRE` (first fill) and `SECONDS_PER_RATE_METRE` (interval) in
`crates/omsi-app/src/traffic_density.rs` are chosen so that a lane at rate 1 starts with about one
vehicle per 250 m and a 100 m lane gets a new one about every 25 s. They set the absolute
density and must be tuned with scenario A below.

## Tests

- Automated: `crates/omsi-app/tests/app/traffic/spawn_density.rs` (rate is linear in the factor,
  unclamped curve, lane split, floor, interval and first fill proportions, clock without
  catch-up, cap, walker target).
- Manual, black box, same map, time and start point, camera still:
  - A. every AI/Traffic option at its maximum, factor 300 %: count vehicles without a timetable
    at 0 s, 30 s, 2, 5 and 10 min; five runs. Accept regime within ±15 % of the reference.
  - B. factor 50/100/200/300 % with a cap that does not limit: counts proportional within ±15 %.
  - C. factor 300 %, cap 50/100/200: count reaches the cap and stays exactly there.
  - D. standing on a busy wide road: vehicles may appear in view.
  - E. 04:00/08:00/12:00/18:00/23:00 on a weekday and a Sunday: counts follow the map's curve.
  - F. limit the frame rate to 60/30/10: density falls with it.
  - G. people: maximum people pool; walking people never above half of it; passengers factor
    changes the crowd at stops, not on the pavements.
  - H. first usable frame after load within ±20 % of the 10-minute count.
