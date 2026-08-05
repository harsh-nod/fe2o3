#!/usr/bin/env bash

set -Eeuo pipefail

readonly SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly SSH_BIN="${FE2O3_SSH:-ssh}"

usage() {
  cat <<'EOF'
Usage:
  scripts/remote-hardware-matrix.sh [options] \
    --entry SSH_HOST FE2O3_TARGET REMOTE_CHECKOUT [--entry ...]

Options:
  --commit COMMIT       Exact 40-digit commit required on every host.
                        Defaults to the local checkout's HEAD.
  --rocm-path PATH      Remote ROCm installation (default: /opt/rocm).
  --gpu-device PATH     Remote GPU device node. By default, auto-detect
                        /dev/kfd or /dev/dxg.
  --entry HOST TARGET CHECKOUT
                        Add one remote host, AMD target, and checkout path.
  -h, --help            Show this help.

Examples:
  scripts/remote-hardware-matrix.sh \
    --entry mi300x gfx942 /path/to/fe2o3 \
    --entry mi350 gfx950 /path/to/fe2o3

SSH authentication and host policy come from the caller's normal SSH
configuration. No credentials are accepted by this script.
EOF
}

die() {
  printf 'remote hardware matrix: %s\n' "$1" >&2
  exit 2
}

valid_host() {
  [[ "$1" =~ ^[A-Za-z0-9][A-Za-z0-9_.@-]*$ ]]
}

valid_target() {
  [[ "$1" =~ ^gfx[0-9a-f]+(:[A-Za-z0-9_+-]+)*$ ]]
}

emit_remote_payload() {
  local commit="$1"
  local target="$2"
  local checkout="$3"
  local rocm_path="$4"
  local gpu_device="$5"

  printf 'set -- %q %q %q %q %q\n' \
    "${commit}" "${target}" "${checkout}" "${rocm_path}" "${gpu_device}"
  cat <<'REMOTE_SCRIPT'
set -Eeuo pipefail

readonly expected_commit="$1"
readonly target="$2"
readonly checkout="$3"
readonly rocm_path="$4"
readonly requested_gpu_device="$5"
stage="bootstrap"

report_exit() {
  local status=$?
  trap - EXIT
  if ((status == 0)); then
    printf 'FE2O3_MATRIX_RESULT\tPASS\tcomplete\t0\n'
  else
    printf 'FE2O3_MATRIX_RESULT\tFAIL\t%s\t%d\n' "${stage}" "${status}"
  fi
  exit "${status}"
}
trap report_exit EXIT

fail() {
  local status="$1"
  shift
  printf 'remote hardware matrix: %s\n' "$*" >&2
  exit "${status}"
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || \
    fail 20 "required command is unavailable: $1"
}

check_clean_checkout() {
  local dirty
  if ! dirty="$(
    git -C "${checkout}" status --porcelain=v1 \
      --untracked-files=all --ignore-submodules=none
  )"; then
    fail 14 "could not inspect checkout state: ${checkout}"
  fi
  [[ -z "${dirty}" ]] || fail 15 "remote checkout is dirty: ${checkout}"
}

export ROCM_PATH="${rocm_path}"
export PATH="${HOME}/.cargo/bin:${ROCM_PATH}/bin:${PATH}"
export FE2O3_TARGET="${target}"

stage="checkout"
require_command git
[[ -n "${checkout}" ]] || fail 10 'remote checkout path is empty'
if [[ "$(git -C "${checkout}" rev-parse --is-inside-work-tree 2>/dev/null)" != true ]]; then
  fail 11 "not a Git checkout: ${checkout}"
fi

stage="exact-commit"
actual_commit="$(git -C "${checkout}" rev-parse --verify HEAD 2>/dev/null)" || \
  fail 12 "could not resolve remote HEAD: ${checkout}"
[[ "${actual_commit}" == "${expected_commit}" ]] || \
  fail 13 "wrong remote commit: expected ${expected_commit}, found ${actual_commit}"

stage="clean-checkout"
check_clean_checkout

