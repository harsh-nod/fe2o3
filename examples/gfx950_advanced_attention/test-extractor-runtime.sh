#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
# shellcheck disable=SC1091
source "$SCRIPT_DIR/gfx950-extractor-runtime.sh"

TEST_ROOT=$(mktemp -d)
trap 'rm -rf -- "$TEST_ROOT"' EXIT
mkdir -p -- "$TEST_ROOT/target/debug/deps"
printf '#!/usr/bin/env bash\nexit 0\n' > "$TEST_ROOT/target/debug/fe2o3-rustc-extract"
chmod 700 "$TEST_ROOT/target/debug/fe2o3-rustc-extract"

resolve_gfx950_extractor_runtime "$TEST_ROOT/target/debug/fe2o3-rustc-extract"
[[ $EXTRACTOR == "$TEST_ROOT/target/debug/fe2o3-rustc-extract" ]]
[[ $EXTRACTOR_RUNTIME_DIR == "$TEST_ROOT/target/debug" ]]
[[ $EXTRACTOR_DEPS_DIR == "$TEST_ROOT/target/debug/deps" ]]

cp -- "$EXTRACTOR" "$EXTRACTOR_DEPS_DIR/fe2o3-rustc-extract-hash"
resolve_gfx950_extractor_runtime "$EXTRACTOR_DEPS_DIR/fe2o3-rustc-extract-hash"
[[ $EXTRACTOR_RUNTIME_DIR == "$TEST_ROOT/target/debug" ]]
[[ $EXTRACTOR_DEPS_DIR == "$TEST_ROOT/target/debug/deps" ]]

ln -s -- "$EXTRACTOR" "$TEST_ROOT/extractor-link"
if resolve_gfx950_extractor_runtime "$TEST_ROOT/extractor-link" 2>/dev/null; then
    printf 'extractor resolver accepted a symlink\n' >&2
    exit 1
fi
rm -rf -- "$TEST_ROOT/target/debug/deps"
if resolve_gfx950_extractor_runtime "$TEST_ROOT/target/debug/fe2o3-rustc-extract" 2>/dev/null; then
    printf 'extractor resolver accepted a missing dependency directory\n' >&2
    exit 1
fi

printf 'PASS gfx950 external extractor runtime path validation\n'
