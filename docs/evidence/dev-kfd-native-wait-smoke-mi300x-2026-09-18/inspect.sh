#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-kfd-native-wait-20260918.* && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(realpath -e -- "$owned") == "$owned" ]]
[[ $(cat "$owned/owner") == fe2o3-native-wait-fcd5a89a113c6538338598bfcdb6769fb5c06042 ]]
date -u +%FT%T.%NZ
cd "$owned"
sha256sum prepare.sh guard.sh run.sh cleanup.sh inspect.sh owner
df -B1 "$owned"
du -sh "$owned"
for variable in HSA_XNACK HSA_ENABLE_SDMA HSA_ENABLE_PEER_SDMA HSA_OVERRIDE_GFX_VERSION HSA_TOOLS_LIB LD_PRELOAD LD_LIBRARY_PATH; do
    printf 'inherited_environment %s=%s\n' "$variable" "${!variable-<unset>}"
done
date -u +%FT%T.%NZ
