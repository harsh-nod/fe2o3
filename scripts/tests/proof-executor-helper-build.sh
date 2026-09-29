#!/usr/bin/env bash
# Command/inspection contracts only: Cargo, readelf and nm are inert mocks.
# No real ELF, helper execution, secure bootstrap or proof admission is exercised.
set -euo pipefail

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly scratch="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-proof-helper-build.XXXXXXXX")"
trap 'rm -rf -- "${scratch}"' EXIT
readonly fixture="${scratch}/fixture repo"
readonly builder="${fixture}/scripts/build-static-proof-executor-helper.sh"
readonly binary="fe2o3-proof-executor-helper"
mkdir -p -- "${fixture}/scripts" "${scratch}/bin"
# Substitute inspection tools only in the disposable script copy, not production.
sed 's@/usr/bin/readelf@"${PROOF_HELPER_TEST_BIN}/readelf"@g; s@/usr/bin/nm@"${PROOF_HELPER_TEST_BIN}/nm"@g' \
  "${repo_root}/scripts/build-static-proof-executor-helper.sh" >"${builder}"

fail() {
  printf 'proof-executor helper build test failed: %s\n' "$*" >&2
  exit 1
}

cat >"${scratch}/bin/cargo" <<'EOF'
#!/bin/bash
set -euo pipefail
printf '%s\n' "${PWD}" "${CARGO_TARGET_DIR}" "$@" >"${PROOF_HELPER_TEST_LOG}"
mkdir -p -- "${CARGO_TARGET_DIR}"
EOF

cat >"${scratch}/bin/readelf" <<'EOF'
#!/bin/bash
set -euo pipefail
[[ $# -eq 6 && "${*:1:5}" == '-hW -lW -dW -sW --' && "$6" == "${PROOF_HELPER_TEST_IMAGE}" ]]
case "${PROOF_HELPER_TEST_CASE}" in
  readelf-error) exit 90 ;;
  elf32) printf 'Class: ELF32\n' ;;
  *) printf 'Class: ELF64\n' ;;
esac
case "${PROOF_HELPER_TEST_CASE}" in
  pie) printf 'Type: DYN\n' ;;
  *) printf 'Type: EXEC\n' ;;
esac
case "${PROOF_HELPER_TEST_CASE}" in
  missing-entry) ;;
  malformed-entry) printf 'Entry point address: invalid\n' ;;
  duplicate-entry) printf 'Entry point address: 0x401000\nEntry point address: 0x401000\n' ;;
  *) printf 'Entry point address: 0x401000\n' ;;
esac
case "${PROOF_HELPER_TEST_CASE}" in
  executable-stack) printf 'GNU_STACK 0 0 0 0 0 RWE 0x10\n' ;;
  missing-stack) ;;
  *) printf 'GNU_STACK 0 0 0 0 0 RW 0x10\n' ;;
esac
case "${PROOF_HELPER_TEST_CASE}" in
  interp) printf 'INTERP\n' ;;
  dynamic) printf 'DYNAMIC\n' ;;
  needed) printf '0x1 (NEEDED) Shared library: [libc.so]\n' ;;
  rpath) printf '0xf (RPATH) Library rpath: [/tmp]\n' ;;
  runpath) printf '0x1d (RUNPATH) Library runpath: [/tmp]\n' ;;
esac
EOF

cat >"${scratch}/bin/nm" <<'EOF'
#!/bin/bash
set -euo pipefail
case "$1" in
  -n)
    [[ $# -eq 4 && "$2" == --defined-only && "$3" == -- && "$4" == "${PROOF_HELPER_TEST_IMAGE}" ]]
    case "${PROOF_HELPER_TEST_CASE}" in
      nm-error) exit 90 ;;
      missing-symbol) ;;
      malformed-symbol) printf 'invalid T fe2o3_secure_start_v1\n' ;;
      mismatched-entry) printf '0000000000402000 T fe2o3_secure_start_v1\n' ;;
      duplicate-symbol) printf '0000000000401000 T fe2o3_secure_start_v1\n%.0s' 1 2 ;;
      *) printf '0000000000401000 T fe2o3_secure_start_v1\n' ;;
    esac
    ;;
  -u)
    [[ $# -eq 3 && "$2" == -- && "$3" == "${PROOF_HELPER_TEST_IMAGE}" ]]
    case "${PROOF_HELPER_TEST_CASE}" in
      undefined) printf 'U unresolved_symbol\n' ;;
      undefined-error) exit 90 ;;
    esac
    ;;
  *) exit 90 ;;
