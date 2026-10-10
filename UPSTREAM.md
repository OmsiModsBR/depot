# Upstream provenance

Project Depot was established from the following openOMSI revision:

- Repository: https://github.com/openOMSI-org/openOMSI
- Version: `0.2.16`
- Date: `2026-10-07`
- Commit: `c85b2e0dda13d219fd12d0496c653d20cdb54145`
- Immutable Depot baseline tag: `depot-baseline-openomsi-0.2.16`

The tag points exactly to the original upstream commit, not a Depot
administrative or release commit. It must never be moved.
The original MIT license, copyright notices, attribution, authorship and
ancestral Git history are preserved. Depot does not claim authorship of
upstream code.

The upstream repository is tracked for research and selective integration only.
Project Depot does not automatically merge or rebase onto upstream `main`.
Future upstream changes must be evaluated individually before integration.

Maintenance remotes:

- `origin`: https://github.com/OmsiModsBR/depot.git
- `upstream`: https://github.com/openOMSI-org/openOMSI.git (consultation only)

The maintenance environment disables pushing to `upstream`, disables automatic
tag following for that remote, and restricts its branch fetch mapping to main.
No Depot branch tracks upstream/main. Remote configuration is local Git metadata,
not a configuration imposed on users cloning the repository.

Verification:

```sh
git rev-parse depot-baseline-openomsi-0.2.16
git merge-base depot-baseline-openomsi-0.2.16 main
```

Both commands must return `c85b2e0dda13d219fd12d0496c653d20cdb54145`.
The Depot release numbers (0.2.17, 0.2.52) are Depot's own and do not mean openOMSI 0.2.17 or 0.2.52 was imported.
