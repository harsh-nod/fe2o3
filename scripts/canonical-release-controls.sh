#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly CONTROL_TOOL="${SCRIPT_DIR}/canonical-release-controls.py"
readonly CANONICAL_REPO=harsh-nod/fe2o3
readonly RELEASE_USER=harsh-nod
readonly RELEASE_USER_ID=3144552
readonly REVIEWER_USER=powderluv
readonly REVIEWER_USER_ID=74956
readonly RELEASE_ENVIRONMENT=release
readonly RELEASE_BRANCH_POLICY=main
readonly TAG_CREATION_NAME=fe2o3-release-tag-creation
readonly TAG_IMMUTABILITY_NAME=fe2o3-release-tag-immutability

die() {
  printf 'canonical release controls: %s\n' "$1" >&2
  exit 2
}

usage() {
  cat <<'EOF'
Usage:
  scripts/canonical-release-controls.sh render \
    --control tag-creation|tag-guard|tag-immutability|environment|environment-policy \
    --release-user-id 3144552 --reviewer-user-id 74956
  scripts/canonical-release-controls.sh bootstrap \
    --repo harsh-nod/fe2o3 \
    --release-user-id 3144552 --reviewer-user-id 74956
  scripts/canonical-release-controls.sh verify \
    --repo harsh-nod/fe2o3 \
    --release-user-id 3144552 --reviewer-user-id 74956

bootstrap requires an authenticated gh token with repository Administration
write permission and no existing v* tags. It creates missing controls, resumes
only its exact fail-closed staging state, and never replaces a control.
EOF
}

[[ "$#" -ge 1 ]] || {
  usage >&2
  exit 2
}
readonly COMMAND="$1"
shift

REPO=""
CONTROL=""
ARG_RELEASE_USER_ID=""
ARG_REVIEWER_USER_ID=""
while (($#)); do
  case "$1" in
    --repo)
      [[ "$#" -ge 2 ]] || die 'missing --repo value'
      REPO="$2"
      shift 2
      ;;
    --control)
      [[ "$#" -ge 2 ]] || die 'missing --control value'
      CONTROL="$2"
      shift 2
      ;;
    --release-user-id)
      [[ "$#" -ge 2 ]] || die 'missing --release-user-id value'
      ARG_RELEASE_USER_ID="$2"
      shift 2
      ;;
    --reviewer-user-id)
      [[ "$#" -ge 2 ]] || die 'missing --reviewer-user-id value'
      ARG_REVIEWER_USER_ID="$2"
      shift 2
      ;;
    *) die "unknown option: $1" ;;
  esac
done

[[ "${ARG_RELEASE_USER_ID}" =~ ^[1-9][0-9]*$ ]] ||
  die 'invalid release user ID'
[[ "${ARG_REVIEWER_USER_ID}" =~ ^[1-9][0-9]*$ ]] ||
  die 'invalid reviewer user ID'
[[ "${ARG_RELEASE_USER_ID}" == "${RELEASE_USER_ID}" ]] ||
  die 'release user ID does not identify harsh-nod on github.com'
[[ "${ARG_REVIEWER_USER_ID}" == "${REVIEWER_USER_ID}" ]] ||
  die 'reviewer user ID does not identify powderluv on github.com'

common=(
  --release-user-id "${ARG_RELEASE_USER_ID}"
  --reviewer-user-id "${ARG_REVIEWER_USER_ID}"
)

case "${COMMAND}" in
  render)
    [[ -z "${REPO}" ]] || die '--repo is not valid for render'
    case "${CONTROL}" in
      tag-creation | tag-guard | tag-immutability | environment | environment-policy) ;;
      *) die 'invalid --control value' ;;
    esac
    exec python3 -I "${CONTROL_TOOL}" render \
      --control "${CONTROL}" "${common[@]}"
    ;;
  bootstrap | verify)
    [[ -z "${CONTROL}" ]] || die '--control is valid only for render'
    [[ "${REPO}" == "${CANONICAL_REPO}" ]] ||
      die "--repo must be ${CANONICAL_REPO}"
    command -v gh >/dev/null || die 'gh is required'
    command -v jq >/dev/null || die 'jq is required'
    ;;
  *) die "unknown command: ${COMMAND}" ;;
esac

work_dir="$(mktemp -d)"
readonly work_dir
trap 'rm -rf "${work_dir}"' EXIT

api() {
  gh api --hostname github.com \
    -H 'Accept: application/vnd.github+json' \
    -H 'X-GitHub-Api-Version: 2026-03-10' "$@"
}

actual_release_user_id="$(api "users/${RELEASE_USER}" --jq .id)" ||
  die 'cannot resolve release user identity'
[[ "${actual_release_user_id}" == "${RELEASE_USER_ID}" ]] ||
  die 'GitHub release user identity does not match pinned ID'
actual_reviewer_user_id="$(api "users/${REVIEWER_USER}" --jq .id)" ||
  die 'cannot resolve release reviewer identity'
