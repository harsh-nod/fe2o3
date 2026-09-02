#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly TOOL="${ROOT}/scripts/canonical-repository-rules.py"
readonly WRAPPER="${ROOT}/scripts/canonical-repository-rules.sh"
readonly SYNTAX_CHECK="${ROOT}/scripts/tests/python-syntax-only.sh"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT

assert_clean_checkout() {
  local stage="$1"
  local status artifacts
  status="$(git -C "${ROOT}" status --porcelain=v1 --untracked-files=all)"
  artifacts="$(
    find "${ROOT}" -path "${ROOT}/.git" -prune -o \
      -path "${ROOT}/target" -prune -o \
      \( -type d -name __pycache__ -o -type f \
      \( -name '*.pyc' -o -name '*.pyo' \) \) -print
  )"
  if [[ -n "${status}" || -n "${artifacts}" ]]; then
    printf 'checkout is not clean %s canonical-rule tests\n' "${stage}" >&2
    [[ -z "${status}" ]] || printf '%s\n' "${status}" >&2
    [[ -z "${artifacts}" ]] || printf '%s\n' "${artifacts}" >&2
    exit 1
  fi
}

assert_clean_checkout before

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'canonical-rule negative test unexpectedly passed: %s\n' "${name}" >&2
    exit 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'canonical-rule negative test produced wrong diagnostic: %s\n' \
      "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    exit 1
  }
}

common=(--actions-integration-id 15368)

python3 "${TOOL}" render "${common[@]}" >"${TEST_ROOT}/rules.json"
python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/rules.json"
bash "${WRAPPER}" render "${common[@]}" >"${TEST_ROOT}/wrapper.json"
cmp "${TEST_ROOT}/rules.json" "${TEST_ROOT}/wrapper.json"

jq -e '
  .enforcement == "active" and
  .bypass_actors == [] and
  .conditions == {
    "ref_name": {"exclude": [], "include": ["~DEFAULT_BRANCH"]}
  } and
  ([.rules[].type] | sort) ==
    (["deletion", "non_fast_forward", "pull_request",
      "required_status_checks"] | sort) and
  ([.rules[] | select(.type == "required_status_checks") |
    .parameters.required_status_checks[].context] | sort) ==
    (["Fork-safe preflight", "Generic parity policy gate",
      "Generic validation"] | sort) and
  ([.rules[] | select(.type == "required_status_checks") |
    .parameters.required_status_checks[].integration_id] | unique) == [15368]
' "${TEST_ROOT}/rules.json" >/dev/null

mutate() {
  local name="$1"
  local filter="$2"
  jq "${filter}" "${TEST_ROOT}/rules.json" >"${TEST_ROOT}/${name}.json"
}

mutate bypass '.bypass_actors = [{"actor_id": 1, "actor_type": "User", "bypass_mode": "always"}]'
expect_failure bypass 'must have no bypass actors' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/bypass.json"

mutate branch '.conditions.ref_name.include = ["refs/heads/main"]'
expect_failure branch 'does not target only the default branch' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/branch.json"

mutate merge_queue '.rules += [{"type": "merge_queue", "parameters": {}}]'
expect_failure merge_queue 'exact admitted rule types' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/merge_queue.json"

mutate workflows '.rules += [{"type": "workflows", "parameters": {}}]'
expect_failure workflows 'exact admitted rule types' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/workflows.json"

mutate protected_context '(.rules[] | select(.type == "required_status_checks") | .parameters.required_status_checks) += [{"context": "Protected signed-evidence gate / Protected publisher authorization", "integration_id": 15368}]'
expect_failure protected_context 'not strict and GitHub-Actions-pinned' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/protected_context.json"

mutate loose_status '(.rules[] | select(.type == "required_status_checks") | .parameters.strict_required_status_checks_policy) = false'
expect_failure loose_status 'not strict and GitHub-Actions-pinned' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/loose_status.json"

mutate wrong_source '(.rules[] | select(.type == "required_status_checks") | .parameters.required_status_checks[0].integration_id) = 7'
expect_failure wrong_source 'not strict and GitHub-Actions-pinned' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/wrong_source.json"

