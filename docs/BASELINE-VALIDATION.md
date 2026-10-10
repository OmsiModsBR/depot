# Baseline establishment and validation

Validation started on 2026-10-09. This record complements the historical
import plan after the authorized Depot rebranding and release publication.

## Provenance and repository

- Independent private repository: https://github.com/OmsiModsBR/depot.
- Default branch: `main`; GitHub reports `fork=false`, `private=true`.
- Original upstream baseline: `c85b2e0dda13d219fd12d0496c653d20cdb54145`,
  openOMSI `v0.2.16` (`2026-10-07`).
- Published immutable tag: `depot-baseline-openomsi-0.2.16`, pointing exactly
  to that commit. Its merge base with Depot main is the same SHA.
- Original MIT LICENSE and ancestral Git history remain intact.
- No later openOMSI commits were imported into main. Depot 0.2.17 is a
  rebranding release, not an import of openOMSI 0.2.17.
- The separate experimental multibody branch was not merged or modified.

`PROJECT.md` and `UPSTREAM.md` record the goals and provenance. Local
maintenance remotes use Depot as origin and canonical openOMSI as upstream;
upstream pushes and automatic tag following are disabled. No Depot branch
tracks upstream/main.

## Automation

All nine inherited workflows and Dependabot remain archived and inactive.
The new `.github/workflows/depot-ci.yml` only tests and builds on Windows,
with read-only repository permissions and no upstream secrets. It verifies
the immutable tag and ancestry. It cannot publish releases, tags, websites
or distribution packages. Test/build failures fail the final CI gate.

GitHub branch-protection and ruleset queries returned HTTP 403 with the
explicit requirement to upgrade to GitHub Pro or make the repository public.
Protection is therefore unavailable under the current private-repository
plan; it is not a missing-credential diagnosis. Privacy was not changed.

## Environment and commands

Windows 11 Pro x64, build 26200; Rust/Cargo 1.99.0; MSVC Visual Studio Build
Tools 2026; CMake 4.3.1-msvc1. Builds used the locked dependency graph,
release profile, `CARGO_PROFILE_RELEASE_DEBUG=0` and
`CARGO_PROFILE_RELEASE_STRIP=debuginfo`. Version stamping was explicitly
0.2.17 for Depot and 0.2.16 for the original baseline.

```text
cargo test --locked --workspace --release --target x86_64-pc-windows-msvc --no-fail-fast -j 2
cargo build --locked --workspace --release --target x86_64-pc-windows-msvc -j 2
cargo test --locked -p omsi-net --lib --release -j 2
```

Cargo nextest was not installed, so the optional nextest check was skipped.
CMake was added to the build process PATH. Build caches, temporary files and
logs were kept on D: because C: had insufficient free space.

## Results

- Depot full release workspace build: passed.
- Initial Depot full suite: 1,319 passed, 1 failed, 36 ignored.
- The sole failure was a stale test assertion expecting the old official
  server alias `OpenOMSI` after the authorized rename to `depot`.
- Commit `d28c9f192861511e9a44cbdd4f56c45c5508e996` changes only that
  assertion to mixed-case `DePoT`, preserving its trim/case-insensitivity
  coverage. No runtime implementation was changed.
- Network package retest: all 61 tests passed.
- Two additional Depot tests passed against installed Berlin-Spandau content:
  `ailists::tests::spandau_takes_line_5_off_in_1991` and
  `tests::lines_follow_the_chrono_date`. The required Chrono and TTData
  directories were present; these were not successful early-return skips.
- Original baseline workspace comparison ran all default groups: 1,319 passed,
  1 stale-executable alias failure and 36 ignored. The two affected cached
  package executables were invalidated; a separate, fresh target directory
  then passed all 23 configuration and 61 network tests with no failure.
  Combining the other baseline groups with these isolated rechecks covers
  the same 1,320 default tests, with no verified original-source failure.
  This is a composite validation record, not one successful clean-target
  whole-workspace invocation. The contaminated run's exit status was 101.
- The original baseline application test binary passed all 614 default tests
  (8 ignored) when run separately while the remaining workspace compiled.
- An additional explicitly ignored baseline test passed with installed stock
  Grundorf content: `tiles::tests::grundorf_traffic_light_parents_exclude_gaussdorf`.
  This does not change the default-suite ignored count or certify graphics.
- GitHub CI on `d28c9f192861511e9a44cbdd4f56c45c5508e996`: full workspace
  release build and tests passed; 1,320 tests passed, 0 failed, 36 ignored.
  Run: https://github.com/OmsiModsBR/depot/actions/runs/38013234790.

Warnings retained from the source include colliding `dump` example output
filenames, deprecated `Atomic::fetch_update` and unnecessary mutable test
variables. No engine refactoring was performed to silence them.

The first original-baseline compilation exhausted compiler memory with two
parallel build jobs. Retrying with one job completed compilation. A shared
target directory then reused two older Depot unit-test executables whose
timestamps predated the baseline build (network and configuration crates).
The impossible old-alias assertion failure was not attributed to the baseline:
its checked-out source still correctly declared the original alias. Those two
cached executables were moved to a recoverable artifact folder and rebuilt.
The contaminated intermediate comparison is not the final validation result.
Network and configuration rechecks passed in a fresh, separate target
directory. The repeated whole-workspace call was stopped because the
inherited build script watches `.git/HEAD` and `.git/index` below a worktree's
`.git` file and restarts application compilation; that source was not changed.
Future validation should use distinct build target directories per revision.

## Scope and limits

The scripting source tree is byte-for-byte unchanged. The simulation crate
diff is an example path and a test fallback path; its runtime physics
implementation is unchanged. The renderer diff is its fallback configuration
directory rename. Other authorized rebranding changes include UI names,
distribution filenames, configuration paths, updater destination and network
branding; the original baseline must not be described as byte-identical to
the Depot application.

Ignored tests requiring GPU/ray-query execution, live presence services,
installed content or explicit diagnostic/benchmark invocation were not
treated as passed. No full interactive map/vehicle session, rendered
frame comparison or end-to-end multiplayer session was certified. Automated
checks establish bounded evidence, not achieved OMSI 2 compatibility 1:1.
The published 0.2.17 packages and release tag were not replaced or moved.
