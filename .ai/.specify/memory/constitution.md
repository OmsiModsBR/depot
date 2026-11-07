# Project Depot Constitution

## Core Principles

### I. Own Path

Project Depot is maintained independently and follows its own path.

- Depot MUST NOT track, merge or rebase onto openOMSI, and does not send changes back to it.
  Compatibility with openOMSI is not a design constraint: moving, renaming or splitting inherited
  code is not a cost.
- The "selective integration" in `DEPOT_BASELINE.md` applies inside Depot itself: every change is
  reviewed and integrated on its own, never in bulk.
- The history of the baseline (openOMSI `v0.2.16`) is kept for attribution, and the original MIT
  licence and copyright notices stay intact.

Rationale: the project chose to evolve physics, simulation and interface without tying each change
to the codebase it came from.

### II. OMSI 2 Fidelity Only With Validation

The goal is 1:1 compatibility with OMSI 2, but fidelity is a claim that needs proof.

- A behaviour is described as matching OMSI 2 only when a black-box test demonstrates it or the
  maintainer has checked it in the game. Otherwise the text describes only what Depot does and
  points to the pending validation (issue #49).
- A change MUST NOT break a stock map or a mod that worked. Larger changes run `omsi-check` on an
  OMSI 2 installation before and after.
- A change that alters vehicle physics or game state on purpose carries the matching label
  (`physics-delta`, `offscreen-delta`) and a public justification; without the label the physics
  and state traces MUST come out identical.

Rationale: unproven "as OMSI 2 does" claims have already led to wrong rules that had to be undone;
proof is what separates compatibility from guesswork.

### III. The Game Does Not Compensate For Misconfigured Content

Depot reads content as it is declared and shows its errors.

- The game MUST NOT repair, at run time, badly made 3D animations, models or vehicle configurations
  (for example re-seating wheels the model moved, or accepting names OMSI 2 would not accept). The
  error shows as it is, and the fix belongs in the model.
- Missing values use only OMSI 2's own defaults, with no minimums or invented values. Old defaults
  kept behind a feature (such as `model-fallbacks`) are temporary and are removed once the physics
  is validated in the game.

Rationale: compensating for a broken model hides the defect from the mod author and moves Depot
away from OMSI 2's behaviour.

### IV. Invalid Content Never Crashes The Game

Content comes from third parties and is untrusted.

- Invalid, missing or malformed content MUST NOT bring the game down: the error is reported
  (log, message to the player) and the game carries on without that piece of content.
- Reporting an error is not repairing it: Principle III still applies, so the content is not
  silently corrected to make it work.
- Parsers read untrusted input; the release profile keeps `overflow-checks = true`.

Rationale: a single broken mod must not cost the player the session, and a visible error tells the
author what to fix.

### V. Domains Organised As Packages

Code is organised by domain and, where it applies, by subdomain. Each domain is a package with three
parts:

- `domain/`: the domain objects (structs, enums, states, constants and their accessors);
- `interface/`: the ports, with `input.rs` (what other domains call) and `output.rs` (what the
  domain asks of the outside);
- `implementation/`: the procedures, one module per subdomain.

Package rules:

- `implementation/` talks to another domain only through that domain's `interface/input.rs`, and
  reaches the outside only through its own `interface/output.rs`.
- `domain/` MUST NOT import `implementation/` or `interface/`.
- The rest of the crate imports the domain only through what its `mod.rs` re-exports.
- A trait in the output port only where there are two real implementations; otherwise `output.rs`
  re-exports or wraps the concrete types.
- A small domain may use `domain.rs` and `implementation.rs` instead of folders, but keeps all
  three parts.

Rationale: explicit boundaries let each domain be refactored, tested and replaced without dragging
the rest of the game along.

### VI. Tests Outside `src/`

- Tests live in each crate's `tests/` folder, NEVER in `src/`. What a test needs from inside the
  crate comes through its `testing` module behind the `test-support` feature. A crate that still has
  tests in `src/` moves them out when it is refactored.
- Every fix comes with a test that fails without it, whenever the defect can be reproduced outside
  the game.
- Tests that need OMSI 2 content read it from a local installation (`OMSI_ROOT`) and skip
  themselves without one; test data in the repository is small and made for the test.
- `cargo test --workspace` and `cargo build --release` pass on every pull request.

Rationale: keeping tests out of `src/` keeps production code clean and makes each crate expose an
explicit test surface; a failing test is the proof that a fix fixes something.

### VII. No Performance Regression Without Justification

- A change MUST NOT make the game slower than it was (frame rate, frame time, loading time, memory).
- Exception: a new feature that widens what runs each frame or at load may cost performance, but
  only with a plausible justification in the pull request, in-depth validation and tests, and
  measurements before and after on the same machine and scenario.
- A refactor or a fix gets no such exception.

Rationale: players notice lost frames before they notice new code; a cost is acceptable only when
it buys something and is measured.

### VIII. Provenance And Public Text

- NOTHING from an OMSI 2 installation enters the repository: textures, models, sounds, maps,
  scripts.
- Public text (code, comments, commits, pull requests, issues, `CHANGELOG.md` and documentation)
  MUST NOT carry addresses, internal API names or thresholds derived from the reference executable.
  Static-analysis findings may guide the work, but public text describes only the observed
  behaviour and the test that demonstrates it.
- Commits and pull requests MUST NOT credit AI agents as authors and MUST NOT link to private
  assistant projects or sessions.
- Third-party files keep their own licence; a new licence needs the maintainer's agreement before
  it comes in.

Rationale: the repository is public and MIT; what goes into it must be publishable and correctly
attributed.

### IX. Small, Traceable Changes

- A pull request makes one change, with a description of what the player notices.
- Every branch uses one of three prefixes, chosen by what it contains:
  - `feat/<name>`: new features or refactors (may hold code, configuration and documentation);
  - `fix/<name>`: only fixes for problems in existing features (and their documentation);
  - `docs/<name>`: only documentation, no code or configuration.
  A branch that outgrows its prefix is renamed, or the extra work moves to a branch of its own.
- Refactoring yields to open fixes: refactor pull requests are short, whoever moves code ports the
  affected open branches, and formatting lands crate by crate.
- A pull request that changes what the player notices adds a line to **Unreleased** in
  `CHANGELOG.md`.

Rationale: short single-topic pull requests are reviewable, revertible and coexist with several
contributors working in parallel.

## Project Constraints

- **Platform**: Windows x64 is the only supported platform; CI builds and tests on it.
- **Language and style**: Rust; `rustfmt` at 100 columns; comments explain why. `rustfmt` blocks on
  the crates already formatted; `clippy`, `cargo audit` and `cargo deny` are informational until the
  backlog in `docs/QUALITY.md` is cleared.
- **Dependencies**: `Cargo.lock` stays pinned; dependency upgrades are separate work.
- **Files**: no file of 1 MB or more, nothing generated or downloaded (builds, caches, converted
  textures, example maps); pictures only for the documentation, under `docs/images/`.
- **Unused code**: code that became unreachable is NOT deleted by default; it leaves the build
  (feature or `cfg`) and stays in the tree to be readapted. Deleting it needs an explicit decision
  by the maintainer.
- **Releases**: made by hand by the maintainers, numbered `MAJOR.MINOR.PATCH`
  (`docs/VERSIONING.md`). CI may build and test, but MUST NOT publish releases, tags, websites or
  packages.

## Development Workflow

- Operational detail (branches, commits, formatting, tests, translations, issues) is in
  `CONTRIBUTING.md`; instructions for AI agents are in `AGENTS.md` and the skills in `.ai/skills/`.
- Features specified with Spec Kit live in `.ai/specs/`. `/speckit-plan` checks every plan against
  this constitution (Constitution Check) before the design and again after it; a deviation goes in
  only when recorded in Complexity Tracking with its justification and the rejected alternative.
- Issues are written in English. Security problems follow `SECURITY.md`, not issues.
- Every pull request goes through CI (build, tests, physics and state traces) and a maintainer's
  review before it is merged.

## Governance

- This constitution supersedes other project practices. When a document or plan contradicts it,
  the constitution holds until it is amended.
- Amendments are made by a `docs/` pull request that changes this file, approved by the maintainer.
  The pull request carries the impact report: old and new version, changed principles, and what
  else has to follow (templates, `CONTRIBUTING.md`, `AGENTS.md`).
- Semantic versioning of the constitution:
  - MAJOR: incompatible removal or redefinition of a principle or governance rule;
  - MINOR: a new principle or section, or materially expanded guidance;
  - PATCH: clarification, wording or a fix with no change of meaning.
- Compliance review: every pull request and review checks the principles above; complexity that
  goes against them has to be justified in the pull request.

**Version**: 1.0.0 | **Ratified**: 2026-11-06 | **Last Amended**: 2026-11-06
