#!/usr/bin/env bash

set -Eeuo pipefail

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
source "${TEST_SCRIPT_DIR}/../ci-local.sh"

TIMEOUT_TEST_ROOT="$(mktemp -d)"
readonly TIMEOUT_TEST_ROOT
cleanup_timeout_test_root() {
  chmod -R u+w -- "${TIMEOUT_TEST_ROOT}" 2>/dev/null || true
  rm -rf -- "${TIMEOUT_TEST_ROOT}"
}
trap cleanup_timeout_test_root EXIT

bash "${TEST_SCRIPT_DIR}/rustc-codegen-shards.sh"
python3 "${TEST_SCRIPT_DIR}/bounded-moe-ci-dispatch.py"

OWNED_TMP_TARGET="${TIMEOUT_TEST_ROOT}/owned-tmp-target"
mkdir -m 700 -- "${OWNED_TMP_TARGET}"
OWNED_TMP_PATH="$(
  env -u TMPDIR \
    CI_LOG_DIR="${TIMEOUT_TEST_ROOT}/owned-tmp-logs" \
    bash -c '
      source "$1"
      CARGO_TARGET_DIRECTORY="$2"
      prepare_private_tmp_root
      printf "%s\n" "${TMPDIR}"
    ' bash "${TEST_SCRIPT_DIR}/../ci-local.sh" "${OWNED_TMP_TARGET}"
)"
[[ "${OWNED_TMP_PATH}" == "${OWNED_TMP_TARGET}/fe2o3-ci-tmp-"* ]] || {
  printf 'ci-local did not create its temporary root under the admitted target: %s\n' \
    "${OWNED_TMP_PATH}" >&2
  exit 1
}
[[ ! -e "${OWNED_TMP_PATH}" && ! -L "${OWNED_TMP_PATH}" ]] || {
  printf 'ci-local did not trap-clean its owned temporary root: %s\n' \
    "${OWNED_TMP_PATH}" >&2
  exit 1
}

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

prepare_cargo_fe2o3_driver() {
  local step_prefix="$1"
  local driver_profile="$2"
  local root="${TIMEOUT_TEST_ROOT}/${step_prefix}-driver"
  local -a feature_args=()
  case "${driver_profile}" in
    production) ;;
    qualification)
      feature_args=(--features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}")
      ;;
    *)
      printf 'mock received unknown driver profile: %s\n' \
        "${driver_profile}" >&2
      return 2
      ;;
  esac
  if [[ ! -d "${root}" ]]; then
    mkdir -m 700 -- "${root}"
    printf '#!/usr/bin/env bash\nexit 0\n' >"${root}/cargo-fe2o3"
    chmod 500 -- "${root}/cargo-fe2o3" "${root}"
  fi
  CARGO_FE2O3_DRIVER_ROOT="${root}"
  CARGO_FE2O3_BINARY="${root}/cargo-fe2o3"
  CARGO_FE2O3_SHA256="$(sha256sum -- "${CARGO_FE2O3_BINARY}")"
  CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256%% *}"
  run_step "${step_prefix}-cargo-fe2o3-bootstrap" \
    cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 \
    "${feature_args[@]}" \
    --message-format=json-render-diagnostics
}

load_example_packages() {
  local lane="$1"
  local destination_name="$2"
  local -n destination="${destination_name}"
  case "${lane}" in
    all)
      destination=(fe2o3-disabled-fixture fe2o3-managed-a fe2o3-managed-b fe2o3-ordinary)
      ;;
    rustc-check)
      destination=(fe2o3-managed-a fe2o3-managed-b fe2o3-ordinary)
      ;;
    wrapper-managed)
      destination=(fe2o3-managed-a fe2o3-managed-b)
      ;;
    *)
      destination=()
      ;;
  esac
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
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --lib" \
  "$(step_command rustc-codegen-lib-tests)" \
  'generic backend library tests are not using bounded debug information'
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
assert_equals 4200 "${COLLECTED_CONTROL_FLOW_TIMEOUT_SECONDS}" \
  'collected control-flow timeout policy changed without review'
