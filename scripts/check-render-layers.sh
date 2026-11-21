#!/usr/bin/env bash
# The layers of crates/omsi-render, checked (crates/omsi-render/ARCHITECTURE.md).
#
# Each file under src/<domain>/ may name `crate::<other>` only for a domain on a lower layer:
#
#   api > renderer > output > modes/vanilla_plus | modes/enhanced > interface > effects
#       > passes > targets > scene > sky > assets | lighting > shaders > gpu
#
# Domains on the same layer do not import each other, nothing imports `api`, and only
# `renderer` and `api` import `output`. Grouped imports (`use crate::{a, b}`) count like paths.
# The crate root (lib.rs) and the tests are not checked. Two more rules (render modes,
# `.ai/specs/001-graphics-modes/contracts/render-structure.md`):
#
#   - only `renderer/build.rs`, the one function that builds the mode, names a mode;
#   - a mode's `shaders/` folder is included by that mode alone (`modes/<a>/shaders/` in a
#     file outside `modes/<a>` is an error).
#
#   scripts/check-render-layers.sh [--strict]
#
# `RENDER_SRC=<dir>` checks another source folder (the script's own tests plant imports there).
#
# Prints one line per import that points up or sideways. Exit status: 0, or 1 with --strict
# when there is one (as CI runs it).
set -euo pipefail

cd "${RENDER_SRC:-$(dirname "$0")/../crates/omsi-render/src}"

layer() {
    case "$1" in
        api) echo 12 ;;
        renderer) echo 11 ;;
        output) echo 10 ;;
        modes | modes/*) echo 9 ;;
        interface) echo 8 ;;
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
    while (/crate::(?:(modes::[a-z_]+|[a-z_]+)|\{)/g) {
        if (defined $1) {
            (my $m = $1) =~ s{::}{/};
            print "$ARGV:", line_at($-[1]), ":$m\n";
            next;
        }
        my ($depth, $start) = (1, pos);
        my $seg = $start;
        for (my $i = $start; $i < length && $depth; $i++) {
            my $c = substr($_, $i, 1);
            if ($c eq "{") { $depth++ }
            elsif ($c eq "}") { $depth-- }
            if ($depth == 0 || ($depth == 1 && $c eq ",")) {
                if (substr($_, $seg, $i - $seg) =~ /^\s*(modes::[a-z_]+|[a-z_]+)\b/) {
                    (my $m = $1) =~ s{::}{/};
                    print "$ARGV:", line_at($seg + $-[1]), ":$m\n";
                }
                $seg = $i + 1;
            }
        }
    }
'

domains=()
for dir in */; do
    if [ "${dir%/}" = modes ]; then
        for mode in modes/*/; do domains+=("${mode%/}"); done
    else
        domains+=("${dir%/}")
    fi
done

count=0
for from in "${domains[@]}"; do
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
        elif [[ "$to" == modes* ]] && [ "$file" != renderer/build.rs ]; then
            why="only renderer/build.rs names a mode"
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

# a mode's shaders belong to that mode: `modes/<name>/shaders/` outside `modes/<name>`
while IFS=: read -r file line name; do
    case "$file" in ./modes/"$name"/*) continue ;; esac
    echo "crates/omsi-render/src/${file#./}:$line: modes/$name/shaders: only modes/$name includes its shaders"
    count=$((count + 1))
done < <(grep -rnoE --include='*.rs' 'modes/[a-z_]+/shaders/' . \
    | sed -E 's#^([^:]+):([0-9]+):modes/([a-z_]+)/shaders/#\1:\2:\3#' | sort -u)

echo "$count import(s) out of layer"
if [ "$count" -gt 0 ] && [ "${1:-}" = --strict ]; then
    exit 1
fi
