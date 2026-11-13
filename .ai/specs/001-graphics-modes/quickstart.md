# Quickstart: how each phase is validated

See the [plan](plan.md) (phases), the [spec](spec.md), [research](research.md) and
[contracts](contracts/).

## Prerequisites

- Windows x64 with OMSI 2 installed (`OMSI_ROOT`), for phases A0, A2, A4, A5 and B4.
- Without a graphics card: lavapipe for `omsi-render`'s `#[ignore]` GPU tests (point
  `VK_ICD_FILENAMES` at its manifest).
- Measuring the wgpu modes: one process per mode (several renderers in one process run out of memory
  on a small machine), the scene of [research.md](research.md#baseline-measurements).

## Q1. Phase A0: wgpu Vanilla x DirectX9 x OMSI 2 (FR-007)

For each row V1-V14 of [research.md](research.md), three screenshots from the same camera: OMSI 2,
Depot with `graphics=vanilla` on DirectX 12 (while it still exists), and Depot on DirectX9.

1. A map at night in the rain, a bus with `[matl_raindropmap]` and `[light_enh]`, a street lamp and a
   tree with a transmap in view.
2. The same camera in all three: `OMSI_INPUT` with keys and `shot` at the same times (for example
   `t=25 key F3; t=28 shot X; t=29 key F4; t=32 shot Y`).
3. Result: an "OMSI 2 = ?" column filled in for each row by the maintainer (decision D1).

Expected: V1, V2, V4, V5, V7 confirmed for DirectX9; V10, V11 and V12 decide whether `fix/dx9-*`
pull requests are needed (phase A4).

## Q2. Phases A0 and A5: DirectX9 against wgpu Vanilla (FR-008)

Same machine, map, bus, stretch and traffic, `OMSI_PROFILE=1`, 5 minutes each:

1. `graphics=vanilla`, `graphics_api=dx12`;
2. `graphics_api=d3d9`.

Compare the `profile window:` lines (frame, GPU, stages) and the loading time. Phase A5 goes in only
if (2) is not slower than (1). If it is, the difference becomes a `fix/dx9-*` and the measurement
is repeated; there is no tolerance.

## Q3. Phase A1: Enhanced+ removed (FR-003)

```text
cargo test --workspace
cargo build --release
grep -rnE "ray_tracing|RayTracer|enhanced_plus|ENHANCED_PLUS|RT_GBUF|NO_RT|DEBUG_RT|rt_frame" crates docs
```

Expected: tests pass; `grep` finds only the `enhanced_plus` migration, its tests and `CHANGELOG.md`.
A `settings.cfg` with `graphics=enhanced_plus` opens in Enhanced and the log says so once;
`--enhanced-plus` and the removed variables are ignored with a log line. `tests/rt_smoke.rs` is
deleted with the code. Enhanced's pixels are unchanged (same-camera screenshot; golden).

## Q4. Phase A2: glass lets the sun through (User Story 4)

- New test in `omsi-render/tests/` (lavapipe): an opaque panel, a pane and the ground; with the sun
  behind them, the opaque panel's shadow is dark, the pane's lies between it and the lit ground, and a
  pane with a higher alpha lets less through. It fails without the change (Principle VI).
- In the game: the bus cab in the sun, before and after, same camera.
- Cost (a new feature, Principle VII): the Enhanced measurement before/after, and a session with
  `OMSI_PROFILE=1` on the same scene; the pull request carries both numbers.

## Q5. Phases A3 and A5: the mode resolved once; Vanilla = DirectX9 (User Story 1)

- `omsi-app/tests/app/settings/`: every row of [contracts/settings-cfg.md](contracts/settings-cfg.md).
- `grep -rn "CLASSIC\|ENHANCED\.load\|graphics_mode(" crates` is empty outside `GraphicsMode`.
- The game with `graphics=vanilla`: the log says it opened Direct3D 9Ex. With `vr=1` or
  `triple_screen=1` as well: Vanilla, one log line, neither applied.
- With the Direct3D 9Ex device made to fail (`NullBackend` refusing, or a machine without
  `d3dcompiler_47.dll`): the error names the cause and says "choose Vanilla+ in the launcher"; the
  game closes; `settings.cfg` is unchanged (FR-011a).

## Q6. Phase A6: wgpu Vanilla removed (FR-002)

- `Lighting::classic` does not exist; `shader.wgsl` has no `classic` branch.
- `offscreen_golden` (lavapipe, CI) and `omsi-render`'s GPU tests pass **without changing** the
  golden: Vanilla+ does not change a pixel.

## Q7. Phases B1 to B3: the options per mode (User Story 2)

- The tests of [contracts/graphics-options.md](contracts/graphics-options.md) pass.
- By hand, once per mode (set the mode in the launcher, then start the game):
  1. launcher, Settings > Graphics: the top section is the mode; "Graphics API" is in "Picture" in
     Vanilla+ and Enhanced and absent in Vanilla; no VR or triple screen in Vanilla;
  2. every row offered changes something in that mode after a start (walk the list);
  3. in the game, Options > Graphics: no mode, no API, no preset, no row with a "next start" hint;
     every row changes the picture in the session;
  4. set night brightness in Enhanced, switch to Vanilla+, save, switch back: the value is still
     there.
- `grep -rn "api_limits\|lock_vanilla\|Backend::D3d9" crates/omsi-app/src/hud crates/omsi-launcher-ui/src`
  is empty.

## Q8. Phase B4: an option made live (FR-020e), per option

- New test in `omsi-render/tests/` (lavapipe): `render_to_image` with the option off, switch it on
  in the same renderer, render again: the second picture equals the picture of a renderer built
  with the option on; and the reverse.
- Frame time with the option unchanged, before and after the pull request, within noise.
- In the game: the row appears in Options > Graphics and acts at once.

## Q9. Phase C6: only the mode's pipelines (FR-026, SC-007)

- The test of [contracts/render-structure.md](contracts/render-structure.md) that counts what a
  `ShadingPath::Linear` renderer builds.
- The Vanilla+ start-up log has no "compiling the post passes shaders", "lamps in the fog" or
  `fs_snow_enhanced` lines.
- Renderer start-up time before/after on lavapipe (about 37 s today in any mode) and on a graphics
  card with the driver's shader cache cleared.

## Q10. Structure and refactor phases (C1 to C10)

Each pull request shows that nothing a player sees changed (FR-030):

- `cargo test --workspace` and `cargo build --release`; the physics and offscreen traces of CI come
  out identical (no `physics-delta` / `offscreen-delta` label);
- `offscreen_golden` and `omsi-render`'s GPU tests (lavapipe) pass without changing a golden;
- the mode measurement of [research.md](research.md#baseline-measurements) before and after, within
  noise; for phases C1, C4, C5 and C7, also a `profile window:` comparison on the same scene;
- `scripts/check-render-layers.sh --strict` passes; from C4 on it knows `interface` and `modes`;
- `wc -l` of the crate's `src/` before and after: not larger, except where the pull request says
  what it added and why (FR-030, SC-009);
- C1: unit tests of the frame decisions for each view (window, mirror, headset eye, triple panel)
  and each path, without a GPU;
- C5: after the two moves, `grep -rnE "\benhanced\b" crates/omsi-render/src --include=*.rs` outside
  `modes/enhanced`, `interface` and the one build function finds comments only; deleting
  `modes/enhanced` (in a scratch copy, with its one `mod` line and build arm) still builds and draws
  Vanilla+, and the other way round (User Story 3, Independent Test);
- C8: `cargo tree -p omsi-render-vanilla` has no `omsi-render`; the contract test runs the same
  `FramePlan` through the translation on `NullBackend`;
- C9: the number of `#[cfg(test)]` modules in `src/` of each crate is zero, and the test count of
  `cargo test -p <crate>` stays the same (SC-010).
