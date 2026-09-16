#!/usr/bin/env bash
set -euo pipefail

manifest=$(cd -- "$(dirname -- "$0")" && pwd)/source-files.sha256
root=${1:?usage: verify-downstream.sh CHECKOUT OUTPUT_DIRECTORY}
out=${2:?usage: verify-downstream.sh CHECKOUT OUTPUT_DIRECTORY}
out=$(realpath -- "$out")
cd -- "$root"
sha256sum --check "$manifest" > "$out/downstream-source-before.log"
# Enable the actual HSA adapter rather than its default inert compatibility marker.
command=(cargo check --locked --offline -p fe2o3-hsa-runtime -p fe2o3-sim-runtime --all-targets --features fe2o3-hsa-runtime/qualification-legacy-hsa-runtime)
printf '%q ' "${command[@]}" > "$out/downstream-api.command"
printf '\n' >> "$out/downstream-api.command"
date -u +%FT%T.%NZ > "$out/downstream-api.started"
if "${command[@]}" > "$out/downstream-api.log" 2>&1; then status=0; else status=$?; fi
date -u +%FT%T.%NZ > "$out/downstream-api.finished"
printf '%s\n' "$status" > "$out/downstream-api.exit"
printf 'downstream-api exit=%s\n' "$status"
sha256sum --check "$manifest" > "$out/downstream-source-after.log"
exit "$status"
