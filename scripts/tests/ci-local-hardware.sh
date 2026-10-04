#!/usr/bin/env bash

set -Eeuo pipefail

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "${TEST_SCRIPT_DIR}/../ci-local.sh"

readonly TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "${TEST_ROOT}"' EXIT

touch "${TEST_ROOT}/kfd" "${TEST_ROOT}/dxg"

require_gpu_access "${TEST_ROOT}/kfd" "${TEST_ROOT}/missing-dxg"

if HSA_ENABLE_DXG_DETECTION= \
  require_gpu_access "${TEST_ROOT}/missing-kfd" "${TEST_ROOT}/dxg"; then
  printf '%s\n' 'DXG access unexpectedly succeeded without opt-in' >&2
  exit 1
fi

HSA_ENABLE_DXG_DETECTION=1 \
  require_gpu_access "${TEST_ROOT}/missing-kfd" "${TEST_ROOT}/dxg"

if require_gpu_access \
  "${TEST_ROOT}/missing-kfd" "${TEST_ROOT}/missing-dxg"; then
  printf '%s\n' 'GPU access unexpectedly succeeded without a device node' >&2
  exit 1
fi

printf '%s\n' 'hardware guard tests passed'
