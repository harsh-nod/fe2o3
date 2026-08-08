#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

TEST_SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_SCRIPT_DIR
REPO_ROOT="$(cd -- "${TEST_SCRIPT_DIR}/../.." && pwd)"
readonly REPO_ROOT
readonly DASHBOARD_SCRIPT="${REPO_ROOT}/scripts/parity-dashboard.sh"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  local log="${TEST_ROOT}/${name}.log"

  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${log}"; then
    printf 'negative dashboard test unexpectedly passed: %s\n' "${name}" >&2
    return 1
  fi
  if ! grep -F -- "${expected}" "${log}" >/dev/null; then
    printf 'negative dashboard test produced the wrong diagnostic: %s\n' "${name}" >&2
    cat "${log}" >&2
    return 1
  fi
}

mutate_claim_field() {
  local source="$1"
  local destination="$2"
  local record_kind="$3"
  local record_id="$4"
  local field="$5"
  local value="$6"
  awk -F '\t' -v OFS='\t' -v kind="${record_kind}" -v id="${record_id}" \
    -v field="${field}" -v value="${value}" '
      $1 == kind && $2 == id { $field = value; changed++ }
      { print }
      END { if (changed != 1) exit 3 }
    ' "${source}" >"${destination}"
}

readonly STATUS="${TEST_ROOT}/status.tsv"
readonly MATRIX="${TEST_ROOT}/matrix.md"
readonly CLAIMS="${TEST_ROOT}/claims.tsv"
readonly OUT_A="${TEST_ROOT}/out-a"
readonly OUT_B="${TEST_ROOT}/out-b"
cp -- "${REPO_ROOT}/docs/cuda-oxide-parity-status.tsv" "${STATUS}"
cp -- "${REPO_ROOT}/docs/cuda-oxide-parity-matrix.md" "${MATRIX}"
mkdir -p -- "${OUT_A}" "${OUT_B}"
"${DASHBOARD_SCRIPT}" claims >"${CLAIMS}"

"${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${CLAIMS}" --repo "${REPO_ROOT}"

readonly SOURCE_DIGESTS="${TEST_ROOT}/source-digests"
sha256sum "${STATUS}" "${MATRIX}" "${CLAIMS}" >"${SOURCE_DIGESTS}"
printf 'sentinel\n' >"${OUT_A}/untouched"
TZ=UTC HOSTNAME=first-host "${DASHBOARD_SCRIPT}" update \
  --status "${STATUS}" --matrix "${MATRIX}" --claims "${CLAIMS}" \
  --repo "${REPO_ROOT}" --markdown "${OUT_A}/dashboard.md" \
  --tsv "${OUT_A}/dashboard.tsv"
TZ=Pacific/Honolulu HOSTNAME=second-host "${DASHBOARD_SCRIPT}" update \
  --status "${STATUS}" --matrix "${MATRIX}" --claims "${CLAIMS}" \
  --repo "${REPO_ROOT}" --markdown "${OUT_B}/dashboard.md" \
  --tsv "${OUT_B}/dashboard.tsv"
cmp -- "${OUT_A}/dashboard.md" "${OUT_B}/dashboard.md"
cmp -- "${OUT_A}/dashboard.tsv" "${OUT_B}/dashboard.tsv"
[[ "$(cat "${OUT_A}/untouched")" == sentinel ]]
sha256sum --check --status "${SOURCE_DIGESTS}"

"${DASHBOARD_SCRIPT}" check --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${CLAIMS}" --repo "${REPO_ROOT}" \
  --markdown "${OUT_A}/dashboard.md" --tsv "${OUT_A}/dashboard.tsv"

# The machine projection is canonical, complete, and keeps evidence kinds apart.
[[ "$(awk -F '\t' '$1 == "normative" || $1 == "supplemental" { count++ } END { print count + 0 }' "${OUT_A}/dashboard.tsv")" == 109 ]]
awk -F '\t' '
  ($1 == "normative" || $1 == "supplemental") && NF != 13 { exit 1 }
  $2 == "59" && !($4 == "N/A" && $6 == "N/A" && $7 == "n/a") { exit 1 }
  $2 == "53" && !($7 ~ /compile-code-object/ && $8 ~ /gfx1151/ && $8 ~ /gfx942/ && $8 ~ /gfx950/) { exit 1 }
  END { if (NR != 114) exit 1 }
' "${OUT_A}/dashboard.tsv"
grep -F '| Local hardware execution | 0 |' "${OUT_A}/dashboard.md" >/dev/null
grep -F '| Remote hardware execution | 0 |' "${OUT_A}/dashboard.md" >/dev/null
grep -F '| Machine-code refinement | 0 |' "${OUT_A}/dashboard.md" >/dev/null
if grep -E '[0-9]{4}-[0-9]{2}-[0-9]{2}T|first-host|second-host' \
  "${OUT_A}/dashboard.md" "${OUT_A}/dashboard.tsv" >/dev/null; then
  printf '%s\n' 'generated dashboard contains host- or time-dependent data' >&2
  exit 1
fi

awk -F '\t' -v OFS='\t' '
  $1 == "evidence" && $2 == "layout" { saved = $0 }
  $1 == "row" && !inserted { print saved; inserted = 1 }
  { print }
' "${CLAIMS}" >"${TEST_ROOT}/duplicate-evidence.tsv"
expect_failure duplicate_evidence 'duplicate evidence ID: layout' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/duplicate-evidence.tsv" --repo "${REPO_ROOT}"

