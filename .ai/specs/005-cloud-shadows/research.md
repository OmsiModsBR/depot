# Research: Cloud shadows on the scenery in Enhanced

**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md) | **Date**: 2026-11-27

Code read at `main` 5feb348. Every decision below is Depot's own design (principle I); none
describes OMSI 2.

## R-1 Technique: a cloud shade map seen from the sun

**Decision**: compute the clouds' optical depth towards the sun once into a 2-D map laid out on the
clouds' base plane (a "cloud shade map"), and let every lit point read it once.

**Rationale**: it is the established real-time practice for cloud shadows on the ground:

- Unreal Engine's volumetric clouds offer "Beer Shadow Maps", a shadow map of the clouds from the
  light that casts shadows on the ground, described as faster but less accurate than ray marching
  per pixel ([Volumetric Cloud component](https://dev.epicgames.com/documentation/en-us/unreal-engine/volumetric-cloud-component-in-unreal-engine)).
- Unity HDRP's Cloud Layer computes a cloud shadow texture and applies it as the sun's light cookie,
  recomputed only when its inputs change; its Volumetric Clouds have their own cloud shadow map
  with a resolution and an area around the camera
  ([Cloud Layer](https://docs.unity3d.com/Packages/com.unity.render-pipelines.high-definition@17.0/manual/cloud-layer-volume-override-reference.html),
  [Volumetric Clouds](https://docs.unity3d.com/Packages/com.unity.render-pipelines.high-definition@17.0/manual/volumetric-clouds-volume-override-reference.html)).
- Depot's clouds already follow the Horizon Zero Dawn / Frostbite cloud model
  (`shaders/sky/sky_light.wgsl` header), whose shape and extinction functions the map reuses, so
  the shade matches the clouds drawn (FR-005).

The per-point cost is one bilinear read, against the nine comparison reads the sun shadow already
does (`sun_shadow_cascades`, 3x3 PCF in `modes/enhanced/shaders/enhanced.wgsl`).

**Alternatives considered**:

- *Ray march per pixel in the scene shader* (24 steps x 5 rays, as `cloud_sun_transmittance` does on
  the CPU): about 120 million texture reads per frame at 3440x1440, a cost that breaks SC-005.
- *Keep one value at the camera* (today): the reported bug.
- *Cumulus never removes the sun* (option B of the thread): no cloud shade at all; rejected by the
  maintainer in favour of the per-point shade.

## R-2 Map layout: indexed on the cloud base plane

**Decision**: the map covers a square of the clouds' base plane (height `H` above the map's height
zero, research R-11) centred on where the sun ray from the camera meets that plane. A point `p` (world, z up) with
sun direction `s` reads it at `p.xy + s.xy * (H - p.z) / s.z`. Each texel holds the optical depth
through the whole layer from the base along `s`.

**Rationale**: indexing on the base plane makes one map right for every height of receiver
(terrain on a hill, a roof, a bus) below the base: the lookup does the projection. It also makes
the cloud drift a plain offset: the map is drawn with the drift of its draw time, and the lookup adds
the drift since then, so the map needs redrawing only when the sun, the cover or the height change,
or the camera moves far (R-5).

For a point inside the layer (FR-006 sets no floor, so a hill or tower can reach into a low cloud),
the depth read is scaled by the share of the layer above the point, `(top - z) / (top - H)`
clamped to 0..1: a point near the top of a cloud gets less shade than one at its base. This is an
approximation (it assumes the cloud is even in height along that ray), accepted for a rare case.

**Alternatives considered**: a map on the ground plane (wrong for receivers far above or below the
reference ground); a cascaded map (Unreal's choice for large outdoor worlds, more cost and code for
a view that rarely passes 5 km in OMSI maps; kept as a fallback if SC-001 fails on the far views).

## R-3 Extent, resolution and the border

**Decision**: one 1024 x 1024 map of half-precision optical depth covering 12 km x 12 km (11.7 m
per texel). Beyond its border the lookup blends, over the last 10 % of the map, into the layer's
average transmittance for the current cover (`exp(-mean depth)` from the map's own mean, computed
when it is drawn). Memory: 2 MiB.

**Rationale**: the farthest scenery in an OMSI map is drawn at a few kilometres; at 12 km the border
is beyond the visible distance on nearly every map, and the blend hides it where it is not
(edge case "no visible border"). 11.7 m per texel is finer than the soft edge of a cloud shadow
(R-4), so no detail is lost.

**Alternatives considered**: 512 x 512 over 8 km (15.6 m texels, border visible on long views from
hills); 2048 x 2048 (8 MiB, no visible gain at the edge widths of R-4).

## R-4 Strength and soft edge (FR-003, FR-007, SC-004)

**Decision**:

- *Strength*: transmittance `T = exp(-tau)` from the stored optical depth `tau`, so thin edges and
  thin clouds shade a little and thick cores remove the sun (FR-003). The extinction is the sky's
  own `cloud_sigma` (base shape, cover, edge softness, bottom softness), without the detail volume
  (it erodes edges by tens of metres, below the soft edge width).
- *Soft edge*: the map is blurred, after drawing, with a separable filter whose radius in metres is
  `r = (H - g) * (tan(sun angular radius) + k_scatter)`, `g` the ground height under the camera.
  `tan(0.27 deg) = 0.0047` is the geometric penumbra of the sun's disc; `k_scatter = 0.01` stands
  for the light the cloud scatters forward round its own edge. At `H - g` = 1000 m the edge is about
  15 m wide, at 3000 m about 44 m: 2.9 times, satisfying SC-004 (at least twice). The blur is done in
  transmittance (not depth), so a thin gap between clouds does not fill in.
- The veil (cirrus) is not drawn into the map: its even dimming of the sun and its softening of the
  sun shadows stay as today (`sun_through_fog`, `enh.cloud_sun[1].w` in `enhanced.wgsl`).

**Rationale**: physically based, matches the sky's clouds by construction, and the edge width grows
linearly with height as FR-007 asks. `k_scatter` is a tuning value; the plan keeps it in one
constant, measured against SC-004 in the GPU test.

**Alternatives considered**: per-pixel PCF on the map with a radius from the receiver's height
(adds 4-8 reads per pixel for an effect that varies little across a view); no blur (aliased,
stair-stepped edges at 11.7 m texels).

## R-5 Redraw policy and drift

**Decision**: draw the map (optical depth pass + 2 blur passes) when any of these change since the
last draw: sun direction by more than 0.25 degrees, cover by more than 0.005, cloud height by more
than 1 m, a weather blend step, the camera's footprint moving more than 1/8 of the map, or the clouds
setting. Between draws, the lookup adds the drift since the draw. In a steady weather the map is
redrawn about every 30-60 s (the sun moves 0.25 degrees in one minute).

**Rationale**: Unity HDRP's Cloud Layer redraws only on input changes (R-1); the drift offset keeps
the patches moving smoothly without redrawing (SC-008).

**Alternatives considered**: redraw every frame (simplest, about 0.05 ms on the maintainer's card
by R-9, but on lavapipe in CI it would cost more than the whole budget); amortise over frames
(more state, no need given the redraw rate).

## R-6 What reads the map

**Decision**: the Enhanced scene shader multiplies the sun term by `T(p)` wherever it applies the
sun shadow (`sun_shadow_soft`, `sun_shadow_hard`, the rain light's sun term, leaves' transmitted
sun), and the moon term by the moon map (R-8). Mirrors and the reflection probe use the same
uniform and map (FR-011). The scene's sun uniform loses its `* cloud_t` (`renderer/sky.rs`, the
`sun:` field of `EnhancedUniform`).

**Rationale**: one place per light; the object shadows then show only where direct light arrives
and scale with it (FR-008).

**Found while reading** (not part of the feature, noted for its own fix): the mirror camera fills
`clouds.w` with `cloud_height` (`renderer/mirror.rs`), while the main camera uses `clouds.w` as the
SSAO switch (`renderer/camera_uniform.rs`, read in `enhanced.wgsl` and `shader.wgsl`). With a cloud
height above 0.5 m, mirrors therefore take the SSAO branch. Reported separately; this plan does not
change it.

## R-7 Overcast, closed deck, precipitation and the cumulus sky (FR-009, FR-018)

**Decision** (consolidated after the analyses of 2026-11-27, findings I1, G1, H1, I2):

- *Shadows switch*: in Enhanced (`ShadingPath::Hdr`), `lighting.shadows` no longer depends on the
  cloud type (`weather_setup.rs`: the `!overcast` and `closed < 0.5` terms go for Hdr; the visibility
  term `fog > 350 m` stays).
- *Direct light is cut once, by the map*: the closed share no longer dims `sun_intensity` in
  `apply_weather` (`omsi-app/src/world/domain/lights.rs`); the weather model's
  `sun_intensity = 1 - closed` goes for Hdr; `enhanced_sky_input` (`sky/air.rs`) no longer multiplies
  the reaching sun by `1 - closed`. Under a closed cover the map's optical depth removes the direct
  sun by itself (FR-009).
- *Precipitation*: rain or snow fall from a closed deck. In Hdr the map is drawn with the cover
  `max(cover, wet_cover)` (`wet_cover = clamp(1.5 * rain, 0, 1)`, as the sky already uses), so a
  rain under a `Cumulus` type shades like a deck; the even sun cuts by rain go for Hdr
  (`sun_intensity *= 1 - 0.75 * rain` in `apply_weather`, and `* (1 - wet_cover)` in
  `enhanced_sky_input`). Rain keeps greying the sky and its light and its part of the gloom.
- *Sky greying and gloom*: the sky and its light (`sky_color`, `ambient`, `secondary`, `fog_color`,
  the sky shader's grey dome) and the cloud part of the picture's gloom
  (`l.night = max(night, 0.45 * gloom)`) follow the closed share `smoothstep(0.85, 1.0, o)` (the
  same "closed" the sky shader uses) instead of `(o - 0.45) / 0.55`, so `Cumulus 1..3` (covers 0.35,
  0.55, 0.75) grey nothing and bring no night look by day (FR-018).
- *Veil*: the cirrus veil is not in the map; its even dimming of the sun (`veil_t`) and its
  softening of the sun shadows stay. It is the one cloud that dims direct light evenly, and it does
  so once (FR-009 as reworded).
- Vanilla+ keeps every current formula (FR-013).

**Rationale**: removes the double darkening (even dimming + per-point shade) the maintainer chose
to drop (Q3), lets the deck and the rain cast through the same rule as every cloud (FR-009), and
keeps the veil, which the map does not hold, as the only even cut.

## R-8 Moonlight (FR-016, FR-017)

**Decision**: a second map for the moon, drawn by the same passes with the moon's direction,
only while `Lighting::casts_moon_shadows()` would hold the moon as a light (moon up, sun down,
bright enough); the sun's map is not drawn then, so at most one map is drawn and both share one
texture (the map stores whichever body lights the scene; the uniform says which). The even dimming
`moon_disc * (1 - 0.9 * oc)` in `sky/atmosphere.rs` becomes plain `moon_disc`: the per-point map
removes the moonlight under clouds, a closed deck included (R-7 revision).

**Rationale**: sun and moon never both matter (spec edge case "sun and moon both up"), so one
texture serves both and the texture budget grows by one, not two.

## R-9 Performance (SC-005, principle VII)

**Estimates** (to be replaced by measurements in each PR):

| Item | Work | Estimate on an RTX 4070 Ti |
| --- | --- | --- |
| Depth pass | 1024² texels x 24 steps, 1-2 reads each of a 256² map | about 0.05 ms per draw |
| Blur | 2 passes x 1024² x up to 9 taps | about 0.02 ms per draw |
| Redraws | about once per 30-60 s in steady weather (R-5) | under 0.001 ms per frame on average |
| Scene | 1 bilinear read per lit fragment (2 MiB map, cache friendly) | under 1 % of the sun shadow cost |
| Memory | 2 MiB (R16F 1024²) + 2 MiB blur scratch | 4 MiB |

**Measurement plan**: (a) lavapipe in CI, 1280x720, the `bench_modos`-style scene of the render
modes survey, Enhanced, before/after each PR, median of 50 frames; (b) the maintainer's machine and
scenario (Desafio, MAN A23, `Cumulus 3`, 3440x1440, 8x MSAA), the frame time over 60 s before and
after, from the in-game fps counter or the debug timing log. Gate: at most +2 % (SC-005).

## R-10 Texture budget (FR-014)

**Decision**: the map is one more sampled texture in the Enhanced scene shader's camera group:
`ENHANCED_SAMPLED_TEXTURES` goes from 17 to 18 (`gpu/caps.rs`). A device that grants 17 but not 18
builds Enhanced without the map (a shader constant switches the read off, and the scene's sun keeps
today's camera value), and logs it once. A device below 17 behaves as today (Enhanced not made).

**Rationale**: desktop DX12 and Vulkan grant far more than 18; the fallback keeps Enhanced on a
device that has exactly 17 instead of losing it.

## R-11 Cloud height (FR-006)

**Decision**: the sky shader's `CLOUD_BOTTOM`/`CLOUD_TOP` constants become a base and top passed in
the Enhanced uniform: base = the weather's height (`Lighting::cloud_height`, already filled from
`[clouds]` and from a METAR base in metres), no floor; top = base + 1400 m (today's thickness). A
height that is absent, zero, negative or not finite uses 1400 m (today's base) silently. The CPU
`cloud_sun_transmittance` (still used for the exposure, R-12) takes the same base and top. A height
change redraws the sky cube and the map.

The height is measured from the map's height zero (finding A1): stable while the camera climbs a
hill, and the only reference a weather file or a METAR can give. The sky shader today draws no
cloud when the camera is above `CLOUD_BOTTOM` (`cloud_layer` returns early); with low bases
allowed (Q2), the camera can stand above or inside the layer on a hilly map, so the march MUST
start at the camera when it is inside the layer and look down through the base when it is above
it (finding U1, FR-005).

## R-12 Exposure (FR-010, Q1)

**Decision**: `CLOUD_SHADE_ADAPT` (`sky/air.rs`) goes from 0.5 to 0.25; the exposure keeps reading
the camera's own cloud transmittance from `cloud_sun_transmittance` (or the map at the camera: same
value, R-1), for the sun by day and the moon by night.
