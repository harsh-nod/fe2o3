#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

die() {
  printf 'protected parity change policy: %s\n' "$1" >&2
  exit 2
}

[[ "$#" == 9 ]] || die 'usage: PROTECTED_ROOT CANDIDATE_ROOT STATUS_PATH REVIEWS_JSON REVIEWERS_FILE REVISION_REPOSITORY BASE_SHA HEAD_SHA EVENT_KIND'

PROTECTED_ROOT="$(realpath -e -- "$1")" || die 'protected root does not resolve'
readonly PROTECTED_ROOT
CANDIDATE_ROOT="$(realpath -e -- "$2")" || die 'candidate root does not resolve'
readonly CANDIDATE_ROOT
readonly STATUS_PATH="$3"
readonly REVIEWS_JSON="$4"
readonly REVIEWERS_FILE="$5"
REVISION_REPOSITORY="$(realpath -e -- "$6")" || die 'revision repository does not resolve'
readonly REVISION_REPOSITORY
readonly BASE_SHA="$7"
readonly CANDIDATE_HEAD="$8"
readonly EVENT_KIND="$9"

[[ -d "${PROTECTED_ROOT}" && ! -L "$1" ]] || die 'protected root must be a real directory'
[[ -d "${CANDIDATE_ROOT}" && ! -L "$2" ]] || die 'candidate root must be a real directory'
[[ "${STATUS_PATH}" == docs/cuda-oxide-parity-status.tsv ]] || die 'unexpected status path'
[[ -d "${REVISION_REPOSITORY}" && ! -L "$6" ]] || die 'revision repository must be a real directory'
[[ "${BASE_SHA}" =~ ^[0-9a-f]{40}$ && ! "${BASE_SHA}" =~ ^0{40}$ ]] || die 'base revision is malformed or zero'
[[ "${CANDIDATE_HEAD}" =~ ^[0-9a-f]{40}$ && ! "${CANDIDATE_HEAD}" =~ ^0{40}$ ]] || die 'candidate head is malformed or zero'
case "${EVENT_KIND}" in
  pull-request|merge-group)
    ;;
  *)
    die 'event kind must be pull-request or merge-group'
    ;;
esac

validate_revision_inputs() {
  local actual
  local common
  local root
  local sha
  [[ "$(git -C "${REVISION_REPOSITORY}" rev-parse --is-bare-repository 2>/dev/null)" == true ]] ||
    die 'revision repository must be bare'
  for sha in "${BASE_SHA}" "${CANDIDATE_HEAD}"; do
    actual="$(git -C "${REVISION_REPOSITORY}" rev-parse --verify "${sha}^{commit}" 2>/dev/null)" ||
      die 'revision does not resolve to a commit'
    [[ "${actual}" == "${sha}" ]] || die 'revision commit identity mismatch'
    [[ "$(git -C "${REVISION_REPOSITORY}" cat-file -t "${sha}" 2>/dev/null)" == commit ]] ||
      die 'revision object is not a commit'
  done
  if [[ "${EVENT_KIND}" == merge-group ]] &&
    ! git -C "${REVISION_REPOSITORY}" merge-base --is-ancestor \
      "${BASE_SHA}" "${CANDIDATE_HEAD}"; then
    die 'merge-group head does not descend from its base'
  fi
  for root in "${PROTECTED_ROOT}:${BASE_SHA}" "${CANDIDATE_ROOT}:${CANDIDATE_HEAD}"; do
    sha="${root##*:}"
    root="${root%:*}"
    actual="$(git -C "${root}" rev-parse --verify 'HEAD^{commit}' 2>/dev/null)" ||
      die 'revision worktree has no commit HEAD'
    [[ "${actual}" == "${sha}" ]] || die 'revision worktree does not match declared commit'
    common="$(git -C "${root}" rev-parse --path-format=absolute --git-common-dir 2>/dev/null)" ||
      die 'revision worktree has no common repository'
    common="$(realpath -e -- "${common}")" || die 'revision worktree common repository does not resolve'
    [[ "${common}" == "${REVISION_REPOSITORY}" ]] ||
      die 'revision worktree is not owned by the revision repository'
    [[ -z "$(git -C "${root}" status --porcelain=v1 --untracked-files=all)" ]] ||
      die 'revision worktree is dirty'
  done
}

CHANGED_PATHS="$(mktemp)" || die 'cannot allocate immutable changed-path list'
readonly CHANGED_PATHS
trap 'rm -f -- "${CHANGED_PATHS}"' EXIT

derive_changed_paths() {
  git -C "${REVISION_REPOSITORY}" diff --no-ext-diff --no-renames \
    --name-only -z "${BASE_SHA}" "${CANDIDATE_HEAD}" -- \
    >"${CHANGED_PATHS}" || die 'cannot derive changed paths from immutable revisions'
}

validate_revision_inputs
derive_changed_paths

