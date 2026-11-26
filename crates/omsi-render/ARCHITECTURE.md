# omsi-render architecture

`omsi-render` has two clear edges and a layered core between them:

* **The entry** (`api`) is the one package the game sees. Everything `pub` in the crate lives
  there: `Renderer` as the public handle with all its public methods, `RenderOptions`, and the
  re-exports of the public types.
* **The outputs** (`output`) are the one package that hands a finished frame to a destination:
  the window, an image, the headset's eyes, the triple screen's panels, and the diagnostic
  numbers.
* **The core** between them is organised in **domains** (one folder each) stacked in
  **layers**. A domain may import from the layers below its own, never from its own layer's
  other domains or from above.

A frame always takes the same path: the game calls `api`, `api` asks `renderer` for the frame,
`renderer` runs the passes and hands the result to an `output` destination. Nothing in the core
presents to the window or reads a texture back on its own.

```text
   Entry           api                          (only pub; nothing imports from it)
12 Orchestration   renderer                     (builds the one mode, in `renderer/build.rs` only)
   Outputs         output                       (window, image, xr, triple, diagnostics)
   Modes           modes/vanilla_plus  modes/enhanced   (a mode never imports the other)
   Interface       interface                    (what the core and the modes agree on)
   Effects         effects                      (draw into the targets with the passes' parts)
   Passes          passes                       (draw into the targets)
   Targets         targets                      (given the layouts their bind groups use)
   Scene           scene
   Sky             sky                          (the sky follows the light model)
   Model           assets   lighting
   Shaders         shaders                      (assembled for what the device can do)
   Base            gpu                          (also the render target formats)
                   (imports point down only)
```

The third rendering path, Vanilla, is not in this crate: it is `omsi-render-vanilla`, a separate
crate on Direct3D 9 that draws a `FramePlan` this crate builds. Neither crate depends on the
other (rule 9).

## Domains

