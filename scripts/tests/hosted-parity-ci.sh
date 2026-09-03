#!/usr/bin/env bash
# shellcheck disable=SC2016

set -Eeuo pipefail

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly PROTECTED_WORKFLOW="${ROOT}/.github/workflows/parity-promotion.yml"
readonly PUBLISHER_WORKFLOW="${ROOT}/.github/workflows/parity-publisher-gate.yml"
readonly GENERIC_WORKFLOW="${ROOT}/.github/workflows/ci.yml"
readonly SHARD_POLICY="${ROOT}/scripts/rustc-codegen-shards.py"
readonly HARDWARE_WORKFLOW="${ROOT}/.github/workflows/hardware-smoke.yml"
readonly ROCM_WORKFLOW="${ROOT}/.github/workflows/rocm-compile.yml"
readonly ROW_SOFTMAX_WORKFLOW="${ROOT}/.github/workflows/row-softmax-v1.yml"
readonly SOURCE_ISA_UNIT_WORKFLOW="${ROOT}/.github/workflows/source-isa-unit-matrix.yml"
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
require_text "${PROTECTED_WORKFLOW}" 'path: protected'
require_text "${PROTECTED_WORKFLOW}" 'path: candidate'
require_text "${PROTECTED_WORKFLOW}" 'persist-credentials: false'
require_text "${PROTECTED_WORKFLOW}" 'uses: ./.github/workflows/parity-publisher-gate.yml'
require_text "${PROTECTED_WORKFLOW}" 'promotion classified; publisher authorization is deferred'
require_text "${PROTECTED_WORKFLOW}" 'path intentionally has no id-token'
require_text "${PROTECTED_WORKFLOW}" 'pull-requests: read'
require_text "${PROTECTED_WORKFLOW}" 'protected/scripts/parity-protected-change-policy.sh'
require_text "${PROTECTED_WORKFLOW}" 'gh api --paginate'
require_text "${PROTECTED_WORKFLOW}" 'trust-change-approved'
require_text "${PROTECTED_WORKFLOW}" '/files?per_page=100'
require_text "${PROTECTED_WORKFLOW}" 'python3 protected/scripts/parity-signed-evidence.py check-trust-update'
require_text "${PROTECTED_WORKFLOW}" '--protected-row-policy protected/docs/parity-row-evidence-policy-v2.tsv'
require_text "${PROTECTED_WORKFLOW}" '--candidate-row-policy candidate/docs/parity-row-evidence-policy-v2.tsv'
require_text "${PROTECTED_WORKFLOW}" 'pull_request_review:'
require_text "${PROTECTED_WORKFLOW}" 'types: [submitted, edited, dismissed]'
require_text "${PROTECTED_WORKFLOW}" 'BASE_REPOSITORY: ${{ github.event.pull_request.base.repo.full_name }}'
require_text "${PROTECTED_WORKFLOW}" '[[ "${BASE_REPOSITORY}" == "${GITHUB_REPOSITORY}" ]]'
require_text "${PROTECTED_WORKFLOW}" '[[ "${BASE_REF}" == "${DEFAULT_BRANCH}" ]]'
require_text "${PROTECTED_WORKFLOW}" 'git init --bare "${default_tip_root}"'
require_text "${PROTECTED_WORKFLOW}" 'fetch --no-tags --depth=1 --force origin'
require_text "${PROTECTED_WORKFLOW}" '"+refs/heads/${DEFAULT_BRANCH}:${default_remote_ref}"'
require_text "${PROTECTED_WORKFLOW}" 'if [[ "${current_default_sha}" != "${BASE_SHA}" ]]; then'
require_text "${PROTECTED_WORKFLOW}" 'pull request base SHA is not current default tip'
require_text "${PROTECTED_WORKFLOW}" 'HEAD_SHA: ${{ github.event.pull_request.head.sha }}'
require_text "${PROTECTED_WORKFLOW}" 'CHANGED_FILE_COUNT: ${{ github.event.pull_request.changed_files }}'
require_text "${PROTECTED_WORKFLOW}" 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683'
if rg -n 'parity-publisher-gate\.yml@' "${PROTECTED_WORKFLOW}"; then
  printf 'protected publisher caller is not candidate-local\n' >&2
  exit 1
fi

