#!/usr/bin/env bash
set -euo pipefail
cd -- "$(dirname -- "$0")"

sha256sum --check runner.sha256
(cd source && sha256sum --check ../source-files.sha256)
python3 audit_results.py
