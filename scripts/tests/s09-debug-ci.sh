#!/usr/bin/env bash

set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly LANE="${ROOT}/scripts/s09-debug-ci.sh"
readonly FINALIZER="${ROOT}/scripts/s09-debug-finalize.sh"
readonly PINNER="${ROOT}/scripts/s09_pinned_snapshot.py"
readonly SOURCE_STATE="${ROOT}/scripts/s09-source-state.py"
readonly CI_LOCAL="${ROOT}/scripts/ci-local.sh"
readonly WORKFLOW="${ROOT}/.github/workflows/s09-debug.yml"
readonly PILOT_DOC="${ROOT}/docs/s09-source-debug-pilot-v1.md"

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
rg -q -- '--test s09_gfx942_alpha_hardware' "${LANE}"
rg -q 's09-rocgdb-profile.sh' "${LANE}"
rg -Fq 's09-debug-hardware) run_s09_debug_hardware' "${CI_LOCAL}"
rg -q 'Manifest V2 capability-only local pilot' "${FINALIZER}"
rg -q 's09-evidence-manifest-v2.tsv' "${FINALIZER}"
rg -q 'identity-fields --hsaco' "${FINALIZER}"
rg -q 's09-identity-fields-v2.tsv' "${FINALIZER}"
expect_fail rg -q 'FE2O3_S09_(PORTABLE|ORDERED|CRATE|KERNEL|OBSERVED|RUSTC|CARGO|BACKEND)' \
  "${LANE}" "${FINALIZER}"
rg -q 'check-capability' "${FINALIZER}"
rg -Fq -- '--host-executable "${HARDWARE_TEST}"' "${FINALIZER}"
rg -Fq -- '--expected-commit "${SOURCE_COMMIT}"' "${FINALIZER}"
rg -Fq -- '--expected-tree "${SOURCE_TREE}"' "${FINALIZER}"
rg -q 's09_pinned_snapshot.py' "${LANE}" "${FINALIZER}"
rg -q 'F_SEAL_WRITE' "${PINNER}"
rg -q 'st_ctime_ns' "${PINNER}"
rg -q '"status"' "${SOURCE_STATE}"
rg -q '"--porcelain=v1"' "${SOURCE_STATE}"
expect_fail rg -q '2f5e34|2d2a566' "${LANE}"
rg -Fq 'pinned_cargo_image_sha256' "${PILOT_DOC}"
rg -Fq 'observed_parent_pid' "${PILOT_DOC}"
rg -Fq 'brokered build observation' "${PILOT_DOC}"
rg -Fq 'descriptor-based `fchdir`' "${PILOT_DOC}"
rg -Fq 'calls `env_clear()`' "${PILOT_DOC}"
rg -Fq 'independently remeasures' "${PILOT_DOC}"
rg -Fq 'alpha-only COV6 HSACO' "${PILOT_DOC}"
rg -Fq '3,359-byte length' "${PILOT_DOC}"
rg -Fq '73c1ff5e2f29d245c8071bdb6c1a38af1c9ee1573b78d47a987633483b37e084' \
  "${PILOT_DOC}"
[[ ! -e "${WORKFLOW}" ]]
git -C "${ROOT}" diff --quiet -- .github/CODEOWNERS
rg -q $'^supplemental\tS09\tMissing$' "${ROOT}/docs/cuda-oxide-parity-status.tsv"

printf 'S09 non-authoritative local pilot guards passed\n'
