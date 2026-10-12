#!/usr/bin/env bash
# The offscreen run's state of two commits, compared frame by frame (B0).
#
# Runs crates/omsi-app/tests/offscreen_state_trace.rs - this checkout's copy of the file, on
# both sides - on <base> and on the working tree, and compares the traces: where the bus
# is, the AI cars, the lights, the rail signals and the duty, every frame. A change that
# only moves code (the steps of the refactoring plan) leaves them identical, to the bit; a
# change that is meant to alter the behaviour shows where it does. The runs render, so they
# need a graphics adapter (a software one is enough: lavapipe on CI).
#
#   scripts/offscreen-trace-diff.sh [base-ref]        (default: origin/main)
#
# OUT          where the traces go (default: target/offscreen-traces)
# CARGO_FLAGS  more flags for `cargo test` (e.g. --ignore-rust-version)
#
# Exit status: 0 the traces are identical (or <base> has no state trace yet, and the change's
# own two runs agree); 1 they differ
# (the first difference of each trace is printed); 2 a run failed, or two runs of the
# working tree differ.
set -euo pipefail

base_ref=${1:-origin/main}
root=$(git rev-parse --show-toplevel)
out=${OUT:-$root/target/offscreen-traces}
harness=crates/omsi-app/tests/offscreen_state_trace.rs

base_sha=$(git -C "$root" rev-parse --verify "$base_ref^{commit}")
rm -rf "$out"
mkdir -p "$out"

# run <checkout> <target dir> <label>: the scenarios, into $out/<label>
# (each checkout builds in a target directory of its own, as in physics-trace-diff.sh)
run() {
    local dir=$1 target=$2 label=$3
    echo "== $label"
    if ! (cd "$dir" && OMSI_STATE_TRACE_DIR="$out/$label" CARGO_TARGET_DIR="$target" \
        cargo test --locked -p omsi-app --release ${CARGO_FLAGS:-} --test offscreen_state_trace -q); then
        echo "offscreen-trace-diff: the scenarios failed on $label" >&2
        exit 2
    fi
    if [ -z "$(find "$out/$label" -name '*.trace' 2>/dev/null)" ]; then
        echo "offscreen-trace-diff: $label wrote no trace" >&2
        exit 2
    fi
}

# report <a> <b>: where each trace of <a> first differs from <b>'s; 0 when none does
report() {
    local a=$1 b=$2 differ=0 f line
    for f in $( (cd "$a" && find . -name '*.trace'; cd "$b" && find . -name '*.trace') | sort -u); do
        if [ ! -f "$a/$f" ] || [ ! -f "$b/$f" ]; then
            echo "  ${f#./}: only on one side"
            differ=1
            continue
        fi
        cmp -s "$a/$f" "$b/$f" && continue
        differ=1
        line=$(cmp "$a/$f" "$b/$f" 2>/dev/null | sed -n 's/.* line \([0-9]*\).*/\1/p')
        line=${line:-1}
        echo "  ${f#./}: first difference at line $line, in $(head -n "$line" "$a/$f" | grep '^frame' | tail -n 1)"
        echo "    - $(sed -n "${line}p" "$a/$f" | cut -c1-240)"
        echo "    + $(sed -n "${line}p" "$b/$f" | cut -c1-240)"
    done
    return $differ
}

run "$root" "${CARGO_TARGET_DIR:-$root/target}" head
# (a base from before the state trace has nothing to compare with: the run above still
# shows that the scenario runs and repeats itself)
if ! git -C "$root" cat-file -e "$base_sha:crates/omsi-app/src/state_trace.rs" 2>/dev/null; then
    echo "offscreen-trace-diff: ${base_ref} (${base_sha:0:10}) has no state trace (OMSI_STATE_TRACE) to compare with; the change's own runs agree"
    exit 0
fi
tree=$(mktemp -d)
git -C "$root" worktree add --detach --force "$tree" "$base_sha" >/dev/null
trap 'git -C "$root" worktree remove --force "$tree" >/dev/null 2>&1 || true; rm -rf "$tree"' EXIT
mkdir -p "$tree/$(dirname "$harness")"
cp "$root/$harness" "$tree/$harness"
run "$tree" "$root/target/offscreen-base" base

# (the test itself runs the root twice and fails when the two runs differ)
traces=$(find "$out/head" -name '*.trace' | wc -l)
if report "$out/base" "$out/head" >"$out/diff.txt"; then
    echo "offscreen-trace-diff: all $traces traces identical to ${base_ref} (${base_sha:0:10})"
    exit 0
fi
echo "offscreen-trace-diff: the offscreen state differs from ${base_ref} (${base_sha:0:10}):"
cat "$out/diff.txt"
exit 1