readonly -a TRUST_FILES=(
  docs/parity-signed-evidence-v2.md
  docs/parity-evidence/trust-policy-v2.example.tsv
  docs/parity-row-evidence-policy-v2.tsv
  docs/parity-evidence/trust-policy-v2.tsv
  scripts/parity-dashboard.sh
  scripts/parity-matrix.sh
  scripts/parity-promotion-projections.sh
  scripts/parity-check-reconcile.sh
  scripts/parity-protected-controller.sh
  scripts/parity-signed-evidence.py
  scripts/parity-protected-change-policy.sh
  scripts/tests/hosted-parity-ci.sh
  scripts/tests/parity-dashboard.sh
  scripts/tests/parity-promotion-projections.sh
  scripts/tests/parity-row-evidence.sh
  .github/workflows/ci.yml
  .github/workflows/parity-promotion.yml
  .github/workflows/parity-review-signal.yml
  .github/CODEOWNERS
  .github/parity-trust-reviewers.txt
)
readonly TRUST_KEY_DIRECTORY=docs/parity-evidence/trusted-keys
readonly EVIDENCE_ARCHIVE=docs/parity-evidence/archive
readonly PROMOTION_LEDGER=docs/generated/cuda-oxide-parity-signed-promotions.tsv
readonly -a PROMOTION_PROJECTIONS=(
  docs/cuda-oxide-parity-matrix.md
  docs/generated/cuda-oxide-parity-dashboard.md
  docs/generated/cuda-oxide-parity-dashboard.tsv
  "${PROMOTION_LEDGER}"
)
readonly -a ADMIN_MIGRATION_WORKFLOWS=(
  .github/workflows/ci.yml
  .github/workflows/parity-review-signal.yml
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
  actual_count=0
  while IFS= read -r -d '' path; do
    ((actual_count += 1))
    [[ -n "${path}" && "${path}" != /* && "${path}" != *$'\n'* &&
      "${path}" != */./* && "${path}" != ../* && "${path}" != */../* &&
      "${path}" != */.. && "${path}" != ./* && "${path}" != *//* ]] ||
      die 'immutable revision path is malformed'
    trusted=false
    case "${path}" in
      docs/parity-signed-evidence-v2.md|docs/parity-evidence/trust-policy-v2.example.tsv|docs/parity-row-evidence-policy-v2.tsv|docs/parity-evidence/trust-policy-v2.tsv|scripts/parity-dashboard.sh|scripts/parity-matrix.sh|scripts/parity-promotion-projections.sh|scripts/parity-check-reconcile.sh|scripts/parity-protected-controller.sh|scripts/parity-signed-evidence.py|scripts/parity-protected-change-policy.sh|scripts/tests/hosted-parity-ci.sh|scripts/tests/parity-dashboard.sh|scripts/tests/parity-promotion-projections.sh|scripts/tests/parity-row-evidence.sh|.github/workflows/ci.yml|.github/workflows/parity-promotion.yml|.github/workflows/parity-review-signal.yml|.github/CODEOWNERS|.github/parity-trust-reviewers.txt)
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
  done <"${CHANGED_PATHS}"
  ((actual_count > 0)) || die 'immutable revisions contain no changed paths'
}

validate_changed_path_ownership() {
  local codeowners="${PROTECTED_ROOT}/.github/CODEOWNERS"
  local owner="@$1"
  local path
  local pattern
  [[ -f "${codeowners}" && ! -L "${codeowners}" ]] ||
    die 'protected CODEOWNERS policy is missing'
  while IFS= read -r -d '' path; do
    case "${path}" in
      docs/parity-evidence/trusted-keys/*)
        pattern=/docs/parity-evidence/trusted-keys/
        ;;
      *)
        pattern="/${path}"
        ;;
    esac
    awk -v pattern="${pattern}" -v owner="${owner}" '
      $1 == pattern {
        for (field_index = 2; field_index <= NF; field_index++) {
          if (tolower($field_index) == tolower(owner)) found = 1
        }
      }
      END { exit found ? 0 : 1 }
    ' "${codeowners}" ||
      die "designated reviewer is not CODEOWNER for changed trust path: ${path}"
  done <"${CHANGED_PATHS}"
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

archive_changed=false
if directory_changed "${EVIDENCE_ARCHIVE}"; then
  archive_changed=true
fi

for relative in "${ADMIN_MIGRATION_WORKFLOWS[@]}"; do
  if file_changed "${relative}"; then
    die "notification workflow changes require an administrator migration: ${relative}"
  fi
done

if [[ "${trust_changed}" == true ]]; then
  [[ "${status_changed}" == false ]] || die 'trust changes must not include parity status changes'
  [[ "${archive_changed}" == false ]] ||
    die 'trust changes must not include parity evidence archive changes'
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
  if [[ "${EVENT_KIND}" == merge-group ]]; then
    printf 'trust-change-merge-group\n'
    exit 0
  fi
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
    validate_changed_path_ownership "${reviewer}"
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
elif [[ "${archive_changed}" == true ]]; then
  die 'parity evidence archive may change only with a signed status promotion'
elif [[ "${projection_changed}" == true ]]; then
  die 'parity projections may change only with a signed status promotion'
else
  printf 'no-op\n'
fi
