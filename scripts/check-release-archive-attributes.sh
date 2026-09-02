#!/usr/bin/env bash

set -Eeuo pipefail
export LC_ALL=C

commit="${1:-}"
[[ "${commit}" =~ ^[0-9a-f]{40}$ ]] || {
  printf '%s\n' 'release archive attributes: expected one full commit SHA' >&2
  exit 2
}
git cat-file -e "${commit}^{commit}"

git ls-tree -r -t -z --name-only "${commit}" |
  git check-attr --source="${commit}" -z --stdin \
    export-ignore export-subst |
  while IFS= read -r -d '' path &&
    IFS= read -r -d '' attribute &&
    IFS= read -r -d '' value; do
    case "${value}" in
      unspecified | unset) ;;
      *)
        printf 'release archive attributes: %s=%s is active for %q\n' \
          "${attribute}" "${value}" "${path}" >&2
        exit 2
        ;;
    esac
  done

printf '%s\n' 'release archive attributes are inert'
