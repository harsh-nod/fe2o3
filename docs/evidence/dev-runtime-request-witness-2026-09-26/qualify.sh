#!/usr/bin/env bash
set -euo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
repo=$(cd -- "$packet/../../.." && pwd)
destination=${1:?new isolated source directory required}
mkdir -- "$destination"
cp -a "$repo/Cargo.toml" "$repo/Cargo.lock" "$repo/rust-toolchain.toml" \
    "$repo/crates" "$repo/examples" "$destination/"
cd -- "$destination"
patch --batch -p1 < "$packet/qualification/overlay.patch"
while IFS=$'\t' read -r source target; do
    mkdir -p -- "$(dirname -- "$target")"
    cp -- "$packet/qualification/$source" "$target"
done < "$packet/qualification/files.tsv"
export CARGO_TARGET_DIR="$destination/target"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
cargo test -p fe2o3-runtime --all-features --lib --locked --offline \
    -- ::qualification:: --test-threads=2