stage="target"
[[ "${target}" =~ ^gfx[0-9a-f]+(:[A-Za-z0-9_+-]+)*$ ]] || \
  fail 16 "invalid or missing AMD target: ${target}"
readonly processor="${target%%:*}"

stage="toolchain"
require_command bash
require_command cargo
require_command rustc
require_command rocminfo
[[ -x "${checkout}/scripts/ci-local.sh" ]] || \
  fail 21 "missing executable scripts/ci-local.sh in ${checkout}"
(cd -- "${checkout}" && cargo --version >/dev/null) || \
  fail 22 'the pinned Cargo toolchain is unavailable'
(cd -- "${checkout}" && rustc --version >/dev/null) || \
  fail 23 'the pinned Rust toolchain is unavailable'

stage="gpu-access"
gpu_device="${requested_gpu_device}"
if [[ "${gpu_device}" == auto ]]; then
  if [[ -e /dev/kfd ]]; then
    gpu_device=/dev/kfd
  elif [[ -e /dev/dxg ]]; then
    gpu_device=/dev/dxg
    export HSA_ENABLE_DXG_DETECTION=1
  else
    fail 24 'no AMD GPU device node found (/dev/kfd or /dev/dxg)'
  fi
fi
[[ -r "${gpu_device}" && -w "${gpu_device}" ]] || \
  fail 25 "GPU device is not readable and writable: ${gpu_device}"

stage="gpu-target"
if ! gpu_info="$(rocminfo 2>&1)"; then
  fail 26 'rocminfo could not enumerate an AMD GPU'
fi
if ! awk -v expected="${processor}" \
  '$1 == "Name:" && $2 == expected { found = 1 } END { exit !found }' \
  <<<"${gpu_info}"; then
  fail 27 "configured target ${processor} was not reported by rocminfo"
fi

cd -- "${checkout}"

stage="rocm-compile"
bash scripts/ci-local.sh rocm-compile

stage="hardware-smoke"
FE2O3_ALLOW_GPU_SMOKE=1 bash scripts/ci-local.sh hardware-smoke

stage="hsaco-inspection"
readonly hsaco="${checkout}/target/fe2o3/vecadd.hsaco"
[[ -s "${hsaco}" ]] || fail 30 "generated HSACO is missing or empty: ${hsaco}"
case "${processor}" in
  gfx9*) readonly wavefront=64 ;;
  gfx*) readonly wavefront=32 ;;
  *) fail 31 "cannot derive wavefront size for ${target}" ;;
esac
FE2O3_TEST_HSACO="${hsaco}" \
FE2O3_TEST_TARGET="${target}" \
FE2O3_TEST_WAVEFRONT="${wavefront}" \
  cargo test --locked -p fe2o3-hsaco --test inspection \
    inspects_real_generated_vecadd_hsaco -- --ignored --exact

stage="post-run-cleanliness"
check_clean_checkout
REMOTE_SCRIPT
}

