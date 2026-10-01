#!/usr/bin/env bash

# Private extension of ci-local-test-gate.sh's command-capture harness.
run_pre_split_generic_core_reference() {
  run_workspace_dependency_policy
  run_standalone_lockfiles
  run_runtime_pure_rust_policy
  run_step example-manifest \
    cargo run --quiet --locked -p cargo-fe2o3 -- examples check
  run_step bounded-moe-docs \
    python3 scripts/test-bounded-moe-docs.py
  run_shard_policy
  run_parity_matrix_checks
  run_format
  run_check
  run_backend_build
  run_step simulation-expectation-tests \
    python3 -I -B scripts/tests/simulation_expectation.py
  run_step tutorial-scalar-gemm-corpus-tests \
    python3 -I -B scripts/tests/tutorial_scalar_gemm_corpus.py
  run_step quickstart-shell-tests bash scripts/tests/quickstart.sh
  run_step kernel-compile-matrix-shell-tests \
    bash scripts/tests/kernel-compile-matrix.sh
  run_step tutorial-cpu-reference-tests \
    python3 -B scripts/tests/tutorial_cpu_reference.py
  run_step no-gpu-source-quickstart bash scripts/quickstart.sh no-gpu
  run_step source-core-branch-hints \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_checked_add_tests::genuine_core_branch_hints_preserve_boolean_values
  run_step source-core-checked-add \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_checked_add_tests::genuine_unsigned_checked_add_preserves_option_boundaries
  run_step source-core-slice-get \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_slice_get_tests::genuine_core_shared_slice_get_preserves_boundaries
  run_step source-core-branch-hint-refusals \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_checked_add_tests::branch_hint_lookalikes_do_not_bypass_source_safety
  run_step source-core-checked-add-refusals \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_checked_add_tests::checked_add_lookalikes_do_not_bypass_source_safety
  run_step source-core-slice-get-refusals \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_slice_get_tests::slice_get_lookalikes_and_other_owners_do_not_bypass_source_safety
  run_step source-core-scalar-enum-payload \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_scalar_enum_payload_tests::genuine_scalar_enum_payload_elision_preserves_source_semantics
  run_step kir-sim-capability-matrix \
    cargo test --locked -p fe2o3-kir-sim --test capability_matrix
  run_step kir-sim-scalar-differential \
    cargo run --quiet --locked -p fe2o3-sim-differential --bin fe2o3-sim-differential -- \
      --seed-start 0 --cases 256
  run_step kir-sim-semantic-differential \
    cargo run --quiet --locked -p fe2o3-sim-differential --bin fe2o3-sim-differential -- \
      semantic-run-v2 --seed 0
  run_step kir-sim-f32-differential \
    cargo run --quiet --locked -p fe2o3-sim-differential --bin fe2o3-sim-differential -- \
      f32-run-v3
  run_step ci-local-test-gate bash scripts/tests/ci-local-test-gate.sh
  run_cpu_tests
  run_rustc_codegen_lib_tests
  run_auxiliary_tests
}

reset_generic_phase_capture() {
  STEP_NAMES=()
  STEP_COMMANDS=()
  STEP_TIMEOUT_OVERRIDES=()
  STANDALONE_LOCKFILES_CHECKED=0
  retire_cargo_fe2o3_driver
}

assert_generic_phase_cli_status() {
  local expected="$1" label="$2" mode="$3" step_timeout="$4"
  shift 4
  local root="${TIMEOUT_TEST_ROOT}/generic-phase-cli"
  local status=0
  : >"${root}/${label}.trace"
  # An external shell keeps its own errexit semantics; do not put sourced
  # production functions in an if/OR-list just to inspect their failures.
  timeout --signal=TERM --kill-after=2s 10s \
    env PATH="${root}/bin:${PATH}" \
      CARGO_TARGET_DIR="${root}/target" CI_LOG_DIR="${root}/${label}-logs" \
      PHASE_MOCK_TRACE="${root}/${label}.trace" PHASE_MOCK_MODE="${mode}" \
      FE2O3_CI_STEP_TIMEOUT_SECONDS="${step_timeout}" \
      FE2O3_CI_STEP_KILL_AFTER_SECONDS=1 \
      bash "${TEST_SCRIPT_DIR}/../ci-local.sh" "$@" \
      >"${root}/${label}.stdout" 2>"${root}/${label}.stderr" || status=$?
  assert_equals "${expected}" "${status}" "phase CLI ${label} status changed"
}

