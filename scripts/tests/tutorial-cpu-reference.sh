#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
readonly REPO_ROOT
TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-cpu-reference-test.XXXXXXXX")"
readonly TEMP_ROOT
trap 'rm -r -- "${TEMP_ROOT}"' EXIT

FAKE_CARGO="${TEMP_ROOT}/cargo"
LOG="${TEMP_ROOT}/cargo.log"
readonly FAKE_CARGO LOG
cat >"${FAKE_CARGO}" <<'EOF'
#!/usr/bin/env bash
[[ "${FE2O3_HIP_SYS_DISABLE}" == 1 && "${FE2O3_HSA_RUNTIME_DISABLE}" == 1 ]] || exit 1
printf '%s\n' "$@" >"${FAKE_CARGO_LOG}"
EOF
chmod 700 "${FAKE_CARGO}"

FAKE_CARGO_LOG="${LOG}" CARGO="${FAKE_CARGO}" CARGO_FE2O3="${FAKE_CARGO}" \
  "${REPO_ROOT}/scripts/run-tutorial-cpu-reference.sh" \
  examples/vecadd/Cargo.toml lib
mapfile -t actual <"${LOG}"
expected=(fe2o3 test --locked --offline --all-targets \
  --manifest-path "${REPO_ROOT}/examples/vecadd/Cargo.toml" --lib)
[[ "${actual[*]}" == "${expected[*]}" ]]

FAKE_CARGO_LOG="${LOG}" CARGO="${FAKE_CARGO}" CARGO_FE2O3="${FAKE_CARGO}" \
  "${REPO_ROOT}/scripts/run-tutorial-cpu-reference.sh" \
  examples/gfx950_advanced_attention/Cargo.toml test reference
mapfile -t actual <"${LOG}"
expected=(fe2o3 test --locked --offline --all-targets --manifest-path \
  "${REPO_ROOT}/examples/gfx950_advanced_attention/Cargo.toml" --test reference)
[[ "${actual[*]}" == "${expected[*]}" ]]

if FAKE_CARGO_LOG="${LOG}" CARGO="${FAKE_CARGO}" CARGO_FE2O3="${FAKE_CARGO}" \
  "${REPO_ROOT}/scripts/run-tutorial-cpu-reference.sh" \
  examples/vecadd/Cargo.toml test '../hostile' 2>"${TEMP_ROOT}/expected-error"; then
  printf '%s\n' 'tutorial CPU reference test: invalid test name was accepted' >&2
  exit 1
fi
mapfile -t actual <"${LOG}"
[[ "${actual[*]}" == "${expected[*]}" ]]