assert_equals \
  "${GENERAL_GEMM_SEMANTIC_FRONTEND_TIMEOUT_SECONDS}" \
  "${STEP_TIMEOUT_OVERRIDES[rustc-codegen-test-general_gemm_semantic_frontend]:-}" \
  'general GEMM semantic frontend did not receive its reviewed timeout override'
assert_equals \
  "${COLLECTED_CONTROL_FLOW_TIMEOUT_SECONDS}" \
  "${STEP_TIMEOUT_OVERRIDES[rustc-codegen-test-collected_executable_scalar_control_flow_v2]:-}" \
  'collected control-flow target did not receive its reviewed timeout override'
assert_equals 2 "${#STEP_TIMEOUT_OVERRIDES[@]}" \
  'an unexpected codegen target received a timeout override'
for backend_command in "${STEP_COMMANDS[@]}"; do
  if [[ "${backend_command}" == *"-p ${RUSTC_CODEGEN_TEST_PACKAGE}"* ]] &&
    [[ "${backend_command}" == *"--all-targets"* ]]; then
    printf 'backend tests still use the ABI-unstable --all-targets build\n' >&2
    exit 1
  fi
done
codegen_integration_prefix="env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test "
serialized_codegen_targets=0
for index in "${!STEP_NAMES[@]}"; do
  if [[ "${STEP_NAMES[index]}" == rustc-codegen-test-* ]] &&
    [[ "${STEP_COMMANDS[index]}" != "${codegen_integration_prefix}"* ]]; then
    printf 'backend integration target does not use the production limited-debug profile: %s\n' \
      "${STEP_NAMES[index]}" >&2
    exit 1
  fi
  if [[ "${STEP_COMMANDS[index]}" == *"-- --test-threads="* ]]; then
    serialized_codegen_targets=$((serialized_codegen_targets + 1))
    assert_equals rustc-codegen-test-collected_executable_scalar_control_flow_v2 \
      "${STEP_NAMES[index]}" \
      'an unrelated codegen target received the serialization policy'
    assert_equals \
      "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test collected_executable_scalar_control_flow_v2 -- --test-threads=1" \
      "${STEP_COMMANDS[index]}" \
      'the heavy control-flow target did not use the reviewed thread bound'
  fi
done
assert_equals 1 "${serialized_codegen_targets}" \
  'the codegen serialization policy must select exactly one target'

STEP_NAMES=()
STEP_COMMANDS=()
run_workspace_tests
assert_codegen_test_driver_once
assert_equals \
  "cargo test --locked --workspace --all-targets --exclude ${RUSTC_CODEGEN_TEST_PACKAGE}" \
  "$(step_command workspace-tests)" \
  'full workspace test command must exclude the backend'
assert_equals \
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --lib" \
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
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test collected_executable_scalar_control_flow_v2 -- --test-threads=1" \
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
  generic-check-cargo-fe2o3-bootstrap \
  workspace-binding-check \
  workspace-binding-check-boundary \
  workspace-binding-projection-revalidation \
  backend-build \
  ci-local-test-gate \
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
  "python3 ${WORKSPACE_DEPENDENCY_POLICY_TESTS}" \
  "$(step_command workspace-dependency-policy-tests)" \
  'generic core did not run workspace dependency policy tests'
assert_equals \
  "python3 ${WORKSPACE_DEPENDENCY_POLICY_CHECKER} --policy ${WORKSPACE_DEPENDENCY_POLICY}" \
  "$(step_command workspace-dependency-policy)" \
  'generic core did not enforce the workspace dependency policy'
assert_step_count workspace-check 0 \
  'generic check retained the unsound raw/wrapper split'
