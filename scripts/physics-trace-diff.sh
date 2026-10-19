#!/usr/bin/env bash
# The vehicle physics of two commits, compared frame by frame.
#
# Runs the scenarios of crates/omsi-sim/tests/physics_traces.rs (the player's vehicle),
# crates/omsi-sim/tests/ai_traces.rs (the AI traffic's motion), traffic_traces.rs (the AI
# drivers on a lane network) and people_traces.rs (the humans' bones) - this checkout's
# copies of the files and of tests/support/, which they share, on both sides - on <base>
# and on the working tree, and compares the traces. A
# change that only moves code (the structural phases of the physics refactor) leaves them
# all identical, to the bit; a change that is meant to alter the physics shows where it
# does.
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
tests=crates/omsi-sim/tests
harnesses=(physics_traces ai_traces traffic_traces people_traces)

base_sha=$(git -C "$root" rev-parse --verify "$base_ref^{commit}")
tree=$(mktemp -d)
rm -rf "$out"
mkdir -p "$out"
git -C "$root" worktree add --detach --force "$tree" "$base_sha" >/dev/null
trap 'git -C "$root" worktree remove --force "$tree" >/dev/null 2>&1 || true; rm -rf "$tree"' EXIT
mkdir -p "$tree/$tests"
for harness in "${harnesses[@]}"; do
    cp "$root/$tests/$harness.rs" "$tree/$tests/$harness.rs"
done
rm -rf "$tree/$tests/support"
cp -R "$root/$tests/support" "$tree/$tests/support"
test_flags=()
for harness in "${harnesses[@]}"; do
    test_flags+=(--test "$harness")
done

# run <checkout> <target dir> <label>: the scenarios, into $out/<label>
# (each checkout builds in a target directory of its own: cargo names the workspace's
# crates the same in both and tells them apart by the files' times, so in one directory
# the second checkout could run the first one's build)
run() {
    local dir=$1 target=$2 label=$3
    echo "== $label"
    if ! (cd "$dir" && OMSI_PHYSICS_TRACE_DIR="$out/$label" CARGO_TARGET_DIR="$target" \
        cargo test --locked -p omsi-sim --release ${CARGO_FLAGS:-} "${test_flags[@]}" -q); then
        echo "physics-trace-diff: the scenarios failed on $label" >&2
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
    echo "physics-trace-diff: all $traces traces identical to ${base_ref} (${base_sha:0:10})"
else
    status=1
    echo "physics-trace-diff: the physics differ from ${base_ref} (${base_sha:0:10}):"
    cat "$out/diff.txt"
fi
exit $status
