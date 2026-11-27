# Implementation Plan: Cloud shadows on the scenery in Enhanced

**Branch**: one per delivery phase (see "Delivery phases") | **Date**: 2026-11-27 |
**Spec**: [spec.md](spec.md)

**Input**: spec 005 (clarified 2026-11-27): every cloud kind shades the scenery per point, strength
from opacity, soft edge from height; the clouds also block the moonlight; exposure makes up a
quarter of a cloud's shade at the camera (Q1); real cloud height with no floor (Q2); only Overcast,
a closed deck and precipitation grey the sky (Q3); no new setting (Q4).

**Execution notes**: `.ai/.specify/extensions.yml` does not exist, so no hooks ran.
`setup-plan.sh` ran (it copied the template). Code read at `main` 5feb348.

## Summary

Today the Enhanced scene's sun is scaled by one cloud transmittance measured at the camera
(`renderer/sky.rs`, `sun: st.sun * cloud_t`), so a cloud over the camera removes the sun and every
sun shadow from the whole scene; the cumulus types also grey the sky and dim the sun evenly; the
clouds sit in a fixed 1400-2800 m layer; and the moonlight is dimmed evenly by the cover.

The plan:

1. draws a **cloud shade map**: the clouds' optical depth towards the light, laid out on the cloud
   base plane, 1024², 12 km, blurred by a radius that grows with the cloud height, redrawn only when
   its inputs change, and shifted by the drift in between (research R-1 to R-5);
2. makes the Enhanced scene shader read it once per lit point for the sun term (and the object
   shadows within it), mirrors and the reflection probe included, and drops the camera-wide
   `cloud_t` from the scene's sun (R-6);
3. passes the weather's cloud height to the sky shader and the CPU transmittance, no floor (R-11);
4. lets only the closed part of the cover grey the sky and dim the light evenly in Enhanced, and
   stops switching shadows off for `Overcast` there (R-7);
5. draws the same map for the moon at night and drops the moon's even dimming (R-8);
6. sets the exposure's cloud share to a quarter (R-12).

Vanilla+ and Vanilla (DX9) do not change (FR-013).

## Technical Context

**Language/Version**: Rust (workspace toolchain as pinned), WGSL
**Primary Dependencies**: `wgpu`, `glam` (already used); no new crate
**Storage**: none (no setting, no file format change)
**Testing**: `cargo test --workspace`; GPU tests on lavapipe in `crates/omsi-render/tests/`
(new `cloud_shade.rs`; `render_modes.rs` and `sky_clouds.rs` as regression); unit tests in
`crates/omsi-app/tests/` for the weather lighting (Hdr vs Linear)
**Target Platform**: Windows x64 desktop (DX12/Vulkan); Enhanced is not built on OpenGL/Android
**Project Type**: game (Cargo workspace), renderer crate `omsi-render`, app crate `omsi-app`
**Performance Goals**: Enhanced frame time +2 % at most on the maintainer's machine (SC-005);
estimates in research R-9 (under 0.1 ms per map draw, a draw every 30-60 s, one read per pixel)
**Constraints**: Vanilla+/DX9 pictures unchanged (SC-007); no rule based on model geometry
(FR-015, project principle on OMSI-tuned models); texture budget 17 -> 18 with fallback (R-10)
**Scale/Scope**: one extra 2 MiB texture, three small render passes, ~6 files touched per phase

## Constitution Check

*Gate before Phase 0 research; re-checked after Phase 1 design.*

| Principle | Check | Result |
| --- | --- | --- |
| I. Own path | Depot feature of Enhanced; no upstream constraint | Pass |
| II. OMSI 2 fidelity only with validation | No OMSI 2 claim anywhere; the spec and plan describe Depot only. No physics or offscreen state change (render only), so no `physics-delta`/`offscreen-delta` label; `apply_weather` changes only the Hdr path's `Lighting` (render input) | Pass (verify the offscreen traces stay identical: they do not read `Lighting`) |
| III. No compensation for misconfigured content | Weather values used as given (height with no floor, Q2) | Pass |
| IV. Invalid content never crashes | Bad heights fall back silently (contract); a device without the texture slot falls back (R-10) | Pass |
| V. Domains as packages | Map pass and resources live in `modes/enhanced/` (mode-only); shared cloud functions stay in core `shaders/sky/`; core never imports the mode (layer script) | Pass |
| VI. Tests outside `src/` | New tests in `crates/*/tests/` | Pass |
| VII. Performance | New feature widening per-frame work: justification (the reported bug), before/after measurements on lavapipe and on the maintainer's machine in every phase PR, gate +2 % | Pass with measurements owed per PR |
| VIII. Provenance and public text | External sources are public engine docs; nothing from the reference executable | Pass |
| IX. Small, traceable changes | Six phase PRs below, each `feat/` or `fix/` with its own test | Pass |
| X. Lean, clean code | The camera-only scene `cloud_t` and the Hdr `!overcast` shadow switch are removed, not gated; one map for sun and moon | Pass |

