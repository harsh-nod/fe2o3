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
EMPTY_WRAPPER_CPU_INTERSECTION=0

run_step() {
  local name="$1"
  local command
  shift
  printf -v command '%q ' "$@"
  if [[ " ${command} " == *" FE2O3_QUALIFICATION_ORACLE_V1="* ]] &&
    { [[ " ${command} " == *"/cargo-fe2o3 build "* ]] ||
      [[ " ${command} " == *"/cargo-fe2o3 run "* ]]; }; then
    printf 'captured lane restored obsolete cargo-fe2o3 qualification: %s\n' \
      "${name}" >&2
    exit 1
  fi
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
  local root
  local -a feature_args=()
  case "${driver_profile}" in
    production)
      # Production bytes are content-identical across callers and therefore
      # share one immutable private root, like the real digest-addressed driver.
      root="${TIMEOUT_TEST_ROOT}/production-driver"
      ;;
    *)
      printf 'mock received unknown driver profile: %s\n' \
        "${driver_profile}" >&2
      return 2
      ;;
  esac
  [[ ! -e "${root}" && ! -L "${root}" ]] || {
    printf 'mock refused duplicate private driver root: %s\n' "${root}" >&2
    return 2
  }
  mkdir -m 700 -- "${root}"
  printf '#!/usr/bin/env bash\nexit 0\n' >"${root}/cargo-fe2o3"
  chmod 500 -- "${root}/cargo-fe2o3" "${root}"
  CARGO_FE2O3_DRIVER_ROOT="${root}"
  CARGO_FE2O3_BINARY="${root}/cargo-fe2o3"
  CARGO_FE2O3_SHA256="$(sha256sum -- "${CARGO_FE2O3_BINARY}")"
  CARGO_FE2O3_SHA256="${CARGO_FE2O3_SHA256%% *}"
  run_step "${step_prefix}-cargo-fe2o3-bootstrap" \
    cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 \
    "${feature_args[@]}" \
    --message-format=json-render-diagnostics
  CARGO_FE2O3_DRIVER_PROFILE="${driver_profile}"
}

reset_mock_production_driver() {
  local root="${TIMEOUT_TEST_ROOT}/production-driver"
  if [[ -d "${root}" && ! -L "${root}" ]]; then
    chmod 700 -- "${root}"
    rm -rf -- "${root}"
  fi
  if [[ "${CARGO_FE2O3_DRIVER_ROOT}" == "${root}" ]]; then
    CARGO_FE2O3_BINARY=
    CARGO_FE2O3_SHA256=
    CARGO_FE2O3_DRIVER_ROOT=
    CARGO_FE2O3_DRIVER_PROFILE=
  fi
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
    cpu-test-raw)
      destination=(fe2o3-ordinary)
      ;;
    cpu-test-wrapper-managed)
      if ((EMPTY_WRAPPER_CPU_INTERSECTION)); then
        destination=()
      else
        destination=(fe2o3-managed-a fe2o3-managed-b)
      fi
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

assert_no_codegen_test_driver() {
  assert_step_count rustc-codegen-driver-bootstrap 0 \
    'selector-free codegen tests unexpectedly built a shared driver'
}

