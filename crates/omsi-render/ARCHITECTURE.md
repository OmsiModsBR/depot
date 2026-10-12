# omsi-render architecture

`omsi-render` is organised in **domains** (one folder each) stacked in **five layers**. A
domain may import from the layers below its own, never from its own layer's other domains or
from above. The question that places a piece of code is *does it know what a frame is?*: the
device and the shaders do not, the scene's data does not, a pass knows how to draw one part of a
frame, and only `renderer` puts a whole frame together.

```text
4  Orchestration   renderer
3  Passes          passes   targets   effects
2  Scene           scene
1  Model           assets   lighting   sky
0  Base            gpu      shaders
                   (imports point down only)
```

## Domains

| Layer | Domain | Owns | Lives today in |
| --- | --- | --- | --- |
| 0 | `gpu` | Opening the device (adapters, limits, the remembered fallbacks, `DevicePoller`), what the device can do (GL backend, `ArrayPath`, VRAM), `GpuArray`, errors (`catch`, device lost, out of memory), `GpuTimers`, the window surface (`SurfaceState`, `winit_window`) | `gpu.rs`, `surface.rs`, parts of `renderer.rs` and `lib.rs` |
| 0 | `shaders` | The WGSL sources, grouped `common/`, `scene/`, `sky/`, `post/`, `lights/`, `effects/`, and the code that assembles them (`scene_shader_source` and friends) | `shaders.rs`, `*.wgsl` |
| 1 | `assets` | Meshes (`GpuMesh`, `MeshPage`, `prepare_mesh`), textures (`GpuTexture`, uploads, mip chains, `fit_texture`), materials (`Material`, `MaterialExtra`, `PbrMaps`, `BindKey`) and their life cycle (free, recycle, `Freed`, pending mesh writes) | `resources.rs`, `material.rs` |
| 1 | `lighting` | The light model (`Lighting`, `PointLight`, `Corona`, `SmokeParticle`, `LightMode`), its GPU records (`GpuPointLight`, `GpuCorona`), the light grid and lamp shadows, the tile light map atlas | `lighting.rs`, parts of `frame.rs` |
| 1 | `sky` | The atmosphere and the clouds' noise (CPU only, no `wgpu`), and the sky's GPU resources (look-up table, cloud textures, reflection probe) | `atmosphere.rs`, `clouds.rs`, parts of `frame.rs` and `renderer.rs` |
| 2 | `scene` | `Scene`, `Instance`, the ids, `RenderPhase`, the camera, the views (`TripleScreen`, `ScreenView`, `ViewCulling`), bounds, visibility (culling and level of detail) and the per-instance settings | `scene.rs`, `camera.rs`, `triple.rs`, parts of `render.rs` |
| 3 | `targets` | The render targets and their per-size caches: HDR, MSAA, depth, AO, render scale | `targets.rs` |
| 3 | `passes` | One module per pass, each holding its pipelines, its `#[repr(C)]` uniform, its bind group layout and its `encode_*`: `draw` (batches, pipeline codes), `shadow`, `depth` (prepass and SSAO), `sky`, `main`, `glass`, `reflections`, `fog_lamps`, `lights` (coronas, smoke), `post` (enhanced post, exposure), `present` (upscale, FXAA, overlay, HUD, XR UI) | `passes.rs`, `pipelines.rs`, `draw.rs`, uniforms in `lib.rs` |
| 3 | `effects` | Self-contained features with their own pipelines and targets: `rt` (ray tracing), `puddles` | `rt.rs`, `puddles.rs` |
| 4 | `renderer` | `Renderer` and `RenderOptions`, building it, the frame (`FrameContext` and its stages), the public entry points (`render`, `render_triple`, the XR eyes, `render_to_image`), read-backs, and the scene and asset methods the game calls on `Renderer` | `renderer.rs`, `render.rs`, `frame.rs` |

The "Lives today in" column shrinks as each domain reaches its folder; when it is empty, this
column goes.

## Rules

1. **Imports point down.** A domain names what it uses (`use crate::assets::GpuMesh;`). No new
   `use super::*`: the import list is the domain's dependency map. The crate-internal globs left
   in `lib.rs` are temporary and go as the domains move.
2. **Smallest visibility.** `pub(super)` inside a domain, `pub(crate)` only for what another
   domain uses, `pub` only through the facade in `lib.rs`.
3. **The facade is the public API.** `lib.rs` re-exports the public items one by one. The game
   (`omsi-app`) imports only those names; `tests/public_api.rs` lists them and must keep
   compiling. Moving code inside the crate never changes this list.
4. **A pass keeps its parts together.** Pipelines, uniform, layout and encoding sit in the
   pass's module; its shader sits in `shaders/<domain>/`. Passes do not import each other: what
   two passes share goes to `passes::draw` or a lower layer.
5. **`Renderer` composes domains.** Its state is grouped by domain (device context, layouts and
   samplers, assets, sky, exposure, shadows, light map, target cache, culling history,
   diagnostics), and a pass receives the parts it uses rather than `&Renderer`. The fields the
   game reads directly (`device`, `queue`, `options`, `adapter_name`, `address_next`,
   `light_map_next`, `stats`, `counts`, `instant_exposure`, `shadow_blobs`, `blend_by_origin`)
   stay fields of `Renderer`.
6. **Tests live with their domain** (`<domain>/tests.rs`); tests of a whole picture live in
   `renderer`. Tests that need their own process environment stay in `tests/`.
7. **Unused code is not deleted.** Code without a caller stays in the tree; at most it leaves
   the build behind a `cfg` or a feature.

## Where new code goes

* A new pass: a module in `passes/` with its pipeline, uniform and `encode_*`, its shader in
  `shaders/`, and one call in the frame's stages in `renderer`.
* A new per-instance setting: the field on `Instance` and its setter in `scene`; the method on
  `Renderer` the game calls only forwards to it.
* A new kind of GPU resource the scene holds (a mesh, texture or material variant): `assets`.
* Anything about the device or the driver (a limit, a workaround for an adapter): `gpu`.

Until a domain has its folder, new code goes into the file listed for it above; a contributor
may also create the domain's folder and put the new code there.

## Proof that nothing changed

Reorganising the crate never changes the picture: the offscreen golden on lavapipe
(`offscreen-golden` in CI) stays identical, the offscreen state traces (`offscreen-traces`)
show no difference, `cargo test -p omsi-render` passes (it validates every shader with naga and
its GLSL translation), and `omsi-app` builds without changes.
