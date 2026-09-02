#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)"
readonly ROOT

metadata="$({
  cd -- "${ROOT}"
  cargo metadata --locked --no-deps --format-version 1
})"

jq -e '
  (.packages | length) > 0 and
  all(.packages[]; .publish == []) and
  ([.packages[] | select(
    .name == "fe2o3-kfd-uapi" or .name == "fe2o3-drm-uapi"
  )] | length) == 2 and
  all(.packages[] | select(
    .name == "fe2o3-kfd-uapi" or .name == "fe2o3-drm-uapi"
  ); .license == "(Apache-2.0 OR MIT) AND MIT")
' <<<"${metadata}" >/dev/null

grep -F -- 'licenseDeclared: "NOASSERTION"' \
  "${ROOT}/.github/workflows/release.yml" >/dev/null
grep -F -- '--branch main' "${ROOT}/.github/workflows/release.yml" >/dev/null
readonly release_workflow="${ROOT}/.github/workflows/release.yml"
# The authority and mirror gates run at admission and immediately before both
# remote writes (artifact attestation and draft release creation).
# shellcheck disable=SC2016
[[ "$(grep -Fc -- '[[ "${GITHUB_REF}" != refs/heads/main ]]' \
  "${release_workflow}")" -eq 3 ]]
# shellcheck disable=SC2016
[[ "$(grep -Fc -- '[[ "${GITHUB_ACTOR}" != "${RELEASE_OPERATOR}" ]]' \
  "${release_workflow}")" -eq 3 ]]
# shellcheck disable=SC2016
[[ "$(grep -Fc -- '[[ "${GITHUB_TRIGGERING_ACTOR}" != "${RELEASE_OPERATOR}" ]]' \
  "${release_workflow}")" -eq 3 ]]
[[ "$(grep -Fc -- 'bash scripts/check-release-mirror.sh' \
  "${release_workflow}")" -eq 3 ]]
[[ "$(grep -Fc -- 'git ls-remote --exit-code --refs "${canonical_url}"' \
  "${release_workflow}")" -eq 2 ]]
[[ "$(grep -Fc -- 'refs/remotes/release-canonical/main' \
  "${release_workflow}")" -eq 4 ]]
grep -F -- '--tree "${release_tree}"' "${release_workflow}" >/dev/null
# shellcheck disable=SC2016
grep -F -- '[[ "${run_head_branch}" != main ]]' \
  "${ROOT}/.github/workflows/release.yml" >/dev/null
readonly generic_workflow="${ROOT}/.github/workflows/ci.yml"
readonly push_trigger="$({
  sed -n '/^  push:$/,/^  pull_request:$/p' "${generic_workflow}"
})"
[[ "${push_trigger}" == $'  push:\n  pull_request:' ]] || {
  grep -F -- '    branches:' "${generic_workflow}" >/dev/null
  grep -F -- '      - "**"' "${generic_workflow}" >/dev/null
}
grep -F -- 'THIRD_PARTY_LICENSES.md' "${ROOT}/README.md" >/dev/null
grep -F -- 'scripts/canonical-release-controls.sh verify' \
  "${ROOT}/docs/release-process.md" >/dev/null
grep -F -- 'bash scripts/check-release-mirror.sh' \
  "${ROOT}/docs/release-process.md" >/dev/null

printf '%s\n' 'community release metadata tests passed'
