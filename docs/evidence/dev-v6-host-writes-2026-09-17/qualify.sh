#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
record() { bash "$archive/record.sh" "$@"; }
profile=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_TERM_COLOR=never)
packages=(-p fe2o3-runtime-model -p fe2o3-runtime -p fe2o3-host)
record source-before sha256sum --check "$archive/source-files.sha256"
record gnu "${profile[@]}" cargo test --locked --offline "${packages[@]}" --all-features --lib -- --test-threads=1
record musl "${profile[@]}" FE2O3_HIP_SYS_DISABLE=1 cargo test --locked --offline "${packages[@]}" --all-features --lib --target x86_64-unknown-linux-musl -- --test-threads=1
record clippy "${profile[@]}" cargo clippy --locked --offline "${packages[@]}" --all-features --all-targets -- -D warnings
record fmt cargo fmt -p fe2o3-runtime-model -p fe2o3-runtime -p fe2o3-host -- --check
record no-default "${profile[@]}" cargo check --locked --offline "${packages[@]}" --no-default-features
record unsafe-policy "${profile[@]}" cargo test --locked --offline -p cargo-fe2o3 --test unsafe_source_policy
record doctests "${profile[@]}" cargo test --locked --offline "${packages[@]}" --all-features --doc
record source-after sha256sum --check "$archive/source-files.sha256"
