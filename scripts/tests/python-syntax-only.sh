#!/usr/bin/env bash

set -Eeuo pipefail

if (($# == 0)); then
  printf '%s\n' 'usage: python-syntax-only.sh FILE...' >&2
  exit 2
fi

exec python3 -I -B - "$@" <<'PY'
import ast
import sys
import tokenize

for path in sys.argv[1:]:
    with tokenize.open(path) as source:
        ast.parse(source.read(), filename=path)
PY
