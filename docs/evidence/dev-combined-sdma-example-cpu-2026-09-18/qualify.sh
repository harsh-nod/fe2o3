#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
record() { local name=$1; shift; bash "$archive/record.sh" "$name" "$@"; }
source_inventory=docs/evidence/dev-sdma-direct-readback-cpu-2026-09-18/source.py
record source-before python3 -I "$source_inventory"
record rustc rustc -vV
record cargo cargo -V
record gnu-example "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --example kfd-compute-aql-queue
record musl-example "${environment[@]}" cargo test --frozen -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --example kfd-compute-aql-queue
record clippy "${environment[@]}" cargo clippy --frozen -p fe2o3-kfd --all-features --all-targets -- -D warnings
record musl-build "${environment[@]}" cargo build --frozen -p fe2o3-kfd --all-features --target x86_64-unknown-linux-musl --example kfd-compute-aql-queue
record binary sha256sum target/x86_64-unknown-linux-musl/debug/examples/kfd-compute-aql-queue
record binary-kind file target/x86_64-unknown-linux-musl/debug/examples/kfd-compute-aql-queue
record fmt cargo fmt --all --check
record diff git diff --check
record source-after python3 -I "$source_inventory"
record source-unchanged cmp "$archive/raw/source-before.log" "$archive/raw/source-after.log"
