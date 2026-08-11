#!/usr/bin/bash
set -euf

case ${BASH_SOURCE[0]} in
  /*) controller=${BASH_SOURCE[0]%/*}/../gfx942_compiler_evidence.py ;;
  *) controller=$PWD/${BASH_SOURCE[0]%/*}/../gfx942_compiler_evidence.py ;;
esac

exec -c /usr/bin/python3.12 "$controller" --self-test
