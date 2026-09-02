#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT
readonly CHECKER="${ROOT}/scripts/check-release-mirror.sh"

scratch="$(mktemp -d)"
cleanup() {
  rm -rf -- "${scratch}"
}
trap cleanup EXIT

expect_failure() {
  local expected="$1"
  shift
  local output
  if output="$("$@" 2>&1)"; then
    printf 'expected command to fail: %q\n' "$*" >&2
    exit 1
  fi
  if [[ "${output}" != *"${expected}"* ]]; then
    printf 'failure did not contain %q:\n%s\n' "${expected}" "${output}" >&2
    exit 1
  fi
}

check_test_mirror() {
  local mirror="$1"
  shift
  (
    cd -- "${source_repo}"
    FE2O3_RELEASE_MIRROR_TEST_ONLY=1 \
      FE2O3_RELEASE_MIRROR_URL="${mirror}" \
      bash "${CHECKER}" "$@"
  )
}

check_untrusted_override() {
  local mirror="$1"
  shift
  (
    cd -- "${source_repo}"
    FE2O3_RELEASE_MIRROR_URL="${mirror}" bash "${CHECKER}" "$@"
  )
}

source_repo="${scratch}/source"
mirror_repo="${scratch}/mirror.git"
missing_main_repo="${scratch}/missing-main.git"
git init -q -b main "${source_repo}"
git -C "${source_repo}" config user.name 'Release Mirror Test'
git -C "${source_repo}" config user.email 'release-mirror-test@example.invalid'
printf 'baseline\n' >"${source_repo}/payload"
git -C "${source_repo}" add payload
git -C "${source_repo}" commit -q -m baseline
baseline_commit="$(git -C "${source_repo}" rev-parse HEAD)"
git clone -q --bare "${source_repo}" "${mirror_repo}"

printf 'release\n' >"${source_repo}/payload"
git -C "${source_repo}" commit -q -am release
release_commit="$(git -C "${source_repo}" rev-parse HEAD)"
release_tree="$(git -C "${source_repo}" rev-parse 'HEAD^{tree}')"

expect_failure 'is not reachable from mirror main' \
  check_test_mirror "${mirror_repo}" \
  --commit "${release_commit}" --tree "${release_tree}"

git -C "${source_repo}" push -q "${mirror_repo}" HEAD:refs/heads/main
check_test_mirror "${mirror_repo}" \
  --commit "${release_commit}" --tree "${release_tree}" \
  >/dev/null

printf 'later\n' >>"${source_repo}/payload"
git -C "${source_repo}" commit -q -am later
git -C "${source_repo}" push -q "${mirror_repo}" HEAD:refs/heads/main
check_test_mirror "${mirror_repo}" \
  --commit "${release_commit}" --tree "${release_tree}" \
  >/dev/null

git -C "${source_repo}" switch -q --detach "${baseline_commit}"
printf 'divergent\n' >"${source_repo}/payload"
git -C "${source_repo}" commit -q -am divergent
git -C "${source_repo}" push -q --force "${mirror_repo}" HEAD:refs/heads/main
expect_failure 'is not reachable from mirror main' \
  check_test_mirror "${mirror_repo}" \
  --commit "${release_commit}" --tree "${release_tree}"

expect_failure 'release commit tree does not match expected tree' \
  check_test_mirror "${mirror_repo}" \
  --commit "${release_commit}" --tree "${baseline_commit}"
expect_failure 'commit must be 40 lowercase hex characters' \
  check_test_mirror "${mirror_repo}" \
  --commit deadbeef --tree "${release_tree}"
expect_failure 'requires FE2O3_RELEASE_MIRROR_TEST_ONLY=1' \
  check_untrusted_override "${mirror_repo}" \
  --commit "${release_commit}" --tree "${release_tree}"

git clone -q --bare "${source_repo}" "${missing_main_repo}"
git --git-dir="${missing_main_repo}" update-ref -d refs/heads/main
expect_failure 'cannot fetch main' \
  check_test_mirror "${missing_main_repo}" \
  --commit "${release_commit}" --tree "${release_tree}"
if [[ -n "$(git -C "${source_repo}" for-each-ref \
  --format='%(refname)' refs/fe2o3-release-mirror-check/)" ]]; then
  printf 'release mirror check leaked a temporary ref\n' >&2
  exit 1
fi

printf '%s\n' 'release mirror tests passed'
