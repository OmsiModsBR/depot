# Contract: weather inputs that drive the cloud shade

No new setting, file or key is added (FR-012, Q4). This contract records the existing inputs the
feature reads and how each is interpreted in Enhanced after the change.

| Input | Where | Interpretation |
| --- | --- | --- |
| `[clouds]` line 1 (type) | weather `.owt` | `-1`/empty: no clouds, no shade. `Cumulus 1..3`: cover 0.35/0.55/0.75, per-point shade only, no even greying (FR-018). `Overcast...`: cover 1, closed: sky greying + the map's shade removing the direct sun everywhere (FR-009). |
| `[clouds]` line 2 (height, m) | weather `.owt` | cloud base above the map's height zero, no floor (Q2); absent, zero, negative or not a number -> 1400 m, no message. |
| METAR `FEW/SCT/BKN/OVC/VVnnn` | real weather | densest layer -> type as above; base `nnn x 30.48` m (unchanged parser, `omsi-content/src/weather/mod.rs`). |
| Weather model (`cumulus`, `deck`, `veil`) | physical weather | cumulus -> cover; deck -> cover and `closed` (R-7); veil -> unchanged even dimming and shadow softening (not drawn in the map). |
| `clouds` setting | `settings.cfg` | off -> no clouds, no shade (FR-012). |
| `shadows` setting | `settings.cfg` | off -> no object shadows; cloud shade still dims the light where clouds stand (spec edge case). |
| Visibility below 350 m | weather fog | sun shadows off, as today. |

Vanilla+ and Vanilla (DX9) keep their current interpretation of every row (FR-013).
