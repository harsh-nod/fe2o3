#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
prefix=${1:-corrected}
[[ $prefix == corrected || $prefix == corrected-rerun ]]
record() { bash "$archive/record.sh" "$prefix-$1" "${@:2}"; }
[[ $(< "$archive/raw/corrected-musl-build.exit") == 0 ]]
binary=$(jq -Rr '
    fromjson? | select(.reason == "compiler-artifact" and
    .target.name == "fe2o3_runtime" and .profile.test == true) | .executable // empty
' "$archive/raw/corrected-musl-build.log")
[[ -n $binary && $binary != *$'\n'* && -x $binary ]]
record native-source-before sha256sum --check "$archive/corrected-source-files.sha256"
record native-binary sha256sum "$binary"
expected=$(awk '{print $1}' "$archive/raw/$prefix-native-binary.log")
ssh_args=(-o BatchMode=yes -o ConnectTimeout=10)
record remote-create ssh "${ssh_args[@]}" mi300x mktemp -d /tmp/fe2o3-c4-harsh-20260917.XXXXXX
scratch=$(< "$archive/raw/$prefix-remote-create.log")
[[ $scratch =~ ^/tmp/fe2o3-c4-harsh-20260917\.[a-zA-Z0-9]+$ ]]
cleanup() {
    record remote-remove-files ssh "${ssh_args[@]}" mi300x rm -f -- "$scratch/runtime" "$scratch/native-guard.sh" &&
    record remote-remove-directory ssh "${ssh_args[@]}" mi300x rmdir -- "$scratch" &&
    record remote-absence ssh "${ssh_args[@]}" mi300x test ! -e "$scratch"
}
trap cleanup EXIT
record remote-binary-upload scp "${ssh_args[@]}" "$binary" "mi300x:$scratch/runtime"
record remote-guard-upload scp "${ssh_args[@]}" "$archive/native-guard-corrected.sh" "mi300x:$scratch/native-guard.sh"
record remote-bytes ssh "${ssh_args[@]}" mi300x sha256sum "$scratch/runtime" "$scratch/native-guard.sh"
guard_hash=$(sha256sum "$archive/native-guard-corrected.sh" | cut -d ' ' -f 1)
diff -u <(printf '%s  %s\n' "$expected" "$scratch/runtime" "$guard_hash" "$scratch/native-guard.sh") \
    "$archive/raw/$prefix-remote-bytes.log"
record remote-inventory-final ssh "${ssh_args[@]}" mi300x ls -la -- "$scratch"
records=(native-c4-cold native-c4-bootstrap native-i2-cold native-i2-bootstrap)
names=(generated_native_cold_full_roster_readback generated_native_bootstrap_full_roster_readback generated_native_cold_issue_complete_readback_retire generated_native_bootstrap_issue_complete_readback_retire)
for index in 0 1 2 3; do
    record "${records[$index]}" ssh "${ssh_args[@]}" mi300x bash "$scratch/native-guard.sh" "$expected" \
        "kfd_backend::generated_adoption::tests::native::${names[$index]}"
done
record native-binary-after sha256sum --check "$archive/raw/$prefix-native-binary.log"
record native-source-after sha256sum --check "$archive/corrected-source-files.sha256"
