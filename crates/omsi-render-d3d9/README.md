# omsi-render-d3d9

A **separate legacy Direct3D 9 renderer**, built step by step. It is meant to render with
the feature set of OMSI 2 itself (not a reduced profile of the wgpu renderer).

## What this is not

* It is **not wired into the game**. No crate depends on it and the game still renders through
  `omsi-render` (wgpu); the default renderer and the inherited backends are untouched.
* It **declares no supported hardware**. Widening the hardware matrix is a product decision.
* It does not yet draw game content: no materials beyond one texture and a tint, no lights,
  fog or culling.

## What is in it

| Part | Platforms | Content |
| --- | --- | --- |
| `backend` | all | The crate's drawing layer `RenderBackend` (create texture/mesh, begin frame, draw a batch, end frame, recover) with the validation every implementation shares, and `NullBackend`, a GPU-less implementation |
| `caps` | all | `Caps` / `ShaderModel`, and the floor: **Shader Model 3.0** in both stages and textures of 2048 or more (maintainers' decision, 2026-10-19) |
| `d3d9` | Windows | The Direct3D 9Ex implementation: off-screen targets with read-back or a window swap chain, textures (one level), meshes (16- or 32-bit indices), depth test, and the alpha modes opaque, test and blend |
| `d3d9::shader` | Windows | HLSL compilation at run time through the system's `d3dcompiler_47.dll` |
| `examples/d3d9_window.rs` | Windows | A window showing the three alpha modes side by side |

### Choices worth knowing

* **Direct3D 9Ex**, not plain 9: Depot needs Windows 10+, where 9Ex is always present and
  resources survive occlusion and mode changes. Only a removed device is rebuilt.
* The device is created with `D3DCREATE_FPU_PRESERVE`, so Direct3D does not switch the
  thread's x87 unit to single precision behind the simulation's back.
* **Alpha test and blending are render states**, applied after the pixel shader, as Direct3D
  9 defines them. The alpha-test reference (`ALPHA_TEST_REF`, 128) is **provisional**: the
  value the original applies is a parity item still to be observed.
* Culling is off until the meshes' winding is settled with the content loader.
* Shaders are compiled when the device opens (two small shaders, milliseconds). A cache on disk
  can come when there are more of them; nothing generated is committed.

## Dependencies and provenance

Only the `windows` crate (Microsoft, MIT OR Apache-2.0), already used by `omsi-render`, at the
workspace version, with the Direct3D 9, Direct3D (blob), FXC (compiler) and GDI features;
`winit` (workspace version) only for the example. All code is original to this repository.

## Testing

`cargo test -p omsi-render-d3d9` runs on every platform. On Windows it also

* compiles every shader for `vs_3_0`/`ps_3_0` (CPU only, so it runs on CI without a GPU), and
* runs device tests that draw small scenes off-screen and check the pixels: clear, texture
  times tint, untextured batches, alpha test at the reference, blend, depth order and the
  matrix layout. They skip themselves without a Direct3D 9Ex driver with Shader Model 3.0;
  the log says which adapter ran them.

## Next steps

1. After the `omsi-render` refactor in #61: inventory of the facade methods the game calls,
   a scene-level trait both renderers implement, and the CPU-side scene code (culling, level
   of detail, alpha ordering) shared between them.
2. The OMSI material features (night, light and environment maps, bump, transparency maps),
   fog, lights, mirrors and off-screen targets; the alpha-test reference and other parity
   items from the maintainer's observation of the original.
3. Selecting the renderer in the game, behind a decision by the maintainers.
