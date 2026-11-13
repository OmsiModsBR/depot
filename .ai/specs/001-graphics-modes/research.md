# Research: Simplify the graphics modes

Research for the [plan](plan.md), which implements the [spec](spec.md). Sections R-V to R-6 were written at
`19290ac` (2026-11-08) and re-checked at `cbeda16` (2026-11-13): nothing of the plan had been built
yet, so their findings stand; line counts are those of `19290ac`. Sections R-7 to R-12 were added at
`cbeda16` for the revised scope (one directory per mode, the options per mode). "OMSI 2" on this page is what
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
  decodes sRGB per pixel to work in gamma space, which DirectX9 does natively. Phase A6 removes it.
- On a computer every mode builds Enhanced's pipelines too (`renderer/build.rs`,
  `leave_out_enhanced`): on lavapipe opening the renderer takes about 37 s in any mode. Phase C6.

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
| V16 | VR and triple screen | ✓ | — | — | not offered in Vanilla (clarified 2026-11-13, FR-018a) |
| V17 | Effort-saving options (`fewer_night_lights`, texture limit, stars, cloud picture) | — | ✓ | Depot additions | keep (options, off by default) |
| V18 | The launcher's showroom preview | drawn as Vanilla when the game is in Vanilla (`launcher_statics` sets `CLASSIC`) | the preview is always wgpu | — | after phase A6 the preview uses Vanilla+; accept it, or draw the preview with DirectX9 (costly) |

### Checked in the game by the maintainer (T001, 2026-11-13)