assert_equals \
  'cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 --message-format=json-render-diagnostics' \
  "$(step_command generic-check-cargo-fe2o3-bootstrap)" \
  'generic check did not retain the feature-free production driver'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/generic-check-driver/cargo-fe2o3 check --workspace --all-targets --locked --exclude fe2o3-production-extraction-fixture --exclude fe2o3-production-ranked-bounds-fixture --exclude fe2o3-disabled-fixture" \
  "$(step_command workspace-binding-check)" \
  'managed check did not cover the whole supported workspace graph'
assert_equals \
  "bash scripts/tests/binding-check-boundary.sh ${TIMEOUT_TEST_ROOT}/generic-check-driver/cargo-fe2o3 fe2o3-managed-a" \
  "$(step_command workspace-binding-check-boundary)" \
  'managed check omitted the backend/artifact/publication hostile boundary'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/generic-check-driver/cargo-fe2o3 examples check-wrapper-managed fe2o3-managed-a fe2o3-managed-b" \
  "$(step_command workspace-binding-projection-revalidation)" \
  'managed check did not revalidate the exact structural package projection'
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
export LD_FE2O3_CI_TEST=injected
export DYLD_FE2O3_CI_TEST=injected
export GLIBC_TUNABLES=glibc.malloc.check=1
export FE2O3_CI_TEST_PRESERVED=present
declare -a loader_environment_removals=()
load_dynamic_loader_environment_removals loader_environment_removals
loader_environment_removal_text="$(printf '%s\n' "${loader_environment_removals[@]}")"
for loader_name in LD_FE2O3_CI_TEST DYLD_FE2O3_CI_TEST GLIBC_TUNABLES; do
  rg -Fx -- "${loader_name}" <<<"${loader_environment_removal_text}" >/dev/null || {
    printf 'ROCm compile loader scrub omitted %s\n' "${loader_name}" >&2
    exit 1
  }
done
if rg -Fx -- FE2O3_CI_TEST_PRESERVED \
  <<<"${loader_environment_removal_text}" >/dev/null; then
  printf '%s\n' 'ROCm compile loader scrub removed an unrelated variable' >&2
  exit 1
fi
DRIVER_IDENTITY_ROOT="${TIMEOUT_TEST_ROOT}/driver-identity"
mkdir -m 700 -- "${DRIVER_IDENTITY_ROOT}"
printf '#!/usr/bin/env bash\nenv\n' >"${DRIVER_IDENTITY_ROOT}/cargo-fe2o3"
chmod 500 -- "${DRIVER_IDENTITY_ROOT}/cargo-fe2o3" "${DRIVER_IDENTITY_ROOT}"
CARGO_FE2O3_DRIVER_ROOT="${DRIVER_IDENTITY_ROOT}"
CARGO_FE2O3_BINARY="${DRIVER_IDENTITY_ROOT}/cargo-fe2o3"
CARGO_FE2O3_SHA256="$(sha256sum -- "${CARGO_FE2O3_BINARY}")"
CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256%% *}"
validate_cargo_fe2o3_driver
driver_environment="$(
  env "${loader_environment_removals[@]}" \
    "${CARGO_FE2O3_BINARY}"
)"
for loader_name in LD_FE2O3_CI_TEST DYLD_FE2O3_CI_TEST GLIBC_TUNABLES; do
  if rg -q "^${loader_name}=" <<<"${driver_environment}"; then
    printf 'direct qualification driver retained %s\n' "${loader_name}" >&2
    exit 1
  fi
done
rg -Fx 'FE2O3_CI_TEST_PRESERVED=present' <<<"${driver_environment}" >/dev/null || {
  printf '%s\n' 'direct qualification driver lost an unrelated variable' >&2
  exit 1
}
PUBLIC_ROOT="${TIMEOUT_TEST_ROOT}/public-root"
mkdir -m 755 -- "${PUBLIC_ROOT}"
if validate_private_directory 'hostile public root' "${PUBLIC_ROOT}" 2>/dev/null; then
  printf '%s\n' 'private directory admission accepted group/world access' >&2
  exit 1
