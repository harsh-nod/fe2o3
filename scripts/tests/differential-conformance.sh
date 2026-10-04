#!/usr/bin/env bash

set -Eeuo pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
REPO_ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly REPO_ROOT
readonly SYNTAX_CHECK="${REPO_ROOT}/scripts/tests/python-syntax-only.sh"

export PYTHONDONTWRITEBYTECODE=1

bash -n "${REPO_ROOT}/scripts/differential/run.sh"
"${SYNTAX_CHECK}" \
  "${REPO_ROOT}/scripts/differential/harness.py" \
  "${REPO_ROOT}/scripts/differential/compare.py" \
  "${REPO_ROOT}/scripts/differential/tests.py"
python3 "${REPO_ROOT}/scripts/differential/tests.py"

printf '%s\n' 'differential conformance self-tests passed'