| # | OMSI 2 | Outcome |
|---|---|---|
| V1, V2, V4, V5, V7, V8 | as DirectX9 draws it | **keep** DirectX9's behaviour; no fix needed |
| V6 | no wind in the trees | **keep** DirectX9's (static trees); `windy_trees` stays fixed "Off" in Vanilla |
| V14 | has dynamic shadows, switched on by a setting; meshes marked `[shadow]` are shadows that are always on | neither implementation draws them in Vanilla, so this does not decide between the two (FR-007) and does not hold phase A5. It is a **gap of Vanilla against OMSI 2**. The text "an improvement OMSI 2 does not have" for the shadow options is wrong and is corrected when the option table is built (phase B1): in Vanilla they are "not drawn by Vanilla yet". Drawing them is a feature of its own, **outside this feature** (decided by the maintainer on 2026-11-13): it gets its own issue and specification |
| V13 | falling snow is sprites of a snowflake picture, carried by the current weather's wind; the flakes do not come inside the player's vehicle | **keep** DirectX9's: it draws sprites of the game's snowflake picture, moved by the wind and kept out of the vehicles (`omsi-app` `weather/implementation/rain.rs`); wgpu Vanilla draws its own flakes in a pass of its own. One difference is left: DirectX9's snow is an option that is **off by default** (`dx9_snow`), while in OMSI 2 it snows without asking. **Decided** (maintainer, 2026-11-13): on by default in Vanilla (`fix/dx9-snow-default`, phase A4). In OMSI 2 the snow depends on the weather alone (the weather set by hand can also switch snow or rain on). Whether the option survives at all as one of Vanilla's effort-saving switches, like the coronas and the stars, is settled by the options audit (phase B1) |
| V3 | a wet road has puddles that mirror the houses and the sky (confirmed by the maintainer, with screenshots); the puddles mirror people as well when a setting in the menu asks for it | **wgpu Vanilla is the side that has it**; DirectX9 lacks it (its README: "wet sheen not yet"). By FR-007 it is brought to DirectX9 before Vanilla changes renderer: `fix/dx9-wet-road` (phase A4), the largest item of that phase. What the reflections include (vehicles, people) follows OMSI 2's own settings; which of them Depot already reads is settled when the fix is planned |
| V12 | **inconclusive**. The thickest fog the weather allows has a 50 m range, too thin over the length of a cab to tell from an inside camera. From an outside camera, with a door open, the fog appears to lie inside the vehicle as well | no `fix/dx9-*`: nothing seen says OMSI 2 keeps the fog out of the player's vehicle, and the outside view suggests it does not. DirectX9's behaviour (the fog falls on the vehicle's inside like on anything else at that distance) stays; the point remains "to validate" and no public text claims either way (FR-032) |
| V9 | three screenshots of a cobbled yard compared (OMSI 2, DirectX9, DirectX 12), 2026-11-13. They do not share camera, weather or season, so the mip filtering cannot be told apart from the lighting: the cobbles' mean brightness reads 0.47-0.48 in OMSI 2, 0.53-0.55 in DirectX9 and 0.50-0.53 in DirectX 12. Between the two Depot pictures the difference is 1 to 2 % | **keep** DirectX9's: the two implementations are not visibly different here, so nothing is lost by the change of renderer, and no fix follows. Which of them matches OMSI 2 stays "to validate" (FR-032) |
| V10, V11 | not checked yet | still open; both decide further `fix/dx9-*` pull requests of phase A4 |

Seen in the same screenshots, outside the V rows and **not yet confirmed** (the cameras differ): the
far hills and the grass are darker and browner in DirectX9 than in DirectX 12 at what looks like the
same hour; OMSI 2's cobbles close to the camera are sharper and show more colour grain than either
Depot picture. Both need a same-camera, same-weather pair before they become rows of this table.

**Decision**: DirectX9 is closer to OMSI 2 in V1, V2, V4, V5, V6, V7 and V8 (checked), and likely V3 and V9. wgpu is
ahead only in V10, V11 and maybe V12, which are gaps of the port and become `fix/dx9-*` pull requests
(phase A4) if validation shows a visible difference. V14 is an open question for both.

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
| E6 | Post processing | tone mapping and contrast | + colour grade, vignette and sharpening (`passes/post.rs`) | nothing: removed with Enhanced+ (clarified 2026-11-13) |
| E7 | Glass lit by the sun (specular glint) | `sun_shadow_hard` (one compare) | the same | nothing |

**Decision**: bring E1 to Enhanced (phase A2). E6 is not carried over (clarified 2026-11-13: only the glass is). E5 stays a candidate for a
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
Enhanced). After phase A6 there is no "classic" path in wgpu. DirectX9 keeps taking `omsi-render`'s
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

