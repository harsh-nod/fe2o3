#!/usr/bin/env bash
set -euo pipefail
stage=$(cd -- "$(dirname -- "$0")" && pwd)
repo=/home/harsh/.codex-tmp/fe2o3-c4-completion-20260917
commit=5d70cb0a6e16fb265fe224690274fdb0be2b0055
[[ $(git -C "$repo" rev-parse "$commit^{commit}") == "$commit" ]]
git -C "$repo" archive --format=tar --output="$stage/source.tar" "$commit" Cargo.toml Cargo.lock rust-toolchain.toml crates examples benchmarks/runtime_gfx942
[[ $(git get-tar-commit-id < "$stage/source.tar") == "$commit" ]]
git -C "$repo" ls-tree -r "$commit" -- Cargo.toml Cargo.lock rust-toolchain.toml crates examples benchmarks/runtime_gfx942 > "$stage/source-tree.txt"
printf '%s\n' "$commit" > "$stage/source.commit"
cd "$stage"
sha256sum source.tar source-tree.txt source.commit
