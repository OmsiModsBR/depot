# Changelog

Changes made by the Project Depot team. History inherited from openOMSI (up to the rebranding
release 0.2.17) is frozen in [CHANGELOG_UPSTREAM.md](CHANGELOG_UPSTREAM.md).
Releases are published manually after review; see [docs/VERSIONING.md](docs/VERSIONING.md).

## Unreleased

### Added
- Navigator map packages: the navigator and the city map can draw a surface (water, green, rivers, names),
  icons, regions, street names and bus lines supplied by an optional per-map package
  (`docs/MAP_PACKAGE.md`), also looked up in the content folder and used for offscreen pictures. New
  settings `nav_surface` and `nav_light`. Source: maintainer (Leo, `lmonteirotech`), fork
  lmonteirotech/openOMSI branch `feat/map-package` (commits `80fb1482`, `dbcb93f9`, `4c7a284d`,
  `d898c8fb`), MIT. Example packages (OpenStreetMap/ODbL data) left out pending licence review. Not
  validated against real OMSI 2 content; only build and automated tests were run.
- Outside camera (F3) gets an independent `outside_fov` setting, 20-120 vertical degrees or Auto. Auto
  preserves the 16:9 horizontal extent on ultrawide windows when no general override is set. Source:
  openOMSI `ea47e092ee6e0a2c472d2c7261ea7a7913e6ca9f` (MIT), selective backport authored by zane_thruston;
  no later upstream architecture imported. See `docs/integrations/OUTSIDE-FOV-ULTRAWIDE.md`.
- Articulated buses can opt into a pusher joint with `[coupling_pitch_offset]` in the rear section's
  `.bus`. Without the key, or with an invalid/zero offset, the baseline puller behaviour is unchanged.
  Source: openOMSI `a97595b1b254cf94abc27bb67844a8a1dfcee2ab` (MIT), authored by zane_thruston
  (contribution confirmed MIT by the maintainer). See `docs/integrations/PUSHER-ARTICULATION.md`.

### Changed
- Articulated buses and trailers: every section is a body of its own. Behind the player's vehicle the rear
  section of an articulated bus and a lorry's trailer run on the same wheel physics as the front, each with
  its own `.bus` mass, inertia, centre of gravity, axles, springs and brakes, held by a joint that follows
  `[coupling_front_character]` (a bus turntable keeps the sections from rolling against each other, a lorry's
  hitch does not). The AI's vehicles, LAN players and trains on rails follow as before, and so does every part
  with `OMSI_KINEMATIC_TRAILERS=1` (A/B). Source: maintainer's fork lmonteirotech/openOMSI branch
  `feat/multibody-articulation`, code authored by zane_thruston, MIT (confirmed by the maintainer); comments
  and docs cleaned of references to addresses of the reference executable. High-risk physics change, not
  validated against real articulated buses. See `docs/FORMATS.md`, `docs/MODDING.md`.

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

### Fixed
- Timetable: the delay shown on the way between two stops follows where the bus is (the last stop's
  departure and the next arrival shared out by the distance covered) instead of standing still at the delay
  it left the last stop with (upstream #1898). Behaviour taken from openOMSI commit `3c75499` (MIT,
  turbo-devv), reimplemented; the existing test now expects the new value. Parity with the reference still
  to be confirmed by observation.
- Sound: an entry that is about to start playing starts at full volume. DirectSound refuses a request
  over 0 dB and keeps the buffer's last volume, so a `[volume]` over 1 that a curve or condition held at 0
  while silent started "audible" at gain 0 (hand brake, door-closing and indicator sounds; upstream #1851,
  #1852; may contribute to #1418, which still needs a reproduction with the reporter's `sound.cfg`).
  Reimplemented from the behaviour described in openOMSI commit `ea26060` (MIT, turbo-devv); regression
  tests added.
- `omsi-plugin` test `panels_clicks_and_focus_through_omsi_ui` failed on Windows CI because the reload is detected by
  modification time and the rewritten file could keep the old timestamp; the test now sets a newer one. The Lua
  feature itself is unchanged (it stays in the crate; it is only unreachable from the client).

### Project
- Identity and links point to OmsiModsBR/depot, omsimods.com.br and omsimods.com.br/discord; new logos and icons.
- LICENSE keeps the upstream copyright and adds the Project Depot line.
- CI: informational rustfmt, clippy, cargo audit and cargo deny (`docs/QUALITY.md`, `deny.toml`);
  blocking check of the committed Steam binaries' hashes; provenance notes in
  `docs/DEPENDENCY-PROVENANCE.md`.
