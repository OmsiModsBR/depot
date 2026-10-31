# Contributing to Project Depot

Thanks for helping! A few rules keep the project healthy:

* **No original code or assets.** Never copy anything from an OMSI 2 installation into the
  repository - no textures, models, sounds, maps, scripts. Tests that need
  content read it from a local installation (`OMSI_ROOT`) and skip themselves without one.
* **Compatibility first.** A change must not break a stock map or a mod that worked. Run
  `cargo run --release -p omsi-check -- "/path/to/OMSI 2"` before and after larger changes.
* **One change per pull request**, with a message that says what the player notices.
* `cargo test --workspace` and `cargo build --release` must pass (CI builds and tests Windows x64, the only supported platform).
* Code style: `rustfmt` defaults, comments explain *why*.
* Tests live in each crate's `tests/` folder, not in `src/`; what a test needs from inside the
  crate comes through its `testing` module behind the `test-support` feature (see
  [docs/OMSI_SIM_REFACTOR.md](docs/OMSI_SIM_REFACTOR.md)). Crates that still have tests in `src/`
  move them out as they are refactored.

## Branches

Every branch follows one of three prefixes, chosen by what it contains. Names after the
prefix are short, lowercase and hyphenated (`feat/route-editor`, `fix/door-sound-loop`).

| Prefix | Use it for | May contain |
|---|---|---|
| `feat/<feature-name>` | New features, or refactors of existing ones | Code, configuration, documentation and anything else the feature needs |
| `fix/<branch-name>` | Only fixes for problems found in existing features | The fix and documentation; no new features or refactors |
| `docs/<branch-name>` | Only documentation updates | Documentation; no code or configuration |

A branch whose changes outgrow its prefix (a docs branch that needs a code change, a fix
that turns into a refactor) is renamed, or the extra work moves to a branch of its own.

Refactoring moves a lot of code around. These git settings make merges follow moved files
and remember conflict resolutions you already made:

```sh
git config merge.conflictStyle zdiff3
git config rerere.enabled true
git config diff.renameLimit 20000
```

When `main` gets a commit that only runs `cargo fmt` on a crate your branch changes, bring
the branch over it with `scripts/fmt-merge.sh <fmt-commit> <crate>...` instead of a plain
merge: it formats your branch the same way and records the format commit as merged.

## Files and licences

* **Contributions are MIT**, like the rest of the code and documentation. The few
  third-party files the game ships (the Roboto fonts, the Material icons with their
  `LICENSE.txt`) keep their own licences; adding another - map data from OpenStreetMap
  (ODbL), CC-BY-SA pictures, other fonts or icon sets - needs the maintainers' agreement
  first. A format can be documented here and its real data published elsewhere (a release,
  a repository of its own, the mod itself).
* **No large files.** A file of a megabyte or more fails the PR checks. Screenshots and
  videos belong in the pull request's description, not in a commit; test data is small and
  made for the test.
* **No generated or downloaded content**: no builds, caches, converted textures or example
  maps.

## Translations

The interface texts are in [`crates/omsi-app/locales/app.yml`](crates/omsi-app/locales/app.yml),
with the English text as the key. A pull request that changes it is checked automatically:

* add or correct texts of your language; keep the placeholders (`%{packs}`, `{}`) of the key;
* do not remove a key or reword the English one - the code looks texts up by it;
* changing another contributor's existing translation is fine when it is a correction - say
  so in the description, the check lists every changed text for the reviewers.

## Issues

* **English only** - titles and text, so every contributor can read and search them. An
  issue in another language is closed automatically with a request to translate it, and
  opens again by itself once it is edited into English. Logs and game text can stay as
  they are.
* One problem or idea per issue, on the latest release. Questions go to the
  [Discord server](https://omsimods.com.br/discord).
* New issues are sorted automatically: the form's "What is it about?" becomes an `area:`
  label and the issue gets a first milestone, which the
  maintainers set and move when it fits better elsewhere. A crash report with the same panic as an
  open issue is closed as its duplicate - add what you were doing to that issue instead.
* **Security problems are not reported as issues**: see [SECURITY.md](SECURITY.md).

## Where things are

See the layout in the [README](README.md#repository-layout) and
[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md). File formats: [docs/FORMATS.md](docs/FORMATS.md).
The renderer's domains and the layers they may import from are in
[crates/omsi-render/ARCHITECTURE.md](crates/omsi-render/ARCHITECTURE.md).

## Releases

Releases are made by hand by the maintainers; nothing is published by a push to `main`. The number is
`MAJOR.MINOR.PATCH`: `MINOR` goes up for a release with new features or removals, `PATCH` for a release that
only fixes the previous one. A contributor does not change `VERSION`, the release notes or the changelog
heading; a pull request that changes what the player notices adds a line to **Unreleased** in
[CHANGELOG.md](CHANGELOG.md) (in the area it belongs to, ending with its `(#number)`). The steps of a release are
in [docs/VERSIONING.md](docs/VERSIONING.md).
