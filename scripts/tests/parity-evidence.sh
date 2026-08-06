#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${TEST_SCRIPT_DIR}/../.." && pwd)"
readonly EVIDENCE_SCRIPT="${REPO_ROOT}/scripts/parity-evidence.sh"
readonly TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "${TEST_ROOT}"' EXIT

row_link() {
  local id="$1"
  printf 'records/test-results/%s.json#result-%s' "${id}" "${id}"
}

write_rows() {
  local destination="$1"
  local id
  local i

  : >"${destination}"
  for ((i = 1; i <= 94; i++)); do
    printf -v id '%02d' "${i}"
    printf 'row\t%s\t%s\n' "${id}" "$(row_link "${id}")" >>"${destination}"
  done
  for ((i = 1; i <= 15; i++)); do
    printf -v id 'S%02d' "${i}"
    printf 'row\t%s\t%s\n' "${id}" "$(row_link "${id}")" >>"${destination}"
  done
}

write_fixture() {
  local destination="$1"
  local rows="${TEST_ROOT}/rows.tsv"
  write_rows "${rows}"
  {
    printf '%s\n' \
      $'hardware_lane\tmi300x-gfx942-release' \
      $'driver_version\tamdgpu/6.10.5;kernel=6.12.0' \
      $'schema_version\t1' \
      $'device_target\tgfx942' \
      $'git_dirty\tfalse' \
      $'rocm_version\t7.2.0' \
      $'git_commit\t0123456789abcdef0123456789abcdef01234567' \
      $'llvm_version\tAMD clang version 21.0.0' \
      $'rustc_version\trelease=1.90.0;commit=0123456789abcdef0123456789abcdef01234567;host=x86_64-unknown-linux-gnu'
    tac "${rows}"
  } >"${destination}"
}

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  local log="${TEST_ROOT}/${name}.log"

  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${log}"; then
    printf 'negative evidence test unexpectedly passed: %s\n' "${name}" >&2
    return 1
  fi
  if ! grep -F -- "${expected}" "${log}" >/dev/null; then
    printf 'negative evidence test produced the wrong diagnostic: %s\n' "${name}" >&2
    cat "${log}" >&2
    return 1
  fi
}

filter_fixture() {
  local source="$1"
  local destination="$2"
  local rejected="$3"
  awk -F '\t' -v rejected="${rejected}" '$1 != rejected { print }' \
    "${source}" >"${destination}"
}

replace_literal() {
  local source="$1"
  local destination="$2"
  local old="$3"
  local replacement="$4"
  awk -v old="${old}" -v replacement="${replacement}" \
    '{ sub(old, replacement); print }' "${source}" >"${destination}"
}

readonly FIXTURE="${TEST_ROOT}/fixture.tsv"
readonly CANONICAL="${TEST_ROOT}/canonical.tsv"
write_fixture "${FIXTURE}"

# Fixture collection performs no discovery commands.
PATH=/does/not/exist /usr/bin/bash "${EVIDENCE_SCRIPT}" collect \
  --fixture "${FIXTURE}" >"${CANONICAL}"
bash "${EVIDENCE_SCRIPT}" validate "${CANONICAL}"

# Canonical collection is byte-for-byte deterministic and idempotent.
bash "${EVIDENCE_SCRIPT}" collect --fixture "${FIXTURE}" \
  >"${TEST_ROOT}/canonical.second.tsv"
cmp -- "${CANONICAL}" "${TEST_ROOT}/canonical.second.tsv"
bash "${EVIDENCE_SCRIPT}" collect --fixture "${CANONICAL}" \
  >"${TEST_ROOT}/canonical.third.tsv"
cmp -- "${CANONICAL}" "${TEST_ROOT}/canonical.third.tsv"
[[ "$(wc -l <"${CANONICAL}")" == 118 ]]

filter_fixture "${FIXTURE}" "${TEST_ROOT}/missing-scalar.tsv" git_commit
expect_failure missing_scalar 'missing scalar field: git_commit' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/missing-scalar.tsv"

cp -- "${FIXTURE}" "${TEST_ROOT}/duplicate-scalar.tsv"
printf '%s\n' $'git_commit\t0123456789abcdef0123456789abcdef01234567' \
  >>"${TEST_ROOT}/duplicate-scalar.tsv"
expect_failure duplicate_scalar 'duplicate scalar field: git_commit' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/duplicate-scalar.tsv"

cp -- "${FIXTURE}" "${TEST_ROOT}/unknown-field.tsv"
printf '%s\n' $'environment\tSECRET=value' >>"${TEST_ROOT}/unknown-field.tsv"
expect_failure unknown_field 'unknown field: environment' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/unknown-field.tsv"

awk -F '\t' '!($1 == "row" && $2 == "42") { print }' "${FIXTURE}" \
  >"${TEST_ROOT}/missing-row.tsv"
expect_failure missing_row 'missing parity row link: 42' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/missing-row.tsv"

