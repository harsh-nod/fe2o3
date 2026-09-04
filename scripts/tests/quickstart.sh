#!/usr/bin/env bash

set -Eeuo pipefail
umask 077

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
readonly REPO_ROOT
FIXTURE="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-quickstart-test.XXXXXXXX")"
readonly FIXTURE
trap 'rm -rf -- "${FIXTURE}"' EXIT
mkdir "${FIXTURE}/bin"
LOG="${FIXTURE}/cargo.log"
readonly LOG

cat >"${FIXTURE}/bin/cargo" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
[[ "${FE2O3_HIP_SYS_DISABLE:-}" == 1 ]] || {
  printf '%s\n' 'mock cargo: FE2O3_HIP_SYS_DISABLE is not enforced' >&2
  exit 91
}
[[ "${FE2O3_HSA_RUNTIME_DISABLE:-}" == 1 ]] || {
  printf '%s\n' 'mock cargo: FE2O3_HSA_RUNTIME_DISABLE is not enforced' >&2
  exit 92
}
printf 'cargo' >>"${QUICKSTART_TEST_LOG}"
printf ' %q' "$@" >>"${QUICKSTART_TEST_LOG}"
printf '\n' >>"${QUICKSTART_TEST_LOG}"

output=
simulator=0
doctor=0
for ((index = 1; index <= $#; index++)); do
  argument="${!index}"
  if [[ "${argument}" == --output ]]; then
    next=$((index + 1))
    output="${!next}"
  elif [[ "${argument}" == fe2o3-kir-sim ]]; then
    simulator=1
  elif [[ "${argument}" == cargo-fe2o3 ]]; then
    doctor=1
  fi
done
if [[ -n "${output}" ]]; then
  printf 'mock-bundle' >"${output}"
fi
if ((simulator)); then
  printf '%s\n' '{"schema":"fe2o3-simulation-result-v1","status":"ok","hardware_observed":false}'
fi
if ((doctor)) && [[ " $* " == *" doctor "* ]]; then
  printf '%s\n' \
    'direct-kfd-preflight: ready' \
    'device[0]: node=2 target=gfx942 wave-width=64 render=/dev/dri/renderD128 render-status=ready' \
    'runtime-libraries: HIP/HSA not-required-or-loaded'
fi
EOF
chmod 700 "${FIXTURE}/bin/cargo"

run_quickstart() {
  env \
    CARGO="${FIXTURE}/bin/cargo" \
    QUICKSTART_TEST_LOG="${LOG}" \
    PATH="${FIXTURE}/bin:/usr/bin:/bin" \
    bash "${REPO_ROOT}/scripts/quickstart.sh" "$@"
}

run_quickstart no-gpu >"${FIXTURE}/no-gpu.stdout" 2>"${FIXTURE}/no-gpu.stderr"
grep -F -- 'fe2o3-export-sim' "${LOG}" >/dev/null
grep -F -- 'build --locked --quiet -p rustc-codegen-fe2o3 --bin fe2o3-rustc-extract' \
  "${LOG}" >/dev/null
grep -F -- '--crate fe2o3_fill' "${LOG}" >/dev/null
grep -F -- '--bundle-version 1' "${LOG}" >/dev/null
grep -F -- '--package fe2o3-fill --lib' "${LOG}" >/dev/null
grep -F -- 'fe2o3-kir-sim' "${LOG}" >/dev/null
grep -F -- '"hardware_observed":false' "${FIXTURE}/no-gpu.stdout" >/dev/null
grep -F -- 'grants no compiler, artifact, GPU, or equivalence authority' \
  "${FIXTURE}/no-gpu.stderr" >/dev/null

: >"${LOG}"
run_quickstart doctor --require-tools-present >"${FIXTURE}/doctor.stdout"
grep -F -- 'doctor --require-tools-present' "${LOG}" >/dev/null
grep -F -- 'runtime-libraries: HIP/HSA not-required-or-loaded' \
  "${FIXTURE}/doctor.stdout" >/dev/null

: >"${LOG}"
run_quickstart source-check "${REPO_ROOT}/examples/vecadd/Cargo.toml"
[[ "$(wc -l <"${LOG}")" -eq 2 ]]
grep -F -- 'cargo-fe2o3 -- check --manifest-path' "${LOG}" >/dev/null
grep -F -- 'cargo-fe2o3 -- test --all-targets --manifest-path' "${LOG}" >/dev/null

: >"${LOG}"
set +e
run_quickstart gfx942-preflight >"${FIXTURE}/gfx942.stdout" 2>"${FIXTURE}/gfx942.stderr"
status=$?
set -e
[[ "${status}" -eq 3 ]]
grep -F -- 'doctor --require-gfx942' "${LOG}" >/dev/null
grep -F -- 'Worker V3 application route is not wired' "${FIXTURE}/gfx942.stderr" >/dev/null

set +e
run_quickstart simulate-source --crate bad --request "${FILL_REQUEST:-/missing}" \
  >"${FIXTURE}/invalid.stdout" 2>"${FIXTURE}/invalid.stderr"
status=$?
set -e
[[ "${status}" -eq 2 ]]
grep -F -- 'requires --crate, --request, and Cargo selection after --' \
  "${FIXTURE}/invalid.stderr" >/dev/null

: >"${LOG}"
run_quickstart simulate-source --crate fe2o3_fill \
  --request "${REPO_ROOT}/scripts/quickstart/fill-request.json" \
  --bundle-version 5 -- --package fe2o3-fill --lib \
  >"${FIXTURE}/v5.stdout" 2>"${FIXTURE}/v5.stderr"
grep -F -- '--bundle-version 5' "${LOG}" >/dev/null
grep -F -- '--bundle-v5' "${LOG}" >/dev/null

set +e
run_quickstart simulate-source --crate fe2o3_fill \
  --request "${REPO_ROOT}/scripts/quickstart/fill-request.json" \
  --bundle-version 4 -- --package fe2o3-fill --lib \
  >"${FIXTURE}/invalid-version.stdout" 2>"${FIXTURE}/invalid-version.stderr"
status=$?
set -e
[[ "${status}" -eq 2 ]]
grep -F -- '--bundle-version must be exactly 1 or 5' \
  "${FIXTURE}/invalid-version.stderr" >/dev/null

printf '%s\n' 'quickstart shell tests passed'
