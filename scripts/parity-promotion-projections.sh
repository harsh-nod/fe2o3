#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

die() {
  printf 'protected parity projections: %s\n' "$1" >&2
  exit 2
}

[[ "$#" == 4 ]] ||
  die 'usage: PROTECTED_ROOT CANDIDATE_ROOT TRANSACTION_PROJECTION ARCHIVE_CLOSURE'

PROTECTED_ROOT="$(realpath -e -- "$1")" ||
  die 'protected root does not resolve'
readonly PROTECTED_ROOT
CANDIDATE_ROOT="$(realpath -e -- "$2")" ||
  die 'candidate root does not resolve'
readonly CANDIDATE_ROOT
TRANSACTION="$(realpath -e -- "$3")" ||
  die 'transaction projection does not resolve'
readonly TRANSACTION
ARCHIVE_CLOSURE="$(realpath -e -- "$4")" ||
  die 'archive closure does not resolve'
readonly ARCHIVE_CLOSURE
[[ -d "${PROTECTED_ROOT}" && ! -L "$1" ]] ||
  die 'protected root must be a real directory'
[[ -d "${CANDIDATE_ROOT}" && ! -L "$2" ]] ||
  die 'candidate root must be a real directory'
[[ -f "${TRANSACTION}" && ! -L "$3" ]] ||
  die 'transaction projection must be a regular file'
[[ -f "${ARCHIVE_CLOSURE}" && ! -L "$4" ]] ||
  die 'archive closure must be a regular file'

readonly STATUS=docs/cuda-oxide-parity-status.tsv
readonly MATRIX=docs/cuda-oxide-parity-matrix.md
readonly DASHBOARD_MD=docs/generated/cuda-oxide-parity-dashboard.md
readonly DASHBOARD_TSV=docs/generated/cuda-oxide-parity-dashboard.tsv
readonly LEDGER=docs/generated/cuda-oxide-parity-signed-promotions.tsv
readonly ARCHIVE=docs/parity-evidence/archive
readonly VERIFIER="${PROTECTED_ROOT}/scripts/parity-signed-evidence.py"
readonly MATRIX_GENERATOR="${PROTECTED_ROOT}/scripts/parity-matrix.sh"
readonly DASHBOARD_GENERATOR="${PROTECTED_ROOT}/scripts/parity-dashboard.sh"

required_paths=(
  "${VERIFIER}"
  "${MATRIX_GENERATOR}"
  "${DASHBOARD_GENERATOR}"
  "${PROTECTED_ROOT}/${STATUS}"
  "${PROTECTED_ROOT}/${MATRIX}"
  "${PROTECTED_ROOT}/${DASHBOARD_MD}"
  "${PROTECTED_ROOT}/${DASHBOARD_TSV}"
  "${PROTECTED_ROOT}/${LEDGER}"
  "${CANDIDATE_ROOT}/${STATUS}"
  "${CANDIDATE_ROOT}/${MATRIX}"
  "${CANDIDATE_ROOT}/${DASHBOARD_MD}"
  "${CANDIDATE_ROOT}/${DASHBOARD_TSV}"
  "${CANDIDATE_ROOT}/${LEDGER}"
)
for path in "${required_paths[@]}"; do
  [[ -f "${path}" && ! -L "${path}" ]] ||
    die "required projection input is not a regular file: ${path}"
done

TEMP_ROOT="$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-parity-projections.XXXXXX")"
readonly TEMP_ROOT
trap 'rm -rf -- "${TEMP_ROOT}"' EXIT

check_archive_entry() {
  local path="$1"
  local root="$2"
  local relative="${path#"${root}/"}"
  local candidate="${CANDIDATE_ROOT}/${ARCHIVE}/${relative}"
  if [[ -d "${path}" && ! -L "${path}" ]]; then
    [[ -d "${candidate}" && ! -L "${candidate}" ]] ||
      die "candidate removed or replaced protected evidence directory: ${relative}"
  elif [[ -f "${path}" && ! -L "${path}" ]]; then
    [[ -f "${candidate}" && ! -L "${candidate}" ]] ||
      die "candidate removed or replaced protected evidence file: ${relative}"
    cmp -s -- "${path}" "${candidate}" ||
      die "candidate mutated protected evidence file: ${relative}"
  else
    die "protected evidence archive contains a non-regular entry: ${relative}"
  fi
}