fi
CAPTURE_TARGET="${TIMEOUT_TEST_ROOT}/capture-target"
mkdir -m 700 -- "${CAPTURE_TARGET}"
CAPTURE_BINARY="${CAPTURE_TARGET}/cargo-fe2o3"
printf '#!/usr/bin/env bash\nexit 0\n' >"${CAPTURE_BINARY}"
chmod 700 -- "${CAPTURE_BINARY}"
CAPTURE_RECEIPT="${TIMEOUT_TEST_ROOT}/capture.json"
CAPTURE_PACKAGE='path+file:///workspace/crates/cargo-fe2o3#0.1.0'
CAPTURE_SOURCE='/workspace/crates/cargo-fe2o3/src/main.rs'
python3 - "${CAPTURE_RECEIPT}" "${CAPTURE_PACKAGE}" \
  "${CAPTURE_SOURCE}" "${CAPTURE_BINARY}" <<'PY'
import json
import sys

receipt, package, source, executable = sys.argv[1:]
record = {
    "reason": "compiler-artifact",
    "package_id": package,
    "target": {
        "name": "cargo-fe2o3",
        "kind": ["bin"],
        "crate_types": ["bin"],
        "src_path": source,
    },
    "profile": {"test": False, "opt_level": "0"},
    "executable": executable,
}
with open(receipt, "w", encoding="utf-8") as output:
    print(json.dumps(record), file=output)
PY
assert_equals \
  "${CAPTURE_BINARY}" \
  "$(resolve_cargo_fe2o3_artifact "${CAPTURE_RECEIPT}" \
    "${CAPTURE_PACKAGE}" "${CAPTURE_SOURCE}" "${CAPTURE_TARGET}")" \
  'exact Cargo JSON driver receipt was not admitted'
for mutation in package source profile opt-level kind crate-type containment duplicate; do
  python3 - "${CAPTURE_RECEIPT}" "${mutation}" \
    "${CAPTURE_PACKAGE}" "${CAPTURE_SOURCE}" "${CAPTURE_BINARY}" <<'PY'
import json
import sys

receipt, mutation, package, source, executable = sys.argv[1:]
record = {
    "reason": "compiler-artifact",
    "package_id": package,
    "target": {
        "name": "cargo-fe2o3",
        "kind": ["bin"],
        "crate_types": ["bin"],
        "src_path": source,
    },
    "profile": {"test": False, "opt_level": "0"},
    "executable": executable,
}
if mutation == "package":
    record["package_id"] = "hostile#0.1.0"
elif mutation == "source":
    record["target"]["src_path"] = "/hostile/main.rs"
elif mutation == "profile":
    record["profile"]["test"] = True
elif mutation == "opt-level":
    record["profile"]["opt_level"] = "3"
elif mutation == "kind":
    record["target"]["kind"] = ["lib"]
elif mutation == "crate-type":
    record["target"]["crate_types"] = ["lib"]
elif mutation == "containment":
    record["executable"] = "/bin/true"
with open(receipt, "w", encoding="utf-8") as output:
    print(json.dumps(record), file=output)
    if mutation == "duplicate":
        print(json.dumps(record), file=output)
PY
  if resolve_cargo_fe2o3_artifact "${CAPTURE_RECEIPT}" \
    "${CAPTURE_PACKAGE}" "${CAPTURE_SOURCE}" "${CAPTURE_TARGET}" \
    >/dev/null 2>&1; then
    printf 'Cargo JSON resolver accepted hostile %s substitution\n' "${mutation}" >&2
    exit 1
  fi
done
chmod 700 -- "${CARGO_FE2O3_BINARY}"
if validate_cargo_fe2o3_driver 2>/dev/null; then
  printf '%s\n' 'qualification driver validator accepted changed mode' >&2
  exit 1
fi
chmod 500 -- "${CARGO_FE2O3_BINARY}"
chmod 700 -- "${CARGO_FE2O3_BINARY}"
printf '# hostile replacement\n' >>"${CARGO_FE2O3_BINARY}"
chmod 500 -- "${CARGO_FE2O3_BINARY}"
if validate_cargo_fe2o3_driver 2>/dev/null; then
  printf '%s\n' 'qualification driver validator accepted changed content' >&2
  exit 1