assert_source_isa_unit_matrix_gate() {
  local omitted name
  local -a required_environment=(
    FE2O3_TEST_CARGO_FE2O3_BIN
    FE2O3_TEST_CARGO_FE2O3_SHA256
    FE2O3_PRODUCTION_BUILD_CONFIG_V2
    FE2O3_AUTHORITY_BACKEND_SHA256_V1
    FE2O3_AUTHORITY_CARGO_SHA256_V1
    FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_PATH_V1
    FE2O3_AUTHORITY_CARGO_BINDING_TRAMPOLINE_SHA256_V1
    FE2O3_AUTHORITY_RUSTC_PATH_V1
    FE2O3_AUTHORITY_RUSTC_RUNTIME_SHA256_V1
    FE2O3_AUTHORITY_RUSTC_SHA256_V1
    FE2O3_BACKEND
  )

  if (unset FE2O3_RUN_SOURCE_ISA_UNIT_MATRIX; run_source_isa_unit_matrix) \
    >/dev/null 2>&1; then
    printf '%s\n' 'source/ISA unit matrix ran without its explicit opt-in' >&2
    exit 1
  fi
  export FE2O3_RUN_SOURCE_ISA_UNIT_MATRIX=1
  for name in "${required_environment[@]}"; do
    printf -v "${name}" '%s' fixture
    export "${name}"
  done
  if (
    uname() {
      case "$1" in
        -s) printf '%s\n' Darwin ;;
        -m) printf '%s\n' x86_64 ;;
        *) return 2 ;;
      esac
    }
    run_source_isa_unit_matrix
  ) >/dev/null 2>&1; then
    printf '%s\n' 'source/ISA unit matrix ran on a non-Linux host' >&2
    exit 1
  fi
  if (
    uname() {
      case "$1" in
        -s) printf '%s\n' Linux ;;
        -m) printf '%s\n' aarch64 ;;
        *) return 2 ;;
      esac
    }
    run_source_isa_unit_matrix
  ) >/dev/null 2>&1; then
    printf '%s\n' 'source/ISA unit matrix ran on a non-x86_64 host' >&2
    exit 1
  fi
  for omitted in "${required_environment[@]}"; do
    if (unset "${omitted}"; run_source_isa_unit_matrix) >/dev/null 2>&1; then
      printf 'source/ISA unit matrix ran without %s\n' "${omitted}" >&2
      exit 1
    fi
  done

  STEP_NAMES=()
  STEP_COMMANDS=()
  run_source_isa_unit_matrix
  assert_equals \
    'cargo test --locked -p cargo-fe2o3 --bin cargo-fe2o3 production_source_isa_unit_matrix_v1::ordinary_source_units_round_trip_through_the_production_observer_on_both_targets -- --ignored --exact --test-threads=1 --nocapture' \
    "$(step_command source-isa-unit-matrix)" \
    'protected source/ISA unit matrix did not retain its exact serial ignored-test command'
  unset FE2O3_RUN_SOURCE_ISA_UNIT_MATRIX
  for name in "${required_environment[@]}"; do
    unset "${name}"
  done
  STEP_NAMES=()
  STEP_COMMANDS=()
}

