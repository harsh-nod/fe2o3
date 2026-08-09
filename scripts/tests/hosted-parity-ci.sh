#!/usr/bin/env bash
# shellcheck disable=SC2016

set -Eeuo pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly PROTECTED_WORKFLOW="${ROOT}/.github/workflows/parity-promotion.yml"
readonly GENERIC_WORKFLOW="${ROOT}/.github/workflows/ci.yml"
readonly HARDWARE_WORKFLOW="${ROOT}/.github/workflows/hardware-smoke.yml"
readonly ROCM_WORKFLOW="${ROOT}/.github/workflows/rocm-compile.yml"
readonly CHANGE_POLICY="${ROOT}/scripts/parity-protected-change-policy.sh"
readonly CODEOWNERS="${ROOT}/.github/CODEOWNERS"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'protected CI negative test unexpectedly passed: %s\n' "${name}" >&2
    exit 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'protected CI negative test produced wrong diagnostic: %s\n' "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    exit 1
  }
}

fetch_default_tip() {
  local repo="$1"
  local default_branch="$2"
  local default_remote_ref="refs/remotes/origin/${default_branch}"
  git -C "${repo}" check-ref-format "refs/heads/${default_branch}" >/dev/null || {
    printf 'invalid default branch ref\n' >&2
    return 2
  }
  git -C "${repo}" fetch --no-tags --prune --force origin \
    "+refs/heads/${default_branch}:${default_remote_ref}" >/dev/null 2>&1 || {
    printf 'cannot fetch current protected default tip\n' >&2
    return 2
  }
  git -C "${repo}" rev-parse --verify "${default_remote_ref}^{commit}" || {
    printf 'cannot resolve current protected default tip\n' >&2
    return 2
  }
}

require_current_pr_base() {
  local repo="$1"
  local default_branch="$2"
  local event_base="$3"
  local current
  current="$(fetch_default_tip "${repo}" "${default_branch}")"
  if [[ "${current}" != "${event_base}" ]]; then
    printf 'pull request base SHA is not current default tip\n' >&2
    return 2
  fi
  printf '%s\n' "${current}"
}

resolve_non_default_base() {
  local repo="$1"
  local default_branch="$2"
  local head="$3"
  local base
  base="$(fetch_default_tip "${repo}" "${default_branch}")"
  if ! git -C "${repo}" merge-base --is-ancestor "${base}" "${head}"; then
    printf 'non-default branch head does not contain current protected default tip\n' >&2
    return 2
  fi
  printf '%s\n' "${base}"
}


require_text() {
  local file="$1"
  local text="$2"
  rg -F -- "${text}" "${file}" >/dev/null || {
    printf 'hosted parity CI is missing: %s\n' "${text}" >&2
    exit 1
  }
}

