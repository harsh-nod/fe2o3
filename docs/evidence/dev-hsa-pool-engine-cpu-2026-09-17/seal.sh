#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
root=$(cd -- "$archive/../../.." && pwd)
[[ ! -e "$archive/SHA256SUMS" && -f "$archive/README.md" ]]
[[ -z $(find "$archive" -type l -print) ]]
names=(base gcc python source-before tool-inputs focused benchmark-suite fixtures-before r26-fixtures r60-fixtures benchmark-suite-corrected native-link python-lint python-format cpp-format unchanged source-after fixtures-after tools-after mi300x-observation)
previous=0
for name in "${names[@]}"; do
    for suffix in command started log finished exit; do
        [[ -f "$archive/raw/$name.$suffix" ]]
    done
    expected=0
    if [[ "$name" == benchmark-suite ]]; then expected=1; fi
    [[ $(< "$archive/raw/$name.exit") == "$expected" ]]
    started=$(date -u -d "$(< "$archive/raw/$name.started")" +%s%N)
    finished=$(date -u -d "$(< "$archive/raw/$name.finished")" +%s%N)
    ((started >= previous && finished >= started))
    previous=$finished
done
[[ $(find "$archive/raw" -type f | wc -l) == $((${#names[@]} * 5)) ]]
cd -- "$root"
base=5ed58ea896e492f5bbc135e9f8449eabd51165bd
[[ $(git rev-parse HEAD) == "$base" && $(< "$archive/raw/base.log") == "$base" ]]
[[ $(sha256sum "$archive/source-files.sha256" | awk '{print $1}') == e76f7e1783b2baf50a5bcbfe0ffbf0daac6e97cfe376220b3bd91c011f64fcf4 ]]
[[ $(sha256sum "$archive/source.patch" | awk '{print $1}') == ae7c39b737353b00fc358efa3e8c687af7ea60229f3a2e8ecd52e22e3c8a2aa4 ]]
[[ $(sha256sum "$archive/fixture-files.sha256" | awk '{print $1}') == 37e9e212dabb870be31b99fdd16022dd27ed6b41cc51bfeaa20337f1ea6410a6 ]]
[[ $(sha256sum "$archive/fixture.patch" | awk '{print $1}') == 2b48533bd5bf15dfda821bb95e45c3cf6f1081f6285027d7229d0af197ff9a73 ]]
mapfile -t sources < "$archive/source-files.list"
mapfile -t changed < "$archive/changed-files.list"
mapfile -t fixtures < "$archive/fixture-files.list"
[[ ${#sources[@]} == 8 && ${#changed[@]} == 7 && ${#fixtures[@]} == 2 ]]
cmp <(sha256sum -- "${sources[@]}") "$archive/source-files.sha256"
cmp <(sha256sum -- "${fixtures[@]}") "$archive/fixture-files.sha256"
cmp <(git diff HEAD --binary -- "${changed[@]}") "$archive/source.patch"
cmp <(git diff HEAD --binary -- "${fixtures[@]}") "$archive/fixture.patch"
git diff --quiet -- "${sources[@]}" "${fixtures[@]}"
scope=(. ':(exclude)docs/evidence/dev-hsa-pool-engine-cpu-2026-09-17')
cmp <(printf '%s\n' "${changed[@]}" "${fixtures[@]}" | LC_ALL=C sort) <(git diff HEAD --name-only -- "${scope[@]}" | LC_ALL=C sort)
[[ -z $(git ls-files --others --exclude-standard -- "${scope[@]}") ]]
cmp "$archive/raw/source-before.log" "$archive/raw/source-after.log"
cmp "$archive/raw/fixtures-before.log" "$archive/raw/fixtures-after.log"
sha256sum --check --quiet "$archive/raw/tool-inputs.log"
grep -Eq '^Ran 10 tests in [0-9.]+s$' "$archive/raw/focused.log"
grep -Fx 'OK' "$archive/raw/focused.log"
grep -Fx 'FAILED (failures=1, errors=2)' "$archive/raw/benchmark-suite.log"
grep -Eq '^Ran 305 tests in [0-9.]+s$' "$archive/raw/benchmark-suite-corrected.log"
grep -Fx 'OK' "$archive/raw/benchmark-suite-corrected.log"
[[ ! -s "$archive/raw/unchanged.log" ]]
grep -Fx 'NATIVE_LINK_OK executable_not_run=1 mock_not_linked=1' "$archive/raw/native-link.log"
cd -- "$archive"
find . -type f ! -name SHA256SUMS -print0 | LC_ALL=C sort -z | xargs -0 sha256sum > SHA256SUMS
sha256sum --check --quiet SHA256SUMS
sha256sum SHA256SUMS
