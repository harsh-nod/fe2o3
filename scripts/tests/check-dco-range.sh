#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly CHECKER="${ROOT}/scripts/check-dco-range.py"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf -- "${TEST_ROOT}"' EXIT

git -C "${TEST_ROOT}" init -q
git -C "${TEST_ROOT}" config user.name 'Test User'
git -C "${TEST_ROOT}" config user.email 'test@example.com'
git -C "${TEST_ROOT}" commit -q --allow-empty -m baseline
base="$(git -C "${TEST_ROOT}" rev-parse HEAD)"

run_check() {
  (
    cd -- "${TEST_ROOT}"
    python3 -I "${CHECKER}" --base "${base}" --head "$1" --repo owner/repository
  )
}

expect_failure() {
  local expected="$1"
  local head="$2"
  if run_check "${head}" >"${TEST_ROOT}/out" 2>"${TEST_ROOT}/err"; then
    printf '%s\n' 'DCO mutation unexpectedly passed' >&2
    exit 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/err" >/dev/null
}

git -C "${TEST_ROOT}" commit -q --allow-empty \
  -m signed -m 'Signed-off-by: Test User <test@example.com>'
signed="$(git -C "${TEST_ROOT}" rev-parse HEAD)"
run_check "${signed}" >/dev/null

if (
  cd -- "${TEST_ROOT}"
  python3 "${CHECKER}" --base "${base}" --head "${signed}" \
    --repo owner/repository
) >"${TEST_ROOT}/unisolated.out" 2>"${TEST_ROOT}/unisolated.err"; then
  printf '%s\n' 'unisolated DCO checker unexpectedly passed' >&2
  exit 1
fi
grep -F -- 'checker must run in Python isolated mode (-I)' \
  "${TEST_ROOT}/unisolated.err" >/dev/null

mkdir -p -- "${TEST_ROOT}/shadow"
cp -- "${CHECKER}" "${TEST_ROOT}/shadow/check-dco-range.py"
printf '%s\n' 'raise RuntimeError("candidate import shadow executed")' \
  >"${TEST_ROOT}/shadow/json.py"
(
  cd -- "${TEST_ROOT}"
  python3 -I "${TEST_ROOT}/shadow/check-dco-range.py" \
    --base "${base}" --head "${signed}" --repo owner/repository
) >/dev/null

git -C "${TEST_ROOT}" switch -q --detach "${base}"
git -C "${TEST_ROOT}" commit -q --allow-empty -m unsigned
unsigned="$(git -C "${TEST_ROOT}" rev-parse HEAD)"
expect_failure 'missing exact trailer Signed-off-by: Test User <test@example.com>' "${unsigned}"

git -C "${TEST_ROOT}" switch -q -c merge-left "${base}"
git -C "${TEST_ROOT}" commit -q --allow-empty \
  -m left -m 'Signed-off-by: Test User <test@example.com>'
git -C "${TEST_ROOT}" switch -q -c merge-right "${base}"
git -C "${TEST_ROOT}" commit -q --allow-empty \
  -m right -m 'Signed-off-by: Test User <test@example.com>'
git -C "${TEST_ROOT}" merge -q --no-ff merge-left -m 'unsigned merge'
merge_head="$(git -C "${TEST_ROOT}" rev-parse HEAD)"
expect_failure 'missing exact trailer Signed-off-by: Test User <test@example.com>' "${merge_head}"

git -C "${TEST_ROOT}" switch -q --detach "${base}"
GIT_AUTHOR_NAME='dependabot[bot]' \
GIT_AUTHOR_EMAIL='49699333+dependabot[bot]@users.noreply.github.com' \
GIT_COMMITTER_NAME='GitHub' \
GIT_COMMITTER_EMAIL='noreply@github.com' \
  git -C "${TEST_ROOT}" commit -q --allow-empty \
    -m dependency -m 'Signed-off-by: dependabot[bot] <support@github.com>'
dependabot="$(git -C "${TEST_ROOT}" rev-parse HEAD)"

mkdir -p -- "${TEST_ROOT}/fake-bin"
ln -s -- "$(command -v jq)" "${TEST_ROOT}/fake-bin/jq"
cat >"${TEST_ROOT}/fake-bin/gh" <<'EOF'
#!/usr/bin/env bash
set -Eeuo pipefail
commit="${*: -1}"
commit="${commit##*/}"
jq -cn \
  --arg sha "${commit}" \
  --argjson verified "${DCO_TEST_VERIFIED:-false}" \
  '{
    sha: $sha,
    author: {login: "dependabot[bot]"},
    committer: {login: "web-flow"},
    commit: {
      author: {
        name: "dependabot[bot]",
        email: "49699333+dependabot[bot]@users.noreply.github.com"
      },
      committer: {name: "GitHub", email: "noreply@github.com"},
      verification: {verified: $verified, reason: "valid"}
    }
  }'
EOF
chmod 700 "${TEST_ROOT}/fake-bin/gh"

PATH="${TEST_ROOT}/fake-bin:/usr/bin:/bin" DCO_TEST_VERIFIED=true \
  run_check "${dependabot}" >/dev/null
PATH="${TEST_ROOT}/fake-bin:/usr/bin:/bin" DCO_TEST_VERIFIED=false \
  expect_failure 'missing exact trailer Signed-off-by: dependabot[bot]' "${dependabot}"

printf '%s\n' 'DCO range tests passed'