require_text "${PROTECTED_WORKFLOW}" 'pull_request_target:'
require_text "${PROTECTED_WORKFLOW}" 'merge_group:'
require_text "${PROTECTED_WORKFLOW}" 'types: [checks_requested]'
require_text "${PROTECTED_WORKFLOW}" 'workflow_run:'
require_text "${PROTECTED_WORKFLOW}" 'workflows: [CI]'
require_text "${PROTECTED_WORKFLOW}" 'types: [completed]'
require_text "${PROTECTED_WORKFLOW}" "github.event_name == 'workflow_run'"
require_text "${PROTECTED_WORKFLOW}" "github.event.workflow_run.event == 'merge_group'"
require_text "${PROTECTED_WORKFLOW}" "github.event.workflow_run.path == '.github/workflows/ci.yml'"
require_text "${PROTECTED_WORKFLOW}" 'WORKFLOW_SHA: ${{ github.workflow_sha }}'
require_text "${PROTECTED_WORKFLOW}" 'RUN_HEAD_SHA: ${{ github.event.workflow_run.head_sha }}'
require_text "${PROTECTED_WORKFLOW}" 'RUN_HEAD_BRANCH: ${{ github.event.workflow_run.head_branch }}'
require_text "${PROTECTED_WORKFLOW}" 'gh api "repos/${GITHUB_REPOSITORY}/actions/runs/${SOURCE_RUN_ID}"'
require_text "${PROTECTED_WORKFLOW}" 'merge-group source run changed during protected verification'
require_text "${PROTECTED_WORKFLOW}" 'REVISION_REPOSITORY="${RUNNER_TEMP}/parity-revisions.git"'
require_text "${PROTECTED_WORKFLOW}" 'git init --bare "${REVISION_REPOSITORY}"'
require_text "${PROTECTED_WORKFLOW}" '"+${BASE_REF}:refs/parity/base"'
require_text "${PROTECTED_WORKFLOW}" '"+${HEAD_REF}:refs/parity/head"'
require_text "${PROTECTED_WORKFLOW}" 'worktree add --detach'
require_text "${PROTECTED_WORKFLOW}" '"${REVISION_REPOSITORY}" "${BASE_SHA}" "${HEAD_SHA}"'
require_text "${PROTECTED_WORKFLOW}" '"${EVENT_KIND}")"'
require_text "${PROTECTED_WORKFLOW}" 'python3 protected/scripts/parity-signed-evidence.py gate'
require_text "${PROTECTED_WORKFLOW}" 'derive-promotion-manifest'
require_text "${PROTECTED_WORKFLOW}" '--protected-archive protected/docs/parity-evidence/archive'
require_text "${PROTECTED_WORKFLOW}" '--candidate-archive candidate/docs/parity-evidence/archive'
require_text "${PROTECTED_WORKFLOW}" '--manifest "${manifest}"'
require_text "${PROTECTED_WORKFLOW}" '--projection-output "${transaction}"'
require_text "${PROTECTED_WORKFLOW}" '--archive-closure-output "${archive_closure}"'
require_text "${PROTECTED_WORKFLOW}" 'bash protected/scripts/parity-promotion-projections.sh'
require_text "${PROTECTED_WORKFLOW}" '--trust-policy protected/docs/parity-evidence/trust-policy-v2.tsv'
require_text "${PROTECTED_WORKFLOW}" '--trusted-policy protected/docs/parity-row-evidence-policy-v2.tsv'
require_text "${PROTECTED_WORKFLOW}" '--candidate-policy candidate/docs/parity-row-evidence-policy-v2.tsv'
require_text "${PROTECTED_WORKFLOW}" 'pull-requests: read'
require_text "${PROTECTED_WORKFLOW}" 'actions: read'
require_text "${PROTECTED_WORKFLOW}" 'cancel-in-progress: false'
require_text "${PROTECTED_WORKFLOW}" 'protected/scripts/parity-protected-change-policy.sh'
require_text "${PROTECTED_WORKFLOW}" 'gh api --paginate'
require_text "${PROTECTED_WORKFLOW}" 'trust-change-approved'
require_text "${PROTECTED_WORKFLOW}" 'python3 protected/scripts/parity-signed-evidence.py check-trust-update'
require_text "${PROTECTED_WORKFLOW}" '--protected-row-policy protected/docs/parity-row-evidence-policy-v2.tsv'
require_text "${PROTECTED_WORKFLOW}" '--candidate-row-policy candidate/docs/parity-row-evidence-policy-v2.tsv'
require_text "${PROTECTED_WORKFLOW}" 'pull_request_review:'
require_text "${PROTECTED_WORKFLOW}" 'types: [submitted, edited, dismissed]'
require_text "${PROTECTED_WORKFLOW}" 'PR_BASE_REPOSITORY: ${{ github.event.pull_request.base.repo.full_name }}'
require_text "${PROTECTED_WORKFLOW}" '[[ "${BASE_REPOSITORY}" == "${GITHUB_REPOSITORY}" ]]'
require_text "${PROTECTED_WORKFLOW}" '[[ "${BASE_REF}" == "refs/heads/${DEFAULT_BRANCH}" ]]'
require_text "${PROTECTED_WORKFLOW}" 'FETCHED_BASE_SHA="$(git -C "${REVISION_REPOSITORY}"'
require_text "${PROTECTED_WORKFLOW}" 'FETCHED_HEAD_SHA="$(git -C "${REVISION_REPOSITORY}"'
require_text "${PROTECTED_WORKFLOW}" '[[ "${FETCHED_BASE_SHA}" == "${BASE_SHA}" ]]'
require_text "${PROTECTED_WORKFLOW}" '[[ "${FETCHED_HEAD_SHA}" == "${HEAD_SHA}" ]]'
require_text "${PROTECTED_WORKFLOW}" 'PR_HEAD_SHA: ${{ github.event.pull_request.head.sha }}'
require_text "${PROTECTED_WORKFLOW}" 'MERGE_BASE_SHA: ${{ github.event.merge_group.base_sha }}'
require_text "${PROTECTED_WORKFLOW}" 'MERGE_HEAD_SHA: ${{ github.event.merge_group.head_sha }}'
require_text "${PROTECTED_WORKFLOW}" 'merge-base --is-ancestor'
require_text "${PROTECTED_WORKFLOW}" 'pull request revision changed during protected verification'
require_text "${PROTECTED_WORKFLOW}" 'gh api "repos/${GITHUB_REPOSITORY}/pulls/${PR_NUMBER}"'
require_text "${PROTECTED_WORKFLOW}" '.base.sha == $base_sha and .head.sha == $head_sha'
require_text "${PROTECTED_WORKFLOW}" '"+${HEAD_REF}:refs/parity/final-head"'
require_text "${PROTECTED_WORKFLOW}" 'environment: parity-verdict'
require_text "${PROTECTED_WORKFLOW}" 'actions/create-github-app-token@fee1f7d63c2ff003460e3d139729b119787bc349'
require_text "${PROTECTED_WORKFLOW}" 'permission-checks: write'
require_text "${PROTECTED_WORKFLOW}" 'app-id: ${{ vars.PARITY_VERDICT_APP_ID }}'
require_text "${PROTECTED_WORKFLOW}" 'private-key: ${{ secrets.PARITY_VERDICT_APP_PRIVATE_KEY }}'
require_text "${PROTECTED_WORKFLOW}" 'CHECK_NAME: fe2o3/protected-parity-promotion'
require_text "${PROTECTED_WORKFLOW}" 'HEAD_SHA: ${{ steps.context.outputs.head_sha }}'
require_text "${PROTECTED_WORKFLOW}" 'GH_TOKEN: ${{ steps.verdict-token.outputs.token }}'
require_text "${PROTECTED_WORKFLOW}" '"repos/${GITHUB_REPOSITORY}/check-runs"'
require_text "${PROTECTED_WORKFLOW}" '"repos/${GITHUB_REPOSITORY}/check-runs/${CHECK_ID}"'
require_text "${PROTECTED_WORKFLOW}" '.app.id | tostring'
require_text "${PROTECTED_WORKFLOW}" '.app.slug == $app_slug'
require_text "${PROTECTED_WORKFLOW}" 'status:"in_progress"'
require_text "${PROTECTED_WORKFLOW}" 'status:"completed",conclusion:$conclusion'
require_text "${PROTECTED_WORKFLOW}" 'continue-on-error: true'
require_text "${PROTECTED_WORKFLOW}" 'if: always() && steps.verdict.outputs.check_id !='
require_text "${PROTECTED_WORKFLOW}" 'if: always() && steps.verdict.outputs.check_id =='
require_text "${CHANGE_POLICY}" 'git -C "${REVISION_REPOSITORY}" diff --no-ext-diff --no-renames'
require_text "${CHANGE_POLICY}" '--name-only -z "${BASE_SHA}" "${CANDIDATE_HEAD}"'
require_text "${CHANGE_POLICY}" 'revision worktree does not match declared commit'
require_text "${CHANGE_POLICY}" 'merge-group head does not descend from its base'

