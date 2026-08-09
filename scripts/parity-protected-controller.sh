#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

die() {
  printf 'protected parity controller: %s\n' "$1" >&2
  exit 2
}

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
PROTECTED_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly PROTECTED_ROOT
readonly CHECK_NAME=fe2o3/protected-parity-promotion

valid_sha() {
  [[ "$1" =~ ^[0-9a-f]{40}$ && ! "$1" =~ ^0{40}$ ]]
}

workflow_blob_oid() {
  local repository="$1"
  local revision="$2"
  local path="$3"
  local oid
  valid_sha "${revision}" || die 'workflow blob revision is malformed'
  [[ "${path}" == .github/workflows/*.yml && "${path}" != *$'\n'* ]] ||
    die 'workflow blob path is malformed'
  oid="$(git -C "${repository}" rev-parse --verify "${revision}:${path}" 2>/dev/null)" ||
    die "workflow blob is missing: ${path}"
  [[ "$(git -C "${repository}" cat-file -t "${oid}" 2>/dev/null)" == blob ]] ||
    die "workflow path is not a blob: ${path}"
  printf '%s\n' "${oid}"
}

require_workflow_blob_match() {
  local repository="$1"
  local protected_revision="$2"
  local source_revision="$3"
  local path="$4"
  local protected_oid
  local source_oid
  protected_oid="$(workflow_blob_oid "${repository}" "${protected_revision}" "${path}")"
  source_oid="$(workflow_blob_oid "${repository}" "${source_revision}" "${path}")"
  [[ "${protected_oid}" == "${source_oid}" ]] ||
    die "workflow blob differs from protected revision: ${path}"
}

verify_source_metadata() {
  local snapshot="$1"
  local run_id="$2"
  local workflow_id="$3"
  local path="$4"
  local event="$5"
  local head_branch="$6"
  local head_sha="$7"
  local pr_number="${8:-0}"
  [[ -f "${snapshot}" && ! -L "${snapshot}" ]] ||
    die 'source workflow snapshot must be a regular file'
  jq -e \
    --argjson id "${run_id}" \
    --argjson workflow_id "${workflow_id}" \
    --arg path "${path}" \
    --arg event "${event}" \
    --arg branch "${head_branch}" \
    --arg head "${head_sha}" \
    --argjson pr_number "${pr_number}" '
      .id == $id and .workflow_id == $workflow_id and
      .path == $path and .event == $event and
      .head_branch == $branch and .head_sha == $head and
      .status == "completed" and
      ($pr_number == 0 or
        ([.pull_requests[]?.number] | length == 1 and .[0] == $pr_number))
    ' "${snapshot}" >/dev/null || die 'source workflow metadata mismatch'
}

emit_reconciliation_targets() {
  local pulls="$1"
  local queue_refs="$2"
  local sha
  local ref
  [[ -f "${pulls}" && ! -L "${pulls}" ]] ||
    die 'open-PR inventory must be a regular file'
  [[ -f "${queue_refs}" && ! -L "${queue_refs}" ]] ||
    die 'merge-queue inventory must be a regular file'
  jq -e 'type == "array"' "${pulls}" >/dev/null ||
    die 'open-PR inventory is malformed'
  jq -e '
    all(.[];
      (.number | type) == "number" and .number > 0 and
      (.head.ref | type) == "string" and (.head.ref | length) > 0 and
      (.head.sha | type) == "string" and
      (.head.sha | test("^[0-9a-f]{40}$")) and
      .head.sha != "0000000000000000000000000000000000000000"
    )
  ' "${pulls}" >/dev/null || die 'open-PR inventory contains a malformed target'
  jq -r '
    .[] |
    ["pull-request",.number,("refs/heads/" + .head.ref),.head.sha] | @tsv
  ' "${pulls}"
  while IFS=$'\t' read -r sha ref; do
    [[ -n "${sha}${ref}" ]] || continue
    valid_sha "${sha}" || die 'merge-queue inventory SHA is malformed'
    git check-ref-format "${ref}" >/dev/null ||
      die 'merge-queue inventory ref is malformed'
    printf 'merge-group\t0\t%s\t%s\n' "${ref}" "${sha}"
  done <"${queue_refs}"
}

case "${1:-}" in
  test-workflow-blob)
    [[ "$#" == 5 ]] ||
      die 'usage: test-workflow-blob REPOSITORY PROTECTED_SHA SOURCE_SHA PATH'
    require_workflow_blob_match "$2" "$3" "$4" "$5"
    exit 0
    ;;
  test-source-metadata)
    [[ "$#" == 9 ]] ||
      die 'usage: test-source-metadata JSON RUN_ID WORKFLOW_ID PATH EVENT REF SHA PR'
    verify_source_metadata "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9"
    exit 0
    ;;
  test-reconciliation-targets)
    [[ "$#" == 3 ]] ||
      die 'usage: test-reconciliation-targets PULLS_JSON QUEUE_REFS_TSV'
    emit_reconciliation_targets "$2" "$3"
    exit 0
    ;;
esac

require_runtime() {
  : "${GITHUB_REPOSITORY:?}"
  : "${GITHUB_WORKSPACE:?}"
  : "${RUNNER_TEMP:?}"
  : "${READ_TOKEN:?}"
  : "${CHECK_TOKEN:?}"
  : "${PROTECTED_SHA:?}"
  : "${DEFAULT_BRANCH:?}"
  : "${EXPECTED_CI_WORKFLOW_ID:?}"
  : "${EXPECTED_REVIEW_WORKFLOW_ID:?}"
  : "${EXPECTED_APP_ID:?}"
  : "${EXPECTED_APP_SLUG:?}"
  [[ "${GITHUB_REPOSITORY}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] ||
    die 'repository identity is malformed'
  valid_sha "${PROTECTED_SHA}" || die 'protected workflow SHA is malformed'
  git check-ref-format "refs/heads/${DEFAULT_BRANCH}" >/dev/null ||
    die 'default branch is malformed'
  [[ "${EXPECTED_CI_WORKFLOW_ID}" =~ ^[1-9][0-9]*$ ]] ||
    die 'CI workflow ID is malformed'
  [[ "${EXPECTED_REVIEW_WORKFLOW_ID}" =~ ^[1-9][0-9]*$ ]] ||
    die 'review-signal workflow ID is malformed'
  [[ "${EXPECTED_APP_ID}" =~ ^[1-9][0-9]*$ ]] || die 'App ID is malformed'
  [[ "${EXPECTED_APP_SLUG}" =~ ^[A-Za-z0-9][A-Za-z0-9-]*$ ]] ||
    die 'App slug is malformed'
  [[ "$(git -C "${PROTECTED_ROOT}" rev-parse --verify 'HEAD^{commit}')" == \
    "${PROTECTED_SHA}" ]] || die 'controller checkout is not the protected workflow revision'
}

read_api() {
  GH_TOKEN="${READ_TOKEN}" gh api "$@"
}

check_api() {
  GH_TOKEN="${CHECK_TOKEN}" gh api "$@"
}

auth_header() {
  printf 'x-access-token:%s' "${READ_TOKEN}" | base64 | tr -d '\n'
}

fetch_ref() {
  local repository="$1"
  local remote_repository="$2"
  local source_ref="$3"
  local destination_ref="$4"
  local header
  [[ "${remote_repository}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] ||
    die 'fetch repository identity is malformed'
  git check-ref-format "${source_ref}" >/dev/null || die 'fetch source ref is malformed'
  git check-ref-format "${destination_ref}" >/dev/null ||
    die 'fetch destination ref is malformed'
  header="$(auth_header)"
  git -C "${repository}" \
    -c "http.https://github.com/.extraheader=AUTHORIZATION: basic ${header}" \
    fetch --no-tags --force \
    "https://github.com/${remote_repository}.git" \
    "+${source_ref}:${destination_ref}"
}

load_named_checks() {
  local head_sha="$1"
  local pages="$2"
  local combined="$3"
  check_api --paginate --method GET \
    -H 'Accept: application/vnd.github+json' \
    "repos/${GITHUB_REPOSITORY}/commits/${head_sha}/check-runs" \
    -f check_name="${CHECK_NAME}" -f filter=all -f per_page=100 >"${pages}"
  jq -s '
    {
      total_count: ([.[].total_count] | max // 0),
      check_runs: ([.[].check_runs[]?])
    }
  ' "${pages}" >"${combined}"
}

upsert_pending_check() {
  local head_sha="$1"
  local external_id="fe2o3-parity-v1:${head_sha}"
  local scratch
  local pages
  local combined
  local decision
  local operation
  local check_id
  local payload
  local request_payload
  local response
  scratch="$(mktemp -d "${RUNNER_TEMP}/parity-check.XXXXXX")"
  pages="${scratch}/pages.json"
  combined="${scratch}/checks.json"
  payload="${scratch}/pending.json"
  request_payload="${scratch}/pending-request.json"
  response="${scratch}/response.json"
  load_named_checks "${head_sha}" "${pages}" "${combined}"
  decision="$(bash "${PROTECTED_ROOT}/scripts/parity-check-reconcile.sh" \
    "${combined}" "${EXPECTED_APP_ID}" "${EXPECTED_APP_SLUG}" \
    "${CHECK_NAME}" "${head_sha}" "${external_id}" select)"
  IFS=$'\t' read -r operation check_id <<<"${decision}"
  jq -n \
    --arg name "${CHECK_NAME}" \
    --arg head_sha "${head_sha}" \
    --arg external_id "${external_id}" \
    --arg started_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" '
      {
        name:$name,
        head_sha:$head_sha,
        status:"in_progress",
        conclusion:null,
        external_id:$external_id,
        started_at:$started_at,
        output:{
          title:"Protected parity verification running",
          summary:"Protected reconciliation is validating this exact revision."
        }
      }
    ' >"${payload}"
  case "${operation}" in
    create)
      cp -- "${payload}" "${request_payload}"
      check_api --method POST -H 'Accept: application/vnd.github+json' \
        "repos/${GITHUB_REPOSITORY}/check-runs" \
        --input "${request_payload}" >"${response}"
      ;;
    update)
      [[ "${check_id}" =~ ^[1-9][0-9]*$ ]] || die 'selected check ID is malformed'
      jq 'del(.head_sha)' "${payload}" >"${request_payload}"
      check_api --method PATCH -H 'Accept: application/vnd.github+json' \
        "repos/${GITHUB_REPOSITORY}/check-runs/${check_id}" \
        --input "${request_payload}" >"${response}"
      ;;
    *)
      die 'unknown check reconciliation operation'
      ;;
  esac
  jq -e \
    --arg name "${CHECK_NAME}" \
    --arg head "${head_sha}" \
    --arg external_id "${external_id}" \
    --arg app_id "${EXPECTED_APP_ID}" \
    --arg app_slug "${EXPECTED_APP_SLUG}" '
      (.id | type) == "number" and .id > 0 and
      .name == $name and .head_sha == $head and
      .external_id == $external_id and .status == "in_progress" and
      ((.app.id | tostring) == $app_id) and .app.slug == $app_slug
    ' "${response}" >/dev/null || die 'pending check response identity mismatch'
  jq -r '.id' "${response}"
  rm -rf -- "${scratch}"
}

complete_check() {
  local check_id="$1"
  local head_sha="$2"
  local conclusion="$3"
  local title
  local summary
  local payload
  local response
  [[ "${check_id}" =~ ^[1-9][0-9]*$ ]] || die 'completion check ID is malformed'
  case "${conclusion}" in
    success)
      title='Protected parity verification passed'
      summary='The protected exact-tree verifier accepted this revision.'
      ;;
    failure)
      title='Protected parity verification failed'
      summary='The protected exact-tree verifier rejected this revision.'
      ;;
    *)
      die 'unsupported check conclusion'
      ;;
  esac
  payload="$(mktemp "${RUNNER_TEMP}/parity-complete.XXXXXX.json")"
  response="$(mktemp "${RUNNER_TEMP}/parity-complete-response.XXXXXX.json")"
  jq -n \
    --arg name "${CHECK_NAME}" \
    --arg external_id "fe2o3-parity-v1:${head_sha}" \
    --arg conclusion "${conclusion}" \
    --arg completed_at "$(date -u +%Y-%m-%dT%H:%M:%SZ)" \
    --arg title "${title}" \
    --arg summary "${summary}" '
      {
        name:$name,
        external_id:$external_id,
        status:"completed",
        conclusion:$conclusion,
        completed_at:$completed_at,
        output:{title:$title,summary:$summary}
      }
    ' >"${payload}"
  check_api --method PATCH -H 'Accept: application/vnd.github+json' \
    "repos/${GITHUB_REPOSITORY}/check-runs/${check_id}" \
    --input "${payload}" >"${response}"
  jq -e \
    --argjson id "${check_id}" \
    --arg name "${CHECK_NAME}" \
    --arg head "${head_sha}" \
    --arg external_id "fe2o3-parity-v1:${head_sha}" \
    --arg conclusion "${conclusion}" \
    --arg app_id "${EXPECTED_APP_ID}" \
    --arg app_slug "${EXPECTED_APP_SLUG}" '
      .id == $id and .name == $name and .head_sha == $head and
      .external_id == $external_id and .status == "completed" and
      .conclusion == $conclusion and
      ((.app.id | tostring) == $app_id) and .app.slug == $app_slug
    ' "${response}" >/dev/null || die 'completed check response identity mismatch'
  rm -f -- "${payload}" "${response}"
}

fetch_reviews() {
  local pr_number="$1"
  local output="$2"
  read_api --paginate \
    "repos/${GITHUB_REPOSITORY}/pulls/${pr_number}/reviews?per_page=100" |
    jq -s 'add' >"${output}"
}

verify_pr_snapshot() {
  local snapshot="$1"
  local pr_number="$2"
  local base_sha="$3"
  local head_sha="$4"
  jq -e \
    --argjson number "${pr_number}" \
    --arg base_repo "${GITHUB_REPOSITORY}" \
    --arg base_ref "${DEFAULT_BRANCH}" \
    --arg base_sha "${base_sha}" \
    --arg head_sha "${head_sha}" '
      .number == $number and .state == "open" and .draft == false and
      .base.repo.full_name == $base_repo and .base.ref == $base_ref and
      .base.sha == $base_sha and .head.sha == $head_sha and
      (.head.repo.full_name | type) == "string" and
      (.head.ref | type) == "string"
    ' "${snapshot}" >/dev/null || die 'pull request snapshot mismatch'
}

canonicalize_reviews() {
  jq -cS '
    map({id,submitted_at,state,commit_id,user_login:(.user.login // null)}) |
    sort_by(.id)
  ' "$1" >"$2"
}

verify_source_provenance() {
  local revision_repository="$1"
  local event_kind="$2"
  local pr_number="$3"
  local target_head_ref="$4"
  local target_head_sha="$5"
  local source_run_id="$6"
  local source_workflow_id="$7"
  local source_path="$8"
  local source_event="$9"
  local source_head_branch="${10}"
  local source_head_sha="${11}"
  local snapshot
  local source_ref
  ((source_run_id > 0)) || return 0
  [[ "${source_run_id}" =~ ^[1-9][0-9]*$ ]] || die 'source run ID is malformed'
  [[ "${source_workflow_id}" =~ ^[1-9][0-9]*$ ]] ||
    die 'source workflow ID is malformed'
  valid_sha "${source_head_sha}" || die 'source workflow SHA is malformed'
  git check-ref-format "refs/heads/${source_head_branch}" >/dev/null ||
    die 'source workflow ref is malformed'
  snapshot="$(mktemp "${RUNNER_TEMP}/parity-source.XXXXXX.json")"
  read_api "repos/${GITHUB_REPOSITORY}/actions/runs/${source_run_id}" >"${snapshot}"
  verify_source_metadata "${snapshot}" "${source_run_id}" \
    "${source_workflow_id}" "${source_path}" "${source_event}" \
    "${source_head_branch}" "${source_head_sha}" "${pr_number}"
  case "${source_event}" in
    merge_group)
      [[ "${event_kind}" == merge-group && "${source_path}" == .github/workflows/ci.yml ]]
      [[ "${source_workflow_id}" == "${EXPECTED_CI_WORKFLOW_ID}" ]]
      [[ "${source_head_branch}" == "${target_head_ref#refs/heads/}" ]]
      [[ "${source_head_sha}" == "${target_head_sha}" ]]
      ;;
    pull_request_review)
      [[ "${event_kind}" == pull-request &&
        "${source_path}" == .github/workflows/parity-review-signal.yml ]]
      [[ "${source_workflow_id}" == "${EXPECTED_REVIEW_WORKFLOW_ID}" ]]
      [[ "${source_head_branch}" == "${target_head_ref#refs/heads/}" ]]
      if [[ "${source_head_sha}" != "${target_head_sha}" ]]; then
        source_ref="refs/pull/${pr_number}/merge"
        fetch_ref "${revision_repository}" "${GITHUB_REPOSITORY}" \
          "${source_ref}" refs/parity/source
        [[ "$(git -C "${revision_repository}" rev-parse --verify \
          'refs/parity/source^{commit}')" == "${source_head_sha}" ]]
      fi
      ;;
    *)
      die 'unsupported source notification event'
      ;;
  esac
  [[ "$(git -C "${revision_repository}" cat-file -t "${source_head_sha}")" == commit ]]
  require_workflow_blob_match "${revision_repository}" "${PROTECTED_SHA}" \
    "${source_head_sha}" "${source_path}"
  rm -f -- "${snapshot}"
}

verify_target() {
  local event_kind="$1"
  local pr_number="$2"
  local target_head_ref="$3"
  local target_head_sha="$4"
  local source_run_id="$5"
  local source_workflow_id="$6"
  local source_path="$7"
  local source_event="$8"
  local source_head_branch="$9"
  local source_head_sha="${10}"
  local scratch
  local revision_repository
  local protected_worktree
  local candidate_worktree
  local head_repository
  local head_ref
  local pr_a
  local pr_b
  local reviews
  local reviews_a
  local reviews_b
  local review_snapshot_a
  local review_snapshot_b
  local mode
  local final_mode
  scratch="$(mktemp -d "${RUNNER_TEMP}/parity-target.XXXXXX")"
  trap 'rm -rf -- "${scratch}"' EXIT
  revision_repository="${scratch}/revisions.git"
  protected_worktree="${scratch}/protected"
  candidate_worktree="${scratch}/candidate"
  git init --bare "${revision_repository}" >/dev/null
  fetch_ref "${revision_repository}" "${GITHUB_REPOSITORY}" \
    "refs/heads/${DEFAULT_BRANCH}" refs/parity/base
  [[ "$(git -C "${revision_repository}" rev-parse --verify \
    'refs/parity/base^{commit}')" == "${PROTECTED_SHA}" ]] ||
    die 'protected default branch moved away from controller revision'

  case "${event_kind}" in
    pull-request)
      [[ "${pr_number}" =~ ^[1-9][0-9]*$ ]] || die 'PR number is malformed'
      pr_a="${scratch}/pr-a.json"
      read_api "repos/${GITHUB_REPOSITORY}/pulls/${pr_number}" >"${pr_a}"
      verify_pr_snapshot "${pr_a}" "${pr_number}" "${PROTECTED_SHA}" \
        "${target_head_sha}"
      head_repository="$(jq -er '.head.repo.full_name' "${pr_a}")"
      head_ref="refs/heads/$(jq -er '.head.ref' "${pr_a}")"
      fetch_ref "${revision_repository}" "${head_repository}" "${head_ref}" \
        refs/parity/head
      ;;
    merge-group)
      [[ "${pr_number}" == 0 ]] || die 'merge-group target has a PR number'
      head_repository="${GITHUB_REPOSITORY}"
      head_ref="${target_head_ref}"
      [[ "${head_ref}" == "refs/heads/gh-readonly-queue/${DEFAULT_BRANCH}/"* ]] ||
        die 'merge-group ref is outside the protected queue namespace'
      fetch_ref "${revision_repository}" "${head_repository}" "${head_ref}" \
        refs/parity/head
      ;;
    *)
      die 'unsupported parity target kind'
      ;;
  esac
  [[ "$(git -C "${revision_repository}" rev-parse --verify \
    'refs/parity/head^{commit}')" == "${target_head_sha}" ]] ||
    die 'target ref does not resolve to the declared SHA'
  [[ "$(git -C "${revision_repository}" cat-file -t "${PROTECTED_SHA}")" == commit ]]
  [[ "$(git -C "${revision_repository}" cat-file -t "${target_head_sha}")" == commit ]]
  if [[ "${event_kind}" == merge-group ]]; then
    git -C "${revision_repository}" merge-base --is-ancestor \
      "${PROTECTED_SHA}" "${target_head_sha}" ||
      die 'merge-group head does not descend from protected default'
  fi

  # Notification workflow changes require an administrator migration. They
  # never authorize the candidate that carries them.
  require_workflow_blob_match "${revision_repository}" "${PROTECTED_SHA}" \
    "${target_head_sha}" .github/workflows/ci.yml
  require_workflow_blob_match "${revision_repository}" "${PROTECTED_SHA}" \
    "${target_head_sha}" .github/workflows/parity-review-signal.yml
  verify_source_provenance "${revision_repository}" "${event_kind}" \
    "${pr_number}" "${head_ref}" "${target_head_sha}" "${source_run_id}" \
    "${source_workflow_id}" "${source_path}" "${source_event}" \
    "${source_head_branch}" "${source_head_sha}"

  git -C "${revision_repository}" worktree add --detach \
    "${protected_worktree}" "${PROTECTED_SHA}" >/dev/null
  git -C "${revision_repository}" worktree add --detach \
    "${candidate_worktree}" "${target_head_sha}" >/dev/null
  reviews="${scratch}/reviews.json"
  if [[ "${event_kind}" == pull-request ]]; then
    fetch_reviews "${pr_number}" "${reviews}"
  else
    printf '[]\n' >"${reviews}"
  fi
  mode="$(bash "${protected_worktree}/scripts/parity-protected-change-policy.sh" \
    "${protected_worktree}" "${candidate_worktree}" \
    docs/cuda-oxide-parity-status.tsv "${reviews}" \
    "${protected_worktree}/.github/parity-trust-reviewers.txt" \
    "${revision_repository}" "${PROTECTED_SHA}" "${target_head_sha}" \
    "${event_kind}")"
  case "${mode}" in
    no-op)
      ;;
    trust-change-approved|trust-change-merge-group)
      python3 "${protected_worktree}/scripts/parity-signed-evidence.py" \
        check-trust-update \
        --protected-root "${protected_worktree}" \
        --protected-policy "${protected_worktree}/docs/parity-evidence/trust-policy-v2.tsv" \
        --candidate-root "${candidate_worktree}" \
        --candidate-policy "${candidate_worktree}/docs/parity-evidence/trust-policy-v2.tsv" \
        --protected-row-policy "${protected_worktree}/docs/parity-row-evidence-policy-v2.tsv" \
        --candidate-row-policy "${candidate_worktree}/docs/parity-row-evidence-policy-v2.tsv"
      ;;
    promotion)
      local manifest
      local transaction
      local archive_closure
      manifest="$(python3 "${protected_worktree}/scripts/parity-signed-evidence.py" \
        derive-promotion-manifest \
        --protected-archive "${protected_worktree}/docs/parity-evidence/archive" \
        --candidate-archive "${candidate_worktree}/docs/parity-evidence/archive")"
      transaction="${scratch}/transaction.tsv"
      archive_closure="${scratch}/archive-closure.tsv"
      python3 "${protected_worktree}/scripts/parity-signed-evidence.py" gate \
        --repo "${candidate_worktree}" \
        --archive-root "${candidate_worktree}/docs/parity-evidence/archive" \
        --trusted-root "${protected_worktree}" \
        --trust-policy "${protected_worktree}/docs/parity-evidence/trust-policy-v2.tsv" \
        --manifest "${manifest}" \
        --trusted-policy "${protected_worktree}/docs/parity-row-evidence-policy-v2.tsv" \
        --candidate-policy "${candidate_worktree}/docs/parity-row-evidence-policy-v2.tsv" \
        --baseline-status "${protected_worktree}/docs/cuda-oxide-parity-status.tsv" \
        --candidate-status "${candidate_worktree}/docs/cuda-oxide-parity-status.tsv" \
        --projection-output "${transaction}" \
        --archive-closure-output "${archive_closure}"
      bash "${protected_worktree}/scripts/parity-promotion-projections.sh" \
        "${protected_worktree}" "${candidate_worktree}" "${transaction}" \
        "${archive_closure}"
      ;;
    *)
      die 'protected classifier returned an unknown mode'
      ;;
  esac

  if [[ "${event_kind}" == pull-request ]]; then
    pr_b="${scratch}/pr-b.json"
    reviews_a="${scratch}/reviews-a.json"
    reviews_b="${scratch}/reviews-b.json"
    review_snapshot_a="${scratch}/review-snapshot-a.json"
    review_snapshot_b="${scratch}/review-snapshot-b.json"
    read_api "repos/${GITHUB_REPOSITORY}/pulls/${pr_number}" >"${pr_b}"
    verify_pr_snapshot "${pr_b}" "${pr_number}" "${PROTECTED_SHA}" \
      "${target_head_sha}"
    fetch_reviews "${pr_number}" "${reviews_a}"
    final_mode="$(bash "${protected_worktree}/scripts/parity-protected-change-policy.sh" \
      "${protected_worktree}" "${candidate_worktree}" \
      docs/cuda-oxide-parity-status.tsv "${reviews_a}" \
      "${protected_worktree}/.github/parity-trust-reviewers.txt" \
      "${revision_repository}" "${PROTECTED_SHA}" "${target_head_sha}" \
      "${event_kind}")"
    [[ "${final_mode}" == "${mode}" ]] ||
      die 'protected change mode changed during approval revalidation'
    canonicalize_reviews "${reviews_a}" "${review_snapshot_a}"
    fetch_reviews "${pr_number}" "${reviews_b}"
    canonicalize_reviews "${reviews_b}" "${review_snapshot_b}"
    cmp -s -- "${review_snapshot_a}" "${review_snapshot_b}" ||
      die 'pull request reviews changed during approval revalidation'
    read_api "repos/${GITHUB_REPOSITORY}/pulls/${pr_number}" >"${pr_a}"
    verify_pr_snapshot "${pr_a}" "${pr_number}" "${PROTECTED_SHA}" \
      "${target_head_sha}"
  else
    fetch_ref "${revision_repository}" "${GITHUB_REPOSITORY}" \
      "refs/heads/${DEFAULT_BRANCH}" refs/parity/final-base
    fetch_ref "${revision_repository}" "${GITHUB_REPOSITORY}" \
      "${head_ref}" refs/parity/final-head
    [[ "$(git -C "${revision_repository}" rev-parse \
      'refs/parity/final-base^{commit}')" == "${PROTECTED_SHA}" ]]
    [[ "$(git -C "${revision_repository}" rev-parse \
      'refs/parity/final-head^{commit}')" == "${target_head_sha}" ]]
  fi
}

process_target() {
  local event_kind="$1"
  local pr_number="$2"
  local target_head_ref="$3"
  local target_head_sha="$4"
  local source_run_id="$5"
  local source_workflow_id="$6"
  local source_path="$7"
  local source_event="$8"
  local source_head_branch="$9"
  local source_head_sha="${10}"
  local check_id
  local rc
  valid_sha "${target_head_sha}" || die 'target head SHA is malformed'
  check_id="$(upsert_pending_check "${target_head_sha}")"
  set +e
  bash "${BASH_SOURCE[0]}" verify-target "${event_kind}" "${pr_number}" \
    "${target_head_ref}" "${target_head_sha}" "${source_run_id}" \
    "${source_workflow_id}" "${source_path}" "${source_event}" \
    "${source_head_branch}" "${source_head_sha}"
  rc=$?
  set -e
  if ((rc == 0)); then
    complete_check "${check_id}" "${target_head_sha}" success
  else
    complete_check "${check_id}" "${target_head_sha}" failure
  fi
  return "${rc}"
}

run_event() {
  local event_json="$1"
  local event_name="${EVENT_NAME:?}"
  local source_run_id=0
  local source_workflow_id=0
  local source_path=''
  local source_event=''
  local source_head_branch=''
  local source_head_sha=''
  local pr_number
  local head_ref
  local head_sha
  [[ -f "${event_json}" && ! -L "${event_json}" ]] || die 'event JSON is missing'
  case "${event_name}" in
    pull_request_target)
      pr_number="$(jq -er '.pull_request.number' "${event_json}")"
      head_ref="refs/heads/$(jq -er '.pull_request.head.ref' "${event_json}")"
      head_sha="$(jq -er '.pull_request.head.sha' "${event_json}")"
      process_target pull-request "${pr_number}" "${head_ref}" "${head_sha}" \
        0 0 '' '' '' ''
      ;;
    workflow_run)
      source_run_id="$(jq -er '.workflow_run.id' "${event_json}")"
      source_workflow_id="$(jq -er '.workflow_run.workflow_id' "${event_json}")"
      source_path="$(jq -er '.workflow_run.path' "${event_json}")"
      source_event="$(jq -er '.workflow_run.event' "${event_json}")"
      source_head_branch="$(jq -er '.workflow_run.head_branch' "${event_json}")"
      source_head_sha="$(jq -er '.workflow_run.head_sha' "${event_json}")"
      case "${source_event}" in
        merge_group)
          head_ref="refs/heads/${source_head_branch}"
          process_target merge-group 0 "${head_ref}" "${source_head_sha}" \
            "${source_run_id}" "${source_workflow_id}" "${source_path}" \
            "${source_event}" "${source_head_branch}" "${source_head_sha}"
          ;;
        pull_request_review)
          pr_number="$(jq -er '
            .workflow_run.pull_requests |
            select(type == "array" and length == 1) | .[0].number
          ' "${event_json}")"
          head_ref="refs/heads/$(jq -er '.workflow_run.pull_requests[0].head.ref' \
            "${event_json}")"
          head_sha="$(jq -er '.workflow_run.pull_requests[0].head.sha' \
            "${event_json}")"
          process_target pull-request "${pr_number}" "${head_ref}" "${head_sha}" \
            "${source_run_id}" "${source_workflow_id}" "${source_path}" \
            "${source_event}" "${source_head_branch}" "${source_head_sha}"
          ;;
        *)
          die 'workflow run is not a recognized parity notification'
          ;;
      esac
      ;;
    *)
      die 'event mode received an unsupported event'
      ;;
  esac
}

run_reconciliation() {
  local pulls
  local queue_refs
  local failures=0
  local event_kind
  local target_number
  local head_ref
  local head_sha
  local header
  pulls="$(mktemp "${RUNNER_TEMP}/parity-open-prs.XXXXXX.json")"
  queue_refs="$(mktemp "${RUNNER_TEMP}/parity-queue-refs.XXXXXX.tsv")"
  read_api --paginate \
    "repos/${GITHUB_REPOSITORY}/pulls?state=open&per_page=100" |
    jq -s 'add' >"${pulls}"
  header="$(auth_header)"
  git -c "http.https://github.com/.extraheader=AUTHORIZATION: basic ${header}" \
    ls-remote --heads "https://github.com/${GITHUB_REPOSITORY}.git" \
    "refs/heads/gh-readonly-queue/${DEFAULT_BRANCH}/*" >"${queue_refs}"
  while IFS=$'\t' read -r event_kind target_number head_ref head_sha; do
    if ! bash "${BASH_SOURCE[0]}" process-target "${event_kind}" \
      "${target_number}" "${head_ref}" "${head_sha}" 0 0 '' '' '' ''; then
      ((failures += 1))
    fi
  done < <(emit_reconciliation_targets "${pulls}" "${queue_refs}")
  rm -f -- "${pulls}" "${queue_refs}"
  ((failures == 0)) || die "reconciliation rejected ${failures} active target(s)"
}

require_runtime
case "${1:-}" in
  event)
    [[ "$#" == 2 ]] || die 'usage: event EVENT_JSON'
    run_event "$2"
    ;;
  reconcile)
    [[ "$#" == 1 ]] || die 'usage: reconcile'
    run_reconciliation
    ;;
  verify-target)
    [[ "$#" == 11 ]] ||
      die 'usage: verify-target KIND PR REF SHA RUN_ID WORKFLOW_ID PATH EVENT SOURCE_REF SOURCE_SHA'
    verify_target "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" \
      "${10}" "${11}"
    ;;
  process-target)
    [[ "$#" == 11 ]] ||
      die 'usage: process-target KIND PR REF SHA RUN_ID WORKFLOW_ID PATH EVENT SOURCE_REF SOURCE_SHA'
    process_target "$2" "$3" "$4" "$5" "$6" "$7" "$8" "$9" \
      "${10}" "${11}"
    ;;
  *)
    die 'mode must be event, reconcile, process-target, or verify-target'
    ;;
esac