mutate two_approvals '(.rules[] | select(.type == "pull_request") | .parameters.required_approving_review_count) = 2'
expect_failure two_approvals 'does not match canonical policy' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/two_approvals.json"

mutate merge_method '(.rules[] | select(.type == "pull_request") | .parameters.allowed_merge_methods) = ["merge"]'
expect_failure merge_method 'does not match canonical policy' \
  python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/merge_method.json"

for setting in \
  dismiss_stale_reviews_on_push \
  require_code_owner_review \
  require_last_push_approval \
  required_review_thread_resolution; do
  mutate "${setting}" \
    "(.rules[] | select(.type == \"pull_request\") | .parameters.${setting}) = false"
  expect_failure "${setting}" 'does not match canonical policy' \
    python3 "${TOOL}" verify "${common[@]}" "${TEST_ROOT}/${setting}.json"
done

expect_failure bad_integration_id 'invalid Actions integration ID' \
  python3 "${TOOL}" render --actions-integration-id 0
expect_failure wrong_integration_id \
  'Actions integration ID does not identify GitHub Actions on github.com' \
  python3 "${TOOL}" render --actions-integration-id 7
expect_failure wrong_repository '--repo must be harsh-nod/fe2o3' \
  bash "${WRAPPER}" verify --repo powderluv/fe2o3 "${common[@]}"

mkdir -p "${TEST_ROOT}/fake-bin" "${TEST_ROOT}/fake-gh"
cat >"${TEST_ROOT}/fake-bin/gh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail

case " $* " in
  *' --method POST '*)
    input=''
    while (($#)); do
      if [[ "$1" == --input && "$#" -ge 2 ]]; then
        input="$2"
        break
      fi
      shift
    done
    [[ -n "${input}" && -f "${input}" ]]
    cp "${input}" "${FAKE_GH_ROOT}/posted.json"
    : >"${FAKE_GH_ROOT}/created"
    cat "${input}"
    ;;
  *'repos/harsh-nod/fe2o3/rulesets/9001?includes_parents=false'*)
    cat "${FAKE_GH_ROOT}/posted.json"
    ;;
  *'repos/harsh-nod/fe2o3/rulesets?includes_parents=false&targets=branch&per_page=100'*)
    if [[ "${FAKE_GH_FAIL_LIST:-0}" == 1 ]]; then
      printf 'injected list failure\n' >&2
      exit 77
    fi
    [[ ! -f "${FAKE_GH_ROOT}/created" ]] || printf '%s\n' 9001
    ;;
  *)
    printf 'unexpected fake gh invocation: %s\n' "$*" >&2
    exit 99
    ;;
esac
EOF
chmod 700 "${TEST_ROOT}/fake-bin/gh"
fake_env=(
  env
  FAKE_GH_ROOT="${TEST_ROOT}/fake-gh"
  PATH="${TEST_ROOT}/fake-bin:/usr/bin:/bin"
)

expect_failure missing_ruleset 'expected exactly one canonical ruleset' \
  "${fake_env[@]}" bash "${WRAPPER}" verify \
  --repo harsh-nod/fe2o3 "${common[@]}"
expect_failure list_failure 'cannot list canonical repository rulesets' \
  env FAKE_GH_FAIL_LIST=1 FAKE_GH_ROOT="${TEST_ROOT}/fake-gh" \
  PATH="${TEST_ROOT}/fake-bin:/usr/bin:/bin" \
  bash "${WRAPPER}" bootstrap --repo harsh-nod/fe2o3 "${common[@]}"
"${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"
cmp "${TEST_ROOT}/rules.json" "${TEST_ROOT}/fake-gh/posted.json"
"${fake_env[@]}" bash "${WRAPPER}" verify \
  --repo harsh-nod/fe2o3 "${common[@]}"
expect_failure duplicate_bootstrap 'canonical ruleset already exists' \
  "${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"

bash -n "${WRAPPER}" "${BASH_SOURCE[0]}"
"${SYNTAX_CHECK}" "${TOOL}"
shellcheck "${WRAPPER}" "${BASH_SOURCE[0]}"
assert_clean_checkout after
printf 'canonical repository rule tests passed\n'
