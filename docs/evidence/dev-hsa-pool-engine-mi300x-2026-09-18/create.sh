#!/usr/bin/env bash
set -euo pipefail
owned=$(mktemp -d /tmp/fe2o3-hsa-pool-engine-20260918.XXXXXXXX)
printf 'fe2o3-pool-engine-3da2d25ac965afa9845b8ab1246e5fdf13c5d821\n' > "$owned/owner"
printf '%s\n' "$owned"
