#!/usr/bin/env bash
# shellcheck disable=SC2016

set -Eeuo pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly PROTECTED_WORKFLOW="${ROOT}/.github/workflows/parity-promotion.yml"
readonly REVIEW_WORKFLOW="${ROOT}/.github/workflows/parity-review-signal.yml"
readonly GENERIC_WORKFLOW="${ROOT}/.github/workflows/ci.yml"
readonly HARDWARE_WORKFLOW="${ROOT}/.github/workflows/hardware-smoke.yml"
readonly ROCM_WORKFLOW="${ROOT}/.github/workflows/rocm-compile.yml"
readonly CHANGE_POLICY="${ROOT}/scripts/parity-protected-change-policy.sh"
readonly PROTECTED_CONTROLLER="${ROOT}/scripts/parity-protected-controller.sh"
readonly CHECK_RECONCILE="${ROOT}/scripts/parity-check-reconcile.sh"
readonly CODEOWNERS="${ROOT}/.github/CODEOWNERS"
readonly PARITY_DOC="${ROOT}/docs/parity-signed-evidence-v2.md"
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
require_text "${PROTECTED_WORKFLOW}" 'workflows: [CI, Parity review signal]'
require_text "${PROTECTED_WORKFLOW}" 'types: [completed]'
require_text "${PROTECTED_WORKFLOW}" "github.event_name == 'workflow_run'"
require_text "${PROTECTED_WORKFLOW}" "github.event.workflow_run.event == 'merge_group'"
require_text "${PROTECTED_WORKFLOW}" "github.event.workflow_run.event == 'pull_request_review'"
require_text "${PROTECTED_WORKFLOW}" 'schedule:'
require_text "${PROTECTED_WORKFLOW}" "cron: '17 * * * *'"
require_text "${PROTECTED_WORKFLOW}" 'workflow_dispatch:'
require_text "${PROTECTED_WORKFLOW}" 'ref: ${{ github.workflow_sha }}'
require_text "${PROTECTED_WORKFLOW}" 'path: protected-controller'
require_text "${PROTECTED_WORKFLOW}" 'persist-credentials: false'
require_text "${PROTECTED_WORKFLOW}" 'PROTECTED_SHA: ${{ github.workflow_sha }}'
require_text "${PROTECTED_WORKFLOW}" 'EXPECTED_CI_WORKFLOW_ID: ${{ vars.PARITY_GENERIC_CI_WORKFLOW_ID }}'
require_text "${PROTECTED_WORKFLOW}" 'EXPECTED_REVIEW_WORKFLOW_ID: ${{ vars.PARITY_REVIEW_SIGNAL_WORKFLOW_ID }}'
require_text "${PROTECTED_WORKFLOW}" 'CHECK_TOKEN: ${{ steps.verdict-token.outputs.token }}'
require_text "${PROTECTED_WORKFLOW}" 'bash protected-controller/scripts/parity-protected-controller.sh'
require_text "${PROTECTED_WORKFLOW}" 'event "${GITHUB_EVENT_PATH}"'
require_text "${PROTECTED_WORKFLOW}" 'Reconcile open PRs and active merge groups'
require_text "${PROTECTED_WORKFLOW}" 'parity-protected-controller.sh reconcile'
require_text "${PROTECTED_WORKFLOW}" 'pull-requests: read'
require_text "${PROTECTED_WORKFLOW}" 'actions: read'
require_text "${PROTECTED_WORKFLOW}" 'group: parity-promotion-controller'
require_text "${PROTECTED_WORKFLOW}" 'cancel-in-progress: false'
require_text "${PROTECTED_WORKFLOW}" 'environment: parity-verdict'
require_text "${PROTECTED_WORKFLOW}" 'actions/create-github-app-token@fee1f7d63c2ff003460e3d139729b119787bc349'
require_text "${PROTECTED_WORKFLOW}" 'permission-checks: write'
require_text "${PROTECTED_WORKFLOW}" 'app-id: ${{ vars.PARITY_VERDICT_APP_ID }}'
require_text "${PROTECTED_WORKFLOW}" 'private-key: ${{ secrets.PARITY_VERDICT_APP_PRIVATE_KEY }}'
require_text "${PROTECTED_WORKFLOW}" 'EXPECTED_APP_OWNER: ${{ vars.PARITY_VERDICT_APP_OWNER }}'
if rg -n 'permission-statuses:|statuses: write' "${PROTECTED_WORKFLOW}"; then
  printf 'parity verdict App unnecessarily receives status-write authority\n' >&2
  exit 1