assert_generic_phase_cli() {
  local root="${TIMEOUT_TEST_ROOT}/generic-phase-cli" tool
  mkdir -m 700 -- "${root}" "${root}/bin" "${root}/target"
  for tool in cargo python3; do
    cat >"${root}/bin/${tool}" <<'MOCK'
#!/usr/bin/env bash
set -Eeuo pipefail
printf '%s' "${0##*/}" >>"${PHASE_MOCK_TRACE}"
printf ' %q' "$@" >>"${PHASE_MOCK_TRACE}"
printf '\n' >>"${PHASE_MOCK_TRACE}"
case "${PHASE_MOCK_MODE}" in
  pass) printf 'phase fixture output\n' ;;
  fail) exit 37 ;;
  hang) exec sleep 5 ;;
  *) exit 99 ;;
esac
MOCK
    chmod 700 -- "${root}/bin/${tool}"
  done

  assert_generic_phase_cli_status 0 list fail 3 generic-core-phases
  assert_equals $'policy\nbuild\nsource-simulation\ncpu\ncodegen-lib\nauxiliary' \
    "$(cat "${root}/list.stdout")" 'phase CLI roster changed'
  [[ ! -s "${root}/list.trace" ]]
  assert_generic_phase_cli_status 2 missing fail 3 generic-core-phase
  assert_generic_phase_cli_status 2 empty fail 3 generic-core-phase ''
  assert_generic_phase_cli_status 2 unknown fail 3 generic-core-phase all
  assert_generic_phase_cli_status 2 extra fail 3 generic-core-phase cpu auxiliary
  assert_generic_phase_cli_status 2 list-extra fail 3 generic-core-phases cpu
  local label
  for label in missing empty unknown extra list-extra; do
    [[ ! -s "${root}/${label}.trace" ]] || {
      printf 'invalid phase CLI %s executed a validation command\n' "${label}" >&2
      return 1
    }
  done

  assert_generic_phase_cli_status 37 build-failure fail 3 generic-core-phase build
  assert_equals 'cargo fmt --all -- --check' \
    "$(cat "${root}/build-failure.trace")" 'build phase continued after failure'
  [[ -f "${root}/build-failure-logs/format.log" ]]
  assert_generic_phase_cli_status 37 source-failure fail 3 generic-core-phase source-simulation
  assert_equals 'python3 -I -B scripts/tests/simulation_expectation.py' \
    "$(cat "${root}/source-failure.trace")" 'source phase continued after failure'
  assert_generic_phase_cli_status 37 core-failure fail 3 generic-core
  assert_equals "python3 ${WORKSPACE_DEPENDENCY_POLICY_TESTS}" \
    "$(cat "${root}/core-failure.trace")" 'full core continued after its first phase failed'
  assert_generic_phase_cli_status 124 deadline hang 1 generic-core-phase codegen-lib
  assert_equals 'cargo test --locked -p rustc-codegen-fe2o3 --lib' \
    "$(cat "${root}/deadline.trace")" 'phase timeout changed its selected command'
  rg -F 'step rustc-codegen-lib-tests timed out after 1 seconds' \
    "${root}/deadline.stderr" >/dev/null

  cat >"${root}/bin/tee" <<'MOCK'
#!/usr/bin/env bash
cat >/dev/null
exit 73
MOCK
  chmod 700 -- "${root}/bin/tee"
  assert_generic_phase_cli_status 73 logger pass 3 generic-core-phase build
  assert_equals 'cargo fmt --all -- --check' \
    "$(cat "${root}/logger.trace")" 'build phase continued after logger failure'
  rg -F 'step format log write failed with status 73' "${root}/logger.stderr" >/dev/null
  rm -- "${root}/bin/tee"
}

