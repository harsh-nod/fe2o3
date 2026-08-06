#!/usr/bin/env bash

set -Eeuo pipefail

readonly TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly PYCACHE_ROOT="${REPO_ROOT}/target/differential/pycache"

export PYTHONDONTWRITEBYTECODE=1
export PYTHONPYCACHEPREFIX="${PYCACHE_ROOT}"

bash -n "${REPO_ROOT}/scripts/differential/run.sh"
python3 -m py_compile \
  "${REPO_ROOT}/scripts/differential/harness.py" \
  "${REPO_ROOT}/scripts/differential/compare.py" \
  "${REPO_ROOT}/scripts/differential/tests.py"
python3 "${REPO_ROOT}/scripts/differential/tests.py"

printf '%s\n' 'differential conformance self-tests passed'
