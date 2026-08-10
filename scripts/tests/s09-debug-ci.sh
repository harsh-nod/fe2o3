#!/usr/bin/env bash

set -Eeuo pipefail

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly LANE="${ROOT}/scripts/s09-debug-ci.sh"
readonly FINALIZER="${ROOT}/scripts/s09-debug-finalize.sh"
readonly PINNER="${ROOT}/scripts/s09_pinned_snapshot.py"
readonly SOURCE_STATE="${ROOT}/scripts/s09-source-state.py"
readonly RAW_GUARD="${ROOT}/scripts/s09-raw-transcript-guard.sh"
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
rg -Fq -- '--host-executable "${RETAINED_HOST}"' "${FINALIZER}"
rg -Fq -- '--debug-archive-manifest "${ARCHIVE_MANIFEST}"' "${FINALIZER}"
rg -Fq -- '--export "host=${RETAINED_HOST}"' "${FINALIZER}"
rg -Fq -- '--expected-commit "${SOURCE_COMMIT}"' "${FINALIZER}"
rg -Fq -- '--expected-tree "${SOURCE_TREE}"' "${FINALIZER}"
rg -q 's09_pinned_snapshot.py' "${LANE}" "${FINALIZER}"
rg -q 'F_SEAL_WRITE' "${PINNER}"
rg -q 's09_guarded_launcher' "${RAW_GUARD}"
rg -Fq '/usr/bin/python3 -I -S -c' "${RAW_GUARD}"
expect_fail rg -q 'unset PYTHON.*PYTHONPATH' "${RAW_GUARD}"
expect_fail rg -Fq 'exec /usr/bin/env -i' "${RAW_GUARD}"
rg -Fq 'PR_SET_DUMPABLE = 4' "${RAW_GUARD}"
rg -Fq 'PR_GET_DUMPABLE = 3' "${RAW_GUARD}"
rg -Fq 'libc.prctl(PR_SET_DUMPABLE, 0, 0, 0, 0) != 0' "${RAW_GUARD}"
rg -Fq 'libc.prctl(PR_GET_DUMPABLE, 0, 0, 0, 0) != 0' "${RAW_GUARD}"
rg -q 'os\.setsid\(\)' "${RAW_GUARD}"
rg -Fq 'os.killpg(os.getpgrp(), signal.SIGTERM)' "${RAW_GUARD}"
rg -Fq 'os.killpg(os.getpgrp(), signal.SIGKILL)' "${RAW_GUARD}"
rg -Fq 'stdin=subprocess.DEVNULL' "${RAW_GUARD}"
rg -q 's09_observe_guarded_exit' "${RAW_GUARD}"
expect_fail rg -q 'S09_GUARDED_STATUS_POLL_LIMIT' "${RAW_GUARD}"
rg -q 'S09_GUARDED_EXIT_POLL_LIMIT' "${RAW_GUARD}"
rg -Fq "printf 'DRAIN\\n'" "${RAW_GUARD}"
expect_fail rg -Fq "kill -TERM -- \"-\${child_pid}\"" "${RAW_GUARD}"
expect_fail rg -Fq "kill -KILL -- \"-\${child_pid}\"" "${RAW_GUARD}"
rg -Fq 'wait "${child_pid}"' "${RAW_GUARD}"
rg -q "trap 's09_guarded_exit' EXIT" "${RAW_GUARD}"
rg -q "trap 's09_guarded_signal 129' HUP" "${RAW_GUARD}"
rg -q "trap 's09_guarded_signal 130' INT" "${RAW_GUARD}"
rg -q "trap 's09_guarded_signal 143' TERM" "${RAW_GUARD}"
rg -q 'st_ctime_ns' "${PINNER}"
rg -q '"status"' "${SOURCE_STATE}"
rg -q '"--porcelain=v1"' "${SOURCE_STATE}"
rg -q 'tracked_regular_blobs' "${SOURCE_STATE}"
rg -q 'F_ADD_SEALS' "${SOURCE_STATE}"
rg -q 'ctime_ns' "${SOURCE_STATE}"
rg -Fq 'GIT_EXECUTABLE = pathlib.Path("/usr/bin/git")' "${SOURCE_STATE}"
rg -Uq '(?s)completed = run_bounded\(.{0,512}self\.descriptor,\s*\)' "${SOURCE_STATE}"
rg -Uq '(?s)def run_bounded\(.{0,128}descriptor: int.{0,512}pass_fds=\(descriptor,\)' "${SOURCE_STATE}"
rg -Fq '"GIT_CONFIG_NOSYSTEM": "1"' "${SOURCE_STATE}"
rg -Fq 'source tree contains unsupported tracked Git mode' "${SOURCE_STATE}"
rg -q -- '--source-supervised' "${LANE}"
expect_fail rg -q '2f5e34|2d2a566' "${LANE}"
rg -Fq 'pinned_cargo_image_sha256' "${PILOT_DOC}"
rg -Fq 'observed_parent_pid' "${PILOT_DOC}"
rg -Fq 'brokered build observation' "${PILOT_DOC}"
rg -Fq 'descriptor-based `fchdir`' "${PILOT_DOC}"
rg -Fq 'calls `env_clear()`' "${PILOT_DOC}"
rg -Fq 'independently remeasures' "${PILOT_DOC}"
rg -Fq 'alpha-only COV6 HSACO' "${PILOT_DOC}"
rg -Fq 'same-user process able' "${PILOT_DOC}"
rg -Fq '3,359-byte length' "${PILOT_DOC}"
rg -Fq '73c1ff5e2f29d245c8071bdb6c1a38af1c9ee1573b78d47a987633483b37e084' \
  "${PILOT_DOC}"
[[ ! -e "${WORKFLOW}" ]]
git -C "${ROOT}" diff --quiet -- .github/CODEOWNERS
rg -q $'^supplemental\tS09\tMissing$' "${ROOT}/docs/cuda-oxide-parity-status.tsv"

printf 'S09 non-authoritative local pilot guards passed\n'
