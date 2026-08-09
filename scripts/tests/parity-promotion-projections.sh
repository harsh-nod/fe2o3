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
readonly ARCHIVE_CLOSURE="${TEST_ROOT}/archive-closure.tsv"
readonly CHECKER="${PROTECTED}/scripts/parity-promotion-projections.sh"

file_size() {
  stat -c %s -- "$1"
}

file_sha() {
  sha256sum -- "$1" | awk '{ print $1 }'
}

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
mkdir -p "${CANDIDATE}/docs/parity-evidence/archive/results" \
  "${CANDIDATE}/docs/parity-evidence/archive/manifests"
printf 'unit evidence\n' >"${CANDIDATE}/docs/parity-evidence/archive/results/04-unit.tsv"
printf 'ui evidence\n' >"${CANDIDATE}/docs/parity-evidence/archive/results/04-ui.tsv"
printf 'signed manifest fixture\n' \
  >"${CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv"
manifest_digest="$(file_sha "${CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv")"
MANIFEST_RELATIVE="manifests/promotion-${manifest_digest}.tsv"
readonly MANIFEST_RELATIVE
mv "${CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv" \
  "${CANDIDATE}/docs/parity-evidence/archive/${MANIFEST_RELATIVE}"
[[ "$(python3 "${PROTECTED}/scripts/parity-signed-evidence.py" \
  derive-promotion-manifest \
  --protected-archive "${PROTECTED}/docs/parity-evidence/archive" \
  --candidate-archive "${CANDIDATE}/docs/parity-evidence/archive")" == \
  "${MANIFEST_RELATIVE}" ]]
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
{
  printf 'promotion_archive_closure_schema_version\t1\n'
  printf 'evidence_set_sha256\t%s\n' 'aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa'
  printf 'manifest_path\t%s\n' "${MANIFEST_RELATIVE}"
  printf 'manifest_sha256\t%s\n' "${manifest_digest}"
  printf 'file_count\t3\n'
  index=0
  for path in "${MANIFEST_RELATIVE}" results/04-ui.tsv results/04-unit.tsv; do
    printf 'file\t%04d\t%s\t%s\t%s\n' "${index}" "${path}" \
      "$(file_size "${CANDIDATE}/docs/parity-evidence/archive/${path}")" \
      "$(file_sha "${CANDIDATE}/docs/parity-evidence/archive/${path}")"
    ((index += 1))
  done
} >"${ARCHIVE_CLOSURE}"

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
cp "${CANDIDATE}/docs/parity-evidence/archive/${MANIFEST_RELATIVE}" \
  "${TEST_ROOT}/good/promotion.tsv"
cp "${CANDIDATE}/docs/parity-evidence/archive/results/04-unit.tsv" \
  "${TEST_ROOT}/good/04-unit.tsv"
cp "${CANDIDATE}/docs/parity-evidence/archive/results/04-ui.tsv" \
  "${TEST_ROOT}/good/04-ui.tsv"

reset_candidate() {
  cp "${TEST_ROOT}/good/matrix.md" "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
  cp "${TEST_ROOT}/good/dashboard.md" "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md"
  cp "${TEST_ROOT}/good/dashboard.tsv" "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv"
  rm -f "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
  cp "${TEST_ROOT}/good/ledger.tsv" "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
  rm -f "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
  cp "${TEST_ROOT}/good/prior.tsv" "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
  cp "${TEST_ROOT}/good/promotion.tsv" \
    "${CANDIDATE}/docs/parity-evidence/archive/${MANIFEST_RELATIVE}"
  cp "${TEST_ROOT}/good/04-unit.tsv" \
    "${CANDIDATE}/docs/parity-evidence/archive/results/04-unit.tsv"
  cp "${TEST_ROOT}/good/04-ui.tsv" \
    "${CANDIDATE}/docs/parity-evidence/archive/results/04-ui.tsv"
  rm -f "${CANDIDATE}/docs/parity-evidence/archive/results/unreferenced.tsv"
  rm -rf "${CANDIDATE}/docs/parity-evidence/archive/results/reserved"
}

bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" \
  "${ARCHIVE_CLOSURE}"
grep -F '| 04 | Pointer Distance (' "${CANDIDATE}/docs/cuda-oxide-parity-matrix.md" |
  grep -F '| Partial |' >/dev/null
grep -F $'normative\t04\tPointer Distance (' \
  "${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv" |
  grep -F $'\tPartial\t' >/dev/null

printf '\nCandidate-authored prose.\n' >>"${CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
expect_failure matrix_prose 'candidate parity matrix is not the protected deterministic projection' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

printf '\nforged dashboard prose\n' >>"${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md"
expect_failure markdown_dashboard 'candidate Markdown dashboard is not the protected deterministic projection' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

printf 'forged\n' >>"${CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv"
expect_failure tsv_dashboard 'candidate TSV dashboard is not the protected deterministic projection' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

sed -i 's/aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa/bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb/' \
  "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
expect_failure ledger_substitution 'candidate signed-promotion ledger is not the canonical transaction merge' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

printf 'mutated evidence\n' >"${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
expect_failure archive_mutation 'candidate mutated protected evidence file: history/prior.tsv' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

rm "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
ln -s ../../results/04-unit.tsv "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
expect_failure archive_symlink 'candidate removed or replaced protected evidence file: history/prior.tsv' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

rm "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
ln -s ../cuda-oxide-parity-dashboard.tsv \
  "${CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
expect_failure ledger_symlink 'required projection input is not a regular file' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

printf 'unreferenced evidence\n' \
  >"${CANDIDATE}/docs/parity-evidence/archive/results/unreferenced.tsv"
expect_failure archive_extra_file \
  'candidate evidence archive has unreferenced file: results/unreferenced.tsv' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

mkdir "${CANDIDATE}/docs/parity-evidence/archive/results/reserved"
expect_failure archive_namespace \
  'candidate evidence archive has unreferenced namespace: results/reserved' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

printf 'substituted evidence\n' \
  >"${CANDIDATE}/docs/parity-evidence/archive/results/04-unit.tsv"
expect_failure archive_digest \
  'candidate evidence file violates signed closure: results/04-unit.tsv' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" "${ARCHIVE_CLOSURE}"
reset_candidate

{
  head -n 4 "${ARCHIVE_CLOSURE}"
  printf 'file_count\t2\n'
  awk -F '\t' -v OFS='\t' -v manifest="${MANIFEST_RELATIVE}" '
    $1 == "file" && $3 == manifest { $2 = "0000"; print }
    $1 == "file" && $3 == "results/04-unit.tsv" { $2 = "0001"; print }
  ' "${ARCHIVE_CLOSURE}"
} >"${TEST_ROOT}/incomplete-archive-closure.tsv"
expect_failure archive_incomplete \
  'promotion archive closure omits transaction result: results/04-ui.tsv' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" \
    "${TEST_ROOT}/incomplete-archive-closure.tsv"

awk -F '\t' -v OFS='\t' '
  $1 == "evidence_set_sha256" { $2 = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb" }
  { print }
' "${ARCHIVE_CLOSURE}" >"${TEST_ROOT}/rebound-archive-closure.tsv"
expect_failure archive_rebound \
  'promotion archive closure does not bind the transaction evidence set' \
  bash "${CHECKER}" "${PROTECTED}" "${CANDIDATE}" "${TRANSACTION}" \
    "${TEST_ROOT}/rebound-archive-closure.tsv"

# A later protected base retains the first manifest and deterministically
# selects one newly appended manifest for a Partial-to-Complete transaction.
SECOND_PROTECTED="${TEST_ROOT}/second-protected"
SECOND_CANDIDATE="${TEST_ROOT}/second-candidate"
SECOND_TRANSACTION="${TEST_ROOT}/second-transaction.tsv"
SECOND_CLOSURE="${TEST_ROOT}/second-closure.tsv"
cp -a "${CANDIDATE}" "${SECOND_PROTECTED}"
cp -a "${SECOND_PROTECTED}" "${SECOND_CANDIDATE}"
first_manifest_before="$(file_sha \
  "${SECOND_PROTECTED}/docs/parity-evidence/archive/${MANIFEST_RELATIVE}")"
printf 'second signed manifest fixture\n' \
  >"${SECOND_CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv"
second_manifest_digest="$(file_sha \
  "${SECOND_CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv")"
SECOND_MANIFEST_RELATIVE="manifests/promotion-${second_manifest_digest}.tsv"
readonly SECOND_MANIFEST_RELATIVE
mv "${SECOND_CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv" \
  "${SECOND_CANDIDATE}/docs/parity-evidence/archive/${SECOND_MANIFEST_RELATIVE}"
for class in unit ui ir compile; do
  printf '%s complete evidence\n' "${class}" \
    >"${SECOND_CANDIDATE}/docs/parity-evidence/archive/results/04-complete-${class}.tsv"
done
[[ "$(python3 "${SECOND_PROTECTED}/scripts/parity-signed-evidence.py" \
  derive-promotion-manifest \
  --protected-archive "${SECOND_PROTECTED}/docs/parity-evidence/archive" \
  --candidate-archive "${SECOND_CANDIDATE}/docs/parity-evidence/archive")" == \
  "${SECOND_MANIFEST_RELATIVE}" ]]
[[ "$(file_sha \
  "${SECOND_CANDIDATE}/docs/parity-evidence/archive/${MANIFEST_RELATIVE}")" == \
  "${first_manifest_before}" ]]
awk -F '\t' -v OFS='\t' -v source="${SOURCE}" '
  $1 == "fe2o3_commit" { $2 = source }
  $1 == "normative" && $2 == "04" { $3 = "Complete" }
  { print }
' "${SECOND_PROTECTED}/docs/cuda-oxide-parity-status.tsv" \
  >"${SECOND_CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
{
  printf 'signed_promotion_projection_schema_version\t1\n'
  printf 'row_count\t1\n'
  printf 'row\t0000\t04\tPartial\tComplete\t%s\tgfx942\tmi300x-gfx942-release\t' "${SOURCE}"
  printf 'unit,ui,ir,compile\tbash\t'
  printf 'results/04-complete-unit.tsv,results/04-complete-ui.tsv,results/04-complete-ir.tsv,results/04-complete-compile.tsv\t%s\n' \
    'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd'
} >"${SECOND_TRANSACTION}"
{
  printf 'promotion_archive_closure_schema_version\t1\n'
  printf 'evidence_set_sha256\t%s\n' 'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd'
  printf 'manifest_path\t%s\n' "${SECOND_MANIFEST_RELATIVE}"
  printf 'manifest_sha256\t%s\n' "${second_manifest_digest}"
  printf 'file_count\t5\n'
  index=0
  for path in \
    "${SECOND_MANIFEST_RELATIVE}" \
    results/04-complete-compile.tsv \
    results/04-complete-ir.tsv \
    results/04-complete-ui.tsv \
    results/04-complete-unit.tsv; do
    printf 'file\t%04d\t%s\t%s\t%s\n' "${index}" "${path}" \
      "$(file_size "${SECOND_CANDIDATE}/docs/parity-evidence/archive/${path}")" \
      "$(file_sha "${SECOND_CANDIDATE}/docs/parity-evidence/archive/${path}")"
    ((index += 1))
  done
} >"${SECOND_CLOSURE}"
rm "${SECOND_CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
python3 "${SECOND_PROTECTED}/scripts/parity-signed-evidence.py" merge-projections \
  --baseline "${SECOND_PROTECTED}/docs/generated/cuda-oxide-parity-signed-promotions.tsv" \
  --transaction "${SECOND_TRANSACTION}" \
  --output "${SECOND_CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
cp "${SECOND_PROTECTED}/docs/cuda-oxide-parity-matrix.md" \
  "${SECOND_CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
bash "${SECOND_PROTECTED}/scripts/parity-matrix.sh" generate \
  "${SECOND_CANDIDATE}/docs/cuda-oxide-parity-status.tsv" \
  "${SECOND_CANDIDATE}/docs/cuda-oxide-parity-matrix.md" >/dev/null
bash "${SECOND_PROTECTED}/scripts/parity-dashboard.sh" update \
  --status "${SECOND_CANDIDATE}/docs/cuda-oxide-parity-status.tsv" \
  --matrix "${SECOND_CANDIDATE}/docs/cuda-oxide-parity-matrix.md" \
  --repo "${SECOND_CANDIDATE}" \
  --signed-promotions "${SECOND_CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv" \
  --markdown "${SECOND_CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md" \
  --tsv "${SECOND_CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv" >/dev/null
bash "${SECOND_PROTECTED}/scripts/parity-promotion-projections.sh" \
  "${SECOND_PROTECTED}" "${SECOND_CANDIDATE}" \
  "${SECOND_TRANSACTION}" "${SECOND_CLOSURE}"
awk -F '\t' '
  $1 == "normative" && $2 == "04" && $6 == "Complete" { found++ }
  END { exit found == 1 ? 0 : 1 }
' "${SECOND_CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv"

{
  printf 'promotion_archive_closure_schema_version\t1\n'
  printf 'evidence_set_sha256\t%s\n' 'dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd'
  printf 'manifest_path\t%s\n' "${MANIFEST_RELATIVE}"
  printf 'manifest_sha256\t%s\n' "${first_manifest_before}"
  printf 'file_count\t5\n'
  index=0
  for path in \
    "${MANIFEST_RELATIVE}" \
    results/04-complete-compile.tsv \
    results/04-complete-ir.tsv \
    results/04-complete-ui.tsv \
    results/04-complete-unit.tsv; do
    printf 'file\t%04d\t%s\t%s\t%s\n' "${index}" "${path}" \
      "$(file_size "${SECOND_CANDIDATE}/docs/parity-evidence/archive/${path}")" \
      "$(file_sha "${SECOND_CANDIDATE}/docs/parity-evidence/archive/${path}")"
    ((index += 1))
  done
} >"${TEST_ROOT}/replayed-manifest-closure.tsv"
expect_failure manifest_replay_projection \
  'promotion archive closure replays a protected manifest' \
  bash "${SECOND_PROTECTED}/scripts/parity-promotion-projections.sh" \
    "${SECOND_PROTECTED}" "${SECOND_CANDIDATE}" \
    "${SECOND_TRANSACTION}" "${TEST_ROOT}/replayed-manifest-closure.tsv"

# Exercise the real legacy-claim shape through every protected projection.
LEGACY_CANDIDATE="${TEST_ROOT}/legacy-candidate"
LEGACY_TRANSACTION="${TEST_ROOT}/legacy-transaction.tsv"
LEGACY_CLOSURE="${TEST_ROOT}/legacy-closure.tsv"
cp -a "${PROTECTED}" "${LEGACY_CANDIDATE}"
mkdir -p "${LEGACY_CANDIDATE}/docs/parity-evidence/archive/manifests" \
  "${LEGACY_CANDIDATE}/docs/parity-evidence/archive/results"
printf 'legacy completion manifest\n' \
  >"${LEGACY_CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv"
legacy_manifest_digest="$(file_sha "${LEGACY_CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv")"
LEGACY_MANIFEST_RELATIVE="manifests/promotion-${legacy_manifest_digest}.tsv"
readonly LEGACY_MANIFEST_RELATIVE
mv "${LEGACY_CANDIDATE}/docs/parity-evidence/archive/manifests/new.tsv" \
  "${LEGACY_CANDIDATE}/docs/parity-evidence/archive/${LEGACY_MANIFEST_RELATIVE}"
for class in unit ui ir compile verus hardware; do
  printf '%s completion evidence\n' "${class}" \
    >"${LEGACY_CANDIDATE}/docs/parity-evidence/archive/results/61-${class}.tsv"
done
awk -F '\t' -v OFS='\t' -v source="${SOURCE}" '
  $1 == "fe2o3_commit" { $2 = source }
  $1 == "normative" && $2 == "61" { $3 = "Complete" }
  { print }
' "${PROTECTED}/docs/cuda-oxide-parity-status.tsv" \
  >"${LEGACY_CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
{
  printf 'signed_promotion_projection_schema_version\t1\n'
  printf 'row_count\t1\n'
  printf 'row\t0000\t61\tPartial\tComplete\t%s\tgfx942\tmi300x-gfx942-release\t' "${SOURCE}"
  printf 'unit,ui,ir,compile,verus,hardware\tbash\t'
  printf 'results/61-unit.tsv,results/61-ui.tsv,results/61-ir.tsv,results/61-compile.tsv,results/61-verus.tsv,results/61-hardware.tsv\t%s\n' \
    'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc'
} >"${LEGACY_TRANSACTION}"
{
  printf 'promotion_archive_closure_schema_version\t1\n'
  printf 'evidence_set_sha256\t%s\n' 'cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc'
  printf 'manifest_path\t%s\n' "${LEGACY_MANIFEST_RELATIVE}"
  printf 'manifest_sha256\t%s\n' "${legacy_manifest_digest}"
  printf 'file_count\t7\n'
  index=0
  for path in \
    "${LEGACY_MANIFEST_RELATIVE}" \
    results/61-compile.tsv \
    results/61-hardware.tsv \
    results/61-ir.tsv \
    results/61-ui.tsv \
    results/61-unit.tsv \
    results/61-verus.tsv; do
    printf 'file\t%04d\t%s\t%s\t%s\n' "${index}" "${path}" \
      "$(file_size "${LEGACY_CANDIDATE}/docs/parity-evidence/archive/${path}")" \
      "$(file_sha "${LEGACY_CANDIDATE}/docs/parity-evidence/archive/${path}")"
    ((index += 1))
  done
} >"${LEGACY_CLOSURE}"
rm "${LEGACY_CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
python3 "${PROTECTED}/scripts/parity-signed-evidence.py" merge-projections \
  --baseline "${PROTECTED}/docs/generated/cuda-oxide-parity-signed-promotions.tsv" \
  --transaction "${LEGACY_TRANSACTION}" \
  --output "${LEGACY_CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
cp "${PROTECTED}/docs/cuda-oxide-parity-matrix.md" \
  "${LEGACY_CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
bash "${PROTECTED}/scripts/parity-matrix.sh" generate \
  "${LEGACY_CANDIDATE}/docs/cuda-oxide-parity-status.tsv" \
  "${LEGACY_CANDIDATE}/docs/cuda-oxide-parity-matrix.md" >/dev/null
bash "${PROTECTED}/scripts/parity-dashboard.sh" update \
  --status "${LEGACY_CANDIDATE}/docs/cuda-oxide-parity-status.tsv" \
  --matrix "${LEGACY_CANDIDATE}/docs/cuda-oxide-parity-matrix.md" \
  --repo "${LEGACY_CANDIDATE}" \
  --signed-promotions "${LEGACY_CANDIDATE}/docs/generated/cuda-oxide-parity-signed-promotions.tsv" \
  --markdown "${LEGACY_CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.md" \
  --tsv "${LEGACY_CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv" >/dev/null
bash "${CHECKER}" "${PROTECTED}" "${LEGACY_CANDIDATE}" \
  "${LEGACY_TRANSACTION}" "${LEGACY_CLOSURE}"
awk -F '\t' '
  $1 == "normative" && $2 == "61" && $6 == "Complete" { found++ }
  END { exit found == 1 ? 0 : 1 }
' "${LEGACY_CANDIDATE}/docs/generated/cuda-oxide-parity-dashboard.tsv"

bash -n "${BASH_SOURCE[0]}" "${CHECKER}"
shellcheck "${BASH_SOURCE[0]}" "${CHECKER}"
printf 'protected parity projection adversarial tests passed\n'
