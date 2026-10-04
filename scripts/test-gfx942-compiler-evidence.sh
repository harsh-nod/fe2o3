#!/usr/bin/bash
set -euf

if [[ $# -ne 2 ]]; then
  printf 'usage: %s RUN_ROOT EVIDENCE_ROOT\n' "$0" >&2
  exit 64
fi

case ${BASH_SOURCE[0]} in
  /*) controller=${BASH_SOURCE[0]%/*}/gfx942-cov6-compiler-evidence.sh ;;
  *) controller=$PWD/${BASH_SOURCE[0]%/*}/gfx942-cov6-compiler-evidence.sh ;;
esac

exec "$controller" "$1" "$2"
