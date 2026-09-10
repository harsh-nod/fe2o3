#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
ulimit -c 0
stage="$1"
commit="$2"
[[ "$stage" =~ ^/dev/shm/fe2o3-r64-owner\.[A-Za-z0-9]+$ ]]
[[ "$commit" =~ ^[0-9a-f]{40}$ ]]
[[ -d "$stage" && ! -L "$stage" ]]
[[ "$(stat -c '%u:%a' "$stage")" == "$(id -u):700" ]]
cleanup() {
  result=$?
  trap - EXIT
  if [[ -n "${runner_pid:-}" ]] && kill -0 "$runner_pid" 2>/dev/null; then
    kill -TERM "$runner_pid" 2>/dev/null || true
    wait "$runner_pid" || true
  fi
  printf '%s\n' "$result" > "$stage/exit-status.txt" || result=1
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
git clone --no-local --branch codex/r64-runtime-admission-executors "$stage/source.bundle" "$stage/checkout" > "$stage/clone.log" 2>&1
[[ "$(git -C "$stage/checkout" rev-parse HEAD)" == "$commit" ]]

# Offline Cargo may resolve dev dependencies even for this guarded example.
# Copy the registry into task-owned storage; never add to the shared cache.
home="$stage/build-home"
available=$(df --output=avail -B1 "$stage" | tail -n1)
(( available > 8 * 1024 * 1024 * 1024 ))
mkdir -p "$home/.cargo" "$home/.rustup"
cp -a /home/harsh/.cargo/registry "$home/.cargo/registry"
cp -a /home/harsh/.cargo/git "$home/.cargo/git"
ln -s /home/harsh/.cargo/bin "$home/.cargo/bin"
cp /home/harsh/.rustup/settings.toml "$home/.rustup/settings.toml"
ln -s /home/harsh/.rustup/toolchains "$home/.rustup/toolchains"
registry=index.crates.io-1949cf8c6b5b557f
[[ "$(sha256sum "$stage/futures-executor-0.3.34.crate" | cut -d ' ' -f1)" == 031b47cf1a3c6cc8bc2fc76cd437f521619387907d469316e7c0bc278f1f5432 ]]
cp "$stage/futures-executor-0.3.34.crate" "$home/.cargo/registry/cache/$registry/"
cp "$stage/futures-executor-index" "$home/.cargo/registry/index/$registry/.cache/fu/tu/futures-executor"
export PYTHONDONTWRITEBYTECODE=1
python3 "$stage/checkout/benchmarks/runtime_gfx942/run-r63-graph-mi300x.py" \
  --repo "$stage/checkout" --allowed-signers "$stage/allowed-signers" \
  --output-dir "$stage/output" --staging-parent "$stage/staging" \
  --build-home "$home" --rocm-path /opt/rocm > "$stage/run.log" 2>&1 &
runner_pid=$!
wait "$runner_pid"
runner_pid=
