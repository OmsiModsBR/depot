# Multiplayer, dedicated server and plugins removed

Date: 2026-10-29. Decision of the maintainer: none of this code will be reused, so it leaves the
repository (step A5 of `docs/MACRO_REFACTOR_DRAFT.md`). It had been switched off since the first
Depot release ([MULTIPLAYER_DISABLED.md](MULTIPLAYER_DISABLED.md)) and kept out of the game's
build behind the `lan`, `server` and `plugins` features of `omsi-app`.

## What went

- The crates `omsi-net` (LAN session, bridge, tunnels, WebSocket gateway) and `omsi-plugin`
  (OMSI plugin DLLs, the Lua plugins, `omsi-plugin-host` and the plugin demo).
- The `lan`, `server` and `plugins` features of `omsi-app` and everything behind them: `lan.rs`,
  `lan_world.rs`, `lan_mods.rs`, `admin.rs`, `server.rs`, `voice.rs`, `online.rs`, `plugins.rs`,
  `plugin_ui.rs` and `launcher/multiplayer.rs`, with their tests.
- The `lan` feature of `omsi-launcher-core` (checking a join code with `omsi-net`).
- What only that code used in the shared modules: the host's people, traffic and light programs
  mirrored on a client (`humans.rs`, `traffic.rs`, `scene.rs`, `schedule.rs`), the server's
  notification cards over the navigator (`hud`), the chat filter, the dedicated server's real-time
  pacing (`offscreen.rs`) and the launcher's saved-server list, with their tests.
- `scripts/build-server.sh`, `scripts/server/`, `scripts/build-plugin-host.sh`, the GreenTeaSpeak
  voice plugin (`tools/greenteaspeak-plugin`), `docs/SERVER.md`, `docs/PLUGINS.md` and the two CI
  steps that watched the features.

## What stays

The game's build never carried this code, so the game does what it did. The pieces that the
default build compiled and ran with empty data stay as they were: the `lan_off` and
`plugins_off` stand-ins, the command-line options `--lan-host`, `--lan-join` and `--server` (still
refused or ignored as before, so a launcher duty that names them does not fail), the chat box and
name tags of the HUD, the launcher's LAN fields of a duty and the voice setting. Taking them out
changes what a screen shows or what the command line accepts, so it is left for `fix/` PRs of
its own.

The removed code can be read in the repository's history, in the commit that removed it.
