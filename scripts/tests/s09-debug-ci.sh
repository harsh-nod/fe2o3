#!/usr/bin/env bash

set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly LANE="${ROOT}/scripts/s09-debug-ci.sh"
readonly CI_LOCAL="${ROOT}/scripts/ci-local.sh"
readonly WORKFLOW="${ROOT}/.github/workflows/s09-debug.yml"
readonly CODEOWNERS="${ROOT}/.github/CODEOWNERS"

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
rg -q 'capability-only local pilot' "${LANE}"
[[ ! -e "${WORKFLOW}" ]]
rg -Fq '/.github/workflows/s09-* @powderluv' "${CODEOWNERS}"
rg -Fq '/crates/rustc-codegen-fe2o3/src/source_debug.rs @powderluv' "${CODEOWNERS}"
rg -Fq '/scripts/s09-* @powderluv' "${CODEOWNERS}"
rg -Fq '/scripts/tests/s09-* @powderluv' "${CODEOWNERS}"
rg -q $'^supplemental\tS09\tMissing$' "${ROOT}/docs/cuda-oxide-parity-status.tsv"

printf 'S09 non-authoritative local pilot guards passed\n'
