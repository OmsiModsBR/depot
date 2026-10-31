# Security

## Reporting a vulnerability

Please do not open a public issue for a security problem. Report it privately through
GitHub instead: **[Report a vulnerability](https://github.com/OmsiModsBR/depot/security/advisories/new)**
(the *Security* tab of the repository). Only the maintainers see it. Project site: <https://omsimods.com.br>.

Say what is affected, how to reproduce it, and what an attacker could do with it. A small
example (a file, a request, a log) helps a lot. The maintainers are volunteers: we aim to
answer within a week, keep you posted while it is fixed, and credit you in the advisory if
you want.

## Supported versions

Only the latest release. The game does not update itself (download new releases from the
Releases page), and fixes are not backported to older builds.

## What counts

Examples of what we want to hear about:

* **content files**: a map, vehicle, script or archive that makes the game read or write
  files outside the game's folders, or run code, just by being loaded;
* the **launcher's downloads** (the OmsiLaunch and module installers of the hidden original OMSI 2 mode): anything that
  could make it fetch or run something it should not;
* the crash reporter or the logs sending or showing more than they say (passwords, tokens,
  personal paths beyond the user name).

Not a vulnerability: a content file that only makes the game
crash, which is a normal bug and goes into an [issue](https://github.com/OmsiModsBR/depot/issues/new/choose).
