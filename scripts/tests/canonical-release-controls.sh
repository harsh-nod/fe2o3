#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

TEST_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly TEST_DIR
ROOT="$(cd -- "${TEST_DIR}/../.." && pwd)"
readonly ROOT
readonly TOOL="${ROOT}/scripts/canonical-release-controls.py"
readonly WRAPPER="${ROOT}/scripts/canonical-release-controls.sh"
readonly SYNTAX_CHECK="${ROOT}/scripts/tests/python-syntax-only.sh"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf "${TEST_ROOT}"' EXIT

assert_clean_checkout() {
  local stage="$1"
  local status artifacts
  status="$(git -C "${ROOT}" status --porcelain=v1 --untracked-files=all)"
  artifacts="$({
    find "${ROOT}" -path "${ROOT}/.git" -prune -o \
      -path "${ROOT}/target" -prune -o \
      \( -type d -name __pycache__ -o -type f \
      \( -name '*.pyc' -o -name '*.pyo' \) \) -print
  })"
  if [[ -n "${status}" || -n "${artifacts}" ]]; then
    printf 'checkout is not clean %s canonical-release tests\n' "${stage}" >&2
    [[ -z "${status}" ]] || printf '%s\n' "${status}" >&2
    [[ -z "${artifacts}" ]] || printf '%s\n' "${artifacts}" >&2
    exit 1
  fi
}

assert_clean_checkout before

for owned_path in \
  /scripts/canonical-release-controls.py \
  /scripts/canonical-release-controls.sh \
  /scripts/tests/canonical-release-controls.sh; do
  grep -Fx -- "${owned_path} @harsh-nod @powderluv" \
    "${ROOT}/.github/CODEOWNERS" >/dev/null
done

expect_failure() {
  local name="$1"
  local expected="$2"
  shift 2
  if "$@" >"${TEST_ROOT}/${name}.out" 2>"${TEST_ROOT}/${name}.err"; then
    printf 'canonical-release negative test unexpectedly passed: %s\n' \
      "${name}" >&2
    exit 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/${name}.err" >/dev/null || {
    printf 'canonical-release negative test produced wrong diagnostic: %s\n' \
      "${name}" >&2
    cat "${TEST_ROOT}/${name}.err" >&2
    exit 1
  }
}

common=(--release-user-id 3144552 --reviewer-user-id 74956)
for control in \
  tag-creation tag-guard tag-immutability environment environment-policy; do
  python3 -I "${TOOL}" render --control "${control}" "${common[@]}" \
    >"${TEST_ROOT}/${control}.json"
  python3 -I "${TOOL}" verify --control "${control}" "${common[@]}" \
    "${TEST_ROOT}/${control}.json"
  bash "${WRAPPER}" render --control "${control}" "${common[@]}" \
    >"${TEST_ROOT}/${control}.wrapper.json"
  cmp "${TEST_ROOT}/${control}.json" \
    "${TEST_ROOT}/${control}.wrapper.json"
done

jq -e '
  .target == "tag" and
  .enforcement == "active" and
  .conditions == {
    "ref_name": {"exclude": [], "include": ["refs/tags/v*"]}
  } and
  .bypass_actors == [{
    "actor_id": 3144552,
    "actor_type": "User",
    "bypass_mode": "always"
  }] and
  .rules == [{"type": "creation"}]
' "${TEST_ROOT}/tag-creation.json" >/dev/null
jq -e '
  .target == "tag" and
  .bypass_actors == [] and
  ([.rules[].type] | sort) == (["deletion", "update"] | sort) and
  (.rules[] | select(.type == "update") |
    .parameters.update_allows_fetch_and_merge) == false
' "${TEST_ROOT}/tag-immutability.json" >/dev/null
jq -e '
  .target == "tag" and
  .bypass_actors == [] and
  ([.rules[].type] | sort) == (["creation", "deletion", "update"] | sort)
' "${TEST_ROOT}/tag-guard.json" >/dev/null
jq -e '
  .wait_timer == 0 and
  .prevent_self_review == true and
  .can_admins_bypass == false and
  .reviewers == [{"id": 74956, "type": "User"}] and
  .deployment_branch_policy == {
    "protected_branches": false,
    "custom_branch_policies": true
  }
' "${TEST_ROOT}/environment.json" >/dev/null
jq -e '
  .name == "main" and .type == "branch"
' "${TEST_ROOT}/environment-policy.json" >/dev/null

mutate() {
  local source="$1"
  local name="$2"
  local filter="$3"
  jq "${filter}" "${TEST_ROOT}/${source}.json" >"${TEST_ROOT}/${name}.json"
}

mutate tag-creation wrong_pattern \
  '.conditions.ref_name.include = ["refs/tags/*"]'