fi
unset LD_FE2O3_CI_TEST DYLD_FE2O3_CI_TEST GLIBC_TUNABLES FE2O3_CI_TEST_PRESERVED

prepare_cargo_fe2o3_driver() {
  local step_prefix="$1"
  local driver_profile="$2"
  local root="${TIMEOUT_TEST_ROOT}/${step_prefix}-driver"
  local -a feature_args=()
  case "${driver_profile}" in
    production) ;;
    qualification)
      feature_args=(--features "${RUSTC_CODEGEN_QUALIFICATION_FEATURE}")
      ;;
    *)
      printf 'mock received unknown driver profile: %s\n' \
        "${driver_profile}" >&2
      return 2
      ;;
  esac
  mkdir -p -- "${TIMEOUT_TEST_ROOT}/external-target"
  chmod 700 -- "${TIMEOUT_TEST_ROOT}/external-target"
  mkdir -m 700 -- "${root}"
  printf '#!/usr/bin/env bash\nexit 0\n' >"${root}/cargo-fe2o3"
  chmod 500 -- "${root}/cargo-fe2o3" "${root}"
  CARGO_TARGET_DIRECTORY="${TIMEOUT_TEST_ROOT}/external-target"
  CARGO_FE2O3_DRIVER_ROOT="${root}"
  CARGO_FE2O3_BINARY="${root}/cargo-fe2o3"
  CARGO_FE2O3_SHA256="$(sha256sum -- "${CARGO_FE2O3_BINARY}")"
  CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256%% *}"
  run_step "${step_prefix}-cargo-fe2o3-bootstrap" \
    cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 \
    "${feature_args[@]}" \
    --message-format=json-render-diagnostics
}
load_example_packages() {
  local destination_name="$2"
  local -n destination="${destination_name}"
  destination=(fe2o3-fill)
}
run_rocm_compile
assert_equals \
  'cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 --features qualification-oracles-test-only --message-format=json-render-diagnostics' \
  "$(step_command rocm-cargo-fe2o3-bootstrap)" \
  'ROCm compile did not build the qualification-enabled shared driver once'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/rocm-driver/cargo-fe2o3 doctor" \
  "$(step_command rocm-doctor)" \
  'ROCm compile did not invoke the resolved driver directly for doctor'
assert_equals \
  "env FE2O3_TEST_CARGO_FE2O3_BIN=${TIMEOUT_TEST_ROOT}/rocm-driver/cargo-fe2o3 FE2O3_TEST_CARGO_FE2O3_SHA256=${CARGO_FE2O3_SHA256} cargo test --locked -p rustc-codegen-fe2o3 --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test trusted_device_items genuine_markers_emit_and_local_external_spoofs_fail_closed -- --ignored --exact" \
  "$(step_command rocm-trusted-device-items)" \
  'ROCm compile did not provide the direct driver contract to trusted-device tests'
assert_equals \
  "env FE2O3_TEST_CARGO_FE2O3_BIN=${TIMEOUT_TEST_ROOT}/rocm-driver/cargo-fe2o3 FE2O3_TEST_CARGO_FE2O3_SHA256=${CARGO_FE2O3_SHA256} cargo test --locked -p rustc-codegen-fe2o3 --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test kernel_ir_codegen selected_pipeline_rejects_invalid_or_unsupported_inputs_and_cleans_stale_artifacts -- --ignored --exact" \
  "$(step_command rocm-kernel-ir-codegen-rejection)" \
  'ROCm compile did not provide the direct driver contract to kernel-IR tests'