codegen_target_prefix() {
  printf 'env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p %s --test ' \
    "${RUSTC_CODEGEN_TEST_PACKAGE}"
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
      [[ "$(step_command "rustc-codegen-test-${test_target}")" == \
        "$(codegen_target_prefix)${test_target}"* ]] || {
        printf 'codegen target %s did not use the selector-free production command\n' \
          "${test_target}" >&2
        exit 1
      }
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

assert_source_isa_unit_matrix_gate
run_tests
assert_no_codegen_test_driver
assert_equals \
  'cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 --message-format=json-render-diagnostics' \
  "$(step_command cpu-tests-cargo-fe2o3-bootstrap)" \
  'CPU tests did not retain the feature-free production driver'
cpu_command="$(step_command cpu-tests)"
if [[ " ${cpu_command} " == *" -p fe2o3-artifact-transaction "* ]]; then
  printf '%s\n' \
    'raw CPU aggregate duplicated the bounded artifact-transaction suite' >&2
  exit 1
fi
if [[ " ${cpu_command} " == *" -p ${RUSTC_CODEGEN_TEST_PACKAGE} "* ]]; then
  printf 'generic CPU tests mixed %s into the shared Cargo process\n' \
    "${RUSTC_CODEGEN_TEST_PACKAGE}" >&2
  exit 1
fi
for managed_package in fe2o3-managed-a fe2o3-managed-b; do
  if [[ " ${cpu_command} " == *" -p ${managed_package} "* ]]; then
    printf 'raw CPU tests included wrapper-managed package %s\n' \
      "${managed_package}" >&2
    exit 1
  fi
done
[[ " ${cpu_command} " == *" -p fe2o3-ordinary "* ]] || {
  printf '%s\n' 'raw CPU tests omitted the computed ordinary example package' >&2
  exit 1
}
if [[ " ${cpu_command} " == *" -p fe2o3-pliron-scalar-add-v1 "* ]]; then
  printf '%s\n' 'raw CPU tests restored the deleted scalar runtime lane' >&2
  exit 1
fi
assert_equals \
  'env FE2O3_HIP_SYS_DISABLE=1 RUST_TEST_THREADS=8 cargo test --locked -p fe2o3-artifact-transaction' \
  "$(step_command fe2o3-artifact-transaction-tests)" \
  'artifact-transaction tests did not retain their descriptor-safe fanout bound'
assert_step_count fe2o3-artifact-transaction-tests 1 \
  'artifact-transaction tests did not run exactly once'
assert_equals \
  "env FE2O3_HIP_SYS_DISABLE=1 ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 test --locked --all-targets -p fe2o3-managed-a -p fe2o3-managed-b" \
  "$(step_command wrapper-managed-cpu-tests)" \
  'managed CPU tests did not use the feature-free binding wrapper projection'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-cpu-test-partition fe2o3-ordinary -- fe2o3-managed-a fe2o3-managed-b" \
  "$(step_command cpu-test-partition-revalidation)" \
  'managed CPU tests did not revalidate both complete package lists'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-wrapper-managed fe2o3-managed-a fe2o3-managed-b" \
  "$(step_command cpu-test-binding-projection-revalidation)" \
  'managed CPU tests did not revalidate the complete structural projection'
assert_equals \
  "python3 ${RUSTC_CODEGEN_SHARD_POLICY} check" \
  "$(step_command rustc-codegen-shard-policy)" \
  'generic tests did not validate the codegen shard policy'
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --lib" \
  "$(step_command rustc-codegen-lib-tests)" \
  'generic backend library test command changed'
assert_equals \
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --test g2_layout" \
  "$(step_command rustc-codegen-test-g2_layout)" \
  'generic backend integration tests are not target-isolated'
assert_all_codegen_targets_once
assert_equals 0 "${#STEP_TIMEOUT_OVERRIDES[@]}" \
  'retired backend targets left a timeout override'
for backend_command in "${STEP_COMMANDS[@]}"; do
  if [[ "${backend_command}" == *"-p ${RUSTC_CODEGEN_TEST_PACKAGE}"* ]] &&
    [[ "${backend_command}" == *"--all-targets"* ]]; then
    printf 'backend tests still use the ABI-unstable --all-targets build\n' >&2
    exit 1
  fi
done
codegen_integration_prefix="env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --test "
for index in "${!STEP_NAMES[@]}"; do
  if [[ "${STEP_NAMES[index]}" == rustc-codegen-test-* ]] &&
    [[ "${STEP_COMMANDS[index]}" != "${codegen_integration_prefix}"* ]]; then
    printf 'backend integration target does not use the production limited-debug profile: %s\n' \
      "${STEP_NAMES[index]}" >&2
    exit 1
  fi
  if [[ "${STEP_NAMES[index]}" == rustc-codegen-test-* ]] &&
    [[ " ${STEP_COMMANDS[index]} " == *" --features "* ]]; then
    printf 'production integration target enabled a non-production feature: %s\n' \
      "${STEP_NAMES[index]}" >&2
    exit 1
  fi
done

STEP_NAMES=()
STEP_COMMANDS=()
run_workspace_tests
assert_no_codegen_test_driver
assert_equals \
  "cargo test --locked --workspace --all-targets --exclude ${RUSTC_CODEGEN_TEST_PACKAGE} --exclude fe2o3-artifact-transaction" \
  "$(step_command workspace-tests)" \
  'full workspace test command must isolate the backend and bounded artifact suite'
assert_equals \
  'env FE2O3_HIP_SYS_DISABLE=1 RUST_TEST_THREADS=8 cargo test --locked -p fe2o3-artifact-transaction' \
  "$(step_command fe2o3-artifact-transaction-tests)" \
  'full workspace tests did not retain the descriptor-safe artifact-transaction bound'
assert_step_count fe2o3-artifact-transaction-tests 1 \
  'full workspace tests did not run artifact-transaction tests exactly once'
assert_equals \
  "cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --lib" \
  "$(step_command rustc-codegen-lib-tests)" \
  'full workspace backend library test command changed'
assert_equals \
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --test g2_layout" \
  "$(step_command rustc-codegen-test-g2_layout)" \
  'full workspace backend integration tests are not target-isolated'
assert_all_codegen_targets_once

STEP_NAMES=()
STEP_COMMANDS=()
run_rustc_codegen_shard 01-production-pipeline
assert_no_codegen_test_driver
assert_equals \
  "python3 ${RUSTC_CODEGEN_SHARD_POLICY} check" \
  "$(step_command rustc-codegen-shard-policy)" \
  'codegen shard did not validate the checked-in assignment'
assert_equals \
  "env CARGO_PROFILE_DEV_DEBUG=1 cargo test --locked -p ${RUSTC_CODEGEN_TEST_PACKAGE} --test production_pipeline" \
  "$(step_command rustc-codegen-test-production_pipeline)" \
  'codegen shard did not keep its target isolated'
assert_step_count rustc-codegen-lib-tests 0 \
  'integration shard unexpectedly reran backend library tests'
for shard_step in "${STEP_NAMES[@]}"; do
  if [[ "${shard_step}" == rustc-codegen-test-* ]] &&
    [[ "${shard_step}" != rustc-codegen-test-production_pipeline ]]; then
    printf 'codegen shard ran an unassigned target: %s\n' "${shard_step}" >&2
    exit 1
  fi
done

STEP_NAMES=()
STEP_COMMANDS=()
retire_cargo_fe2o3_driver
run_generic_core
assert_step_count source-isa-unit-matrix 0 \
  'generic core unexpectedly ran the protected source/ISA unit matrix'
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
  standalone-tiled-gemm-general-host-check \
  standalone-flash-attention-general-host-check \
  backend-build \
  backend-all-features-build \
  ci-local-test-gate \
  cargo-fe2o3-tests \
  cargo-fe2o3-worker-v3-envelope-tests \
  fe2o3-pliron-default-api-ui \
  fe2o3-artifact-transaction-tests \
  cpu-tests \
  wrapper-managed-cpu-tests \
  cpu-test-partition-revalidation \
  cpu-test-binding-projection-revalidation \
  rustc-codegen-lib-tests \
  core-doc-tests \
  device-copy-renamed-dependency \
  device-copy-derive-real-trait \
  device-copy-derive-ui \
  core-production-runtime-surface-ui \
  s09-debug-checker; do
  assert_step_count "${core_step}" 1 \
    "generic core did not run ${core_step} exactly once"
done
assert_equals \
  "env FE2O3_HIP_SYS_DISABLE=1 cargo test --locked -p cargo-fe2o3" \
  "$(step_command cargo-fe2o3-tests)" \
  'generic core did not gate the feature-invariant cargo-fe2o3 suite'
assert_equals \
  "env FE2O3_HIP_SYS_DISABLE=1 cargo test --locked -p cargo-fe2o3 --features ${CARGO_FE2O3_WORKER_V3_INTEGRATION_FEATURE} --test worker_v3_load_envelope_vertical -- --test-threads=1" \
  "$(step_command cargo-fe2o3-worker-v3-envelope-tests)" \
  'generic core did not gate the strict Worker V3 envelope vertical suite'
assert_equals \
  'cargo test --locked -p fe2o3-pliron --no-default-features --test middle_end_evidence_ui default_api_cannot_self_authorize -- --exact' \
  "$(step_command fe2o3-pliron-default-api-ui)" \
  'generic core did not gate the feature-free Pliron public API'
assert_equals \
  'env FE2O3_HIP_SYS_DISABLE=1 RUST_TEST_THREADS=8 cargo test --locked -p fe2o3-artifact-transaction' \
  "$(step_command fe2o3-artifact-transaction-tests)" \
  'generic core did not retain the descriptor-safe artifact-transaction fanout bound'
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
  'env CARGO_PROFILE_DEV_DEBUG=1 cargo build --locked -p rustc-codegen-fe2o3 --all-features' \
  "$(step_command backend-all-features-build)" \
  'generic core did not build the all-feature production backend'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 check --workspace --all-targets --locked --exclude fe2o3-production-extraction-fixture --exclude fe2o3-production-ranked-bounds-fixture --exclude fe2o3-disabled-fixture" \
  "$(step_command workspace-binding-check)" \
  'managed check did not cover the whole supported workspace graph'
assert_equals \
  "bash scripts/tests/binding-check-boundary.sh ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 fe2o3-managed-a" \
  "$(step_command workspace-binding-check-boundary)" \
  'managed check omitted the backend/artifact/publication hostile boundary'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-wrapper-managed fe2o3-managed-a fe2o3-managed-b" \
  "$(step_command workspace-binding-projection-revalidation)" \
  'managed check did not revalidate the exact structural package projection'
assert_equals \
  "env FE2O3_HIP_SYS_DISABLE=1 ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 check --locked --all-targets --manifest-path examples/tiled_gemm_general_v1/Cargo.toml" \
  "$(step_command standalone-tiled-gemm-general-host-check)" \
  'generic core did not check the complete standalone dynamic GEMM host surface'
assert_equals \
  "env FE2O3_HIP_SYS_DISABLE=1 ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 check --locked --all-targets --manifest-path examples/flash_attention_general_v1/Cargo.toml" \
  "$(step_command standalone-flash-attention-general-host-check)" \
  'generic core did not check the complete standalone dynamic attention host surface'
assert_equals \
  'env FE2O3_HIP_SYS_DISABLE=1 cargo test --locked -p fe2o3-core --test production_runtime_surface_ui' \
  "$(step_command core-production-runtime-surface-ui)" \
  'generic core did not retain the default-feature raw launch rejection UI'
assert_equals \
  0 \
  "$(step_count cpu-tests-cargo-fe2o3-bootstrap)" \
  'generic core rebuilt its byte-identical content-addressed production driver'
assert_equals \
  "env FE2O3_HIP_SYS_DISABLE=1 ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 test --locked --all-targets -p fe2o3-managed-a -p fe2o3-managed-b" \
  "$(step_command wrapper-managed-cpu-tests)" \
  'generic core did not route managed CPU tests through cargo-fe2o3'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-cpu-test-partition fe2o3-ordinary -- fe2o3-managed-a fe2o3-managed-b" \
  "$(step_command cpu-test-partition-revalidation)" \
  'generic core did not revalidate both complete CPU package lists'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-wrapper-managed fe2o3-managed-a fe2o3-managed-b" \
  "$(step_command cpu-test-binding-projection-revalidation)" \
  'generic core did not revalidate the CPU binding projection'
for core_step in "${STEP_NAMES[@]}"; do
  if [[ "${core_step}" == rustc-codegen-test-* ]]; then
    printf 'generic core unexpectedly ran integration target: %s\n' "${core_step}" >&2
    exit 1
  fi
done
assert_no_codegen_test_driver

STEP_NAMES=()
STEP_COMMANDS=()
retire_cargo_fe2o3_driver
run_generic
assert_no_codegen_test_driver
assert_all_codegen_targets_once
assert_step_count rustc-codegen-shard-policy 1 \
  'serial generic gate did not run shard policy exactly once'
assert_step_count rustc-codegen-lib-tests 1 \
  'serial generic gate did not run backend library tests exactly once'

STEP_NAMES=()
STEP_COMMANDS=()
EMPTY_WRAPPER_CPU_INTERSECTION=1
CARGO_FE2O3_DRIVER_PROFILE=
reset_mock_production_driver
run_cpu_tests
assert_step_count wrapper-managed-cpu-tests 0 \
  'empty managed CPU intersection still invoked the binding test command'
assert_equals \
  'env FE2O3_HIP_SYS_DISABLE=1 RUST_TEST_THREADS=8 cargo test --locked -p fe2o3-artifact-transaction' \
  "$(step_command fe2o3-artifact-transaction-tests)" \
  'empty managed CPU intersection dropped the descriptor-safe artifact-transaction bound'
assert_step_count fe2o3-artifact-transaction-tests 1 \
  'empty managed CPU intersection did not run artifact-transaction tests exactly once'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-cpu-test-partition fe2o3-ordinary --" \
  "$(step_command cpu-test-partition-revalidation)" \
  'empty managed CPU intersection skipped complete partition revalidation'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-wrapper-managed fe2o3-managed-a fe2o3-managed-b" \
  "$(step_command cpu-test-binding-projection-revalidation)" \
  'empty managed CPU intersection skipped full structural revalidation'
EMPTY_WRAPPER_CPU_INTERSECTION=0

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
    printf 'direct sealed driver retained %s\n' "${loader_name}" >&2
    exit 1
  fi
done
rg -Fx 'FE2O3_CI_TEST_PRESERVED=present' <<<"${driver_environment}" >/dev/null || {
  printf '%s\n' 'direct sealed driver lost an unrelated variable' >&2
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
  printf '%s\n' 'sealed driver validator accepted changed mode' >&2
  exit 1
fi
chmod 500 -- "${CARGO_FE2O3_BINARY}"
chmod 700 -- "${CARGO_FE2O3_BINARY}"
printf '# hostile replacement\n' >>"${CARGO_FE2O3_BINARY}"
chmod 500 -- "${CARGO_FE2O3_BINARY}"
if validate_cargo_fe2o3_driver 2>/dev/null; then
  printf '%s\n' 'sealed driver validator accepted changed content' >&2
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
declare -a validated_managed_wrapper_packages=()
validate_managed_wrapper_source_namespaces() {
  validated_managed_wrapper_packages=("$@")
}
load_example_packages() {
  local destination_name="$2"
  local -n destination="${destination_name}"
  destination=(fe2o3-fill)
}
export FE2O3_WORKER_V2_CONFIG_V2="${TIMEOUT_TEST_ROOT}/hostile-worker-v2-config.json"
run_rocm_compile
if (export FE2O3_TARGET=gfx1100; run_rocm_compile >/dev/null 2>&1); then
  printf '%s\n' 'ROCm compilation accepted a target not covered by its production drivers' >&2
  exit 1
fi
if [[ -v FE2O3_WORKER_V2_CONFIG_V2 ]]; then
  printf '%s\n' \
    'ROCm compilation retained an ambient Worker V2 configuration' >&2
  exit 1
fi
assert_equals \
  'fe2o3-typed-alias-spoof' \
  "${ROCM_EXPLICIT_NAMESPACE_FALLBACK_PACKAGES[*]}" \
  'ROCm explicit namespace fallback allowlist expanded or changed'
for managed_package in \
  fe2o3-fill \
  fe2o3-vecadd \
  fe2o3-trusted-item-renamed-genuine \
  fe2o3-trusted-item-lookalike-type \
  fe2o3-trusted-item-lookalike-helper \
  fe2o3-trusted-item-lookalike-thread \
  fe2o3-trusted-item-external-spoof \
  fe2o3-trusted-item-local-marker \
  fe2o3-typed-alias-spoof; do
  printf '%s\n' "${validated_managed_wrapper_packages[@]}" |
    rg -Fx -- "${managed_package}" >/dev/null || {
      printf 'ROCm compile namespace gate omitted %s\n' "${managed_package}" >&2
      exit 1
    }
done
assert_equals \
  'cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 --message-format=json-render-diagnostics' \
  "$(step_command rocm-cargo-fe2o3-bootstrap)" \
  'ROCm compile did not build the feature-invariant shared driver once'
assert_equals \
  "env ${TIMEOUT_TEST_ROOT}/rocm-driver/cargo-fe2o3 doctor" \
  "$(step_command rocm-doctor)" \
  'ROCm compile did not invoke the resolved driver directly for doctor'
for production_step in \
  rocm-production-extraction-safe-kernel \
  rocm-production-extraction-unsafe-rejection \
  rocm-production-general-matrix \
  rocm-production-general-attention \
  rocm-production-transaction \
  rocm-production-ranked-bounds \
  rocm-production-barrier-cfg \
  rocm-production-simulation-bundle-gfx942 \
  rocm-production-simulation-bundle-gfx950 \
  rocm-production-simulation-bundle-v2-source-variables \
  rocm-production-simulation-bundle-v2-invalid-name; do
  assert_step_count "${production_step}" 1 \
    "ROCm compile did not run ${production_step} exactly once"
  if [[ " $(step_command "${production_step}") " == *" --features "* ]]; then
    printf 'ROCm production step enabled a non-production feature: %s\n' \
      "${production_step}" >&2
    exit 1
  fi
done
for index in "${!STEP_NAMES[@]}"; do
  step_name="${STEP_NAMES[index]}"
  step_command_value="${STEP_COMMANDS[index]}"
  if [[ "${step_name}" == rocm-qualification-tests-* ]] ||
    { [[ "${step_name}" == rocm-* ]] &&
      [[ " ${step_command_value} " == *" --features "* ]] &&
      { [[ " ${step_command_value} " == *" -p fe2o3-host "* ]] ||
        [[ " ${step_command_value} " == *" -p fe2o3-pliron-scalar-add-v1 "* ]]; }; }; then
    printf 'deleted ROCm qualification test lane returned as %s\n' "${step_name}" >&2
    exit 1
  fi
  if [[ " ${step_command_value} " == *" FE2O3_QUALIFICATION_ORACLE_V1="* ]] &&
    [[ " ${step_command_value} " == *"/cargo-fe2o3 build "* ]]; then
    printf 'ROCm compile restored an obsolete Cargo qualification build: %s\n' \
      "${step_name}" >&2
    exit 1
  fi
done
assert_equals \
  'env cargo test --locked -p rustc-codegen-fe2o3 --test production_general_matrix_driver_v1 dynamic_matrix_kernel_reaches_gfx942_llvm -- --ignored --exact' \
  "$(step_command rocm-production-general-matrix)" \
  'ROCm compile did not run the exact successful dynamic matrix compiler test'
assert_equals \
  'env cargo test --locked -p rustc-codegen-fe2o3 --test production_general_matrix_driver_v1 dynamic_attention_kernel_reaches_gfx942_llvm -- --ignored --exact' \
  "$(step_command rocm-production-general-attention)" \
  'ROCm compile did not run the exact successful dynamic attention compiler test'
assert_equals \
  "cargo test --locked -p dialect-amdgcn --test lowering rocm_compiles_the_golden_to_an_amdgpu_code_object -- --ignored --exact" \
  "$(step_command rocm-g1-code-object)" \
  'ROCm compile did not produce the target code-object fixture'
for retired_step in \
  rocm-trusted-device-items \
  rocm-trusted-device-item-stale-cleanup \
  rocm-cross-crate-typed-binding \
  rocm-kernel-ir-codegen-rejection \
  rocm-kernel-ir-vecadd; do
  assert_step_count "${retired_step}" 0 \
    "ROCm compile restored retired qualification step ${retired_step}"
done
assert_step_count rocm-clean-kernel-ir-v1-fe2o3-fill 0 \
  'ROCm compile restored obsolete manifest-routed cleanup'
assert_step_count rocm-build-kernel-ir-v1-fe2o3-fill 0 \
  'ROCm compile restored obsolete Cargo qualification'
assert_step_count rocm-artifacts-kernel-ir-v1-fe2o3-fill 0 \
  'ROCm compile restored obsolete manifest-routed artifact inspection'

STEP_NAMES=()
STEP_COMMANDS=()
require_gpu_access() {
  return 0
}
export FE2O3_ALLOW_GPU_SMOKE=1
export FE2O3_TARGET=gfx942:xnack-
run_hardware_smoke
assert_equals \
  "env FE2O3_ALLOW_RUNTIME_IDENTITY_ORACLE=1 bash ${RUNTIME_IDENTITY_ORACLE}" \
  "$(step_command hardware-runtime-identity-oracle)" \
  'hardware smoke did not run the KFD runtime identity oracle'
assert_equals \
  'cargo run --locked -p fe2o3-kfd --example kfd-device-identity -- --all' \
  "$(step_command hardware-kfd-device-identity)" \
  'hardware smoke did not admit every visible KFD device'
assert_equals \
  'cargo run --locked -p fe2o3-kfd --features live-validation --example kfd-host-visible-memory -- --all' \
  "$(step_command hardware-kfd-host-visible-memory)" \
  'hardware smoke did not exercise KFD host-visible memory on every device'
assert_equals \
  'cargo run --locked -p fe2o3-kfd --features live-validation --example kfd-compute-aql-queue -- --all' \
  "$(step_command hardware-kfd-compute-aql-queue)" \
  'hardware smoke did not exercise KFD AQL queue ownership on every device'
assert_equals \
  'cargo test --locked -p fe2o3-debug-cli --features live-validation --test hardware_v2_live -- --test-threads=1' \
  "$(step_command hardware-kfd-debug-protocol-v2)" \
  'hardware smoke did not exercise the KFD hardware debugger V2 protocol'
assert_equals \
  'cargo test --locked -p fe2o3-debug-cli --features live-validation --test live_kfd_v3_live -- --exact mi300x_live_kfd_v3_binds_observes_controls_and_terminates --nocapture --test-threads=1' \
  "$(step_command hardware-kfd-live-gpu-debug-v3)" \
  'hardware smoke did not exercise the exact-bound live GPU debugger V3 protocol'
for retired_hardware_step in \
  hardware-cargo-fe2o3-bootstrap \
  hardware-hip-device-properties-build \
  hardware-hip-device-properties-test \
  hardware-observed-device-target \
  hardware-device-copy-transfer \
  hardware-kernel-ir-fill \
  hardware-kernel-ir-vecadd \
  hardware-hsaco-inspection; do
  assert_step_count "${retired_hardware_step}" 0 \
    "KFD hardware smoke restored retired step ${retired_hardware_step}"
done
assert_step_count hardware-smoke 0 \
  'hardware smoke retained the selector-free manifest runner'
unset FE2O3_ALLOW_GPU_SMOKE FE2O3_TARGET

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