[[ "${actual_reviewer_user_id}" == "${REVIEWER_USER_ID}" ]] ||
  die 'GitHub release reviewer identity does not match pinned ID'
reviewer_can_read="$({
  api "repos/${REPO}/collaborators/${REVIEWER_USER}/permission" \
    --jq '.user.permissions.pull'
} 2>/dev/null)" || die 'cannot verify release reviewer repository access'
[[ "${reviewer_can_read}" == true ]] ||
  die 'release reviewer must have repository read access'

render_control() {
  local control="$1"
  local output="$2"
  python3 -I "${CONTROL_TOOL}" render --control "${control}" \
    "${common[@]}" >"${output}"
  python3 -I "${CONTROL_TOOL}" verify --control "${control}" \
    "${common[@]}" "${output}" >/dev/null
}

verify_control() {
  local control="$1"
  local document="$2"
  python3 -I "${CONTROL_TOOL}" verify --control "${control}" \
    "${common[@]}" "${document}" >/dev/null
}

list_ruleset_ids() {
  local name="$1"
  local output="$2"
  if ! api --paginate \
      "repos/${REPO}/rulesets?includes_parents=false&targets=tag&per_page=100" \
      --jq ".[] | select(.name == \"${name}\") | .id" >"${output}"; then
    die 'cannot list canonical tag rulesets'
  fi
  local id
  while IFS= read -r id; do
    [[ -z "${id}" || "${id}" =~ ^[1-9][0-9]*$ ]] ||
      die 'GitHub returned an invalid tag ruleset ID'
  done <"${output}"
}

