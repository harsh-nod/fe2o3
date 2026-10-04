#!/usr/bin/env bash

set -Eeuo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR

command="${1:-}"
[[ -n "${command}" ]] || {
  printf 'usage: %s <validate|run> [options]\n' "$0" >&2
  exit 2
}
shift
case "${command}" in
  validate)
    exec "${SCRIPT_DIR}/parity-row-evidence.sh" validate-queue "$@"
    ;;
  run)
    exec "${SCRIPT_DIR}/parity-row-evidence.sh" queue-run "$@"
    ;;
  *)
    printf 'unknown MI300X evidence queue command: %s\n' "${command}" >&2
    exit 2
    ;;
esac
