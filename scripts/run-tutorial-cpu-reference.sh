#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
CARGO_COMMAND="${CARGO:-cargo}"
readonly CARGO_COMMAND
CARGO_FE2O3_COMMAND="${CARGO_FE2O3:-${REPO_ROOT}/target/debug/cargo-fe2o3}"
readonly CARGO_FE2O3_COMMAND

usage() {
  printf '%s\n' \
    'Usage: scripts/run-tutorial-cpu-reference.sh MANIFEST (lib | test TEST_NAME)'
}

main() {
  (($# == 2 || $# == 3)) || {
    usage >&2
    return 2
  }
  local manifest="$1" mode="$2" test_name="${3:-}"
  [[ "${manifest}" == examples/*/Cargo.toml && "${manifest}" != *..* ]] || {
    printf 'tutorial CPU reference: invalid manifest path: %s\n' "${manifest}" >&2
    return 2
  }
  manifest="$(realpath --canonicalize-existing -- "${REPO_ROOT}/${manifest}")"
  [[ "${manifest}" == "${REPO_ROOT}"/* && -f "${manifest}" && ! -L "${manifest}" ]] || {
    printf 'tutorial CPU reference: manifest is not a regular repository file: %s\n' \
      "${manifest}" >&2
    return 2
  }
  command -v "${CARGO_COMMAND}" >/dev/null 2>&1 || {
    printf 'tutorial CPU reference: Cargo command is unavailable: %s\n' \
      "${CARGO_COMMAND}" >&2
    return 2
  }

  local -a selection=(--lib)
  case "${mode}" in
    lib)
      [[ -z "${test_name}" ]] || {
        usage >&2
        return 2
      }
      ;;
    test)
      [[ "${test_name}" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]] || {
        printf 'tutorial CPU reference: invalid integration-test name: %s\n' \
          "${test_name}" >&2
        return 2
      }
      selection=(--test "${test_name}")
      ;;
    *)
      usage >&2
      return 2
      ;;
  esac

  cd -- "${REPO_ROOT}"
  if [[ ! -x "${CARGO_FE2O3_COMMAND}" ]]; then
    "${CARGO_COMMAND}" build --locked --offline --quiet \
      -p cargo-fe2o3 --bin cargo-fe2o3
  fi
  [[ -x "${CARGO_FE2O3_COMMAND}" && -f "${CARGO_FE2O3_COMMAND}" && ! -L "${CARGO_FE2O3_COMMAND}" ]] || {
    printf 'tutorial CPU reference: cargo-fe2o3 is not a regular executable: %s\n' \
      "${CARGO_FE2O3_COMMAND}" >&2
    return 2
  }
  FE2O3_HIP_SYS_DISABLE=1 FE2O3_HSA_RUNTIME_DISABLE=1 \
    "${CARGO_FE2O3_COMMAND}" fe2o3 test --locked --offline --all-targets \
      --manifest-path "${manifest}" "${selection[@]}"
}

main "$@"
