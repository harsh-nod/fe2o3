#!/usr/bin/bash
set -euf

if [[ $# -ne 2 ]]; then
  printf 'usage: %s RUN_ROOT EVIDENCE_ROOT\n' "$0" >&2
  exit 64
fi

case ${BASH_SOURCE[0]} in
  /*) controller=${BASH_SOURCE[0]%/*}/gfx942_compiler_evidence.py ;;
  *) controller=$PWD/${BASH_SOURCE[0]%/*}/gfx942_compiler_evidence.py ;;
esac

# The absolute shebang selects the measured Bash. exec -c removes the inherited
# environment before Python constructs the fixed per-run allowlist.
exec -c /usr/bin/python3.12 "$controller" "$1" "$2"