require_text "${PUBLISHER_WORKFLOW}" 'workflow_call:'
require_text "${PUBLISHER_WORKFLOW}" 'environment: protected-publisher'
require_text "${PUBLISHER_WORKFLOW}" 'id-token: write'
require_text "${PUBLISHER_WORKFLOW}" 'FE2O3_PUBLISHER_REPOSITORY_OWNER_ID: ${{ github.repository_owner_id }}'
require_text "${PUBLISHER_WORKFLOW}" 'FE2O3_PUBLISHER_DEFAULT_BRANCH: ${{ github.event.repository.default_branch }}'
require_text "${PUBLISHER_WORKFLOW}" 'FE2O3_PUBLISHER_SERVICE_URL: ${{ vars.FE2O3_PUBLISHER_SERVICE_URL }}'
require_text "${PUBLISHER_WORKFLOW}" 'FE2O3_PUBLISHER_SERVICE_HOST: ${{ vars.FE2O3_PUBLISHER_SERVICE_HOST }}'
require_text "${PUBLISHER_WORKFLOW}" 'FE2O3_PUBLISHER_GITHUB_ENVIRONMENT: protected-publisher'
require_text "${PUBLISHER_WORKFLOW}" 'FE2O3_PUBLISHER_OIDC_AUDIENCE: ${{ vars.FE2O3_PUBLISHER_OIDC_AUDIENCE }}'
require_text "${PUBLISHER_WORKFLOW}" 'python3 protected/scripts/parity-publisher-client.py'
require_text "${PUBLISHER_WORKFLOW}" 'python3 protected/scripts/parity-signed-evidence.py gate'
require_text "${PUBLISHER_WORKFLOW}" 'github.event.merge_group.base_sha'
require_text "${PUBLISHER_WORKFLOW}" 'github.event.merge_group.head_sha'
require_text "${PUBLISHER_WORKFLOW}" 'merge-group base SHA is not current default tip'
require_text "${PUBLISHER_WORKFLOW}" '--publisher-receipt-root "${publisher_receipt_root}"'
require_text "${PUBLISHER_WORKFLOW}" '--expected-default-tip "${BASE_SHA}"'
require_text "${PUBLISHER_WORKFLOW}" '--expected-candidate-head "${HEAD_SHA}"'
if sed -n '/^on:/,/^permissions:/p' "${PUBLISHER_WORKFLOW}" |
  rg -n '^[[:space:]]+(inputs|secrets):' >/dev/null; then
  printf 'publisher reusable workflow unexpectedly accepts caller data\n' >&2
  exit 1
fi
if sed -n '/^  verify-merge-group:/,$p' "${PROTECTED_WORKFLOW}" |
  rg -n '^[[:space:]]+(with|secrets):' >/dev/null; then
  printf 'publisher caller unexpectedly forwards caller-controlled data\n' >&2
  exit 1
fi

