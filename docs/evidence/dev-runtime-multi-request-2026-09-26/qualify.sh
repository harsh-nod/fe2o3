#!/usr/bin/env bash
set -euo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
prior="$repo/docs/evidence/dev-runtime-request-witness-2026-09-26/qualification"
destination=${1:?new isolated source directory required}
mkdir -- "$destination"
mkdir -p -- "$packet/raw"
cp -a "$repo/Cargo.toml" "$repo/Cargo.lock" "$repo/rust-toolchain.toml" \
    "$repo/crates" "$repo/examples" "$destination/"
cd -- "$destination"
patch --batch --fuzz=0 --no-backup-if-mismatch -p1 < "$prior/overlay.patch"
while IFS=$'\t' read -r source target; do
    mkdir -p -- "$(dirname -- "$target")"
    cp -- "$prior/$source" "$target"
done < "$prior/files.tsv"
patch --batch --fuzz=0 --no-backup-if-mismatch -p1 < "$packet/qualification/overlay.patch"
cp -- "$packet/qualification/multi-tests.rs" crates/fe2o3-runtime/src/kfd_backend/multi_admission/qualification.rs
mkdir -p -- crates/fe2o3-runtime/src/kfd_backend/tests/sdma_allocation_tests
cp -- "$packet/qualification/cold-tests.rs" crates/fe2o3-runtime/src/kfd_backend/tests/sdma_allocation_tests/qualification.rs
bash "$packet/check-overlay.sh" "$destination"
export CARGO_TARGET_DIR="$destination/target"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p fe2o3-runtime --all-features --lib --locked --offline \
    -- ::qualification:: --test-threads=2
