# Code quality checks

The `quality` job in `.github/workflows/depot-ci.yml` runs `cargo fmt --check`, `cargo clippy`,
`cargo audit` and `cargo deny check` (policy in `deny.toml`) on every push and pull request.
They are **informational**: each step is `continue-on-error`, so they never fail the workflow
that guards `main` (build and tests). Do not add `-D warnings` until the backlog below is cleared.

The release profile also builds with `overflow-checks = true` (integer overflow panics instead
of wrapping), because the parsers read untrusted content.

## Status when this was introduced (toolchain 1.99.0, Linux)

| Check | Result | What is pending |
| --- | --- | --- |
| `cargo build --locked --workspace` | passes | - |
| `cargo test --locked --workspace` | passes (1317 passed, 0 failed, 35 ignored) | - |
| `cargo fmt --all -- --check` | fails: about 8000 diff hunks; the baseline was never formatted | Run `cargo fmt --all` in one dedicated, mechanical commit, then make the step blocking. |
| `cargo clippy --workspace --all-targets` | passes with about 550 warnings | The most frequent: `chunks_exact` with constant size, very complex types, `is_multiple_of`, needless borrows/clones, too many arguments. Fix per crate, then use `-D warnings`. |
| `cargo audit` | 2 warnings (unmaintained, no known vulnerability): `paste` (RUSTSEC-2024-0436), `ttf-parser` (RUSTSEC-2026-0192) | Find the dependency that pulls them in and replace or accept the risk in writing. |
| `cargo deny check` | advisories: same two crates; licenses: `omsi-net` has no `license` field, and `webpki-roots` is CDLA-Permissive-2.0 (allowed in `deny.toml` pending maintainer confirmation) | Add `license` to the `omsi-net` manifest; confirm the CDLA decision. Sources and bans pass. |

When a row is cleared, switch its CI step from informational to blocking and update this table.

## Offscreen golden

`crates/omsi-app/tests/offscreen_golden.rs` renders a synthetic root (written by the test, no OMSI 2 content)
with the real `depot --offscreen`. The picture checks need a graphics adapter (a software one is enough) and print
`SKIP` and pass without one; the root read-back and the window/offscreen `signal_aspects` check always run. The
thumbnail comparison runs only when `tests/golden/offscreen_synthetic.txt` exists and was recorded on the same
adapter; regenerate it with `OMSI_GOLDEN_BLESS=1 cargo test -p omsi-app --test offscreen_golden -- --nocapture`.
