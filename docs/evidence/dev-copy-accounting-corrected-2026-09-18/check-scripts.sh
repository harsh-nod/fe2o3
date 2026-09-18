#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
ruff check "$archive/verify.py" "$archive/binaries.py" "$archive/source.py"
ruff format --check "$archive/verify.py" "$archive/binaries.py" "$archive/source.py"
shellcheck "$archive/record.sh" "$archive/qualify-cpu.sh" "$archive/seal.sh" "$archive/check-scripts.sh"
