#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT

cd -- "${REPO_ROOT}"

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  printf '%s\n' 'authenticated Verus V2 requires the reviewed Linux x86_64 host' >&2
  exit 2
fi

rustup show active-toolchain
rustc -Vv
cargo -V

cargo test -p fe2o3-verifier \
  --test authenticated_verus_execution_v2 \
  -- --include-ignored --test-threads=1
cargo test --release -p fe2o3-verifier \
  --test authenticated_verus_execution_v2 \
  -- --include-ignored --test-threads=1