if rg -n '^\s+paths:' "${PROTECTED_WORKFLOW}"; then
  printf 'required protected verdict is incorrectly path-filtered\n' >&2
  exit 1
fi
for ownership in \
  /docs/parity-signed-evidence-v2.md \
  /docs/parity-evidence/trust-policy-v2.example.tsv \
  /docs/parity-row-evidence-policy-v2.tsv \
  /docs/parity-evidence/trust-policy-v2.tsv \
  /docs/parity-evidence/trusted-keys/ \
  /scripts/parity-signed-evidence.py \
  /scripts/parity-protected-change-policy.sh \
  /scripts/parity-dashboard.sh \
  /scripts/parity-matrix.sh \
  /scripts/parity-promotion-projections.sh \
  /scripts/tests/hosted-parity-ci.sh \
  /scripts/tests/parity-dashboard.sh \
  /scripts/tests/parity-promotion-projections.sh \
  /scripts/tests/parity-row-evidence.sh \
  /.github/workflows/parity-promotion.yml \
  /.github/CODEOWNERS; do
  require_text "${CODEOWNERS}" "${ownership} @powderluv"
done


require_text "${GENERIC_WORKFLOW}" 'git archive "${BASE_SHA}"'
require_text "${GENERIC_WORKFLOW}" 'python3 "${trusted}/scripts/parity-signed-evidence.py" gate'
require_text "${GENERIC_WORKFLOW}" 'derive-promotion-manifest'
require_text "${GENERIC_WORKFLOW}" '--protected-archive "${trusted}/docs/parity-evidence/archive"'
require_text "${GENERIC_WORKFLOW}" '--candidate-archive "${archive}"'
require_text "${GENERIC_WORKFLOW}" '--manifest "${manifest}"'
require_text "${GENERIC_WORKFLOW}" '--projection-output "${transaction}"'
require_text "${GENERIC_WORKFLOW}" '--archive-closure-output "${archive_closure}"'
require_text "${GENERIC_WORKFLOW}" 'bash "${trusted}/scripts/parity-promotion-projections.sh"'
require_text "${GENERIC_WORKFLOW}" 'git archive "${BASE_SHA}" | tar -x -C "${trusted}"'
require_text "${GENERIC_WORKFLOW}" '--trust-policy "${trusted}/docs/parity-evidence/trust-policy-v2.tsv"'
require_text "${GENERIC_WORKFLOW}" '--trusted-policy "${trusted}/docs/parity-row-evidence-policy-v2.tsv"'
require_text "${GENERIC_WORKFLOW}" 'EVENT_NAME: ${{ github.event_name }}'
require_text "${GENERIC_WORKFLOW}" 'PUSH_BEFORE_SHA: ${{ github.event.before }}'
require_text "${GENERIC_WORKFLOW}" 'EVENT_HEAD_SHA: ${{ github.sha }}'
require_text "${GENERIC_WORKFLOW}" 'PR_HEAD_SHA: ${{ github.event.pull_request.head.sha }}'
require_text "${GENERIC_WORKFLOW}" 'MERGE_BASE_SHA: ${{ github.event.merge_group.base_sha }}'
require_text "${GENERIC_WORKFLOW}" 'MERGE_HEAD_SHA: ${{ github.event.merge_group.head_sha }}'
require_text "${GENERIC_WORKFLOW}" 'merge_group:'
require_text "${GENERIC_WORKFLOW}" 'types: [checks_requested]'
require_text "${GENERIC_WORKFLOW}" 'BASE_SHA="${MERGE_BASE_SHA}"'
require_text "${GENERIC_WORKFLOW}" 'HEAD_SHA="${MERGE_HEAD_SHA}"'
require_text "${GENERIC_WORKFLOW}" 'merge-group head does not contain its declared base'
require_text "${GENERIC_WORKFLOW}" 'REF_NAME: ${{ github.ref_name }}'
require_text "${GENERIC_WORKFLOW}" 'git fetch --no-tags --prune --force origin'
require_text "${GENERIC_WORKFLOW}" '"+refs/heads/${DEFAULT_BRANCH}:${DEFAULT_REMOTE_REF}"'
require_text "${GENERIC_WORKFLOW}" 'BASE_SHA="$(git rev-parse --verify "${DEFAULT_REMOTE_REF}^{commit}")"'
require_text "${GENERIC_WORKFLOW}" 'git merge-base --is-ancestor "${BASE_SHA}" "${HEAD_SHA}"'
require_text "${GENERIC_WORKFLOW}" 'non-default branch head does not contain current protected default tip'
require_text "${GENERIC_WORKFLOW}" 'git diff --quiet "${BASE_SHA}" "${HEAD_SHA}"'
require_text "${GENERIC_WORKFLOW}" 'docs/parity-evidence/archive/'
require_text "${GENERIC_WORKFLOW}" 'parity evidence archive changed without a signed status promotion'
require_text "${GENERIC_WORKFLOW}" 'missing or zero parity base SHA'
require_text "${GENERIC_WORKFLOW}" 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683'
require_text "${HARDWARE_WORKFLOW}" 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683'
require_text "${ROCM_WORKFLOW}" 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683'
if rg -n 'git rev-parse HEAD\^' "${GENERIC_WORKFLOW}"; then
  printf 'generic parity CI does not cover the complete push range\n' >&2
  exit 1
