#!/usr/bin/env bash
set -euo pipefail
owned=$(mktemp -d /tmp/fe2o3-kfd-native-wait-20260918.XXXXXXXX)
printf '%s\n' fe2o3-native-wait-fcd5a89a113c6538338598bfcdb6769fb5c06042 > "$owned/owner"
printf '%s\n' "$owned"
