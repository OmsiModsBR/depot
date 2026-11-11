#!/usr/bin/env bash
# Every equation of the physics model documents checked by a test (spec 002, SC-004).
#
# Collects the ids of docs/physics/*.md (the "## C-1." ... "## KA-6b." headings: C-*, K-*,
# KA-*, D-*, DA-*) and the ids the tests name on their "Checks:" lines (crates/*/tests), and
# lists every id no test names.
#
#   scripts/physics-model-coverage.sh
#
# Exit status: 0 every id is named by a test; 1 some are not (they are listed).
set -euo pipefail

root=$(git rev-parse --show-toplevel)
cd "$root"
ids=$(grep -hoE '^## (C|K|KA|D|DA)-[0-9]+b?\.' docs/physics/*.md | sed 's/^## //; s/\.$//' | sort -u)
named=$(grep -rhoE 'Checks:.*' crates/*/tests | grep -oE '\b(C|K|KA|D|DA)-[0-9]+b?\b' | sort -u)
missing=$(comm -23 <(echo "$ids") <(echo "$named"))
if [ -n "$missing" ]; then
    echo "physics-model-coverage: no test names these ids (a 'Checks:' line):"
    echo "$missing" | sed 's/^/  /'
    exit 1
fi
echo "physics-model-coverage: every id of docs/physics/ is named by a test ($(echo "$ids" | wc -l))"
