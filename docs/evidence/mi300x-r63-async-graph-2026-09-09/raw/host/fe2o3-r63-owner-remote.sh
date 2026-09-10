#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
ulimit -c 0
stage="$1"
commit="$2"
[[ "$stage" =~ ^/dev/shm/fe2o3-r63-owner\.[A-Za-z0-9]+$ ]]
[[ "$commit" =~ ^[0-9a-f]{40}$ ]]
cleanup() {
  result=$?
  trap - EXIT
  if [[ -n "${runner_pid:-}" ]] && kill -0 "$runner_pid" 2>/dev/null; then
    kill -TERM "$runner_pid" 2>/dev/null || true
    wait "$runner_pid" || true
  fi
  printf '%s\n' "$result" > "$stage/exit-status.txt"
  tar -cf - -C "$stage" output run.log clone.log exit-status.txt || result=1
  find "$stage" -type d -exec chmod u+rwx {} + || result=1
  find "$stage" -depth -delete || result=1
  exit "$result"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir "$stage/output" "$stage/staging"
touch "$stage/run.log" "$stage/clone.log"
git clone --no-local --branch codex/r63-runtime-graph-v1 "$stage/source.bundle" "$stage/checkout" > "$stage/clone.log" 2>&1
[[ "$(git -C "$stage/checkout" rev-parse HEAD)" == "$commit" ]]
export PYTHONDONTWRITEBYTECODE=1
python3 "$stage/checkout/benchmarks/runtime_gfx942/run-r63-graph-mi300x.py" \
  --repo "$stage/checkout" --allowed-signers "$stage/allowed-signers" \
  --output-dir "$stage/output" --staging-parent "$stage/staging" \
  --build-home /home/harsh --rocm-path /opt/rocm > "$stage/run.log" 2>&1 &
runner_pid=$!
wait "$runner_pid"
runner_pid=
