#!/usr/bin/env bash

set -Eeuo pipefail

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "${TEST_SCRIPT_DIR}/../ci-local.sh"

TIMEOUT_TEST_ROOT="$(mktemp -d)"
readonly TIMEOUT_TEST_ROOT
trap 'rm -rf "${TIMEOUT_TEST_ROOT}"' EXIT

set +e
timeout 10s env \
  FE2O3_CI_STEP_TIMEOUT_SECONDS=1 \
  CI_LOG_DIR="${TIMEOUT_TEST_ROOT}/logs" \
  bash -c '
    source "$1"
    mkdir -p "${CI_LOG_DIR}"
    run_step hanging-fixture bash -c "printf '\''hanging fixture started\\n'\''; sleep 30"
  ' bash "${TEST_SCRIPT_DIR}/../ci-local.sh" \
  >"${TIMEOUT_TEST_ROOT}/hanging.out" \
  2>"${TIMEOUT_TEST_ROOT}/hanging.err"
hanging_status=$?
set -e
if ((hanging_status != 124)); then
  printf 'hanging step returned %d instead of timeout status 124\n' \
    "${hanging_status}" >&2
  cat "${TIMEOUT_TEST_ROOT}/hanging.err" >&2
  exit 1
fi
rg -F 'hanging fixture started' \
  "${TIMEOUT_TEST_ROOT}/logs/hanging-fixture.log" >/dev/null
rg -F 'step hanging-fixture timed out after 1 seconds' \
  "${TIMEOUT_TEST_ROOT}/hanging.err" >/dev/null
rg -F 'step hanging-fixture failed with status 124' \
  "${TIMEOUT_TEST_ROOT}/hanging.err" >/dev/null

set +e
env FE2O3_CI_STEP_TIMEOUT_SECONDS=5 \
  CI_LOG_DIR="${TIMEOUT_TEST_ROOT}/status-logs" \
  bash -c '
    source "$1"
    mkdir -p "${CI_LOG_DIR}"
    run_step status-fixture bash -c "printf '\''status fixture logged\\n'\''; exit 37"
  ' bash "${TEST_SCRIPT_DIR}/../ci-local.sh" \
  >"${TIMEOUT_TEST_ROOT}/status.out" \
  2>"${TIMEOUT_TEST_ROOT}/status.err"
status_fixture=$?
set -e
if ((status_fixture != 37)); then
  printf 'failing step returned %d instead of original status 37\n' \
    "${status_fixture}" >&2
  cat "${TIMEOUT_TEST_ROOT}/status.err" >&2
  exit 1
fi
rg -F 'status fixture logged' \
  "${TIMEOUT_TEST_ROOT}/status-logs/status-fixture.log" >/dev/null
rg -F 'step status-fixture failed with status 37' \
  "${TIMEOUT_TEST_ROOT}/status.err" >/dev/null

FAILING_TEE_BIN="${TIMEOUT_TEST_ROOT}/failing-tee-bin"
readonly FAILING_TEE_BIN
mkdir -p "${FAILING_TEE_BIN}"
cat >"${FAILING_TEE_BIN}/tee" <<'EOF'
#!/usr/bin/env bash
cat >/dev/null
exit 73
EOF
chmod 755 "${FAILING_TEE_BIN}/tee"

set +e
# shellcheck disable=SC2016  # The inner shell expands its own CI_LOG_DIR.
env PATH="${FAILING_TEE_BIN}:${PATH}" \
  FE2O3_CI_STEP_TIMEOUT_SECONDS=5 \
  CI_LOG_DIR="${TIMEOUT_TEST_ROOT}/logger-logs" \
  bash -c '
    source "$1"
    mkdir -p "${CI_LOG_DIR}"
    run_step logger-fixture bash -c "printf '\''logger fixture output\\n'\''"
  ' bash "${TEST_SCRIPT_DIR}/../ci-local.sh" \
  >"${TIMEOUT_TEST_ROOT}/logger.out" \
  2>"${TIMEOUT_TEST_ROOT}/logger.err"
logger_status=$?
set -e
if ((logger_status != 73)); then
  printf 'logger failure returned %d instead of status 73\n' \
    "${logger_status}" >&2
  cat "${TIMEOUT_TEST_ROOT}/logger.err" >&2
  exit 1
fi
rg -F 'step logger-fixture log write failed with status 73' \
  "${TIMEOUT_TEST_ROOT}/logger.err" >/dev/null

set +e
# shellcheck disable=SC2016  # The inner shell expands its own CI_LOG_DIR.
env PATH="${FAILING_TEE_BIN}:${PATH}" \
  FE2O3_CI_STEP_TIMEOUT_SECONDS=5 \
  CI_LOG_DIR="${TIMEOUT_TEST_ROOT}/dual-failure-logs" \
  bash -c '
    source "$1"
    mkdir -p "${CI_LOG_DIR}"
    run_step dual-failure-fixture bash -c "printf '\''dual failure output\\n'\''; exit 37"
  ' bash "${TEST_SCRIPT_DIR}/../ci-local.sh" \
  >"${TIMEOUT_TEST_ROOT}/dual-failure.out" \
  2>"${TIMEOUT_TEST_ROOT}/dual-failure.err"
