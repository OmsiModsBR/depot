# Dependency provenance

## gpu-allocator (git fork)

`Cargo.toml` patches `gpu-allocator` 0.28.0 to
`https://github.com/openOMSI-Project/gpu-allocator` at the pinned commit
`6ad8a7eeb9c3e1dcd65656f718c593a0a1a3915d`. The fork is only used for this patch; `deny.toml` allows this one git source.

Difference from the published crates.io `gpu-allocator 0.28.0` (checked 2026-10-10; only `src/vulkan/mod.rs` and the manifest differ):
for `MemoryLocation::CpuToGpu`, memory types that are host-visible VRAM on a small BAR heap
(heap <= 512 MiB, i.e. without Resizable BAR) are skipped, with a fallback to a host-visible
and host-coherent type outside such a heap. New helper `is_small_bar`. Purpose: avoid exhausting
the 256 MiB BAR window shared with other applications.

Pending: the fork lives under the old upstream organisation (`openOMSI-Project`); mirror it under
`OmsiModsBR` or vendor it, and review this diff on every wgpu update. The same applies to the
other forked git dependencies listed in the upstream report (`wgpu-openomsi`, `angle-openomsi`,
`egui_retained`).

## Steam redistributables

See `assets/steam_redist/README.md` (hashes recorded and verified in CI; origin and SDK version still undocumented).
