#!/usr/bin/env bash

set -Eeuo pipefail
SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
export LC_ALL=C
exec python3 "${SCRIPT_DIR}/parity-signed-evidence.py" "$@"
