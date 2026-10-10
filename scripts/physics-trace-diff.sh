#!/usr/bin/env bash
# The vehicle physics of two commits, compared frame by frame.
#
# Runs the scenarios of crates/omsi-sim/tests/physics_traces.rs - this checkout's copy of
# the file, on both sides - on <base> and on the working tree, once with the physics' A/B
# switches off and once with each of them on, and compares the traces. A change that only
# moves code (the structural phases of the physics refactor) leaves them all identical,
# to the bit; a change that is meant to alter the physics shows where it does.
#
#   scripts/physics-trace-diff.sh [base-ref]        (default: origin/main)
#
# OUT          where the traces go (default: target/physics-traces)
# CARGO_FLAGS  more flags for `cargo test` (e.g. --ignore-rust-version)
#
# Exit status: 0 the traces are identical; 1 they differ (the first difference of each
# trace is printed); 2 a run failed, or two runs of the working tree differ.
set -euo pipefail

base_ref=${1:-origin/main}
root=$(git rev-parse --show-toplevel)
out=${OUT:-$root/target/physics-traces}
harness=crates/omsi-sim/tests/physics_traces.rs
# (each switch is read once a process, so each takes a run of its own)
modes=(default OMSI_TYRE_SUSPENSION OMSI_NO_WHEEL_SLIP OMSI_KINEMATIC_TRAILERS)

base_sha=$(git -C "$root" rev-parse --verify "$base_ref^{commit}")
tree=$(mktemp -d)
rm -rf "$out"
mkdir -p "$out"
git -C "$root" worktree add --detach --force "$tree" "$base_sha" >/dev/null
trap 'git -C "$root" worktree remove --force "$tree" >/dev/null 2>&1 || true; rm -rf "$tree"' EXIT
mkdir -p "$tree/$(dirname "$harness")"
cp "$root/$harness" "$tree/$harness"

# run <checkout> <target dir> <label>: the scenarios in every mode, into $out/<label>/<mode>
# (each checkout builds in a target directory of its own: cargo names the workspace's
# crates the same in both and tells them apart by the files' times, so in one directory
# the second checkout could run the first one's build)
run() {
    local dir=$1 target=$2 label=$3 mode
    for mode in "${modes[@]}"; do
        local switch=()
        if [ "$mode" != default ]; then switch=("$mode=1"); fi
        echo "== $label: $mode"
        if ! (cd "$dir" && env ${switch[@]+"${switch[@]}"} OMSI_PHYSICS_TRACE_DIR="$out/$label/$mode" CARGO_TARGET_DIR="$target" \
            cargo test --locked -p omsi-sim --release ${CARGO_FLAGS:-} --test physics_traces -q); then
            echo "physics-trace-diff: the scenarios failed on $label ($mode)" >&2
            exit 2
        fi
    done
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
        echo "  ${f#./}: first difference at line $line, in $(head -n "$line" "$a/$f" | grep '^frame' | tail -n 1 | cut -d' ' -f1-4)"
        echo "    - $(sed -n "${line}p" "$a/$f" | cut -c1-240)"
        echo "    + $(sed -n "${line}p" "$b/$f" | cut -c1-240)"
    done
    return $differ
}

run "$tree" "$root/target/physics-base" base
run "$root" "${CARGO_TARGET_DIR:-$root/target}" head
run "$root" "${CARGO_TARGET_DIR:-$root/target}" head-again

status=0
if ! report "$out/head" "$out/head-again" >"$out/determinism.txt"; then
    echo "physics-trace-diff: two runs of the same commit differ - the traces are not deterministic:" >&2
    cat "$out/determinism.txt" >&2
    exit 2
fi
traces=$(find "$out/head" -name '*.trace' | wc -l)
if report "$out/base" "$out/head" >"$out/diff.txt"; then
    echo "physics-trace-diff: all $traces traces identical to ${base_ref} (${base_sha:0:10}), in ${#modes[@]} modes"
else
    status=1
    echo "physics-trace-diff: the physics differ from ${base_ref} (${base_sha:0:10}):"
    cat "$out/diff.txt"
fi
exit $status
