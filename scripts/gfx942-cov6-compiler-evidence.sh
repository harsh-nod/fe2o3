#!/usr/bin/bash
set -euf

if [[ $# -eq 1 && $1 == --self-test ]]; then
  mode=self-test
elif [[ $# -eq 2 ]]; then
  mode=run
else
  printf 'usage: %s [--self-test | RUN_ROOT EVIDENCE_ROOT]\n' "$0" >&2
  exit 64
fi

case ${BASH_SOURCE[0]} in
  /*) controller=${BASH_SOURCE[0]%/*}/gfx942_compiler_evidence.py ;;
  *) controller=$PWD/${BASH_SOURCE[0]%/*}/gfx942_compiler_evidence.py ;;
esac

# The absolute shebang selects the measured Bash. The absolute env clears the
# inherited environment; both Python controls independently prohibit bytecode.
if [[ $mode == self-test ]]; then
  exec /usr/bin/env -i PYTHONDONTWRITEBYTECODE=1 \
    /usr/bin/python3.12 -B "$controller" --self-test
fi
exec /usr/bin/env -i PYTHONDONTWRITEBYTECODE=1 \
  /usr/bin/python3.12 -B "$controller" "$1" "$2"
