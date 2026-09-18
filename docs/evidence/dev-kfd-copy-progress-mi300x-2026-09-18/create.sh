#!/usr/bin/env bash
set -euo pipefail
owned=$(mktemp -d /tmp/fe2o3-kfd-copy-progress-20260918.XXXXXXXX)
printf 'fe2o3-copy-progress-3e12ef82bbb41fb116afbb7ddf7cffdad735ec7d\n' > "$owned/owner"
printf '%s\n' "$owned"