fi
if rg -n 'manifests/promotion-v2\.tsv' "${GENERIC_WORKFLOW}" "${PROTECTED_WORKFLOW}"; then
  printf 'hosted parity CI hardcodes a mutable promotion manifest path\n' >&2
  exit 1
fi
if rg -n 'BASE_SHA="\$\(git merge-base' "${GENERIC_WORKFLOW}"; then
  printf 'generic parity CI trusts a historical merge base\n' >&2
  exit 1
fi
if rg -n '/pulls/.*/files|/files\?per_page|CHANGED_FILE_COUNT|changed_files' \
  "${PROTECTED_WORKFLOW}"; then
  printf 'protected parity CI trusts a live pull-request file list\n' >&2
  exit 1
fi
if rg -n 'actions/checkout@(v[0-9]+|main|master)' \
  "${GENERIC_WORKFLOW}" "${PROTECTED_WORKFLOW}" "${HARDWARE_WORKFLOW}" "${ROCM_WORKFLOW}"; then
  printf 'hosted CI uses a mutable checkout action reference\n' >&2
  exit 1
fi
if rg -n 'actions/create-github-app-token@(v[0-9]+|main|master)' \
  "${PROTECTED_WORKFLOW}"; then
  printf 'protected parity CI uses a mutable token action reference\n' >&2
  exit 1
fi
if rg -n '^  checks: write$' "${PROTECTED_WORKFLOW}"; then
  printf 'workflow token must not have checks write permission\n' >&2
  exit 1
fi
bootstrap_job="$(sed -n '/^  merge-group-bootstrap:/,/^  verify:/p' \
  "${PROTECTED_WORKFLOW}")"
if rg -n 'environment:|PARITY_VERDICT_APP|verdict-token|permission-checks' \
  <<<"${bootstrap_job}"; then
  printf 'unprivileged merge-group bootstrap receives verdict authority\n' >&2
  exit 1
fi


if rg -n 'python3 candidate/|candidate/scripts/|scripts/parity-row-evidence.sh gate' "${PROTECTED_WORKFLOW}"; then
  printf 'protected parity CI invokes candidate executable content\n' >&2
  exit 1
fi

# Check-run payloads and responses remain bound to the exact revision and the
# dedicated verdict App. A candidate workflow can copy the name, but its App
# identity cannot satisfy the protected response/ruleset binding.
verify_check_response() {
  local response="$1"
  local expected_id="$2"
  local expected_name="$3"
  local expected_head="$4"
  local expected_app_id="$5"
  local expected_app_slug="$6"
  local expected_status="$7"
  local expected_conclusion="$8"
  jq -e \
    --argjson id "${expected_id}" \
    --arg name "${expected_name}" \
    --arg head "${expected_head}" \
    --arg app_id "${expected_app_id}" \
    --arg app_slug "${expected_app_slug}" \
    --arg status "${expected_status}" \
    --arg conclusion "${expected_conclusion}" \
    '.id == $id and .name == $name and .head_sha == $head and
      .status == $status and
      (($conclusion == "" and .conclusion == null) or
       ($conclusion != "" and .conclusion == $conclusion)) and
      (.app.id | tostring) == $app_id and .app.slug == $app_slug' \
    "${response}" >/dev/null || {
    printf 'check response binding mismatch\n' >&2
    return 2
  }
}

