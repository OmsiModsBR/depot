# Contract: where the rendering code lives

The directories are the spec's ("Code structure", approved 2026-11-13). This page adds the file
level and the import rules the CI check enforces. Reasons: [research.md](../research.md), R-7 to
R-10.

## Directories and what moves into them

```text
crates/omsi-render/src/
├── interface/
│   ├── mode.rs            the trait a mode implements; the parts the core hands it
│   └── frame_plan.rs      FramePlan and its parts              <- renderer/mirror.rs
├── modes/
│   ├── vanilla_plus/
│   │   ├── mod.rs         implements interface::mode
│   │   ├── pipelines.rs   the puddles' own pass, FXAA without MSAA, sprite pipelines   <- renderer/build.rs
│   │   ├── frame.rs       its frame decisions                 <- renderer/frame.rs, encode.rs, passes.rs
│   │   ├── sky.rs         the sky from the envir.cfg pictures <- passes/sky.rs, sky/resources.rs (its part)
│   │   └── shaders/       sky.wgsl; corona, smoke and snow fragment entries; fs_vanilla_reflections
│   └── enhanced/
│       ├── mod.rs         implements interface::mode
│       ├── pipelines.rs                                        <- renderer/build.rs
│       ├── frame.rs       HDR frame, masked frame, lead view  <- renderer/frame.rs, encode.rs, prepare.rs
│       ├── lights.rs      linear colours, spot profiles       <- lighting/model.rs, renderer/lights.rs (its part)
│       ├── post.rs                                             <- passes/post.rs
│       ├── exposure.rs                                         <- lighting/exposure.rs
│       ├── atmosphere/    atmosphere, air, clouds, probe       <- sky/{atmosphere,air,clouds,probe}.rs
│       ├── fog_lamps.rs                                        <- passes/fog_lamps.rs
│       ├── targets.rs                                          <- targets/hdr.rs
│       └── shaders/       enhanced.wgsl, enhanced_common.wgsl, sky_enhanced.wgsl, post.wgsl,
│                          fog_lamps.wgsl, lamp_air.wgsl; corona, smoke and snow fragment entries
├── gpu/ assets/ lighting/ sky/ scene/ targets/ passes/ effects/ shaders/ output/
│                          the core: what both modes run, **including the base surface shading**
│                          (fs_main), which Enhanced uses for the mirrors' pictures (research R-8)
└── renderer/              builds the one mode from RenderOptions::path; runs the frame through it

crates/omsi-render-vanilla/src/     (renamed from omsi-render-d3d9)
├── domain/            scene.rs, ui.rs, caps.rs, render_scale.rs, glass_rain.rs
├── interface/         input.rs: RenderBackend and its descriptors
└── implementation/    d3d9/ (device, passes, HLSL), null.rs (NullBackend)

crates/omsi-app/src/output/implementation/
├── backend.rs         Backend::of(mode)
└── vanilla.rs         FramePlan -> SceneFrame                  <- d3d9.rs
```

A file listed with "(its part)" is split: the part both modes run stays where it is. Where a move
finds that a file is used by both modes after all, it stays in the core and this page is corrected
in the same pull request.

## Import rules (`scripts/check-render-layers.sh --strict`, blocking in CI)

Layer order, lowest first:

```text
gpu < shaders < assets | lighting < sky < scene < targets < passes < effects
    < interface < modes/vanilla_plus | modes/enhanced < output < renderer < api
```

1. `modes/vanilla_plus` and `modes/enhanced` never import each other (same layer).
2. Nothing below `modes` imports `modes` (imports point down).
3. A mode imports the core and `interface` only; it never imports `output`, `renderer` or `api`.
4. Only `renderer` names both modes, in the one function that builds the mode.
5. `omsi-render-vanilla` does not depend on `omsi-render`, and `omsi-render` does not depend on it;
   only `omsi-app` depends on both (checked from `Cargo.toml` by the same script).
6. No `.wgsl` file under `modes/<a>/shaders/` is included by `modes/<b>` or by the core.

## Tests the contract asks for

- the layer script fails on a planted import for each of rules 1 to 3 (`scripts/tests/`);
- `cargo test -p omsi-render`: every module each mode assembles validates with naga;
- building the renderer with `ShadingPath::Linear` creates no Enhanced pipeline, look-up table or
  probe, and the other way round (counted through the `testing` module);
- the offscreen golden and the GPU tests pass with unchanged references after every move;
- `omsi-app/tests/`: the same `FramePlan` through `vanilla.rs` on `NullBackend` records the same
  batches, coronas, sky and lights.
