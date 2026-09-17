#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
record() { bash "$archive/record.sh" "$@"; }
profile=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0)
record gates-source-before sha256sum --check "$archive/source-files.sha256"
record clippy "${profile[@]}" cargo clippy --locked --offline \
    -p fe2o3-host -p fe2o3-runtime --all-features --all-targets -- -D warnings
record fmt cargo fmt --all -- --check
record no-default "${profile[@]}" cargo check --locked --offline \
    -p fe2o3-host -p fe2o3-runtime --no-default-features
record unsafe-policy "${profile[@]}" cargo test --locked --offline \
    -p cargo-fe2o3 --test unsafe_source_policy
record doctests "${profile[@]}" cargo test --locked --offline \
    -p fe2o3-host -p fe2o3-runtime --all-features --doc
record gates-source-after sha256sum --check "$archive/source-files.sha256"