awk -F '\t' -v OFS='\t' '
  $1 == "row" && $2 == "02" { saved = $0 }
  { print }
  END { print saved }
' "${CLAIMS}" >"${TEST_ROOT}/duplicate-row.tsv"
expect_failure duplicate_row 'duplicate claim for parity row: 02' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/duplicate-row.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/unknown-row.tsv" row 02 2 95
expect_failure unknown_row 'claim references unknown parity row: 95' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/unknown-row.tsv" --repo "${REPO_ROOT}"

awk -F '\t' '!($1 == "row" && $2 == "02") { print }' "${CLAIMS}" \
  >"${TEST_ROOT}/missing-row.tsv"
expect_failure missing_row 'claim rows are missing or out of order: expected 02, found 03' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/missing-row.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/stale-path.tsv" \
  evidence layout 3 crates/removed/layout.rs
expect_failure stale_path 'references a stale implementation path: crates/removed/layout.rs' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/stale-path.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/traversal-path.tsv" \
  evidence layout 3 crates/../Cargo.toml
expect_failure traversal_path 'has a traversing implementation path' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/traversal-path.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/stale-command.tsv" \
  evidence codegen 4 'FE2O3_TARGET=gfx1151 scripts/removed-ci.sh rocm-compile'
expect_failure stale_command 'references a stale test command: scripts/removed-ci.sh' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/stale-command.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/generic-compile.tsv" \
  evidence atomics 6 generic
expect_failure generic_compile 'claims compile-code-object without an exact GPU lane' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/generic-compile.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/no-rocm.tsv" \
  evidence atomics 7 rust-nightly-2026-04-03
expect_failure no_rocm 'claims compile-code-object without an exact GPU lane, ROCm identity, and compile command' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/no-rocm.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/fake-local.tsv" \
  evidence scalar-ir 8 source-unit,negative-adversarial,local-hardware
expect_failure fake_local 'claims local-hardware without an exact lane and hardware command' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/fake-local.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/fake-remote.tsv" \
  evidence atomics 8 source-unit,negative-adversarial,compile-code-object,remote-hardware
expect_failure fake_remote 'claims remote-hardware without an exact lane and remote command' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/fake-remote.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/fake-refinement.tsv" \
  evidence scalar-ir 8 source-unit,negative-adversarial,machine-code-refinement
expect_failure fake_refinement 'claims machine-code-refinement without dedicated evidence' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/fake-refinement.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/unknown-strength.tsv" \
  evidence scalar-ir 8 source-unit,model-checking
expect_failure unknown_strength 'has unknown strength: model-checking' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/unknown-strength.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/stale-commit.tsv" evidence layout 5 \
  0000000000000000000000000000000000000000
expect_failure stale_commit \
  'claim layout evidence commit is not a landed descendant of the status snapshot' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/stale-commit.tsv" --repo "${REPO_ROOT}"

mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/bad-transition.tsv" row 02 3 Complete
expect_failure bad_transition 'malformed status transition for 02: claim Complete, source Partial' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${TEST_ROOT}/bad-transition.tsv" --repo "${REPO_ROOT}"

awk '
  /^\| 59 \|/ { sub(/\| N\/A \| N\/A \|/, "| Exact | N/A |") }
  { print }
' "${MATRIX}" >"${TEST_ROOT}/impossible-na.md"
expect_failure impossible_na 'impossible N/A equivalence for 59: class is Exact' \
  "${DASHBOARD_SCRIPT}" validate --status "${STATUS}" \
  --matrix "${TEST_ROOT}/impossible-na.md" --claims "${CLAIMS}" --repo "${REPO_ROOT}"

awk -F '\t' -v OFS='\t' '$1 == "normative" && $2 == "02" { $3 = "Complete" } { print }' \
  "${STATUS}" >"${TEST_ROOT}/complete-status.tsv"
awk '/^\| 02 \|/ { sub(/\| Partial \|/, "| Complete |") } { print }' \
  "${MATRIX}" >"${TEST_ROOT}/complete-matrix.md"
mutate_claim_field "${CLAIMS}" "${TEST_ROOT}/complete-claims.tsv" row 02 3 Complete
expect_failure unsupported_upgrade 'unsupported Complete upgrade for 02: missing compile-code-object evidence' \
  "${DASHBOARD_SCRIPT}" validate --status "${TEST_ROOT}/complete-status.tsv" \
  --matrix "${TEST_ROOT}/complete-matrix.md" --claims "${TEST_ROOT}/complete-claims.tsv" \
  --repo "${REPO_ROOT}"

printf 'drift\n' >>"${OUT_A}/dashboard.md"
expect_failure generated_drift 'generated Markdown drift' \
  "${DASHBOARD_SCRIPT}" check --status "${STATUS}" --matrix "${MATRIX}" \
  --claims "${CLAIMS}" --repo "${REPO_ROOT}" \
  --markdown "${OUT_A}/dashboard.md" --tsv "${OUT_A}/dashboard.tsv"

bash -n "${DASHBOARD_SCRIPT}"
bash -n "${BASH_SOURCE[0]}"
if command -v shellcheck >/dev/null 2>&1; then
  shellcheck "${DASHBOARD_SCRIPT}" "${BASH_SOURCE[0]}"
fi

printf '%s\n' 'parity dashboard tests passed'
