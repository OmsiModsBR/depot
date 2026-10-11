# Versioning and releases

## Project Depot release policy

Depot follows **stable base + selective upstream integration**. Releases are
reviewed and published manually; a push to `main` does not publish anything.
Inherited workflows are archived in `docs/upstream-automation/`, not active
under `.github/workflows/`. No automatic upstream synchronization is enabled.

**Depot 0.2.52 is the current release** (2026-10-10, tag `v0.2.52`), the first with
behaviour changes since the rebranding. Depot release numbers are Depot's own count and do not
mean the same openOMSI version was imported. See [release notes](releases/0.2.52.md).

**Depot 0.2.17 is a rebranding-only release of the openOMSI 0.2.16 baseline.**
The version increment does not indicate new physics, simulation, renderer,
scripting, gameplay or multiplayer functionality. See [release notes](releases/0.2.17.md).
The only published packages for this release are Windows x64 game/launcher and
dedicated server, named `depot-0.2.17-windows-x64.zip` and
`depot-0.2.17-server-windows-x64.zip`.

For a reviewed build, pass `OPENOMSI_VERSION` explicitly and check
`depot --version` before packaging. The first release used
`OPENOMSI_VERSION=0.2.17`; the inherited build-script fallback returned `0.2.0`
when determining version history from the crate directory, so it must not be
relied on for release stamping.

## Historical upstream policy — not active in Depot

The remaining text records the inherited openOMSI versioning and automation.
Descriptions of automatic releases, PR builds, badges and website publication
below are historical, not current Depot behavior. Refer to the archived
workflows rather than the inactive `.github/workflows/` paths mentioned below.

## The version number: `MAJOR.MINOR.COMMIT`

| Part | Where it comes from | Example |
| --- | --- | --- |
| `MAJOR` | the first number in the [`VERSION`](../VERSION) file, changed by hand | `1` |
| `MINOR` | the second number in `VERSION`, changed by hand | `1.2` |
| `COMMIT` | counted automatically: commits on `main` since `VERSION` last changed (not counting those marked `[skip ci]` or `[skip actions]`, which are never released) | `1.2.17` |

`COMMIT` restarts from `0` whenever `VERSION` changes: after `1.1.23`, a commit that sets
`VERSION` to `1.2` is released as `1.2.0`, and the next one as `1.2.1`.

To start a new version, edit one line and push:

```sh
echo 1.2 > VERSION
git commit -am "Version 1.2" && git push
```

`scripts/version.sh` prints the version of the checked-out commit; the game shows it in the
launcher's side bar, in its log and in `openomsi --version` (baked in by
`crates/omsi-app/build.rs`, which uses the same rule).

## Releases

Every push to `main` runs [`.github/workflows/release.yml`](../.github/workflows/release.yml):

1. works out the version with `scripts/version.sh`;
2. builds Windows x64 and ARM64 (MSVC), macOS for Apple silicon and Intel, Linux x64 and
   ARM64 and Android in parallel, and packs the dedicated server from the Linux and Windows
   builds;
3. creates the release `v<version>` (tag on that commit) with the archives
   `openOMSI-<version>-windows-x64.zip`, `-windows-arm64.zip`, `-macos-arm64.zip`,
   `-macos-x64.zip`, `-linux-x64.zip`, `-linux-arm64.zip`, `-android-arm64.apk`,
   `-server-linux-x64.zip`, `-server-linux-arm64.zip`, `-server-windows-x64.zip`,
   `-server-windows-arm64.zip`, and release notes generated from the commits.

Pull requests run the same builds without publishing anything. Build output never goes
into the repository (`target/` and `dist/` are ignored).

### Test builds of a pull request

A pull request's builds are test builds for anyone who wants to try the change:

* their version names the pull request, `<version>-pr<number>` (e.g. `0.1.1313-pr1192`),
  in the launcher's side bar, `game.log` and crash reports;
* nothing in the program replaces them (in-app updating is disabled), so a test build stays until it is deleted;
* the Android one is an app of its own, *openOMSI PR #N* (`org.openomsi.game.pr`), that
  installs beside the release and shares its `openOMSI` folder;
* [`.github/workflows/pr_builds.yml`](../.github/workflows/pr_builds.yml) comments on the
  pull request with a download link per platform (through [nightly.link](https://nightly.link),
  no GitHub account needed) once they are built, and updates that comment on every push.
  The files are kept for 14 days.

The version badge at the top of the README always shows the newest release.

## The website

[`.github/workflows/pages.yml`](../.github/workflows/pages.yml) publishes `site/` together with
the Markdown files of `docs/` to GitHub Pages
(temporary address https://omsimodsbr.github.io/depot/; the official site is planned for https://omsimods.com.br) whenever they change on `main`. The site renders
the Markdown in the browser, so a documentation change is one edit in `docs/`.