CHECK_NAME=fe2o3/protected-parity-promotion
CHECK_HEAD=1111111111111111111111111111111111111111
CHECK_APP_ID=424242
CHECK_APP_SLUG=fe2o3-parity-verdict
CREATE_PAYLOAD="${TEST_ROOT}/check-create-payload.json"
COMPLETE_PAYLOAD="${TEST_ROOT}/check-complete-payload.json"
CHECK_RESPONSE="${TEST_ROOT}/check-response.json"
jq -n --arg name "${CHECK_NAME}" --arg head_sha "${CHECK_HEAD}" \
  '{name:$name,head_sha:$head_sha,status:"in_progress"}' >"${CREATE_PAYLOAD}"
jq -e --arg name "${CHECK_NAME}" --arg head "${CHECK_HEAD}" \
  '.name == $name and .head_sha == $head and .status == "in_progress" and
    (has("conclusion") | not)' "${CREATE_PAYLOAD}" >/dev/null
jq -n '{status:"completed",conclusion:"success"}' >"${COMPLETE_PAYLOAD}"
jq -e '.status == "completed" and .conclusion == "success" and
  (has("head_sha") | not)' "${COMPLETE_PAYLOAD}" >/dev/null
jq -n --arg name "${CHECK_NAME}" --arg head "${CHECK_HEAD}" \
  --arg slug "${CHECK_APP_SLUG}" --argjson app_id "${CHECK_APP_ID}" \
  '{id:7,name:$name,head_sha:$head,status:"in_progress",conclusion:null,
    app:{id:$app_id,slug:$slug}}' >"${CHECK_RESPONSE}"
verify_check_response "${CHECK_RESPONSE}" 7 "${CHECK_NAME}" "${CHECK_HEAD}" \
  "${CHECK_APP_ID}" "${CHECK_APP_SLUG}" in_progress ''
expect_failure check_wrong_sha 'check response binding mismatch' verify_check_response \
  "${CHECK_RESPONSE}" 7 "${CHECK_NAME}" \
  2222222222222222222222222222222222222222 \
  "${CHECK_APP_ID}" "${CHECK_APP_SLUG}" in_progress ''
expect_failure check_wrong_id 'check response binding mismatch' verify_check_response \
  "${CHECK_RESPONSE}" 8 "${CHECK_NAME}" "${CHECK_HEAD}" \
  "${CHECK_APP_ID}" "${CHECK_APP_SLUG}" in_progress ''
expect_failure candidate_spoof_source 'check response binding mismatch' verify_check_response \
  "${CHECK_RESPONSE}" 7 "${CHECK_NAME}" "${CHECK_HEAD}" \
  15368 github-actions in_progress ''

jq -n --arg name "${CHECK_NAME}" --arg head "${CHECK_HEAD}" \
  --arg slug "${CHECK_APP_SLUG}" --argjson app_id "${CHECK_APP_ID}" \
  '{id:7,name:$name,head_sha:$head,status:"completed",conclusion:"failure",
    app:{id:$app_id,slug:$slug}}' >"${CHECK_RESPONSE}"
verify_check_response "${CHECK_RESPONSE}" 7 "${CHECK_NAME}" "${CHECK_HEAD}" \
  "${CHECK_APP_ID}" "${CHECK_APP_SLUG}" completed failure
expect_failure wrong_conclusion 'check response binding mismatch' verify_check_response \
  "${CHECK_RESPONSE}" 7 "${CHECK_NAME}" "${CHECK_HEAD}" \
  "${CHECK_APP_ID}" "${CHECK_APP_SLUG}" completed success

verify_pr_snapshot() {
  local snapshot="$1"
  local expected_base="$2"
  local expected_head="$3"
  jq -e --arg base "${expected_base}" --arg head "${expected_head}" \
    '.number == 17 and .state == "open" and .draft == false and
      .base.sha == $base and .head.sha == $head and
      .base.repo.full_name == "powderluv/fe2o3" and
      .head.repo.full_name == "contributor/fe2o3" and
      .base.ref == "main" and .head.ref == "promotion"' \
    "${snapshot}" >/dev/null || {
    printf 'pull request snapshot mismatch\n' >&2
    return 2
  }
}

PR_SNAPSHOT="${TEST_ROOT}/current-pr.json"
PR_BASE=aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
PR_HEAD=bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
jq -n --arg base "${PR_BASE}" --arg head "${PR_HEAD}" \
  '{number:17,state:"open",draft:false,
    base:{sha:$base,ref:"main",repo:{full_name:"powderluv/fe2o3"}},
    head:{sha:$head,ref:"promotion",repo:{full_name:"contributor/fe2o3"}}}' \
  >"${PR_SNAPSHOT}"
