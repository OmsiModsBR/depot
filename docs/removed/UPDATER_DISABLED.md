# In-app updater: disabled

Status: **removed** (product decision by the maintainer). Supersedes backlog items S6, S7 and D3.

## What was disabled

All self-update functionality of the launcher and the game:

* the update check (GitHub API `releases/latest`, the github.com redirect fallback), the
  periodic re-check and the in-game background check (`update_watch.rs`);
* the download (with range resume and SHA-256 check) and the install: unpacking an archive
  beside the program, swapping files (`*.old-update`, `.depot-update`, `.depot-files`),
  relaunching the launcher, the Android PackageInstaller hand-over;
* settings `update_check`, `update_auto`, `update_notify` (no longer offered, read or written;
  old lines in a user's `settings.cfg` are kept as inert unknown keys);
* environment variables `OMSI_NO_UPDATE` and `OMSI_UPDATE_URL`;
* the launcher UI: the update dialog, the "Updated to ..." banner, the Settings → General
  "Updates" section (toggles, "Check now"), the navigator "update available" card kind;
* Android: `OmsiActivity.installApk`, `getInstallStatus`, `getInstallMessage`, the
  `org.openomsi.game.INSTALL_STATUS` result handling (an exported activity that acted on an
  intent without checking who sent it, audit S10), the "install unknown apps" flow, the
  `REQUEST_INSTALL_PACKAGES` permission, the rescue-update after a start that never drew
  (`rescue_update`, `start-pending`);
* the translations of the removed texts, and the documentation of the feature.

## What remains

* The version (`about::current_version`), shown in Settings → General → About and in crash
  reports.
* A plain, static link to the release page, `https://github.com/OmsiModsBR/depot/releases`,
  opened by the system browser **only when the player clicks it**. `about::open_url` refuses
  every URL that is not `https://github.com/OmsiModsBR/depot` or a path/query/fragment
  inside it (tested in `about::tests`). The crash dialog's "Report on GitHub" uses the same
  function.

## Where

| Was | Now |
| --- | --- |
| `crates/omsi-app/src/updater.rs` | `crates/omsi-app/src/about.rs` (version, validated link only) |
| `crates/omsi-app/src/update_watch.rs` | deleted |
| `crates/omsi-app/src/launcher/update.rs` | `launcher/dialogs.rs` (only the crash and disconnect dialogs that lived in the same file) |
| `launcher/pages.rs` Updates section | "About" section |
| `omsi-launcher-core` settings defaults / parse / write | the three `update_*` keys removed |
| `android/java/.../OmsiActivity.java`, `AndroidManifest.xml`, `android.rs` | install code and permission removed |

## Why

A self-replacing binary downloaded from a network location is the highest-impact thing an
application can do; Depot does not want it until releases are signed and the update channel
has been reviewed. Players download releases manually.

## How to revert

`updater-disabled.patch` in this directory is the code change of the commit that disabled the
updater (`git show <sha> -- . ':!docs/removed'`). To bring the feature back:

    git apply -R docs/removed/updater-disabled.patch

(the reverse patch applies to the tree as it was right after that commit; later edits to the
same lines need a manual merge). Before re-enabling, review at least: signature verification of
the downloaded archive (the SHA-256 came from the same server as the file), the Android
`PendingIntent`/result handling (validate the sender, make it immutable), and the `file://`
test URL of `OMSI_UPDATE_URL`.
