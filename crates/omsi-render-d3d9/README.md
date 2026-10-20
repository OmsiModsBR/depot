# omsi-render-d3d9

A **separate legacy Direct3D 9 renderer**, built step by step. It is meant to render with
the feature set of OMSI 2 itself (not a reduced profile of the wgpu renderer).

## What this is not

* It is **not wired into the game**. No crate depends on it and the game still renders through
  `omsi-render` (wgpu); the default renderer and the inherited backends are untouched.
* It **declares no supported hardware**. Widening the hardware matrix is a product decision.
* It does not yet draw everything the game shows: see "The scene layer" below for what the
  scene shader covers so far.

## What is in it

| Part | Platforms | Content |
| --- | --- | --- |
| `backend` | all | The crate's drawing layer `RenderBackend` (create texture/mesh, begin frame, draw a batch, end frame, recover) with the validation every implementation shares, and `NullBackend`, a GPU-less implementation |
| `caps` | all | `Caps` / `ShaderModel`, and the floor: **Shader Model 3.0** in both stages and textures of 2048 or more (maintainers' decision, 2026-10-19) |
| `d3d9` | Windows | The Direct3D 9Ex implementation: off-screen targets with read-back or a window swap chain, textures (one level), meshes (16- or 32-bit indices), depth test, and the alpha modes opaque, test and blend |
| `scene` | all | The game's scene as this renderer takes it: textures (RGBA or DXT1/3/5, with their levels, or a mip chain made here), meshes under the game's ids, and frames of batches over a per-draw list, with the camera and the light; checks shared by every implementation |
| `d3d9` scene layer | Windows | `set_scene_mesh`/`set_scene_texture` (and their update, move and free), and `draw_scene`, one instanced draw per batch with the scene shader |
| `d3d9::shader` | Windows | HLSL compilation at run time through the system's `d3dcompiler_47.dll` |
| `examples/d3d9_window.rs` | Windows | A window showing the three alpha modes side by side |

### Choices worth knowing

* **Direct3D 9Ex**, not plain 9: this crate needs Windows 10 or later (the minimum of Rust's
  Windows targets since Rust 1.78), where 9Ex is always present and resources survive
  occlusion and mode changes. Only a removed device is rebuilt. This is a requirement of the
  crate, not a statement about the platforms Depot supports.
* The device is created with `D3DCREATE_FPU_PRESERVE`, so Direct3D does not switch the
  thread's x87 unit to single precision behind the simulation's back.
* **Alpha test and blending are render states**, applied after the pixel shader, as Direct3D
  9 defines them. The alpha-test reference (`ALPHA_TEST_REF`, 128) is **provisional**: the
  value the original applies is a parity item still to be observed.
* Culling is off until the meshes' winding is settled with the content loader.
* Shaders are compiled when the device opens (two small shaders, milliseconds). A cache on disk
  can come when there are more of them; nothing generated is committed.
* `d3dcompiler_47.dll` is bound at load time through the normal DLL search order. Before this
  crate is wired into the game it must be loaded explicitly from the system directory, so the
  game does not pick up a copy planted next to it.
* The backend's own textures and meshes (`RenderBackend`) are never freed; the scene's are,
  under the game's ids.

## The scene layer

The layouts follow the default renderer (`omsi-render`), so both draw from the same values:
a vertex is position, normal and uv; each draw has a model matrix and two parameter vectors
(alpha multiplier, visibility, uv offset; the surface code), sent as instance data; a
material is its uniform's 11 vectors; the light is the camera uniform's vectors. The game
maps its frame plan onto `SceneFrame` and its asset events onto the `set_scene_*` calls.

The scene shader (`shaders/scene.hlsl`) is the vanilla picture: colour worked on as stored
(gamma-encoded), the texture times the vertex light clamped at 1, the fog mixed in the same
space. It covers the sun, the light from above and the ambient light with the material's
diffuse, ambient and emissive colours, unlit materials, transmaps, texture borders and
mirror-once addressing, terrain uv, the alpha kinds (opaque, cut, blended with or without
depth write, depth-only, painted ground), back-face culling, the road surfaces' pull
towards the eye and their depth bias, invisible instances, and Direct3D 9's half-pixel
offset. Depth is reversed as in the default renderer (cleared to 0, nearer is greater).

Not yet (later phases): shadows and ambient occlusion, the map's and the vehicles' lamps,
the specular highlight, night, light and environment maps, bump maps, detail texturing,
swaying trees, weather, the sky, coronas and smoke, the fog's way through the player's
vehicle, mirrors and render-to-texture, and texture addressing other than wrap in the
sampler. The cut of the alpha-tested materials (`scene::ALPHA_CUT`, alpha above 128) is the
default renderer's; like `ALPHA_TEST_REF` it is a **provisional** parity item.

A device that is rebuilt after a removal (`recover`) starts with an empty scene; the game
uploads its assets again.

## Dependencies and provenance

Only the `windows` crate (Microsoft, MIT OR Apache-2.0), already used by `omsi-render`, at the
workspace version, with the Direct3D 9, Direct3D (blob), FXC (compiler) and GDI features;
`winit` (workspace version) only for the example. All code is original to this repository.

## Testing

`cargo test -p omsi-render-d3d9` runs on every platform. On Windows it also

* compiles every shader for `vs_3_0`/`ps_3_0` (CPU only, so it runs on CI without a GPU), and
* runs device tests that draw small scenes off-screen and check the pixels: clear, texture
  times tint, untextured batches, alpha test at the reference, blend, depth order and the
  matrix layout, and `recover` rejecting old handles; and the scene layer: the texture under
  the ambient light, an unlit and a missing texture, fog, the alpha cut at 128, reversed
  depth, instances and invisible ones, a DXT1 texture, a missing mesh skipped, and a frame
  pointing outside its data refused. They skip themselves only when there is
  no Direct3D 9Ex driver or the device is below the floor; any other failure to open fails
  them. The log says which adapter ran them.

Verified on the maintainer's PC (Intel UHD 630, 16/16 at the first phase-1 head): the shader
compile tests and the off-screen device tests; the window example opened. **Not covered by
any test:** `resize`, the window swap chain and present, `status` on a lost or occluded
device, and `recover` after a real device removal (it cannot be forced portably). The
scene layer's shader and device tests have not run on a Windows machine yet.

## Next steps

The maintainers decided (2026-10-19) that the game gets this renderer as a render option,
"DirectX9 (D3D9Ex)", off by default. In that mode `omsi-render` keeps doing the CPU work
(scene, culling, level of detail, alpha order) on a device that draws nothing, and hands its
frame plan and asset events to this crate. In order:

1. The game opens with this renderer: the window, the frame loop and the asset events.
2. The option in the launcher's render options, off by default, Windows only.
3. The UI (menus, the Alt bar, the HUD) on this renderer.
4. The OMSI material features (night, light and environment maps, bump maps), then
   shadows, coronas, smoke and the sky, then mirrors and maps (render to texture).
5. The alpha-test cut and the other parity items from the maintainer's observation of the
   original.
