# Quickstart: validating cloud shadows in Enhanced

**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

## Automated (CI, lavapipe)

```sh
cargo test -p omsi-render --test cloud_shade      # new GPU test (research R-1..R-4, R-8)
cargo test -p omsi-render --test render_modes     # Vanilla+ unchanged (SC-007)
cargo test -p omsi-app --test weather_lighting    # cumulus no longer greys the sky in Hdr (FR-018)
```

Expected:

- `cloud_shade`: in one frame with a synthetic cloud field, a ground point under a cloud gets less
  than 10 % of the sun and a point in a gap more than 90 % (FR-001, FR-003); the edge 10-90 % width
  at base 3000 m is at least twice the width at 1000 m (SC-004); with `Overcast` every point gets
  under 5 % (FR-009); at night with the moon as light the same holds for the moonlight (FR-016);
  with the clouds setting off every point gets 100 %.
- `render_modes`: Vanilla+ picture identical to the reference.
- `weather_lighting`: with `ShadingPath::Hdr`, `Cumulus 3` leaves `sun_intensity` and
  `sky_color` as for a clear sky; `Overcast` greys as today; with `ShadingPath::Linear` both match
  today's values.

## In game (maintainer, Windows)

1. Build the PR, set Enhanced, shadows on, clouds on.
2. **Q1 (US1, SC-001)**: Desafio, MAN A23, 13:00, weather `Cumulus 3` (or the METAR `BKN009`).
   Take 10 views over 5 minutes, half with the camera under a cloud. Every view with sunlit ground
   shows object shadows there.
3. **Q2 (US2, SC-002)**: same place and time with `Cumulus 1`, `Cumulus 2`, `Overcast`. The share of
   ground in shade grows with the number; `Overcast` shades everything.
4. **Q3 (US3, SC-004)**: a custom weather with `Cumulus 2` at 500 m and at 3000 m. The shadow edges
   are visibly wider at 3000 m; the clouds sit lower/higher in the sky to match.
5. **Q4 (US4, SC-006)**: drive from sun into a cloud's shade and stop. After a few seconds the
   foreground is clearly darker than before (about 1.7 stops) and sunlit areas ahead stay brighter.
6. **Q5 (US5, SC-009)**: a night with a bright moon high up, `Cumulus 3`, a road without lamps. Moon
   shadows show in moonlit patches and not in cloud shade; `Overcast` removes them.
7. **Q6 (SC-005)**: with the in-game fps counter, 60 s on the same route before and after the PR;
   average frame time grows by at most 2 %.
8. **Q7 (FR-013)**: Vanilla+ and Vanilla (DX9) look as before in the same weather.
