#!/bin/sh
# Build the dedicated server (Linux, or any Unix) into dist/server - or into the folder
# given as the first argument. The server is the game binary started with --server; the
# folder gets start.sh and, on the first start, server.cfg with its defaults.
set -eu
# NOTICE: the dedicated server is no longer built or packaged, on any architecture. Its source
# is kept untouched for the future multiplayer redo (docs/removed/MULTIPLAYER_DISABLED.md).
echo "Dedicated server builds are disabled (see docs/removed/MULTIPLAYER_DISABLED.md)." >&2
exit 1
repo="$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)"
cd "$repo"
dest="${1:-$repo/dist/server}"
export PATH="$HOME/.cargo/bin:$PATH"
command -v cargo >/dev/null || { echo "Install Rust (https://rustup.rs) first." >&2; exit 1; }
export OPENOMSI_VERSION="${OPENOMSI_VERSION:-$(sh scripts/version.sh 2>/dev/null || echo 0.0.0)}"
cargo build --locked --release -p omsi-app
mkdir -p "$dest"
cp target/release/depot "$dest/depot"
# (Steam's library beside it, x86-64 only: the program does not start without it there)
[ -f target/release/libsteam_api.so ] && cp target/release/libsteam_api.so "$dest/"
cp scripts/server/start.sh "$dest/start.sh"
cp docs/SERVER.md "$dest/README.md"
chmod +x "$dest/start.sh" "$dest/depot"
touch "$dest/.depot-content"
echo "Project Depot server $OPENOMSI_VERSION in $dest. Start it: $dest/start.sh /path/to/OMSI2"
