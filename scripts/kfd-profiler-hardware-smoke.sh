#!/usr/bin/env bash

set -Eeuo pipefail

if [[ "${FE2O3_ALLOW_GPU_SMOKE:-}" != 1 ]]; then
  printf '%s\n' 'refusing direct-KFD profiler smoke without FE2O3_ALLOW_GPU_SMOKE=1' >&2
  exit 2
fi
if [[ "${FE2O3_TARGET:-}" != gfx942 && "${FE2O3_TARGET:-}" != gfx942:xnack- ]]; then
  printf 'direct-KFD profiler smoke requires FE2O3_TARGET=gfx942[:xnack-], got %s\n' \
    "${FE2O3_TARGET:-}" >&2
  exit 2
fi

readonly ROOT="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)"
readonly OUTPUT="${ROOT}/target/fe2o3/kfd-profiler-hardware-smoke"
rm -rf -- "${OUTPUT}"
mkdir -p -m 700 -- "${OUTPUT}"

scope="$({
  cat /proc/sys/kernel/random/uuid
  cat /proc/sys/kernel/random/uuid
} | tr -cd -- '0-9a-f')"
readonly scope
if [[ ! "${scope}" =~ ^[0-9a-f]{64}$ || "${scope}" =~ ^0+$ ]]; then
  printf '%s\n' 'kernel UUID source did not produce a nonzero 256-bit capture scope' >&2
  exit 2
fi

cd -- "${ROOT}"
cargo run --locked -p fe2o3-runtime --features hardware-qualification \
  --example gfx942-runtime-vecadd-benchmark -- \
  auto 1 1 3 "${scope}" "${OUTPUT}/capture.json"

printf '%s\n' \
  '{"schema":"fe2o3-agent-kfd-profiler-request-v1","request_id":1,"operation":"inspect_capture"}' \
  | cargo run --locked --quiet -p fe2o3-profiler-protocol \
      --bin fe2o3-kfd-profiler-query -- "${OUTPUT}/capture.json" \
      >"${OUTPUT}/query.jsonl"

python3 -I - "${OUTPUT}/query.jsonl" <<'PY'
import json
import pathlib
import sys

rows = pathlib.Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
if len(rows) != 1:
    raise SystemExit("profiler smoke query did not return exactly one response")
response = json.loads(rows[0])
body = response.get("body", {})
coverage = body.get("coverage", {})
device = body.get("device", {})
identity = body.get("capture_identity", {})
if response.get("schema") != "fe2o3-agent-kfd-profiler-response-v1":
    raise SystemExit("profiler smoke returned the wrong response schema")
if response.get("request_id") != 1 or body.get("response") != "capture":
    raise SystemExit("profiler smoke did not return the requested capture record")
if coverage.get("complete_runtime_operation_history") is not True:
    raise SystemExit("profiler smoke capture is not complete")
if coverage.get("dropped_events") != 0 or coverage.get("observed_events", 0) == 0:
    raise SystemExit("profiler smoke capture lost or omitted runtime events")
if device.get("target_profile") != "gfx942:xnack-" or device.get("wave_width") != 64:
    raise SystemExit("profiler smoke capture did not bind the admitted gfx942 Wave64 target")
if identity.get("byte_len") != pathlib.Path(sys.argv[1]).with_name("capture.json").stat().st_size:
    raise SystemExit("profiler smoke capture identity length does not match its file")
print(
    "direct-KFD profiler smoke: "
    f"capture={identity.get('digest')} bytes={identity.get('byte_len')} "
    f"events={coverage.get('observed_events')} dropped=0 status=passed"
)
PY
