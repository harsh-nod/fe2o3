#!/usr/bin/env bash
set -uo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
cd -- "$repo" || exit 2
export CARGO_TARGET_DIR=${1:?owned target directory required}
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
packages=(-p fe2o3-runtime-model -p fe2o3-resource-accounting -p fe2o3-kfd -p fe2o3-runtime)
receipt="$packet/raw/final-status.tsv"
printf 'check\texit\n' > "$receipt"
failed=0
run() {
    local name=$1
    shift
    "$@" > "$packet/raw/final-$name.log" 2>&1
    local status=$?
    printf '%s\t%s\n' "$name" "$status" >> "$receipt"
    printf '%s: exit %s\n' "$name" "$status"
    if ((status != 0)); then failed=1; fi
}
run focused timeout 1800 cargo test "${packages[@]}" --all-features --lib --locked --offline retained_charge -- --test-threads=2
run libraries timeout 1800 cargo test "${packages[@]}" --all-features --lib --no-fail-fast --locked --offline \
    -- --test-threads=2 --skip queue::live::construction_primary::integration_tests
run construction timeout 1800 cargo test "${packages[@]}" --all-features --lib --no-fail-fast --locked --offline \
    -- initial_bind_cases replacement_cases same_engine_auxiliary_ construction_auxiliary::tests \
    backing_constructor_forwarding persistent_owner_and_queue_layouts recycled_detach --test-threads=2
run clippy timeout 1800 cargo clippy "${packages[@]}" --all-features --all-targets --locked --offline -- -D warnings
run doctests timeout 1800 cargo test "${packages[@]}" --all-features --doc --locked --offline
run no-default timeout 1800 cargo check "${packages[@]}" --no-default-features --locked --offline
run fmt timeout 1800 cargo fmt --all -- --check
exit "$failed"
