#!/usr/bin/env bash

set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly LANE="${ROOT}/scripts/s09-debug-ci.sh"
readonly CI_LOCAL="${ROOT}/scripts/ci-local.sh"
readonly WORKFLOW="${ROOT}/.github/workflows/s09-debug.yml"

expect_fail() {
  if "$@" >/dev/null 2>&1; then
    printf 'S09 CI guard unexpectedly succeeded:' >&2
    printf ' %q' "$@" >&2
    printf '\n' >&2
    exit 1
  fi
}

expect_fail "${LANE}"
expect_fail "${LANE}" relative-evidence
rg -q 'FE2O3_ALLOW_S09_DEBUG=1' "${LANE}"
rg -q 'worker_v2_s09_alpha_o0_preserves_source_dwarf_in_hsaco' "${LANE}"
rg -q -- '--test gfx942_two_kernel_hardware' "${LANE}"
rg -q 's09-rocgdb-profile.sh' "${LANE}"
rg -Fq 's09-debug-hardware) run_s09_debug_hardware' "${CI_LOCAL}"
rg -q 'scripts/ci-local.sh s09-debug-hardware' "${WORKFLOW}"
rg -q 'runs-on: \[self-hosted, linux, x64, rocm, amd-gpu\]' "${WORKFLOW}"
rg -q 'FE2O3_ALLOW_S09_DEBUG:' "${WORKFLOW}"
rg -q $'^supplemental\tS09\tMissing$' "${ROOT}/docs/cuda-oxide-parity-status.tsv"

printf 'S09 explicit CI lane guards passed\n'