fi
require_text "${PARITY_DOC}" 'organization-level required-workflow rule is therefore'
require_text "${PARITY_DOC}" 'Commit statuses: write'
require_text "${PARITY_DOC}" 'not required and must remain disabled'
require_text "${PARITY_DOC}" 'requires zero same-App, same-name legacy checks'
require_text "${PARITY_DOC}" 'Before activation, a live App canary is a hard blocker'
require_text "${PARITY_DOC}" 'Neither deployment is active'

require_text "${PROTECTED_CONTROLLER}" 'workflow_blob_oid'
require_text "${PROTECTED_CONTROLLER}" 'ls-tree -z --full-tree'
require_text "${PROTECTED_CONTROLLER}" 'workflow tree entry is not exact 100644 blob'
require_text "${PROTECTED_CONTROLLER}" 'require_workflow_blob_match'
require_text "${PROTECTED_CONTROLLER}" 'workflow blob differs from protected revision'
require_text "${PROTECTED_CONTROLLER}" '.github/workflows/ci.yml'
require_text "${PROTECTED_CONTROLLER}" '.github/workflows/parity-review-signal.yml'
require_text "${PROTECTED_CONTROLLER}" 'verify_source_metadata'
require_text "${PROTECTED_CONTROLLER}" '.status == "completed"'
if rg -n '\.conclusion == "success"|RUN_CONCLUSION|SOURCE_ACCEPTED' \
  "${PROTECTED_CONTROLLER}" "${PROTECTED_WORKFLOW}"; then
  printf 'source workflow conclusion still authorizes protected parity\n' >&2
  exit 1
fi
require_text "${PROTECTED_CONTROLLER}" 'merge-base --is-ancestor'
require_text "${PROTECTED_CONTROLLER}" 'parity-protected-change-policy.sh'
require_text "${PROTECTED_CONTROLLER}" 'parity-signed-evidence.py" gate'
require_text "${PROTECTED_CONTROLLER}" 'derive-promotion-manifest'
require_text "${PROTECTED_CONTROLLER}" 'parity-promotion-projections.sh'
require_text "${PROTECTED_CONTROLLER}" 'pull request reviews changed during approval revalidation'
require_text "${PROTECTED_CONTROLLER}" 'ls-remote --heads'
require_text "${PROTECTED_CONTROLLER}" 'pulls?state=open&per_page=100'
require_text "${PROTECTED_CONTROLLER}" 'upsert_pending_check'
require_text "${PROTECTED_CONTROLLER}" 'fe2o3-parity-v1:${head_sha}'
require_text "${PROTECTED_CONTROLLER}" 'status:"in_progress"'
require_text "${PROTECTED_CONTROLLER}" '.status == "in_progress"'
require_text "${PROTECTED_CONTROLLER}" 'pending check response identity mismatch'
require_text "${PROTECTED_CONTROLLER}" "jq 'del(.head_sha)'"
require_text "${PROTECTED_CONTROLLER}" 'process-target "${event_kind}"'
require_text "${PROTECTED_CONTROLLER}" 'process_target pull-request'
require_text "${PROTECTED_CONTROLLER}" 'process_target merge-group'
require_text "${PROTECTED_CONTROLLER}" 'complete_check "${check_id}" "${target_head_sha}" failure'
require_text "${PROTECTED_CONTROLLER}" 'commits/${head_sha}/check-suites'
require_text "${PROTECTED_CONTROLLER}" 'check-suites/${suite_id}/check-runs'
require_text "${PROTECTED_CONTROLLER}" 'suite-ids "${suite_pages}"'
require_text "${CHECK_RECONCILE}" 'legacy App check blocks deterministic-check activation'
require_text "${CHECK_RECONCILE}" 'multiple deterministic App checks block reconciliation'
require_text "${CHECK_RECONCILE}" 'check-suite inventory reached the GitHub server cap'
require_text "${CHECK_RECONCILE}" 'check-run inventory reached the GitHub server cap'
require_text "${CHECK_RECONCILE}" 'invalid page boundaries'
require_text "${CHECK_RECONCILE}" 'MAX_SUITE_BYTES=4194304'
require_text "${CHECK_RECONCILE}" 'MAX_RUN_BYTES=67108864'
if rg -n 'commits/\$\{head_sha\}/check-runs' "${PROTECTED_CONTROLLER}"; then
  printf 'check reconciliation still trusts commit check-run aggregation\n' >&2
  exit 1
fi
if rg -n 'max_by\(\.id\)|sort_by\(\.id\).*last' \
  "${PROTECTED_CONTROLLER}" "${CHECK_RECONCILE}"; then
  printf 'check reconciliation assumes check ID ordering\n' >&2
  exit 1
