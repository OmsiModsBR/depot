# omsi-render-d3d9

Foundation for a **separate legacy Direct3D 9 renderer**. It is a first step only.

## What this is not

* It is **not wired into the game**. No crate depends on it and the game still renders through
  `omsi-render` (wgpu); the default renderer and the inherited backends are untouched.
* It **declares no supported hardware**. Widening the hardware matrix is a product decision;
  nothing here changes `docs` on supported platforms.
* It draws nothing yet (no meshes, textures, shaders or alpha modes).

## What is in it

| Part | Platforms | Content |
| --- | --- | --- |
| `backend` | all | `RenderBackend`, the small interface the renderer must satisfy (create texture/mesh, begin frame, draw a batch, end frame, recover from device loss), and `NullBackend`, a GPU-less implementation that validates arguments so the interface is testable everywhere |
| `caps` | all | `Caps` / `ShaderModel`: the device facts a reduced feature profile needs |
| `d3d9` | Windows | Opens a HAL device on the default adapter, reads its capabilities, clears an off-screen render target and reads it back, reports device loss and resets |

## Dependencies and provenance

Only the `windows` crate (Microsoft, MIT OR Apache-2.0), already used by `omsi-render`, at the
workspace version, with the `Win32_Graphics_Direct3D9` feature. No new packages enter
`Cargo.lock` besides this crate. All code is original to this repository.

## Testing

`cargo test -p omsi-render-d3d9` runs the interface tests on every platform. On Windows it also
runs a device test that skips itself only when no Direct3D 9 driver is present (any other
failure to open fails the test), and prints the adapter it ran on.

Verified so far: opening the device, reading its capabilities, and clearing and reading back an
off-screen target (maintainer's PC, Intel UHD 630). **Not tested:** device loss and `reset()`,
which cannot be forced portably.

## Next steps (each its own change, after the `omsi-render` refactor in #61 finishes)

1. Inventory of the operations the Vanilla profile needs, taken from the modularised
   `omsi-render`.
2. Golden off-screen scenes and a tolerance metric to compare this renderer with the wgpu one.
3. Meshes, textures, the OMSI alpha modes and HLSL SM3.0 shaders behind `RenderBackend`.
4. Selecting the renderer in the game, behind a decision by the maintainers.
