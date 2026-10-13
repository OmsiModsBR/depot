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
8  Orchestration   renderer
   Outputs         output                       (window, image, xr, triple, diagnostics)
7  Effects         effects                      (draw into the targets with the passes' parts)
6  Targets         targets                      (their bind groups use the passes' layouts)
5  Passes          passes
4  Scene           scene
3  Sky             sky                          (the sky follows the light model)
2  Model           assets   lighting
1  Shaders         shaders                      (assembled for what the device can do)
0  Base            gpu                          (also the render target formats)
                   (imports point down only)
```
## Domains

| Layer | Domain | Owns | Lives today in |
| --- | --- | --- | --- |
| Entry | `api` | The facade: every `pub fn` on `Renderer` (opening, settings, scene, assets, lights, `render*`), split into `renderer`, `scene`, `assets`, `lights`, `options` (`RenderOptions` and the public constants) and `types` (re-exports). Thin forwarding only, no GPU logic | in place (`api/`: `renderer`, `scene`, `assets`, `lights`, `types`); the methods still carry their whole bodies, which move to their domains in the next steps, and `RenderOptions` is still in `renderer/options.rs` |
| 8 | `renderer` | The internal state of `Renderer`, building it, the frame (`FrameContext` and its stages), ending by handing the frame to an `output` destination | in place (`renderer/`: `state`, `options`, `build`, `prepare`, `frame`, `lists`, `encode`): `Renderer::build` calls one constructor per sub-struct, and `render_inner` runs `begin_frame` (the `FrameContext`), `draw_lists` (visibility and batches), `ShadowResources::schedule`, `encode_frame` and `submit_frame`, which hands the frame to an `Output`; opening the renderer is still in `api/renderer.rs` |
| Outputs | `output` | The frame's destinations: `window` (`SurfaceState`, `winit_window`, presenting with upscale, FXAA, overlay and HUD), `image` (`render_to_image`, texture read-back), `xr` (the eyes, the headset UI), `triple` (the panels), `diagnostics` (`stats`, `counts`, `GpuTimers`, GPU pass times, exposure log, draw audit) | in place (`output/`: `destination` (`Output`: `Window`, `Texture`, `XrEye`, `TriplePanel`), `window` (with `Presenter`), `image`, `triple`, `diagnostics` (with `Diagnostics`)); the XR methods are still in `api/renderer.rs` and the exposure log in `lighting/exposure.rs` |
| 7 | `effects` | Self-contained features with their own pipelines and targets: `rt` (ray tracing), `puddles` | in place (`effects/`) |
| 6 | `targets` | The render targets and their per-size caches: HDR, MSAA, depth, AO, the puddles' reflection targets | in place (`targets/`) |
| 5 | `passes` | One module per pass, each holding its pipelines, its `#[repr(C)]` uniform, its bind group layout and its `encode_*`: `draw` (batches, pipeline codes), `shadow` (with the cascades' ranges), `depth` (prepass and SSAO), `sky`, `main`, `glass`, `reflections`, `fog_lamps`, `lights` (coronas, smoke), `post` (enhanced post, exposure) | in place (`passes/`), each pass with its own struct (`ShadowResources`, `PrepassPipelines`, `SsaoPass`, `FogLampsPass`, `SpriteResources`, `PostPipelines`, `Exposure`); `shadow`, `sky` and `fog_lamps` encode on their own struct, while `depth`, `main`, `glass`, `reflections` and `post` still take `&Renderer` because they call other `Renderer` methods or the `Presenter` |
| 4 | `scene` | `Scene`, `Instance`, `RenderPhase`, the camera, the views' geometry (`TripleScreen`, `ScreenView`, `ViewCulling`), bounds, visibility (culling and level of detail) and the logic behind the per-instance settings | in place (`scene/`, with the shared bind group `Layouts` and the `CullHistory`); visibility (culling, level of detail) runs in `renderer/lists.rs` |
| 3 | `sky` | The atmosphere and the clouds' noise (CPU only, no `wgpu`), and the sky's GPU resources (look-up table, the clouds' noise textures, reflection probe) | in place (`sky/`, the GPU resources in `SkyResources`) |
| 2 | `assets` | The ids (`MeshId`, `TextureId`, `MaterialId`) and `Vertex`, meshes (`GpuMesh`, `MeshPage`), textures (`GpuTexture`, uploads, mip chains, fitting), materials (`Material`, `MaterialExtra`, `PbrMaps`, `BindKey`) and their life cycle (free, recycle, `Freed`, pending mesh writes) | in place (`assets/`, the state in `AssetStore`); `buffer_init` went to `gpu/buffers.rs`, the scene's instance slots to `scene.rs`, glass and render-to-texture to `passes.rs` and `frame.rs` |
| 2 | `lighting` | The light model (`Lighting`, `PointLight`, `Corona`, `SmokeParticle`, `LightMode`), its GPU records (`GpuPointLight`, `GpuCorona`), the light grid and lamp shadows, the tile light map atlas | in place (`lighting/`, the atlas in `LightMapAtlas`) |
| 1 | `shaders` | The WGSL sources, grouped `common/` (included by several shaders), `scene/`, `sky/`, `post/`, `lights/`, `effects/`, `output/`, `assets/`, and the code that assembles them | in place (`shaders/`) |
| 0 | `gpu` | Opening the device (adapters, limits, the remembered fallbacks, `DevicePoller`), what the device can do (GL backend, `ArrayPath`, VRAM), `GpuArray`, errors (`catch`, device lost, out of memory), the render targets' formats (`DEPTH_FORMAT`, `HDR_FORMAT`, the masks and G-buffer, `color_targets`) | in place (`gpu/`, the device's callbacks, encoder pool and poller in `GpuContext`, the samplers in `Samplers`); the adapter and device opening is still in `Renderer::new_on` (`api/renderer.rs`) |

The "Lives today in" column shrinks as each domain reaches its folder; when it is empty, this
column goes.

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
   only `renderer` and `api` import from `output`. The globs left in `lib.rs` serve only the tests
   (`tests.rs`, `tests_units.rs`) and go when those move to their domains.
   `scripts/check-render-layers.sh` lists the imports that break this rule (informational on CI
   for now).
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
7. **Tests live with their domain** (`<domain>/tests.rs`); tests of a whole picture live in
   `output::image`, where the picture comes out. Tests that need their own process environment
   stay in `tests/`.
8. **Unused code is not deleted.** Code without a caller stays in the tree; at most it leaves
   the build behind a `cfg` or a feature.

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

Until a domain has its folder, new code goes into the file listed for it above; a contributor
may also create the domain's folder and put the new code there.

## Proof that nothing changed

Reorganising the crate never changes the picture: the offscreen golden on lavapipe
(`offscreen-golden` in CI) stays identical, the offscreen state traces (`offscreen-traces`)
show no difference, `cargo test -p omsi-render` passes (it validates every shader with naga and
its GLSL translation), and `omsi-app` builds without changes.