Re-check after Phase 1: no violation; Complexity Tracking empty.

## Project Structure

### Documentation (this feature)

```text
.ai/specs/005-cloud-shadows/
├── spec.md
├── plan.md              # this file
├── research.md          # R-1..R-12
├── data-model.md
├── quickstart.md
├── contracts/
│   └── weather-clouds.md
├── checklists/
│   └── requirements.md
└── tasks.md             # /speckit-tasks (not created here)
```

### Source Code (repository root)

```text
crates/omsi-render/src/
├── modes/enhanced/
│   ├── cloud_shade.rs                 # NEW: map resources, redraw policy (R-3, R-5), uniform fields
│   └── shaders/
│       ├── cloud_shade.wgsl           # NEW: depth pass + blur passes (R-2, R-4)
│       └── enhanced.wgsl              # read the map in the sun and moon terms (R-6, R-8)
├── shaders/sky/sky_light.wgsl         # CLOUD_BOTTOM/TOP -> uniform base/top (R-11)
├── shaders/common/enhanced_common.wgsl# uniform: cloud_shade, cloud_layer, cloud_light
├── renderer/sky.rs                    # scene sun without cloud_t; uniform fill (R-6)
├── renderer/mirror.rs                 # mirrors read the same map (FR-011)
├── sky/air.rs                         # CLOUD_SHADE_ADAPT 0.25 (R-12); base/top in cloud_sun_transmittance
├── sky/atmosphere.rs                  # moonlight dimmed by `closed` only (R-8)
└── gpu/caps.rs                        # ENHANCED_SAMPLED_TEXTURES 18 + fallback flag (R-10)
crates/omsi-render/tests/
└── cloud_shade.rs                     # NEW GPU test
crates/omsi-app/src/
├── world/domain/lights.rs             # apply_weather: Hdr greys only by `closed` (R-7)
└── weather/implementation/weather_setup.rs  # Hdr: no `!overcast` shadow switch (R-7)
crates/omsi-app/tests/
└── weather_lighting.rs                # NEW or extended: Hdr vs Linear
```

**Structure Decision**: the map is an Enhanced-only resource, so it lives in `modes/enhanced/`
next to the mode's other resources (`pipelines.rs`, `lights.rs`), built by `EnhancedResources`;
the cloud shape/extinction functions it needs stay in the core sky shader, which the mode already
uses.

## Delivery phases (one PR each, in order)

| Phase | Branch | Content | Spec | Gate |
| --- | --- | --- | --- | --- |
| P1 | `feat/enhanced-cloud-shade-map` | map pass + blur + redraw/drift, scene sun reads it, `cloud_t` out of the scene sun, texture budget + fallback, exposure 0.25 | US1, US4, FR-001..005, 008, 010, 011, 014, 015 | `cloud_shade` test; lavapipe + maintainer before/after (SC-005) |
| P2 | `feat/enhanced-cloud-height` | weather height to sky shader, CPU transmittance and map (no floor), edge radius from height | US3, FR-006, FR-007, SC-004 | edge-width test |
| P3 | `fix/enhanced-cumulus-ceu-cinza` | Hdr `apply_weather` greys by `closed` only; Hdr drops the `!overcast` shadow switch | US2, FR-009, FR-018, SC-002, SC-010 | `weather_lighting` test; Vanilla+ unchanged |
| P4 | `feat/enhanced-cloud-shade-moon` | map for the moon at night, moon even dimming by `closed` | US5, FR-016, FR-017, SC-009 | moon case of `cloud_shade` |
| P5 | `docs/spec-005-results` | in-game quickstart results, measurements table in this plan | SC-001..010 | maintainer's in-game checks |

P2 can be built before P1 if the maintainer prefers to see the height first; P3 depends on P1 (without
the map, removing the even dimming would leave cumulus skies with no darkening at all).

## Risks

- **Far border on long views from hills**: blend to the mean (R-3); fallback design: a second,
  coarse map (cascade) if Q1 views show the border.
- **Low clouds cut through scenery (Q2)**: accepted by the maintainer; the sky cube and the clouds
  drawn inside the view must not show artefacts at the layer crossing (check in Q3 of quickstart).
- **Exposure flicker** when the camera's cloud value crosses an edge: the existing 0.7 s smoothing
  of `cloud_t` and the slower dark adaptation stay.
- **Golden pictures**: Enhanced goldens (if any in `offscreen-golden`) change on purpose in P1/P3;
  update them in the same PR with before/after images.

## Complexity Tracking

No constitution violation to justify.
