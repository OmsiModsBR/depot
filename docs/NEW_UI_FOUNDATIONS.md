# New UI: foundations (design draft)

Status: design document. Tracks UI-A of #166. This document changes no behavior. What the draft pull requests already implement is listed under "Implementation status" below; everything else here is still a plan.

The new UI consumes the settings model of #151. A new screen is not evidence of parity, and no existing function, shortcut or file compatibility may disappear because of it.

## Where we are

- The launcher is a window of the game binary (`omsi-app::launcher`), drawn with `omsi-ui` (wgpu, no WebView) in the same process as the game renderer. `crates/omsi-launcher-core` holds the launcher logic that does not need a window and also ships a command-line launcher (`depot-launcher --cli`), which must keep working.
- `omsi-ui` already provides text, icons, an atlas and a painter (rounded boxes, shadows, lines). The launcher look lives in `omsi-app/src/launcher/theme.rs` as plain constants.

## Goals

1. A launcher that is a separate program: no world, scripts, simulation audio or 3D pipeline in the launcher process. (It may still use the GPU to draw its own interface.)
2. A small design system (tokens and components) shared by the launcher and the in-game menus.
3. Bus previews as cached images, produced by a short-lived helper process; a live 3D preview only on demand.
4. Reversible migration: the current launcher stays as the fallback behind a feature gate until the new one passes regression.

Non-goals: WebView, C++, a new toolkit, multiplayer, plug-ins, renaming the project, new releases.

## 1. Design tokens and components

Move the constants of `theme.rs` into `omsi-ui` as named tokens (colors, radii, spacing, row height, type scale, elevation) with a dark theme first. Reusable components, each with the states normal, hover, focus, pressed, disabled (with a reason), loading, empty, error:

- sidebar navigation (collapsible), tabs, buttons, switches, selects, sliders, search field
- virtualized list and grid (large mod collections must scroll without building every row)
- card, badge, tooltip, modal dialog, toast, progress bar (determinate and indeterminate)
- table

Rules: keyboard-only operation with a visible focus ring and a defined tab order; layout works from 1280x720 up, DPI scale 100-200%, windowed and fullscreen; contrast checked per token pair. Do not copy mockup pixels; mockups are references for hierarchy, not specs.

Layer priority (highest first), to be tested: modal dialog, pause menu, shortcut bar, tooltip, map, editor, HUD, cabin controls. Esc closes the topmost closable layer only.

## 2. Launcher and game as separate processes

Components:

- `launcher` process: content catalog, settings store, profiles, mods, sessions, diagnostics, preview cache. Reuses `omsi-launcher-core`; must not depend on the game's `App` or scene.
- `game` process: the runtime, started by the launcher with explicit arguments (as today).
- `preview helper`: the game binary in a preview mode (proposed flag `--preview`, not yet existing) that renders one bus and paint to an image and exits.

### Launcher/game contract (proposal)

Versioned and explicit, with no hidden coupling:

- Handshake fields: `protocol_version`, `launcher_version`, `runtime_version`, settings schema version, catalog schema version. Mismatch gives a visible, actionable message, never silent behavior.
- Events from launcher to game or helper: `session.start_requested`, `preview.request`, `settings.changed`.
- Events back: `session.started`, `session.failed`, `session.ended`, `preview.ready`, `preview.error`, `content.scan_progress`, `load.stage_progress`.
- Transport: to be decided in the spike (the current CLI arguments plus a local pipe or socket are candidates). It must work on every platform where Depot builds, and must fail closed when the other side is missing or too old.
- Failure cases to design for: helper cannot start, crashes, times out or is cancelled; launcher minimized; several launcher instances; corrupt cache.

### Preview helper

