#!/usr/bin/env bash
# Selection/command contracts only: Cargo, image checks and the executable are inert mocks.
# This does not validate a real ELF image, protected boot, V3 handoff or GPU execution.
set -euo pipefail

readonly repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly scratch="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-conditional-issuer-build.XXXXXXXX")"
trap 'rm -rf -- "${scratch}"' EXIT
readonly fixture="${scratch}/fixture repo"
readonly builder="${fixture}/scripts/build-static-compiler-execution-issuer.sh"
readonly log="${scratch}/commands"
mkdir -p -- "${fixture}/scripts" "${scratch}/bin"
cp -- "${repo_root}/scripts/build-static-compiler-execution-issuer.sh" "${builder}"

fail() {
  printf 'conditional issuer build selection test failed: %s\n' "$*" >&2
  exit 1
}

cat >"${scratch}/record.sh" <<'EOF'
record_call() {
  printf '%s' "$1"
  shift
  printf ' %q' "$@"
  printf '\n'
}
record_cargo() {
  record_call cargo "$@"
  printf 'cwd %q\ntarget_dir %q\n' "${PWD}" "${CARGO_TARGET_DIR-}"
  local variable
  for variable in \
    FE2O3_STATIC_COMPILER_EXECUTION_ISSUER \
    FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE \
    FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_CONDITIONAL; do
    printf '%s %q\n' "${variable}" "${!variable-}"
  done
}
EOF
source "${scratch}/record.sh"

cat >"${scratch}/bin/cargo" <<'EOF'
#!/bin/bash
set -euo pipefail
source "${ISSUER_BUILD_TEST_RECORD}"
record_cargo "$@" >>"${ISSUER_BUILD_TEST_LOG}"
case "${1-}" in
  rustc)
    mkdir -p -- "$(dirname -- "${ISSUER_BUILD_TEST_IMAGE}")"
    cp -- "${ISSUER_BUILD_TEST_SMOKE}" "${ISSUER_BUILD_TEST_IMAGE}"
    chmod 0700 "${ISSUER_BUILD_TEST_IMAGE}"
    ;;
  test) ;;
  *) exit 90 ;;
esac
EOF
chmod 0700 "${scratch}/bin/cargo"

cat >"${fixture}/scripts/check-static-compiler-execution-issuer-image.sh" <<'EOF'
#!/bin/bash
set -euo pipefail
source "${ISSUER_BUILD_TEST_RECORD}"
record_call image-check "$@" >>"${ISSUER_BUILD_TEST_LOG}"
EOF

cat >"${scratch}/smoke.sh" <<'EOF'
#!/bin/bash
set -euo pipefail
printf 'inert smoke stub only\n' >"$0.smoke"
exit 1
EOF

readonly -a fixture_env=(
  "PATH=${scratch}/bin:/usr/bin:/bin"
  "ISSUER_BUILD_TEST_RECORD=${scratch}/record.sh"
  "ISSUER_BUILD_TEST_LOG=${log}"
  "ISSUER_BUILD_TEST_SMOKE=${scratch}/smoke.sh"
  "CARGO_TARGET_DIR=${scratch}/ignored ambient target"
  "FE2O3_STATIC_COMPILER_EXECUTION_ISSUER=inherited v1 image"
  "FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE=inherited v2 image"
  "FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_CONDITIONAL=inherited v3 image"
)

refuses() {
  local status
  : >"${log}"
  if env -i "${fixture_env[@]}" bash "${builder}" "$@" \
    >"${scratch}/out" 2>"${scratch}/err"; then
    fail "accepted invalid arguments: $*"
  else
    status=$?
  fi
  [[ ${status} -eq 2 ]] || fail "wrong rejection status for: $*"
  grep -Fq -- '[--native|--conditional]' "${scratch}/err" || fail 'missing selector usage'
  [[ ! -s "${log}" && ! -s "${scratch}/out" ]] || fail 'invalid selector reached a command'
}

