#!/usr/bin/env bash

# Sourced by ci-local.sh; a phase result is not aggregate qualification.
readonly -a GENERIC_CORE_PHASES=(
  policy
  build
  source-simulation
  cpu
  codegen-lib
  auxiliary
)

list_generic_core_phases() {
  if (($# != 0)); then
    printf '%s\n' 'generic-core-phases accepts no arguments' >&2
    return 2
  fi
  printf '%s\n' "${GENERIC_CORE_PHASES[@]}"
}

run_generic_core_policy() {
  run_workspace_dependency_policy
  run_standalone_lockfiles
  run_runtime_pure_rust_policy
  run_step example-manifest \
    cargo run --quiet --locked -p cargo-fe2o3 -- examples check
  run_step bounded-moe-docs \
    python3 scripts/test-bounded-moe-docs.py
  run_shard_policy
  run_parity_matrix_checks
}

run_generic_core_build() {
  run_format
  run_check
  run_backend_build
}

run_generic_core_source_simulation() {
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
  run_step source-core-typed-indirect-constants \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      typed_indirect_constant_tests::genuine_typed_indirect_constants_preserve_source_semantics
  run_step source-core-scalar-enum-returns \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_scalar_enum_return_tests::genuine_scalar_enum_helper_returns_preserve_source_semantics
  run_step source-core-scalar-enum-return-refusals \
    bash scripts/ci-cargo-test-json.sh rustc-codegen-fe2o3 production_extraction_driver_v1 \
      core_scalar_enum_return_tests::pointer_and_capability_enum_returns_remain_refused
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
}

run_rustc_codegen_lib_tests() {
  # Do not combine this with integration targets: Cargo can emit a test rlib
  # and an unversioned backend dylib with different Rust symbol hashes.
  # Keep the aggregate rustc-private harness bounded like the isolated targets;
  # full debuginfo can exceed the executable identity measurement limit.
  run_step rustc-codegen-lib-tests \
    cargo test --locked -p "${RUSTC_CODEGEN_TEST_PACKAGE}" --lib
  run_step source-formal-execution-discharge \
    bash scripts/ci-cargo-test-json.sh --lib rlib,dylib \
      "${RUSTC_CODEGEN_TEST_PACKAGE}" rustc_codegen_fe2o3 \
      production_rustc_driver_v1::checked_output_source_v1_tests::formal_memory_diagnostic::ordinary_lds_source_retains_owner_bound_execution_discharge
  run_step source-slice-constant-index \
    bash scripts/ci-cargo-test-json.sh --lib rlib,dylib \
      "${RUSTC_CODEGEN_TEST_PACKAGE}" rustc_codegen_fe2o3 \
      production_semantic_body_v1::slice_constant_index_source_v1_tests::genuine_slice_constant_indices_preserve_retained_mir
}

run_generic_core_phase() {
  if (($# != 1)); then
    printf '%s\n' 'generic-core-phase requires exactly one phase id' >&2
    return 2
  fi
  case "$1" in
    policy) run_generic_core_policy ;;
    build) run_generic_core_build ;;
    source-simulation) run_generic_core_source_simulation ;;
    cpu) run_cpu_tests ;;
    codegen-lib) run_rustc_codegen_lib_tests ;;
    auxiliary) run_auxiliary_tests ;;
    *)
      printf 'unknown generic-core phase: %s\n' "$1" >&2
      return 2
      ;;
  esac
}

run_generic_core() {
  local phase
  for phase in "${GENERIC_CORE_PHASES[@]}"; do
    run_generic_core_phase "${phase}"
  done
}
