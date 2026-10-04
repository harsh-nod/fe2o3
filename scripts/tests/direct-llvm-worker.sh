#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
driver="$repo_root/scripts/test-direct-llvm-worker.sh"
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT

fake_toolchain="$temporary/nightly-2026-04-03-fake"
mkdir -p "$fake_toolchain/bin" "$fake_toolchain/lib"
cp -- /usr/bin/true "$fake_toolchain/bin/cargo"
cp -- /usr/bin/true "$fake_toolchain/bin/rustc"
fake_cargo="$fake_toolchain/bin/cargo"
fake_rustc="$fake_toolchain/bin/rustc"
read -r cargo_sha256 _ < <(sha256sum -- "$fake_cargo")
read -r rustc_sha256 _ < <(sha256sum -- "$fake_rustc")
read -r manifest_sha256 _ < <(sha256sum -- "$repo_root/rust-toolchain.toml")
source_commit=$(git -C "$repo_root" rev-parse HEAD)

run_probe() {
  local build_dir=$1
  set +e
  probe_output=$(
    "$driver" "$build_dir" /missing/llvm /missing/lld 22.0.0git \
      /missing/build-id gfx942 "$fake_cargo" "$cargo_sha256" \
      "$fake_rustc" "$rustc_sha256" nightly-2026-04-03 \
      "$manifest_sha256" "$source_commit" 2>&1
  )
  probe_status=$?
  set -e
}

fresh_build="$temporary/fresh-build"
run_probe "$fresh_build"
if ((probe_status == 0)) ||
  [[ $probe_output != *"pinned Cargo executable returned an invalid version"* ]] ||
  [[ $probe_output == *"native integration: PASS"* ]] || [[ -e $fresh_build ]]; then
  printf 'fake Cargo probe failed (status=%d):\n%s\n' \
    "$probe_status" "$probe_output" >&2
  exit 1
fi

reused_build="$temporary/reused-build"
mkdir -p "$reused_build/direct-llvm-integration"
printf 'stale false-pass payload\n' \
  >"$reused_build/direct-llvm-integration/pinned-worker.hsaco"
run_probe "$reused_build"
if ((probe_status != 73)) ||
  [[ $probe_output != *"BUILD_DIR must not already exist"* ]] ||
  [[ $probe_output == *"native integration: PASS"* ]]; then
  printf 'reused build probe failed (status=%d):\n%s\n' \
    "$probe_status" "$probe_output" >&2
  exit 1
fi


printf 'direct LLVM worker orchestration regression probes passed\n'
