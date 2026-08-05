#!/usr/bin/env bash

set -Eeuo pipefail

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${TEST_SCRIPT_DIR}/../.." && pwd)"
readonly MATRIX_SCRIPT="${REPO_ROOT}/scripts/remote-hardware-matrix.sh"
readonly TEST_ROOT="$(mktemp -d)"
readonly FAKE_BIN="${TEST_ROOT}/fake-bin"
readonly FAKE_ROCM="${TEST_ROOT}/fake-rocm"
readonly GPU_DEVICE="${TEST_ROOT}/kfd"
trap 'rm -rf -- "${TEST_ROOT}"' EXIT

mkdir -p -- "${FAKE_BIN}" "${FAKE_ROCM}/bin"
mkdir -p -- "${TEST_ROOT}/home"
: >"${GPU_DEVICE}"

cat >"${FAKE_BIN}/ssh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ "$1" == -- ]] || exit 90
shift
host="$1"
shift
export FAKE_REMOTE_HOST="${host}"
printf '%s\n' "${host}" >>"${FAKE_SSH_INVOCATIONS}"
[[ "$1" == bash && "$2" == -s && $# == 2 ]] || exit 91
exec bash -s
EOF

cat >"${FAKE_BIN}/cargo" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "${1:-}" == --version ]]; then
  if [[ -f test-mode && "$(<test-mode)" == toolchain-fail ]]; then
    exit 44
  fi
  printf '%s\n' 'cargo 1.0.0 (fake)'
  exit 0
fi
if [[ -f test-mode && "$(<test-mode)" == hsaco-fail ]]; then
  exit 43
fi
exit 0
EOF

cat >"${FAKE_BIN}/rustc" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
printf '%s\n' 'rustc 1.0.0 (fake)'
EOF

cat >"${FAKE_BIN}/rocminfo" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
if [[ "${FAKE_REMOTE_HOST:-}" == target-mismatch-host ]]; then
  printf '%s\n' '  Name:                    gfx000'
else
  printf '  Name:                    %s\n' "${FE2O3_TARGET%%:*}"
fi
EOF

chmod +x \
  "${FAKE_BIN}/ssh" "${FAKE_BIN}/cargo" \
  "${FAKE_BIN}/rustc" "${FAKE_BIN}/rocminfo"
ln -s "${FAKE_BIN}/rocminfo" "${FAKE_ROCM}/bin/rocminfo"

make_repo() {
  local path="$1"
  local mode="$2"
  mkdir -p -- "${path}/scripts"
  printf '/target/\n' >"${path}/.gitignore"
  printf '%s\n' "${mode}" >"${path}/test-mode"
  cat >"${path}/scripts/ci-local.sh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
mode="$(<test-mode)"
case "${1:-}" in
  rocm-compile)
    if [[ "${mode}" == compile-fail ||
      ("${mode}" == aggregate && "${FAKE_REMOTE_HOST:-}" == first-fails) ]]; then
      exit 41
    fi
    ;;
  hardware-smoke)
    [[ "${mode}" != smoke-fail ]] || exit 42
    mkdir -p target/fe2o3
    printf '%s\n' 'fake hsaco' >target/fe2o3/vecadd.hsaco
    ;;
  *) exit 40 ;;
esac
EOF
  chmod +x "${path}/scripts/ci-local.sh"
  git -C "${path}" init -q
  git -C "${path}" config user.name 'Matrix Test'
  git -C "${path}" config user.email 'matrix-test@example.invalid'
  git -C "${path}" add .
  git -C "${path}" commit -q -m "fixture ${mode}"
  git -C "${path}" rev-parse HEAD
}

run_matrix() {
  local name="$1"
  shift
  local output="${TEST_ROOT}/${name}.out"
  : >"${TEST_ROOT}/${name}.ssh"
  set +e
  HOME="${TEST_ROOT}/home" \
  PATH="${FAKE_BIN}:${PATH}" \
  FE2O3_SSH="${FAKE_BIN}/ssh" \
  FAKE_SSH_INVOCATIONS="${TEST_ROOT}/${name}.ssh" \
  FE2O3_MATRIX_LOG_DIR="${TEST_ROOT}/${name}-logs" \
    bash "${MATRIX_SCRIPT}" --gpu-device "${GPU_DEVICE}" "$@" \
      --rocm-path "${FAKE_ROCM}" \
      >"${output}" 2>&1
  RUN_STATUS=$?
  set -e
  RUN_OUTPUT="${output}"
  RUN_SSH_LOG="${TEST_ROOT}/${name}.ssh"
}

expect_status() {
  local expected="$1"
  if [[ "${RUN_STATUS}" != "${expected}" ]]; then
    printf 'expected status %s, got %s\n' "${expected}" "${RUN_STATUS}" >&2
    cat "${RUN_OUTPUT}" >&2
    return 1
  fi
}

expect_output() {
  local expected="$1"
  if ! grep -F -- "${expected}" "${RUN_OUTPUT}" >/dev/null; then
    printf 'missing output: %s\n' "${expected}" >&2
    cat "${RUN_OUTPUT}" >&2
    return 1
  fi
}

