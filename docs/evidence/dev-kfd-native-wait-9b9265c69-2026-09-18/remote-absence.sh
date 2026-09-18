#!/usr/bin/env bash
set -euo pipefail
stage=$(cd -- "$(dirname -- "$0")" && pwd)
exec ssh -o BatchMode=yes -o ConnectTimeout=10 mi300x /usr/bin/python3 -B - < "$stage/absence.py"
