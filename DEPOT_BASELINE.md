# Project Depot — historical baseline

Project Depot is maintained independently at https://github.com/OmsiModsBR/depot.
It began as a project derived from openOMSI, with the explicit goal of achieving
1:1 compatibility with OMSI 2 and prioritizing evolutions to the simulator's
physics and simulation. Full compatibility is a development target, not an
already-verified guarantee.
Its initial policy is **stable base + selective upstream integration**.

## Canonical source

- Repository: https://github.com/openOMSI-org/openOMSI
- Release/tag: `v0.2.16`
- Version: `0.2.16`
- Historical release date: `2026-10-07`
- Commit: `c85b2e0dda13d219fd12d0496c653d20cdb54145`
- Immutable Depot baseline tag: `depot-baseline-openomsi-0.2.16`

The Git history through this commit is retained for attribution and comparison.
The original MIT license and copyright notices remain intact. GitHub fork
association is not required to preserve Git ancestry.

## Independence

`origin` is the Depot repository. No upstream branch is tracked automatically.
The application updater targets Depot releases. Executables and packages use
the `depot` name; user-facing text uses Project Depot. Internal identifiers and
source filenames remain unchanged.
Inherited GitHub workflows and Dependabot configuration are archived in
`docs/upstream-automation/`; enabling Depot automation requires a separate review.
The README is adapted to describe Depot's origin, goals and distribution names.
See [PROJECT.md](PROJECT.md) and [UPSTREAM.md](UPSTREAM.md).
A Depot-owned CI workflow builds and tests with read-only repository permissions;
it does not publish releases, tags, websites or packages and uses no upstream secrets.
See [baseline validation and limitations](docs/BASELINE-VALIDATION.md).

## Selective integration

Each upstream change must be reviewed individually, with its original SHA,
purpose, dependencies and validation recorded in the integration PR.
Do not merge upstream main wholesale or enable automatic upstream synchronization.
Prefer small cherry-picks with source attribution (`git cherry-pick -x`).
Review merge commits by their individual changes rather than importing their
entire ancestry. Keep `Cargo.lock` pinned; dependency upgrades are separate work.

Changes from the previous user fork are not part of this import. Reapply them individually after comparing
their patches with this exact baseline and testing their behavior.

## Scope of this import

No gameplay, renderer, physics, scripting or multiplayer feature is added.
The baseline adaptation only records provenance, documents the integration
policy, redirects the updater and archives inherited automation.

## Depot 0.2.17 — rebranding only

The first Depot release is simply a rebranding of the imported openOMSI 0.2.16
baseline. It changes visible branding, executable/package/data-folder names,
repository/update configuration and documentation. No gameplay, renderer,
physics, simulation, scripting or multiplayer feature or fix is introduced.
The MIT license and internal source identifiers are preserved.

Physics/simulation evolutions and 1:1 compatibility are future goals, not claims
about improvements in 0.2.17. Only Windows x64 game/launcher and dedicated-server
packages are published. See [release notes](docs/releases/0.2.17.md).
