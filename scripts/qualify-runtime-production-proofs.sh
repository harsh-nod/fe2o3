#!/usr/bin/env bash
# Signed-source campaigns; each controller retains its own admission and limits.
set -Eeuo pipefail
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
: "${VERUS:?an exact pinned Verus executable is required}"
: "${A2_CAMPAIGN_ROOT:?an existing private campaign directory is required}"
[[ "$VERUS" == /* && -f "$VERUS" && -x "$VERUS" ]]
[[ "$A2_CAMPAIGN_ROOT" == /* && -d "$A2_CAMPAIGN_ROOT" && ! -L "$A2_CAMPAIGN_ROOT" ]]
cd -- "$root"
readonly proof_root=crates/fe2o3-runtime-model/verus

python3 -I -B "$proof_root/check-dispatch-template-prepare.py" \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/template-prepare"
python3 -I -B "$proof_root/check-dispatch-template-preflight.py" \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/template-preflight"
python3 -I -B "$proof_root/check-dispatch-template-bind.py" \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/template-bind"
python3 -I -B "$proof_root/check-dispatch-template-bind.py" --roster \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/template-roster"
python3 -I -B "$proof_root/check-producer-input-preflight.py" \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/producer-preflight"
python3 -I -B "$proof_root/check-retained-pair-routing.py" \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/retained-routing"
python3 -I -B "$proof_root/check-retained-credit-dispatch.py" --campaign \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/retained-credit"
python3 -I -B "$proof_root/qualify-graph-version-ledger-v1.py" \
  --verus "$VERUS" --output "$A2_CAMPAIGN_ROOT/graph-version-ledger"