dual_failure_status=$?
set -e
if ((dual_failure_status != 37)); then
  printf 'combined command/logger failure returned %d instead of primary status 37\n' \
    "${dual_failure_status}" >&2
  cat "${TIMEOUT_TEST_ROOT}/dual-failure.err" >&2
  exit 1
fi
rg -F 'step dual-failure-fixture log write failed with status 73' \
  "${TIMEOUT_TEST_ROOT}/dual-failure.err" >/dev/null
rg -F 'step dual-failure-fixture failed with status 37' \
  "${TIMEOUT_TEST_ROOT}/dual-failure.err" >/dev/null

KILL_MARKER="${TIMEOUT_TEST_ROOT}/kill-pids"
readonly KILL_MARKER
cat >"${TIMEOUT_TEST_ROOT}/term-resistant.sh" <<'EOF'
#!/usr/bin/env bash
trap '' TERM
bash -c 'trap "" TERM; while :; do sleep 60; done' &
printf '%s %s\n' "$$" "$!" >"$1"
wait
EOF
chmod 755 "${TIMEOUT_TEST_ROOT}/term-resistant.sh"
set +e
# shellcheck disable=SC2016  # The inner shell expands its own CI_LOG_DIR and arguments.
timeout 10s env \
  FE2O3_CI_STEP_TIMEOUT_SECONDS=1 \
  FE2O3_CI_STEP_KILL_AFTER_SECONDS=1 \
  CI_LOG_DIR="${TIMEOUT_TEST_ROOT}/kill-logs" \
  bash -c '
    source "$1"
    mkdir -p "${CI_LOG_DIR}"
    run_step kill-fixture "$2" "$3"
  ' bash "${TEST_SCRIPT_DIR}/../ci-local.sh" \
  "${TIMEOUT_TEST_ROOT}/term-resistant.sh" "${KILL_MARKER}" \
  >"${TIMEOUT_TEST_ROOT}/kill.out" \
  2>"${TIMEOUT_TEST_ROOT}/kill.err"
kill_status=$?
set -e
if ((kill_status != 137)); then
  printf 'TERM-resistant step returned %d instead of SIGKILL status 137\n' \
    "${kill_status}" >&2
  cat "${TIMEOUT_TEST_ROOT}/kill.err" >&2
  exit 1
fi
read -r kill_parent kill_child <"${KILL_MARKER}"
for killed_pid in "${kill_parent}" "${kill_child}"; do
  if kill -0 "${killed_pid}" 2>/dev/null; then
    printf 'timed-out process %s survived TERM/KILL escalation\n' \
      "${killed_pid}" >&2
    exit 1
  fi
done

declare -a STEP_NAMES=()
declare -a STEP_COMMANDS=()

run_step() {
  local name="$1"
  local command
  shift
  printf -v command '%q ' "$@"
  STEP_NAMES+=("${name}")
  STEP_COMMANDS+=("${command% }")
}

load_example_packages() {
  local destination_name="$2"
  local -n destination="${destination_name}"
  destination=()
}

step_command() {
  local expected_name="$1"
  local index
  for index in "${!STEP_NAMES[@]}"; do
    if [[ "${STEP_NAMES[index]}" == "${expected_name}" ]]; then
      printf '%s\n' "${STEP_COMMANDS[index]}"
      return 0
    fi
  done
  printf 'missing recorded step: %s\n' "${expected_name}" >&2
  return 1
}

assert_equals() {
  local expected="$1"
  local actual="$2"
  local context="$3"
  if [[ "${actual}" != "${expected}" ]]; then
    printf '%s\nexpected: %s\nactual:   %s\n' \
      "${context}" "${expected}" "${actual}" >&2
    exit 1
  fi
}

run_tests
cpu_command="$(step_command cpu-tests)"
backend_command="$(step_command rustc-codegen-tests)"
if [[ " ${cpu_command} " == *" -p ${RUSTC_CODEGEN_TEST_PACKAGE} "* ]]; then
  printf 'generic CPU tests mixed %s into the shared Cargo process\n' \
    "${RUSTC_CODEGEN_TEST_PACKAGE}" >&2
  exit 1
fi
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --all-targets" \
  "${backend_command}" \
  'generic backend test command changed'

STEP_NAMES=()
STEP_COMMANDS=()
run_workspace_tests
assert_equals \
  "cargo test --locked --workspace --all-targets --exclude ${RUSTC_CODEGEN_TEST_PACKAGE}" \
  "$(step_command workspace-tests)" \
  'full workspace test command must exclude the backend'
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --all-targets" \
  "$(step_command rustc-codegen-tests)" \
  'full workspace backend test command changed'

STEP_NAMES=()
STEP_COMMANDS=()
main parity-evidence
assert_equals \
  "bash scripts/tests/parity-row-evidence.sh" \
  "$(step_command parity-row-evidence-tests)" \
  'parity evidence command did not dispatch the signed row suite'
assert_equals \
  "bash scripts/tests/mi300x-evidence-queue.sh" \
  "$(step_command mi300x-evidence-queue-tests)" \
  'parity evidence command did not dispatch the serialized queue suite'
assert_equals \
  "bash scripts/tests/hosted-parity-ci.sh" \
  "$(step_command hosted-parity-ci-tests)" \
  'parity evidence command did not dispatch the hosted trust-boundary suite'

printf '%s\n' 'ci-local test gate regression passed'
