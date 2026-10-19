#!/usr/bin/env bash
# The layers of crates/omsi-render, checked (crates/omsi-render/ARCHITECTURE.md).
#
# Each file under src/<domain>/ may name `crate::<other>` only for a domain on a lower layer:
#
#   api > renderer > output > effects > passes > targets > scene > sky > assets | lighting
#       > shaders > gpu
#
# Domains on the same layer do not import each other, nothing imports `api`, and only
# `renderer` and `api` import `output`. Grouped imports (`use crate::{a, b}`) count like paths.
# The crate root (lib.rs) and the tests are not checked.
#
#   scripts/check-render-layers.sh [--strict]
#
# Prints one line per import that points up or sideways. Exit status: 0, or 1 with --strict
# when there is one (as CI runs it).
set -euo pipefail

cd "$(dirname "$0")/../crates/omsi-render/src"

layer() {
    case "$1" in
        api) echo 10 ;;
        renderer) echo 9 ;;
        output) echo 8 ;;
        effects) echo 7 ;;
        passes) echo 6 ;;
        targets) echo 5 ;;
        scene) echo 4 ;;
        sky) echo 3 ;;
        assets | lighting) echo 2 ;;
        shaders) echo 1 ;;
        gpu) echo 0 ;;
        *) echo "" ;;
    esac
}

# Prints `file:line:name` for each module a file names after `crate::`, whether as a path
# (`crate::gpu::Gpu`) or inside a group (`use crate::{gpu::Gpu, passes}`, over several lines
# and nested), with the line the name is on.
imports='
    sub line_at { 1 + (substr($_, 0, $_[0]) =~ tr/\n//) }
    while (/crate::(?:([a-z_]+)|\{)/g) {
        if (defined $1) {
            print "$ARGV:", line_at($-[1]), ":$1\n";
            next;
        }
        my ($depth, $start) = (1, pos);
        my $seg = $start;
        for (my $i = $start; $i < length && $depth; $i++) {
            my $c = substr($_, $i, 1);
            if ($c eq "{") { $depth++ }
            elsif ($c eq "}") { $depth-- }
            if ($depth == 0 || ($depth == 1 && $c eq ",")) {
                if (substr($_, $seg, $i - $seg) =~ /^\s*([a-z_]+)\b/) {
                    print "$ARGV:", line_at($seg + $-[1]), ":$1\n";
                }
                $seg = $i + 1;
            }
        }
    }
'

count=0
for dir in */; do
    from="${dir%/}"
    from_layer="$(layer "$from")"
    [ -n "$from_layer" ] || continue
    while IFS=: read -r file line to; do
        to_layer="$(layer "$to")"
        [ -n "$to_layer" ] || continue
        [ "$to" != "$from" ] || continue
        why=""
        if [ "$to" = api ]; then
            why="nothing imports api"
        elif [ "$to" = output ] && [ "$from" != renderer ] && [ "$from" != api ]; then
            why="only renderer and api import output"
        elif [ "$to_layer" -gt "$from_layer" ]; then
            why="points up ($from is below $to)"
        elif [ "$to_layer" -eq "$from_layer" ]; then
            why="same layer ($from and $to)"
        fi
        if [ -n "$why" ]; then
            echo "crates/omsi-render/src/$file:$line: crate::$to: $why"
            count=$((count + 1))
        fi
    done < <(find "$from" -name '*.rs' -print0 | xargs -0 -r perl -0777 -ne "$imports" | sort -u)
done

echo "$count import(s) out of layer"
if [ "$count" -gt 0 ] && [ "${1:-}" = --strict ]; then
    exit 1
fi
