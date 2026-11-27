# Data model: Cloud shadows on the scenery in Enhanced

**Spec**: [spec.md](spec.md) | **Plan**: [plan.md](plan.md)

## Cloud layer (per frame, CPU)

| Field | Source | Rule |
| --- | --- | --- |
| `cover` | `Lighting::cloud_density` (weather type: Cumulus 1/2/3 = 0.35/0.55/0.75, Overcast = 1; or the weather model) | 0..1 |
| `base` (m above the map's height zero) | `Lighting::cloud_height` (`[clouds]` height, METAR base in metres) | no floor; absent/zero/negative/non-finite -> 1400 |
| `top` (m) | `base + 1400` | |
| `drift` (m) | `Lighting::cloud_offset * 2500` | moves the map lookup between draws |
| `closed` | `smoothstep(0.85, 1.0, cover)`, or the model deck's closed share | greys the sky and its light and drives the gloom; never dims direct sun or moon (R-7 revision) |
| `light_dir` | sun direction by day; moon direction while the moon is the scene's light | the map is drawn for this body |

## Cloud shade map (GPU)

| Field | Value |
| --- | --- |
| Format | R16Float, 1024 x 1024 (+ one scratch target of the same size for the blur) |
| Content | transmittance after the blur (R-4); before the blur, optical depth from the base along `light_dir` |
| Extent | 12 km square of the base plane, centred where the light ray from the camera meets it |
| Lookup | `uv = (p.xy + light_dir.xy * (base - p.z) / light_dir.z + drift_since_draw - centre) / 12 km + 0.5` |
| Inside the layer | depth scaled by `clamp((top - p.z) / (top - base), 0, 1)` |
| Outside the map | blend over the last 10 % to `mean_t` (the map's mean transmittance) |
| Valid for | `light_dir`, `cover`, `base` and centre of its draw (redraw rules: research R-5) |

## Uniform additions (Enhanced)

| Field | Meaning |
| --- | --- |
| `cloud_shade.xy` | map centre on the base plane (world, camera-relative) |
| `cloud_shade.z` | 1 / extent |
| `cloud_shade.w` | `mean_t` |
| `cloud_layer.xy` | base, top (m) |
| `cloud_layer.zw` | drift since the draw (m) |
| `cloud_light` | 0 = map holds the sun, 1 = the moon, -1 = no map (fallback, R-10) |

## State transitions

- *No map* (clouds off, cover 0, light below the horizon, fallback device) -> the read returns 1.
- *Map valid* -> redraw when a R-5 trigger fires; between draws only the drift offset changes.
- *Day to night*: when the moon becomes the scene's light (`casts_moon_shadows`), the map is redrawn
  for the moon; back at dawn, for the sun.
