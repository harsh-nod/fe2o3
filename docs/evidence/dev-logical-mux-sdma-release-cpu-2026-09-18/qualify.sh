#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
record() { local name=$1; shift; bash "$archive/record.sh" "$name" "$@"; }
source_inventory=docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py
filters=(queue::live::construction_primary::integration_tests::release_cases:: queue::live::primary_release::tests:: sdma_cleanup)
record source-before python3 -I "$source_inventory"
record rustc rustc -vV
record cargo cargo -V
record clippy "${environment[@]}" cargo clippy --frozen -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
record no-default "${environment[@]}" cargo check --frozen -p fe2o3-kfd -p fe2o3-runtime --no-default-features
record gnu-roster "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --lib -- --list "${filters[@]}"
record gnu-release "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --lib -- "${filters[@]}"
record musl-roster "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib -- --list "${filters[@]}"
record musl-release "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib -- "${filters[@]}"
record runtime "${environment[@]}" cargo test --frozen -p fe2o3-runtime --all-features --lib kfd_backend::retained_release_tests::runtime_
record unsafe-source "${environment[@]}" cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
record fmt cargo fmt --all --check
record diff git diff --check
record source-after python3 -I "$source_inventory"
record source-unchanged cmp "$archive/raw/source-before.log" "$archive/raw/source-after.log"
record calibration python3 -B docs/evidence/dev-logical-mux-sdma-release-cpu-2026-09-18/test_verify.py
