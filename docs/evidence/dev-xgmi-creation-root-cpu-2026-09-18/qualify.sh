#!/usr/bin/env bash
set -euo pipefail
ulimit -c 0
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1 CARGO_PROFILE_TEST_OPT_LEVEL=1 CARGO_PROFILE_TEST_DEBUG_ASSERTIONS=true CARGO_PROFILE_TEST_OVERFLOW_CHECKS=true)
record() { local name=$1; shift; bash "$archive/record.sh" "$name" "$@"; }
source_inventory=docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py
filters=(sdma::xgmi_creation::tests:: sdma::tests::xgmi_ sdma::tests::creation_guards_cover_the_first_memory_operation_and_xgmi_route_scope sdma::tests::sdma_copy_manifest_digest_is_frozen queue_linux::tests::terminal_creation_arm_poisons_on_drop_or_unwind_and_disarms_only_on_success queue::live::construction_primary::integration_tests::release_cases::sdma_creation_cases::)
record source-before python3 -I "$source_inventory"
record rustc rustc -vV
record cargo cargo -V
record clippy "${environment[@]}" cargo clippy --frozen -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
record no-default "${environment[@]}" cargo check --frozen -p fe2o3-kfd -p fe2o3-runtime --no-default-features
for target in gnu musl; do
    target_args=()
    if [[ $target == musl ]]; then target_args=(--target x86_64-unknown-linux-musl); fi
    record "$target-roster" "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features "${target_args[@]}" --lib -- --list "${filters[@]}"
    record "$target-kfd" "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features "${target_args[@]}" --lib -- "${filters[@]}"
    record "$target-runtime-roster" "${environment[@]}" cargo test --frozen -p fe2o3-runtime --all-features "${target_args[@]}" --lib -- --list kfd_backend::tests::native_xgmi_
    record "$target-runtime" "${environment[@]}" cargo test --frozen -p fe2o3-runtime --all-features "${target_args[@]}" --lib -- kfd_backend::tests::native_xgmi_
    record "$target-example" "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features "${target_args[@]}" --example kfd-sdma-xgmi-peer-benchmark
done
record unsafe-source "${environment[@]}" cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
record fmt cargo fmt --all --check
record diff git diff --check
record source-after python3 -I "$source_inventory"
record source-unchanged cmp "$archive/raw/source-before.log" "$archive/raw/source-after.log"
record calibration python3 -B docs/evidence/dev-xgmi-creation-root-cpu-2026-09-18/test_verify.py
