#!/usr/bin/env bash
# Tests of scripts/check-render-layers.sh: a planted import the script must fail on, for each
# rule, and a clean tree it must pass.
#
#   scripts/tests/check-render-layers-test.sh
set -euo pipefail

script="$(cd "$(dirname "$0")/.." && pwd)/check-render-layers.sh"
work="$(mktemp -d)"
trap 'rm -rf "$work"' EXIT

failures=0

# A source tree with every domain, nothing imported: `tree <dir>`.
tree() {
    rm -rf "$1"
    mkdir -p "$1"
    for d in api renderer output modes/vanilla_plus modes/enhanced interface effects passes \
        targets scene sky assets lighting shaders gpu; do
        mkdir -p "$1/$d"
        echo "// $d" >"$1/$d/mod.rs"
    done
    echo "// the build function" >"$1/renderer/build.rs"
}

# The two rendering crates' manifests, each with a dependency list: `crates_tree <dir>`.
crates_tree() {
    rm -rf "$1"
    for c in omsi-render omsi-render-vanilla; do
        mkdir -p "$1/$c"
        printf '[package]\nname = "%s"\n\n[dependencies]\nlog = "0.4"\n' "$c" >"$1/$c/Cargo.toml"
    done
}

# `plant <dir> <file> <line>`: appends a line to a file of the tree.
plant() {
    echo "$3" >>"$1/$2"
}

# `expect_fail <name> <file> <line>`: the script exits 1 on the tree with the line planted.
expect_fail() {
    local dir="$work/t"
    tree "$dir"
    plant "$dir" "$2" "$3"
    if RENDER_SRC="$dir" "$script" --strict >/dev/null; then
        echo "FAIL  $1: the script passed a planted import ($3 in $2)"
        failures=$((failures + 1))
    else
        echo "ok    $1"
    fi
}

# `expect_pass <name> <file> <line>`: the script exits 0 on the tree with the line planted.
expect_pass() {
    local dir="$work/t"
    tree "$dir"
    plant "$dir" "$2" "$3"
    if RENDER_SRC="$dir" "$script" --strict >/dev/null; then
        echo "ok    $1"
    else
        echo "FAIL  $1: the script refused ($3 in $2)"
        failures=$((failures + 1))
    fi
}

tree "$work/clean"
if RENDER_SRC="$work/clean" "$script" --strict >/dev/null; then
    echo "ok    a clean tree"
else
    echo "FAIL  a clean tree was refused"
    failures=$((failures + 1))
fi

# the layers that were there before
expect_fail "gpu imports passes" gpu/mod.rs "use crate::passes::Batch;"
expect_fail "nothing imports api" renderer/build.rs "use crate::api::Renderer;"
expect_fail "only renderer and api import output" effects/mod.rs "use crate::output::Presenter;"
expect_pass "passes import gpu" passes/mod.rs "use crate::gpu::Gpu;"

# interface sits between effects and the modes
expect_fail "effects import interface" effects/mod.rs "use crate::interface::FramePlan;"
expect_fail "interface imports a mode" interface/mod.rs "use crate::modes::enhanced::Mode;"
expect_fail "interface imports renderer" interface/mod.rs "use crate::renderer::Renderer;"
expect_pass "interface imports the core" interface/mod.rs "use crate::{passes::Batch, scene::Scene};"

# rule 1: the modes never import each other
expect_fail "vanilla+ imports enhanced" modes/vanilla_plus/mod.rs "use crate::modes::enhanced::Mode;"
expect_fail "enhanced imports vanilla+ (grouped)" modes/enhanced/mod.rs "use crate::{modes::vanilla_plus, gpu};"

# rule 2: nothing below the modes imports them
expect_fail "passes import a mode" passes/mod.rs "use crate::modes::vanilla_plus::Mode;"
expect_fail "scene imports a mode (grouped)" scene/mod.rs "use crate::{gpu, modes::enhanced};"

# rule 3: a mode imports the core and interface only
expect_fail "a mode imports output" modes/enhanced/mod.rs "use crate::output::Presenter;"
expect_fail "a mode imports renderer" modes/vanilla_plus/mod.rs "use crate::renderer::Renderer;"
expect_fail "a mode imports api" modes/enhanced/mod.rs "use crate::api::Renderer;"
expect_pass "a mode imports the core and interface" modes/enhanced/mod.rs \
    "use crate::{gpu::Gpu, interface::FramePlan, passes::Batch};"

# rule 4: only renderer/build.rs names a mode
expect_fail "renderer names a mode outside build.rs" renderer/mod.rs "use crate::modes::enhanced::Mode;"
expect_fail "api names a mode" api/mod.rs "use crate::modes::vanilla_plus::Mode;"
expect_pass "build.rs names both modes" renderer/build.rs \
    "use crate::modes::{enhanced::Mode as A, vanilla_plus::Mode as B};"

# rule 6: a mode's shaders belong to that mode
expect_fail "the core includes a mode's shaders" shaders/mod.rs \
    'const S: &str = include_str!("../modes/enhanced/shaders/post.wgsl");'
expect_fail "a mode includes another's shaders" modes/vanilla_plus/mod.rs \
    'const S: &str = include_str!("../enhanced/shaders/post.wgsl"); // modes/enhanced/shaders/post.wgsl'
expect_pass "a mode includes its own shaders" modes/enhanced/mod.rs \
    'const S: &str = include_str!("shaders/post.wgsl"); // modes/enhanced/shaders/post.wgsl'

# rule 5: neither rendering crate depends on the other
expect_crates_fail() {
    local dir="$work/crates"
    crates_tree "$dir"
    echo "$3" >>"$dir/$2/Cargo.toml"
    if RENDER_CRATES="$dir" RENDER_SRC="$work/clean" "$script" --strict >/dev/null; then
        echo "FAIL  $1: the script passed a planted dependency ($3 in $2)"
        failures=$((failures + 1))
    else
        echo "ok    $1"
    fi
}
crates_tree "$work/crates"
if RENDER_CRATES="$work/crates" RENDER_SRC="$work/clean" "$script" --strict >/dev/null; then
    echo "ok    two independent crates"
else
    echo "FAIL  two independent crates were refused"
    failures=$((failures + 1))
fi
expect_crates_fail "omsi-render depends on the vanilla crate" omsi-render \
    'omsi-render-vanilla = { path = "../omsi-render-vanilla" }'
expect_crates_fail "omsi-render depends on it through the workspace" omsi-render \
    'omsi-render-vanilla.workspace = true'
expect_crates_fail "the vanilla crate depends on omsi-render" omsi-render-vanilla \
    'omsi-render = { path = "../omsi-render" }'
expect_crates_fail "a dependency table names the other crate" omsi-render-vanilla \
    $'[dev-dependencies.omsi-render]\npath = "../omsi-render"'

if [ "$failures" -gt 0 ]; then
    echo "$failures check(s) failed"
    exit 1
fi
echo "all checks passed"
