#!/usr/bin/env bash
set -uo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
cd -- "$repo"
mkdir -p -- "$packet/raw"
export CARGO_TARGET_DIR=${1:?owned target directory required}
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
failed=0
run() {
    local name=$1
    shift
    printf '%s\n' "$*" > "$packet/raw/$name.command"
    "$@" > "$packet/raw/$name.log" 2>&1
    local status=$?
    printf '%s\n' "$status" > "$packet/raw/$name.status"
    printf '%s: %s\n' "$name" "$status"
    if (( status != 0 )); then failed=1; fi
}
run focused cargo test -p fe2o3-runtime --all-features --lib --locked --offline -- worker --test-threads=2
run libraries cargo test -p fe2o3-runtime --all-features --lib --locked --offline -- --test-threads=2
run docs cargo test -p fe2o3-runtime --all-features --doc --locked --offline
run clippy cargo clippy -p fe2o3-runtime --all-features --all-targets --locked --offline -- -D warnings
run minimal cargo check -p fe2o3-runtime --no-default-features --locked --offline
run formatting cargo fmt -p fe2o3-runtime -- --check
run whitespace git diff --check
find "$CARGO_TARGET_DIR/debug/deps" -maxdepth 1 -type f -executable -name 'fe2o3_runtime-*' \
    -exec sha256sum '{}' + > "$packet/raw/production-binaries.sha256"
exit "$failed"
