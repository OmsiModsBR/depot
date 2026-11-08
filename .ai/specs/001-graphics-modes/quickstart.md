# Quickstart: how each phase is validated

See the [plan](plan.md) (phases), [research](research.md) (R-V, R-E) and [contracts](contracts/).

## Prerequisites

- Windows x64 with OMSI 2 installed (`OMSI_ROOT`), for phases 0, 2, 4 and 5.
- Without a graphics card: lavapipe for `omsi-render`'s `#[ignore]` GPU tests (point
  `VK_ICD_FILENAMES` at its manifest).
- Measuring the wgpu modes: one process per mode (several renderers in one process run out of memory
  on a small machine), the scene of [research.md](research.md#baseline-measurements).

## Q1. Phase 0: wgpu Vanilla x DirectX9 x OMSI 2

For each row V1-V14 of [research.md](research.md), three screenshots from the same camera: OMSI 2,
Depot with `graphics=vanilla` on DirectX 12 (while it still exists), and Depot on DirectX9.

1. A map at night in the rain, a bus with `[matl_raindropmap]` and `[light_enh]`, a street lamp and a
   tree with a transmap in view.
2. The same camera in all three: `OMSI_INPUT` with keys and `shot` at the same times (for example
   `t=25 key F3; t=28 shot X; t=29 key F4; t=32 shot Y`).
3. Result: an "OMSI 2 = ?" column filled in for each row (decision D1).

Expected: V1, V2, V4, V5, V7 confirmed for DirectX9; V10, V11 and V12 decide whether `fix/dx9-*`
pull requests are needed.

## Q2. Phase 1: Enhanced+ removed

```text
cargo test --workspace
cargo build --release
grep -rnE "ray_tracing|RayTracer|enhanced_plus|ENHANCED_PLUS|RT_GBUF|NO_RT|DEBUG_RT|rt_frame" crates docs
```

Expected: tests pass; `grep` finds only the `enhanced_plus` migration in `GraphicsMode::parse` (or
`graphics_mode` before phase 3), its tests and `CHANGELOG.md`. A `settings.cfg` with
`graphics=enhanced_plus` opens in Enhanced and the log says so once. `tests/rt_smoke.rs` is deleted
with the code. Enhanced's pixels are unchanged (same-camera screenshot).

## Q3. Phase 2: glass lets the sun through (Enhanced)

- New test in `omsi-render/tests/` (lavapipe): an opaque panel, a pane and the ground; with the sun
  behind them, the opaque panel's shadow is dark, the pane's lies between it and the lit ground, and a
  pane with a higher alpha lets less through. It fails without the change (Principle VI).
- In the game: the bus cab in the sun, before and after, same camera.
- Cost (a new feature, Principle VII): the Enhanced measurement before/after, and a session with
  `OMSI_PROFILE=1` on the same scene; the pull request carries both numbers.

## Q4. Phases 0 and 5: DirectX9 against wgpu Vanilla

Same map, bus, stretch and traffic, `OMSI_PROFILE=1`, 5 minutes each:

1. `graphics=vanilla`, `graphics_api=dx12`;
2. `graphics_api=d3d9`.

Compare the `profile window:` lines (frame, GPU, stages) and the loading time. Phase 5 goes in only if
(2) is not slower than (1); otherwise the difference becomes a `fix/dx9-*` first.

## Q5. Phases 3 and 5: the mode resolved once; Vanilla = DirectX9

- `omsi-app/tests/app/settings/`: every row of [contracts/settings-cfg.md](contracts/settings-cfg.md).
- `grep -rn "CLASSIC\|ENHANCED\.load\|graphics_mode(" crates` is empty outside `GraphicsMode`.
- Launcher: in Vanilla, the graphics API list, VR and triple screen are hidden (decision D3); in
  Vanilla+ and Enhanced, DirectX9 is not in the API list.
- The game with `graphics=vanilla`: the log says it opened Direct3D 9Ex.

## Q6. Phase 6: wgpu Vanilla removed

- `Lighting::classic` does not exist; `shader.wgsl` has no `classic` branch.
- `offscreen_golden` (lavapipe, CI) and `omsi-render`'s GPU tests pass **without changing** the
  golden: Vanilla+ does not change a pixel.

## Q7. Phase 7: only the mode's pipelines

- The Vanilla+ start-up log has no "compiling the post passes shaders", "lamps in the fog" or
  `fs_snow_enhanced` lines.
- Renderer start-up time before/after on lavapipe (about 37 s today in any mode) and on a graphics
  card with the driver's shader cache cleared.

## Q8. Refactor phases (8-14)

Each refactor pull request shows that nothing a player sees changed:

- `cargo test --workspace` and `cargo build --release`; the physics and offscreen traces of CI come
  out identical (no `physics-delta` / `offscreen-delta` label);
- `offscreen_golden` and `omsi-render`'s GPU tests (lavapipe) pass without changing a golden;
- the mode measurement of [research.md](research.md#baseline-measurements) before and after, within
  noise; for phases 8 and 11, also a `profile window:` comparison on the same scene;
- phase 8: unit tests of `FrameFeatures::decide` for each view (window, mirror, headset eye, triple
  panel) and each path, without a GPU;
- phase 12: the contract test runs the same `FramePlan` through the translation on `NullBackend` and
  checks the batches, coronas, sky and lights it records;
- phase 13: the number of `#[cfg(test)]` modules in `src/` of the crate drops to zero, and the test
  count of `cargo test -p <crate>` stays the same.