cp -- "${FIXTURE}" "${TEST_ROOT}/duplicate-row.tsv"
printf '%s\n' $'row\t42\trecords/other.json#result-42' \
  >>"${TEST_ROOT}/duplicate-row.tsv"
expect_failure duplicate_row 'duplicate parity row ID: 42' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/duplicate-row.tsv"

cp -- "${FIXTURE}" "${TEST_ROOT}/unknown-row.tsv"
printf '%s\n' $'row\t95\trecords/95.json#result-95' \
  >>"${TEST_ROOT}/unknown-row.tsv"
expect_failure unknown_row 'unknown parity row ID: 95' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/unknown-row.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/absolute-link.tsv" \
  'records/test-results/42.json#result-42' '/tmp/42.json#result-42'
expect_failure absolute_link 'malformed row link for 42' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/absolute-link.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/traversal-link.tsv" \
  'records/test-results/42.json#result-42' 'records/../42.json#result-42'
expect_failure traversal_link 'malformed row link for 42' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/traversal-link.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/url-link.tsv" \
  'records/test-results/42.json#result-42' 'https://ci.example/42?token=secret'
expect_failure url_link 'malformed row link for 42' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/url-link.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/missing-fragment.tsv" \
  'records/test-results/42.json#result-42' 'records/test-results/42.json'
expect_failure missing_fragment 'malformed row link for 42' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/missing-fragment.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/bad-commit.tsv" \
  '0123456789abcdef0123456789abcdef01234567' 'ABCDEF'
expect_failure bad_commit 'git_commit must be a lowercase 40-digit hash' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/bad-commit.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/bad-dirty.tsv" \
  $'git_dirty\tfalse' $'git_dirty\tclean'
expect_failure bad_dirty 'git_dirty must be exactly true or false' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/bad-dirty.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/bad-target.tsv" \
  $'device_target\tgfx942' $'device_target\tGFX942'
expect_failure bad_target 'device_target must be a canonical AMD gfx target' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/bad-target.tsv"

replace_literal "${FIXTURE}" "${TEST_ROOT}/bad-lane.tsv" \
  mi300x-gfx942-release 'host/mi300x'
expect_failure bad_lane 'hardware_lane must be a bounded lowercase lane identity' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/bad-lane.tsv"

cp -- "${FIXTURE}" "${TEST_ROOT}/extra-field.tsv"
printf '%s\n' $'row\t42\trecords/42.json#result\textra' \
  >>"${TEST_ROOT}/extra-field.tsv"
expect_failure extra_field 'row line' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/extra-field.tsv"

cp -- "${FIXTURE}" "${TEST_ROOT}/blank-line.tsv"
printf '\n' >>"${TEST_ROOT}/blank-line.tsv"
expect_failure blank_line 'blank or carriage-return line' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/blank-line.tsv"

long_value="$(printf 'a%.0s' {1..257})"
replace_literal "${FIXTURE}" "${TEST_ROOT}/long-version.tsv" \
  'AMD clang version 21.0.0' "${long_value}"
expect_failure long_version 'invalid or unbounded llvm_version' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${TEST_ROOT}/long-version.tsv"

# Validation rejects valid but non-canonical order instead of silently fixing it.
{
  sed -n '2p' "${CANONICAL}"
  sed -n '1p' "${CANONICAL}"
  sed -n '3,$p' "${CANONICAL}"
} >"${TEST_ROOT}/wrong-scalar-order.tsv"
expect_failure wrong_scalar_order 'non-canonical scalar order' \
  bash "${EVIDENCE_SCRIPT}" validate "${TEST_ROOT}/wrong-scalar-order.tsv"

awk 'NR == 10 { first = $0; next } NR == 11 { print; print first; next } { print }' \
  "${CANONICAL}" >"${TEST_ROOT}/wrong-row-order.tsv"
expect_failure wrong_row_order 'non-canonical row order: expected 01, found 02' \
  bash "${EVIDENCE_SCRIPT}" validate "${TEST_ROOT}/wrong-row-order.tsv"

expect_failure fixture_mixed '--fixture cannot be combined with live collection options' \
  bash "${EVIDENCE_SCRIPT}" collect --fixture "${FIXTURE}" --hardware-lane lane
expect_failure live_missing_lane '--hardware-lane is required for live collection' \
  bash "${EVIDENCE_SCRIPT}" collect --rows "${TEST_ROOT}/rows.tsv"
expect_failure unknown_option 'unknown collect option: --environment' \
  bash "${EVIDENCE_SCRIPT}" collect --environment dump
expect_failure unknown_command 'unknown command: publish' \
  bash "${EVIDENCE_SCRIPT}" publish

bash -n "${EVIDENCE_SCRIPT}"
bash -n "${BASH_SOURCE[0]}"
if command -v shellcheck >/dev/null 2>&1; then
  shellcheck "${EVIDENCE_SCRIPT}" "${BASH_SOURCE[0]}"
fi

printf '%s\n' 'parity evidence tests passed'
