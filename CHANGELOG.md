# Changelog

Changes made by the Project Depot team. History inherited from openOMSI (up to the rebranding
release 0.2.17) is frozen in [CHANGELOG_UPSTREAM.md](CHANGELOG_UPSTREAM.md).
Releases are published manually after review; see [docs/VERSIONING.md](docs/VERSIONING.md).

## Unreleased

### Added
- Articulated buses can opt into a pusher joint with `[coupling_pitch_offset]` in the rear section's
  `.bus`. Without the key, or with an invalid/zero offset, the baseline puller behaviour is unchanged.
  Source: openOMSI `a97595b1b254cf94abc27bb67844a8a1dfcee2ab` (MIT), authored by zane_thruston
  (contribution confirmed MIT by the maintainer). See `docs/integrations/PUSHER-ARTICULATION.md`.

### Removed (reversible records in [docs/removed](docs/removed))
- In-app updater (check, download, install, `update_*` settings, flags, UI, Android install flow). The
  Settings "About" section shows the version and a link to the releases page. `docs/removed/UPDATER_DISABLED.md`.
- Discord Rich Presence and its borrowed application ID, until Depot registers its own. The community
  Discord links stay. `docs/removed/DISCORD_DISABLED.md`.
- Worker "playing now" presence counter. `docs/removed/PRESENCE_WORKER.md`.
- Multiplayer, dedicated server and Lua plugins are disabled in the client (code kept, unreachable).
  `docs/removed/MULTIPLAYER_DISABLED.md`.
- RAR and 7z installation and automatic ZIP extraction: the installer takes a folder or a `.zip`, which
  is stored under `Archives/` unextracted. Drops `unrar-rs`, `sevenz-rust2`, `reedsolomon-rs` (GPL-3.0
  dependency removed). `docs/removed/RAR_7Z_ZIP_EXTRACTION_REMOVED.md`.
- Donation links and button on the website.

### Security
- ZIP64: capacity and allocation limits in the archive reader (`docs/security/ZIP64_FIX.md`).
- ZIP entry names: length/depth caps and no panic on non-ASCII case folding.
- `.x` models: no panic on non-ASCII bytes, nesting limit; terrain size limit and clamped sampling.
- Patchwork respects the station cap; outline self-crossing test no longer quadratic.
- One shared ceiling for texture sizes and list counts in content parsers (`omsi_cfg::limits`),
  mirrors and dynamic textures bounded.
- HTML/JS engine: parse and DOM depth limits, bounded arrays/strings, no deadlock on self-referencing
  arrays, bounded inline-block layout, `font-size` clamp; script textures and `STDrawRect` bounded;
  script string builtins capped.
- The terrain editor no longer overwrites an unreadable `.terrain`, and writes atomically with a `.bak`.
- Radio and METAR URLs: https only, public hosts only, ICAO code validated and encoded.
- Head tracking listens on 127.0.0.1 by default (`head_tracking_bind` to change).
- Android: `env.txt` is limited to an allow-list, no default signing password, unused permissions dropped.
- Website: Markdown sanitised with DOMPurify, Content-Security-Policy, marked and DOMPurify vendored.
- Release builds use `overflow-checks`.

### Project
- Identity and links point to OmsiModsBR/depot, omsimods.com.br and omsimods.com.br/discord; new logos and icons.
- LICENSE keeps the upstream copyright and adds the Project Depot line.
- CI: informational rustfmt, clippy, cargo audit and cargo deny (`docs/QUALITY.md`, `deny.toml`);
  blocking check of the committed Steam binaries' hashes; provenance notes in
  `docs/DEPENDENCY-PROVENANCE.md`.