assert_equals \
  'cargo clean -p fe2o3-fill' \
  "$(step_command rocm-clean-fe2o3-fill)" \
  'ROCm compile did not invalidate the example host fingerprint'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/rocm-driver/cargo-fe2o3 build -p fe2o3-fill" \
  "$(step_command rocm-build-fe2o3-fill)" \
  'ROCm compile did not invoke the resolved driver directly for example build'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/rocm-driver/cargo-fe2o3 examples check-artifacts fe2o3-fill" \
  "$(step_command rocm-artifacts-fe2o3-fill)" \
  'ROCm compile did not invoke the resolved driver directly for artifact inspection'
assert_equals \
  'rocm-clean-fe2o3-fill rocm-build-fe2o3-fill rocm-artifacts-fe2o3-fill' \
  "$(printf '%s\n' "${STEP_NAMES[@]}" | rg '^rocm-(clean|build|artifacts)-fe2o3-fill$' | paste -sd ' ' -)" \
  'ROCm compile did not clean, build, and inspect the example in order'

STEP_NAMES=()
STEP_COMMANDS=()
require_gpu_access() {
  return 0
}
resolve_cargo_target_directory() {
  printf '%s\n' "${TIMEOUT_TEST_ROOT}/external-target"
}
export FE2O3_ALLOW_GPU_SMOKE=1
export FE2O3_TARGET=gfx942:xnack-
run_hardware_smoke
assert_equals \
  'cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 --features qualification-oracles-test-only --message-format=json-render-diagnostics' \
  "$(step_command hardware-cargo-fe2o3-bootstrap)" \
  'hardware smoke did not build the qualification-enabled driver'
assert_equals \
  "cc -std=c11 -Wall -Wextra -Werror -D__HIP_PLATFORM_AMD__ -I /opt/rocm/include -I ${REPO_ROOT}/crates/fe2o3-hip-sys/native ${REPO_ROOT}/crates/fe2o3-hip-sys/native/device_properties_test.c -L /opt/rocm/lib -Wl\,-rpath\,/opt/rocm/lib -lamdhip64 -o ${TIMEOUT_TEST_ROOT}/external-target/ci-native/fe2o3-hip-device-properties-test" \
  "$(step_command hardware-hip-device-properties-build)" \
  'hardware smoke native helper did not use the resolved external target directory'
assert_equals \
  "env FE2O3_TEST_CARGO_FE2O3_BIN=${TIMEOUT_TEST_ROOT}/hardware-driver/cargo-fe2o3 FE2O3_TEST_CARGO_FE2O3_SHA256=${CARGO_FE2O3_SHA256} cargo test --locked -p rustc-codegen-fe2o3 --features ${RUSTC_CODEGEN_QUALIFICATION_FEATURE} --test kernel_ir_codegen opt_in_vecadd_publishes_exact_g1_and_executes_on_the_gpu -- --ignored --exact" \
  "$(step_command hardware-kernel-ir-vecadd)" \
  'hardware smoke did not provide the direct driver contract to nested tests'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/hardware-driver/cargo-fe2o3 smoke" \
  "$(step_command hardware-smoke)" \
  'hardware smoke did not invoke the qualification driver directly'
unset FE2O3_ALLOW_GPU_SMOKE FE2O3_TARGET

STEP_NAMES=()
STEP_COMMANDS=()
export FE2O3_S09_EVIDENCE_DIR="${TIMEOUT_TEST_ROOT}/s09-evidence"
run_s09_debug_hardware
assert_equals \
  'cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 --features qualification-oracles-test-only --message-format=json-render-diagnostics' \
  "$(step_command s09-cargo-fe2o3-bootstrap)" \
  'S09 did not build the qualification-enabled driver'
assert_equals \
  "env FE2O3_TEST_CARGO_FE2O3_BIN=${TIMEOUT_TEST_ROOT}/s09-driver/cargo-fe2o3 FE2O3_TEST_CARGO_FE2O3_SHA256=${CARGO_FE2O3_SHA256} bash scripts/s09-debug-ci.sh ${TIMEOUT_TEST_ROOT}/s09-evidence" \
  "$(step_command s09-debug-hardware)" \
  'S09 hardware did not provide the direct driver contract'
unset FE2O3_S09_EVIDENCE_DIR

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