assert_core_source_auth_steps() {
  local -a steps=(
    source-core-branch-hints
    source-core-checked-add
    source-core-slice-get
    source-core-branch-hint-refusals
    source-core-checked-add-refusals
    source-core-slice-get-refusals
    source-core-scalar-enum-payload
  )
  local -a parents=(
    core_checked_add_tests::genuine_core_branch_hints_preserve_boolean_values
    core_checked_add_tests::genuine_unsigned_checked_add_preserves_option_boundaries
    core_slice_get_tests::genuine_core_shared_slice_get_preserves_boundaries
    core_checked_add_tests::branch_hint_lookalikes_do_not_bypass_source_safety
    core_checked_add_tests::checked_add_lookalikes_do_not_bypass_source_safety
    core_slice_get_tests::slice_get_lookalikes_and_other_owners_do_not_bypass_source_safety
    core_scalar_enum_payload_tests::genuine_scalar_enum_payload_elision_preserves_source_semantics
  )
  local -a actual=()
  local step index expected
  for step in "${STEP_NAMES[@]}"; do
    if [[ "${step}" == source-core-* ]]; then
      actual+=("${step}")
    fi
  done
  assert_equals "$(printf '%s\n' "${steps[@]}")" \
    "$(printf '%s\n' "${actual[@]}")" 'source authentication parent order or roster changed'
  for index in "${!steps[@]}"; do
    step="${steps[index]}"
    assert_step_count "${step}" 1 'source authentication parent was omitted or duplicated'
    expected="bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1"
    expected+=" ${parents[index]}"
    assert_equals "${expected}" "$(step_command "${step}")" \
      'source authentication parent lost its exact ignored serial production selection'
  done
}

assert_core_source_auth_fail_fast() {
  local step status trace
  for step in \
    source-core-branch-hints source-core-checked-add source-core-slice-get \
    source-core-branch-hint-refusals source-core-checked-add-refusals \
    source-core-slice-get-refusals source-core-scalar-enum-payload; do
    trace="${TIMEOUT_TEST_ROOT}/${step}.trace"
    status=0
    # Exercise the real phase body in its own shell, retaining errexit semantics.
    timeout --signal=TERM --kill-after=2s 10s \
      env CORE_SOURCE_FAIL_STEP="${step}" CORE_SOURCE_TRACE="${trace}" \
      bash -c '
        source "$1"
        run_step() {
          printf "%s\n" "$1" >>"${CORE_SOURCE_TRACE}"
          [[ "$1" != "${CORE_SOURCE_FAIL_STEP}" ]] || return 37
        }
        run_generic_core_source_simulation
        printf "%s\n" unexpected-success
      ' -- "${TEST_SCRIPT_DIR}/../ci-local.sh" \
      >"${trace}.stdout" 2>"${trace}.stderr" || status=$?
    assert_equals 37 "${status}" 'a source authentication parent failure was suppressed'
    assert_equals "${step}" "$(tail -n 1 "${trace}")" \
      'source simulation continued after an authentication parent failed'
    [[ ! -s "${trace}.stdout" ]]
  done
}

