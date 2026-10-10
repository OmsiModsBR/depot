# Steam redistributable libraries

Prebuilt Steamworks runtime libraries loaded by the `steamworks` crate (0.13.1).

| File | Size (bytes) | SHA-256 |
| --- | --- | --- |
| steam_api64.dll | 319128 | e6d9bafb9a41e42fba7b21553db49f8719027af3f0deb23ff86cfc44d60e776d |
| libsteam_api.so | 385840 | eb2dd015b84177cf4f4326fe578aab375fd8931bbbd719c7492420d9777007fe |
| libsteam_api.dylib | 428960 | a37faf19675b943a3c9717d25d9a881f5addb7f0ee728e03de7aac143ba759d2 |

* Added upstream in commit `3d875f2` (2026-10-07), inherited by Depot with the baseline.
* **Origin and SDK version: unknown / not documented.** The libraries must come from the official
  Steamworks SDK; the exact SDK version and download source still have to be recorded here by a
  maintainer, together with confirmation that the Steamworks SDK terms allow redistributing them in
  this repository (the SDK is not MIT).
* `SHA256SUMS` is checked by the `provenance` job of `.github/workflows/depot-ci.yml`, so any change to
  these binaries fails CI until the hashes and this file are updated in the same reviewed commit.
