#!/usr/bin/env bash
set -euo pipefail

archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
out="$archive/closure"
mkdir -p -- "$out"
cd -- "$root"
remote=$(< "$archive/native/create-scratch.log")
[[ $remote == /tmp/fe2o3-r126-native-967a62df.CoT8qwY4 ]]
receipt="$root/docs/evidence/dev-r126-auxiliary-release-2026-09-16/final/musl-runtime-binary.sha256"

record() {
    local expected=$1 name=$2 status
    shift 2
    [[ ! -e "$out/$name.command" ]]
    printf '%q ' "$@" > "$out/$name.command"
    printf '\n' >> "$out/$name.command"
    date -u +%FT%T.%NZ > "$out/$name.started"
    if "$@" > "$out/$name.log" 2>&1; then status=0; else status=$?; fi
    date -u +%FT%T.%NZ > "$out/$name.finished"
    printf '%s\n' "$status" > "$out/$name.exit"
    printf '%s exit=%s expected=%s\n' "$name" "$status" "$expected"
    [[ $status == "$expected" ]]
}

record 0 remote-hash ssh mi300x sha256sum "$remote/runtime-tests"
cmp "$archive/native/remote-hash-before.log" "$out/remote-hash.log"
record 0 scratch-owner ssh mi300x stat -c "'%U %a'" "$remote"
[[ $(< "$out/scratch-owner.log") == 'harsh 700' ]]
record 0 scratch-members ssh mi300x find "$remote" -mindepth 1 -maxdepth 1 -printf "'%y %u %f\\n'"
[[ $(< "$out/scratch-members.log") == 'f harsh runtime-tests' ]]
record 1 process-absent ssh mi300x fuser "$remote/runtime-tests"
[[ ! -s "$out/process-absent.log" ]]
record 0 remove-binary ssh mi300x rm -- "$remote/runtime-tests"
record 0 remove-directory ssh mi300x rmdir -- "$remote"
record 0 scratch-absent ssh mi300x test ! -e "$remote"
record 0 scratch-link-absent ssh mi300x test ! -L "$remote"
record 0 final-use ssh mi300x /opt/rocm/bin/rocm-smi --showuse --showmeminfo vram --json
record 0 final-identity ssh mi300x /opt/rocm/bin/rocm-smi --showuniqueid --showbus --json
record 0 final-pids ssh mi300x /opt/rocm/bin/rocm-smi --showpidgpus
record 0 binary-after sha256sum --check "$receipt"
record 0 source-after git rev-parse HEAD
cmp "$archive/native/source.log" "$out/source-after.log"
record 0 source-clean git diff --exit-code -- crates
record 0 source-index-clean git diff --cached --exit-code -- crates
