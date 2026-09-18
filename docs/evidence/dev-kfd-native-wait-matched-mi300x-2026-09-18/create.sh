#!/usr/bin/env bash
set -euo pipefail
owned=$(mktemp -d /tmp/fe2o3-kfd-native-wait-matched-20260918.XXXXXXXX)
printf 'fe2o3-native-wait-ecad7245fa9fa051daf3a9cb3cd82724da5446eb\n' > "$owned/owner"
printf '%s\n' "$owned"
