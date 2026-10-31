# Versioning and releases

## Project Depot release policy

Depot follows **stable base + selective upstream integration**. Releases are reviewed and
published by hand; a push to `main` publishes nothing. Inherited release workflows are archived in
`docs/upstream-automation/`, not active under `.github/workflows/`. No automatic upstream
synchronization is enabled.

**Depot 0.3.0 is the current release** (2026-10-30, tag `v0.3.0`). See its
[release notes](releases/0.3.0.md).

## The version number: `MAJOR.MINOR.PATCH`

Since 0.3.0 the number is chosen by hand, in the usual three parts:

| Part | Goes up when | Example |
| --- | --- | --- |
| `MAJOR` | something changes so much that a new line of releases is warranted (decided by the maintainers; still `0` while 1:1 compatibility is a goal, not a result) | `0` |
| `MINOR` | a release **adds features or removes some** | `0.3` → `0.4` |
| `PATCH` | a release **only fixes the previous one** | `0.3.0` → `0.3.1` |

A release number is Depot's own count. It does not mean that the openOMSI release of the same number
was imported: the historical baseline is openOMSI `v0.2.16` and no later openOMSI commit is imported
wholesale (see [UPSTREAM.md](../UPSTREAM.md)).

Where the number lives:

- The [`VERSION`](../VERSION) file holds `MAJOR.MINOR` of the next release (for example `0.3`).
- A development build gets a third part from the number of commits on `main` since `VERSION` last
  changed (`scripts/version.sh`, and `crates/omsi-app/build.rs` for the program). That count only
  names development builds; it is **not** the `PATCH` of a release.
- A release build passes the full version explicitly: `OPENOMSI_VERSION=0.3.0`. The build script's
  fallback must not be relied on for release stamping (it once returned `0.2.0`).
- The program shows its version in `depot --version`, the launcher, the log and crash reports.

## How a release is made

1. **Decide the number** (`MINOR` for new features or removals, `PATCH` for fixes only) and set
   `VERSION` in a pull request of its own when the `MAJOR.MINOR` changes.
2. **Write it down** in one documentation pull request (branch `docs/...`): the `## X.Y.Z - date`
   heading in [CHANGELOG.md](../CHANGELOG.md) over what was under *Unreleased*, the release notes
   `docs/releases/X.Y.Z.md` (what changed, known limitations, build and validation, rollback, credits and
   licences), and the README, `PROJECT.md`, `UPSTREAM.md`, `DEPOT_BASELINE.md` and site pages that name the
   current release.
3. **Review and merge** by the project's rule: an independent review of the exact head and green CI on
   that head.
4. **Build the package** from the merge commit, on Windows x64 with the locked dependencies, with
   `OPENOMSI_VERSION=X.Y.Z`, and check `depot --version` before packing. Name it
   `depot-X.Y.Z-windows-x64.zip`.
5. **Record the build** in the release notes: commit, toolchain, target, `depot --version` and the
   SHA256 of the zip.
6. **Tag and publish by hand**: tag `vX.Y.Z` on the merge commit and a GitHub release whose text is the release
   notes. Links in that text must be absolute (see below).

Only the Windows x64 game and launcher package is published. No ARM64, Linux, macOS or Android package,
and no dedicated server, is part of any Depot release; the hardware matrix is a product decision and does
not grow because the code could compile elsewhere.

### Links in release notes

The same text is read in the repository, on the GitHub release page and on the website. A relative link
(`../../CHANGELOG.md`) works only in the first, so release notes link with absolute addresses fixed to
the release tag, for example
`https://github.com/OmsiModsBR/depot/blob/v0.3.0/CHANGELOG.md`.

## Release history

| Release | Date | What it was |
| --- | --- | --- |
| [0.3.0](releases/0.3.0.md) | 2026-10-30 | First release numbered by hand: DirectX 9 renderer, new interface and launcher, integrity checks, physics and passenger fixes, multiplayer and plugin code removed |
| [0.2.52](releases/0.2.52.md) | 2026-10-10 | First release with behaviour changes since the rebranding (articulated buses as separate bodies, vehicle physics refactor, navigator map packages, pusher joint, fixes) |
| [0.2.17](releases/0.2.17.md) | 2026-10-09 | Rebranding only, from the openOMSI 0.2.16 baseline; no behaviour changes |

For a reviewed build, check `depot --version` before packaging; the first release used
`OPENOMSI_VERSION=0.2.17`.

## The website

[`.github/workflows/pages.yml`](../.github/workflows/pages.yml) publishes `site/` together with the
Markdown files of `docs/` to GitHub Pages (temporary address https://omsimodsbr.github.io/depot/; the
official site is planned for https://omsimods.com.br) whenever they change on `main`. The site renders
the Markdown in the browser, so a documentation change is one edit in `docs/`.

## Historical: the inherited scheme (not used by Depot)

openOMSI numbered every push to `main` as `MAJOR.MINOR.COMMIT` and published a release for each, with
builds for several systems and processors, test builds for each pull request and a version badge. None of
that is active in Depot: the workflows are archived in `docs/upstream-automation/` and the history is frozen
in [CHANGELOG_UPSTREAM.md](../CHANGELOG_UPSTREAM.md).
