#!/bin/sh
# Start the Project Depot dedicated server: start.sh /path/to/OMSI2 [server.cfg]
# (the OMSI 2 folder: a complete original installation, as the players have it; mods
# go into this folder's Vehicles, maps, Sceneryobjects … as in the game's content folder)
set -eu
# The dedicated server is disabled (docs/removed/MULTIPLAYER_DISABLED.md).
echo "The dedicated server is disabled." >&2
exit 1
here="$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)"
root="${1:?usage: start.sh /path/to/OMSI2 [server.cfg]}"
cfg="${2:-$here/server.cfg}"
exec "$here/depot" --root "$root" --server "$cfg" 2>&1 | tee -a "$here/server.log"
