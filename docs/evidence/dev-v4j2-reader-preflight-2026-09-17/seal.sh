#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
cd -- "$root"
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
names=(source-base rustc-version cargo-version source-before runner-policy runner-inventory gnu musl rosters binaries clippy fmt no-default unsafe-policy doctests reader-self-test reader-campaign global-verus source-after binaries-after)
previous=0
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    [[ $(< "$archive/raw/$name.exit") == 0 ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    (( started >= previous && finished >= started ))
    previous=$finished
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
[[ -z $(find "$archive" -type l -print) ]]
grep -Fx 'READ_PREFLIGHT_OK obligations=103 inherited=69 new=34 executable_mutations=21' "$archive/raw/reader-campaign.log"
mapfile -t transcripts < <(grep '^FE2O3_RUNTIME_MODEL_VERUS_OK ' "$archive/raw/global-verus.log")
[[ ${#transcripts[@]} == 1 ]]
transcript_hash=$(printf '%s\n' "${transcripts[0]}" | sha256sum | awk '{print $1}')
[[ "$transcript_hash" == $(< crates/fe2o3-runtime-model/verus/pins/TRANSCRIPT_SHA256) ]]
sha256sum --check "$archive/source-files.sha256"
sha256sum --check "$archive/raw/binaries.log"
mapfile -t sources < "$archive/source-files.list"
scope=(crates/fe2o3-runtime-model docs/runtime-a1-a2-swarm-current.md docs/runtime-context-generated-read-leases-v1.md docs/runtime-context-read-preflight-v1.md)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
cmp "$archive/source-files.list" <(git diff --name-only ed5b5d64bf95116c21e9bf350c30132ff2bbb524 -- "${scope[@]}")
cmp "$archive/source.patch" <(git diff --binary ed5b5d64bf95116c21e9bf350c30132ff2bbb524 -- "${sources[@]}")
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check SHA256SUMS
