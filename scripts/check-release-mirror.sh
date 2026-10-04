#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

readonly PRODUCTION_MIRROR_URL="https://github.com/powderluv/fe2o3.git"
readonly MIRROR_BRANCH="main"

fail() {
  printf 'release mirror check: %s\n' "$*" >&2
  exit 2
}

usage() {
  printf 'usage: %s --commit <40-hex-commit> --tree <40-hex-tree>\n' \
    "${0##*/}" >&2
  exit 2
}

commit=""
expected_tree=""
while (($#)); do
  case "$1" in
    --commit)
      (($# >= 2)) || usage
      commit="$2"
      shift 2
      ;;
    --tree)
      (($# >= 2)) || usage
      expected_tree="$2"
      shift 2
      ;;
    *)
      usage
      ;;
  esac
done

[[ "${commit}" =~ ^[0-9a-f]{40}$ ]] || fail 'commit must be 40 lowercase hex characters'
[[ "${expected_tree}" =~ ^[0-9a-f]{40}$ ]] || fail 'tree must be 40 lowercase hex characters'
[[ "$(git cat-file -t "${commit}" 2>/dev/null || true)" == commit ]] ||
  fail "local release commit does not exist: ${commit}"
actual_tree="$(git rev-parse "${commit}^{tree}")"
[[ "${actual_tree}" == "${expected_tree}" ]] ||
  fail "release commit tree does not match expected tree: ${expected_tree}"

mirror_url="${FE2O3_RELEASE_MIRROR_URL:-${PRODUCTION_MIRROR_URL}}"
if [[ "${mirror_url}" != "${PRODUCTION_MIRROR_URL}" ]] &&
  [[ "${FE2O3_RELEASE_MIRROR_TEST_ONLY:-}" != 1 ]]; then
  fail 'a non-production mirror URL requires FE2O3_RELEASE_MIRROR_TEST_ONLY=1'
fi

readonly temporary_ref="refs/fe2o3-release-mirror-check/$$-${RANDOM}"
if git show-ref --verify --quiet "${temporary_ref}"; then
  fail "temporary ref already exists: ${temporary_ref}"
fi
cleanup() {
  git update-ref -d "${temporary_ref}" >/dev/null 2>&1 || true
}
trap cleanup EXIT

git fetch --quiet --no-tags --no-write-fetch-head --force "${mirror_url}" \
  "+refs/heads/${MIRROR_BRANCH}:${temporary_ref}" >/dev/null ||
  fail "cannot fetch ${MIRROR_BRANCH} from ${mirror_url}"
mirror_commit="$(git rev-parse "${temporary_ref}^{commit}")"
if ! git merge-base --is-ancestor "${commit}" "${mirror_commit}"; then
  fail "release commit ${commit} is not reachable from mirror ${MIRROR_BRANCH} ${mirror_commit}"
fi

printf 'release mirror check passed: commit=%s tree=%s mirror_main=%s\n' \
  "${commit}" "${expected_tree}" "${mirror_commit}"
