#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
environment=(env CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_BUILD_JOBS=2 CARGO_TERM_COLOR=never RUST_TEST_THREADS=1)
record() { bash "$archive/record.sh" "$@"; }

record source-before-final python3 -I "$archive/source.py"
record rustc-final rustc -vV
record cargo-final cargo -V
record gnu-runtime-build-final "${environment[@]}" \
    cargo test --frozen -p fe2o3-runtime --all-features --lib --no-run
record musl-runtime-build-final "${environment[@]}" \
    cargo test --frozen -p fe2o3-runtime --all-features --lib \
    --target x86_64-unknown-linux-musl --no-run
record binaries-before-final python3 -I "$archive/binaries.py" final
record gnu-runtime-final "${environment[@]}" \
    cargo test --frozen -p fe2o3-runtime --all-features --lib
record musl-runtime-final "${environment[@]}" \
    cargo test --frozen -p fe2o3-runtime --all-features --lib \
    --target x86_64-unknown-linux-musl
record binaries-after-final python3 -I "$archive/binaries.py" final
record source-after-final python3 -I "$archive/source.py"
