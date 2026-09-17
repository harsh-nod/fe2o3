#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT

cd -- "${REPO_ROOT}"

if [[ "$(uname -s)" != Linux || "$(uname -m)" != x86_64 ]]; then
  printf '%s\n' 'authenticated Verus V2 requires the reviewed Linux x86_64 host' >&2
  exit 2
fi

rustup show active-toolchain
rustc -Vv
cargo -V

cargo test --locked -p fe2o3-verifier \
  --test authenticated_verus_execution_v2 \
  -- --include-ignored --test-threads=1
cargo test --locked --release -p fe2o3-verifier \
  --test authenticated_verus_execution_v2 \
  -- --include-ignored --test-threads=1

readonly PROTECTED_LEASE_TESTS=(
  functional_refinement_runtime_v1::tests::protected_public_lease_executes_real_verus
  functional_refinement_runtime_v1::tests::protected_public_lease_rejects_false_proof
  functional_refinement_receipt_v2::reviewed_host_tests::protected_ranked_scalar_prepares_bound_receipt
  functional_refinement_receipt_v2::reviewed_host_tests::protected_ranked_scalar_rejects_semantic_mismatch
  functional_refinement_receipt_v2::reviewed_host_tests::protected_ranked_effect_executes_and_imports_bound_receipt
  functional_refinement_receipt_v2::reviewed_host_tests::protected_ranked_effect_rejects_coordinate_mismatch
  functional_refinement_receipt_v2::reviewed_host_tests::protected_ieee_operator_congruence_imports_bound_receipt
  functional_refinement_receipt_v2::reviewed_host_tests::protected_ieee_operator_mutation_rejects_bound_receipt
)

readonly PROTECTED_SOURCE_TESTS=(
  reviewed_host_tests::protected_rust_reference_positive_reaches_verified_callback
  reviewed_host_tests::protected_rust_reference_mutation_fails_functional_proof
)

run_protected_tests() {
  local suite=$1
  shift
  local -a cargo_command=(cargo test --locked "$@") test_names
  case "$suite" in
    lease)
      cargo_command+=(-p fe2o3-verifier --lib)
      test_names=("${PROTECTED_LEASE_TESTS[@]}")
      ;;
    source)
      cargo_command+=(-p rustc-codegen-fe2o3 --test reference_binding_v1)
      test_names=("${PROTECTED_SOURCE_TESTS[@]}")
      ;;
    *) return 2 ;;
  esac
  local listing test_name output
  listing="$("${cargo_command[@]}" -- --ignored --list --format terse)"
  for test_name in "${test_names[@]}"; do
    if [[ "$(grep -Fxc -- "${test_name}: test" <<<"${listing}")" != 1 ]]; then
      printf 'protected proof test missing or duplicated: %s\n' "${test_name}" >&2
      exit 1
    fi
  done

  for test_name in "${test_names[@]}"; do
    if output="$("${cargo_command[@]}" "${test_name}" \
      -- --ignored --exact --test-threads=1 --format pretty --color never 2>&1)"; then
      printf '%s\n' "${output}"
    else
      printf '%s\n' "${output}" >&2
      printf 'protected proof test failed: %s\n' "${test_name}" >&2
      exit 1
    fi
    if [[ "$(grep -Fxc -- 'running 1 test' <<<"${output}")" != 1 \
      || "$(grep -Fxc -- "test ${test_name} ... ok" <<<"${output}")" != 1 \
      || "$(grep -Ec -- '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in .+$' <<<"${output}")" != 1 ]]; then
      printf 'protected proof test execution count differs: %s\n' "${test_name}" >&2
      exit 1
    fi
  done
}

run_protected_tests lease
run_protected_tests lease --release
run_protected_tests source
run_protected_tests source --release