verify_pr_snapshot "${PR_SNAPSHOT}" "${PR_BASE}" "${PR_HEAD}"
expect_failure force_push_current_pr 'pull request snapshot mismatch' verify_pr_snapshot \
  "${PR_SNAPSHOT}" "${PR_BASE}" cccccccccccccccccccccccccccccccccccccccc
expect_failure base_advanced_current_pr 'pull request snapshot mismatch' verify_pr_snapshot \
  "${PR_SNAPSHOT}" dddddddddddddddddddddddddddddddddddddddd "${PR_HEAD}"

verify_merge_source_run() {
  local snapshot="$1"
  local expected_head="$2"
  local expected_branch="$3"
  jq -e --arg head "${expected_head}" --arg branch "${expected_branch}" \
    '.id == 91 and .event == "merge_group" and
      .path == ".github/workflows/ci.yml" and .status == "completed" and
      .head_sha == $head and .head_branch == $branch' \
    "${snapshot}" >/dev/null || {
    printf 'merge-group source run mismatch\n' >&2
    return 2
  }
}

MERGE_SOURCE="${TEST_ROOT}/merge-source-run.json"
MERGE_HEAD=eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
MERGE_BRANCH=gh-readonly-queue/main/pr-17-abcdef
jq -n --arg head "${MERGE_HEAD}" --arg branch "${MERGE_BRANCH}" \
  '{id:91,event:"merge_group",path:".github/workflows/ci.yml",
    status:"completed",head_sha:$head,head_branch:$branch}' >"${MERGE_SOURCE}"
verify_merge_source_run "${MERGE_SOURCE}" "${MERGE_HEAD}" "${MERGE_BRANCH}"
expect_failure merge_source_wrong_head 'merge-group source run mismatch' \
  verify_merge_source_run "${MERGE_SOURCE}" \
  ffffffffffffffffffffffffffffffffffffffff "${MERGE_BRANCH}"
expect_failure merge_source_wrong_ref 'merge-group source run mismatch' \
  verify_merge_source_run "${MERGE_SOURCE}" "${MERGE_HEAD}" \
  gh-readonly-queue/main/pr-18-fedcba
jq '.path = ".github/workflows/spoof.yml"' "${MERGE_SOURCE}" \
  >"${TEST_ROOT}/merge-source-spoof.json"
expect_failure merge_source_wrong_path 'merge-group source run mismatch' \
  verify_merge_source_run "${TEST_ROOT}/merge-source-spoof.json" \
  "${MERGE_HEAD}" "${MERGE_BRANCH}"

# A strengthened default policy invalidates a stale non-default branch. Once
# updated, the branch is checked against and archives trust from the exact tip.
REMOTE="${TEST_ROOT}/default-tip-origin.git"
SEED="${TEST_ROOT}/default-tip-seed"
RUNNER="${TEST_ROOT}/default-tip-runner"
git init -q --bare "${REMOTE}"
git init -q "${SEED}"
git -C "${SEED}" config user.name 'Evidence Test'
git -C "${SEED}" config user.email evidence@example.invalid
mkdir -p "${SEED}/docs"
printf 'weak\n' >"${SEED}/docs/parity-row-evidence-policy-v2.tsv"
git -C "${SEED}" add docs/parity-row-evidence-policy-v2.tsv
git -C "${SEED}" commit -qm 'baseline policy'
git -C "${SEED}" branch -M main
git -C "${SEED}" remote add origin "${REMOTE}"
git -C "${SEED}" push -q -u origin main
HISTORICAL_BASE="$(git -C "${SEED}" rev-parse HEAD)"
git -C "${SEED}" switch -q -c feature
printf 'feature\n' >"${SEED}/feature.txt"
git -C "${SEED}" add feature.txt
git -C "${SEED}" commit -qm 'feature commit'
git -C "${SEED}" push -q -u origin feature
FEATURE_HEAD="$(git -C "${SEED}" rev-parse HEAD)"
git -C "${SEED}" switch -q main
printf 'strong\n' >"${SEED}/docs/parity-row-evidence-policy-v2.tsv"
git -C "${SEED}" commit -qam 'strengthen default policy'
git -C "${SEED}" push -q origin main
DEFAULT_TIP="$(git -C "${SEED}" rev-parse HEAD)"
git clone -q --branch feature "${REMOTE}" "${RUNNER}"
git -C "${RUNNER}" merge-base --is-ancestor "${HISTORICAL_BASE}" "${DEFAULT_TIP}"
expect_failure stale_historical_pr_base \
  'pull request base SHA is not current default tip' \
  require_current_pr_base "${RUNNER}" main "${HISTORICAL_BASE}"
[[ "$(require_current_pr_base "${RUNNER}" main "${DEFAULT_TIP}")" == "${DEFAULT_TIP}" ]]
expect_failure stale_branch_strengthened_default \
  'non-default branch head does not contain current protected default tip' \
  resolve_non_default_base "${RUNNER}" main "${FEATURE_HEAD}"
