# Research: Simplify the graphics modes

Phase 0 of the [plan](plan.md). Code read at `19290ac` (2026-11-08). "OMSI 2" on this page is what
the original game is expected to show and **still has to be checked in the game** (Principle II,
#49).

## Baseline measurements

The same synthetic scene (an 800 m ground and 1,600 instanced boxes, daylight, no fog) drawn with
`Renderer::render_to_image` at 1280x720, MSAA 1, shadow map 2048, on lavapipe (Mesa 25.2.8, 4 CPU
cores), one process per mode, 15 measured frames after 3 warm-up frames. Medians in ms per frame:

| Case | ms |
|---|---:|
| Vanilla+ without shadows or AO | 25 |
| Vanilla (wgpu) | 28-33 |
| Vanilla+ with shadows | 39 |
| Vanilla+ with SSAO | 54 |
| Vanilla+ with shadows and SSAO | 60 |
| Enhanced without shadows, AO or FXAA | 114 |
| Enhanced with shadows, SSAO and FXAA | 147 |
| Enhanced+ (ray queries active on lavapipe) | 300-345 |

A software adapter makes per-pixel work weigh far more than on a graphics card, so these numbers
compare the modes with each other only. DirectX9 could not be measured (Windows only): see R-4.
Two observations matter for the plan:

- wgpu Vanilla was slower than Vanilla+ in every run. Its `classic` branch decodes, re-encodes and
  decodes sRGB per pixel to work in gamma space, which DirectX9 does natively. Phase 6 removes it.
- On a computer every mode builds Enhanced's pipelines too (`renderer/build.rs`,
  `leave_out_enhanced`): on lavapipe opening the renderer takes about 37 s in any mode. Phase 7.

## R-V. wgpu Vanilla x DirectX9: what each draws

wgpu Vanilla is `shader.wgsl` with its `classic` branch on (`camera.sky_color.w = 1`). DirectX9 is
`scene.hlsl`, a hand port of that branch, fed by the same `FramePlan` from `omsi-render`. The scene,
culling, level of detail, alpha order, light list, coronas, smoke and sun glare are worked out once
in `omsi-render` and serve both: the differences lie only in what each backend draws and how it
blends colour.

### The same in both

