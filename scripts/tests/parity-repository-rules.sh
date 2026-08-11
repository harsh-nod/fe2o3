#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly TOOL="${ROOT}/scripts/parity-repository-rules.py"
readonly WRAPPER="${ROOT}/scripts/parity-repository-rules.sh"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'repository-rule negative test unexpectedly passed: %s\n' "${name}" >&2
    exit 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'repository-rule negative test produced wrong diagnostic: %s\n' \
      "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    exit 1
  }
}

common=(
  --repository-id 4242
  --actions-integration-id 15368
  --default-branch main
)
python3 "${TOOL}" render "${common[@]}" >"${TEST_ROOT}/rules.json"
python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/rules.json"
bash "${WRAPPER}" render "${common[@]}" >"${TEST_ROOT}/wrapper.json"
cmp "${TEST_ROOT}/rules.json" "${TEST_ROOT}/wrapper.json"

jq -e '
  .enforcement == "active" and
  .bypass_actors == [] and
  ([.rules[].type] | index("merge_queue")) != null and
  ([.rules[].type] | index("workflows")) != null and
  ([.rules[] | select(.type == "required_status_checks") |
    .parameters.required_status_checks[].context] | sort) ==
    (["Generic parity policy gate", "Generic validation",
      "Protected signed-evidence gate"] | sort)
' "${TEST_ROOT}/rules.json" >/dev/null

mutate() {
  local name="$1"
  local filter="$2"
  jq "${filter}" "${TEST_ROOT}/rules.json" >"${TEST_ROOT}/${name}.json"
}

mutate bypass '.bypass_actors = [{"actor_id": 1, "actor_type": "User", "bypass_mode": "always"}]'
expect_failure bypass 'must have no bypass actors' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/bypass.json"

mutate loose_status '(.rules[] | select(.type == "required_status_checks") | .parameters.strict_required_status_checks_policy) = false'
expect_failure loose_status 'not strict and source-pinned' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/loose_status.json"

mutate wrong_source '(.rules[] | select(.type == "required_status_checks") | .parameters.required_status_checks[0].integration_id) = 7'
expect_failure wrong_source 'not strict and source-pinned' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/wrong_source.json"

mutate grouped_prs '(.rules[] | select(.type == "merge_queue") | .parameters.max_entries_to_merge) = 2'
expect_failure grouped_prs 'one fully checked PR per group' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/grouped_prs.json"

mutate workflow_ref '(.rules[] | select(.type == "workflows") | .parameters.workflows[0].ref) = "refs/heads/feature"'
expect_failure workflow_ref 'not pinned to the protected default branch' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/workflow_ref.json"

mutate stale_reviews '(.rules[] | select(.type == "pull_request") | .parameters.dismiss_stale_reviews_on_push) = false'
expect_failure stale_reviews 'review enforcement is weaker' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/stale_reviews.json"

mutate inactive '.enforcement = "evaluate"'
expect_failure inactive 'is not active on branches' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/inactive.json"

expect_failure bad_repository_id 'invalid repository ID' \
  python3 "${TOOL}" render --repository-id 0 \
  --actions-integration-id 15368 --default-branch main

bash -n "${WRAPPER}" "${BASH_SOURCE[0]}"
python3 -m py_compile "${TOOL}"
shellcheck "${WRAPPER}" "${BASH_SOURCE[0]}"
printf 'parity repository rule tests passed\n'