fi
pending_line="$(rg -n 'check_id="\$\(upsert_pending_check' \
  "${PROTECTED_CONTROLLER}" | cut -d: -f1)"
verify_line="$(rg -n 'bash "\$\{BASH_SOURCE\[0\]\}" verify-target' \
  "${PROTECTED_CONTROLLER}" | cut -d: -f1)"
[[ "${pending_line}" =~ ^[0-9]+$ && "${verify_line}" =~ ^[0-9]+$ &&
  "${pending_line}" -lt "${verify_line}" ]] || {
  printf 'target check is not pending before protected/source verification\n' >&2
  exit 1
}
require_text "${CHANGE_POLICY}" 'git -C "${REVISION_REPOSITORY}" diff --no-ext-diff --no-renames'
require_text "${CHANGE_POLICY}" '--name-only -z "${BASE_SHA}" "${CANDIDATE_HEAD}"'
require_text "${CHANGE_POLICY}" 'immutable_path_changed'
require_text "${CHANGE_POLICY}" 'diff --quiet --no-ext-diff --no-renames'
require_text "${CHANGE_POLICY}" 'revision worktree does not match declared commit'
require_text "${CHANGE_POLICY}" 'merge-group head does not descend from its base'
require_text "${CHANGE_POLICY}" 'designated reviewer is not CODEOWNER for changed trust path'
require_text "${CHANGE_POLICY}" 'notification workflow changes require an administrator migration'

if rg -n '^  pull_request_review:' "${PROTECTED_WORKFLOW}"; then
  printf 'privileged protected workflow listens directly for review events\n' >&2
  exit 1
fi
require_text "${REVIEW_WORKFLOW}" 'name: Parity review signal'
require_text "${REVIEW_WORKFLOW}" 'pull_request_review:'
require_text "${REVIEW_WORKFLOW}" 'types: [submitted, edited, dismissed]'
require_text "${REVIEW_WORKFLOW}" 'contents: read'
require_text "${REVIEW_WORKFLOW}" 'pull-requests: read'
require_text "${REVIEW_WORKFLOW}" 'runs-on: ubuntu-24.04'
require_text "${REVIEW_WORKFLOW}" 'cancel-in-progress: false'
if rg -n 'environment:|secrets\.|self-hosted|create-github-app-token|checks: write' \
  "${REVIEW_WORKFLOW}"; then
  printf 'review signal workflow has privileged execution authority\n' >&2
  exit 1
fi
require_text "${GENERIC_WORKFLOW}" 'permissions:'
require_text "${GENERIC_WORKFLOW}" 'contents: read'
require_text "${GENERIC_WORKFLOW}" 'runs-on: ubuntu-24.04'
if rg -n 'secrets\.|self-hosted|checks: write' "${GENERIC_WORKFLOW}"; then
  printf 'generic CI notification source has elevated or self-hosted execution\n' >&2
  exit 1
fi

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
  /docs/generated/cuda-oxide-parity-signed-promotions.tsv \
  /scripts/parity-signed-evidence.py \
  /scripts/parity-protected-change-policy.sh \
  /scripts/parity-dashboard.sh \
  /scripts/parity-matrix.sh \
  /scripts/parity-promotion-projections.sh \
  /scripts/parity-check-reconcile.sh \
  /scripts/parity-protected-controller.sh \
  /scripts/tests/hosted-parity-ci.sh \
  /scripts/tests/parity-dashboard.sh \
  /scripts/tests/parity-promotion-projections.sh \
  /scripts/tests/parity-row-evidence.sh \
  /.github/workflows/parity-promotion.yml \
  /.github/workflows/parity-review-signal.yml \
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
require_text "${GENERIC_WORKFLOW}" 'checked-out revision does not match parity head SHA'
require_text "${GENERIC_WORKFLOW}" 'merge-group workflow is not bound to its exact head SHA'
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

# A deterministic external ID selects exactly one App-owned check without check
# ID ordering. Any legacy same-App check blocks activation, including when the
# deterministic check already exists.
CHECK_NAME=fe2o3/protected-parity-promotion
CHECK_HEAD=1111111111111111111111111111111111111111
CHECK_APP_ID=424242
CHECK_APP_SLUG=fe2o3-parity-verdict
CHECK_APP_OWNER=powderluv
CHECK_EXTERNAL_ID="fe2o3-parity-v1:${CHECK_HEAD}"
SUITE_PAGES="${TEST_ROOT}/suite-pages.json"
RUN_GROUPS="${TEST_ROOT}/run-groups.json"
CHECKS_JSON="${TEST_ROOT}/checks.json"

select_check() {
  bash "${CHECK_RECONCILE}" select "$1" "${CHECK_APP_ID}" \
    "${CHECK_APP_SLUG}" "${CHECK_APP_OWNER}" "${CHECK_NAME}" \
    "${CHECK_HEAD}" "${CHECK_EXTERNAL_ID}"
}

jq -n '[{total_count:2,check_suites:[{id:11},{id:12}]}]' \
  >"${SUITE_PAGES}"
jq -n --arg name "${CHECK_NAME}" --arg head "${CHECK_HEAD}" \
  --arg external "${CHECK_EXTERNAL_ID}" --arg slug "${CHECK_APP_SLUG}" \
  --arg owner "${CHECK_APP_OWNER}" --argjson app_id "${CHECK_APP_ID}" '
  [
    {suite_id:11,pages:[{total_count:1,check_runs:[
      {id:4,name:$name,head_sha:$head,external_id:$external,
       status:"completed",conclusion:"success",check_suite:{id:11},
       app:{id:$app_id,slug:$slug,owner:{login:$owner}}}
    ]}]},
    {suite_id:12,pages:[{total_count:1,check_runs:[
      {id:9,name:$name,head_sha:$head,external_id:"candidate-spoof",
       check_suite:{id:12},app:{id:15368,slug:"github-actions",
       owner:{login:"github"}}}
    ]}]}
  ]' >"${RUN_GROUPS}"
[[ "$(bash "${CHECK_RECONCILE}" suite-ids "${SUITE_PAGES}")" == \
  $'11\n12' ]]
bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" "${RUN_GROUPS}" \
  >"${CHECKS_JSON}"
[[ "$(select_check "${CHECKS_JSON}")" == $'update\t4' ]]
jq '.check_runs[0].status = "in_progress" | .check_runs[0].conclusion = null' \
  "${CHECKS_JSON}" >"${TEST_ROOT}/interrupted-check.json"
[[ "$(select_check "${TEST_ROOT}/interrupted-check.json")" == $'update\t4' ]]
jq '.check_runs[0].conclusion = "failure"' "${CHECKS_JSON}" \
  >"${TEST_ROOT}/dismissed-check.json"
[[ "$(select_check "${TEST_ROOT}/dismissed-check.json")" == $'update\t4' ]]

jq '
  .total_count = 1 |
  .check_runs = [.check_runs[0] | .id = 3 | .external_id = "legacy-run-id"]
' "${CHECKS_JSON}" >"${TEST_ROOT}/legacy-check.json"
expect_failure legacy_check 'legacy App check blocks deterministic-check activation' \
  select_check "${TEST_ROOT}/legacy-check.json"
jq '.total_count = 2 | .check_runs += [.check_runs[0] |
  .id = 6 | .external_id = "second-legacy-run-id"]' \
  "${TEST_ROOT}/legacy-check.json" >"${TEST_ROOT}/multiple-legacy.json"
