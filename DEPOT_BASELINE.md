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

## Selective integration

Each upstream change must be reviewed individually, with its original SHA,
purpose, dependencies and validation recorded in the integration PR.
Do not merge upstream main wholesale or enable automatic upstream synchronization.
Prefer small cherry-picks with source attribution (`git cherry-pick -x`).
Review merge commits by their individual changes rather than importing their
entire ancestry. Keep `Cargo.lock` pinned; dependency upgrades are separate work.

Changes from the previous user fork, including Zanin's and other contributors'
PRs, are not part of this import. Reapply them individually after comparing
their patches with this exact baseline and testing their behavior.

## Scope of this import

No gameplay, renderer, physics, scripting or multiplayer feature is added.
The baseline adaptation only records provenance, documents the integration
policy, redirects the updater and archives inherited automation.
