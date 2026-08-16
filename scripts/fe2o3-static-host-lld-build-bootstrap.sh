#!/usr/bin/env bash

# This helper measures bootstrap inputs after Bash has already started. It is a
# build-closure measurement, not a self-authenticating trust root.

declare -ag FE2O3_BOOTSTRAP_DIRECTORY_LABELS=()
declare -ag FE2O3_BOOTSTRAP_DIRECTORY_PATHS=()
declare -ag FE2O3_BOOTSTRAP_DIRECTORY_FDS=()
declare -ag FE2O3_BOOTSTRAP_DIRECTORY_IDENTITIES=()
declare -ag FE2O3_BOOTSTRAP_FILE_LABELS=()
declare -ag FE2O3_BOOTSTRAP_FILE_PATHS=()
declare -ag FE2O3_BOOTSTRAP_FILE_FDS=()
declare -ag FE2O3_BOOTSTRAP_FILE_IDENTITIES=()
declare -ag FE2O3_BOOTSTRAP_FILE_SHA256=()
declare -Ag FE2O3_BOOTSTRAP_RETAINED_PATHS=()
FE2O3_BOOTSTRAP_ERROR=

fe2o3_bootstrap_reject() {
  # shellcheck disable=SC2034
  FE2O3_BOOTSTRAP_ERROR=$1
  return 1
}

fe2o3_bootstrap_safe_text() {
  local value=$1
  [[ -n "$value" && "$value" != *$'\n'* && "$value" != *$'\r'* &&
    "$value" != *$'\t'* ]]
}

fe2o3_bootstrap_identity() {
  /usr/bin/stat -Lc '%d:%i:%h:%s:%a:%f:%y:%z' -- "$1"
}

fe2o3_bootstrap_sha256() {
  /usr/bin/sha256sum -- "$1" | /usr/bin/cut -d ' ' -f 1
}

