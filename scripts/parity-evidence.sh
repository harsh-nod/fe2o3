#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

readonly MAX_INPUT_BYTES=262144
readonly MAX_VERSION_LENGTH=256
readonly MAX_LINK_LENGTH=240
readonly MAX_RESULT_RECORD_BYTES=1048576
readonly MAX_RESULT_ITEMS=1024
readonly SCRIPT_PATH="${BASH_SOURCE[0]}"
SCRIPT_DIR="$(cd -- "${SCRIPT_PATH%/*}" && pwd)"
readonly SCRIPT_DIR
DEFAULT_REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly DEFAULT_REPO_ROOT

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
  scripts/parity-evidence.sh record --repo PATH --archive-root PATH \
    --record RECORD.tsv --log LOG.txt [options] -- /absolute/command [args...]
  scripts/parity-evidence.sh verify-record --repo PATH --archive-root PATH RECORD.tsv

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

Result record options:
  --env NAME[=VALUE]      Add one variable to the otherwise empty command environment
  --tool NAME=/abs/path  Bind an additional tool binary by SHA-256
  --artifact NAME=PATH   Bind an archive-relative regular file by size and SHA-256

The record command requires a clean detached checkout. RECORD, LOG, and artifact
paths are relative to an archive root outside that checkout. It writes a V1
record even when the command fails, then returns the command's exit status.
Legacy collect/validate declarations remain canonical V1 declarations and do
not constitute command-result evidence.
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
      if ((${#value} < 1 || ${#value} > 64)) ||
        [[ ! "${value}" =~ ^[a-z0-9][a-z0-9._-]*$ ]]; then
        die 'hardware_lane must be a bounded lowercase lane identity'
      fi
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

valid_name() {
  local value="$1"
  ((${#value} >= 1 && ${#value} <= 64)) &&
    [[ "${value}" =~ ^[A-Za-z_][A-Za-z0-9_]*$ ]]
}

valid_label() {
  local value="$1"
  ((${#value} >= 1 && ${#value} <= 64)) &&
    [[ "${value}" =~ ^[a-z][a-z0-9._-]*$ ]]
}

valid_relative_path() {
  local value="$1"
  local segment
  local -a segments=()

  ((${#value} >= 1 && ${#value} <= 512)) || return 1
  [[ "${value}" != /* && "${value}" != *$'\n'* && "${value}" != *$'\t'* ]] || return 1
  [[ "${value}" != *//* && "${value}" != */ ]] || return 1
  [[ "${value}" =~ ^[A-Za-z0-9][A-Za-z0-9._/+:-]*$ ]] || return 1
  IFS=/ read -r -a segments <<<"${value}"
  for segment in "${segments[@]}"; do
    [[ -n "${segment}" && "${segment}" != . && "${segment}" != .. ]] || return 1
  done
}

