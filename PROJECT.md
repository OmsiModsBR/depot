# Project Depot

Project Depot is an OMSI 2-compatible runtime project maintained by OmsiModsBR.
It originated from the openOMSI codebase and is maintained as an independent
repository, not a GitHub fork.

Its development model prioritizes stability, OMSI 2 compatibility, controlled
evolution, regression prevention and selective upstream integration.
Achieving 1:1 compatibility and evolving the simulator's physics and simulation
are explicit goals, not guarantees already fulfilled.

Depot does not automatically track, merge or rebase onto openOMSI main.
Upstream changes are reviewed individually on integration branches and tested
before integration into Depot main. Changes from experimental forks are not
imported wholesale.

Depot 0.2.17 was only a rebranding of the historical openOMSI 0.2.16 baseline.
Depot 0.2.52 (current) adds the changes listed in its release notes, some
validated only by automated tests. See [provenance](UPSTREAM.md),
[baseline policy](DEPOT_BASELINE.md), [0.2.17 notes](docs/releases/0.2.17.md)
and [0.2.52 notes](docs/releases/0.2.52.md).

Releases are reviewed and published manually. CI may build and test, but must
not publish releases, tags, websites or packages automatically.
