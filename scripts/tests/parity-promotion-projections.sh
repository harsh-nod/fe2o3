#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf -- "${TEST_ROOT}"' EXIT
readonly PROTECTED="${TEST_ROOT}/protected"
readonly CANDIDATE="${TEST_ROOT}/candidate"
readonly TRANSACTION="${TEST_ROOT}/transaction.tsv"
readonly CHECKER="${PROTECTED}/scripts/parity-promotion-projections.sh"

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'negative projection test unexpectedly passed: %s\n' "${name}" >&2
    return 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'negative projection test produced the wrong diagnostic: %s\n' "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    return 1
  }
}

git clone -q --no-hardlinks "${ROOT}" "${PROTECTED}"
for path in \
  scripts/parity-signed-evidence.py \
  scripts/parity-matrix.sh \
  scripts/parity-dashboard.sh \
  scripts/parity-promotion-projections.sh \
  docs/generated/cuda-oxide-parity-signed-promotions.tsv; do
  cp -- "${ROOT}/${path}" "${PROTECTED}/${path}"
done
mkdir -p "${PROTECTED}/docs/parity-evidence/archive/history"
printf 'immutable evidence\n' >"${PROTECTED}/docs/parity-evidence/archive/history/prior.tsv"
cp -a "${PROTECTED}" "${CANDIDATE}"

SOURCE="$(git -C "${CANDIDATE}" rev-parse HEAD)"
readonly SOURCE
mkdir -p "${CANDIDATE}/docs/parity-evidence/archive/results"
printf 'unit evidence\n' >"${CANDIDATE}/docs/parity-evidence/archive/results/04-unit.tsv"
printf 'ui evidence\n' >"${CANDIDATE}/docs/parity-evidence/archive/results/04-ui.tsv"
awk -F '\t' -v OFS='\t' -v source="${SOURCE}" '
  $1 == "fe2o3_commit" { $2 = source }
  $1 == "normative" && $2 == "04" { $3 = "Partial" }
  { print }
' "${PROTECTED}/docs/cuda-oxide-parity-status.tsv" >"${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
cat >"${TRANSACTION}" <<EOF
signed_promotion_projection_schema_version	1
row_count	1
row	0000	04	Missing	Partial	${SOURCE}	gfx942	mi300x-gfx942-release	unit,ui	bash	results/04-unit.tsv,results/04-ui.tsv	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
EOF

rm "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
python3 "${PROTECTED}/scripts/parity-signed-evidence.py" merge-projections \
  --baseline "${PROTECTED}/docs/generated/cuda-oxide-parity-signed-promotions.tsv" \
  --transaction "${TRANSACTION}" \
  --output "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
cp "${PROTECTED}/docs/cuda-oxide-parity-matrix.md" \
  "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
bash "${PROTECTED}/scripts/parity-matrix.sh" generate \
  "${CANDIDATE}/docs/cuda-oxide-parity-status.tsv" \
  "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md" >/dev/null
bash "${PROTECTED}/scripts/parity-dashboard.sh" update \
  --status "${CANDIDATE}/docs/cuda-oxide-parity-status.tsv" \
  --matrix "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md" \
  --repo "${CANDIDATE}" \
  --signed-promotions "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv" \
  --markdown "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md" \
  --tsv "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv" >/dev/null

mkdir -p "${TEST_ROOT}/good"
cp "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md" "${TEST_ROOT}/good/matrix.md"
cp "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md" "${TEST_ROOT}/good/dashboard.md"
cp "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv" "${TEST_ROOT}/good/dashboard.tsv"
cp "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv" "${TEST_ROOT}/good/ledger.tsv"
cp "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv" "${TEST_ROOT}/good/prior.tsv"

reset_candidate() {
  cp "${TEST_ROOT}/good/matrix.md" "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
  cp "${TEST_ROOT}/good/dashboard.md" "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md"
  cp "${TEST_ROOT}/good/dashboard.tsv" "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv"
  rm -f "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
  cp "${TEST_ROOT}/good/ledger.tsv" "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
  rm -f "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
  cp "${TEST_ROOT}/good/prior.tsv" "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
}

bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
grep -F '| 04 | Pointer Distance (' "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md" |
  grep -F '| Partial |' >/dev/null
grep -F $'normative\t04\tPointer Distance (' \
  "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv" |
  grep -F $'\tPartial\t' >/dev/null

printf '\nCandidate-authored prose.\n' >>"${CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
expect_failure matrix_prose 'candidate parity matrix is not the protected deterministic projection' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
reset_candidate

printf '\nforged dashboard prose\n' >>"${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md"
expect_failure markdown_dashboard 'candidate Markdown dashboard is not the protected deterministic projection' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
reset_candidate

printf 'forged\n' >>"${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv"
expect_failure tsv_dashboard 'candidate TSV dashboard is not the protected deterministic projection' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
reset_candidate

sed -i 's/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/' \
  "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
expect_failure ledger_substitution 'candidate signed-promotion ledger is not the canonical transaction merge' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
reset_candidate

printf 'mutated evidence\n' >"${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
expect_failure archive_mutation 'candidate mutated protected evidence file: history/prior.tsv' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
reset_candidate

rm "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
ln -s ../../results/04-unit.tsv "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
expect_failure archive_symlink 'candidate removed or replaced protected evidence file: history/prior.tsv' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
reset_candidate

rm "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
ln -s ../cuda-oxide-parity-dashboard.tsv \
  "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
expect_failure ledger_symlink 'required projection input is not a regular file' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}"
reset_candidate

bash -n "${BASH_SOURCE[0]}" "${CHECKER}"
shellcheck "${BASH_SOURCE[0]}" "${CHECKER}"
printf 'protected parity projection adversarial tests passed\n'
