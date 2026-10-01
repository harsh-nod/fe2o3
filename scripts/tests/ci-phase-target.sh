#!/usr/bin/env bash

set -Eeuo pipefail
umask 077
source "$(dirname -- "${BASH_SOURCE[0]}")/../ci-phase-target.sh"
fixture="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-phase-target-test.XXXXXXXX")"
trap 'chmod -R u+w -- "${fixture}"; rm -rf -- "${fixture}"' EXIT
log="${fixture}/clean.log"
: >"${log}"

expect_status() {
  local expected="$1" status=0
  shift
  "$@" >"${fixture}/stdout" 2>"${fixture}/stderr" || status=$?
  [[ "${status}" == "${expected}" ]] || {
    printf 'expected status %s, got %s\n' "${expected}" "${status}" >&2
    cat "${fixture}/stderr" >&2
    exit 1
  }
}

mock_clean() {
  [[ $# == 5 && "$1" == clean && "$2" == --locked &&
    "$3" == --offline && "$4" == --target-dir ]] || return
  printf '%s\n' "$5" >>"${log}"
  rm -rf -- "$5"
}

unset FE2O3_CI_EPHEMERAL_SUBTARGETS
declare -a ticket=()
ci_phase_target_begin ticket "${fixture}/absent" default
ci_phase_target_finish ticket mock_clean
[[ ! -e "${fixture}/absent" && ! -s "${log}" ]]
FE2O3_CI_EPHEMERAL_SUBTARGETS=0 ci_phase_target_begin ticket /absent default
for value in '' true 2 -1; do
  FE2O3_CI_EPHEMERAL_SUBTARGETS="${value}" expect_status 2 \
    ci_phase_target_begin ticket "${fixture}" invalid
done
export FE2O3_CI_EPHEMERAL_SUBTARGETS=1
expect_status 2 ci_phase_target_begin 'ticket[$(false)]' "${fixture}" unit
expect_status 2 ci_phase_target_begin ticket "${fixture}" '../escape'
ln -s "${fixture}" "${fixture}/alias"
expect_status 2 ci_phase_target_begin ticket "${fixture}/alias" unit
rm -- "${fixture}/alias"

mkdir "${fixture}/runtime-pure-rust-policy" "${fixture}/fe2o3-sim-export"
printf 'evidence\n' >"${fixture}/evidence"
ci_phase_target_begin ticket "${fixture}" runtime
first="${ticket[2]}"
declare -a sibling=()
ci_phase_target_begin sibling "${fixture}" runtime
second="${sibling[2]}"
[[ "${first}" != "${second}" && "$(stat -c %a "${first}")" == 700 ]]
printf 'artifact\n' >"${first}/artifact"
expect_status 2 ci_phase_target_begin ticket "${fixture}" runtime
ci_phase_target_finish ticket mock_clean
[[ ${#ticket[@]} == 0 && ! -e "${first}" && -d "${second}" ]]
ci_phase_target_finish ticket mock_clean
[[ "$(wc -l <"${log}")" == 1 ]]
[[ -d "${fixture}/runtime-pure-rust-policy" && -d "${fixture}/fe2o3-sim-export" ]]
[[ "$(cat "${fixture}/evidence")" == evidence ]]
ci_phase_target_finish sibling mock_clean

for mutation in mode inode symlink outside root-mode owner; do
  ticket=()
  ci_phase_target_begin ticket "${fixture}" negative
  path="${ticket[2]}"
  before="$(wc -l <"${log}")"
  case "${mutation}" in
    mode) chmod 755 "${path}" ;;
    inode) mv "${path}" "${path}.old"; mkdir -m 700 "${path}" ;;
    symlink) mv "${path}" "${path}.old"; ln -s "${path}.old" "${path}" ;;
    outside) ticket[2]="${fixture}/runtime-pure-rust-policy" ;;
    root-mode) chmod 755 "${fixture}" ;;
    owner)
      stat() {
        local value
        value="$(command stat "$@")"
        if [[ "${!#}" == "${path}" ]]; then
          printf '%s:%s:700\n' "${value%:*:*}" "$(( $(id -u) + 1 ))"
        else
          printf '%s\n' "${value}"
        fi
      }
      ;;
  esac
  expect_status 2 ci_phase_target_finish ticket mock_clean
  [[ "$(wc -l <"${log}")" == "${before}" ]]
  unset -f stat 2>/dev/null || true
  chmod 700 "${fixture}"
  [[ -L "${path}" ]] || chmod 700 "${path}"
done

ticket=()
ci_phase_target_begin ticket "${fixture}" failure
failed="${ticket[2]}"
fail_clean() { return 37; }
expect_status 37 ci_phase_target_finish ticket fail_clean
[[ ${#ticket[@]} == 4 && -d "${failed}" ]]
replace_clean() {
  mv "$5" "$5.old"
  ln -s "$5.old" "$5"
}
expect_status 2 ci_phase_target_finish ticket replace_clean
[[ -L "${failed}" && -d "${failed}.old" ]]
printf '%s\n' 'phase target shell tests passed'
