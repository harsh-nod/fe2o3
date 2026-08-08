#!/usr/bin/env bash

set -Eeuo pipefail

readonly TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly REPO_ROOT="$(cd -- "${TEST_SCRIPT_DIR}/../.." && pwd)"
readonly PARITY_SCRIPT="${REPO_ROOT}/scripts/parity-matrix.sh"
readonly SOURCE_STATUS="${REPO_ROOT}/docs/cuda-oxide-parity-status.tsv"
readonly SOURCE_MATRIX="${REPO_ROOT}/docs/cuda-oxide-parity-matrix.md"
readonly TEST_ROOT="$(mktemp -d)"
trap 'rm -rf "${TEST_ROOT}"' EXIT

reset_case() {
  cp -- "${SOURCE_STATUS}" "${TEST_ROOT}/status.tsv"
  cp -- "${SOURCE_MATRIX}" "${TEST_ROOT}/matrix.md"
}

replace_file() {
  local destination="$1"
  mv -- "${destination}.new" "${destination}"
}

expect_check_failure() {
  local name="$1"
  local expected="$2"
  local output="${TEST_ROOT}/${name}.log"

  if bash "${PARITY_SCRIPT}" check \
    "${TEST_ROOT}/status.tsv" "${TEST_ROOT}/matrix.md" \
    >"${output}" 2>&1; then
    printf 'negative parity test unexpectedly passed: %s\n' "${name}" >&2
    return 1
  fi
  if ! grep -F -- "${expected}" "${output}" >/dev/null; then
    printf 'negative parity test produced the wrong diagnostic: %s\n' "${name}" >&2
    cat "${output}" >&2
    return 1
  fi
}

# The checked-in source and projection are the positive fixture.
bash "${PARITY_SCRIPT}" check "${SOURCE_STATUS}" "${SOURCE_MATRIX}"

# Generation repairs valid drift and is byte-for-byte deterministic.
reset_case
awk '{
  sub(/2db97134d9a3a79fe71c211e65a616dacdf03235/,
      "3db97134d9a3a79fe71c211e65a616dacdf03235")
  if ($0 ~ /^\| Normative \|/) {
    count = split($0, cells, "|")
    if (count == 8) {
      printf "| Normative | %d | %d | %d | %d | %d |\n", \
        cells[3], cells[4] - 1, cells[5] + 1, cells[6], cells[7]
      next
    }
  }
  if ($0 ~ /^\| 01 \|/) {
    sub(/\| Missing \|/, "| Partial |")
  }
  print
}' "${TEST_ROOT}/matrix.md" >"${TEST_ROOT}/matrix.md.new"
replace_file "${TEST_ROOT}/matrix.md"
bash "${PARITY_SCRIPT}" generate \
  "${TEST_ROOT}/status.tsv" "${TEST_ROOT}/matrix.md"
cmp -- "${SOURCE_MATRIX}" "${TEST_ROOT}/matrix.md"
cp -- "${TEST_ROOT}/matrix.md" "${TEST_ROOT}/matrix.once.md"
bash "${PARITY_SCRIPT}" generate \
  "${TEST_ROOT}/status.tsv" "${TEST_ROOT}/matrix.md"
cmp -- "${TEST_ROOT}/matrix.once.md" "${TEST_ROOT}/matrix.md"

reset_case
awk 'BEGIN { FS = OFS = "\t" }
  $1 == "normative" && $2 == "02" { $2 = "01" }
  { print }
' "${TEST_ROOT}/status.tsv" >"${TEST_ROOT}/status.tsv.new"
replace_file "${TEST_ROOT}/status.tsv"
expect_check_failure duplicate_id "expected 02, found 01"

reset_case
awk 'BEGIN { FS = OFS = "\t" }
  !($1 == "normative" && $2 == "02") { print }
' "${TEST_ROOT}/status.tsv" >"${TEST_ROOT}/status.tsv.new"
replace_file "${TEST_ROOT}/status.tsv"
expect_check_failure missing_id "expected 02, found 03"

reset_case
awk '{
  if ($0 ~ /^\| 02 \|/) {
    sub(/^\| 02 \|/, "| 01 |")
  }
  print
}' "${TEST_ROOT}/matrix.md" >"${TEST_ROOT}/matrix.md.new"
replace_file "${TEST_ROOT}/matrix.md"
expect_check_failure duplicate_markdown_id "expected 02, found 01"

reset_case
awk '$0 !~ /^\| 02 \|/ { print }' \
  "${TEST_ROOT}/matrix.md" >"${TEST_ROOT}/matrix.md.new"
replace_file "${TEST_ROOT}/matrix.md"
expect_check_failure missing_markdown_id "expected 02, found 03"

reset_case
awk 'BEGIN { FS = OFS = "\t" }
  $1 == "normative" && $2 == "01" { $3 = "Unknown" }
  { print }
' "${TEST_ROOT}/status.tsv" >"${TEST_ROOT}/status.tsv.new"
replace_file "${TEST_ROOT}/status.tsv"
expect_check_failure invalid_status "invalid status for 01: Unknown"

reset_case
awk '{
  if ($0 ~ /^\| 01 \|/) {
    sub(/\| Partial \|/, "| Unknown |")
  }
  print
}' "${TEST_ROOT}/matrix.md" >"${TEST_ROOT}/matrix.md.new"
replace_file "${TEST_ROOT}/matrix.md"
expect_check_failure invalid_markdown_status \
  "invalid Markdown status for 01: Unknown"

reset_case
awk '{
  if ($0 ~ /^\| Normative \|/) {
    count = split($0, cells, "|")
    if (count == 8) {
      printf "| Normative | %d | %d | %d | %d | %d |\n", \
        cells[3], cells[4] - 1, cells[5], cells[6], cells[7]
      next
    }
  }
  print
}' "${TEST_ROOT}/matrix.md" >"${TEST_ROOT}/matrix.md.new"
replace_file "${TEST_ROOT}/matrix.md"
expect_check_failure count_mismatch "parity matrix is out of date"

reset_case
awk '{
  sub(/2db97134d9a3a79fe71c211e65a616dacdf03235/,
      "3db97134d9a3a79fe71c211e65a616dacdf03235")
  print
}' "${TEST_ROOT}/matrix.md" >"${TEST_ROOT}/matrix.md.new"
replace_file "${TEST_ROOT}/matrix.md"
expect_check_failure stale_pinned_commits "parity matrix is out of date"

printf '%s\n' 'parity matrix tests passed'