expect_failure multiple_legacy_checks \
  'legacy App check blocks deterministic-check activation' \
  select_check "${TEST_ROOT}/multiple-legacy.json"
jq '.total_count = 3 | .check_runs += [.check_runs[0] |
  .id = 5 | .external_id = "legacy-run-id"]' \
  "${CHECKS_JSON}" >"${TEST_ROOT}/dedicated-and-legacy.json"
expect_failure dedicated_and_legacy \
  'legacy App check blocks deterministic-check activation' \
  select_check "${TEST_ROOT}/dedicated-and-legacy.json"
jq '.total_count = 3 | .check_runs += [.check_runs[0] | .id = 5]' \
  "${CHECKS_JSON}" >"${TEST_ROOT}/duplicate-dedicated.json"
expect_failure duplicate_dedicated_check \
  'multiple deterministic App checks block reconciliation' \
  select_check "${TEST_ROOT}/duplicate-dedicated.json"
jq '.total_count = 1' "${CHECKS_JSON}" >"${TEST_ROOT}/truncated-checks.json"
expect_failure truncated_check_inventory 'check-run inventory is malformed' \
  select_check "${TEST_ROOT}/truncated-checks.json"
jq '.check_runs[0].app.owner.login = "other-owner"' "${CHECKS_JSON}" \
  >"${TEST_ROOT}/wrong-owner.json"
expect_failure wrong_app_owner 'App owner does not match configured owner' \
  select_check "${TEST_ROOT}/wrong-owner.json"

printf '[{"total_count":0,"check_suites":[]}]\n' \
  >"${TEST_ROOT}/empty-suite-pages.json"
printf '[]\n' >"${TEST_ROOT}/empty-run-groups.json"
bash "${CHECK_RECONCILE}" inventory "${TEST_ROOT}/empty-suite-pages.json" \
  "${TEST_ROOT}/empty-run-groups.json" >"${TEST_ROOT}/empty-checks.json"
[[ "$(select_check "${TEST_ROOT}/empty-checks.json")" == create ]]

jq -n '[
  {total_count:101,check_suites:[range(1;101) | {id:.}]},
  {total_count:101,check_suites:[{id:101}]}
]' >"${TEST_ROOT}/paged-suites.json"
bash "${CHECK_RECONCILE}" suite-ids "${TEST_ROOT}/paged-suites.json" \
  >"${TEST_ROOT}/paged-suite-ids.txt"