| Layer | Domain | Owns |
| --- | --- | --- |
| Entry | `api` | The facade: every `pub fn` on `Renderer` (opening, settings, scene, assets, lights, `render*`), split into `renderer`, `scene`, `assets`, `lights`, and `types` (re-exports; `RenderOptions` itself lives in `renderer/options.rs`). Thin forwarding only, no GPU logic |
| Orchestration | `renderer` | The internal state of `Renderer`, building it, the asset life cycle on it (uploads, materials, freed slots), the frame (`FrameContext` and its stages, with the lights, coronas, smoke and sky uploaded for it), ending by handing the frame to an `output` destination |
| Modes | `modes/vanilla_plus`, `modes/enhanced` | The two shading paths of this crate, each in its own folder. A mode answers how a picture is shaded, which pipelines draw it, its light records and what the leading view feeds it (`interface::RenderMode`). `modes/enhanced` also holds everything only Enhanced uses: the post passes (`post.rs`), the lamps' light in the fog (`fog_lamps.rs`), the light rules (`lights.rs`), its pipelines (`pipelines.rs`), the scene shader assembly (`scene_shader.rs`) and `shaders/` (`enhanced`, `sky`, `fog_lamps`, `corona`, `snow`). `modes/vanilla_plus` holds its pipelines and `shaders/reflections.wgsl`. A mode imports the core and `interface` only, never `output`, `renderer` or `api`; only `renderer/build.rs` names a mode; a mode's `shaders/` folder is included by that mode alone |
| Interface | `interface` | What the core and the modes agree on: `mode` (the `RenderMode` trait and the data it takes and gives), `frame_plan` (`FramePlan`, `PlanBatch`, `PlanKind`, `PlanSink`: what a picture draws, as data, the contract with the Vanilla renderer) |
| Outputs | `output` | The frame's destinations: `window` (`SurfaceState`, `winit_window`, the `Presenter`: presenting with upscale, FXAA, overlay and HUD), `image` (texture read-back), `xr` (the eyes, the headset UI), `triple` (the panels' culling), `diagnostics` (`stats`, `counts`, `GpuTimers`, GPU pass times, exposure log, draw audit) |
| Effects | `effects` | Self-contained features with their own pipelines and targets: `puddles` |
| Passes | `passes` | One module per pass, each holding its pipelines, its `#[repr(C)]` uniform, its bind group layout and its `encode_*`: `draw` (batches, pipeline codes), `shadow` (with the cascades' ranges), `depth` (prepass and SSAO), `sky`, `main`, `glass`, `fog_lamps`, `lights` (coronas, smoke), `post` (enhanced post, exposure) |
| Targets | `targets` | The render targets and their per-size caches: HDR, MSAA, depth, AO, the puddles' reflection targets |
| Scene | `scene` | `Scene`, `Instance`, `RenderPhase`, the camera, the views' geometry (`TripleScreen`, `ScreenView`, `ViewCulling`), bounds, visibility (culling and level of detail) and the logic behind the per-instance settings |
| Sky | `sky` | The atmosphere and the clouds' noise (CPU only, no `wgpu`), and the sky's GPU resources (look-up table, the clouds' noise textures, reflection probe) |
| Model | `assets` | The ids (`MeshId`, `TextureId`, `MaterialId`) and `Vertex`, meshes (`GpuMesh`, `MeshPage`), textures (`GpuTexture`, uploads, mip chains, fitting), materials (`Material`, `MaterialExtra`, `PbrMaps`, `BindKey`) and their life cycle (free, recycle, `Freed`, pending mesh writes) |
| Model | `lighting` | The light model (`Lighting`, `PointLight`, `Corona`, `SmokeParticle`, `LightMode`), its GPU records (`GpuPointLight`, `GpuCorona`), the light grid and lamp shadows, the tile light map atlas |
| Shaders | `shaders` | The WGSL sources, grouped `common/` (included by several shaders), `scene/`, `sky/`, `post/`, `lights/`, `effects/`, `output/`, `assets/`, and the code that assembles them |
| Base | `gpu` | Opening the device (adapters, limits, the remembered fallbacks, `DevicePoller`), what the device can do (GL backend, `ArrayPath`, VRAM), `GpuArray`, errors (`catch`, device lost, out of memory), the render targets' formats (`DEPTH_FORMAT`, `HDR_FORMAT`, the masks and G-buffer, `color_targets`) |

The layer order is checked by `scripts/check-render-layers.sh` (api 12, renderer 11, output 10,
modes 9, interface 8, effects 7, passes 6, targets 5, scene 4, sky 3, assets and lighting 2,
shaders 1, gpu 0).

## Rules

1. **Only `api` is `pub`.** Every `pub fn` on `Renderer` and every public re-export sits in
   `api/`; the rest of the crate is at most `pub(crate)`. An `api` method checks its arguments
   and forwards, in a few lines. `lib.rs` holds only the `mod` lines and the re-exports from
   `api`, item by item. `tests/public_api.rs` lists the public names and must keep compiling.
2. **Only `output` delivers a frame.** Presenting to the window, copying into a panel, reading
   pixels back and publishing diagnostic numbers are `output`'s. `renderer` ends a frame by
   calling a destination (window, image, XR eye, triple panel) and does not know how it
   delivers.
3. **Imports point down.** A domain names what it uses (`use crate::assets::GpuMesh;`). No new
   `use super::*`: the import list is the domain's dependency map. Nothing imports from `api`;
   only `renderer` and `api` import from `output`.
   `scripts/check-render-layers.sh` lists the imports that break this rule, grouped ones
   (`use crate::{a, b}`) included, and CI fails on any.
4. **Smallest visibility.** `pub(super)` inside a domain, `pub(crate)` only for what another
   domain uses.
5. **A pass keeps its parts together.** Pipelines, uniform, layout and encoding sit in the
   pass's module; its shader sits in `shaders/<domain>/`. Passes do not import each other: what
   two passes share goes to `passes::draw` or a lower layer.
6. **`Renderer` composes domains.** Its state is grouped by domain (device context, layouts and
   samplers, assets, sky, exposure, shadows, light map, target cache, culling history, the
   presenter, diagnostics), and a pass receives the parts it uses rather than `&Renderer`. The
   fields the game reads directly (`device`, `queue`, `options`, `adapter_name`, `address_next`,
   `light_map_next`, `stats`, `counts`, `instant_exposure`, `shadow_blobs`, `blend_by_origin`)
   stay fields of `Renderer`. `gpu` (`GpuContext`) is the last field, so the device poller
   is dropped after everything that uses the device.
7. **Tests live outside `src/`** (constitution, Principle VI). They sit in `tests/`, and reach
   internals only through the public API or through `testing.rs`, which the `test-support`
   feature turns on (the crate lists itself as a dev-dependency with that feature).
8. **Unused code is removed** (constitution, Principle X). Code without a caller is deleted, not
   kept behind a `cfg` or a feature.
9. **The two rendering crates are independent.** `omsi-render` and `omsi-render-vanilla` never
   depend on each other; the game (`omsi-app`) picks one by the graphics mode, and
   `FramePlan` (`interface::frame_plan`) is the data the Vanilla crate draws. The launcher's
   vehicle preview is always drawn by this crate as Vanilla+.

## Where new code goes

* Something new the game calls: a thin `pub fn` in `api/`, forwarding to the domain that does
  the work.
* A new place a frame goes (a new kind of window, a capture, a stream): a destination in
  `output/`.
* A new pass: a module in `passes/` with its pipeline, uniform and `encode_*`, its shader in
  `shaders/`, and one call in the frame's stages in `renderer`.
* A new per-instance setting: the field on `Instance` and its logic in `scene`; the `api` method
  only forwards to it.
* A new kind of GPU resource the scene holds (a mesh, texture or material variant): `assets`.
* Anything about the device or the driver (a limit, a workaround for an adapter): `gpu`.
* Something only one mode uses (a pass, a pipeline, a shader, a light rule, a target that only
  it reads): that mode's folder under `modes/`, with its shader in the mode's `shaders/`. A
  shared pass or target stays in the core; before moving code into a mode, check that the other
  mode really does not use it.
* Something Vanilla (Direct3D 9) draws differently: `omsi-render-vanilla`, see its README.
* A new graphics mode: a folder under `modes/` implementing `interface::RenderMode`, one line
  in `renderer/build.rs`, no change to the other modes.

## Proof that nothing changed

Reorganising the crate never changes the picture: the offscreen golden on lavapipe
(`offscreen-golden` in CI) stays identical, the offscreen state traces (`offscreen-traces`)
show no difference, `cargo test -p omsi-render` passes (it validates every shader with naga and
its GLSL translation), and `omsi-app` builds without changes.
