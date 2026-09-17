#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
names=(source-base rustc-version cargo-version source-before runner-policy runner-inventory gnu musl rosters binaries clippy fmt no-default unsafe-policy doctests commit-self-test commit-campaign global-verus source-after binaries-after)
previous=0
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    [[ $(< "$archive/raw/$name.exit") == 0 ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    ((started >= previous && finished >= started))
    previous=$finished
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
cd -- "$root"
base=04d9f3ca37cd17536901d4d7cab405bf06f54454
[[ $(git rev-parse HEAD) == "$base" && $(< "$archive/raw/source-base.log") == "$base" ]]
[[ $(sha256sum "$archive/source-files.sha256" | awk '{print $1}') == 6da0df0b2e545eeecb9a47d5088b6a2f0bf2cfbec9c1bd3c680b4a21041c375d ]]
[[ $(sha256sum "$archive/source.patch" | awk '{print $1}') == 2f072604b80147c8f82e689dac78cb593f915a3c87aaf1eb3a40e44375df1434 ]]
mapfile -t sources < "$archive/source-files.list"
[[ ${#sources[@]} == 11 && $(printf '%s\n' "${sources[@]}" | LC_ALL=C sort -u | wc -l) == 11 ]]
cmp <(sha256sum -- "${sources[@]}") "$archive/source-files.sha256"
cmp <(git diff HEAD --binary -- "${sources[@]}") "$archive/source.patch"
git diff --quiet -- "${sources[@]}"
scope=(. ':(exclude)docs/evidence/dev-v4j3-reader-commits-2026-09-17' ':(exclude)docs/evidence/dev-single-packet-copy-engines-mi300x-2026-09-17')
cmp <(printf '%s\n' "${sources[@]}" | LC_ALL=C sort) <(git diff HEAD --name-only -- "${scope[@]}" | LC_ALL=C sort)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
sha256sum --check --quiet "$archive/source-files.sha256"
sha256sum --check --quiet "$archive/raw/binaries.log"
cmp "$archive/raw/source-before.log" "$archive/raw/source-after.log"
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
[[ $(wc -l < "$archive/gnu.complete-roster") == 2146 ]]
[[ $(wc -l < "$archive/added-tests.list") == 2 ]]
[[ $(wc -l < "$archive/source-files.list") == 11 ]]
[[ $(wc -l < "$archive/raw/binaries.log") == 6 ]]
actual=$(tail -n 1 "$archive/raw/global-verus.log" | sha256sum | awk '{print $1}')
[[ "$actual" == $(< crates/fe2o3-runtime-model/verus/pins/TRANSCRIPT_SHA256) ]]
grep -Fx 'READ_COMMIT_OK obligations=127 inherited=103 new=24 executable_mutations=15' "$archive/raw/commit-campaign.log"
grep -Fx 'READ_COMMIT_OK obligations=127 inherited=103 new=24 executable_mutations=15' "$archive/raw/global-verus.log"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
