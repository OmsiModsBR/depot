# Discord Rich Presence: disabled

Status: **removed** (decision D5 by the maintainer). Resolves backlog S15.

## What was disabled

Everything that talked to Discord from the program:

* `crates/omsi-app/src/discord.rs` (the Rich Presence client: local IPC socket / named pipe
  `discord-ipc-N`, handshake, activity frames, the worker thread) and its tests;
* the **shared test application ID** constant (`DEFAULT_APP_ID`), which was not Depot's own
  registered application;
* the launcher's presence logic (`Launcher::update_discord`, the hand-off before a game starts)
  and the game's periodic presence update (map, bus, line, multiplayer flag);
* the settings `discord_status` and `discord_app_id` (in `omsi-launcher-core`, `settings.rs`),
  the Settings → General switch "Discord Rich Presence" and its help text;
* the translations of those two texts and the user-guide paragraph.

The code had no external crate dependency (the IPC was hand written), so no `Cargo.toml` or
`Cargo.lock` change was needed. Old `discord_status=` / `discord_app_id=` lines in a user's
`settings.cfg` are ignored; the launcher keeps unmanaged lines when it saves, so they stay in
the file, inert.

## What stays

The community links to the Discord server (the website, README, `CONTRIBUTING.md`, the issue
template config) are plain web links and were not touched. Historical `CHANGELOG.md` entries
are history and were not rewritten. Steam's rich presence is unrelated and unchanged.

## Why

The presence client identified the program to Discord with an application ID that belongs to
someone else's test application. Until Depot has its own registered Discord application ID,
nothing should speak to the local Discord client under another identity.

## How to revert

`discord-disabled.patch` in this directory is the code change of the commit that disabled it:

    git apply -R docs/removed/discord-disabled.patch

Before re-enabling, register a Depot application at the Discord developer portal and put its
ID in the new constant (and decide whether `discord_app_id` should stay user-overridable).
Edits made since that commit to the touched lines need a manual merge.