git -C "${RUNNER}" config user.name 'Evidence Test'
git -C "${RUNNER}" config user.email evidence@example.invalid
git -C "${RUNNER}" merge --no-edit refs/remotes/origin/main >/dev/null
UPDATED_HEAD="$(git -C "${RUNNER}" rev-parse HEAD)"
RESOLVED_TIP="$(resolve_non_default_base "${RUNNER}" main "${UPDATED_HEAD}")"
[[ "${RESOLVED_TIP}" == "${DEFAULT_TIP}" ]]
[[ "$(git -C "${RUNNER}" show "${RESOLVED_TIP}:docs/parity-row-evidence-policy-v2.tsv")" == strong ]]

# Exercise the protected classifier against exact commits in a runner-owned
# bare repository. API-reported file lists are deliberately not an input.
SEED_REPOSITORY="${TEST_ROOT}/revision-seed"
REVISION_REPOSITORY="${TEST_ROOT}/revisions.git"
PROTECTED="${TEST_ROOT}/protected"
CANDIDATE="${TEST_ROOT}/candidate"
REVIEWS="${TEST_ROOT}/reviews.json"
FAKE_API_FILES="${TEST_ROOT}/untrusted-api-files.json"
git init -q "${SEED_REPOSITORY}"
git -C "${SEED_REPOSITORY}" config user.name 'Evidence Test'
git -C "${SEED_REPOSITORY}" config user.email evidence@example.invalid
mkdir -p \
  "${SEED_REPOSITORY}/docs/generated" \
  "${SEED_REPOSITORY}/docs/parity-evidence/archive/history" \
  "${SEED_REPOSITORY}/docs/parity-evidence/trusted-keys" \
  "${SEED_REPOSITORY}/scripts" "${SEED_REPOSITORY}/.github"
printf 'status\tMissing\n' >"${SEED_REPOSITORY}/docs/cuda-oxide-parity-status.tsv"
printf 'protected matrix prose\n' >"${SEED_REPOSITORY}/docs/cuda-oxide-parity-matrix.md"
printf 'dashboard\n' >"${SEED_REPOSITORY}/docs/generated/cuda-oxide-parity-dashboard.md"
printf 'dashboard-tsv\n' >"${SEED_REPOSITORY}/docs/generated/cuda-oxide-parity-dashboard.tsv"
printf 'signed_promotion_projection_schema_version\t1\nrow_count\t0\n' \
  >"${SEED_REPOSITORY}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
printf 'protected verifier\n' >"${SEED_REPOSITORY}/scripts/parity-signed-evidence.py"
printf 'powderluv\n' >"${SEED_REPOSITORY}/.github/parity-trust-reviewers.txt"
printf 'protected evidence\n' \
  >"${SEED_REPOSITORY}/docs/parity-evidence/archive/history/prior.tsv"
printf 'key one\n' \
  >"${SEED_REPOSITORY}/docs/parity-evidence/trusted-keys/runner.pem"
git -C "${SEED_REPOSITORY}" add .
git -C "${SEED_REPOSITORY}" commit -qm 'protected baseline'
BASE_SHA="$(git -C "${SEED_REPOSITORY}" rev-parse HEAD)"
git clone -q --bare "${SEED_REPOSITORY}" "${REVISION_REPOSITORY}"
git -C "${REVISION_REPOSITORY}" worktree add -q --detach "${PROTECTED}" "${BASE_SHA}"
git -C "${REVISION_REPOSITORY}" worktree add -q -b candidate "${CANDIDATE}" "${BASE_SHA}"
git -C "${CANDIDATE}" config user.name 'Evidence Test'
git -C "${CANDIDATE}" config user.email evidence@example.invalid
HEAD_SHA="${BASE_SHA}"

write_approval() {
  jq -n --arg head "${HEAD_SHA}" \
    '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}}]' \
    >"${REVIEWS}"
}

refresh_policy_args() {
  policy_args=(
    "${PROTECTED}"
    "${CANDIDATE}"
    docs/cuda-oxide-parity-status.tsv
    "${REVIEWS}"
    "${PROTECTED}/.github/parity-trust-reviewers.txt"
    "${REVISION_REPOSITORY}"
    "${BASE_SHA}"
    "${HEAD_SHA}"
    pull-request
  )
}

commit_candidate() {
  local message="$1"
  git -C "${CANDIDATE}" add -A
  git -C "${CANDIDATE}" commit -qm "${message}"
  HEAD_SHA="$(git -C "${CANDIDATE}" rev-parse HEAD)"
  write_approval
  refresh_policy_args
}

reset_candidate() {
  git -C "${CANDIDATE}" reset -q --hard "${BASE_SHA}"
  git -C "${CANDIDATE}" clean -qfdx
  HEAD_SHA="${BASE_SHA}"
  write_approval
  refresh_policy_args
}

write_approval
refresh_policy_args
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == no-op ]]

printf 'status\tPartial\n' >"${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
commit_candidate 'status promotion'
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == promotion ]]
reset_candidate

printf 'candidate prose\n' >>"${CANDIDATE}/docs/cuda-oxide-parity-matrix.md"
commit_candidate 'projection without status'
expect_failure projection_without_status \
  'parity projections may change only with a signed status promotion' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