[[ "$(wc -l <"${TEST_ROOT}/paged-suite-ids.txt")" == 101 ]]
jq -n '[
  {suite_id:11,pages:[
    {total_count:101,check_runs:[
      range(1;101) | {id:(1000 + .),check_suite:{id:11}}
    ]},
    {total_count:101,check_runs:[{id:1101,check_suite:{id:11}}]}
  ]},
  {suite_id:12,pages:[
    {total_count:1,check_runs:[{id:1201,check_suite:{id:12}}]}
  ]}
]' >"${TEST_ROOT}/paged-runs.json"
bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" \
  "${TEST_ROOT}/paged-runs.json" >"${TEST_ROOT}/paged-checks.json"
jq -e '.total_count == 102 and (.check_runs | length) == 102' \
  "${TEST_ROOT}/paged-checks.json" >/dev/null

jq '.[0].total_count = 3' "${SUITE_PAGES}" \
  >"${TEST_ROOT}/hidden-suite.json"
expect_failure hidden_suite 'check-suite pagination is truncated' \
  bash "${CHECK_RECONCILE}" suite-ids "${TEST_ROOT}/hidden-suite.json"
jq '.[].total_count = 1000' "${SUITE_PAGES}" \
  >"${TEST_ROOT}/capped-suite.json"
expect_failure capped_suite 'check-suite inventory reached the GitHub server cap' \
  bash "${CHECK_RECONCILE}" suite-ids "${TEST_ROOT}/capped-suite.json"
jq '.[0].check_suites[1].id = 11' "${SUITE_PAGES}" \
  >"${TEST_ROOT}/duplicate-suite.json"
expect_failure duplicate_suite 'duplicate IDs' \
  bash "${CHECK_RECONCILE}" suite-ids "${TEST_ROOT}/duplicate-suite.json"
jq '.[0].check_suites[1].id = 11.5' "${SUITE_PAGES}" \
  >"${TEST_ROOT}/fractional-suite.json"
expect_failure fractional_suite 'duplicate IDs' \
  bash "${CHECK_RECONCILE}" suite-ids "${TEST_ROOT}/fractional-suite.json"
jq -n '[range(0;11) | {total_count:0,check_suites:[]}]' \
  >"${TEST_ROOT}/too-many-suite-pages.json"
expect_failure too_many_suite_pages 'check-suite pagination shape/count is malformed' \
  bash "${CHECK_RECONCILE}" suite-ids \
  "${TEST_ROOT}/too-many-suite-pages.json"
jq '.[0].pages[0].total_count = 2' "${RUN_GROUPS}" \
  >"${TEST_ROOT}/hidden-run.json"
expect_failure hidden_run 'check-run inventory is truncated' \
  bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" \
  "${TEST_ROOT}/hidden-run.json"
jq '.[0].pages[0].total_count = 1000' "${RUN_GROUPS}" \
  >"${TEST_ROOT}/capped-run.json"
expect_failure capped_run 'check-run inventory reached the GitHub server cap' \
  bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" \
  "${TEST_ROOT}/capped-run.json"
jq '.[0].pages = [range(0;11) | {total_count:0,check_runs:[]}]' \
  "${RUN_GROUPS}" >"${TEST_ROOT}/too-many-run-pages.json"
expect_failure too_many_run_pages 'check-run pagination shape/count is malformed' \
  bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" \
  "${TEST_ROOT}/too-many-run-pages.json"
jq '.[1].pages[0].check_runs[0].id = 4' "${RUN_GROUPS}" \
  >"${TEST_ROOT}/duplicate-run.json"
expect_failure duplicate_run 'check-run IDs are duplicated across check suites' \
  bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" \
  "${TEST_ROOT}/duplicate-run.json"
jq '.[1].pages[0].check_runs[0].id = 4.5' "${RUN_GROUPS}" \
  >"${TEST_ROOT}/fractional-run.json"
expect_failure fractional_run 'check-run inventory is truncated, duplicated' \
  bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" \
  "${TEST_ROOT}/fractional-run.json"
truncate -s 67108865 "${TEST_ROOT}/oversized-runs.json"
expect_failure oversized_run_inventory 'check-run pages exceeds its size bound' \
  bash "${CHECK_RECONCILE}" inventory "${SUITE_PAGES}" \
  "${TEST_ROOT}/oversized-runs.json"

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

MERGE_SOURCE="${TEST_ROOT}/merge-source-run.json"
MERGE_HEAD=eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee
MERGE_BRANCH=gh-readonly-queue/main/pr-17-abcdef
jq -n --arg head "${MERGE_HEAD}" --arg branch "${MERGE_BRANCH}" \
  '{id:91,workflow_id:31337,event:"merge_group",
    path:".github/workflows/ci.yml",status:"completed",conclusion:"success",
    head_sha:$head,head_branch:$branch,pull_requests:[]}' >"${MERGE_SOURCE}"