expect_failure wrong_pattern 'does not target refs/tags/v*' \
  python3 -I "${TOOL}" verify --control tag-creation "${common[@]}" \
  "${TEST_ROOT}/wrong_pattern.json"
mutate tag-creation extra_creator \
  '.bypass_actors += [{"actor_id": 7, "actor_type": "User", "bypass_mode": "always"}]'
expect_failure extra_creator 'bypass actors do not match policy' \
  python3 -I "${TOOL}" verify --control tag-creation "${common[@]}" \
  "${TEST_ROOT}/extra_creator.json"
mutate tag-creation extra_creation_rule '.rules += [{"type": "deletion"}]'
expect_failure extra_creation_rule 'must contain only creation' \
  python3 -I "${TOOL}" verify --control tag-creation "${common[@]}" \
  "${TEST_ROOT}/extra_creation_rule.json"
mutate tag-immutability immutable_bypass \
  '.bypass_actors = [{"actor_id": 3144552, "actor_type": "User", "bypass_mode": "always"}]'
expect_failure immutable_bypass 'bypass actors do not match policy' \
  python3 -I "${TOOL}" verify --control tag-immutability "${common[@]}" \
  "${TEST_ROOT}/immutable_bypass.json"
mutate tag-immutability mutable_tag \
  '(.rules[] | select(.type == "update") | .parameters.update_allows_fetch_and_merge) = true'
expect_failure mutable_tag 'does not prohibit all updates' \
  python3 -I "${TOOL}" verify --control tag-immutability "${common[@]}" \
  "${TEST_ROOT}/mutable_tag.json"
mutate environment self_review '.prevent_self_review = false'
expect_failure self_review 'must prevent self review' \
  python3 -I "${TOOL}" verify --control environment "${common[@]}" \
  "${TEST_ROOT}/self_review.json"
mutate environment wrong_reviewer '.reviewers[0].id = 3144552'
expect_failure wrong_reviewer 'does not match powderluv' \
  python3 -I "${TOOL}" verify --control environment "${common[@]}" \
  "${TEST_ROOT}/wrong_reviewer.json"
mutate environment restricted_ref \
  '.deployment_branch_policy = null'
expect_failure restricted_ref 'must use an exact custom main-branch policy' \
  python3 -I "${TOOL}" verify --control environment "${common[@]}" \
  "${TEST_ROOT}/restricted_ref.json"
mutate environment admin_bypass '.can_admins_bypass = true'
expect_failure admin_bypass 'must prohibit administrator bypass' \
  python3 -I "${TOOL}" verify --control environment "${common[@]}" \
  "${TEST_ROOT}/admin_bypass.json"
mutate environment-policy wildcard_policy '.name = "*"'
expect_failure wildcard_policy 'must match only main' \
  python3 -I "${TOOL}" verify --control environment-policy "${common[@]}" \
  "${TEST_ROOT}/wildcard_policy.json"
mutate environment-policy tag_policy '.type = "tag"'
expect_failure tag_policy 'must target a branch' \
  python3 -I "${TOOL}" verify --control environment-policy "${common[@]}" \
  "${TEST_ROOT}/tag_policy.json"

expect_failure unisolated 'must run with isolated Python (-I)' \
  python3 "${TOOL}" render --control environment "${common[@]}"
expect_failure wrong_release_id \
  'release user ID does not identify harsh-nod on github.com' \
  python3 -I "${TOOL}" render --control environment \
  --release-user-id 7 --reviewer-user-id 74956
expect_failure wrong_reviewer_id \
  'reviewer user ID does not identify powderluv on github.com' \
  python3 -I "${TOOL}" render --control environment \
  --release-user-id 3144552 --reviewer-user-id 7
expect_failure wrong_repository '--repo must be harsh-nod/fe2o3' \
  bash "${WRAPPER}" verify --repo powderluv/fe2o3 "${common[@]}"

mkdir -p "${TEST_ROOT}/fake-bin" "${TEST_ROOT}/fake-gh"
cat >"${TEST_ROOT}/fake-bin/gh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail

args=" $* "
root="${FAKE_GH_ROOT}"
if [[ "${args}" == *' users/harsh-nod '* ]]; then
  printf '%s\n' 3144552
elif [[ "${args}" == *' users/powderluv '* ]]; then
  printf '%s\n' 74956
elif [[ "${args}" == *'/collaborators/powderluv/permission '* ]]; then
  printf '%s\n' true
elif [[ "${args}" == *'/git/matching-refs/tags/v?per_page=100 '* ]]; then
  if [[ "${FAKE_GH_RACE_TAG:-0}" == 1 ]]; then
    count=0
    [[ ! -f "${root}/matching-ref-count" ]] ||
      count="$(cat "${root}/matching-ref-count")"
    count="$((count + 1))"
    printf '%s\n' "${count}" >"${root}/matching-ref-count"
    [[ "${count}" -lt 2 ]] || printf '%s\n' refs/tags/v0.1.0-dev.1
  else
    [[ ! -f "${root}/release-ref" ]] || cat "${root}/release-ref"
  fi
