# Outside FOV / ultrawide — experimental integration

Branch: `feat/outside-fov-ultrawide`, based on Depot main
`b4d50a878156ab436b375b07a0c58d0ef8461439`.

Source branch: https://github.com/lmonteirotech/openOMSI/tree/feat/outside-fov-ultrawide.
Observed source tip: `fbbda40cdebd7a4fe97978a2ed5dcd97e5a1e73a`.
Selected feature commit: `ea47e092ee6e0a2c472d2c7261ea7a7913e6ca9f`,
authored by zane_thruston with its original co-author trailer retained.

The source branch had 195 commits beyond the historical baseline, including
unrelated main merges and the pusher feature. None of that ancestry was
merged. Only the outside-camera feature was cherry-picked with provenance.
The later desktop-defaults formatting commit was not needed.

The camera hunk was adapted from the newer split `app_events/frame/view_camera.rs`
to the baseline's existing `app_events.rs`. Existing desktop defaults were
preserved and only `outside_fov: 0.0` added. No architecture refactor was imported.
Launcher parsing rejects nonfinite values consistently with game settings.

`outside_fov` is an independent vertical FOV override for F3: 20–120 degrees,
or 0 / Auto. Auto uses a configured general FOV first; otherwise it keeps 60
degrees on 16:9 and narrower screens and preserves the 16:9 horizontal extent
on wider windows. Window aspect is read every frame; zoom remains applied after
the selected FOV. The game options and launcher Camera tab expose this setting.

Regression checks cover 16:9, 4:3, 21:9, 32:9, invalid aspects, settings
round trips, invalid values and launcher persistence. Validation results will
be recorded after execution. Visual/gameplay behavior on a physical ultrawide
monitor is not certified by unit tests.

This branch is not merged into main and is not part of released Depot 0.2.17.
MIT, internal identifiers, updater destination and Depot branding are retained.
