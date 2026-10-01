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
