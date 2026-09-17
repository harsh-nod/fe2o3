#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
bash "$archive/qualify-host-runtime.sh" gnu
bash "$archive/qualify-host-runtime.sh" musl-no-hip
bash "$archive/qualify-gates.sh"
bash "$archive/record.sh" parser bash "$archive/check-parser.sh"
