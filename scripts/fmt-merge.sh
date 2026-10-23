#!/usr/bin/env bash
# Brings the current branch over a commit that only ran `cargo fmt` on some crates, without
# the conflicts a plain merge of that commit would give (macro-refactor plan, front A2).
#
#   scripts/fmt-merge.sh <fmt-commit> <crate>...
#
# 1. merges the commit just before the format commit, so the branch has everything else;
# 2. runs `cargo fmt -p <crate>` on the branch itself and commits the result;
# 3. merges the format commit with `-s ours`: the branch's own formatted code is kept, and
#    git records the format commit as merged, so later merges of main do not bring it again.
#
# The format commit must change nothing but formatting; anything else in it would be dropped
# by step 3. Use the toolchain CI uses (`cargo +<version> fmt`, set CARGO_FMT if needed), or
# the branch may come out formatted differently from main. A conflict in step 1 stops the
# script: resolve it, commit, and run the script again.
set -euo pipefail

if [ $# -lt 2 ]; then
    echo "usage: $0 <fmt-commit> <crate>..." >&2
    exit 2
fi
fmt_commit=$(git rev-parse --verify "$1^{commit}")
shift

if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    echo "the working tree has changes: commit or stash them first" >&2
    exit 1
fi
if git merge-base --is-ancestor "$fmt_commit" HEAD; then
    echo "the branch already contains $fmt_commit"
    exit 0
fi

git merge --no-edit "$fmt_commit^"

packages=()
for crate in "$@"; do
    packages+=(-p "$crate")
done
${CARGO_FMT:-cargo fmt} "${packages[@]}"
if [ -n "$(git status --porcelain --untracked-files=no)" ]; then
    git commit -q -am "Format the branch as in $(git rev-parse --short "$fmt_commit")"
fi

git merge --no-edit -s ours -m "Merge the format commit $(git rev-parse --short "$fmt_commit")" "$fmt_commit"
echo "done: merge main now to take what came after $(git rev-parse --short "$fmt_commit")"
