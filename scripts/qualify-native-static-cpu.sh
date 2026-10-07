#!/usr/bin/env bash
# Static image and private-mount deployment contracts, never service activation.
set -Eeuo pipefail
umask 077
test "$EUID" -ne 0
root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
export FE2O3_STATIC_PROOF_CUSTODIAN_TARGET_DIR
FE2O3_STATIC_PROOF_CUSTODIAN_TARGET_DIR=$(realpath -m -- \
  "${FE2O3_STATIC_PROOF_CUSTODIAN_TARGET_DIR:-$root/target/static-proof-custodian}")
export FE2O3_HIP_SYS_DISABLE=1
export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-2}"
readonly target=x86_64-unknown-linux-musl
readonly images="$FE2O3_STATIC_PROOF_CUSTODIAN_TARGET_DIR/$target/release"
cd -- "$root"
bash scripts/build-static-proof-custodian.sh

# Select the actual libtest artifact from Cargo's structured output, not a stale
# filename glob. pipefail rejects a compiler failure even if it emitted an image.
binaries=$(CARGO_TARGET_DIR="$FE2O3_STATIC_PROOF_CUSTODIAN_TARGET_DIR" \
  cargo test --locked --release --target "$target" -p fe2o3-proof-custodian \
    -p fe2o3-compiler-execution-coordinator -p fe2o3-protected-service-spawn \
    --lib --no-run --message-format=json | \
  python3 -I -c '
import json, pathlib, sys
root, images = map(pathlib.Path, sys.argv[1:])
names = ("fe2o3_proof_custodian", "fe2o3_compiler_execution_coordinator", "fe2o3_protected_service_spawn")
found, finished = {}, 0
for line in sys.stdin:
    event = json.loads(line)
    if event.get("reason") == "build-finished":
        assert event.get("success") is True, "Cargo build failed"
        finished += 1
    if event.get("reason") == "compiler-artifact" and event.get("executable"):
        target = event["target"]
        name = target["name"]
        assert name in names and name not in found
        assert target["kind"] == ["lib"]
        assert pathlib.Path(target["src_path"]) == root / "crates" / name.replace("_", "-") / "src/lib.rs"
        assert event["profile"]["test"] is True
        path = pathlib.Path(event["executable"]).resolve(strict=True)
        assert path.is_relative_to(images.resolve(strict=True))
        assert "\n" not in str(path)
        found[name] = path
assert finished == 1 and set(found) == set(names), "missing or duplicate exact libtest artifact"
for name in names: print(found[name])
' "$root" "$images")
mapfile -t binaries <<< "$binaries"
test "${#binaries[@]}" -eq 3
binary=${binaries[0]}

run_group() {
  local image=$1 filter=$2 count=$3 listing
  listing=$(timeout --kill-after=1s 10s "$image" --list "$filter")
  [[ "$listing" == *$'\n\n'"$count tests, 0 benchmarks" ||
     ( "$count" == 1 && "$listing" == *$'\n\n1 test, 0 benchmarks' ) ]]
  timeout --signal=TERM --kill-after=5s 120s "$image" "$filter" --nocapture --test-threads=1
}
run_group "${binaries[1]}" native_entrypoint::tests::phase_control:: 3
run_group "${binaries[1]}" native_entrypoint::fixed_phase::tests:: 2
run_group "${binaries[2]}" process_reaper::native::phase_tests:: 6
run_group "${binaries[2]}" syscall::proof_controller_tests::polling_ 2
run_group "${binaries[2]}" creator_scope::tests::original_pool_shutdown_is_the_only_nonfatal_retirement 1

manager="$images/fe2o3-native-application-manager"
manager_pin=$(sha256sum -- "$manager")
manager_pin=${manager_pin%% *}
observation=$(mktemp -d -- "${TMPDIR:-/tmp}/fe2o3-native-packaged-refusal.XXXXXXXX")
trap 'rm -rf -- "$observation"' EXIT
if ! python3 scripts/qualify_native_manager_cpu.py --image "$manager" --image-sha256 "$manager_pin" \
  --output "$observation/result"; then
  if [[ -f "$observation/result/report.json" ]]; then cat -- "$observation/result/report.json"; fi
  if [[ -f "$observation/result/manager.log" ]]; then cat -- "$observation/result/manager.log"; fi
  exit 1
fi
cat -- "$observation/result/report.json" "$observation/result/manager.log"
libraries="$(rustc --print sysroot)/lib"
controller="$images/fe2o3-native-application-proof-controller"
for mode in run run-manager; do
  sudo -n -- bash crates/fe2o3-proof-custodian/tests/qualify-native-proof-deployment.sh \
    "$mode" "$binary" "$libraries" "$controller"
done
printf '%s\n' 'Native static CPU refusal/controller contracts passed; joinedProtectedPhaseQualified=false rootServiceCleanupQualified=false; no service or GPU qualification performed.'