valid_absolute_tool_path() {
  local value="$1"
  local segment
  local -a segments=()

  ((${#value} >= 2 && ${#value} <= 1024)) || return 1
  [[ "${value}" =~ ^/[A-Za-z0-9._/+:-]+$ ]] || return 1
  [[ "${value}" != *//* && "${value}" != */ ]] || return 1
  IFS=/ read -r -a segments <<<"${value}"
  for segment in "${segments[@]}"; do
    [[ "${segment}" != . && "${segment}" != .. ]] || return 1
  done
}

valid_sha256() {
  [[ "$1" =~ ^[0-9a-f]{64}$ ]]
}

valid_uint() {
  [[ "$1" =~ ^(0|[1-9][0-9]*)$ ]]
}

hex_encode() {
  local value="$1"
  local encoded

  if [[ -z "${value}" ]]; then
    printf '%s' -
    return
  fi
  encoded="$(printf '%s' "${value}" | od -An -v -tx1 | tr -d ' \n')"
  [[ -n "${encoded}" ]] || die 'could not encode result-record value'
  printf '%s' "${encoded}"
}

valid_hex_value() {
  [[ "$1" == - || "$1" =~ ^([0-9a-f]{2})+$ ]]
}

sha256_file() {
  local output
  output="$(sha256sum -- "$1")" || die "could not hash file: $1"
  printf '%s' "${output%% *}"
}

absolute_existing_directory() {
  local path="$1"
  local resolved
  [[ -d "${path}" ]] || die "directory does not exist: ${path}"
  resolved="$(realpath -- "${path}")" || die "could not resolve directory: ${path}"
  printf '%s' "${resolved}"
}

path_is_within() {
  local candidate="$1"
  local parent="$2"
  [[ "${candidate}" == "${parent}" || "${candidate}" == "${parent}/"* ]]
}

archive_path() {
  local root="$1"
  local relative="$2"
  local resolved

  valid_relative_path "${relative}" || die "invalid archive-relative path: ${relative}"
  resolved="$(realpath -m -- "${root}/${relative}")" ||
    die "could not resolve archive path: ${relative}"
  path_is_within "${resolved}" "${root}" || die "archive path escapes root: ${relative}"
  printf '%s' "${resolved}"
}

require_regular_archive_file() {
  local root="$1"
  local relative="$2"
  local absolute

  absolute="$(archive_path "${root}" "${relative}")"
  [[ -f "${absolute}" && ! -L "${absolute}" ]] ||
    die "archive entry is missing or not a regular file: ${relative}"
  path_is_within "$(realpath -- "${absolute}")" "${root}" ||
    die "archive entry resolves outside root: ${relative}"
  printf '%s' "${absolute}"
}

git_identity() {
  local repo="$1"
  local phase="$2"
  local output

  output="$(git -C "${repo}" rev-parse --verify HEAD)" ||
    die "could not identify repository ${phase} command"
  [[ "${output}" =~ ^[0-9a-f]{40}$ ]] || die "invalid Git commit ${phase} command"
  if git -C "${repo}" symbolic-ref -q HEAD >/dev/null 2>&1; then
    die "repository must be detached ${phase} command"
  fi
  output="$(git -C "${repo}" status --porcelain=v1 --untracked-files=all --ignore-submodules=none)" ||
    die "could not inspect repository ${phase} command"
  [[ -z "${output}" ]] || die "repository must be clean ${phase} command"
  git -C "${repo}" rev-parse --verify HEAD
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

record_main() {
  local repo=""
  local archive_root=""
  local record_relative=""
  local log_relative=""
  local name=""
  local value=""
  local command_path=""
  local record_absolute=""
  local log_absolute=""
  local temp_record=""
  local commit_before=""
  local commit_after=""
  local exit_status=0
  local absolute=""
  local digest=""
  local index=0
  local -a command_argv=()
  local -a environment_assignments=()
  local -a names=()
  local -A environment=([LC_ALL]=C)
  local -A tools=()
  local -A tool_digests=()
  local -A artifacts=()

  while (($# > 0)); do
    case "$1" in
      --repo | --archive-root | --record | --log | --env | --tool | --artifact)
        (($# >= 2)) || die "$1 requires a value"
        case "$1" in
          --repo) repo="$2" ;;
          --archive-root) archive_root="$2" ;;
          --record) record_relative="$2" ;;
          --log) log_relative="$2" ;;
          --env)
            if [[ "$2" == *=* ]]; then
              name="${2%%=*}"
              value="${2#*=}"
            else
              name="$2"
              valid_name "${name}" || die "invalid environment name: ${name}"
              [[ -v "${name}" ]] || die "environment variable is unset: ${name}"
              value="${!name}"
            fi
            valid_name "${name}" || die "invalid environment name: ${name}"
            [[ ! -v "environment[${name}]" ]] ||
              die "duplicate environment variable: ${name}"
            environment["${name}"]="${value}"
            ;;
          --tool)
            [[ "$2" == *=* ]] || die 'tool must use NAME=VALUE syntax'
            name="${2%%=*}"
            value="${2#*=}"
            valid_label "${name}" || die "invalid tool label: ${name}"
            [[ "${name}" != command ]] || die 'tool label command is reserved'
            [[ ! -v "tools[${name}]" ]] || die "duplicate tool label: ${name}"
            valid_absolute_tool_path "${value}" ||
              die "tool path must be a bounded absolute path: ${name}"
            tools["${name}"]="${value}"
            ;;
          --artifact)
            [[ "$2" == *=* ]] || die 'artifact must use NAME=VALUE syntax'
            name="${2%%=*}"
            value="${2#*=}"
            valid_label "${name}" || die "invalid artifact label: ${name}"
            [[ ! -v "artifacts[${name}]" ]] || die "duplicate artifact label: ${name}"
            valid_relative_path "${value}" ||
              die "invalid artifact archive path: ${name}"
            artifacts["${name}"]="${value}"
            ;;
        esac
        shift 2
        ;;
      --)
        shift
        command_argv=("$@")
        break
        ;;
      -h | --help)
        usage
        return 0
        ;;
      *) die "unknown record option: $1" ;;
    esac
  done

  [[ -n "${repo}" ]] || die '--repo is required for result recording'
  [[ -n "${archive_root}" ]] || die '--archive-root is required for result recording'
  [[ -n "${record_relative}" ]] || die '--record is required for result recording'
  [[ -n "${log_relative}" ]] || die '--log is required for result recording'
  ((${#command_argv[@]} > 0)) || die 'record requires a command after --'
  ((${#command_argv[@]} <= MAX_RESULT_ITEMS)) || die 'command argument count exceeds V1 bound'

  require_command git
  require_command realpath
  require_command sha256sum
  require_command stat
  require_command od
  require_command tr
  require_command sort
  require_command env

  repo="$(absolute_existing_directory "${repo}")"
  archive_root="$(absolute_existing_directory "${archive_root}")"
  path_is_within "${archive_root}" "${repo}" &&
    die 'archive root must be outside the repository checkout'
  path_is_within "${repo}" "${archive_root}" &&
    die 'archive root must not contain the repository checkout'

  command_path="${command_argv[0]}"
  valid_absolute_tool_path "${command_path}" ||
    die 'record command must use a bounded absolute executable path'
  [[ -f "${command_path}" && -x "${command_path}" ]] ||
    die "record command is not an executable file: ${command_path}"
  tools[command]="${command_path}"

  ((${#environment[@]} <= MAX_RESULT_ITEMS)) || die 'environment count exceeds V1 bound'
  ((${#tools[@]} <= MAX_RESULT_ITEMS)) || die 'tool count exceeds V1 bound'
  ((${#artifacts[@]} <= MAX_RESULT_ITEMS)) || die 'artifact count exceeds V1 bound'

  record_absolute="$(archive_path "${archive_root}" "${record_relative}")"
  log_absolute="$(archive_path "${archive_root}" "${log_relative}")"
  [[ "${record_absolute}" != "${log_absolute}" ]] || die 'record and log paths must differ'
  [[ ! -e "${record_absolute}" && ! -L "${record_absolute}" ]] ||
    die "record already exists: ${record_relative}"
  [[ ! -e "${log_absolute}" && ! -L "${log_absolute}" ]] ||
    die "log already exists: ${log_relative}"

  for name in "${!tools[@]}"; do
    [[ -f "${tools[${name}]}" && -x "${tools[${name}]}" ]] ||
      die "tool is not an executable file: ${name}"
    tool_digests["${name}"]="$(sha256_file "${tools[${name}]}")"
  done

  commit_before="$(git_identity "${repo}" before)"
  mkdir -p -- "${record_absolute%/*}" "${log_absolute%/*}"
  mapfile -t names < <(printf '%s\n' "${!environment[@]}" | sort)
  for name in "${names[@]}"; do
    environment_assignments+=("${name}=${environment[${name}]}")
  done

  set +e
  (
    cd -- "${repo}"
    env -i "${environment_assignments[@]}" "${command_argv[@]}"
  ) >"${log_absolute}" 2>&1
  exit_status=$?
  set -e
  command sync -f -- "${log_absolute}" 2>/dev/null || true

  commit_after="$(git_identity "${repo}" after)"
  [[ "${commit_after}" == "${commit_before}" ]] ||
    die 'repository commit changed while recording result'
  for name in "${!tools[@]}"; do
    [[ -f "${tools[${name}]}" && -x "${tools[${name}]}" ]] ||
      die "tool disappeared while recording result: ${name}"
    [[ "$(sha256_file "${tools[${name}]}")" == "${tool_digests[${name}]}" ]] ||
      die "tool changed while recording result: ${name}"
  done

  for name in "${!artifacts[@]}"; do
    require_regular_archive_file "${archive_root}" "${artifacts[${name}]}" >/dev/null
  done

  temp_record="$(mktemp -- "${record_absolute}.tmp.XXXXXX")"
  {
    printf 'record_schema_version\t1\n'
    printf 'git_commit\t%s\n' "${commit_before}"
    printf 'git_detached\ttrue\n'
    printf 'git_clean_before\ttrue\n'
    printf 'git_clean_after\ttrue\n'
    printf 'argv_count\t%d\n' "${#command_argv[@]}"
    for index in "${!command_argv[@]}"; do
      printf 'argv\t%04d\t%s\n' "${index}" "$(hex_encode "${command_argv[${index}]}")"
    done
    printf 'environment_count\t%d\n' "${#environment[@]}"
    mapfile -t names < <(printf '%s\n' "${!environment[@]}" | sort)
    for name in "${names[@]}"; do
      printf 'environment\t%s\t%s\n' "${name}" "$(hex_encode "${environment[${name}]}")"
    done
    printf 'tool_count\t%d\n' "${#tools[@]}"
    mapfile -t names < <(printf '%s\n' "${!tools[@]}" | sort)
    for name in "${names[@]}"; do
      printf 'tool\t%s\t%s\t%s\n' \
        "${name}" "${tools[${name}]}" "${tool_digests[${name}]}"
    done
    printf 'exit_status\t%d\n' "${exit_status}"
    printf 'log_path\t%s\n' "${log_relative}"
    printf 'log_sha256\t%s\n' "$(sha256_file "${log_absolute}")"
    printf 'log_size\t%s\n' "$(stat -c %s -- "${log_absolute}")"
    printf 'artifact_count\t%d\n' "${#artifacts[@]}"
    names=()
    if ((${#artifacts[@]} > 0)); then
      mapfile -t names < <(printf '%s\n' "${!artifacts[@]}" | sort)
    fi
    for name in "${names[@]}"; do
      absolute="$(require_regular_archive_file "${archive_root}" "${artifacts[${name}]}")"
      digest="$(sha256_file "${absolute}")"
      printf 'artifact\t%s\t%s\t%s\t%s\n' \
        "${name}" "${artifacts[${name}]}" "${digest}" "$(stat -c %s -- "${absolute}")"
    done
  } >"${temp_record}"
  digest="$(sha256_file "${temp_record}")"
  printf 'record_sha256\t%s\n' "${digest}" >>"${temp_record}"
  if (($(stat -c %s -- "${temp_record}") > MAX_RESULT_RECORD_BYTES)); then
    rm -f -- "${temp_record}"
    die 'generated result record exceeds the V1 size bound'
  fi
  mv -- "${temp_record}" "${record_absolute}"
  command sync -f -- "${record_absolute}" 2>/dev/null || true

  if ((exit_status != 0)); then
    printf 'parity evidence: command failed with exit status %d; result record retained at %s\n' \
      "${exit_status}" "${record_relative}" >&2
  fi
  return "${exit_status}"
}

record_read_scalar() {
  local -n record_lines="$1"
  local -n record_index="$2"
  local expected="$3"
  local -n output="$4"
  local key=""
  local extra=""

  ((record_index < ${#record_lines[@]})) || die "missing result-record field: ${expected}"
  IFS=$'\t' read -r key output extra <<<"${record_lines[${record_index}]}"
  [[ "${key}" == "${expected}" && -n "${output}" && -z "${extra}" ]] ||
    die "non-canonical result-record field: expected ${expected}"
  ((record_index += 1))
}

verify_record_main() {
  local repo=""
  local archive_root=""
  local record_relative=""
  local record_absolute=""
  local current_commit=""
  local value=""
  local name=""
  local path=""
  local hash=""
  local digest=""
  local size=""
  local previous=""
  local absolute=""
  local argv_zero_hex=""
  local saw_lc_all=false
  local saw_command_tool=false
  local index=0
  local count=0
  local item=0
  local byte_count=0
  local computed_digest=""
  local -a lines=()
  local -a fields=()

  while (($# > 0)); do
    case "$1" in
      --repo | --archive-root)
        (($# >= 2)) || die "$1 requires a value"
        case "$1" in
          --repo) repo="$2" ;;
          --archive-root) archive_root="$2" ;;
        esac
        shift 2
        ;;
      -h | --help)
        usage
        return 0
        ;;
      --*) die "unknown verify-record option: $1" ;;
      *)
        [[ -z "${record_relative}" ]] ||
          die 'verify-record accepts exactly one record path'
        record_relative="$1"
        shift
        ;;
    esac
  done

  [[ -n "${repo}" ]] || die '--repo is required for result verification'
  [[ -n "${archive_root}" ]] || die '--archive-root is required for result verification'
  [[ -n "${record_relative}" ]] || die 'verify-record requires one record path'
  require_command git
  require_command realpath
  require_command sha256sum
  require_command stat
  require_command od
  require_command tr

  repo="$(absolute_existing_directory "${repo}")"
  archive_root="$(absolute_existing_directory "${archive_root}")"
  path_is_within "${archive_root}" "${repo}" &&
    die 'archive root must be outside the repository checkout'
  path_is_within "${repo}" "${archive_root}" &&
    die 'archive root must not contain the repository checkout'
  record_absolute="$(require_regular_archive_file "${archive_root}" "${record_relative}")"
  byte_count="$(stat -c %s -- "${record_absolute}")"
  ((byte_count >= 1 && byte_count <= MAX_RESULT_RECORD_BYTES)) ||
    die 'result record exceeds the V1 size bound or is empty'
  mapfile -t lines <"${record_absolute}"
  ((${#lines[@]} > 0)) || die 'result record is empty'
  for value in "${lines[@]}"; do
    [[ -n "${value}" && "${value}" != *$'\r'* ]] ||
      die 'result record contains a blank or carriage-return line'
  done

  record_read_scalar lines index record_schema_version value
  [[ "${value}" == 1 ]] || die 'record_schema_version must be exactly 1'
  record_read_scalar lines index git_commit hash
  [[ "${hash}" =~ ^[0-9a-f]{40}$ ]] || die 'invalid result-record Git commit'
  record_read_scalar lines index git_detached value
  [[ "${value}" == true ]] || die 'git_detached must be exactly true'
  record_read_scalar lines index git_clean_before value
  [[ "${value}" == true ]] || die 'git_clean_before must be exactly true'
  record_read_scalar lines index git_clean_after value
  [[ "${value}" == true ]] || die 'git_clean_after must be exactly true'

  record_read_scalar lines index argv_count value
  valid_uint "${value}" || die 'invalid argv_count in result record'
  ((value >= 1 && value <= MAX_RESULT_ITEMS)) || die 'invalid argv_count in result record'
  count="${value}"
  for ((item = 0; item < count; item++)); do
    IFS=$'\t' read -r -a fields <<<"${lines[${index}]:-}"
    ((${#fields[@]} == 3)) || die 'non-canonical argv result-record entry'
    [[ "${fields[0]}" == argv ]] || die 'non-canonical argv result-record entry'
    printf -v value '%04d' "${item}"
    [[ "${fields[1]}" == "${value}" ]] || die 'non-canonical argv index'
    valid_hex_value "${fields[2]}" || die 'invalid argv encoding'
    if ((item == 0)); then
      argv_zero_hex="${fields[2]}"
    fi
    ((index += 1))
  done

  record_read_scalar lines index environment_count value
  valid_uint "${value}" || die 'invalid environment_count in result record'
  ((value >= 1 && value <= MAX_RESULT_ITEMS)) ||
    die 'invalid environment_count in result record'
  count="${value}"
  previous=""
  for ((item = 0; item < count; item++)); do
    IFS=$'\t' read -r -a fields <<<"${lines[${index}]:-}"
    ((${#fields[@]} == 3)) || die 'non-canonical environment result-record entry'
    [[ "${fields[0]}" == environment ]] ||
      die 'non-canonical environment result-record entry'
    name="${fields[1]}"
    valid_name "${name}" || die 'invalid environment name in result record'
    [[ -z "${previous}" || "${previous}" < "${name}" ]] ||
      die 'environment entries are not unique and sorted'
    valid_hex_value "${fields[2]}" || die 'invalid environment encoding'
    if [[ "${name}" == LC_ALL ]]; then
      [[ "${fields[2]}" == 43 ]] || die 'result-record LC_ALL must be exactly C'
      saw_lc_all=true
    fi
    previous="${name}"
    ((index += 1))
  done

  record_read_scalar lines index tool_count value
  valid_uint "${value}" || die 'invalid tool_count in result record'
  ((value >= 1 && value <= MAX_RESULT_ITEMS)) || die 'invalid tool_count in result record'
  count="${value}"
  previous=""
  for ((item = 0; item < count; item++)); do
    IFS=$'\t' read -r -a fields <<<"${lines[${index}]:-}"
    ((${#fields[@]} == 4)) || die 'non-canonical tool result-record entry'
    [[ "${fields[0]}" == tool ]] || die 'non-canonical tool result-record entry'
    name="${fields[1]}"
    path="${fields[2]}"
    digest="${fields[3]}"
    valid_label "${name}" || die 'invalid tool label in result record'
    [[ -z "${previous}" || "${previous}" < "${name}" ]] ||
      die 'tool entries are not unique and sorted'
    valid_absolute_tool_path "${path}" || die "invalid tool path in result record: ${name}"
    valid_sha256 "${digest}" || die "invalid tool digest in result record: ${name}"
    [[ -f "${path}" && -x "${path}" ]] || die "recorded tool is missing: ${name}"
    [[ "$(sha256_file "${path}")" == "${digest}" ]] ||
      die "recorded tool digest mismatch: ${name}"
    if [[ "${name}" == command ]]; then
      [[ "$(hex_encode "${path}")" == "${argv_zero_hex}" ]] ||
        die 'command tool path does not match argv zero'
      saw_command_tool=true
    fi
    previous="${name}"
    ((index += 1))
  done
  [[ "${saw_lc_all}" == true ]] || die 'result record is missing LC_ALL=C'
  [[ "${saw_command_tool}" == true ]] || die 'result record is missing command tool identity'

  record_read_scalar lines index exit_status value
  valid_uint "${value}" || die 'invalid exit_status in result record'
  ((value <= 255)) || die 'invalid exit_status in result record'
  [[ "${value}" == 0 ]] || die 'exit_status must be exactly 0 for passing evidence'
  record_read_scalar lines index log_path path
  valid_relative_path "${path}" || die 'invalid log_path in result record'
  record_read_scalar lines index log_sha256 digest
  valid_sha256 "${digest}" || die 'invalid log_sha256 in result record'
  record_read_scalar lines index log_size size
  valid_uint "${size}" || die 'invalid log_size in result record'
  absolute="$(require_regular_archive_file "${archive_root}" "${path}")"
  [[ "$(stat -c %s -- "${absolute}")" == "${size}" ]] || die 'recorded log size mismatch'
  [[ "$(sha256_file "${absolute}")" == "${digest}" ]] || die 'recorded log digest mismatch'

  record_read_scalar lines index artifact_count value
  valid_uint "${value}" || die 'invalid artifact_count in result record'
  ((value <= MAX_RESULT_ITEMS)) || die 'invalid artifact_count in result record'
  count="${value}"
  previous=""
  for ((item = 0; item < count; item++)); do
    IFS=$'\t' read -r -a fields <<<"${lines[${index}]:-}"
    ((${#fields[@]} == 5)) || die 'non-canonical artifact result-record entry'
    [[ "${fields[0]}" == artifact ]] || die 'non-canonical artifact result-record entry'
    name="${fields[1]}"
    path="${fields[2]}"
    digest="${fields[3]}"
    size="${fields[4]}"
    valid_label "${name}" || die 'invalid artifact label in result record'
    [[ -z "${previous}" || "${previous}" < "${name}" ]] ||
      die 'artifact entries are not unique and sorted'
    valid_relative_path "${path}" || die "invalid artifact path in result record: ${name}"
    valid_sha256 "${digest}" || die "invalid artifact digest in result record: ${name}"
    valid_uint "${size}" || die "invalid artifact size in result record: ${name}"
    absolute="$(require_regular_archive_file "${archive_root}" "${path}")"
    [[ "$(stat -c %s -- "${absolute}")" == "${size}" ]] ||
      die "recorded artifact size mismatch: ${name}"
    [[ "$(sha256_file "${absolute}")" == "${digest}" ]] ||
      die "recorded artifact digest mismatch: ${name}"
    previous="${name}"
    ((index += 1))
  done
  record_read_scalar lines index record_sha256 digest
  valid_sha256 "${digest}" || die 'invalid record_sha256 in result record'
  ((index == ${#lines[@]})) || die 'unexpected trailing result-record field'

  computed_digest="$({
    for ((item = 0; item < ${#lines[@]} - 1; item++)); do
      printf '%s\n' "${lines[${item}]}"
    done
  } | sha256sum)"
  computed_digest="${computed_digest%% *}"
  [[ "${computed_digest}" == "${digest}" ]] || die 'result record digest mismatch'

  current_commit="$(git_identity "${repo}" while-verifying)"
  [[ "${current_commit}" == "${hash}" ]] ||
    die 'recorded Git commit does not match verification checkout'
  printf 'parity result record is valid: %s\n' "${record_relative}"
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
    record) record_main "$@" ;;
    verify-record) verify_record_main "$@" ;;
    -h | --help | help) usage ;;
    *)
      usage >&2
      die "unknown command: ${command}"
      ;;
  esac
}

main "$@"