fe2o3_bootstrap_retain_directory() {
  local label=$1 path=$2 canonical descriptor identity named index
  fe2o3_bootstrap_safe_text "$label" && fe2o3_bootstrap_safe_text "$path" ||
    fe2o3_bootstrap_reject 'directory label or path is noncanonical' || return
  [[ "$path" == /* && -d "$path" && ! -L "$path" ]] ||
    fe2o3_bootstrap_reject "bootstrap directory is invalid: $path" || return
  canonical=$(/usr/bin/readlink -f -- "$path") ||
    fe2o3_bootstrap_reject "cannot canonicalize bootstrap directory: $path" || return
  [[ "$canonical" == "$path" ]] ||
    fe2o3_bootstrap_reject "bootstrap directory is not canonical: $path" || return
  [[ -z ${FE2O3_BOOTSTRAP_RETAINED_PATHS["$path"]+present} ]] || return 0
  exec {descriptor}<"$path" ||
    fe2o3_bootstrap_reject "cannot retain bootstrap directory: $path" || return
  identity=$(fe2o3_bootstrap_identity "/proc/self/fd/$descriptor") ||
    fe2o3_bootstrap_reject "cannot identify retained bootstrap directory: $path" || return
  named=$(fe2o3_bootstrap_identity "$path") ||
    fe2o3_bootstrap_reject "cannot identify named bootstrap directory: $path" || return
  [[ "$named" == "$identity" ]] ||
    fe2o3_bootstrap_reject "bootstrap directory changed while retained: $path" || return
  index=${#FE2O3_BOOTSTRAP_DIRECTORY_LABELS[@]}
  FE2O3_BOOTSTRAP_DIRECTORY_LABELS[index]=$label
  FE2O3_BOOTSTRAP_DIRECTORY_PATHS[index]=$path
  FE2O3_BOOTSTRAP_DIRECTORY_FDS[index]=$descriptor
  FE2O3_BOOTSTRAP_DIRECTORY_IDENTITIES[index]=$identity
  FE2O3_BOOTSTRAP_RETAINED_PATHS["$path"]="D:$index"
}

fe2o3_bootstrap_retain_file() {
  local label=$1 path=$2 expected_sha256=${3:-} expected_length=${4:-}
  local expected_mode=${5:-} canonical descriptor before after named digest index
  fe2o3_bootstrap_safe_text "$label" && fe2o3_bootstrap_safe_text "$path" ||
    fe2o3_bootstrap_reject 'file label or path is noncanonical' || return
  [[ "$path" == /* && -f "$path" && ! -L "$path" ]] ||
    fe2o3_bootstrap_reject "bootstrap file is invalid: $path" || return
  canonical=$(/usr/bin/readlink -f -- "$path") ||
    fe2o3_bootstrap_reject "cannot canonicalize bootstrap file: $path" || return
  [[ "$canonical" == "$path" ]] ||
    fe2o3_bootstrap_reject "bootstrap file is not canonical: $path" || return
  if [[ -n ${FE2O3_BOOTSTRAP_RETAINED_PATHS["$path"]+present} ]]; then
    return 0
  fi
  exec {descriptor}<"$path" ||
    fe2o3_bootstrap_reject "cannot retain bootstrap file: $path" || return
  before=$(fe2o3_bootstrap_identity "/proc/self/fd/$descriptor") ||
    fe2o3_bootstrap_reject "cannot identify retained bootstrap file: $path" || return
  digest=$(fe2o3_bootstrap_sha256 "/proc/self/fd/$descriptor") ||
    fe2o3_bootstrap_reject "cannot hash retained bootstrap file: $path" || return
  after=$(fe2o3_bootstrap_identity "/proc/self/fd/$descriptor") ||
    fe2o3_bootstrap_reject "cannot reidentify retained bootstrap file: $path" || return
  named=$(fe2o3_bootstrap_identity "$path") ||
    fe2o3_bootstrap_reject "cannot identify named bootstrap file: $path" || return
  [[ "$before" == "$after" && "$after" == "$named" ]] ||
    fe2o3_bootstrap_reject "bootstrap file changed while retained: $path" || return
  [[ -z "$expected_sha256" || "$digest" == "$expected_sha256" ]] ||
    fe2o3_bootstrap_reject "bootstrap file digest differs from pin: $path" || return
  [[ -z "$expected_length" ||
    $(/usr/bin/stat -Lc '%s' -- "/proc/self/fd/$descriptor") == "$expected_length" ]] ||
    fe2o3_bootstrap_reject "bootstrap file length differs from pin: $path" || return
  [[ -z "$expected_mode" ||
    $(/usr/bin/stat -Lc '%a' -- "/proc/self/fd/$descriptor") == "$expected_mode" ]] ||
    fe2o3_bootstrap_reject "bootstrap file mode differs from pin: $path" || return
  index=${#FE2O3_BOOTSTRAP_FILE_LABELS[@]}
  FE2O3_BOOTSTRAP_FILE_LABELS[index]=$label
  FE2O3_BOOTSTRAP_FILE_PATHS[index]=$path
  FE2O3_BOOTSTRAP_FILE_FDS[index]=$descriptor
  FE2O3_BOOTSTRAP_FILE_IDENTITIES[index]=$after
  FE2O3_BOOTSTRAP_FILE_SHA256[index]=$digest
  FE2O3_BOOTSTRAP_RETAINED_PATHS["$path"]="F:$index"
}

fe2o3_bootstrap_verify_all() {
  local phase=$1 index path descriptor expected observed before after digest
  for index in "${!FE2O3_BOOTSTRAP_DIRECTORY_PATHS[@]}"; do
    path=${FE2O3_BOOTSTRAP_DIRECTORY_PATHS[index]}
    descriptor=${FE2O3_BOOTSTRAP_DIRECTORY_FDS[index]}
    expected=${FE2O3_BOOTSTRAP_DIRECTORY_IDENTITIES[index]}
    [[ -d "$path" && ! -L "$path" ]] ||
      fe2o3_bootstrap_reject "bootstrap directory disappeared during $phase: $path" || return
    observed=$(fe2o3_bootstrap_identity "/proc/self/fd/$descriptor") ||
      fe2o3_bootstrap_reject "retained bootstrap directory failed during $phase: $path" || return
    [[ "$observed" == "$expected" ]] ||
      fe2o3_bootstrap_reject "retained bootstrap directory changed during $phase: $path" || return
    observed=$(fe2o3_bootstrap_identity "$path") ||
      fe2o3_bootstrap_reject "named bootstrap directory failed during $phase: $path" || return
    [[ "$observed" == "$expected" ]] ||
      fe2o3_bootstrap_reject "named bootstrap directory changed during $phase: $path" || return
  done
  for index in "${!FE2O3_BOOTSTRAP_FILE_PATHS[@]}"; do
    path=${FE2O3_BOOTSTRAP_FILE_PATHS[index]}
    descriptor=${FE2O3_BOOTSTRAP_FILE_FDS[index]}
    expected=${FE2O3_BOOTSTRAP_FILE_IDENTITIES[index]}
    [[ -f "$path" && ! -L "$path" ]] ||
      fe2o3_bootstrap_reject "bootstrap file disappeared during $phase: $path" || return
    before=$(fe2o3_bootstrap_identity "/proc/self/fd/$descriptor") ||
      fe2o3_bootstrap_reject "retained bootstrap file failed during $phase: $path" || return
    [[ "$before" == "$expected" ]] ||
      fe2o3_bootstrap_reject "retained bootstrap file changed during $phase: $path" || return
    digest=$(fe2o3_bootstrap_sha256 "/proc/self/fd/$descriptor") ||
      fe2o3_bootstrap_reject "retained bootstrap file hash failed during $phase: $path" || return
    after=$(fe2o3_bootstrap_identity "/proc/self/fd/$descriptor") ||
      fe2o3_bootstrap_reject "retained bootstrap file recheck failed during $phase: $path" || return
    [[ "$after" == "$expected" &&
      "$digest" == "${FE2O3_BOOTSTRAP_FILE_SHA256[index]}" ]] ||
      fe2o3_bootstrap_reject "retained bootstrap file changed during $phase: $path" || return
    observed=$(fe2o3_bootstrap_identity "$path") ||
      fe2o3_bootstrap_reject "named bootstrap file failed during $phase: $path" || return
    [[ "$observed" == "$expected" ]] ||
      fe2o3_bootstrap_reject "named bootstrap file changed during $phase: $path" || return
  done
}

fe2o3_bootstrap_file_fields() {
  local path=$1 entry index
  entry=${FE2O3_BOOTSTRAP_RETAINED_PATHS["$path"]:-}
  [[ "$entry" == F:* ]] ||
    fe2o3_bootstrap_reject "bootstrap file was not retained: $path" || return
  index=${entry#F:}
  printf '%s\t%s\t%s\n' "${FE2O3_BOOTSTRAP_FILE_SHA256[index]}" \
    "$(/usr/bin/stat -Lc '%s' -- "/proc/self/fd/${FE2O3_BOOTSTRAP_FILE_FDS[index]}")" \
    "$(/usr/bin/stat -Lc '%a' -- "/proc/self/fd/${FE2O3_BOOTSTRAP_FILE_FDS[index]}")"
}

fe2o3_bootstrap_file_descriptor_path() {
  local path=$1 entry index
  entry=${FE2O3_BOOTSTRAP_RETAINED_PATHS["$path"]:-}
  [[ "$entry" == F:* ]] ||
    fe2o3_bootstrap_reject "bootstrap file was not retained: $path" || return
  index=${entry#F:}
  printf '/proc/self/fd/%s\n' "${FE2O3_BOOTSTRAP_FILE_FDS[index]}"
}

fe2o3_bootstrap_write_manifest() {
  local output=$1 index
  {
    printf 'FORMAT=fe2o3-static-host-lld-bootstrap-measurement-v1\n'
    printf 'STATUS=measured-no-authority\n'
    printf 'ROOT_ASSUMPTION=already-running-bash-linux-vfs-reviewed-pins\n'
    printf 'MUTATION_DETECTION=retained-descriptor-identity-ctime-nlink-mode-size-sha256\n'
    printf 'DIRECTORIES=%s\n' "${#FE2O3_BOOTSTRAP_DIRECTORY_PATHS[@]}"
    for index in "${!FE2O3_BOOTSTRAP_DIRECTORY_PATHS[@]}"; do
      printf 'D\t%s\t%s\t%s\n' \
        "${FE2O3_BOOTSTRAP_DIRECTORY_LABELS[index]}" \
        "${FE2O3_BOOTSTRAP_DIRECTORY_PATHS[index]}" \
        "${FE2O3_BOOTSTRAP_DIRECTORY_IDENTITIES[index]}"
    done
    printf 'FILES=%s\n' "${#FE2O3_BOOTSTRAP_FILE_PATHS[@]}"
    for index in "${!FE2O3_BOOTSTRAP_FILE_PATHS[@]}"; do
      printf 'F\t%s\t%s\t%s\t%s\n' \
        "${FE2O3_BOOTSTRAP_FILE_LABELS[index]}" \
        "${FE2O3_BOOTSTRAP_FILE_PATHS[index]}" \
        "${FE2O3_BOOTSTRAP_FILE_IDENTITIES[index]}" \
        "${FE2O3_BOOTSTRAP_FILE_SHA256[index]}"
    done
    printf 'FINAL_REVALIDATION=required-before-success\n'
  } >"$output"
}
