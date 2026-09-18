#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
filters=(constructed_generic_release constructed_directional_release constructed_primary_release primary_release::tests shared_memory::tests::pristine_abort)
record() { local name=$1; shift; bash "$archive/record.sh" "$name-final" "$@"; }
source_inventory=docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py

record source-before python3 -I "$source_inventory"
record rustc rustc -vV
record cargo cargo -V
record clippy "${environment[@]}" cargo clippy --frozen -p fe2o3-kfd -p fe2o3-runtime --all-features --all-targets -- -D warnings
record gnu-kfd "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --lib -- "${filters[@]}"
record gnu-runtime "${environment[@]}" cargo test --frozen -p fe2o3-runtime --all-features --lib
record musl-kfd "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --lib -- "${filters[@]}"
record musl-runtime "${environment[@]}" cargo test --frozen -p fe2o3-runtime --all-features --target x86_64-unknown-linux-musl --lib
record no-default "${environment[@]}" cargo check --frozen -p fe2o3-kfd -p fe2o3-runtime --no-default-features
record docs "${environment[@]}" cargo test --frozen -p fe2o3-kfd -p fe2o3-runtime --all-features --doc
record unsafe-policy "${environment[@]}" cargo test --frozen -p cargo-fe2o3 --test unsafe_source_policy
record fmt cargo fmt --all --check
record diff git diff --check
record source-after python3 -I "$source_inventory"
record source-unchanged cmp "$archive/raw/source-before-final.log" "$archive/raw/source-after-final.log"
