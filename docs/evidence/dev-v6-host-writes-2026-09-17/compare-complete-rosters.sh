#!/usr/bin/env bash
set -euo pipefail
archive=$(cd -- "$(dirname -- "$0")" && pwd)
[[ ! -e "$archive/SHA256SUMS" ]]
for target in gnu musl; do
    [[ ! -e "$archive/$target.complete-roster" ]]
    awk '
        function emit(line, fields, name) {
            split(line, fields, " ")
            name = package SUBSEP fields[2]
            if (seen[name]++) exit 1
            if (line ~ /\.\.\. ok$/) passed[package]++
            else if (line ~ /\.\.\. ignored/) ignored[package]++
            else exit 1
            print package, line
        }
        /Running unittests/ {
            if (pending != "") exit 1
            package = $NF
            gsub(/[()]/, "", package)
            sub(/^.*\//, "", package)
            sub(/-[0-9a-f]+$/, "", package)
            packages[package] = 1
        }
        /^test [^ ]+ \.\.\. $/ {
            if (pending != "") exit 1
            pending = $0
            next
        }
        /^ok$/ {
            if (pending == "") exit 1
            emit(pending "ok")
            pending = ""
            next
        }
        /^test [^ ]+ \.\.\. (ok|ignored)/ { emit($0) }
        /^test result: ok\./ {
            expected_passed[package] = $4
            expected_ignored[package] = $8
            if ($6 != 0 || $12 != 0) exit 1
        }
        END {
            if (pending != "") exit 1
            for (p in packages) {
                if (!(p in expected_passed) || passed[p] != expected_passed[p] || ignored[p] != expected_ignored[p]) exit 1
            }
        }
    ' "$archive/raw/$target.log" | LC_ALL=C sort > "$archive/$target.complete-roster"
done
test -s "$archive/gnu.complete-roster"
cmp "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
wc -l "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
sha256sum "$archive/gnu.complete-roster" "$archive/musl.complete-roster"