bash "${PROTECTED_CONTROLLER}" test-source-metadata "${MERGE_SOURCE}" 91 31337 \
  .github/workflows/ci.yml merge_group "${MERGE_BRANCH}" "${MERGE_HEAD}" 0
expect_failure merge_source_wrong_head 'source workflow metadata mismatch' \
  bash "${PROTECTED_CONTROLLER}" test-source-metadata "${MERGE_SOURCE}" 91 31337 \
  .github/workflows/ci.yml merge_group "${MERGE_BRANCH}" \
  ffffffffffffffffffffffffffffffffffffffff 0
expect_failure merge_source_wrong_ref 'source workflow metadata mismatch' \
  bash "${PROTECTED_CONTROLLER}" test-source-metadata "${MERGE_SOURCE}" 91 31337 \
  .github/workflows/ci.yml merge_group gh-readonly-queue/main/pr-18-fedcba \
  "${MERGE_HEAD}" 0
expect_failure merge_source_wrong_id 'source workflow metadata mismatch' \
  bash "${PROTECTED_CONTROLLER}" test-source-metadata "${MERGE_SOURCE}" 91 31338 \
  .github/workflows/ci.yml merge_group "${MERGE_BRANCH}" "${MERGE_HEAD}" 0
jq '.path = ".github/workflows/spoof.yml"' "${MERGE_SOURCE}" \
  >"${TEST_ROOT}/merge-source-spoof.json"
expect_failure merge_source_wrong_path 'source workflow metadata mismatch' \
  bash "${PROTECTED_CONTROLLER}" test-source-metadata \
  "${TEST_ROOT}/merge-source-spoof.json" 91 31337 .github/workflows/ci.yml \
  merge_group "${MERGE_BRANCH}" "${MERGE_HEAD}" 0
for conclusion in success failure cancelled skipped neutral timed_out action_required stale; do
  jq --arg conclusion "${conclusion}" '.conclusion = $conclusion' \
    "${MERGE_SOURCE}" >"${TEST_ROOT}/source-${conclusion}.json"
  bash "${PROTECTED_CONTROLLER}" test-source-metadata \
    "${TEST_ROOT}/source-${conclusion}.json" 91 31337 .github/workflows/ci.yml \
    merge_group "${MERGE_BRANCH}" "${MERGE_HEAD}" 0
done
jq '.status = "in_progress"' "${MERGE_SOURCE}" \
  >"${TEST_ROOT}/source-in-progress.json"
expect_failure source_not_completed 'source workflow metadata mismatch' \
  bash "${PROTECTED_CONTROLLER}" test-source-metadata \
  "${TEST_ROOT}/source-in-progress.json" 91 31337 .github/workflows/ci.yml \
  merge_group "${MERGE_BRANCH}" "${MERGE_HEAD}" 0

REVIEW_SOURCE="${TEST_ROOT}/review-source-run.json"
jq -n --arg head 9999999999999999999999999999999999999999 \
  '{id:91,workflow_id:41414,event:"pull_request_review",
    path:".github/workflows/parity-review-signal.yml",status:"completed",
    conclusion:"failure",head_sha:$head,head_branch:"promotion",
    pull_requests:[{number:17}]}' \
  >"${REVIEW_SOURCE}"
bash "${PROTECTED_CONTROLLER}" test-source-metadata "${REVIEW_SOURCE}" 91 41414 \
  .github/workflows/parity-review-signal.yml pull_request_review promotion \
  9999999999999999999999999999999999999999 17

# Exact source-tree workflow blobs, not workflow IDs or paths, establish source
# provenance. Trigger or runner substitutions change the blob and fail.
BLOB_REPOSITORY="${TEST_ROOT}/workflow-blobs"
git init -q "${BLOB_REPOSITORY}"
git -C "${BLOB_REPOSITORY}" config user.name 'Evidence Test'
git -C "${BLOB_REPOSITORY}" config user.email evidence@example.invalid
mkdir -p "${BLOB_REPOSITORY}/.github/workflows"
cp "${GENERIC_WORKFLOW}" "${BLOB_REPOSITORY}/.github/workflows/ci.yml"
cp "${REVIEW_WORKFLOW}" \
  "${BLOB_REPOSITORY}/.github/workflows/parity-review-signal.yml"
