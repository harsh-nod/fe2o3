#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
names=(source-base rustc-version cargo-version source-before runner-policy runner-inventory invariant-self-test invariant-campaign gnu musl rosters binaries clippy fmt python-lint python-format no-default unsafe-policy doctests global-verus source-after binaries-after)
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
base=f815d1dc7406851e39e77f488b6bc4b4b3f94ae2
[[ $(git rev-parse HEAD) == "$base" && $(< "$archive/raw/source-base.log") == "$base" ]]
[[ $(sha256sum "$archive/source-files.sha256" | awk '{print $1}') == f9096736466e74a55014fbd165426fbd118c284f593d48e69c818ac0c83bba41 ]]
[[ $(sha256sum "$archive/source.patch" | awk '{print $1}') == 8c285592c643ab9b88a240d22c742ffb42ecce01fd0f1df36e62ff2646b543b4 ]]
mapfile -t sources < "$archive/source-files.list"
[[ ${#sources[@]} == 11 && $(printf '%s\n' "${sources[@]}" | LC_ALL=C sort -u | wc -l) == 11 ]]
cmp <(sha256sum -- "${sources[@]}") "$archive/source-files.sha256"
cmp <(git diff HEAD --binary -- "${sources[@]}") "$archive/source.patch"
git diff --quiet -- "${sources[@]}"
scope=(. ':(exclude)docs/evidence/dev-v4j4-reader-invariant-2026-09-17')
cmp <(printf '%s\n' "${sources[@]}" | LC_ALL=C sort) <(git diff HEAD --name-only -- "${scope[@]}" | LC_ALL=C sort)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
sha256sum --check --quiet "$archive/source-files.sha256"
sha256sum --check --quiet "$archive/raw/binaries.log"
cmp "$archive/raw/source-before.log" "$archive/raw/source-after.log"
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
[[ $(wc -l < "$archive/gnu.complete-roster") == 2147 ]]
[[ $(wc -l < "$archive/added-tests.list") == 1 ]]
[[ $(wc -l < "$archive/source-files.list") == 11 ]]
[[ $(wc -l < "$archive/raw/binaries.log") == 6 ]]
actual=$(tail -n 1 "$archive/raw/global-verus.log" | sha256sum | awk '{print $1}')
[[ "$actual" == $(< crates/fe2o3-runtime-model/verus/pins/TRANSCRIPT_SHA256) ]]
summary='READ_INVARIANT_OK obligations=155 inherited=127 new=28 test_obligations=1 invariant_mutations=16'
grep -Fx "$summary" "$archive/raw/invariant-campaign.log"
grep -Fx "$summary" "$archive/raw/global-verus.log"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
