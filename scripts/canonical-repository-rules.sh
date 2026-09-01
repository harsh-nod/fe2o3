#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly RULE_TOOL="${SCRIPT_DIR}/canonical-repository-rules.py"
readonly RULESET_NAME=fe2o3-canonical-default-branch
readonly CANONICAL_REPO=harsh-nod/fe2o3
readonly GITHUB_ACTIONS_INTEGRATION_ID=15368

die() {
  printf 'canonical repository rules: %s\n' "$1" >&2
  exit 2
}

usage() {
  cat <<'EOF'
Usage:
  scripts/canonical-repository-rules.sh render \
    --actions-integration-id ID
  scripts/canonical-repository-rules.sh bootstrap \
    --repo harsh-nod/fe2o3 --actions-integration-id ID
  scripts/canonical-repository-rules.sh verify \
    --repo harsh-nod/fe2o3 --actions-integration-id ID

bootstrap requires an authenticated gh token with repository Administration
write permission. verify requires enough access for bypass_actors to be
returned. The tool targets only the canonical repository and never updates or
replaces an existing ruleset.
EOF
}

[[ "$#" -ge 1 ]] || {
  usage >&2
  exit 2
}
readonly COMMAND="$1"
shift

REPO=""
ACTIONS_INTEGRATION_ID=""
while (($#)); do
  case "$1" in
    --repo)
      [[ "$#" -ge 2 ]] || die 'missing --repo value'
      REPO="$2"
      shift 2
      ;;
    --actions-integration-id)
      [[ "$#" -ge 2 ]] || die 'missing --actions-integration-id value'
      ACTIONS_INTEGRATION_ID="$2"
      shift 2
      ;;
    *) die "unknown option: $1" ;;
  esac
done

common=(--actions-integration-id "${ACTIONS_INTEGRATION_ID}")
[[ "${ACTIONS_INTEGRATION_ID}" =~ ^[1-9][0-9]*$ ]] ||
  die 'invalid Actions integration ID'
[[ "${ACTIONS_INTEGRATION_ID}" == "${GITHUB_ACTIONS_INTEGRATION_ID}" ]] ||
  die 'Actions integration ID does not identify GitHub Actions on github.com'

case "${COMMAND}" in
  render)
    [[ -z "${REPO}" ]] || die '--repo is not valid for render'
    exec python3 "${RULE_TOOL}" render "${common[@]}"
    ;;
  bootstrap | verify)
    [[ "${REPO}" == "${CANONICAL_REPO}" ]] ||
      die "--repo must be ${CANONICAL_REPO}"
    command -v gh >/dev/null || die 'gh is required'
    ;;
  *) die "unknown command: ${COMMAND}" ;;
esac

ids_file="$(mktemp)"
trap 'rm -f "${ids_file}"' EXIT
if ! gh api --hostname github.com --paginate \
    -H 'Accept: application/vnd.github+json' \
    -H 'X-GitHub-Api-Version: 2026-03-10' \
    "repos/${REPO}/rulesets?includes_parents=false&targets=branch&per_page=100" \
    --jq ".[] | select(.name == \"${RULESET_NAME}\") | .id" >"${ids_file}"; then
  die 'cannot list canonical repository rulesets'
fi
mapfile -t ids <"${ids_file}"
rm -f "${ids_file}"
trap - EXIT
for id in "${ids[@]}"; do
  [[ "${id}" =~ ^[1-9][0-9]*$ ]] || die 'GitHub returned an invalid ruleset ID'
done

if [[ "${COMMAND}" == bootstrap ]]; then
  ((${#ids[@]} == 0)) || die 'canonical ruleset already exists'
  payload="$(mktemp)"
  response="$(mktemp)"
  trap 'rm -f "${payload}" "${response}"' EXIT
  python3 "${RULE_TOOL}" render "${common[@]}" >"${payload}"
  python3 "${RULE_TOOL}" verify "${common[@]}" "${payload}" >/dev/null
  gh api --hostname github.com --method POST \
    -H 'Accept: application/vnd.github+json' \
    -H 'X-GitHub-Api-Version: 2026-03-10' \
    "repos/${REPO}/rulesets" --input "${payload}" >"${response}"
  python3 "${RULE_TOOL}" verify "${common[@]}" "${response}" >/dev/null
  printf 'canonical repository ruleset created\n'
  exit 0
fi

((${#ids[@]} == 1)) || die 'expected exactly one canonical ruleset'
ruleset="$(mktemp)"
trap 'rm -f "${ruleset}"' EXIT
gh api --hostname github.com \
  -H 'Accept: application/vnd.github+json' \
  -H 'X-GitHub-Api-Version: 2026-03-10' \
  "repos/${REPO}/rulesets/${ids[0]}?includes_parents=false" >"${ruleset}"
python3 "${RULE_TOOL}" verify "${common[@]}" "${ruleset}"