for path in \
  docs/parity-evidence/trust-policy-v2.tsv \
  docs/parity-evidence/trusted-keys/** \
  scripts/parity-publisher-client.py \
  scripts/parity-signed-evidence.py \
  scripts/parity-protected-change-policy.sh \
  scripts/parity-repository-rules.py \
  scripts/parity-repository-rules.sh \
  .github/workflows/ci.yml \
  .github/workflows/parity-promotion.yml \
  .github/workflows/parity-publisher-gate.yml \
  .github/workflows/source-isa-unit-matrix.yml \
  .github/CODEOWNERS \
  .github/parity-trust-reviewers.txt; do
  require_text "${PROTECTED_WORKFLOW}" "- ${path}"
done
for ownership in \
  /docs/parity-row-evidence-policy-v2.tsv \
  /docs/parity-evidence/trust-policy-v2.tsv \
  /docs/parity-evidence/trusted-keys/ \
  /scripts/parity-publisher-client.py \
  /scripts/parity-signed-evidence.py \
  /scripts/parity-protected-change-policy.sh \
  /scripts/parity-repository-rules.py \
  /scripts/parity-repository-rules.sh \
  /.github/workflows/parity-promotion.yml \
  /.github/workflows/parity-publisher-gate.yml \
  /.github/workflows/source-isa-unit-matrix.yml \
  /.github/CODEOWNERS; do
  require_text "${CODEOWNERS}" "${ownership} @powderluv"
done


require_text "${GENERIC_WORKFLOW}" 'direct default-branch parity promotion is forbidden; repository rules must prevent this push'
require_text "${GENERIC_WORKFLOW}" 'name: Generic parity policy gate'
require_text "${GENERIC_WORKFLOW}" 'name: Generic validation'
require_text "${GENERIC_WORKFLOW}" 'name: Generic core'
require_text "${GENERIC_WORKFLOW}" 'name: Rustc codegen shard (${{ matrix.shard }})'
if ! sed -n '/^  parity-policy:/,/^  generic-core:/p' "${GENERIC_WORKFLOW}" |
  rg -Fx '    timeout-minutes: 10' >/dev/null; then
  printf 'generic parity policy timeout is not exactly 10 minutes\n' >&2
  exit 1
fi
if ! sed -n '/^  generic-core:/,/^  rustc-codegen-shards:/p' "${GENERIC_WORKFLOW}" |
  rg -Fx '    timeout-minutes: 90' >/dev/null; then
  printf 'generic core timeout is not exactly 90 minutes\n' >&2
  exit 1
fi
if ! sed -n '/^  rustc-codegen-shards:/,/^  generic-validation:/p' "${GENERIC_WORKFLOW}" |
  rg -Fx '    timeout-minutes: 90' >/dev/null; then
  printf 'generic codegen shard timeout is not exactly 90 minutes\n' >&2
  exit 1
fi
if ! sed -n '/^  generic-validation:/,$p' "${GENERIC_WORKFLOW}" |
  rg -Fx '    timeout-minutes: 10' >/dev/null; then
  printf 'generic aggregate timeout is not exactly 10 minutes\n' >&2
  exit 1
fi
require_text "${GENERIC_WORKFLOW}" 'run: scripts/ci-local.sh generic-core'
if ! sed -n '/^  generic-core:/,/^  rustc-codegen-shards:/p' "${GENERIC_WORKFLOW}" |
  rg -Fx '          sudo apt-get install --yes llvm ripgrep' >/dev/null; then
  printf 'generic core must install the llvm-as provider and ripgrep\n' >&2
  exit 1
fi
require_text "${GENERIC_WORKFLOW}" 'run: scripts/ci-local.sh rustc-codegen-shard "${{ matrix.shard }}"'
require_text "${GENERIC_WORKFLOW}" 'fail-fast: false'
require_text "${GENERIC_WORKFLOW}" 'CARGO_TARGET_DIR: ${{ github.workspace }}/target/ci/generic-core'
require_text "${GENERIC_WORKFLOW}" 'CI_LOG_DIR: ${{ github.workspace }}/target/ci-logs/generic-core'
require_text "${GENERIC_WORKFLOW}" 'CARGO_TARGET_DIR: ${{ github.workspace }}/target/ci/rustc-codegen-${{ matrix.shard }}'
require_text "${GENERIC_WORKFLOW}" 'CI_LOG_DIR: ${{ github.workspace }}/target/ci-logs/rustc-codegen-${{ matrix.shard }}'
require_text "${GENERIC_WORKFLOW}" 'name: generic-core-logs-${{ github.run_attempt }}'
require_text "${GENERIC_WORKFLOW}" 'name: rustc-codegen-${{ matrix.shard }}-logs-${{ github.run_attempt }}'
if [[ "$(rg -Fc 'if: ${{ failure() || cancelled() }}' "${GENERIC_WORKFLOW}")" -ne 2 ]]; then
  printf 'generic core and shards do not both upload logs after failure or cancellation\n' >&2
  exit 1
fi
require_text "${GENERIC_WORKFLOW}" 'if: ${{ always() }}'
require_text "${GENERIC_WORKFLOW}" '      - generic-core'
require_text "${GENERIC_WORKFLOW}" '      - rustc-codegen-shards'
require_text "${GENERIC_WORKFLOW}" 'GENERIC_CORE_RESULT: ${{ needs.generic-core.result }}'
require_text "${GENERIC_WORKFLOW}" 'RUSTC_CODEGEN_SHARDS_RESULT: ${{ needs.rustc-codegen-shards.result }}'
require_text "${GENERIC_WORKFLOW}" 'scripts/require-ci-success.sh'

workflow_shards="$(
  sed -n '/^  rustc-codegen-shards:/,/^  generic-validation:/p' "${GENERIC_WORKFLOW}" |
    sed -n '/^        shard:/,/^    env:/p' |
    sed -n 's/^          - //p'
)"
manifest_shards="$(python3 "${SHARD_POLICY}" list)"
if [[ "${workflow_shards}" != "${manifest_shards}" ]]; then
  printf '%s\n' 'generic workflow matrix differs from the checked-in shard manifest' >&2
  diff -u <(printf '%s\n' "${manifest_shards}") \
    <(printf '%s\n' "${workflow_shards}") >&2 || true
  exit 1
fi
require_text "${GENERIC_WORKFLOW}" 'merge_group:'
require_text "${GENERIC_WORKFLOW}" 'MERGE_BASE_SHA: ${{ github.event.merge_group.base_sha }}'
require_text "${GENERIC_WORKFLOW}" 'MERGE_HEAD_SHA: ${{ github.event.merge_group.head_sha }}'
require_text "${GENERIC_WORKFLOW}" 'authorized only by the protected merge-group reusable workflow'
require_text "${GENERIC_WORKFLOW}" 'EVENT_NAME: ${{ github.event_name }}'
require_text "${GENERIC_WORKFLOW}" 'PUSH_BEFORE_SHA: ${{ github.event.before }}'
require_text "${GENERIC_WORKFLOW}" 'EVENT_HEAD_SHA: ${{ github.sha }}'
require_text "${GENERIC_WORKFLOW}" 'REF_NAME: ${{ github.ref_name }}'
require_text "${GENERIC_WORKFLOW}" 'git fetch --no-tags --prune --force origin'
require_text "${GENERIC_WORKFLOW}" '"+refs/heads/${DEFAULT_BRANCH}:${default_remote_ref}"'
require_text "${GENERIC_WORKFLOW}" 'git merge-base --is-ancestor "${BASE_SHA}" "${HEAD_SHA}"'
require_text "${GENERIC_WORKFLOW}" 'non-default branch head does not contain current protected default tip'
require_text "${GENERIC_WORKFLOW}" 'git diff --quiet "${BASE_SHA}" "${HEAD_SHA}"'
require_text "${GENERIC_WORKFLOW}" 'missing or zero parity base SHA'
require_text "${GENERIC_WORKFLOW}" 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683'
require_text "${HARDWARE_WORKFLOW}" 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683'
require_text "${ROCM_WORKFLOW}" 'actions/checkout@11bd71901bbe5b1630ceea73d27597364c9af683'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'workflow_dispatch:'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" "github.repository == 'harsh-nod/fe2o3'"
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" "github.ref == 'refs/heads/main'"
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'runs-on: [self-hosted, linux, x64, fe2o3-source-isa-protected-v1]'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" "FE2O3_RUN_SOURCE_ISA_UNIT_MATRIX: '1'"
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'run: scripts/ci-local.sh source-isa-unit-matrix'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'confirm_characteristic_contract_v2:'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'name: Validate lossless characteristic V2 contract and adapter'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" "FE2O3_RUN_SOURCE_ISA_CHARACTERISTIC_CONTRACT_V2: '1'"
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'run: scripts/ci-local.sh source-isa-characteristic-contract-v2'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'confirm_characteristic_matrix_v2:'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'name: Observe exact characteristics for three ordinary units on gfx942 and gfx950'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" "FE2O3_RUN_SOURCE_ISA_CHARACTERISTIC_MATRIX_V2: '1'"
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'run: scripts/ci-local.sh source-isa-characteristic-matrix-v2'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'timeout-minutes: 120'
require_text "${SOURCE_ISA_UNIT_WORKFLOW}" 'persist-credentials: false'
if sed -n '/^on:/,/^permissions:/p' "${SOURCE_ISA_UNIT_WORKFLOW}" |
  rg -n '^[[:space:]]+(push|pull_request|pull_request_target|merge_group|schedule|workflow_call):' \
    >/dev/null; then
  printf 'protected source/ISA unit workflow is not workflow_dispatch-only\n' >&2
  exit 1
fi
if rg -n 'source-isa-(unit-matrix|characteristic-(contract|matrix)-v2)|FE2O3_RUN_SOURCE_ISA_(UNIT_MATRIX|CHARACTERISTIC_(CONTRACT|MATRIX)_V2)' "${GENERIC_WORKFLOW}"; then
  printf 'generic hosted CI unexpectedly schedules a protected source/ISA contract or matrix\n' >&2
  exit 1
fi
if ! sed -n '/^  host-contract:/,$p' "${ROW_SOFTMAX_WORKFLOW}" \
  | grep -F -- 'fetch-depth: 0' >/dev/null; then
  printf 'row-softmax host contract checkout lacks complete lineage history\n' >&2
  exit 1
fi
if rg -n 'git rev-parse HEAD\^' "${GENERIC_WORKFLOW}"; then
  printf 'generic parity CI does not cover the complete push range\n' >&2
  exit 1
fi
if rg -n 'BASE_SHA="\$\(git merge-base' "${GENERIC_WORKFLOW}"; then
  printf 'generic parity CI trusts a historical merge base\n' >&2
  exit 1
fi
if rg -n 'actions/checkout@(v[0-9]+|main|master)' \
  "${GENERIC_WORKFLOW}" "${PROTECTED_WORKFLOW}" "${PUBLISHER_WORKFLOW}" \
  "${HARDWARE_WORKFLOW}" "${ROCM_WORKFLOW}" "${ROW_SOFTMAX_WORKFLOW}" \
  "${SOURCE_ISA_UNIT_WORKFLOW}"; then
  printf 'hosted CI uses a mutable checkout action reference\n' >&2
  exit 1
fi


if rg -n 'python3 candidate/|candidate/scripts/|scripts/parity-row-evidence.sh gate' \
  "${PROTECTED_WORKFLOW}" "${PUBLISHER_WORKFLOW}"; then
  printf 'protected parity CI invokes candidate executable content\n' >&2
  exit 1
fi
if rg -n 'id-token: write|parity-publisher-client|PUBLISHER_SERVICE' \
  "${GENERIC_WORKFLOW}"; then
  printf 'candidate-controlled generic workflow has publisher authority\n' >&2
  exit 1
fi
if [[ "$(rg -n 'id-token: write' \
  "${PROTECTED_WORKFLOW}" "${PUBLISHER_WORKFLOW}" "${GENERIC_WORKFLOW}" |
  wc -l)" -ne 2 ]]; then
  printf 'publisher OIDC permission is not limited to caller and called jobs\n' >&2
  exit 1
fi
if sed -n '/^  verify:/,/^  verify-merge-group:/p' "${PROTECTED_WORKFLOW}" |
  rg -n 'id-token: write|parity-publisher-client|PUBLISHER_SERVICE'; then
  printf 'pull-request review path has publisher authority\n' >&2
  exit 1
fi
if rg -n 'python3 candidate/scripts/parity-publisher-client.py' \
  "${GENERIC_WORKFLOW}" "${PROTECTED_WORKFLOW}" "${PUBLISHER_WORKFLOW}"; then
  printf 'hosted parity CI invokes a candidate-owned publisher client\n' >&2
  exit 1
fi
if rg -n -- '--publisher-receipt-root[[:space:]]+candidate' \
  "${GENERIC_WORKFLOW}" "${PROTECTED_WORKFLOW}" "${PUBLISHER_WORKFLOW}"; then
  printf 'hosted parity CI accepts a candidate-owned publisher receipt\n' >&2
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

# Exercise the protected classifier without executing candidate content.
PROTECTED="${TEST_ROOT}/protected"
CANDIDATE="${TEST_ROOT}/candidate"
REVIEWS="${TEST_ROOT}/reviews.json"
FILES="${TEST_ROOT}/files.json"
HEAD_SHA=1111111111111111111111111111111111111111
readonly FILES HEAD_SHA
mkdir -p \
  "${PROTECTED}/docs" "${PROTECTED}/scripts" "${PROTECTED}/.github" \
  "${CANDIDATE}/docs" "${CANDIDATE}/scripts" "${CANDIDATE}/.github"
printf 'status\tMissing\n' >"${PROTECTED}/docs/cuda-oxide-parity-status.tsv"
cp "${PROTECTED}/docs/cuda-oxide-parity-status.tsv" \
  "${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
printf 'protected verifier\n' >"${PROTECTED}/scripts/parity-signed-evidence.py"
cp "${PROTECTED}/scripts/parity-signed-evidence.py" \
  "${CANDIDATE}/scripts/parity-signed-evidence.py"
printf 'powderluv\n' >"${PROTECTED}/.github/parity-trust-reviewers.txt"
cp "${PROTECTED}/.github/parity-trust-reviewers.txt" \
  "${CANDIDATE}/.github/parity-trust-reviewers.txt"
jq -n --arg head "${HEAD_SHA}" '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}}]' >"${REVIEWS}"
printf '%s\n' '[{"filename":"scripts/parity-signed-evidence.py"}]' >"${FILES}"

policy_args=(
  "${PROTECTED}"
  "${CANDIDATE}"
  docs/cuda-oxide-parity-status.tsv
  "${REVIEWS}"
  "${PROTECTED}/.github/parity-trust-reviewers.txt"
  "${FILES}"
  1
  "${HEAD_SHA}"
)
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == no-op ]]

printf 'status\tPartial\n' >"${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == promotion ]]
cp "${PROTECTED}/docs/cuda-oxide-parity-status.tsv" \
  "${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"

printf 'candidate verifier\n' >"${CANDIDATE}/scripts/parity-signed-evidence.py"
printf '[]\n' >"${REVIEWS}"
expect_failure missing_designated_review 'designated reviewer has no review' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
jq -n --arg head "${HEAD_SHA}" '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}}]' >"${REVIEWS}"
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == trust-change-approved ]]

jq -n '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":"2222222222222222222222222222222222222222","user":{"login":"powderluv"}}]' >"${REVIEWS}"
expect_failure stale_head_approval 'approval does not bind candidate head' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
jq -n --arg head "${HEAD_SHA}" '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}}]' >"${REVIEWS}"

printf '%s\n' '[{"filename":"scripts/parity-signed-evidence.py"},{"filename":"README.md"}]' >"${FILES}"
policy_args[6]=2
expect_failure arbitrary_mixed_path 'trust changes must be isolated from arbitrary paths: README.md' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
printf '%s\n' '[{"filename":"scripts/parity-signed-evidence.py"}]' >"${FILES}"
policy_args[6]=2
expect_failure changed_file_count 'changed-file count does not match pull request' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
policy_args[6]=1
printf '[]\n' >"${FILES}"
expect_failure empty_changed_paths 'changed-file data is malformed or empty' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
printf '%s\n' '[{"filename":"scripts/parity-signed-evidence.py"}]' >"${FILES}"

jq -n --arg head "${HEAD_SHA}" '[{"id":1,"submitted_at":"2026-01-01T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}},{"id":2,"submitted_at":"2026-01-02T00:00:00Z","state":"CHANGES_REQUESTED","commit_id":$head,"user":{"login":"powderluv"}}]' >"${REVIEWS}"
expect_failure latest_review_state 'latest state is not APPROVED' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
jq -n --arg head "${HEAD_SHA}" '[{"id":3,"submitted_at":"2026-01-03T00:00:00Z","state":"APPROVED","commit_id":$head,"user":{"login":"powderluv"}}]' >"${REVIEWS}"
printf 'status\tPartial\n' >"${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"
expect_failure mixed_trust_status 'trust changes must not include parity status changes' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
cp "${PROTECTED}/docs/cuda-oxide-parity-status.tsv" \
  "${CANDIDATE}/docs/cuda-oxide-parity-status.tsv"

cp "${PROTECTED}/scripts/parity-signed-evidence.py" \
  "${CANDIDATE}/scripts/parity-signed-evidence.py"
mkdir -p \
  "${PROTECTED}/docs/parity-evidence/trusted-keys" \
  "${CANDIDATE}/docs/parity-evidence/trusted-keys"
printf 'key one\n' >"${PROTECTED}/docs/parity-evidence/trusted-keys/runner.pem"
printf 'key two\n' >"${CANDIDATE}/docs/parity-evidence/trusted-keys/runner.pem"
[[ "$(bash "${CHANGE_POLICY}" "${policy_args[@]}")" == trust-change-approved ]]

cp "${PROTECTED}/docs/parity-evidence/trusted-keys/runner.pem" \
  "${CANDIDATE}/docs/parity-evidence/trusted-keys/runner.pem"
rm "${CANDIDATE}/scripts/parity-signed-evidence.py"
ln -s ../docs/cuda-oxide-parity-status.tsv \
  "${CANDIDATE}/scripts/parity-signed-evidence.py"
expect_failure candidate_trust_symlink 'candidate trust path is not a regular file' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"
rm "${CANDIDATE}/scripts/parity-signed-evidence.py"
printf 'candidate verifier\n' >"${CANDIDATE}/scripts/parity-signed-evidence.py"
printf 'powderluv\nPOWDERLUV\n' >"${PROTECTED}/.github/parity-trust-reviewers.txt"
cp "${PROTECTED}/.github/parity-trust-reviewers.txt" \
  "${CANDIDATE}/.github/parity-trust-reviewers.txt"
expect_failure duplicate_designated_reviewer 'duplicate protected reviewer identity' \
  bash "${CHANGE_POLICY}" "${policy_args[@]}"


bash -n "${BASH_SOURCE[0]}"
shellcheck "${BASH_SOURCE[0]}"
printf 'hosted parity CI trust-boundary tests passed\n'
