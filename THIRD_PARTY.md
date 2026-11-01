# Third-party code in Project Depot

Project Depot's own code is MIT licensed (see [LICENSE](LICENSE)). The history of the code taken
from openOMSI is in [UPSTREAM.md](UPSTREAM.md), and the Rust and native dependencies are listed
in [docs/DEPENDENCY-PROVENANCE.md](docs/DEPENDENCY-PROVENANCE.md). This file lists source code
from other projects that was **adapted into this repository's own files**.

## AMD FidelityFX Super Resolution 1 (FSR 1)

| | |
| --- | --- |
| Used for | The optional "Sharper (FSR 1)" up-scaling of the render scale under "DirectX9 (D3D9Ex)" |
| Where | `crates/omsi-render-d3d9/src/d3d9/shaders/upscale.hlsl`, entry points `ps_easu` and `ps_rcas` |
| Source | https://github.com/GPUOpen-Effects/FidelityFX-FSR, tag `v1.0.2` |
| Commit | `a21ffb8f6c13233ba336352bdff293894c706575` (2021-08-20) |
| Files used | `ffx-fsr/ffx_fsr1.h` (`FsrEasuF`, `FsrEasuTapF`, `FsrEasuSetF`, `FsrRcasF`, `FsrEasuCon`, `FsrRcasCon`); `ffx-fsr/ffx_a.h` only for the meaning of its helper functions |
| Author / owner | Advanced Micro Devices, Inc. |
| Licence | MIT, "Copyright (c) 2021 Advanced Micro Devices, Inc. All rights reserved." (`license.txt` of the repository at that commit; the same text heads `ffx_fsr1.h`) |
| Method | **Adapted** (not copied verbatim, not a clean-room rewrite): the algorithm and its constants are AMD's; the code was rewritten for Shader Model 3.0 |
| Taken in | 2026-11-01, by the render scale change (see CHANGELOG) |

What was changed from the reference, so that it runs as a `ps_3_0` pixel shader:

- The reference reads four texels at once with `gather4`; Shader Model 3.0 has no such read,
  so the twelve texels of the kernel are read one by one, at their centres, with point
  sampling.
- The reference's approximations of 1/x and 1/sqrt(x) (integer bit tricks) are replaced by the
  exact `rcp()` and `rsqrt()`. Because `rcp(0)` is infinite where the approximation stays
  finite, the divisor of the edge-length term is held off zero, and a hair is added to the two
  divisors of the sharpening pass's limiters, which are zero on pure black and pure white (the
  reference's 0 times 1/0 would be NaN there).
- The 16-bit paths, the packed variants and the optional noise removal are not used.
- The sharpening strength is a fixed 0.25 stops below the maximum (`RCAS_STOPS` in
  `render_scale.rs`); there is no option for it.
- The two passes run on a Direct3D 9Ex device through the Rust code in
  `crates/omsi-render-d3d9/src/d3d9/upscale.rs`, which is Depot's own.

The licence text, as it must stay with the adapted code, is in the header of `upscale.hlsl`.

**Trademarks.** "FidelityFX" and "FSR" are trademarks of Advanced Micro Devices, Inc. They are
used here only to say which published algorithm the option is adapted from; Project Depot is not
affiliated with or endorsed by AMD.

**Patents: not assessed.** The MIT licence AMD publishes carries no patent grant or patent
notice, and nothing in the repository mentions patents. Whether any patent covers the
algorithm was not examined (no legal review was done). The maintainers should decide whether
that is acceptable before the option ships in a release; the option is off by default and can
be removed by deleting `ps_easu` and `ps_rcas` and the `Fsr1` choice.

FSR 2, FSR 3 and the FidelityFX SDK are different works with their own licences and are not
used.
