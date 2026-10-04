#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

readonly SCRIPT_PATH="${BASH_SOURCE[0]}"
readonly SCRIPT_DIR="$(cd -- "${SCRIPT_PATH%/*}" && pwd)"
readonly DEFAULT_REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly EVIDENCE_SCRIPT="${SCRIPT_DIR}/parity-evidence.sh"
readonly DEFAULT_TIMEOUT_SECONDS=7200
readonly MAX_TIMEOUT_SECONDS=86400
readonly MAX_PATH_BYTES=2048

declare -ar CORE_SHARDS=(Q1 Q2 Q3 Q4 Q5 Q6 Q7)

usage() {
  cat <<'EOF'
Usage:
  scripts/run-parity-snapshot.sh list
  scripts/run-parity-snapshot.sh <dry-run|run|verify-only> \
    --repo PATH --archive-root PATH [options]

Options:
  --shard NAME             Select one Q1-Q7 shard; repeat to select several
  --gfx942-compile         Add the optional gfx942 compile shard
  --gfx942-hardware        Fail closed: the Worker V2 hardware shard is retired
  --verus PATH             Absolute MIR/PLIRON Verus executable for Q7
  --runtime-model-verus PATH
                           Absolute runtime-model Verus executable for Q7
  --timeout-seconds N      Per-shard bound, 1..86400 (default: 7200)
  --path PATH              Exact absolute-only PATH recorded for commands
  --home PATH              Exact HOME recorded for commands
  --cargo-home PATH        Exact CARGO_HOME recorded for commands
  --rustup-home PATH       Exact RUSTUP_HOME recorded for commands

With no --shard option, Q1-Q7 are selected. Optional gfx942 lanes are never
selected implicitly. Every run shard receives distinct CARGO_TARGET_DIR,
TMPDIR, CI_LOG_DIR, output, log, and result-record paths under the archive.
The runner never changes parity declarations, status, matrix, or dashboard.
EOF
}

die() {
  printf 'parity snapshot: %s\n' "$1" >&2
  exit 2
}

valid_shard() {
  case "$1" in
    Q1 | Q2 | Q3 | Q4 | Q5 | Q6 | Q7 | GFX942-COMPILE) return 0 ;;
    *) return 1 ;;
  esac
}

shard_slug() {
  printf '%s' "${1,,}" | tr _ -
}

valid_uint() {
  [[ "$1" =~ ^(0|[1-9][0-9]*)$ ]]
}

valid_path_value() {
  local value="$1"
  ((${#value} >= 1 && ${#value} <= MAX_PATH_BYTES)) &&
    [[ "${value}" == /* && "${value}" != *$'\n'* && "${value}" != *$'\t'* ]]
}

validate_path_list() {
  local value="$1"
  local entry
  local -a entries=()

  [[ "${value}" != *$'\n'* && "${value}" != *$'\t'* ]] ||
    die 'PATH must not contain tabs or newlines'
  IFS=: read -r -a entries <<<"${value}"
  ((${#entries[@]} > 0 && ${#entries[@]} <= 128)) || die 'PATH entry count is out of bounds'
  for entry in "${entries[@]}"; do
    valid_path_value "${entry}" || die 'PATH must contain only bounded absolute entries'
  done
}

path_is_within() {
  local candidate="$1"
  local parent="$2"
  [[ "${candidate}" == "${parent}" || "${candidate}" == "${parent}/"* ]]
}

absolute_directory() {
  local path="$1"
  [[ -d "${path}" ]] || die "directory does not exist: ${path}"
  realpath -- "${path}"
}

git_identity() {
  local repo="$1"
  local commit
  local status

  commit="$(git -C "${repo}" rev-parse --verify HEAD)" || die 'could not identify Git commit'
  [[ "${commit}" =~ ^[0-9a-f]{40}$ ]] || die 'Git commit is not a full lowercase hash'
  if git -C "${repo}" symbolic-ref -q HEAD >/dev/null 2>&1; then
    die 'repository must be detached'
  fi
  status="$(git -C "${repo}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)" ||
    die 'could not inspect Git status'
  [[ -z "${status}" ]] || die 'repository must be clean'
  printf '%s' "${commit}"
}

find_tool() {
  local name="$1"
  local search_path="$2"
  local found

  found="$(PATH="${search_path}" command -v -- "${name}" 2>/dev/null || true)"
  [[ -n "${found}" && "${found}" == /* && -f "${found}" && -x "${found}" ]] ||
    die "required executable is unavailable on the recorded PATH: ${name}"
  printf '%s' "${found}"
}

shell_join() {
  local result=""
  local argument
  local quoted
  for argument in "$@"; do
    printf -v quoted '%q' "${argument}"
    [[ -z "${result}" ]] || result+=' '
    result+="${quoted}"
  done
  printf '%s' "${result}"
}

append_command() {
  local -n destination="$1"
  shift
  local line
  line="$(shell_join "$@")"
  [[ -z "${destination}" ]] || destination+=$'\n'
  destination+="${line}"
}

build_shard_command() {
  local shard="$1"
  local bash_bin="$2"
  local cargo_bin="$3"
  local command=""

  case "${shard}" in
    Q1)
      append_command command "${bash_bin}" scripts/ci-local.sh workspace-test
      append_command command "${bash_bin}" scripts/parity-matrix.sh check
      append_command command "${bash_bin}" scripts/tests/parity-matrix.sh
      append_command command "${bash_bin}" scripts/parity-dashboard.sh check
      append_command command "${bash_bin}" scripts/tests/parity-dashboard.sh
      append_command command "${bash_bin}" scripts/tests/parity-evidence.sh
      ;;
    Q2)
      append_command command "${cargo_bin}" test -p rustc-codegen-fe2o3 --locked --lib
      ;;
    Q3)
      append_command command "${cargo_bin}" test -p dialect-mir --locked
      append_command command "${cargo_bin}" test -p fe2o3-kernel-ir --locked
      append_command command "${cargo_bin}" test -p fe2o3-kernel-analysis --locked
      append_command command "${cargo_bin}" test -p dialect-amdgcn --locked
      ;;
    Q4)
      append_command command "${cargo_bin}" test -p fe2o3-artifacts --locked
      append_command command "${cargo_bin}" test -p fe2o3-artifact-transaction --locked
      append_command command "${cargo_bin}" test -p fe2o3-hsaco --locked
      append_command command "${cargo_bin}" test -p fe2o3-hsaco-finalize --locked
      ;;
    Q5)
      append_command command "${cargo_bin}" test -p fe2o3-core --locked
      append_command command "${cargo_bin}" test -p fe2o3-hip-sys --locked
      append_command command "${cargo_bin}" test -p fe2o3-device --locked
      append_command command "${cargo_bin}" test -p fe2o3-completion --locked
      append_command command "${cargo_bin}" test -p fe2o3-verifier --locked
      append_command command "${cargo_bin}" test -p fe2o3-host --locked
      append_command command "${cargo_bin}" test -p fe2o3-hsa-runtime --locked
      ;;
    Q6)
      append_command command "${cargo_bin}" test -p cargo-fe2o3 --locked
      append_command command "${cargo_bin}" test -p fe2o3-differential --locked
      append_command command "${bash_bin}" scripts/tests/differential-conformance.sh
      ;;
    Q7)
      append_command command "${bash_bin}" scripts/ci-local.sh verus
      ;;
    GFX942-COMPILE)
      append_command command "${bash_bin}" scripts/ci-local.sh rocm-compile
      ;;
    *) die "unknown shard: ${shard}" ;;
  esac
  printf '%s' "${command}"
}

shard_uses_cargo() {
  [[ "$1" != Q7 ]]
}

hex_encode() {
  local value="$1"
  if [[ -z "${value}" ]]; then
    printf '%s' -
  else
    printf '%s' "${value}" | od -An -v -tx1 | tr -d ' \n'
  fi
}

list_shards() {
  cat <<'EOF'
Q1	core	workspace tests and parity validators
Q2	core	rustc codegen tests
Q3	core	MIR, Kernel IR, kernel analysis, and AMDGPU lowering tests
Q4	core	artifact, transaction, HSACO, and finalization tests
Q5	core	core, HIP, device, completion, verifier, host, and HSA tests
Q6	core	Cargo integration and differential conformance tests
Q7	core	positive and negative Verus proof fixtures
GFX942-COMPILE	optional	gfx942 production ROCm compilation
GFX942-HARDWARE	unavailable	retired Worker V2 evidence has no production replacement
EOF
}

main() {
  local mode="${1:-}"
  [[ -n "${mode}" ]] || { usage >&2; return 2; }
  shift || true

  if [[ "${mode}" == list ]]; then
    (($# == 0)) || die 'list takes no options'
    list_shards
    return 0
  fi
  case "${mode}" in
    dry-run | run | verify-only) ;;
    -h | --help | help) usage; return 0 ;;
    *) usage >&2; return 2 ;;
  esac

  local repo="${DEFAULT_REPO_ROOT}"
  local archive_root=""
  local timeout_seconds="${DEFAULT_TIMEOUT_SECONDS}"
  local recorded_path="${PATH:-}"
  local recorded_home="${HOME:-}"
  local cargo_home="${CARGO_HOME:-${recorded_home}/.cargo}"
  local rustup_home="${RUSTUP_HOME:-${recorded_home}/.rustup}"
  local verus=""
  local runtime_model_verus=""
  local explicit_selection=false
  local want_gfx942_compile=false
  local want_gfx942_hardware=false
  local shard=""
  local commit=""
  local bash_bin=""
  local cargo_bin=""
  local timeout_bin=""
  local slug=""
  local command=""
  local record_relative=""
  local log_relative=""
  local target_dir=""
  local tmp_dir=""
  local output_dir=""
  local index=0
  local -a selected=()
  local -a core_selection=()
  local -a outer_argv=()
  local -A seen=()

  while (($# > 0)); do
    case "$1" in
      --repo | --archive-root | --timeout-seconds | --path | --home | --cargo-home | --rustup-home | --verus | --runtime-model-verus | --shard)
        (($# >= 2)) || die "$1 requires a value"
        case "$1" in
          --repo) repo="$2" ;;
          --archive-root) archive_root="$2" ;;
          --timeout-seconds) timeout_seconds="$2" ;;
          --path) recorded_path="$2" ;;
          --home) recorded_home="$2" ;;
          --cargo-home) cargo_home="$2" ;;
          --rustup-home) rustup_home="$2" ;;
          --verus) verus="$2" ;;
          --runtime-model-verus) runtime_model_verus="$2" ;;
          --shard)
            valid_shard "$2" || die "unknown shard: $2"
            [[ "$2" == Q[1-7] ]] || die '--shard accepts only Q1 through Q7'
            explicit_selection=true
            core_selection+=("$2")
            ;;
        esac
        shift 2
        ;;
      --gfx942-compile) want_gfx942_compile=true; shift ;;
      --gfx942-hardware) want_gfx942_hardware=true; shift ;;
      -h | --help) usage; return 0 ;;
      *) die "unknown option: $1" ;;
    esac
  done

  [[ "${want_gfx942_hardware}" == false ]] ||
    die 'the gfx942 hardware shard is unavailable: its retired Worker V2 evidence has no production replacement'
  [[ -n "${archive_root}" ]] || die '--archive-root is required'
  valid_uint "${timeout_seconds}" && ((timeout_seconds >= 1 && timeout_seconds <= MAX_TIMEOUT_SECONDS)) ||
    die '--timeout-seconds must be an integer from 1 through 86400'
  validate_path_list "${recorded_path}"
  valid_path_value "${recorded_home}" || die 'HOME must be a bounded absolute path'
  valid_path_value "${cargo_home}" || die 'CARGO_HOME must be a bounded absolute path'
  valid_path_value "${rustup_home}" || die 'RUSTUP_HOME must be a bounded absolute path'

  repo="$(absolute_directory "${repo}")"
  archive_root="$(absolute_directory "${archive_root}")"
  path_is_within "${archive_root}" "${repo}" && die 'archive root must be outside the repository'
  path_is_within "${repo}" "${archive_root}" && die 'archive root must not contain the repository'
  commit="$(git_identity "${repo}")"

  if [[ "${explicit_selection}" == true ]]; then
    selected=("${core_selection[@]}")
  else
    selected=("${CORE_SHARDS[@]}")
  fi
  [[ "${want_gfx942_compile}" == false ]] || selected+=(GFX942-COMPILE)
  for shard in "${selected[@]}"; do
    [[ ! -v "seen[${shard}]" ]] || die "duplicate shard selection: ${shard}"
    seen["${shard}"]=1
  done

  if [[ "${mode}" == verify-only ]]; then
    for shard in "${selected[@]}"; do
      slug="$(shard_slug "${shard}")"
      "${EVIDENCE_SCRIPT}" verify-record --repo "${repo}" \
        --archive-root "${archive_root}" "records/${slug}.tsv"
    done
    printf 'parity snapshot: verified %d independent shard record(s) at %s\n' \
      "${#selected[@]}" "${commit}"
    return 0
  fi

  bash_bin="$(find_tool bash "${recorded_path}")"
  cargo_bin="$(find_tool cargo "${recorded_path}")"
  timeout_bin="$(find_tool timeout "${recorded_path}")"
  if [[ -v 'seen[Q7]' ]]; then
    valid_path_value "${verus}" && [[ -f "${verus}" && -x "${verus}" ]] ||
      die 'Q7 requires --verus with an absolute executable path'
    valid_path_value "${runtime_model_verus}" &&
      [[ -f "${runtime_model_verus}" && -x "${runtime_model_verus}" ]] ||
      die 'Q7 requires --runtime-model-verus with an absolute executable path'
  fi
  if [[ "${mode}" == dry-run ]]; then
    printf 'snapshot_plan_schema_version\t1\n'
    printf 'git_commit\t%s\n' "${commit}"
    printf 'repo\t%s\n' "${repo}"
    printf 'archive_root\t%s\n' "${archive_root}"
    printf 'timeout_seconds\t%s\n' "${timeout_seconds}"
  fi

  if [[ "${mode}" == run ]]; then
    for shard in "${selected[@]}"; do
      slug="$(shard_slug "${shard}")"
      [[ ! -e "${archive_root}/records/${slug}.tsv" &&
        ! -L "${archive_root}/records/${slug}.tsv" ]] ||
        die "result record already exists: records/${slug}.tsv"
      [[ ! -e "${archive_root}/logs/${slug}.log" &&
        ! -L "${archive_root}/logs/${slug}.log" ]] ||
        die "result log already exists: logs/${slug}.log"
      [[ ! -e "${archive_root}/work/${slug}" &&
        ! -L "${archive_root}/work/${slug}" ]] ||
        die "shard work directory already exists: work/${slug}"
    done
  fi

  for shard in "${selected[@]}"; do
    slug="$(shard_slug "${shard}")"
    record_relative="records/${slug}.tsv"
    log_relative="logs/${slug}.log"
    target_dir="${archive_root}/work/${slug}/target"
    tmp_dir="${archive_root}/work/${slug}/tmp"
    output_dir="${archive_root}/work/${slug}/output"
    command="$(build_shard_command "${shard}" "${bash_bin}" "${cargo_bin}")"
    outer_argv=(
      "${timeout_bin}" --foreground --signal=TERM --kill-after=30
      "${timeout_seconds}" "${bash_bin}" -Eeuo pipefail -c "${command}"
    )

    if [[ "${mode}" == dry-run ]]; then
      printf 'shard\t%s\t%s\t%s\t%s\t%s\t%s\n' \
        "${shard}" "${record_relative}" "${log_relative}" \
        "${target_dir}" "${tmp_dir}" "${output_dir}"
      printf 'environment\t%s\tCARGO_HOME\t%s\n' "${shard}" "$(hex_encode "${cargo_home}")"
      printf 'environment\t%s\tCARGO_TARGET_DIR\t%s\n' "${shard}" "$(hex_encode "${target_dir}")"
      printf 'environment\t%s\tCI_LOG_DIR\t%s\n' "${shard}" "$(hex_encode "${output_dir}/ci-logs")"
      printf 'environment\t%s\tFE2O3_EVIDENCE_OUTPUT_DIR\t%s\n' "${shard}" "$(hex_encode "${output_dir}")"
      printf 'environment\t%s\tHOME\t%s\n' "${shard}" "$(hex_encode "${recorded_home}")"
      printf 'environment\t%s\tLC_ALL\t%s\n' "${shard}" "$(hex_encode C)"
      printf 'environment\t%s\tPATH\t%s\n' "${shard}" "$(hex_encode "${recorded_path}")"
      printf 'environment\t%s\tRUSTUP_HOME\t%s\n' "${shard}" "$(hex_encode "${rustup_home}")"
      printf 'environment\t%s\tTMPDIR\t%s\n' "${shard}" "$(hex_encode "${tmp_dir}")"
      if [[ "${shard}" == Q7 ]]; then
        printf 'environment\t%s\tFE2O3_RUNTIME_MODEL_VERUS\t%s\n' \
          "${shard}" "$(hex_encode "${runtime_model_verus}")"
        printf 'environment\t%s\tVERUS\t%s\n' "${shard}" "$(hex_encode "${verus}")"
      fi
      [[ "${shard}" != GFX942-COMPILE ]] ||
        printf 'environment\t%s\tFE2O3_TARGET\t%s\n' "${shard}" "$(hex_encode gfx942)"
      printf 'tool\t%s\tbash\t%s\n' "${shard}" "${bash_bin}"
      shard_uses_cargo "${shard}" && printf 'tool\t%s\tcargo\t%s\n' "${shard}" "${cargo_bin}"
      printf 'tool\t%s\tcommand\t%s\n' "${shard}" "${timeout_bin}"
      if [[ "${shard}" == Q7 ]]; then
        printf 'tool\t%s\truntime-model-verus\t%s\n' \
          "${shard}" "${runtime_model_verus}"
        printf 'tool\t%s\tverus\t%s\n' "${shard}" "${verus}"
      fi
      for index in "${!outer_argv[@]}"; do
        printf 'argv\t%s\t%04d\t%s\n' \
          "${shard}" "${index}" "$(hex_encode "${outer_argv[${index}]}")"
      done
      continue
    fi

    mkdir -p -- "${target_dir}" "${tmp_dir}" "${output_dir}/ci-logs"

    local -a record_args=(
      record --repo "${repo}" --archive-root "${archive_root}"
      --record "${record_relative}" --log "${log_relative}"
      --env "PATH=${recorded_path}"
      --env "HOME=${recorded_home}"
      --env "CARGO_HOME=${cargo_home}"
      --env "RUSTUP_HOME=${rustup_home}"
      --env "CARGO_TARGET_DIR=${target_dir}"
      --env "TMPDIR=${tmp_dir}"
      --env "CI_LOG_DIR=${output_dir}/ci-logs"
      --env "FE2O3_EVIDENCE_OUTPUT_DIR=${output_dir}"
      --tool "bash=${bash_bin}"
    )
    if shard_uses_cargo "${shard}"; then
      record_args+=(--tool "cargo=${cargo_bin}")
    fi
    if [[ "${shard}" == Q7 ]]; then
      record_args+=(
        --env "FE2O3_RUNTIME_MODEL_VERUS=${runtime_model_verus}"
        --env "VERUS=${verus}"
        --tool "runtime-model-verus=${runtime_model_verus}"
        --tool "verus=${verus}"
      )
    fi
    if [[ "${shard}" == GFX942-COMPILE ]]; then
      record_args+=(--env FE2O3_TARGET=gfx942)
    fi
    record_args+=(-- "${outer_argv[@]}")

    printf 'parity snapshot: running %s -> %s\n' "${shard}" "${record_relative}"
    "${EVIDENCE_SCRIPT}" "${record_args[@]}"
    "${EVIDENCE_SCRIPT}" verify-record --repo "${repo}" \
      --archive-root "${archive_root}" "${record_relative}"
  done

  printf 'parity snapshot: recorded and verified %d independent shard(s) at %s\n' \
    "${#selected[@]}" "${commit}"
}

main "$@"
