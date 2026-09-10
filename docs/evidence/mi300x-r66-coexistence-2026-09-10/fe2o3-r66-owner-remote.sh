#!/usr/bin/env bash
set -Eeuo pipefail
umask 077
ulimit -c 0
[[ "$#" == 2 ]]
stage="$1"
commit="$2"
[[ "$stage" =~ ^/dev/shm/fe2o3-r66-owner\.[A-Za-z0-9]+$ ]]
[[ "$commit" =~ ^[0-9a-f]{40}$ ]]
[[ -d "$stage" && ! -L "$stage" ]]
[[ "$(stat -c '%u:%a' "$stage")" == "$(id -u):700" ]]
cleanup() {
  result=$?
  trap - EXIT
  if [[ -n "${runner_pid:-}" && -n "${runner_start:-}" && -r "/proc/$runner_pid/stat" ]] &&
      [[ "$(awk '{print $22}' "/proc/$runner_pid/stat")" == "$runner_start" ]]; then
    kill -TERM "$runner_pid" 2>/dev/null || true
    deadline=$((SECONDS + 120))
    while [[ -r "/proc/$runner_pid/stat" ]] &&
        [[ "$(awk '{print $22}' "/proc/$runner_pid/stat")" == "$runner_start" ]] &&
        [[ "$(awk '{print $3}' "/proc/$runner_pid/stat")" != Z ]]; do
      if (( SECONDS >= deadline )); then
        printf 'Owned runner did not stop; preserving stage for recovery: %s PID=%s start=%s\n' \
          "$stage" "$runner_pid" "$runner_start" >&2
        exit 1
      fi
      sleep 1
    done
    wait "$runner_pid" || true
  fi
  printf '%s\n' "$result" > "$stage/exit-status.txt" || result=1
  tar -cf - -C "$stage" output run.log clone.log process-identities.txt exit-status.txt || result=1
  find "$stage" -type d -exec chmod u+rwx {} + || result=1
  find "$stage" -depth -delete || result=1
  exit "$result"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
mkdir "$stage/output" "$stage/staging"
touch "$stage/run.log" "$stage/clone.log" "$stage/process-identities.txt"
printf 'wrapper %s %s\n' "$$" "$(awk '{print $22}' "/proc/$$/stat")" >> "$stage/process-identities.txt"
git clone --no-local --branch codex/r65-runtime-drain-versions "$stage/source.bundle" "$stage/checkout" > "$stage/clone.log" 2>&1
[[ "$(git -C "$stage/checkout" rev-parse HEAD)" == "$commit" ]]

# The installed toolchain is read-only; all task cache/build writes are private.
toolchain=/home/harsh/.rustup/toolchains/nightly-2026-04-03-x86_64-unknown-linux-gnu
[[ -x "$toolchain/bin/rustc" && -x "$toolchain/bin/cargo" ]]
[[ -d "$toolchain/lib/rustlib/x86_64-unknown-linux-musl/lib" ]]
for component in cargo rustc rust-std rustc-dev rust-analyzer-preview rustfmt-preview clippy-preview; do
  grep -qx "$component-x86_64-unknown-linux-gnu" "$toolchain/lib/rustlib/components"
done
grep -qx rust-src "$toolchain/lib/rustlib/components"
grep -qx rust-std-x86_64-unknown-linux-musl "$toolchain/lib/rustlib/components"
case ",$(findmnt -n -o OPTIONS --target "$stage")," in
  *,noexec,*) printf 'Private build stage is noexec\n' >&2; exit 1 ;;
esac
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
python3 "$stage/checkout/benchmarks/runtime_gfx942/run-r66-coexistence-mi300x.py" \
  --repo "$stage/checkout" --allowed-signers "$stage/allowed-signers" \
  --output-dir "$stage/output" --staging-parent "$stage/staging" \
  --build-home "$home" --rocm-path /opt/rocm > "$stage/run.log" 2>&1 &
runner_pid=$!
runner_start=$(awk '{print $22}' "/proc/$runner_pid/stat")
printf 'runner %s %s\n' "$runner_pid" "$runner_start" >> "$stage/process-identities.txt"
wait "$runner_pid"
runner_pid=
