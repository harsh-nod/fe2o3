#!/usr/bin/bash
set -euf

case ${BASH_SOURCE[0]} in
  /*) controller=${BASH_SOURCE[0]%/*}/../gfx942_compiler_evidence.py ;;
  *) controller=$PWD/${BASH_SOURCE[0]%/*}/../gfx942_compiler_evidence.py ;;
esac

exec /usr/bin/env -i PYTHONDONTWRITEBYTECODE=1 \
  /usr/bin/python3.12 -B "$controller" --self-test
