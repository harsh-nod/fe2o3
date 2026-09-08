#!/usr/bin/env bash
set -euo pipefail

if [[ $# -lt 4 || $# -gt 5 ]]; then
    printf '%s\n' 'usage: run-tutorial-negative-test.sh ID MANIFEST TEST TEST-NAME [ignored]' >&2
    exit 2
fi

readonly test_id="$1"
readonly manifest="$2"
readonly test_target="$3"
readonly test_name="$4"
readonly mode="${5:-normal}"

if [[ "${mode}" != "normal" && "${mode}" != "ignored" ]]; then
    printf 'invalid negative test mode: %s\n' "${mode}" >&2
    exit 2
fi

readonly repository="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly scratch="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-negative-test.XXXXXX")"
cleanup() {
    rm -rf -- "${scratch}"
}
trap cleanup EXIT

unset CARGO_BUILD_RUSTC CARGO_ENCODED_RUSTFLAGS RUSTC RUSTC_WRAPPER
unset RUSTC_WORKSPACE_WRAPPER RUSTFLAGS
export CARGO_TERM_COLOR=never

arguments=(
    test --locked --offline --manifest-path "${manifest}"
    --test "${test_target}" -- "${test_name}" --exact --nocapture
)
if [[ "${mode}" == "ignored" ]]; then
    arguments+=(--ignored)
fi

if ! cargo "${arguments[@]}" >"${scratch}/stdout" 2>"${scratch}/stderr"; then
    cat -- "${scratch}/stdout" >&2
    cat -- "${scratch}/stderr" >&2
    exit 1
fi

printf 'negative test passed: %s\n' "${test_id}"