refuses ''
refuses --
refuses --infer-family
refuses conditional
refuses v3
refuses --conditional=true
refuses --native=true
refuses --native --conditional
refuses --conditional --native
refuses --conditional --conditional
refuses --native --native
refuses --conditional extra
refuses --native extra

check_selection() {
  local label="$1" binary="$2" default_dir="$3" target_variable="$4"
  local image_variable="$5" image_test="$6"
  shift 6
  local mode target_dir image variable
  local -a target_environment
  for mode in default empty override; do
    target_dir="${fixture}/target/${default_dir}"
    if [[ "${mode}" == override ]]; then
      target_dir="${scratch}/custom ${label} target"
    fi
    image="${target_dir}/x86_64-unknown-linux-musl/release/${binary}"
    target_environment=()
    for variable in \
      FE2O3_STATIC_ISSUER_TARGET_DIR \
      FE2O3_STATIC_NATIVE_ISSUER_TARGET_DIR \
      FE2O3_STATIC_CONDITIONAL_ISSUER_TARGET_DIR; do
      if [[ "${variable}" != "${target_variable}" ]]; then
        target_environment+=("${variable}=${scratch}/unused ${variable}")
      elif [[ "${mode}" == override ]]; then
        target_environment+=("${variable}=${target_dir}")
      elif [[ "${mode}" == empty ]]; then
        target_environment+=("${variable}=")
      fi
    done
    : >"${log}"
    rm -f -- "${image}.smoke"
    env -i "${fixture_env[@]}" "${target_environment[@]}" \
      "ISSUER_BUILD_TEST_IMAGE=${image}" bash "${builder}" "$@" \
      >"${scratch}/out" 2>"${scratch}/err" || fail "${label}/${mode} command routing"

    (
      cd -- "${fixture}"
      export FE2O3_STATIC_COMPILER_EXECUTION_ISSUER='inherited v1 image'
      export FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE='inherited v2 image'
      export FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_CONDITIONAL='inherited v3 image'
      CARGO_TARGET_DIR="${target_dir}" record_cargo rustc \
        --locked --release --target x86_64-unknown-linux-musl \
        -p fe2o3-compiler-execution-issuer --bin "${binary}" -- \
        -C target-feature=+crt-static -C relocation-model=static \
        -C link-arg=-static -C link-arg=-no-pie \
        -C link-arg=-Wl,-e,fe2o3_secure_start_v1
      record_call image-check "${image}" "${target_dir}/${binary}.readelf.txt"
      printf -v "${image_variable}" '%s' "${image}"
      CARGO_TARGET_DIR="${target_dir}/profile-test" record_cargo test \
        --locked -p fe2o3-compiler-execution-issuer --test static_image \
        "${image_test}" -- --exact --ignored
    ) >"${scratch}/expected"
    diff -u -- "${scratch}/expected" "${log}" || fail "${label}/${mode} argument/environment mismatch"
    [[ "$(<"${scratch}/out")" == "${image}" && ! -s "${scratch}/err" ]] ||
      fail "${label}/${mode} selected image output"
    [[ -f "${image}.smoke" ]] || fail "${label}/${mode} inert smoke stub was not invoked"
  done
}

check_selection v1 fe2o3-compiler-execution-issuer static-issuer \
  FE2O3_STATIC_ISSUER_TARGET_DIR FE2O3_STATIC_COMPILER_EXECUTION_ISSUER \
  release_image_is_loader_independent_static_elf
check_selection v2 fe2o3-compiler-execution-issuer-native static-native-issuer \
  FE2O3_STATIC_NATIVE_ISSUER_TARGET_DIR FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_NATIVE \
  native_release_image_is_loader_independent_static_elf --native
check_selection v3 fe2o3-compiler-execution-issuer-conditional static-conditional-issuer \
  FE2O3_STATIC_CONDITIONAL_ISSUER_TARGET_DIR FE2O3_STATIC_COMPILER_EXECUTION_ISSUER_CONDITIONAL \
  conditional_release_image_is_loader_independent_static_elf --conditional

printf 'issuer selector/argument/environment mock tests passed (no protected boot exercised)\n'
