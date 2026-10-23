#!/usr/bin/env bash
set -euo pipefail
if [[ $# -gt 1 ]]; then
    printf 'usage: %s [tile|simt]\n' "$0" >&2
    exit 2
fi
export FE2O3_GFX950_ADVANCED_MODE=${1:-${FE2O3_GFX950_ADVANCED_MODE:-tile}}
exec "$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)/run-gfx950.sh" kernel-content-sparse-attention
