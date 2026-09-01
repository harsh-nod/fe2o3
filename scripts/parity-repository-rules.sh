#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
readonly RULE_TOOL="${SCRIPT_DIR}/parity-repository-rules.py"
readonly RULESET_NAME=fe2o3-production-parity

die() {
  printf 'parity repository rules: %s\n' "$1" >&2
  exit 2
}

usage() {
  cat <<'EOF'
Usage:
  scripts/parity-repository-rules.sh render --repository-id ID \
    --actions-integration-id ID --default-branch BRANCH
  scripts/parity-repository-rules.sh bootstrap --repo OWNER/REPO \
    --repository-id ID --actions-integration-id ID --default-branch BRANCH
  scripts/parity-repository-rules.sh verify --repo OWNER/REPO \
    --repository-id ID --actions-integration-id ID --default-branch BRANCH

bootstrap requires an authenticated gh token with repository Administration
write permission. verify requires enough access for bypass_actors to be returned.
The tool never updates or replaces an existing ruleset. Merge-queue rules are
admitted only for organization-owned repositories because GitHub does not offer
merge queues to repositories owned by personal accounts.
EOF
}

[[ "$#" -ge 1 ]] || {
  usage >&2
  exit 2
}
readonly COMMAND="$1"
shift

REPO=""
REPOSITORY_ID=""
ACTIONS_INTEGRATION_ID=""
DEFAULT_BRANCH=""
while (($#)); do
  case "$1" in
    --repo)
      [[ "$#" -ge 2 ]] || die 'missing --repo value'
      REPO="$2"
      shift 2
      ;;
    --repository-id)
      [[ "$#" -ge 2 ]] || die 'missing --repository-id value'
      REPOSITORY_ID="$2"
      shift 2
      ;;
    --actions-integration-id)
      [[ "$#" -ge 2 ]] || die 'missing --actions-integration-id value'
      ACTIONS_INTEGRATION_ID="$2"
      shift 2
      ;;
    --default-branch)
      [[ "$#" -ge 2 ]] || die 'missing --default-branch value'
      DEFAULT_BRANCH="$2"
      shift 2
      ;;
    *) die "unknown option: $1" ;;
  esac
done

common=(
  --repository-id "${REPOSITORY_ID}"
  --actions-integration-id "${ACTIONS_INTEGRATION_ID}"
  --default-branch "${DEFAULT_BRANCH}"
)

case "${COMMAND}" in
  render)
    [[ -z "${REPO}" ]] || die '--repo is not valid for render'
    exec python3 "${RULE_TOOL}" render "${common[@]}"
    ;;
  bootstrap | verify)
    [[ "${REPO}" =~ ^[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+$ ]] || die 'invalid --repo'
    command -v gh >/dev/null || die 'gh is required'
    owner="${REPO%%/*}"
    owner_type="$(
      gh api \
        -H 'Accept: application/vnd.github+json' \
        -H 'X-GitHub-Api-Version: 2026-03-10' \
        "users/${owner}" --jq .type
    )"
    [[ "${owner_type}" == Organization ]] ||
      die 'merge-queue rules require an organization-owned repository'
    ;;
  *) die "unknown command: ${COMMAND}" ;;
esac

mapfile -t ids < <(
  gh api --paginate \
    -H 'Accept: application/vnd.github+json' \
    -H 'X-GitHub-Api-Version: 2026-03-10' \
    "repos/${REPO}/rulesets?includes_parents=false&targets=branch&per_page=100" \
    --jq ".[] | select(.name == \"${RULESET_NAME}\") | .id"
)

if [[ "${COMMAND}" == bootstrap ]]; then
  ((${#ids[@]} == 0)) || die 'production parity ruleset already exists'
  payload="$(mktemp)"
  trap 'rm -f "${payload}"' EXIT
  python3 "${RULE_TOOL}" render "${common[@]}" >"${payload}"
  gh api --method POST \
    -H 'Accept: application/vnd.github+json' \
    -H 'X-GitHub-Api-Version: 2026-03-10' \
    "repos/${REPO}/rulesets" --input "${payload}" >/dev/null
  printf 'production parity repository ruleset created\n'
  exit 0
fi

((${#ids[@]} == 1)) || die 'expected exactly one production parity ruleset'
ruleset="$(mktemp)"
trap 'rm -f "${ruleset}"' EXIT
gh api \
  -H 'Accept: application/vnd.github+json' \
  -H 'X-GitHub-Api-Version: 2026-03-10' \
  "repos/${REPO}/rulesets/${ids[0]}?includes_parents=false" >"${ruleset}"
python3 "${RULE_TOOL}" verify "${common[@]}" "${ruleset}"
