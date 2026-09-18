#!/usr/bin/env bash
set -euo pipefail
owned=$(mktemp -d /tmp/fe2o3-kfd-native-wait-5d70cb0a-20260918.XXXXXXXX)
printf 'fe2o3-native-wait-5d70cb0a6e16fb265fe224690274fdb0be2b0055\n' > "$owned/owner"
printf '%s\n' "$owned"