- Input: bus and paint identifiers plus a content fingerprint. Output: an image plus metadata (and a sanitized log on failure).
- Limits: time out, RAM/VRAM budget, maximum resolution, bounded queue, cancellation.
- Cache key: relative path, size, modification time or hash (to be chosen), dependent paint/texture/script files, shader and renderer version, graphics system. Invalidate on any change; never serve a stale image for changed content.
- Live 3D preview: only while the user keeps that screen visible; the helper exits on leaving the screen, on idle time out, or when the launcher exits.
- Fallback: a generic thumbnail. A failed preview must never block the launcher.

## 3. Content status labels

The UI may only show what was actually checked. Each check is separate and has a source and a timestamp:

| State | Meaning |
| --- | --- |
| found | the file or entry exists |
| missing | expected and not there |
| unverified | not checked yet |
| warning | parsed with a recoverable problem (file:line when known) |
| error | failed to parse or load |

A file being present is not the same as parsing without errors, and "not found" is never turned into "incompatible" without evidence. Words such as "compatible" or "scripts OK" appear only when a check really produced them. Mockup numbers are placeholders.

## 4. Hardening

All third-party data (vehicle, map, timetable, description, image files) is untrusted: bounded sizes and time, bounded image dimensions, path traversal checks, legacy encodings with a safe fallback, descriptions rendered as plain text only. A map without enriched cartography must still be usable.

## 5. Migration plan

1. Design system and components in `omsi-ui`, demonstrated by an isolated prototype page; old launcher untouched.
2. Catalog and settings pages on the shared settings registry (OPT-01), still in the current launcher.
3. Contract and preview helper behind a feature gate.
4. Separate launcher binary, old launcher kept as fallback.
5. Make it the default only after regression passes; remove the fallback only on explicit approval.

Each step is its own small pull request with before/after screenshots, a navigation test and a resource comparison (idle launcher RAM/VRAM, startup time, preview cache hit/miss time).

## 6. Regression checklist for every UI pull request

- keyboard shortcuts: Esc, Alt+Enter, Alt+S, P, Shift+M/N/Y, Insert, F-keys, Alt+I/J/K/L, Ctrl combinations (Alt alone must not swallow combinations)
- settings still persist and apply with the same timing
- mods drag and drop, in-place ZIP install, timetable copy editor, parallel sessions, driver profiles, original-folder setup, `--cli`
- resolution matrix: 1280x720, 1366x768, 1920x1080, ultrawide, DPI 100/150/200%
- broken helper, missing preview, corrupt cache: launcher stays usable

## Implementation status

Built as stacked draft pull requests, merged in order only when the maintainer confirms:

| Part of this document | State |
| --- | --- |
| Keyboard focus in the widget kit (Tab order, Enter or Space, a visible ring), off by default (#321) | draft; arrow keys on sliders and lists, and Tab into a text field, are not done |
| Separate launcher program, sidebar only (#252) | draft; the launcher inside the game binary stays the fallback |
| Vehicles and Maps pages on the real catalog, with the status labels of section 3 (#253, #254) | draft; scripts, textures and sounds are shown as "not checked yet", never "found" |
| Loading screen with the player's screenshots as a slow slideshow (#255) | draft |
| Pause menu column with the city-map bar (#256) | draft; the caller supplies the items and the map |
| Alt icon bar and the in-game windows' building blocks (#257) | draft; the Alt bar never pauses the simulation |
| Offscreen example drawing every screen with placeholder data (#250) | draft |

Not started: design tokens moved into `omsi-ui` (the colors still live next to each screen), virtualized lists and grids, tabs, modal dialogs and toasts, the launcher/game contract, the preview helper and its cache, and the layer-priority tests. The screens are not wired into the game's own menus yet.

## Open decisions (maintainer)

- final launcher/game transport (the toolkit stays `omsi-ui`)
- cache key strategy
- diagnostic states and criteria
- decided: the loading screen's background is the player's own screenshots of that map, as a slideshow, and the default background when there are none; the Alt bar never pauses the simulation

## References

Parent issues: #166 (new UI), #151 (settings parity).
