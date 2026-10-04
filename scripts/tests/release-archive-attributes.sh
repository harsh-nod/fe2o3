#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly CHECK="${ROOT}/scripts/check-release-archive-attributes.sh"
TEST_ROOT="$(mktemp -d)"
readonly TEST_ROOT
trap 'rm -rf -- "${TEST_ROOT}"' EXIT

git -C "${TEST_ROOT}" init -q
git -C "${TEST_ROOT}" config user.name 'fe2o3 release test'
git -C "${TEST_ROOT}" config user.email 'release-test@invalid.example'
printf '%s\n' source >"${TEST_ROOT}/source.rs"
printf '%s\n' ignored >"${TEST_ROOT}/ignored.txt"
mkdir -p -- "${TEST_ROOT}/nested"
printf '%s\n' nested >"${TEST_ROOT}/nested/source.rs"
git -C "${TEST_ROOT}" add source.rs ignored.txt nested/source.rs
git -C "${TEST_ROOT}" commit -q -m baseline
baseline="$(git -C "${TEST_ROOT}" rev-parse HEAD)"
(cd -- "${TEST_ROOT}" && bash "${CHECK}" "${baseline}") >/dev/null

expect_rejection() {
  local expected="$1"
  local commit
  commit="$(git -C "${TEST_ROOT}" rev-parse HEAD)"
  if (cd -- "${TEST_ROOT}" && bash "${CHECK}" "${commit}") \
    >"${TEST_ROOT}/check.out" 2>"${TEST_ROOT}/check.err"; then
    printf '%s\n' 'release archive attribute mutation unexpectedly passed' >&2
    exit 1
  fi
  grep -F -- "${expected}" "${TEST_ROOT}/check.err" >/dev/null
}

printf '%s\n' 'ignored.txt export-ignore' >"${TEST_ROOT}/.gitattributes"
git -C "${TEST_ROOT}" add .gitattributes
git -C "${TEST_ROOT}" commit -q -m export-ignore
expect_rejection 'export-ignore=set'

printf '%s\n' 'nested export-ignore' >"${TEST_ROOT}/.gitattributes"
git -C "${TEST_ROOT}" add .gitattributes
git -C "${TEST_ROOT}" commit -q -m directory-export-ignore
expect_rejection 'export-ignore=set'

printf '%s\n' 'source.rs export-subst' >"${TEST_ROOT}/.gitattributes"
git -C "${TEST_ROOT}" add .gitattributes
git -C "${TEST_ROOT}" commit -q -m export-subst
expect_rejection 'export-subst=set'

printf '%s\n' 'source.rs -export-subst' >"${TEST_ROOT}/.gitattributes"
git -C "${TEST_ROOT}" add .gitattributes
git -C "${TEST_ROOT}" commit -q -m explicitly-unset
unset_commit="$(git -C "${TEST_ROOT}" rev-parse HEAD)"
(cd -- "${TEST_ROOT}" && bash "${CHECK}" "${unset_commit}") >/dev/null

printf '%s\n' 'release archive attribute tests passed'