ensure_ruleset() {
  local control="$1"
  local name="$2"
  local create_missing="$3"
  local ids_file="${work_dir}/${control}.ids"
  local payload="${work_dir}/${control}.payload.json"
  local response="${work_dir}/${control}.response.json"
  local -a ids=()

  list_ruleset_ids "${name}" "${ids_file}"
  mapfile -t ids <"${ids_file}"
  ((${#ids[@]} <= 1)) || die "duplicate canonical ruleset: ${name}"
  if ((${#ids[@]} == 0)); then
    [[ "${create_missing}" == true ]] ||
      die "missing canonical ruleset: ${name}"
    render_control "${control}" "${payload}"
    if ! api --method POST "repos/${REPO}/rulesets" \
        --input "${payload}" >"${response}"; then
      die "cannot create canonical ruleset: ${name}"
    fi
  else
    if ! api "repos/${REPO}/rulesets/${ids[0]}?includes_parents=false" \
        >"${response}"; then
      die "cannot read canonical ruleset: ${name}"
    fi
  fi
  verify_control "${control}" "${response}"
}

IMMUTABILITY_ID=""
IMMUTABILITY_STATE=""
ensure_immutability_guard() {
  local ids_file="${work_dir}/tag-immutability.ids"
  local payload="${work_dir}/tag-guard.payload.json"
  local response="${work_dir}/tag-guard.response.json"
  local -a ids=()

  list_ruleset_ids "${TAG_IMMUTABILITY_NAME}" "${ids_file}"
  mapfile -t ids <"${ids_file}"
  ((${#ids[@]} <= 1)) ||
    die "duplicate canonical ruleset: ${TAG_IMMUTABILITY_NAME}"
  if ((${#ids[@]} == 0)); then
    render_control tag-guard "${payload}"
    if ! api --method POST "repos/${REPO}/rulesets" \
        --input "${payload}" >"${response}"; then
      die 'cannot create fail-closed release tag bootstrap guard'
    fi
    IMMUTABILITY_ID="$(jq -er '.id' "${response}")" ||
      die 'created tag bootstrap guard has no ruleset ID'
    [[ "${IMMUTABILITY_ID}" =~ ^[1-9][0-9]*$ ]] ||
      die 'created tag bootstrap guard has an invalid ruleset ID'
    verify_control tag-guard "${response}"
    IMMUTABILITY_STATE=guard
    return
  fi

  IMMUTABILITY_ID="${ids[0]}"
  if ! api "repos/${REPO}/rulesets/${IMMUTABILITY_ID}?includes_parents=false" \
      >"${response}"; then
    die 'cannot read canonical release tag immutability ruleset'
  fi
  if verify_control tag-guard "${response}" 2>/dev/null; then
    IMMUTABILITY_STATE=guard
  elif verify_control tag-immutability "${response}" 2>/dev/null; then
    IMMUTABILITY_STATE=final
  else
    verify_control tag-immutability "${response}"
    die 'unreachable invalid tag immutability ruleset'
  fi
}

finalize_immutability_guard() {
  [[ "${IMMUTABILITY_STATE}" == guard ]] || return 0
  local payload="${work_dir}/tag-immutability.payload.json"
  local response="${work_dir}/tag-immutability.final.json"
  render_control tag-immutability "${payload}"
  if ! api --method PUT "repos/${REPO}/rulesets/${IMMUTABILITY_ID}" \
      --input "${payload}" >"${response}"; then
    die 'cannot finalize canonical release tag immutability ruleset'
  fi
  verify_control tag-immutability "${response}"
  IMMUTABILITY_STATE=final
}

list_environment_names() {
  local output="$1"
  if ! api --paginate "repos/${REPO}/environments?per_page=100" \
      --jq ".environments[] | select(.name == \"${RELEASE_ENVIRONMENT}\") | .name" \
      >"${output}"; then
    die 'cannot list canonical release environments'
  fi
}

ensure_environment() {
  local create_missing="$1"
  local names_file="${work_dir}/environment.names"
  local payload="${work_dir}/environment.payload.json"
  local response="${work_dir}/environment.response.json"
  local -a names=()

  list_environment_names "${names_file}"
  mapfile -t names <"${names_file}"
  ((${#names[@]} <= 1)) || die 'duplicate canonical release environment'
  if ((${#names[@]} == 0)); then
    [[ "${create_missing}" == true ]] ||
      die 'missing canonical release environment'
    render_control environment "${payload}"
    if ! api --method PUT "repos/${REPO}/environments/${RELEASE_ENVIRONMENT}" \
        --input "${payload}" >"${response}"; then
      die 'cannot create canonical release environment'
    fi
  else
    if ! api "repos/${REPO}/environments/${RELEASE_ENVIRONMENT}" \
        >"${response}"; then
      die 'cannot read canonical release environment'
    fi
  fi
  verify_control environment "${response}"
}

ensure_environment_policy() {
  local create_missing="$1"
  local policies_file="${work_dir}/environment-policies"
  local payload="${work_dir}/environment-policy.payload.json"
  local response="${work_dir}/environment-policy.response.json"
  local -a policies=()
  local policy_id policy_name

  if ! api --paginate \
      "repos/${REPO}/environments/${RELEASE_ENVIRONMENT}/deployment-branch-policies?per_page=100" \
      --jq '.branch_policies[] | [.id, .name] | @tsv' >"${policies_file}"; then
    die 'cannot list release environment deployment policies'
  fi
  mapfile -t policies <"${policies_file}"
  ((${#policies[@]} <= 1)) ||
    die 'release environment must contain only the exact main policy'
  if ((${#policies[@]} == 0)); then
    [[ "${create_missing}" == true ]] ||
      die 'missing release environment main deployment policy'
    render_control environment-policy "${payload}"
    if ! api --method POST \
        "repos/${REPO}/environments/${RELEASE_ENVIRONMENT}/deployment-branch-policies" \
        --input "${payload}" >"${response}"; then
      die 'cannot create release environment main deployment policy'
    fi
  else
    IFS=$'\t' read -r policy_id policy_name <<<"${policies[0]}"
    [[ "${policy_id}" =~ ^[1-9][0-9]*$ ]] ||
      die 'GitHub returned an invalid environment deployment policy ID'
    [[ "${policy_name}" == "${RELEASE_BRANCH_POLICY}" ]] ||
      die 'release environment must contain only the exact main policy'
    if ! api \
        "repos/${REPO}/environments/${RELEASE_ENVIRONMENT}/deployment-branch-policies/${policy_id}" \
        >"${response}"; then
      die 'cannot read release environment main deployment policy'
    fi
  fi
  verify_control environment-policy "${response}"
}

require_no_release_tags() {
  local release_refs="${work_dir}/release-refs"
  if ! api --paginate "repos/${REPO}/git/matching-refs/tags/v?per_page=100" \
      --jq '.[].ref' >"${release_refs}"; then
    die 'cannot inspect existing release tags'
  fi
  [[ ! -s "${release_refs}" ]] ||
    die 'bootstrap requires a repository with no existing v* tags'
}

if [[ "${COMMAND}" == bootstrap ]]; then
  require_no_release_tags

  # Lock every matching operation first. Any interruption leaves creation,
  # update, and deletion denied until an exact bootstrap resume completes.
  ensure_immutability_guard
  # The guard makes this second observation stable and detects a tag created
  # between the initial observation and guard activation.
  require_no_release_tags
  if [[ "${IMMUTABILITY_STATE}" == guard ]]; then
    ensure_environment true
    ensure_environment_policy true
    ensure_ruleset tag-creation "${TAG_CREATION_NAME}" true
    finalize_immutability_guard
  else
    # A final immutability ruleset cannot be a partial state produced by this
    # bootstrap. Accept it only when every other final control already exists.
    ensure_environment false
    ensure_environment_policy false
    ensure_ruleset tag-creation "${TAG_CREATION_NAME}" false
  fi
else
  ensure_environment false
  ensure_environment_policy false
  ensure_ruleset tag-immutability "${TAG_IMMUTABILITY_NAME}" false
  ensure_ruleset tag-creation "${TAG_CREATION_NAME}" false
fi

printf 'canonical release API controls are enforceable\n'
