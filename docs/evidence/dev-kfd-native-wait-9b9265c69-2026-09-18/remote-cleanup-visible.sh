#!/usr/bin/env bash
set -euo pipefail
stage=$(cd -- "$(dirname -- "$0")" && pwd)
mode=${1:?cleanup mode}
[[ "$mode" == --cleanup || "$mode" == --absence ]]
pids=1442498,1442500,1442501,1442502,1442503,1442504,1442505,1442506,1442519,1442982,1443119,1444339,1444479,1444480,1444653,1446536,1446576,1448902,1449031,1449033,1449084,1449409,1449500,1450746,1450913,1450928,1450949,1450950,1450967
exec ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x /usr/bin/python3 -B - "$mode" "$pids" < "$stage/cleanup-visible.py"