Not measurable without Windows. **Decision**: phase A0 measures on the maintainer's machine, same
map and stretch, `OMSI_PROFILE=1`, one session with `graphics=vanilla` on DirectX 12 and one on
DirectX9, comparing the `profile window:` lines (#683). Criterion: phase A5 goes in only if DirectX9 is
not slower (Principle VII); if it is, the difference becomes a `fix/dx9-*` first and is measured
again, with no tolerance (clarified 2026-11-13, FR-008).

## R-5. DirectX9 in CI

**Decision**: unchanged (`NullBackend`, no GPU). A DirectX9 golden on a Windows runner depends on
the runner's basic adapter opening Direct3D 9Ex with Shader Model 3.0, which was not checked; this
plan does not need it.

## R-6. Refactor of the code that stays

The initial analysis of the modes (2026-11-07) listed seven points. How each is handled:

| Point | Finding (at `a132fb0`) | Where it lands |
|---|---|---|
| R1. A type for the mode | the mode as a `String`, a redundant `bool`, three global `AtomicBool`s, bools in `Lighting`/`RenderOptions`, a bit in `camera.sky_color.w`; `graphics_mode` copied in two crates; six ways in | phase A3 ([data-model.md](data-model.md)) |
| R2. Bools as parameters | `encode_main_pass` takes 22 parameters, 9 of them bools; `encode_main_pass`, `encode_enhanced_post` and `encode_glass` together take 28. `FrameFeatures` already exists but the passes get its fields one by one, and the decisions behind it sit inside `begin_frame` | phase C1 |
| R3. Long functions | `begin_frame` 507 lines, `Renderer::build` 504, `encode_main_pass` 478, `open_on` 432, `visibility` 324, `encode_frame` 295, `draw_scene_inner` (DirectX9) 271, `add_material_inner` 247; `effects/rt.rs` `prepare` 285 and `new` 268 go with phase A1 | phases C3, C7, C8 (the rest noted for later) |
| R4. Enhanced pipelines built in every mode | `leave_out_enhanced` true only on Android/OpenGL | phase C6, after phase C3 |
| R5. Environment flags in the renderer | `omsi-render` reads 62 different `omsi_cfg::flags`, several on every frame; `omsi-render-d3d9` reads none; `OMSI_FAKE_GPU_ERROR` hooks in 22 places across 6 files | phase C2 (the mode flags already go in phases A1 and A3) |
| R6. Vanilla x DirectX9 parity without a test | `scene.hlsl` is a hand port of the `classic` branch; fixes were made twice by hand (#686, #691, #697) | phase A6 removes the second copy; phase C8 tests the plan translation |
| R7. Organisation and documents | tests in `src/` (3 large files, 30 files with `#[cfg(test)]` in the two crates); `omsi-render-d3d9/README.md` and the `d3d9.rs` header out of date | phases C9, C10 |

Also removed on the way, by Principle X: the never-taken `camera.flags.z` branch of `shader.wgsl`
(kept only for a Metal compiler quirk; Windows is the only supported platform) in phase A6, and the
second copy of the material classification rule in phase A1.

**Decision**: refactor after the removals, one concern per pull request, in the order C1 → C2 →
C3 → C4 → C5 → C6 → C7 → C8; C9 and C10 whenever convenient. C4 and C5 are the per-mode structure
of R-7 to R-9.

**Rationale**: refactoring code that phase A1 or A6 deletes is wasted work; phase C1 first because the
pure `FrameFeatures::decide` makes the later splits checkable by unit tests; phase C3 before C6
because leaving a shading path out is a one-line choice once each pass builds its own pipelines.

**Alternatives considered**: one large "render cleanup" pull request: rejected (Principle IX, and a
large diff makes a pixel change hard to find). A shared backend trait for wgpu and DirectX9:
rejected, as in `docs/MACRO_REFACTOR_DRAFT.md` (only two implementations, and the plan hand-off
already works).

## R-7. Where the line between Vanilla+ and Enhanced runs today

Read at `cbeda16`. Both modes are one `Renderer` on one device; the mode is the bool
`Lighting::enhanced`, tested in 46 places of `renderer/frame.rs` alone.

| What | Vanilla+ | Enhanced | Shared today |
|---|---|---|---|
| Surface shading | `fs_main` (`shader.wgsl`): linear, texture times light | `fs_enhanced` (`enhanced.wgsl`, `enhanced_common.wgsl`): GGX, spherical harmonics, probe, pre-exposed HDR | the vertex shader, the shadow, depth and transmap entry points, the structs and bindings (all in `shader.wgsl`) |
| Sky | `sky.wgsl` (the `envir.cfg` pictures) | `sky_enhanced.wgsl`, `sky/atmosphere.rs`, `sky/air.rs`, `sky/clouds.rs`, `sky/probe.rs` | stars, sun and moon sprites, snow geometry |
| Target | the window's format (plus a scale target) | `targets/hdr.rs` (HDR chain, glow levels) | depth, MSAA, AO targets, the cache |
| After the scene | FXAA when there is no MSAA (`vanilla_fxaa`) | `passes/post.rs` (glow, metering, tone map, FXAA), `lighting/exposure.rs`, `passes/fog_lamps.rs` | upscale, overlay, HUD (`output`) |
| Puddles | their own pass after the scene (`reflection_frame`, `fs_vanilla_reflections`) | inside the main pass (masked frame) | `effects/puddles.rs` (trace, blur, resolve), the vehicle capture |
| Lights, sprites | `fs_main`, `fs_smoke`, `fs_snow`; `gpu_light(.., false)` | `fs_enhanced`, `fs_smoke_enhanced`, `fs_snow_enhanced`; `gpu_light(.., true)`, `linear_light_colour` | the light grid, lamp shadows, corona and smoke geometry |
| Shadows, prepass, SSAO, glass, culling, level of detail, assets | — | — | all shared |

Three files hold code of both modes in one unit: `shader.wgsl` (both fragment paths' helpers),
`corona.wgsl` and `snow.wgsl` (one fragment entry per mode), and on the Rust side `renderer/build.rs`,
`renderer/frame.rs`, `renderer/encode.rs`, `renderer/passes.rs`, `passes/main.rs`, `lighting/model.rs`
and `effects/puddles.rs` (branches on `enhanced`).

**One dependency crosses the line**: in Enhanced the mirrors' pictures are drawn with the plain
shading (`fs_main`), not the enhanced one (`renderer/frame.rs`, `enhanced_frame`): 0.6 ms against
12.7 ms for one 256-pixel mirror, by the comment's own measurement. So Enhanced runs the plain
surface shading every frame a mirror is in view.

## R-8. The plain shading is shared core, not Vanilla+'s

**Decision**: the plain surface shading (`fs_main` and what it calls) stays in the shared core as the
**base shading**. `modes/vanilla_plus/` holds what only Vanilla+ runs: its frame decisions, the sky
from the `envir.cfg` pictures, FXAA without MSAA, the puddles' own pass and the non-HDR sprite
entries. `modes/enhanced/` holds the HDR shading, the atmosphere, the probe, the post passes, the
exposure, the lamps in fog and the HDR targets, and asks the core for the base shading when it draws
a mirror.

**Rationale**: FR-021 puts in a mode's directory the code that **only** that mode runs; the base
shading is run by both, so by the spec's own rule it is core. It also keeps rule 1 (the modes never
import each other) without a pixel or a millisecond changing (FR-030).

**Alternatives considered**:

- Enhanced draws its mirrors with the enhanced shading: rejected, a measured regression (Principle
  VII).
- Enhanced gets a small mirror shader of its own: rejected, a second copy of the plain shading to
  keep in step (Principle X), and the mirrors' pixels would change.
- Vanilla+ owns the plain shading and Enhanced imports it: rejected, it breaks rule 1 of the
  structure.

**Agreed by the maintainer on 2026-11-13 (D5)**: `modes/vanilla_plus/` is therefore smaller than the
tree in the spec suggests ("its shaders" there are the sky, the FXAA step, the puddles' pass and the
sprite entries, not the scene's fragment shader).

## R-9. The mode interface

**Decision**: `interface/mode.rs` declares a trait the two modes implement, and the data types the
core hands them (the parts structs the passes already take: `MainParts`, `PostParts`, `GroupParts`).
The renderer holds one boxed mode, built by the one function that names both
(`renderer/build.rs`). What the trait asks of a mode:

| Step | Vanilla+ | Enhanced |
|---|---|---|
| build | its pipelines and fixed resources | its pipelines, look-up tables, probe, noise |
| features for a frame | adds FXAA-without-MSAA and the puddles' pass to the shared decisions | adds the HDR frame, the masked frame, the lead-view updates |
| prepare | nothing | atmosphere, probe redraw, exposure |
| light records | plain colours | linear colours, spot profiles |
| scene target and pipelines for a view | window format | HDR for the window and headset, base shading for mirrors |
| after the scene | puddles' pass, FXAA | fog lamps, glow, metering, tone map, FXAA |

`interface/frame_plan.rs` takes `FramePlan` and its parts out of `renderer/mirror.rs`: it is data
only (ids, matrices, lists), the contract with the Vanilla renderer.

**Rationale**: Principle V allows a trait where there are two real implementations, and there are.
A trait object costs one indirect call per step, about ten per frame and per view, against thousands
of draw calls; phase C4 still carries a before/after frame-time measurement.

**Alternatives considered**: an enum with a `match` per step: the renderer would then name both
modes at every step instead of once at build. Generic `Renderer<M>`: the game would carry the type
through `omsi-app`, for no measurable gain.

**Layer order** (for `scripts/check-render-layers.sh`): `gpu < shaders < assets | lighting < sky <
scene < targets < passes < effects < interface < modes < output < renderer < api`. `interface` names
only lower layers; `modes/vanilla_plus` and `modes/enhanced` count as two domains of one layer, so the
existing "same layer does not import each other" rule covers rule 1, and "imports point down" covers
rule 2.

## R-10. WGSL per mode

Shaders are assembled by concatenating `include_str!` sources (`shaders/mod.rs`). **Decision**: the
core keeps `shaders/scene/` (structs, bindings, vertex shader, shadow and depth entries, base
shading); each mode keeps its sources under `modes/<mode>/shaders/` and assembles its module as core
sources + its own. `corona.wgsl` and `snow.wgsl` are split into a common part (vertex stage, shape)
and one fragment file per mode. `enhanced_common.wgsl`, `sky_enhanced.wgsl`, `post.wgsl` and
`fog_lamps.wgsl` move whole to Enhanced; `sky.wgsl` to Vanilla+ if the move confirms Enhanced's
mirrors do not use it (`sky_mirror_pipeline` suggests they use `fs_enhanced_mirror`).

**Rationale**: text moved between files gives the same module text in the same order, so naga's
output and the pixels do not change; `cargo test -p omsi-render` validates every assembled module.

## R-11. The options per mode: what the code does today

Read at `cbeda16`. The launcher lists its options in `omsi-launcher-ui/src/window/domain/settings.rs`
(sections Graphics, Advanced graphics, Display and VR) and decides visibility in `shown()` from two
sources: the `api_limits::D3D9` table (hidden, locked, fewer choices) and loose rules on `graphics`
(`classic`, `traced`). The in-game menu builds a **separate, hand-written** list
(`omsi-app/src/hud/implementation/lists/pages.rs`) with `limit_or` for the same table and its own
`Backend::D3d9` tests. Nothing ties the two lists together, and no table says what Vanilla+ and
Enhanced differ in.

Applicability (✓ applies, — does not apply, `=x` fixed) and when a change takes effect in the game
today (**live**: read every frame; **start**: read when the renderer or the world is built):

| Key | Vanilla | Vanilla+ | Enhanced | Takes effect | Note |
|---|---|---|---|---|---|
| `graphics` | ✓ | ✓ | ✓ | start | launcher only (FR-020c) |
| `graphics_api` | — | ✓ | ✓ | start | launcher only; loses `d3d9` |
| `preset` | — | ✓ | ✓ | start | Vanilla: every key a preset sets is fixed or hidden there except a few; not offered (FR-017) |
| `msaa` | ✓ | ✓ | ✓ | start | |
| `render_scale` | ✓ (no automatic) | ✓ | ✓ | start | |
| `dx9_upscale` | ✓ (scale below 100 %) | — | — | start | |
| `anisotropy` | ✓ | ✓ | ✓ | start | |
| `view_distance`, `max_obj_dist`, `min_obj_size` | ✓ | ✓ | ✓ | start | |
| `shadows` | — | ✓ | ✓ | **live** | |
| `shadow_size`, `shadow_casters` | — | ✓ | ✓ | start | |
| `ssao` | — | ✓ | ✓ | start | candidate for FR-020e |
| `shadow_blobs` | ✓ | ✓ | ✓ | start | to confirm in Vanilla |
| `night_brightness` | — | — | ✓ | **live** | offered in Vanilla+ today, where it does nothing |
| `led_glow`, `led_mips` | — | to confirm | ✓ | **live** | the 2026-11-07 survey found `led_glow` Enhanced-only; `led_mips` acts on the texture's mip bias and may act in Vanilla+ |
| `detail_textures` | — | ✓ | ✓ | **live** | |
| `reflections` | ✓ | ✓ | ✓ | start | candidate for FR-020e |
| `mirror_size` | ✓ (OMSI 2's sizes) | ✓ | ✓ | start | |
| `mirror_refresh` | ✓ (off, full) | ✓ | ✓ | **live** | |
| `clouds` | `=As OMSI 2` | ✓ | ✓ | start | candidate for FR-020e |
| `sun_glow` | ✓ | ✓ | ✓ | **live** | |
| `windy_trees` | `=Off` | ✓ | ✓ | **live** | |
| `smoke`, `smoke_user_only`, `smoke_max`, `smoke_in_mirrors` | ✓ | ✓ | ✓ | start | |
| `dx9_snow`, `fewer_night_lights`, `dx9_glass_rain`, `dx9_coronas`, `dx9_stars`, `dx9_cloud_picture`, `dx9_texture_limit` | ✓ | — | — | to confirm (handed to the device on a settings change) | labels describe the effect, not the API (FR-018) |
| `texture_memory`, `texture_compression` | ✓ | ✓ | ✓ | start | |
| `fullscreen`, `vsync`, `max_fps`, `fov`, `outside_fov` | ✓ | ✓ | ✓ | **live** | Display section |
| `vr`, `vr_*` | — | ✓ | ✓ | start | FR-018a |
| `triple_screen`, `triple_*` | — | ✓ | ✓ | start | FR-018a |

Every "to confirm" and every Vanilla cell is checked by the task that builds the table (phase B1);
the table in the code, not this page, is then the reference, and the user guide is generated from
it.

**Decision**: one table, `GRAPHICS_OPTIONS`, next to `GraphicsMode` in `omsi-cfg`
([data-model.md](data-model.md)); the launcher's `shown()` and the in-game list both ask it;
`api_limits.rs` and the loose rules go. The in-game list is **generated** from the launcher's option
list filtered by the table (mode, live), instead of being written by hand.

**Rationale**: FR-014 (one source) and FR-020 (a test can walk the table). `omsi-cfg` is the one
crate the game, the launcher core and the launcher UI all depend on.

**Alternatives considered**: keep the table in `omsi-launcher-ui` (the game already reaches it):
rejected, the game would keep depending on the launcher's UI crate for a rule about the renderer.

## R-12. Which options are made to apply at once (FR-020e)

**Decision** (agreed by the maintainer on 2026-11-13, D6): three on/off effects: `ssao`,
`reflections`, `clouds`. One pull request each.

| Key | Why it needs a start today | What "at once" takes |
|---|---|---|
| `ssao` | `RenderOptions::ssao` is copied when the renderer is built | the frame already decides `ao_on` per frame; the option becomes a field the app may change, and the AO targets are made on first use |
| `reflections` | `RenderOptions::reflections` likewise; also read by the puddles | the same: a per-frame read; materials keep their maps loaded either way |
| `clouds` | `make.rs` decides once whether the cloud layer exists | the layer is kept and its drawing switched |

**Rationale**: all three switch a pass or a term on and off without rebuilding pipelines, targets of
another size or textures; the test of FR-020e (picture after the switch equals a fresh start) is a
plain `render_to_image` comparison. `msaa`, `shadow_size`, `render_scale`, `anisotropy`, the
distances and the texture options rebuild resources and stay launcher-only.

**Alternatives considered**: `shadow_size` and `msaa` live: rejected for this feature (they rebuild
targets and pipelines; FR-020e excludes that). If one of the three turns out to need a rebuild when
its task starts, it is dropped from the set with a note here, not forced.