elif [[ "${args}" == *'/environments?per_page=100 '* ]]; then
  [[ "${FAKE_GH_FAIL_ENV_LIST:-0}" != 1 ]] || {
    printf 'injected environment list failure\n' >&2
    exit 77
  }
  [[ ! -f "${root}/environment.json" ]] || printf '%s\n' release
elif [[ "${args}" == *'/deployment-branch-policies?per_page=100 '* ]]; then
  if [[ -f "${root}/environment-policy.json" ]]; then
    printf '9201\tmain\n'
  fi
  if [[ -f "${root}/extra-environment-policy" ]]; then
    printf '9202\trelease/*\n'
  fi
elif [[ "${args}" == *' --method PUT '* && \
    "${args}" == *'/environments/release '* ]]; then
  input=''
  while (($#)); do
    if [[ "$1" == --input && "$#" -ge 2 ]]; then
      input="$2"
      break
    fi
    shift
  done
  jq '{
    name: "release",
    can_admins_bypass: .can_admins_bypass,
    deployment_branch_policy: .deployment_branch_policy,
    protection_rules: [{
      type: "required_reviewers",
      prevent_self_review: .prevent_self_review,
      reviewers: [.reviewers[] | {
        type: .type,
        reviewer: {id: .id, login: "powderluv"}
      }]
    }, {type: "branch_policy"}]
  }' "${input}" >"${root}/environment.json"
  cat "${root}/environment.json"
elif [[ "${args}" == *' --method POST '* && \
    "${args}" == *'/deployment-branch-policies '* ]]; then
  input=''
  while (($#)); do
    if [[ "$1" == --input && "$#" -ge 2 ]]; then
      input="$2"
      break
    fi
    shift
  done
  jq '. + {id: 9201}' "${input}" >"${root}/environment-policy.json"
  cat "${root}/environment-policy.json"
elif [[ "${args}" == *'/deployment-branch-policies/9201 '* ]]; then
  cat "${root}/environment-policy.json"
elif [[ "${args}" == *'/environments/release '* ]]; then
  cat "${root}/environment.json"
elif [[ "${args}" == *'/rulesets?includes_parents=false&targets=tag&per_page=100 '* ]]; then
  if [[ "${args}" == *'fe2o3-release-tag-creation'* && \
      -f "${root}/tag-creation.json" ]]; then
    printf '%s\n' 9101
  elif [[ "${args}" == *'fe2o3-release-tag-immutability'* && \
      -f "${root}/tag-immutability.json" ]]; then
    printf '%s\n' 9102
  fi
elif [[ "${args}" == *' --method POST '* && \
    "${args}" == *' repos/harsh-nod/fe2o3/rulesets '* ]]; then
  input=''
  while (($#)); do
    if [[ "$1" == --input && "$#" -ge 2 ]]; then
      input="$2"
      break
    fi
    shift
  done
  name="$(jq -r .name "${input}")"
  case "${name}" in
    fe2o3-release-tag-creation)
      destination="${root}/tag-creation.json"
      id=9101
      ;;
    fe2o3-release-tag-immutability)
      destination="${root}/tag-immutability.json"
      id=9102
      ;;
    *) exit 98 ;;
  esac
  jq --argjson id "${id}" '. + {id: $id}' "${input}" >"${destination}"
  cat "${destination}"
elif [[ "${args}" == *' --method PUT '* && \
    "${args}" == *'/rulesets/9102 '* ]]; then
  input=''
  while (($#)); do
    if [[ "$1" == --input && "$#" -ge 2 ]]; then
      input="$2"
      break
    fi
    shift
  done
  jq '. + {id: 9102}' "${input}" >"${root}/tag-immutability.json"
  cat "${root}/tag-immutability.json"
elif [[ "${args}" == *'/rulesets/9101?includes_parents=false '* ]]; then
  cat "${root}/tag-creation.json"
elif [[ "${args}" == *'/rulesets/9102?includes_parents=false '* ]]; then
  cat "${root}/tag-immutability.json"
else
  printf 'unexpected fake gh invocation: %s\n' "$*" >&2
  exit 99
fi
EOF
chmod 700 "${TEST_ROOT}/fake-bin/gh"
fake_env=(
  env
  FAKE_GH_ROOT="${TEST_ROOT}/fake-gh"
  PATH="${TEST_ROOT}/fake-bin:/usr/bin:/bin"
)

expect_failure missing_environment 'missing canonical release environment' \
  "${fake_env[@]}" bash "${WRAPPER}" verify \
  --repo harsh-nod/fe2o3 "${common[@]}"

mkdir -p "${TEST_ROOT}/fake-gh-race"
expect_failure raced_release_tag \
  'bootstrap requires a repository with no existing v* tags' \
  env FAKE_GH_RACE_TAG=1 FAKE_GH_ROOT="${TEST_ROOT}/fake-gh-race" \
  PATH="${TEST_ROOT}/fake-bin:/usr/bin:/bin" \
  bash "${WRAPPER}" bootstrap --repo harsh-nod/fe2o3 "${common[@]}"
jq -e '
  .bypass_actors == [] and
  ([.rules[].type] | sort) == (["creation", "deletion", "update"] | sort)
' "${TEST_ROOT}/fake-gh-race/tag-immutability.json" >/dev/null

expect_failure environment_list_failure \
  'cannot list canonical release environments' \
  env FAKE_GH_FAIL_ENV_LIST=1 FAKE_GH_ROOT="${TEST_ROOT}/fake-gh" \
  PATH="${TEST_ROOT}/fake-bin:/usr/bin:/bin" \
  bash "${WRAPPER}" bootstrap --repo harsh-nod/fe2o3 "${common[@]}"
jq -e '
  .bypass_actors == [] and
  ([.rules[].type] | sort) == (["creation", "deletion", "update"] | sort)
' "${TEST_ROOT}/fake-gh/tag-immutability.json" >/dev/null

"${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"
"${fake_env[@]}" bash "${WRAPPER}" verify \
  --repo harsh-nod/fe2o3 "${common[@]}"
jq -e '
  .bypass_actors == [] and
  ([.rules[].type] | sort) == (["deletion", "update"] | sort)
' "${TEST_ROOT}/fake-gh/tag-immutability.json" >/dev/null

printf '%s\n' refs/tags/v0.1.0-dev.1 >"${TEST_ROOT}/fake-gh/release-ref"
expect_failure existing_release_tag \
  'bootstrap requires a repository with no existing v* tags' \
  "${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"
rm -f "${TEST_ROOT}/fake-gh/release-ref"

cp "${TEST_ROOT}/fake-gh/tag-creation.json" \
  "${TEST_ROOT}/fake-gh/tag-creation.saved.json"
rm -f "${TEST_ROOT}/fake-gh/tag-creation.json"
expect_failure unsafe_final_missing_creation \
  'missing canonical ruleset: fe2o3-release-tag-creation' \
  "${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"
mv "${TEST_ROOT}/fake-gh/tag-creation.saved.json" \
  "${TEST_ROOT}/fake-gh/tag-creation.json"

mv "${TEST_ROOT}/fake-gh/environment.json" \
  "${TEST_ROOT}/fake-gh/environment.saved.json"
expect_failure unsafe_final_missing_environment \
  'missing canonical release environment' \
  "${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"
mv "${TEST_ROOT}/fake-gh/environment.saved.json" \
  "${TEST_ROOT}/fake-gh/environment.json"

mv "${TEST_ROOT}/fake-gh/environment-policy.json" \
  "${TEST_ROOT}/fake-gh/environment-policy.saved.json"
expect_failure unsafe_final_missing_environment_policy \
  'missing release environment main deployment policy' \
  "${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"
mv "${TEST_ROOT}/fake-gh/environment-policy.saved.json" \
  "${TEST_ROOT}/fake-gh/environment-policy.json"

: >"${TEST_ROOT}/fake-gh/extra-environment-policy"
expect_failure extra_environment_policy \
  'must contain only the exact main policy' \
  "${fake_env[@]}" bash "${WRAPPER}" verify \
  --repo harsh-nod/fe2o3 "${common[@]}"
rm -f "${TEST_ROOT}/fake-gh/extra-environment-policy"

"${fake_env[@]}" bash "${WRAPPER}" verify \
  --repo harsh-nod/fe2o3 "${common[@]}"

jq '.bypass_actors = []' "${TEST_ROOT}/fake-gh/tag-creation.json" \
  >"${TEST_ROOT}/fake-gh/tag-creation.drifted.json"
mv "${TEST_ROOT}/fake-gh/tag-creation.drifted.json" \
  "${TEST_ROOT}/fake-gh/tag-creation.json"
expect_failure existing_drift 'bypass actors do not match policy' \
  "${fake_env[@]}" bash "${WRAPPER}" bootstrap \
  --repo harsh-nod/fe2o3 "${common[@]}"

bash -n "${WRAPPER}" "${BASH_SOURCE[0]}"
"${SYNTAX_CHECK}" "${TOOL}"
shellcheck "${WRAPPER}" "${BASH_SOURCE[0]}"
assert_clean_checkout after
printf 'canonical release control tests passed\n'
