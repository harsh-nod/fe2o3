#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

die() {
  printf 'protected parity change policy: %s\n' "$1" >&2
  exit 2
}

[[ "$#" == 8 ]] || die 'usage: PROTECTED_ROOT CANDIDATE_ROOT STATUS_PATH REVIEWS_JSON REVIEWERS_FILE CHANGED_FILES_JSON EXPECTED_CHANGE_COUNT CANDIDATE_HEAD'

PROTECTED_ROOT="$(realpath -e -- "$1")" || die 'protected root does not resolve'
readonly PROTECTED_ROOT
CANDIDATE_ROOT="$(realpath -e -- "$2")" || die 'candidate root does not resolve'
readonly CANDIDATE_ROOT
readonly STATUS_PATH="$3"
readonly REVIEWS_JSON="$4"
readonly REVIEWERS_FILE="$5"
readonly CHANGED_FILES_JSON="$6"
readonly EXPECTED_CHANGE_COUNT="$7"
readonly CANDIDATE_HEAD="$8"

[[ -d "${PROTECTED_ROOT}" && ! -L "$1" ]] || die 'protected root must be a real directory'
[[ -d "${CANDIDATE_ROOT}" && ! -L "$2" ]] || die 'candidate root must be a real directory'
[[ "${STATUS_PATH}" == docs/cuda-oxide-parity-status.tsv ]] || die 'unexpected status path'
[[ "${EXPECTED_CHANGE_COUNT}" =~ ^[1-9][0-9]*$ ]] || die 'expected change count is malformed or empty'
[[ "${CANDIDATE_HEAD}" =~ ^[0-9a-f]{40}$ ]] || die 'candidate head is malformed'

readonly -a TRUST_FILES=(
  docs/parity-signed-evidence-v2.md
  docs/parity-evidence/trust-policy-v2.example.tsv
  docs/parity-row-evidence-policy-v2.tsv
  docs/parity-evidence/trust-policy-v2.tsv
  scripts/parity-dashboard.sh
  scripts/parity-matrix.sh
  scripts/parity-promotion-projections.sh
  scripts/parity-signed-evidence.py
  scripts/parity-protected-change-policy.sh
  scripts/tests/hosted-parity-ci.sh
  scripts/tests/parity-dashboard.sh
  scripts/tests/parity-promotion-projections.sh
  scripts/tests/parity-row-evidence.sh
  .github/workflows/ci.yml
  .github/workflows/parity-promotion.yml
  .github/CODEOWNERS
  .github/parity-trust-reviewers.txt
)
readonly TRUST_KEY_DIRECTORY=docs/parity-evidence/trusted-keys
readonly PROMOTION_LEDGER=docs/generated/cuda-oxide-parity-signed-promotions.tsv
readonly -a PROMOTION_PROJECTIONS=(
  docs/cuda-oxide-parity-matrix.md
  docs/generated/cuda-oxide-parity-dashboard.md
  docs/generated/cuda-oxide-parity-dashboard.tsv
  "${PROMOTION_LEDGER}"
)

file_changed() {
  local relative="$1"
  local protected="${PROTECTED_ROOT}/${relative}"
  local candidate="${CANDIDATE_ROOT}/${relative}"
  if [[ -L "${protected}" || -L "${candidate}" ]]; then
    return 0
  fi
  if [[ -f "${protected}" && -f "${candidate}" ]]; then
    if cmp -s -- "${protected}" "${candidate}"; then
      return 1
    fi
    return 0
  fi
  [[ -e "${protected}" || -e "${candidate}" ]]
}

directory_changed() {
  local relative="$1"
  local protected="${PROTECTED_ROOT}/${relative}"
  local candidate="${CANDIDATE_ROOT}/${relative}"
  if [[ -L "${protected}" || -L "${candidate}" ]]; then
    return 0
  fi
  if [[ ! -e "${protected}" && ! -e "${candidate}" ]]; then
    return 1
  fi
  if [[ ! -d "${protected}" || ! -d "${candidate}" ]]; then
    return 0
  fi
  if diff --no-dereference -qr -- "${protected}" "${candidate}" >/dev/null; then
    return 1
  fi
  return 0
}

validate_candidate_trust_types() {
  local relative
  local path
  for relative in "${TRUST_FILES[@]}"; do
    path="${CANDIDATE_ROOT}/${relative}"
    if [[ -e "${path}" || -L "${path}" ]]; then
      [[ -f "${path}" && ! -L "${path}" ]] || die "candidate trust path is not a regular file: ${relative}"
    fi
  done
  path="${CANDIDATE_ROOT}/${TRUST_KEY_DIRECTORY}"
  if [[ -e "${path}" || -L "${path}" ]]; then
    [[ -d "${path}" && ! -L "${path}" ]] || die 'candidate trusted-keys path is not a real directory'
    while IFS= read -r -d '' path; do
      [[ -f "${path}" && ! -L "${path}" ]] || die 'candidate trusted-keys contains a non-regular entry'
    done < <(find "${path}" -mindepth 1 -print0)
  fi
}