assert_generic_core_phases() {
  python3 -I -B "${TEST_SCRIPT_DIR}/cargo_test_json.py"
  bash "${TEST_SCRIPT_DIR}/ci-cargo-test-json.sh"
  local -a expected_names expected_commands
  local phase step
  assert_equals $'policy\nbuild\nsource-simulation\ncpu\ncodegen-lib\nauxiliary' \
    "$(list_generic_core_phases)" 'complete ordered phase roster changed'

  reset_generic_phase_capture
  run_pre_split_generic_core_reference
  expected_names=("${STEP_NAMES[@]}")
  expected_commands=("${STEP_COMMANDS[@]}")

  reset_generic_phase_capture
  run_generic_core
  assert_core_source_auth_steps
  assert_equals "$(printf '%s\n' "${expected_names[@]}")" \
    "$(printf '%s\n' "${STEP_NAMES[@]}")" 'default core step order changed'
  assert_equals "$(printf '%s\n' "${expected_commands[@]}")" \
    "$(printf '%s\n' "${STEP_COMMANDS[@]}")" 'default core command arguments changed'

  reset_generic_phase_capture
  for phase in "${GENERIC_CORE_PHASES[@]}"; do
    run_generic_core_phase "${phase}"
  done
  assert_core_source_auth_steps
  assert_equals "$(printf '%s\n' "${expected_names[@]}")" \
    "$(printf '%s\n' "${STEP_NAMES[@]}")" 'phase concatenation changed core step order'
  assert_equals "$(printf '%s\n' "${expected_commands[@]}")" \
    "$(printf '%s\n' "${STEP_COMMANDS[@]}")" 'phase concatenation changed core arguments'
  assert_equals 0 "${#STEP_TIMEOUT_OVERRIDES[@]}" 'phases introduced timeout overrides'
  assert_step_count standalone-lockfiles 1 'warm phases repeated standalone-lock checks'
  assert_step_count cpu-tests-cargo-fe2o3-bootstrap 0 'warm phases rebuilt their sealed driver'
  for step in "${STEP_NAMES[@]}"; do
    [[ "${step}" != *cache-clean* && "${step}" != rustc-codegen-test-* ]] || {
      printf 'phase split added cleanup or an integration shard: %s\n' "${step}" >&2
      return 1
    }
  done

  reset_generic_phase_capture
  run_generic_core_phase source-simulation
  assert_core_source_auth_steps
  assert_step_count backend-build 0 'fresh source phase ran the build phase'
  assert_step_count cpu-tests 0 'fresh source phase ran the CPU phase'
  assert_step_count rustc-codegen-lib-tests 0 'fresh source phase ran codegen library tests'
  assert_equals 0 "${#STEP_TIMEOUT_OVERRIDES[@]}" 'source parents introduced timeout overrides'
  assert_core_source_auth_fail_fast

  reset_generic_phase_capture
  run_generic_core_phase cpu
  assert_step_count cpu-tests-cargo-fe2o3-bootstrap 1 'fresh CPU phase skipped driver authentication'
  assert_step_count cargo-fe2o3-tests 1 'fresh CPU phase omitted Cargo package tests'
  assert_step_count cargo-fe2o3-worker-v3-envelope-tests 1 'fresh CPU phase omitted worker coverage'
  assert_step_count cpu-tests 1 'fresh CPU phase omitted the raw package partition'
  assert_step_count wrapper-managed-cpu-tests 1 'fresh CPU phase omitted the wrapper partition'
  assert_step_count cpu-test-partition-revalidation 1 'fresh CPU phase skipped partition revalidation'
  assert_step_count cpu-test-binding-projection-revalidation 1 'fresh CPU phase skipped source rescan'
  assert_step_count backend-build 0 'fresh CPU phase ran another phase'
  assert_step_count rustc-codegen-lib-tests 0 'fresh CPU phase ran backend library tests'
  assert_equals \
    'cargo build --locked -p cargo-fe2o3 --bin cargo-fe2o3 --message-format=json-render-diagnostics' \
    "$(step_command cpu-tests-cargo-fe2o3-bootstrap)" 'fresh CPU phase changed driver features'
  assert_equals \
    "env ${TIMEOUT_TEST_ROOT}/production-driver/cargo-fe2o3 examples check-cpu-test-partition fe2o3-ordinary -- fe2o3-managed-a fe2o3-managed-b" \
    "$(step_command cpu-test-partition-revalidation)" 'fresh CPU phase changed its complete partition'

  reset_generic_phase_capture
  run_generic_core_phase build
  assert_step_count standalone-lockfiles 1 'fresh build phase skipped standalone lock validation'
  assert_step_count generic-check-cargo-fe2o3-bootstrap 1 'fresh build phase skipped driver authentication'
  assert_step_count backend-build 1 'fresh build phase omitted the backend'
  assert_step_count backend-all-features-build 1 'fresh build phase omitted the all-feature backend'
  assert_step_count cpu-tests 0 'fresh build phase ran another phase'
  assert_generic_phase_cli
}
