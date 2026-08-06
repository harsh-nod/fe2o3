#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

readonly MAX_INPUT_BYTES=262144
readonly MAX_VERSION_LENGTH=256
readonly MAX_LINK_LENGTH=240
readonly SCRIPT_PATH="${BASH_SOURCE[0]}"
readonly SCRIPT_DIR="$(cd -- "${SCRIPT_PATH%/*}" && pwd)"
readonly DEFAULT_REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"

declare -ar SCALAR_KEYS=(
  schema_version
  git_commit
  git_dirty
  rustc_version
  llvm_version
  rocm_version
  driver_version
  device_target
  hardware_lane
)

declare -A SCALARS=()
declare -A ROW_LINKS=()

usage() {
  cat <<'EOF'
Usage:
  scripts/parity-evidence.sh collect --fixture INPUT.tsv
  scripts/parity-evidence.sh collect --rows ROWS.tsv --hardware-lane LANE [options]
  scripts/parity-evidence.sh validate EVIDENCE.tsv

Live collection options:
  --repo PATH             Git checkout to identify (default: this checkout)
  --rocm-path PATH        ROCm installation (default: /opt/rocm)
  --device-target TARGET  Exact AMD target; required if discovery is ambiguous
  --hardware-lane LANE    Stable, non-hostname lane identity
  --rows ROWS.tsv         Complete row-to-record link map

Fixture input contains the complete evidence fields and performs no Git,
toolchain, ROCm, driver, or device discovery. Output is canonical V1 TSV on
stdout. Row links are archive-relative records/<path>#<record-id> references.
This command reads evidence and never changes the normative parity status.
EOF
}

die() {
  printf 'parity evidence: %s\n' "$1" >&2
  exit 2
}

is_scalar_key() {
  local candidate="$1"
  local key
  for key in "${SCALAR_KEYS[@]}"; do
    [[ "${candidate}" == "${key}" ]] && return 0
  done
  return 1
}

valid_row_id() {
  local id="$1"
  if [[ "${id}" =~ ^[0-9][0-9]$ ]]; then
    ((10#${id} >= 1 && 10#${id} <= 94))
  else
    [[ "${id}" =~ ^S[0-9][0-9]$ ]] &&
      ((10#${id:1} >= 1 && 10#${id:1} <= 15))
  fi
}

valid_version() {
  local value="$1"
  ((${#value} >= 1 && ${#value} <= MAX_VERSION_LENGTH)) &&
    [[ "${value}" =~ ^[A-Za-z0-9][A-Za-z0-9._+:/,\;=@\(\)\ -]*[A-Za-z0-9\)]+$ ||
      "${value}" =~ ^[A-Za-z0-9]$ ]]
}

valid_row_link() {
  local link="$1"
  local path
  local segment
  local -a segments=()

  ((${#link} >= 1 && ${#link} <= MAX_LINK_LENGTH)) || return 1
  [[ "${link}" =~ ^records/[A-Za-z0-9][A-Za-z0-9._/-]*#[A-Za-z0-9][A-Za-z0-9._:-]*$ ]] ||
    return 1
  path="${link%%#*}"
  IFS=/ read -r -a segments <<<"${path}"
  for segment in "${segments[@]}"; do
    [[ -n "${segment}" && "${segment}" != . && "${segment}" != .. ]] || return 1
  done
}

validate_scalar() {
  local key="$1"
  local value="$2"

  case "${key}" in
    schema_version)
      [[ "${value}" == 1 ]] || die 'schema_version must be exactly 1'
      ;;
    git_commit)
      [[ "${value}" =~ ^[0-9a-f]{40}$ ]] ||
        die 'git_commit must be a lowercase 40-digit hash'
      ;;
    git_dirty)
      [[ "${value}" == true || "${value}" == false ]] ||
        die 'git_dirty must be exactly true or false'
      ;;
    rustc_version | llvm_version | rocm_version | driver_version)
      valid_version "${value}" || die "invalid or unbounded ${key}"
      ;;
    device_target)
      [[ "${value}" =~ ^gfx[0-9a-f]+(:[A-Za-z0-9_+-]+)*$ ]] ||
        die 'device_target must be a canonical AMD gfx target'
      ;;
    hardware_lane)
      ((${#value} >= 1 && ${#value} <= 64)) &&
        [[ "${value}" =~ ^[a-z0-9][a-z0-9._-]*$ ]] ||
        die 'hardware_lane must be a bounded lowercase lane identity'
      ;;
    *) die "unknown scalar field: ${key}" ;;
  esac
}

expected_row_id() {
  local row_index="$1"
  if ((row_index <= 94)); then
    printf '%02d' "${row_index}"
  else
    printf 'S%02d' "$((row_index - 94))"
  fi
}

parse_input() {
  local input="$1"
  local input_kind="$2"
  local require_canonical="$3"
  local line=""
  local key=""
  local value=""
  local extra=""
  local -a fields=()
  local line_number=0
  local byte_count=0
  local row_position=0
  local expected=""

  [[ -f "${input}" && -r "${input}" ]] || die "input is not a readable file: ${input}"

  while IFS= read -r line || [[ -n "${line}" ]]; do
    ((line_number += 1))
    ((byte_count += ${#line} + 1))
    ((byte_count <= MAX_INPUT_BYTES)) || die 'input exceeds the V1 size bound'
    [[ -n "${line}" && "${line}" != *$'\r'* ]] ||
      die "blank or carriage-return line at ${line_number}"

    IFS=$'\t' read -r -a fields <<<"${line}"
    key="${fields[0]:-}"
    if [[ "${key}" == row ]]; then
      ((${#fields[@]} == 3)) ||
        die "row line ${line_number} must contain exactly three fields"
      value="${fields[1]}"
      extra="${fields[2]}"
      valid_row_id "${value}" || die "unknown parity row ID: ${value}"
      valid_row_link "${extra}" || die "malformed row link for ${value}"
      [[ ! -v "ROW_LINKS[${value}]" ]] || die "duplicate parity row ID: ${value}"
      ROW_LINKS["${value}"]="${extra}"

      if [[ "${require_canonical}" == true ]]; then
        ((row_position += 1))
        expected="$(expected_row_id "${row_position}")"
        [[ "${value}" == "${expected}" ]] ||
          die "non-canonical row order: expected ${expected}, found ${value}"
        ((line_number > ${#SCALAR_KEYS[@]})) ||
          die 'row records must follow all scalar fields'
      fi
      continue
    fi

    [[ "${input_kind}" == complete ]] ||
      die "rows input contains non-row field: ${key}"
    is_scalar_key "${key}" || die "unknown field: ${key}"
    ((${#fields[@]} == 2)) ||
      die "scalar line ${line_number} must contain exactly two fields"
    value="${fields[1]}"
    [[ ! -v "SCALARS[${key}]" ]] || die "duplicate scalar field: ${key}"
    validate_scalar "${key}" "${value}"
    SCALARS["${key}"]="${value}"

    if [[ "${require_canonical}" == true ]]; then
      ((${#ROW_LINKS[@]} == 0)) || die 'scalar fields must precede row records'
      expected="${SCALAR_KEYS[$((line_number - 1))]:-}"
      [[ "${key}" == "${expected}" ]] ||
        die "non-canonical scalar order at line ${line_number}: expected ${expected:-row}"
    fi
  done <"${input}"

  ((line_number > 0)) || die 'input is empty'
}

require_complete() {
  local key
  local id
  local i

  for key in "${SCALAR_KEYS[@]}"; do
    [[ -v "SCALARS[${key}]" ]] || die "missing scalar field: ${key}"
  done
  ((${#SCALARS[@]} == ${#SCALAR_KEYS[@]})) || die 'unexpected scalar field count'

  for ((i = 1; i <= 109; i++)); do
    id="$(expected_row_id "${i}")"
    [[ -v "ROW_LINKS[${id}]" ]] || die "missing parity row link: ${id}"
  done
  ((${#ROW_LINKS[@]} == 109)) || die 'unexpected parity row count'
}

emit_canonical() {
  local key
  local id
  local i

  for key in "${SCALAR_KEYS[@]}"; do
    printf '%s\t%s\n' "${key}" "${SCALARS[${key}]}"
  done
  for ((i = 1; i <= 109; i++)); do
    id="$(expected_row_id "${i}")"
    printf 'row\t%s\t%s\n' "${id}" "${ROW_LINKS[${id}]}"
  done
}

require_command() {
  command -v "$1" >/dev/null 2>&1 || die "required command is unavailable: $1"
}

capture_output() {
  local name="$1"
  shift
  local output
  output="$("$@")" || die "could not collect ${name}"
  ((${#output} <= 1048576)) || die "${name} output exceeds the collection bound"
  printf '%s' "${output}"
}

first_line() {
  local input="$1"
  local line
  IFS=$'\n' read -r line _ <<<"${input}"
  printf '%s' "${line}"
}

normalize_version() {
  local value="$1"
  local normalized=""
  local word

  for word in ${value}; do
    if [[ -n "${normalized}" ]]; then
      normalized+=" "
    fi
    normalized+="${word}"
  done
  printf '%s' "${normalized}"
}

collect_rustc_identity() {
  local output="$1"
  local line
  local release=""
  local commit=""
  local host=""

  while IFS= read -r line; do
    case "${line}" in
      release:\ *) release="${line#release: }" ;;
      commit-hash:\ *) commit="${line#commit-hash: }" ;;
      host:\ *) host="${line#host: }" ;;
    esac
  done <<<"${output}"
  [[ -n "${release}" && "${commit}" =~ ^[0-9a-f]{40}$ && -n "${host}" ]] ||
    die 'rustc -Vv did not report release, commit-hash, and host'
  printf 'release=%s;commit=%s;host=%s' "${release}" "${commit}" "${host}"
}

discover_device_target() {
  local rocminfo_bin="$1"
  local output
  local line
  local candidate
  local selected=""

  output="$(capture_output rocminfo "${rocminfo_bin}")"
  while IFS= read -r line; do
    line="${line#"${line%%[![:space:]]*}"}"
    if [[ "${line}" =~ ^Name:[[:space:]]+(gfx[0-9a-f]+)$ ]]; then
      candidate="${BASH_REMATCH[1]}"
      if [[ -z "${selected}" ]]; then
        selected="${candidate}"
      elif [[ "${candidate}" != "${selected}" ]]; then
        die 'multiple AMD device targets found; pass --device-target explicitly'
      fi
    fi
  done <<<"${output}"
  [[ -n "${selected}" ]] || die 'rocminfo did not report an AMD gfx target'
  printf '%s' "${selected}"
}

collect_live() {
  local rows_file="$1"
  local repo_root="$2"
  local rocm_path="$3"
  local device_target="$4"
  local hardware_lane="$5"
  local output
  local dirty
  local clang_bin
  local rocminfo_bin
  local version_file
  local version_line=""
  local kernel_release

  [[ -n "${rows_file}" ]] || die '--rows is required for live collection'
  [[ -n "${hardware_lane}" ]] || die '--hardware-lane is required for live collection'
  validate_scalar hardware_lane "${hardware_lane}"
  [[ -d "${repo_root}" ]] || die "repository path does not exist: ${repo_root}"
  [[ "${rocm_path}" != *$'\n'* && -n "${rocm_path}" ]] || die 'invalid ROCm path'

  require_command git
  require_command rustc
  SCALARS[schema_version]=1
  output="$(capture_output 'Git commit' git -C "${repo_root}" rev-parse --verify HEAD)"
  SCALARS[git_commit]="$(first_line "${output}")"
  validate_scalar git_commit "${SCALARS[git_commit]}"

  output="$(capture_output 'Git status' git -C "${repo_root}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)"
  dirty=false
  [[ -z "${output}" ]] || dirty=true
  SCALARS[git_dirty]="${dirty}"

  output="$(capture_output 'rustc identity' rustc -Vv)"
  SCALARS[rustc_version]="$(collect_rustc_identity "${output}")"

  clang_bin="${rocm_path}/llvm/bin/clang"
  if [[ ! -x "${clang_bin}" ]]; then
    require_command clang
    clang_bin="$(command -v clang)"
  fi
  output="$(capture_output 'LLVM identity' "${clang_bin}" --version)"
  SCALARS[llvm_version]="$(normalize_version "$(first_line "${output}")")"

  version_file="${rocm_path}/.info/version"
  if [[ -r "${version_file}" ]]; then
    IFS= read -r version_line <"${version_file}" || true
  fi
  if [[ -z "${version_line}" ]]; then
    rocminfo_bin="${rocm_path}/bin/rocminfo"
    [[ -x "${rocminfo_bin}" ]] || die 'ROCm version file and rocminfo are unavailable'
    output="$(capture_output 'ROCm identity' "${rocminfo_bin}" --version)"
    version_line="$(first_line "${output}")"
  fi
  SCALARS[rocm_version]="$(normalize_version "${version_line}")"

  require_command uname
  kernel_release="$(capture_output 'kernel release' uname -r)"
  if [[ -r /sys/module/amdgpu/version ]]; then
    IFS= read -r version_line </sys/module/amdgpu/version || true
    SCALARS[driver_version]="amdgpu/${version_line};kernel=${kernel_release}"
  elif [[ -d /sys/module/amdgpu ]]; then
    SCALARS[driver_version]="amdgpu/builtin;kernel=${kernel_release}"
  elif [[ -d /sys/module/dxgkrnl ]]; then
    SCALARS[driver_version]="dxgkrnl/builtin;kernel=${kernel_release}"
  else
    die 'no supported AMD GPU driver module identity is available'
  fi

  if [[ -z "${device_target}" ]]; then
    rocminfo_bin="${rocm_path}/bin/rocminfo"
    [[ -x "${rocminfo_bin}" ]] || die 'rocminfo is required to discover the device target'
    device_target="$(discover_device_target "${rocminfo_bin}")"
  fi
  SCALARS[device_target]="${device_target}"
  SCALARS[hardware_lane]="${hardware_lane}"

  local key
  for key in "${SCALAR_KEYS[@]}"; do
    validate_scalar "${key}" "${SCALARS[${key}]}"
  done
  parse_input "${rows_file}" rows false
  require_complete
  emit_canonical
}

collect_main() {
  local fixture=""
  local rows_file=""
  local repo_root="${DEFAULT_REPO_ROOT}"
  local rocm_path="/opt/rocm"
  local device_target=""
  local hardware_lane=""

  while (($# > 0)); do
    case "$1" in
      --fixture | --rows | --repo | --rocm-path | --device-target | --hardware-lane)
        (($# >= 2)) || die "$1 requires a value"
        case "$1" in
          --fixture) fixture="$2" ;;
          --rows) rows_file="$2" ;;
          --repo) repo_root="$2" ;;
          --rocm-path) rocm_path="$2" ;;
          --device-target) device_target="$2" ;;
          --hardware-lane) hardware_lane="$2" ;;
        esac
        shift 2
        ;;
      -h | --help)
        usage
        return 0
        ;;
      *) die "unknown collect option: $1" ;;
    esac
  done

  if [[ -n "${fixture}" ]]; then
    [[ -z "${rows_file}${device_target}${hardware_lane}" &&
      "${repo_root}" == "${DEFAULT_REPO_ROOT}" && "${rocm_path}" == /opt/rocm ]] ||
      die '--fixture cannot be combined with live collection options'
    parse_input "${fixture}" complete false
    require_complete
    emit_canonical
    return
  fi

  collect_live "${rows_file}" "${repo_root}" "${rocm_path}" \
    "${device_target}" "${hardware_lane}"
}

validate_main() {
  (($# == 1)) || die 'validate requires exactly one evidence file'
  parse_input "$1" complete true
  require_complete
  printf 'parity evidence is canonical: 109 row records\n'
}

main() {
  local command="${1:-}"
  [[ -n "${command}" ]] || {
    usage >&2
    return 2
  }
  shift

  case "${command}" in
    collect) collect_main "$@" ;;
    validate) validate_main "$@" ;;
    -h | --help | help) usage ;;
    *)
      usage >&2
      die "unknown command: ${command}"
      ;;
  esac
}

main "$@"
