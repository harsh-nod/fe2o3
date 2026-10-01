#!/usr/bin/env bash

# Sourced helpers. Tickets stay in the invoking shell, never in a cache path.
ci_phase_target_validate_mode() {
  case "${FE2O3_CI_EPHEMERAL_SUBTARGETS-0}" in
    0 | 1) ;;
    *)
      printf '%s\n' 'FE2O3_CI_EPHEMERAL_SUBTARGETS must be 0 or 1' >&2
      return 2
      ;;
  esac
}

ci_phase_target_array() {
  [[ "$1" =~ ^[a-z][a-z0-9_]*$ ]] &&
    [[ "$(declare -p "$1" 2>/dev/null)" == 'declare -a '* ]] || {
    printf '%s\n' 'phase target requires a caller-owned indexed array' >&2
    return 2
  }
}

ci_phase_target_identity() {
  local _path="$1" _canonical _identity _device _inode _owner _mode
  [[ "${_path}" == /* && "${_path}" != *$'\n'* &&
    -d "${_path}" && ! -L "${_path}" ]] || return 2
  _canonical="$(realpath --canonicalize-existing -- "${_path}")" || return 2
  _identity="$(stat -c '%d:%i:%u:%a' -- "${_path}")" || return 2
  IFS=: read -r _device _inode _owner _mode <<<"${_identity}"
  [[ "${_canonical}" == "${_path}" && "${_mode}" == 700 &&
    "${_owner}" == "$(id -u)" ]] || return 2
  printf '%s\n' "${_identity}"
}

ci_phase_target_begin() {
  ci_phase_target_validate_mode || return
  ci_phase_target_array "$1" || return
  local -n _phase="$1"
  local _root="$2" _label="$3" _root_identity _path _identity
  ((${#_phase[@]} == 0)) || return 2
  [[ "${FE2O3_CI_EPHEMERAL_SUBTARGETS-0}" == 1 ]] || return 0
  [[ "${_label}" =~ ^[a-z][a-z0-9-]{0,63}$ ]] || return 2
  _root_identity="$(ci_phase_target_identity "${_root}")" || {
    printf 'phase target root is not canonical, owner-held mode 700: %s\n' "${_root}" >&2
    return 2
  }
  _path="$(mktemp -d -- "${_root}/.fe2o3-ci-phase-${_label}.XXXXXXXX")" || return
  printf 'phase target created: %s\n' "${_path}" >&2
  _identity="$(ci_phase_target_identity "${_path}")" || return
  _phase=("${_root}" "${_root_identity}" "${_path}" "${_identity}")
  ci_phase_target_verify "$1"
}

ci_phase_target_verify() {
  ci_phase_target_array "$1" || return
  local -n _phase="$1"
  ((${#_phase[@]} == 4)) || return 2
  [[ "${_phase[2]%/*}" == "${_phase[0]}" &&
    "${_phase[2]##*/}" == .fe2o3-ci-phase-* &&
    "${_phase[1]%%:*}" == "${_phase[3]%%:*}" &&
    "$(ci_phase_target_identity "${_phase[0]}")" == "${_phase[1]}" &&
    "$(ci_phase_target_identity "${_phase[2]}")" == "${_phase[3]}" ]] || {
    printf '%s\n' 'phase target ownership or path changed; refusing cleanup' >&2
    return 2
  }
}

# The caller supplies a bounded, logged Cargo command as an argument vector.
# Invoke only after the last consumer succeeds; failed phases keep their cache.
ci_phase_target_finish() {
  ci_phase_target_array "$1" || return
  local -n _phase="$1"
  local _ticket="$1" _path
  shift
  ((${#_phase[@]} != 0)) || return 0
  (($# > 0)) || return 2
  ci_phase_target_verify "${_ticket}" || return
  _path="${_phase[2]}"
  printf 'phase target cleanup:' >&2
  printf ' %q' "$@" clean --locked --offline --target-dir "${_path}" >&2
  printf '\n' >&2
  "$@" clean --locked --offline --target-dir "${_path}" || return
  [[ "$(ci_phase_target_identity "${_phase[0]}")" == "${_phase[1]}" ]] || return 2
  if [[ -e "${_path}" || -L "${_path}" ]]; then
    ci_phase_target_verify "${_ticket}" || return
    rmdir -- "${_path}" || return
  fi
  _phase=()
  printf 'phase target retired: %s\n' "${_path}" >&2
}
