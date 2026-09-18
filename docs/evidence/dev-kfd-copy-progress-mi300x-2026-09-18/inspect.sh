#!/usr/bin/env bash
set -euo pipefail
owned=${1:?owned absolute directory}
[[ "$owned" == /tmp/fe2o3-kfd-copy-progress-20260918.rrF6He9N && -O "$owned" && -d "$owned" && ! -L "$owned" ]]
[[ $(realpath -e -- "$owned") == "$owned" ]]
date -u +%FT%T.%NZ
cd "$owned"
sha256sum prepare.sh guard.sh run.sh cleanup.sh owner
df -B1 "$owned"
du -sh "$owned"
for variable in HSA_XNACK HSA_ENABLE_SDMA HSA_ENABLE_PEER_SDMA HSA_OVERRIDE_GFX_VERSION HSA_TOOLS_LIB LD_PRELOAD LD_LIBRARY_PATH; do
    printf 'inherited_environment %s=%s\n' "$variable" "${!variable-<unset>}"
done
date -u +%FT%T.%NZ
