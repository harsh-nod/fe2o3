#!/usr/bin/env bash
set -euo pipefail

repo_root=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
driver="$repo_root/scripts/test-direct-llvm-worker.sh"
verifier="$repo_root/scripts/verify-cargo-test-json.py"
temporary=$(mktemp -d)
trap 'rm -rf -- "$temporary"' EXIT

true_bin=$(realpath -e -- /usr/bin/true)
read -r true_sha256 _ < <(sha256sum -- "$true_bin")
read -r manifest_sha256 _ < <(sha256sum -- "$repo_root/rust-toolchain.toml")
source_commit=$(git -C "$repo_root" rev-parse HEAD)

run_probe() {
  local build_dir=$1
  set +e
  probe_output=$(
    "$driver" "$build_dir" /missing/llvm /missing/lld 22.0.0git \
      /missing/build-id gfx942 "$true_bin" "$true_sha256" \
      "$true_bin" "$true_sha256" nightly-2026-04-03 \
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

valid_json="$temporary/valid.jsonl"
printf '%s\n' \
  '{"reason":"compiler-artifact","target":{"name":"direct_llvm_worker_integration","kind":["test"]},"executable":"/tmp/test"}' \
  '{"reason":"build-finished","success":true}' \
  '{"type":"suite","event":"started","test_count":1}' \
  '{"type":"test","event":"started","name":"real_worker_links_mixed_inputs_through_pinned_supervision"}' \
  '{"type":"test","name":"real_worker_links_mixed_inputs_through_pinned_supervision","event":"ok"}' \
  '{"type":"suite","event":"ok","passed":1,"failed":0,"ignored":0,"measured":0,"filtered_out":0}' \
  >"$valid_json"
python3 "$verifier" "$valid_json" \
  --test-target direct_llvm_worker_integration \
  --test-name real_worker_links_mixed_inputs_through_pinned_supervision

empty_json="$temporary/empty.jsonl"
: >"$empty_json"
if python3 "$verifier" "$empty_json" \
  --test-target direct_llvm_worker_integration \
  --test-name real_worker_links_mixed_inputs_through_pinned_supervision; then
  printf 'empty Cargo evidence was accepted\n' >&2
  exit 1
fi

printf 'direct LLVM worker orchestration regression probes passed\n'
