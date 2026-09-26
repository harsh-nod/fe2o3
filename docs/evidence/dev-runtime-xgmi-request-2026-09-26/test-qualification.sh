#!/usr/bin/env bash
set -euo pipefail
packet=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)
copy=${1:?qualification source directory required}
name=${2:-qualification}
cd -- "$copy"
export CARGO_TARGET_DIR="$copy/target"
export CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 CARGO_INCREMENTAL=0
status=0
printf '%s\n' 'cargo test -p fe2o3-runtime --all-features --lib --locked --offline -- ::qualification:: --test-threads=2' > "$packet/raw/$name.command"
cargo test -p fe2o3-runtime --all-features --lib --locked --offline \
    -- ::qualification:: --test-threads=2 > "$packet/raw/$name.log" 2>&1 || status=$?
printf '%s\n' "$status" > "$packet/raw/$name.status"
find "$CARGO_TARGET_DIR/debug/deps" -maxdepth 1 -type f -executable -name 'fe2o3_runtime-*' \
    -exec sha256sum '{}' + > "$packet/raw/$name-binaries.sha256"
cat "$packet/raw/$name.log"
exit "$status"
