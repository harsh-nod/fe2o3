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
)

run_protected_lease_tests() {
  local -a cargo_command=(cargo test --locked "$@" -p fe2o3-verifier --lib)
  local listing test_name output
  listing="$("${cargo_command[@]}" \
    functional_refinement_runtime_v1::tests::protected_public_lease_ \
    -- --ignored --list --format terse)"
  for test_name in "${PROTECTED_LEASE_TESTS[@]}"; do
    if [[ "$(grep -Fxc -- "${test_name}: test" <<<"${listing}")" != 1 ]]; then
      printf 'protected public-lease test missing or duplicated: %s\n' "${test_name}" >&2
      exit 1
    fi
  done

  for test_name in "${PROTECTED_LEASE_TESTS[@]}"; do
    if output="$("${cargo_command[@]}" "${test_name}" \
      -- --ignored --exact --test-threads=1 --format pretty --color never 2>&1)"; then
      printf '%s\n' "${output}"
    else
      printf '%s\n' "${output}" >&2
      printf 'protected public-lease test failed: %s\n' "${test_name}" >&2
      exit 1
    fi
    if [[ "$(grep -Fxc -- 'running 1 test' <<<"${output}")" != 1 \
      || "$(grep -Fxc -- "test ${test_name} ... ok" <<<"${output}")" != 1 \
      || "$(grep -Ec -- '^test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; [0-9]+ filtered out; finished in .+$' <<<"${output}")" != 1 ]]; then
      printf 'protected public-lease test execution count differs: %s\n' "${test_name}" >&2
      exit 1
    fi
  done
}

run_protected_lease_tests
run_protected_lease_tests --release
