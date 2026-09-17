#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
record() { bash "$archive/record.sh" "corrected-$1" "${@:2}"; }
record clippy env CARGO_INCREMENTAL=0 cargo clippy --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime -p fe2o3-service-host --all-features --all-targets -- -D warnings
record fmt cargo fmt --all -- --check
record no-default env CARGO_INCREMENTAL=0 cargo check --locked --offline \
    -p fe2o3-runtime --no-default-features
record unsafe-policy env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p cargo-fe2o3 --test unsafe_source_policy
record doctests env CARGO_INCREMENTAL=0 cargo test --locked --offline \
    -p fe2o3-kfd -p fe2o3-runtime -p fe2o3-service-host --all-features --doc
record parser bash "$archive/check-parser.sh"
record source-check sha256sum --check "$archive/corrected-source-files.sha256"