# Success executes the complete remote protocol at the exact commit.
success_repo="${TEST_ROOT}/success"
success_commit="$(make_repo "${success_repo}" success)"
run_matrix success --commit "${success_commit}" \
  --entry success-host gfx942 "${success_repo}"
expect_status 0
expect_output 'RESULT host=success-host target=gfx942 status=PASS stage=complete exit=0'

# A clean checkout at a different commit fails before any build command.
wrong_repo="${TEST_ROOT}/wrong-commit"
wrong_commit="$(make_repo "${wrong_repo}" success)"
printf '%s\n' second >"${wrong_repo}/second-commit"
git -C "${wrong_repo}" add second-commit
git -C "${wrong_repo}" commit -q -m second
run_matrix wrong-commit --commit "${wrong_commit}" \
  --entry wrong-host gfx942 "${wrong_repo}"
expect_status 1
expect_output 'status=FAIL stage=exact-commit exit=13'

# Untracked dirt participates in the fail-closed status check.
dirty_repo="${TEST_ROOT}/dirty"
dirty_commit="$(make_repo "${dirty_repo}" success)"
printf '%s\n' dirty >"${dirty_repo}/untracked"
run_matrix dirty --commit "${dirty_commit}" \
  --entry dirty-host gfx942 "${dirty_repo}"
expect_status 1
expect_output 'status=FAIL stage=clean-checkout exit=15'

# Missing pinned tools, GPU access, and the configured GPU target fail early.
toolchain_repo="${TEST_ROOT}/toolchain-fail"
toolchain_commit="$(make_repo "${toolchain_repo}" toolchain-fail)"
run_matrix toolchain-fail --commit "${toolchain_commit}" \
  --entry toolchain-host gfx942 "${toolchain_repo}"
expect_status 1
expect_output 'status=FAIL stage=toolchain exit=22'

run_matrix gpu-missing --gpu-device "${TEST_ROOT}/missing-kfd" \
  --commit "${success_commit}" --entry gpu-host gfx942 "${success_repo}"
expect_status 1
expect_output 'status=FAIL stage=gpu-access exit=25'

run_matrix target-mismatch --commit "${success_commit}" \
  --entry target-mismatch-host gfx950 "${success_repo}"
expect_status 1
expect_output 'status=FAIL stage=gpu-target exit=27'

# Compile, smoke, and explicit HSACO inspection failures retain their stage.
compile_repo="${TEST_ROOT}/compile-fail"
compile_commit="$(make_repo "${compile_repo}" compile-fail)"
run_matrix compile-fail --commit "${compile_commit}" \
  --entry compile-host gfx942 "${compile_repo}"
expect_status 1
expect_output 'status=FAIL stage=rocm-compile exit=41'

smoke_repo="${TEST_ROOT}/smoke-fail"
smoke_commit="$(make_repo "${smoke_repo}" smoke-fail)"
run_matrix smoke-fail --commit "${smoke_commit}" \
  --entry smoke-host gfx950 "${smoke_repo}"
expect_status 1
expect_output 'status=FAIL stage=hardware-smoke exit=42'

hsaco_repo="${TEST_ROOT}/hsaco-fail"
hsaco_commit="$(make_repo "${hsaco_repo}" hsaco-fail)"
run_matrix hsaco-fail --commit "${hsaco_commit}" \
  --entry hsaco-host gfx942 "${hsaco_repo}"
expect_status 1
expect_output 'status=FAIL stage=hsaco-inspection exit=43'

# The checkout is injected into a Bash payload with shell-safe quoting.
canary="${TEST_ROOT}/quoting-canary"
quoted_repo="${TEST_ROOT}/repo with spaces ' \$(touch ${canary}) ;"
quoted_commit="$(make_repo "${quoted_repo}" success)"
run_matrix quoting --commit "${quoted_commit}" \
  --entry quote-host gfx942 "${quoted_repo}"
expect_status 0
expect_output 'RESULT host=quote-host target=gfx942 status=PASS stage=complete exit=0'
[[ ! -e "${canary}" ]] || {
  printf '%s\n' 'remote checkout path was evaluated as shell code' >&2
  exit 1
}

# Every host runs even when an earlier host fails, and the aggregate fails.
aggregate_fail_repo="${TEST_ROOT}/aggregate-first"
aggregate_commit="$(make_repo "${aggregate_fail_repo}" aggregate)"
aggregate_second_repo="${TEST_ROOT}/aggregate-second"
git clone -q "${aggregate_fail_repo}" "${aggregate_second_repo}"
run_matrix aggregate --commit "${aggregate_commit}" \
  --entry first-fails gfx942 "${aggregate_fail_repo}" \
  --entry second-runs gfx942 "${aggregate_second_repo}"
expect_status 1
expect_output 'RESULT host=first-fails target=gfx942 status=FAIL stage=rocm-compile exit=41'
expect_output 'RESULT host=second-runs target=gfx942 status=PASS stage=complete exit=0'
[[ "$(wc -l <"${RUN_SSH_LOG}")" == 2 ]] || {
  printf '%s\n' 'matrix did not invoke every configured host' >&2
  cat "${RUN_SSH_LOG}" >&2
  exit 1
}

printf '%s\n' 'remote hardware matrix tests passed'