validate_changed_paths() {
  local actual_count
  local path
  local trusted
  [[ -f "${CHANGED_FILES_JSON}" && ! -L "${CHANGED_FILES_JSON}" ]] ||
    die 'runner changed-file data is missing'
  jq -e '
    def valid_path:
      type == "string" and length > 0 and
      (startswith("/") | not) and
      (contains("\u0000") | not) and
      (contains("\n") | not) and
      (split("/") | all(. != "" and . != "." and . != ".."));
    type == "array" and length > 0 and
    all(.[]; type == "object" and (.filename | valid_path) and
      ((has("previous_filename") | not) or (.previous_filename | valid_path)))
  ' "${CHANGED_FILES_JSON}" >/dev/null || die 'runner changed-file data is malformed or empty'
  actual_count="$(jq -r 'length' "${CHANGED_FILES_JSON}")"
  [[ "${actual_count}" == "${EXPECTED_CHANGE_COUNT}" ]] ||
    die 'runner changed-file count does not match pull request'

  while IFS= read -r path; do
    trusted=false
    case "${path}" in
      docs/parity-signed-evidence-v2.md|docs/parity-evidence/trust-policy-v2.example.tsv|docs/parity-row-evidence-policy-v2.tsv|docs/parity-evidence/trust-policy-v2.tsv|scripts/parity-dashboard.sh|scripts/parity-matrix.sh|scripts/parity-promotion-projections.sh|scripts/parity-signed-evidence.py|scripts/parity-protected-change-policy.sh|scripts/tests/hosted-parity-ci.sh|scripts/tests/parity-dashboard.sh|scripts/tests/parity-promotion-projections.sh|scripts/tests/parity-row-evidence.sh|.github/workflows/ci.yml|.github/workflows/parity-promotion.yml|.github/CODEOWNERS|.github/parity-trust-reviewers.txt)
        trusted=true
        ;;
      "${PROMOTION_LEDGER}")
        trusted=true
        ;;
      docs/parity-evidence/trusted-keys/*)
        trusted=true
        ;;
    esac
    [[ "${trusted}" == true ]] ||
      die "trust changes must be isolated from arbitrary paths: ${path}"
  done < <(jq -r '.[] | .filename, (.previous_filename // empty)' "${CHANGED_FILES_JSON}")
}

status_changed=false
if file_changed "${STATUS_PATH}"; then
  status_changed=true
fi

projection_changed=false
for relative in "${PROMOTION_PROJECTIONS[@]}"; do
  if file_changed "${relative}"; then
    projection_changed=true
  fi
done

trust_changed=false
for relative in "${TRUST_FILES[@]}"; do
  if file_changed "${relative}"; then
    trust_changed=true
  fi
done
if directory_changed "${TRUST_KEY_DIRECTORY}"; then
  trust_changed=true
fi

if [[ "${trust_changed}" == true ]]; then
  [[ "${status_changed}" == false ]] || die 'trust changes must not include parity status changes'
  if [[ "${projection_changed}" == true ]]; then
    [[ ! -e "${PROTECTED_ROOT}/${PROMOTION_LEDGER}" &&
      -f "${CANDIDATE_ROOT}/${PROMOTION_LEDGER}" &&
      ! -L "${CANDIDATE_ROOT}/${PROMOTION_LEDGER}" ]] ||
      die 'trust changes must not include parity projection changes'
    [[ "$(cat -- "${CANDIDATE_ROOT}/${PROMOTION_LEDGER}")" == $'signed_promotion_projection_schema_version\t1\nrow_count\t0' ]] ||
      die 'initial signed-promotion ledger is not canonical and empty'
    for relative in "${PROMOTION_PROJECTIONS[@]}"; do
      [[ "${relative}" == "${PROMOTION_LEDGER}" ]] && continue
      file_changed "${relative}" &&
        die 'trust changes must not include parity projection changes'
    done
  fi
  validate_candidate_trust_types
  validate_changed_paths
  [[ -f "${REVIEWERS_FILE}" && ! -L "${REVIEWERS_FILE}" ]] || die 'protected reviewer policy is missing'
  [[ -f "${REVIEWS_JSON}" && ! -L "${REVIEWS_JSON}" ]] || die 'runner review data is missing'
  jq -e 'type == "array"' "${REVIEWS_JSON}" >/dev/null || die 'runner review data is malformed'

  declare -A seen_reviewers=()
  reviewer_count=0
  while IFS= read -r reviewer || [[ -n "${reviewer}" ]]; do
    [[ "${reviewer}" =~ ^[A-Za-z0-9]([A-Za-z0-9-]{0,37}[A-Za-z0-9])?$ ]] || die 'malformed protected reviewer identity'
    reviewer="${reviewer,,}"
    [[ -z "${seen_reviewers[${reviewer}]:-}" ]] || die 'duplicate protected reviewer identity'
    seen_reviewers["${reviewer}"]=1
    ((reviewer_count += 1))
    review="$(jq -cer --arg reviewer "${reviewer}" '
      [ .[] | select(.user.login? and ((.user.login | ascii_downcase) == $reviewer)) ]
      | sort_by([.submitted_at, .id])
      | last
    ' "${REVIEWS_JSON}")" || die "designated reviewer has no review: ${reviewer}"
    state="$(jq -r '.state // empty' <<<"${review}")"
    [[ "${state}" == APPROVED ]] || die "designated reviewer latest state is not APPROVED: ${reviewer}"
    review_commit="$(jq -r '.commit_id // empty' <<<"${review}")"
    [[ "${review_commit}" == "${CANDIDATE_HEAD}" ]] ||
      die "designated reviewer approval does not bind candidate head: ${reviewer}"
  done <"${REVIEWERS_FILE}"
  ((reviewer_count > 0)) || die 'protected reviewer policy is empty'
  printf 'trust-change-approved\n'
elif [[ "${status_changed}" == true ]]; then
  printf 'promotion\n'
elif [[ "${projection_changed}" == true ]]; then
  die 'parity projections may change only with a signed status promotion'
else
  printf 'no-op\n'
fi