git -C "${BLOB_REPOSITORY}" add .github/workflows
git -C "${BLOB_REPOSITORY}" commit -qm 'protected workflow blobs'
BLOB_BASE="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
printf 'data only\n' >"${BLOB_REPOSITORY}/candidate.txt"
git -C "${BLOB_REPOSITORY}" add candidate.txt
git -C "${BLOB_REPOSITORY}" commit -qm 'unchanged notification workflows'
BLOB_UNCHANGED="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_UNCHANGED}" .github/workflows/ci.yml
git -C "${BLOB_REPOSITORY}" reset -q --hard "${BLOB_BASE}"
chmod 755 "${BLOB_REPOSITORY}/.github/workflows/ci.yml"
git -C "${BLOB_REPOSITORY}" commit -qam 'executable CI workflow'
BLOB_EXECUTABLE="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
expect_failure executable_workflow 'workflow tree entry is not exact 100644 blob' \
  bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_EXECUTABLE}" .github/workflows/ci.yml
git -C "${BLOB_REPOSITORY}" reset -q --hard "${BLOB_BASE}"
rm "${BLOB_REPOSITORY}/.github/workflows/parity-review-signal.yml"
ln -s ci.yml \
  "${BLOB_REPOSITORY}/.github/workflows/parity-review-signal.yml"
git -C "${BLOB_REPOSITORY}" add .github/workflows/parity-review-signal.yml
git -C "${BLOB_REPOSITORY}" commit -qm 'symlink review workflow'
BLOB_SYMLINK="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
expect_failure symlink_workflow 'workflow tree entry is not exact 100644 blob' \
  bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_SYMLINK}" .github/workflows/parity-review-signal.yml
git -C "${BLOB_REPOSITORY}" reset -q --hard "${BLOB_BASE}"
git -C "${BLOB_REPOSITORY}" rm -q .github/workflows/parity-review-signal.yml
git -C "${BLOB_REPOSITORY}" commit -qm 'missing review workflow'
BLOB_MISSING="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
expect_failure missing_workflow 'workflow tree entry is missing or ambiguous' \
  bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_MISSING}" .github/workflows/parity-review-signal.yml
git -C "${BLOB_REPOSITORY}" reset -q --hard "${BLOB_BASE}"
git -C "${BLOB_REPOSITORY}" mv .github/workflows/parity-review-signal.yml \
  .github/workflows/Parity-review-signal.yml
git -C "${BLOB_REPOSITORY}" commit -qm 'case-mismatched review workflow'
BLOB_CASE_MISMATCH="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
expect_failure case_mismatch_workflow 'workflow tree entry is missing or ambiguous' \
  bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_CASE_MISMATCH}" \
  .github/workflows/parity-review-signal.yml
git -C "${BLOB_REPOSITORY}" reset -q --hard "${BLOB_BASE}"
git -C "${BLOB_REPOSITORY}" update-index --add --cacheinfo \
  "160000,${BLOB_BASE},.github/workflows/ci.yml"
git -C "${BLOB_REPOSITORY}" commit -qm 'gitlink CI workflow'
BLOB_GITLINK="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
expect_failure gitlink_workflow 'workflow tree entry is not exact 100644 blob' \
  bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_GITLINK}" .github/workflows/ci.yml
git -C "${BLOB_REPOSITORY}" reset -q --hard "${BLOB_BASE}"
printf '\n  workflow_dispatch:\n' >>"${BLOB_REPOSITORY}/.github/workflows/ci.yml"
git -C "${BLOB_REPOSITORY}" commit -qam 'changed CI trigger'
BLOB_CHANGED_TRIGGER="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
expect_failure changed_ci_trigger 'workflow blob differs from protected revision' \
  bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_CHANGED_TRIGGER}" .github/workflows/ci.yml
git -C "${BLOB_REPOSITORY}" checkout -q "${BLOB_BASE}" -- \
  .github/workflows/ci.yml .github/workflows/parity-review-signal.yml
printf '\n    runs-on: self-hosted\n' \
  >>"${BLOB_REPOSITORY}/.github/workflows/parity-review-signal.yml"
git -C "${BLOB_REPOSITORY}" commit -qam 'changed review runner'
BLOB_CHANGED_RUNNER="$(git -C "${BLOB_REPOSITORY}" rev-parse HEAD)"
expect_failure changed_review_runner 'workflow blob differs from protected revision' \
  bash "${PROTECTED_CONTROLLER}" test-workflow-blob "${BLOB_REPOSITORY}" \
  "${BLOB_BASE}" "${BLOB_CHANGED_RUNNER}" \
  .github/workflows/parity-review-signal.yml

RECONCILE_PULLS="${TEST_ROOT}/reconcile-pulls.json"
RECONCILE_QUEUE="${TEST_ROOT}/reconcile-queue.tsv"
jq -n '[{number:17,head:{ref:"promotion",
  sha:"1111111111111111111111111111111111111111"}}]' >"${RECONCILE_PULLS}"
