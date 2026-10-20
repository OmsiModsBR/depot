# omsi-render-d3d9

A **separate legacy Direct3D 9 renderer**, built step by step. It is meant to render with
the feature set of OMSI 2 itself (not a reduced profile of the wgpu renderer).

## How the game uses it

Only under the render option "DirectX9 (D3D9Ex)" (`graphics_api = d3d9`, or
`OMSI_BACKEND=d3d9`), on Windows and without VR; the default renderer and the other
backends are untouched. In that mode `omsi-render` runs on wgpu's noop device and keeps
the books (scene, culling, level of detail, alpha order); its asset events and each window
picture's frame plan come here (`omsi-app/src/d3d9_output.rs`), and this crate draws and
presents the window. It **declares no supported hardware**: widening the hardware matrix
is a product decision.

A window picture is one Direct3D frame: the plan sink draws the scene and the HUD's
pictures and leaves the frame open, the new interface (the Esc menu, the Alt bar, the
loading screen, drawn by `omsi-ui`) is mirrored into it, and the game presents it when its
frame is done. Not drawn in that mode yet: the sky, triple screen, screenshots, pictures the
GPU draws into textures (the navigator's maps, mirrors), and what "The scene layer" lists as
not yet.

## What is in it

| Part | Platforms | Content |
| --- | --- | --- |
| `backend` | all | The crate's drawing layer `RenderBackend` (create texture/mesh, begin frame, draw a batch, end frame, recover) with the validation every implementation shares, and `NullBackend`, a GPU-less implementation |
| `caps` | all | `Caps` / `ShaderModel`, and the floor: **Shader Model 3.0** in both stages and textures of 2048 or more (maintainers' decision, 2026-10-19) |
| `d3d9` | Windows | The Direct3D 9Ex implementation: off-screen targets with read-back or a window swap chain, textures (one level), meshes (16- or 32-bit indices), depth test, and the alpha modes opaque, test and blend |
| `scene` | all | The game's scene as this renderer takes it: textures (RGBA or DXT1/3/5, with their levels, or a mip chain made here), meshes under the game's ids, and frames of batches over a per-draw list, with the camera and the light; checks shared by every implementation |
| `d3d9` scene layer | Windows | `set_scene_mesh`/`set_scene_texture` (and their update, move and free), and `draw_scene`, one instanced draw per batch with the scene shader |
| `ui` | all | The interface as this renderer takes it: `omsi-ui`'s layers, draws and vertices, and the HUD's pictures; the layout, viewport and scissor rules shared with `omsi-ui` |
| `d3d9` interface layer | Windows | Each `omsi-ui` Gpu's pictures, atlas and vertex lists (`ui_image`, `ui_atlas`, `ui_vertices`), `draw_ui` and `draw_overlays`, with the interface shader (`shaders/ui.hlsl`, a port of `omsi-ui`'s) |
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
* `d3dcompiler_47.dll` is loaded from the system directory (`LOAD_LIBRARY_SEARCH_SYSTEM32`)
  the first time a shader is compiled, not bound at load time: a game that never opens this
  renderer starts without it, and a copy planted next to the game is not picked up.
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

The material's maps: the light map (as strong as the instance says; a terrain's detail
texture), the night map (by night, as strong as the instance says, or always when something
switches it; a terrain's night map), the sphere map of `[matl_envmap]` mixed in by its
factor (truncated and spilled as Direct3D's packed texture factor, `scene::env_factor`) or
by its mask's alpha, the bump map moving the sphere-map lookup, the material's specular
highlight lit at the vertex from the sun and the light above, and the material's texture
addressing (wrap, mirror, clamp, mirror once) in the samplers. A map that is not uploaded
reads as nothing.

The lamps: the map's lamps and the vehicles' lights from the default renderer's grid of
cells round the camera (`omsi-render`'s `Scene::frame_lights` and `frame_light_grid`), point
lights and spots, not on a `[nomaplighting]` material, a `[LightMapMapping]` one or a tree's
leaves; and the vehicles' interior lamps on the meshes their instances name. They are read
from two float textures (the light list, four texels a light, and the grid's indices); a
device without dynamic 32-bit float textures draws no lamps and says so in the log. The
reflection's light takes the lamps in, per pixel.

The tile light maps: the lamps baked into the map, one picture a tile, put up into the
default renderer's atlas of tiles round the camera (`MirrorEvent::LightMapTile`, placed by
`FramePlan::light_map`). They light the roads, the plates and the `[LightMapMapping]`
objects at night, as the ground's night map lights the ground; outside the loaded square
they light nothing.

Not yet (later phases): shadows and ambient occlusion, the detail noise, swaying trees, weather (rain, wet sheen, snow), the sky, coronas and smoke,
the fog's way through the player's vehicle, and mirrors and render-to-texture. The cut of the alpha-tested materials (`scene::ALPHA_CUT`, alpha above 128) is the
default renderer's; like `ALPHA_TEST_REF` it is a **provisional** parity item.

Known differences from the default renderer, each for a later PR:

* Only the vanilla ("classic") path is ported: the camera uniform's flag for the other
  looks (`sky_color.w`) is not read, so a frame asking for another look is drawn vanilla.
* The mip chains made here are box-filtered on the stored (gamma-encoded) values, and the
  samplers have no anisotropic filtering yet.
* The depth-only pass of the transmaps' fully opaque parts is missing, and the depth-only
  surface draws use the scaled terrain uv.
* The picture's own alpha is blended with one over what is there, as there, only where the
  device has separate alpha blending (`SEPARATEALPHABLENDENABLE`); elsewhere with the
  colour's factors. Compressed textures are refused where the device lacks the format.

The device keeps a system-memory copy of what each scene mesh and texture was made from, as
Direct3D 9's managed pool would (a 9Ex device has none; its default pool is used): a device
rebuilt after a removal (`recover`) puts them all up again under the same ids, so the game
sends nothing twice. The copies follow every change, move and free; `clear_scene` drops
them. The cost is the scene's asset memory once more in system RAM.

## The interface layer

The new interface draws as `omsi-ui` draws it: each draw with its layer's camera, layout
box, viewport, scissor, rounded clip and opacity, premultiplied alpha. Where the device
converts to sRGB after blending (`D3DPMISCCAPS_POSTBLENDSRGBCONVERT`, with sRGB reads and
writes for the formats used), the pictures are read as sRGB and the blend is in linear
light, as in the default renderer. Elsewhere it is all done on the stored values, which
differs only where alpha is partial (edges, fades). The HUD's pictures are drawn with the
same shader as quads in pixels. Pictures, atlas and vertex lists keep their pixels, so a
device opened again after a loss gets them back.

A Gpu can also draw into one of the renderer's render textures (the minimap and the city
map do): `omsi-ui`'s `Gpu::set_mirror_target` names the texture, and `draw_ui_into` draws
into a render target of this device under the same scene texture id, inside a frame or
outside one, which the HUD's overlays and the materials then show. A device opened again
has it back at the owner's next draw.

Known differences: no MSAA on the interface (`omsi-ui` draws the in-game overlay without it
too), and a picture `omsi-ui` takes from a render target (`add_view`) draws nothing.

## Dependencies and provenance

Only the `windows` crate (Microsoft, MIT OR Apache-2.0), already used by `omsi-render`, at the
workspace version, with the Direct3D 9, Direct3D (blob), GDI and library-loader features;
`winit` (workspace version) only for the example. All code is original to this repository.

## Testing

`cargo test -p omsi-render-d3d9` runs on every platform. On Windows it also

* compiles every shader for `vs_3_0`/`ps_3_0` (CPU only, so it runs on CI without a GPU), and
* runs device tests that draw small scenes off-screen and check the pixels: clear, texture
  times tint, untextured batches, alpha test at the reference, blend, depth order and the
  matrix layout, and `recover` rejecting old handles; and the scene layer: the texture under
  the ambient light, an unlit and a missing texture, fog, the alpha cut at 128, reversed
  depth, instances and invisible ones, a DXT1 texture, a missing mesh skipped, a frame
  pointing outside its data refused, and the scene coming back after `recover`; and the
  interface layer: colours over the frame, a layer's clip and opacity, pictures drawn and
  missing ones not, draws outside their buffer or a frame refused, the HUD's pictures
  straight and premultiplied, the interface drawn into a scene texture and shown, and the
  interface coming back after `recover`. They skip themselves only when there is
  no Direct3D 9Ex driver or the device is below the floor; any other failure to open fails
  them. The log says which adapter ran them.

Verified on the maintainer's PC (Intel UHD 630, 16/16 at the first phase-1 head): the shader
compile tests and the off-screen device tests; the window example opened. **Not covered by
any test:** `resize`, the window swap chain and present, `status` on a lost or occluded
device, and `recover` after a real device removal (it cannot be forced portably). The
scene layer's shader and device tests ran on the same PC at its first head (35/35); the
tests added since (unlit, culling, depth-only) ran there at 98da9fdd (38/38); the scene
coming back after `recover` has not run there yet. Not covered: the
road surfaces' pull and depth bias, border and mirror-once addressing, transmaps, and the
painted-ground kind.

## Next steps

The maintainers decided (2026-10-19) that the game gets this renderer as a render option,
"DirectX9 (D3D9Ex)", off by default. In that mode `omsi-render` keeps doing the CPU work
(scene, culling, level of detail, alpha order) on a device that draws nothing, and hands its
frame plan and asset events to this crate. In order:

1. The game opens with this renderer: the window, the frame loop and the asset events.
2. The option in the launcher's render options, off by default, Windows only.
3. The UI (menus, the Alt bar, the HUD) on this renderer (done: the new interface, the
   HUD's pictures, the minimap and the city map).
4. The OMSI material features (done: night, light and environment maps, bump maps, the
   specular highlight, the lamps and the tile light maps), then shadows, coronas, smoke and the sky, then mirrors and maps (render to texture).
5. The alpha-test cut and the other parity items from the maintainer's observation of the
   original.