main() {
  local commit=""
  local rocm_path="/opt/rocm"
  local gpu_device="auto"
  local -a hosts=()
  local -a targets=()
  local -a checkouts=()
  local -a result_statuses=()
  local -a result_stages=()
  local -a result_exits=()
  local -a result_logs=()

  while (($# > 0)); do
    case "$1" in
      --commit)
        (($# >= 2)) || die '--commit requires a value'
        commit="$2"
        shift 2
        ;;
      --rocm-path)
        (($# >= 2)) || die '--rocm-path requires a value'
        rocm_path="$2"
        shift 2
        ;;
      --gpu-device)
        (($# >= 2)) || die '--gpu-device requires a value'
        gpu_device="$2"
        shift 2
        ;;
      --entry)
        (($# >= 4)) || die '--entry requires HOST TARGET CHECKOUT'
        valid_host "$2" || die "invalid SSH host: $2"
        valid_target "$3" || die "invalid AMD target: $3"
        [[ -n "$4" && "$4" != *$'\n'* ]] || die 'invalid remote checkout path'
        hosts+=("$2")
        targets+=("$3")
        checkouts+=("$4")
        shift 4
        ;;
      -h | --help)
        usage
        return 0
        ;;
      *) die "unknown argument: $1" ;;
    esac
  done

  ((${#hosts[@]} > 0)) || die 'at least one --entry is required'
  [[ -n "${rocm_path}" && "${rocm_path}" != *$'\n'* ]] || \
    die 'invalid remote ROCm path'
  [[ -n "${gpu_device}" && "${gpu_device}" != *$'\n'* ]] || \
    die 'invalid remote GPU device path'

  if [[ -z "${commit}" ]]; then
    commit="$(git -C "${REPO_ROOT}" rev-parse --verify HEAD)" || \
      die 'could not resolve the local checkout HEAD'
  fi
  [[ "${commit}" =~ ^[0-9a-f]{40}$ ]] || \
    die "commit must be exactly 40 lowercase hexadecimal digits: ${commit}"

  local log_dir="${FE2O3_MATRIX_LOG_DIR:-${REPO_ROOT}/target/remote-hardware-matrix/${commit}}"
  mkdir -p -- "${log_dir}"

  printf 'Remote hardware matrix commit=%s hosts=%d\n' \
    "${commit}" "${#hosts[@]}"

  local overall_status=0
  local index
  for ((index = 0; index < ${#hosts[@]}; index++)); do
    local host="${hosts[index]}"
    local target="${targets[index]}"
    local checkout="${checkouts[index]}"
    local safe_host="${host//@/_at_}"
    local log_file
    printf -v log_file '%s/%02d-%s-%s.log' \
      "${log_dir}" "$((index + 1))" "${safe_host}" "${target%%:*}"

    printf 'RUN host=%s target=%s\n' "${host}" "${target}"
    set +e
    emit_remote_payload \
      "${commit}" "${target}" "${checkout}" "${rocm_path}" "${gpu_device}" |
      "${SSH_BIN}" -- "${host}" bash -s >"${log_file}" 2>&1
    local -a pipeline_status=("${PIPESTATUS[@]}")
    set -e

    local producer_status="${pipeline_status[0]}"
    local ssh_status="${pipeline_status[1]}"
    local marker_outcome=""
    local marker_stage=""
    local marker_exit=""
    local marker outcome stage status remainder
    while IFS=$'\t' read -r marker outcome stage status remainder; do
      if [[ "${marker}" == FE2O3_MATRIX_RESULT ]]; then
        marker_outcome="${outcome}"
        marker_stage="${stage}"
        marker_exit="${status}"
      fi
    done <"${log_file}"

    local result_status=FAIL
    local result_stage=ssh
    local result_exit="${ssh_status}"
    if ((producer_status != 0)); then
      result_stage=local-payload
      result_exit="${producer_status}"
    elif [[ "${marker_outcome}" == PASS && "${marker_stage}" == complete &&
      "${marker_exit}" == 0 && "${ssh_status}" == 0 ]]; then
      result_status=PASS
      result_stage=complete
      result_exit=0
    elif [[ "${marker_outcome}" == FAIL && -n "${marker_stage}" &&
      "${marker_exit}" =~ ^[0-9]+$ ]]; then
      result_stage="${marker_stage}"
      result_exit="${marker_exit}"
    elif ((ssh_status == 0)); then
      result_stage=protocol
      result_exit=1
    fi

    if [[ "${result_status}" != PASS ]]; then
      overall_status=1
    fi
    result_statuses+=("${result_status}")
    result_stages+=("${result_stage}")
    result_exits+=("${result_exit}")
    result_logs+=("${log_file}")
  done

  printf '\nSummary\n'
  for ((index = 0; index < ${#hosts[@]}; index++)); do
    printf 'RESULT host=%s target=%s status=%s stage=%s exit=%s log=%q\n' \
      "${hosts[index]}" "${targets[index]}" "${result_statuses[index]}" \
      "${result_stages[index]}" "${result_exits[index]}" "${result_logs[index]}"
  done

  return "${overall_status}"
}

main "$@"