protected_archive="${PROTECTED_ROOT}/${ARCHIVE}"
candidate_archive="${CANDIDATE_ROOT}/${ARCHIVE}"
if [[ -e "${protected_archive}" || -L "${protected_archive}" ]]; then
  [[ -d "${protected_archive}" && ! -L "${protected_archive}" ]] ||
    die 'protected evidence archive is not a real directory'
  [[ "$(realpath -e -- "${protected_archive}")" == "${protected_archive}" ]] ||
    die 'protected evidence archive has a symlinked ancestor'
  [[ -d "${candidate_archive}" && ! -L "${candidate_archive}" ]] ||
    die 'candidate removed or replaced the protected evidence archive'
  while IFS= read -r -d '' path; do
    check_archive_entry "${path}" "${protected_archive}"
  done < <(find -P "${protected_archive}" -mindepth 1 -print0)
fi
if [[ -e "${candidate_archive}" || -L "${candidate_archive}" ]]; then
  [[ -d "${candidate_archive}" && ! -L "${candidate_archive}" ]] ||
    die 'candidate evidence archive is not a real directory'
  [[ "$(realpath -e -- "${candidate_archive}")" == "${candidate_archive}" ]] ||
    die 'candidate evidence archive has a symlinked ancestor'
  while IFS= read -r -d '' path; do
    [[ (-d "${path}" || -f "${path}") && ! -L "${path}" ]] ||
      die 'candidate evidence archive contains a non-regular entry'
  done < <(find -P "${candidate_archive}" -mindepth 1 -print0)
fi

python3 "${VERIFIER}" verify-promotion-archive \
  --protected-archive "${protected_archive}" \
  --candidate-archive "${candidate_archive}" \
  --transaction "${TRANSACTION}" \
  --closure "${ARCHIVE_CLOSURE}"

expected_ledger="${TEMP_ROOT}/signed-promotions.tsv"
python3 "${VERIFIER}" merge-projections --baseline "${PROTECTED_ROOT}/${LEDGER}" --transaction "${TRANSACTION}" --output "${expected_ledger}"
cmp -s -- "${expected_ledger}" "${CANDIDATE_ROOT}/${LEDGER}" ||
  die 'candidate signed-promotion ledger is not the canonical transaction merge'

expected_matrix="${TEMP_ROOT}/matrix.md"
cp -- "${PROTECTED_ROOT}/${MATRIX}" "${expected_matrix}"
bash "${MATRIX_GENERATOR}" generate "${CANDIDATE_ROOT}/${STATUS}" "${expected_matrix}" >/dev/null
cmp -s -- "${expected_matrix}" "${CANDIDATE_ROOT}/${MATRIX}" ||
  die 'candidate parity matrix is not the protected deterministic projection'

expected_dashboard_md="${TEMP_ROOT}/dashboard.md"
expected_dashboard_tsv="${TEMP_ROOT}/dashboard.tsv"
bash "${DASHBOARD_GENERATOR}" update --status "${CANDIDATE_ROOT}/${STATUS}" --matrix "${expected_matrix}" --repo "${CANDIDATE_ROOT}" --signed-promotions "${expected_ledger}" --markdown "${expected_dashboard_md}" --tsv "${expected_dashboard_tsv}" >/dev/null
cmp -s -- "${expected_dashboard_md}" "${CANDIDATE_ROOT}/${DASHBOARD_MD}" ||
  die 'candidate Markdown dashboard is not the protected deterministic projection'
cmp -s -- "${expected_dashboard_tsv}" "${CANDIDATE_ROOT}/${DASHBOARD_TSV}" ||
  die 'candidate TSV dashboard is not the protected deterministic projection'

printf 'protected parity projections are canonical and archive history is immutable\n'