printf 'unbound evidence\n' \
  >"${CANDIDATE}/docs/parity-evidence/archive/unbound.tsv"
commit_candidate 'archive-only add'
expect_failure archive_only_add \
  'parity evidence archive may change only with a signed status promotion' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

printf 'mutated evidence\n' \
  >"${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
commit_candidate 'archive-only modify'
expect_failure archive_only_modify \
  'parity evidence archive may change only with a signed status promotion' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

rm "${CANDIDATE}/docs/parity-evidence/archive/history/prior.tsv"
commit_candidate 'archive-only delete'
expect_failure archive_only_delete \
  'parity evidence archive may change only with a signed status promotion' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

printf 'candidate verifier\n' >"${CANDIDATE}/scripts/parity-signed-evidence.py"
commit_candidate 'approved trust change'
printf '[]\n' >"${REVIEWS}"
expect_failure missing_designated_review 'designated reviewer has no review' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
write_approval
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == trust-change-approved ]]
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]:0:8}" merge-group)" == \
  trust-change-merge-group ]]

APPROVED_HEAD="${HEAD_SHA}"
printf 'force-pushed verifier\n' >"${CANDIDATE}/scripts/parity-signed-evidence.py"
commit_candidate 'force-pushed trust revision'
jq -n --arg head "${APPROVED_HEAD}" \
  '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}}]' \
  >"${REVIEWS}"
expect_failure force_push_stale_approval 'approval does not bind candidate head' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
write_approval

DECLARED_HEAD="${HEAD_SHA}"
printf 'dirty replacement\n' >"${CANDIDATE}/scripts/parity-signed-evidence.py"
expect_failure dirty_revision 'revision worktree is dirty' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
git -C "${CANDIDATE}" reset -q --hard "${DECLARED_HEAD}"
policy_args[7]="${APPROVED_HEAD}"
expect_failure mismatched_revision 'revision worktree does not match declared commit' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
policy_args[7]="${HEAD_SHA}"

# A forged live API list cannot hide the README delta: it is not consumed, and
# the immutable two-tree diff still rejects the arbitrary path.
printf '%s\n' '[{"filename":"scripts/parity-signed-evidence.py"}]' >"${FAKE_API_FILES}"
printf 'unrelated\n' >"${CANDIDATE}/README.md"
commit_candidate 'mixed trust and arbitrary path'
expect_failure mismatched_api_list \
  'trust changes must be isolated from arbitrary paths: README.md' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

printf 'candidate verifier\n' >"${CANDIDATE}/scripts/parity-signed-evidence.py"
commit_candidate 'review state fixture'
jq -n --arg head "${HEAD_SHA}" \
  '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}},{"id":2,"submitted_at":"2026-01-02T00:00:00Z","state":"CHANGES_REQUESTED","commit_id":$head,"user":{"login":"powderluv"}}]' \
  >"${REVIEWS}"
expect_failure latest_review_state 'latest state is not APPROVED' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
write_approval
printf 'status\tPartial\n' >"${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
commit_candidate 'mixed trust and status'
expect_failure mixed_trust_status 'trust changes must not include parity status changes' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

printf 'key two\n' >"${CANDIDATE}/docs/parity-evidence/trusted-keys/runner.pem"
commit_candidate 'trusted key update'
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == trust-change-approved ]]
reset_candidate

rm "${CANDIDATE}/scripts/parity-signed-evidence.py"
ln -s ../docs/cuda-oxide-parity-status.tsv \
  "${CANDIDATE}/scripts/parity-signed-evidence.py"
commit_candidate 'trust symlink'
expect_failure candidate_trust_symlink 'candidate trust path is not a regular file' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

TREE_OBJECT="$(git -C "${REVISION_REPOSITORY}" rev-parse "${BASE_SHA}^{tree}")"
policy_args[7]="${TREE_OBJECT}"
expect_failure non_commit_revision 'revision does not resolve to a commit' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
policy_args[7]="${HEAD_SHA}"

printf 'orphan verifier\n' >"${CANDIDATE}/scripts/parity-signed-evidence.py"
git -C "${CANDIDATE}" add -A
ORPHAN_TREE="$(git -C "${CANDIDATE}" write-tree)"
ORPHAN_SHA="$(printf 'orphan\n' | git -C "${REVISION_REPOSITORY}" \
  -c user.name='Evidence Test' -c user.email=evidence@example.invalid \
  commit-tree "${ORPHAN_TREE}")"
git -C "${CANDIDATE}" reset -q --hard "${ORPHAN_SHA}"
HEAD_SHA="${ORPHAN_SHA}"
write_approval
refresh_policy_args
policy_args[8]=merge-group
expect_failure merge_group_ancestry \
  'merge-group head does not descend from its base' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

bash "${ROOT}/scripts/tests/parity-promotion-projections.sh"

bash -n "${BASH_SOURCE[0]}"
shellcheck "${BASH_SOURCE[0]}"
printf 'hosted parity CI trust-boundary tests passed\n'