printf '%s\t%s\n' 2222222222222222222222222222222222222222 \
  refs/heads/gh-readonly-queue/main/pr-17-abcdef >"${RECONCILE_QUEUE}"
RECONCILE_TARGETS="$(bash "${PROTECTED_CONTROLLER}" \
  test-reconciliation-targets "${RECONCILE_PULLS}" "${RECONCILE_QUEUE}")"
[[ "${RECONCILE_TARGETS}" == \
  $'pull-request\t17\trefs/heads/promotion\t1111111111111111111111111111111111111111\nmerge-group\t0\trefs/heads/gh-readonly-queue/main/pr-17-abcdef\t2222222222222222222222222222222222222222' ]]
jq '.[0].head.sha = "not-a-sha"' "${RECONCILE_PULLS}" \
  >"${TEST_ROOT}/bad-reconcile-pulls.json"
expect_failure malformed_reconcile_pr 'open-PR inventory contains a malformed target' \
  bash "${PROTECTED_CONTROLLER}" test-reconciliation-targets \
  "${TEST_ROOT}/bad-reconcile-pulls.json" "${RECONCILE_QUEUE}"
printf '%s\t%s\n' bad refs/heads/gh-readonly-queue/main/pr-17-abcdef \
  >"${TEST_ROOT}/bad-reconcile-queue.tsv"
expect_failure malformed_reconcile_queue 'merge-queue inventory SHA is malformed' \
  bash "${PROTECTED_CONTROLLER}" test-reconciliation-targets \
  "${RECONCILE_PULLS}" "${TEST_ROOT}/bad-reconcile-queue.tsv"

APPROVAL_A="${TEST_ROOT}/approval-a.json"
APPROVAL_B="${TEST_ROOT}/approval-b.json"
jq -n '[{id:5,submitted_at:"2026-01-01T00:00:00Z",state:"APPROVED",
  commit_id:"1111111111111111111111111111111111111111",
  user:{login:"powderluv"}}]' >"${APPROVAL_A}"
jq '.[] .state = "DISMISSED"' "${APPROVAL_A}" >"${APPROVAL_B}"
if cmp -s -- "${APPROVAL_A}" "${APPROVAL_B}"; then
  printf 'review dismissal did not change canonical approval snapshot\n' >&2
  exit 1
fi

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
  "${SEED_REPOSITORY}/scripts" "${SEED_REPOSITORY}/.github/workflows"
printf 'status\tMissing\n' >"${SEED_REPOSITORY}/docs/cuda-oxide-parity-status.tsv"
printf 'protected matrix prose\n' >"${SEED_REPOSITORY}/docs/cuda-oxide-parity-matrix.md"
printf 'dashboard\n' >"${SEED_REPOSITORY}/docs/generated/cuda-oxide-parity-dashboard.md"
printf 'dashboard-tsv\n' >"${SEED_REPOSITORY}/docs/generated/cuda-oxide-parity-dashboard.tsv"
printf 'signed_promotion_projection_schema_version\t1\nrow_count\t0\n' \
  >"${SEED_REPOSITORY}/docs/generated/cuda-oxide-parity-signed-promotions.tsv"
printf 'protected verifier\n' >"${SEED_REPOSITORY}/scripts/parity-signed-evidence.py"
printf 'powderluv\n' >"${SEED_REPOSITORY}/.github/parity-trust-reviewers.txt"
printf '/scripts/parity-signed-evidence.py @powderluv\n/docs/parity-evidence/trusted-keys/ @powderluv\n/.github/workflows/ci.yml @powderluv\n/.github/workflows/parity-review-signal.yml @powderluv\n' \
  >"${SEED_REPOSITORY}/.github/CODEOWNERS"
printf 'name: CI\n' >"${SEED_REPOSITORY}/.github/workflows/ci.yml"
printf 'name: Parity review signal\n' \
  >"${SEED_REPOSITORY}/.github/workflows/parity-review-signal.yml"
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

chmod 755 "${CANDIDATE}/scripts/parity-signed-evidence.py"
commit_candidate 'mode-only verifier change'
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == trust-change-approved ]]
reset_candidate

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

printf 'candidate review signal\n' \
  >"${CANDIDATE}/.github/workflows/parity-review-signal.yml"
commit_candidate 'candidate review signal workflow'
expect_failure review_signal_admin_migration \
  'notification workflow changes require an administrator migration' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

printf 'candidate CI trigger\n' >"${CANDIDATE}/.github/workflows/ci.yml"
commit_candidate 'candidate CI workflow'
expect_failure ci_admin_migration \
  'notification workflow changes require an administrator migration' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
reset_candidate

chmod 755 "${CANDIDATE}/.github/workflows/ci.yml"
commit_candidate 'mode-only CI workflow'
expect_failure mode_only_ci_admin_migration \
  'notification workflow changes require an administrator migration' \
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