Gamma-space shading (texture times vertex light clamped at 1), fog mixed in gamma, the light map
(`ADDSMOOTH`), the night map, the tile light maps, the lamp grid, `[interiorlight]`, `[matl_envmap]`
with its mask and bump map, vertex specular, transmaps, texture addressing, the sky from the three
`envir.cfg` pictures, sun and moon (`sonne.bmp`/`mond.bmp`, #686), the stars from `stars.dat`
(#700), the weather's cloud layer, the sun glare as a sprite (#691), `[smoke]`, flat mirrors; no sun
shadows, no SSAO, no procedural detail texture, no snow cover, no moonlight.

### Different

| # | Feature | wgpu Vanilla | DirectX9 | Closer to OMSI 2 (to validate) | Proposal |
|---|---|---|---|---|---|
| V1 | `[matl_raindropmap]` drops on the glass, cleared by the wiper | — | ✓ (`glass_rain`, `dx9_glass_rain`) | **DirectX9**: it is an OMSI 2 material key | keep |
| V2 | `[light_enh]` coronas blended over the scene | in linear light (sRGB target) | in gamma (#697) | **DirectX9**: #697 was made to match OMSI 2 | keep |
| V3 | Wet road sheen and puddle reflections in the rain | ✓ (`shader.wgsl`, `effects/puddles.rs`) | — (README: wet sheen not yet) | likely **DirectX9** (no puddles); check whether OMSI 2 darkens or shines a wet road | if OMSI 2 has a sheen, `fix/dx9-wet-road` |
| V4 | Tyre spray on wet roads | ✓ (a Depot addition) | — | **DirectX9** (the code calls it a game addition) | keep |
| V5 | Fair-weather clouds in a clear sky without weather | ✓ | — (`fair_weather_clouds` false) | **DirectX9** (a Depot addition) | keep |
| V6 | Trees in the wind | option | locked off | likely **DirectX9** (static) | keep |
| V7 | FXAA without MSAA | ✓ (`vanilla_fxaa`) | — (MSAA only; bilinear or FSR1 upscale below scale 1) | **DirectX9** (OMSI 2 offers MSAA only) | keep |
| V8 | Automatic render scale | ✓ | only the player's choice | no difference to OMSI 2 | keep |
| V9 | Mip chains of non-DDS textures | filtered in linear light | filtered on the gamma values (README) | likely **DirectX9** | keep; check thin textures (fences, leaves) |
| V10 | Depth-only pass of the transmaps' fully opaque parts | ✓ | — (README: missing) | **wgpu**, if OMSI 2 hides what lies behind a transmap's opaque parts | `fix/dx9-transmap-depth` if validation shows a difference |
| V11 | Depth-only surface draws use the terrain uv | correct | scaled terrain uv (README) | **wgpu** | `fix/dx9-surface-uv` (small) |
| V12 | Fog through the player's vehicle | handled | — (README: not yet) | to check | decide after validation |
| V13 | Falling snow | its own pass | `dx9_snow` option, drawn as coronas | to check | keep if approved |
| V14 | Real-time shadows of `[shadow]` meshes | — (off in Vanilla) | — (README: later phases) | **neither**, if OMSI 2 has them: `RenderOptions::omsi_shadow_casters` says "as in OMSI 2" while `api_limits` calls shadows "an improvement OMSI 2 does not have"; the two texts contradict each other | validate; if OMSI 2 has them, a DirectX9 gap outside this plan |
| V15 | Interface on an sRGB target | as `omsi-ui` | the same where the device converts after blending; in gamma elsewhere (edges differ) | no difference | keep |
| V16 | VR and triple screen | ✓ | — | — | decision D3 |
| V17 | Effort-saving options (`fewer_night_lights`, texture limit, stars, cloud picture) | — | ✓ | Depot additions | keep (options, off by default) |
| V18 | The launcher's showroom preview | drawn as Vanilla when the game is in Vanilla (`launcher_statics` sets `CLASSIC`) | the preview is always wgpu | — | after phase 6 the preview uses Vanilla+; accept it, or draw the preview with DirectX9 (costly) |

**Decision**: DirectX9 is closer to OMSI 2 in V1, V2, V4, V5, V7 and likely V3, V6 and V9. wgpu is
ahead only in V10, V11 and maybe V12, which are gaps of the port and become `fix/dx9-*` pull requests
(phase 4) if validation shows a visible difference. V14 is an open question for both.

**Rationale**: DirectX9 was built under the rule "draw only what OMSI 2 has", and the latest parity
fixes (#686, #691, #697) were made there first. wgpu Vanilla inherited Vanilla+ additions (puddles,
spray, fair-weather clouds) that are not OMSI 2's.

**Alternatives considered**: keep wgpu Vanilla as a "modern Vanilla" next to DirectX9: rejected by
the request (Vanilla+ already is "OMSI 2 on a modern API") and by Principle X (two implementations of
the same frame).

## R-E. Enhanced x Enhanced+, with a focus on transparency

Enhanced+ is Enhanced plus three ray passes (`effects/rt.rs`): the sun's shadow and the sky's
occlusion per pixel, and reflections. Both sort materials by the **same rule**, written twice
(`renderer/lists.rs` for the shadow map, `RayTracer::traced_ranges` in `rt.rs`): opaque = solid;
alpha-tested = cut-out; blended with a transmap and writing depth = cut-out (a body painted through
a transmap); any other blend = glass (or a decal, dirt, a rain film).

| # | Topic | Enhanced | Enhanced+ | What Enhanced takes, without rays |
|---|---|---|---|---|
| E1 | **Sunlight through glass** (bus and car windows) | a pane casts no shadow at all: the sun comes through a window in full | a pane tints the shadow: 45 % of the sun gets through (`rt.wgsl`, one fixed value for every pane) | **Yes.** A transmission term: the glass materials drawn into an extra shadow cascade (or into a colour target beside the shadow map), and `sun_shadow_soft` multiplied by the transmission. Better than Enhanced+: use the material's alpha/colour instead of a fixed 0.45 |
| E2 | Cut-out shadows (leaves, fences) | shadow map with the alpha test | the same: Enhanced+ keeps cut-outs in the shadow map because rays cannot read their texels | nothing |
| E3 | Solid shadows | cascaded shadow map (PCF edges) | a ray per pixel up to 420 m (an edge that softens with the distance to the caster), the map beyond | no (ray tracing) |
| E4 | Ambient occlusion | SSAO from the prepass depth: cut-outs occlude by their exact texels; glass and blended layers do not occlude and take the AO of the surface under them (`tests/decal_ao.rs`) | rays against the structures: cut-outs occlude stochastically by the texture's **mean** alpha; glass is not seen | nothing: for transparent surfaces SSAO is already the more faithful one |
| E5 | Reflections | sky probe + `[matl_envmap]` maps + screen-space puddles | traced: on screen the picture itself; off screen the hit mesh's **mean colour**; glass not reflected | optional: screen-space reflections over the probe give the "on screen" part without rays. A new feature with its own cost (Principle VII), outside this plan |
| E6 | Post processing | tone mapping and contrast | + colour grade, vignette and sharpening (`passes/post.rs`) | decision D2 (aesthetic) |
| E7 | Glass lit by the sun (specular glint) | `sun_shadow_hard` (one compare) | the same | nothing |

**Decision**: bring E1 to Enhanced (phase 2). E6 waits on decision D2. E5 stays a candidate for a
later Enhanced feature. The rest of Enhanced+ is removed.

**Rationale**: E1 is the one transparency difference where Enhanced+ is right and Enhanced is
visibly wrong (a cab far too bright in the sun). In E2 and E4 Enhanced already handles transparent
surfaces as well or better.

**Alternatives considered**: keep ray tracing behind a hidden option: rejected (Principle X: nothing
is kept behind a `cfg` or an option for later; and the request rules ray tracing out).

## R-1. Where the mode type lives

**Decision**: `GraphicsMode` in `omsi-cfg` (new module `graphics.rs`), with `parse(&str)` taking the
old spellings and `as_str()`.

**Rationale**: `omsi-app`, `omsi-launcher-core` and `omsi-launcher-ui` already depend on `omsi-cfg`;
today `graphics_mode` is copied in the app and in the launcher core, and 21 comparisons with the
literal names are spread over the three crates.

**Alternatives considered**: in `omsi-launcher-core` (the launcher UI would depend on the core for
this alone); in `omsi-render` (the renderer must not know about backends or DirectX9).

## R-2. What the wgpu renderer needs to know

**Decision**: `Lighting` and the camera uniform take a `ShadingPath { Linear, Hdr }` (Vanilla+ and
Enhanced). After phase 6 there is no "classic" path in wgpu. DirectX9 keeps taking `omsi-render`'s
`FramePlan`, which runs on wgpu's noop device as today.

**Rationale**: `omsi-render` does not need to know that a Vanilla mode exists; the gamma path then
lives in `scene.hlsl` only.

## R-3. Migration of saved settings

**Decision**: done when the file is read, without rewriting it until the player saves:
`graphics=enhanced_plus` (and its old spellings) → `enhanced`; `graphics=vanilla` → Vanilla, which
now means DirectX9 whatever `graphics_api` says; `graphics_api=d3d9` → see
[contracts/settings-cfg.md](contracts/settings-cfg.md).

**Rationale**: Principle IV (an invalid value never crashes), and no player is left in a mode that no
longer exists.

## R-4. DirectX9 against wgpu Vanilla (NEEDS CLARIFICATION, resolved as a task)

Not measurable without Windows. **Decision**: phase 0 measures on the maintainer's machine, same
map and stretch, `OMSI_PROFILE=1`, one session with `graphics=vanilla` on DirectX 12 and one on
DirectX9, comparing the `profile window:` lines (#683). Criterion: phase 5 goes in only if DirectX9 is
not slower (Principle VII); if it is, the difference becomes a `fix/dx9-*` first.

## R-5. DirectX9 in CI

**Decision**: unchanged (`NullBackend`, no GPU). A DirectX9 golden on a Windows runner depends on
the runner's basic adapter opening Direct3D 9Ex with Shader Model 3.0, which was not checked; this
plan does not need it.

## R-6. Refactor of the code that stays

The initial analysis of the modes (2026-11-07) listed seven points. How each is handled:

| Point | Finding (at `a132fb0`) | Where it lands |
|---|---|---|
| R1. A type for the mode | the mode as a `String`, a redundant `bool`, three global `AtomicBool`s, bools in `Lighting`/`RenderOptions`, a bit in `camera.sky_color.w`; `graphics_mode` copied in two crates; six ways in | phase 3 ([data-model.md](data-model.md)) |
| R2. Bools as parameters | `encode_main_pass` takes 22 parameters, 9 of them bools; `encode_main_pass`, `encode_enhanced_post` and `encode_glass` together take 28. `FrameFeatures` already exists but the passes get its fields one by one, and the decisions behind it sit inside `begin_frame` | phase 8 |
| R3. Long functions | `begin_frame` 507 lines, `Renderer::build` 504, `encode_main_pass` 478, `open_on` 432, `visibility` 324, `encode_frame` 295, `draw_scene_inner` (DirectX9) 271, `add_material_inner` 247; `effects/rt.rs` `prepare` 285 and `new` 268 go with phase 1 | phases 10, 11, 12 (the rest noted for later) |
| R4. Enhanced pipelines built in every mode | `leave_out_enhanced` true only on Android/OpenGL | phase 7, after phase 10 |
| R5. Environment flags in the renderer | `omsi-render` reads 62 different `omsi_cfg::flags`, several on every frame; `omsi-render-d3d9` reads none; `OMSI_FAKE_GPU_ERROR` hooks in 22 places across 6 files | phase 9 (the mode flags already go in phases 1 and 3) |
| R6. Vanilla x DirectX9 parity without a test | `scene.hlsl` is a hand port of the `classic` branch; fixes were made twice by hand (#686, #691, #697) | phase 6 removes the second copy; phase 12 tests the plan translation |
| R7. Organisation and documents | tests in `src/` (3 large files, 30 files with `#[cfg(test)]` in the two crates); `omsi-render-d3d9/README.md` and the `d3d9.rs` header out of date | phases 13, 14 |

Also removed on the way, by Principle X: the never-taken `camera.flags.z` branch of `shader.wgsl`
(kept only for a Metal compiler quirk; Windows is the only supported platform) in phase 6, and the
second copy of the material classification rule in phase 1.

**Decision**: refactor after the removals, one concern per pull request, in the order 8 → 9 →
10 → 7 → 11 → 12; 13 and 14 whenever convenient.

**Rationale**: refactoring code that phase 1 or 6 deletes is wasted work; phase 8 first because the
pure `FrameFeatures::decide` makes the later splits checkable by unit tests; phase 10 before 7
because leaving a shading path out is a one-line choice once each pass builds its own pipelines.

**Alternatives considered**: one large "render cleanup" pull request: rejected (Principle IX, and a
large diff makes a pixel change hard to find). A shared backend trait for wgpu and DirectX9:
rejected, as in `docs/MACRO_REFACTOR_DRAFT.md` (only two implementations, and the plan hand-off
already works).