esac
EOF
chmod 0700 "${scratch}/bin/cargo" "${scratch}/bin/readelf" "${scratch}/bin/nm"

readonly -a fixture_env=(
  "PATH=${scratch}/bin:/usr/bin:/bin"
  "PROOF_HELPER_TEST_BIN=${scratch}/bin"
  "PROOF_HELPER_TEST_LOG=${scratch}/cargo.args"
  "CARGO_TARGET_DIR=${scratch}/ignored ambient target"
)

for argument in '' --help v1 --debug; do
  rm -f -- "${scratch}/cargo.args"
  status=0
  env -i "${fixture_env[@]}" bash "${builder}" "${argument}" \
    >"${scratch}/out" 2>"${scratch}/err" || status=$?
  [[ ${status} -eq 2 && ! -e "${scratch}/cargo.args" && ! -s "${scratch}/out" ]] ||
    fail "invalid argument reached a build: ${argument}"
  grep -Fq 'usage:' "${scratch}/err" || fail 'missing usage on invalid argument'
done

for mode in default empty override; do
  target_dir="${fixture}/target/static-proof-executor-helper"
  target_environment=()
  case "${mode}" in
    empty) target_environment+=(FE2O3_STATIC_PROOF_EXECUTOR_HELPER_TARGET_DIR=) ;;
    override)
      target_dir="${scratch}/custom target"
      target_environment+=("FE2O3_STATIC_PROOF_EXECUTOR_HELPER_TARGET_DIR=${target_dir}")
      ;;
  esac
  image="${target_dir}/x86_64-unknown-linux-musl/release/${binary}"
  env -i "${fixture_env[@]}" "${target_environment[@]}" \
    PROOF_HELPER_TEST_CASE=valid "PROOF_HELPER_TEST_IMAGE=${image}" \
    bash "${builder}" >"${scratch}/out" 2>"${scratch}/err" || fail "${mode} valid image"
  printf '%s\n' "${fixture}" "${target_dir}" rustc \
    --locked --release --target x86_64-unknown-linux-musl \
    -p fe2o3-verifier --bin "${binary}" -- \
    -C target-feature=+crt-static -C relocation-model=static \
    -C link-arg=-static -C link-arg=-no-pie \
    -C link-arg=-Wl,-e,fe2o3_secure_start_v1 >"${scratch}/expected"
  diff -u -- "${scratch}/expected" "${scratch}/cargo.args" || fail "${mode} build arguments"
  [[ "$(<"${scratch}/out")" == "${image}" && ! -s "${scratch}/err" ]] || fail "${mode} output"
done

for scenario in elf32 pie missing-entry malformed-entry duplicate-entry \
  missing-symbol malformed-symbol duplicate-symbol mismatched-entry \
  interp dynamic needed rpath runpath executable-stack missing-stack \
  undefined readelf-error nm-error undefined-error; do
  status=0
  env -i "${fixture_env[@]}" "${target_environment[@]}" \
    "PROOF_HELPER_TEST_CASE=${scenario}" "PROOF_HELPER_TEST_IMAGE=${image}" \
    bash "${builder}" >"${scratch}/out" 2>"${scratch}/err" || status=$?
  [[ ${status} -ne 0 && ! -s "${scratch}/out" ]] || fail "accepted ${scenario}"
done

printf 'proof helper build/ELF-inspection mock tests passed (no build or helper execution)\n'
