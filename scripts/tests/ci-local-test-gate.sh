#!/usr/bin/env bash

set -Eeuo pipefail

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "${TEST_SCRIPT_DIR}/../ci-local.sh"

TIMEOUT_TEST_ROOT="$(mktemp -d)"
readonly TIMEOUT_TEST_ROOT
trap 'rm -rf "${TIMEOUT_TEST_ROOT}"' EXIT

bash "${TEST_SCRIPT_DIR}/rustc-codegen-shards.sh"
python3 "${TEST_SCRIPT_DIR}/bounded-moe-ci-dispatch.py"

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
declare -A STEP_TIMEOUT_OVERRIDES=()

run_step() {
  local name="$1"
  local command
  shift
  printf -v command '%q ' "$@"
  STEP_NAMES+=("${name}")
  STEP_COMMANDS+=("${command% }")
}

run_step_with_timeout() {
  local timeout_seconds="$1"
  local name="$2"
  shift 2
  STEP_TIMEOUT_OVERRIDES["${name}"]="${timeout_seconds}"
  run_step "${name}" "$@"
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

step_count() {
  local expected_name="$1"
  local count=0
  local name
  for name in "${STEP_NAMES[@]}"; do
    if [[ "${name}" == "${expected_name}" ]]; then
      count=$((count + 1))
    fi
  done
  printf '%d\n' "${count}"
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

assert_step_count() {
  local expected_name="$1"
  local expected_count="$2"
  local context="$3"
  assert_equals "${expected_count}" "$(step_count "${expected_name}")" "${context}"
}

assert_codegen_test_driver_once() {
  assert_step_count rustc-codegen-driver-bootstrap 1 \
    'codegen tests did not build the shared test driver exactly once'
  assert_equals \
    "env CARGO_BUILD_JOBS=1 CARGO_PROFILE_DEV_DEBUG=1 cargo build --locked -p ${RUSTC_CODEGEN_TEST_DRIVER_PACKAGE} --bin ${RUSTC_CODEGEN_TEST_DRIVER_PACKAGE}" \
    "$(step_command rustc-codegen-driver-bootstrap)" \
    'codegen test driver bootstrap is not bounded or production-profiled'
}

assert_all_codegen_targets_once() {
  local -a shard_ids test_targets
  local shard_id test_target
  local expected_total=0
  local actual_total=0
  load_rustc_codegen_shards shard_ids
  for shard_id in "${shard_ids[@]}"; do
    load_rustc_codegen_shard_targets "${shard_id}" test_targets
    for test_target in "${test_targets[@]}"; do
      expected_total=$((expected_total + 1))
      assert_step_count "rustc-codegen-test-${test_target}" 1 \
        "codegen target ${test_target} did not run exactly once"
    done
  done
  for test_target in "${STEP_NAMES[@]}"; do
    if [[ "${test_target}" == rustc-codegen-test-* ]]; then
      actual_total=$((actual_total + 1))
    fi
  done
  assert_equals "${expected_total}" "${actual_total}" \
    'codegen integration target count differs from the manifest'
}

run_tests
assert_codegen_test_driver_once
cpu_command="$(step_command cpu-tests)"
if [[ " ${cpu_command} " == *" -p ${RUSTC_CODEGEN_TEST_PACKAGE} "* ]]; then
  printf 'generic CPU tests mixed %s into the shared Cargo process\n' \
    "${RUSTC_CODEGEN_TEST_PACKAGE}" >&2
  exit 1
fi
assert_equals \
  "python3 ${RUSTC_CODEGEN_SHARD_POLICY} check" \
  "$(step_command rustc-codegen-shard-policy)" \
  'generic tests did not validate the codegen shard policy'
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --lib" \
  "$(step_command rustc-codegen-lib-tests)" \
  'generic backend library test command changed'
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --lib qualification_selection::tests" \
  "$(step_command rustc-codegen-qualification-route-tests)" \
  'generic backend qualification route test command changed'
assert_equals \
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test g2_layout" \
  "$(step_command rustc-codegen-test-g2_layout)" \
  'generic backend integration tests are not target-isolated'
assert_all_codegen_targets_once
assert_equals 4200 "${GENERAL_GEMM_SEMANTIC_FRONTEND_TIMEOUT_SECONDS}" \
  'general GEMM semantic frontend timeout policy changed without review'
assert_equals \
  "${GENERAL_GEMM_SEMANTIC_FRONTEND_TIMEOUT_SECONDS}" \
  "${STEP_TIMEOUT_OVERRIDES[rustc-codegen-test-general_gemm_semantic_frontend]:-}" \
  'general GEMM semantic frontend did not receive its reviewed timeout override'
assert_equals 1 "${#STEP_TIMEOUT_OVERRIDES[@]}" \
  'an unexpected codegen target received a timeout override'
for backend_command in "${STEP_COMMANDS[@]}"; do
  if [[ "${backend_command}" == *"-p ${RUSTC_CODEGEN_TEST_PACKAGE}"* ]] &&
    [[ "${backend_command}" == *"--all-targets"* ]]; then
    printf 'backend tests still use the ABI-unstable --all-targets build\n' >&2
    exit 1
  fi
done
codegen_integration_prefix="env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test "
for index in "${!STEP_NAMES[@]}"; do
  if [[ "${STEP_NAMES[index]}" == rustc-codegen-test-* ]] &&
    [[ "${STEP_COMMANDS[index]}" != "${codegen_integration_prefix}"* ]]; then
    printf 'backend integration target does not use the production limited-debug profile: %s\n' \
      "${STEP_NAMES[index]}" >&2
    exit 1
  fi
done

STEP_NAMES=()
STEP_COMMANDS=()
run_workspace_tests
assert_codegen_test_driver_once
assert_equals \
  "cargo test --locked --workspace --all-targets --exclude ${RUSTC_CODEGEN_TEST_PACKAGE}" \
  "$(step_command workspace-tests)" \
  'full workspace test command must exclude the backend'
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --lib" \
  "$(step_command rustc-codegen-lib-tests)" \
  'full workspace backend library test command changed'
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --lib qualification_selection::tests" \
  "$(step_command rustc-codegen-qualification-route-tests)" \
  'full workspace backend qualification route test command changed'
assert_equals \
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test g2_layout" \
  "$(step_command rustc-codegen-test-g2_layout)" \
  'full workspace backend integration tests are not target-isolated'
assert_all_codegen_targets_once

STEP_NAMES=()
STEP_COMMANDS=()
run_rustc_codegen_shard 01-control-flow
assert_codegen_test_driver_once
assert_equals \
  "python3 ${RUSTC_CODEGEN_SHARD_POLICY} check" \
  "$(step_command rustc-codegen-shard-policy)" \
  'codegen shard did not validate the checked-in assignment'
assert_equals \
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test collected_executable_scalar_control_flow_v2" \
  "$(step_command rustc-codegen-test-collected_executable_scalar_control_flow_v2)" \
  'codegen shard did not keep its target isolated'
assert_step_count rustc-codegen-lib-tests 0 \
  'integration shard unexpectedly reran backend library tests'
for shard_step in "${STEP_NAMES[@]}"; do
  if [[ "${shard_step}" == rustc-codegen-test-* ]] &&
    [[ "${shard_step}" != rustc-codegen-test-collected_executable_scalar_control_flow_v2 ]]; then
    printf 'codegen shard ran an unassigned target: %s\n' "${shard_step}" >&2
    exit 1
  fi
done

STEP_NAMES=()
STEP_COMMANDS=()
run_generic_core
for core_step in \
  workspace-dependency-policy-tests \
  workspace-dependency-policy \
  example-manifest \
  bounded-moe-docs \
  rustc-codegen-shard-policy \
  parity-matrix-check \
  parity-matrix-tests \
  parity-evidence-tests \
  parity-oci-executor-tests \
  parity-oci-operator-tests \
  authority-launcher-tests \
  rustc-trampoline-tests \
  cargo-binding-trampoline-tests \
  parity-row-evidence-tests \
  parity-publisher-client-tests \
  parity-signed-evidence-fd-tests \
  parity-repository-rules-tests \
  mi300x-evidence-queue-tests \
  hosted-parity-ci-tests \
  format \
  workspace-check \
  backend-build \
  ci-local-test-gate \
  cargo-fe2o3-worker-v3-envelope-tests \
  cpu-tests \
  rustc-codegen-lib-tests \
  core-doc-tests \
  device-copy-renamed-dependency \
  device-copy-derive-real-trait \
  device-copy-derive-ui \
  s09-debug-checker \
  s09-debug-ci-guard; do
  assert_step_count "${core_step}" 1 \
    "generic core did not run ${core_step} exactly once"
done
assert_equals \
  "env FE2O3_HIP_SYS_DISABLE=1 cargo test --locked -p cargo-fe2o3 --features ${CARGO_FE2O3_WORKER_V3_INTEGRATION_FEATURE} --test worker_v3_load_envelope_vertical" \
  "$(step_command cargo-fe2o3-worker-v3-envelope-tests)" \
  'generic core did not gate the strict Worker V3 envelope vertical suite'
workspace_check_command="$(step_command workspace-check)"
for wrapper_only_fixture in \
  fe2o3-production-extraction-fixture \
  fe2o3-production-ranked-bounds-fixture; do
  if [[ "${workspace_check_command}" != *"--exclude ${wrapper_only_fixture}"* ]]; then
    printf 'generic workspace check compiled wrapper-only fixture: %s\n' \
      "${wrapper_only_fixture}" >&2
    exit 1
  fi
done
assert_equals \
  "python3 ${WORKSPACE_DEPENDENCY_POLICY_TESTS}" \
  "$(step_command workspace-dependency-policy-tests)" \
  'generic core did not run workspace dependency policy tests'
assert_equals \
  "python3 ${WORKSPACE_DEPENDENCY_POLICY_CHECKER} --policy ${WORKSPACE_DEPENDENCY_POLICY}" \
  "$(step_command workspace-dependency-policy)" \
  'generic core did not enforce the workspace dependency policy'
for core_step in "${STEP_NAMES[@]}"; do
  if [[ "${core_step}" == rustc-codegen-test-* ]]; then
    printf 'generic core unexpectedly ran integration target: %s\n' "${core_step}" >&2
    exit 1
  fi
done
assert_step_count rustc-codegen-driver-bootstrap 0 \
  'generic core unexpectedly built the codegen integration test driver'

STEP_NAMES=()
STEP_COMMANDS=()
run_generic
assert_codegen_test_driver_once
assert_all_codegen_targets_once
assert_step_count rustc-codegen-shard-policy 1 \
  'serial generic gate did not run shard policy exactly once'
assert_step_count rustc-codegen-lib-tests 1 \
  'serial generic gate did not run backend library tests exactly once'

STEP_NAMES=()
STEP_COMMANDS=()
load_example_packages() {
  local destination_name="$2"
  local -n destination="${destination_name}"
  destination=(fe2o3-add-inplace)
}
run_rocm_compile
assert_equals \
  'cargo clean -p fe2o3-add-inplace' \
  "$(step_command rocm-clean-fe2o3-add-inplace)" \
  'ROCm compile did not invalidate the example host fingerprint'
assert_equals \
  'cargo run --locked -p cargo-fe2o3 -- build -p fe2o3-add-inplace' \
  "$(step_command rocm-build-fe2o3-add-inplace)" \
  'ROCm compile example build command changed'
assert_equals \
  'cargo run --quiet --locked -p cargo-fe2o3 -- examples check-artifacts fe2o3-add-inplace' \
  "$(step_command rocm-artifacts-fe2o3-add-inplace)" \
  'ROCm compile example artifact check changed'
assert_equals \
  'rocm-clean-fe2o3-add-inplace rocm-build-fe2o3-add-inplace rocm-artifacts-fe2o3-add-inplace' \
  "$(printf '%s\n' "${STEP_NAMES[@]}" | rg '^rocm-(clean|build|artifacts)-fe2o3-add-inplace$' | paste -sd ' ' -)" \
  'ROCm compile did not clean, build, and inspect the example in order'

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
