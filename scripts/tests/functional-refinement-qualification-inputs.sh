#!/usr/bin/env bash
set -euo pipefail
repo="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly repo
source "$repo/scripts/functional-refinement-verus-runtime-v1.sh"
scratch=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-qualification-input-tests-XXXXXXXX")
readonly scratch
trap 'find -P "$scratch" -xdev -depth -delete' EXIT
checks=0

fail() { printf 'qualification input test failed: %s\n' "$*" >&2; exit 1; }
reject() {
    local message=$1
    shift
    if ( "$@" ) > "$scratch/rejection.log" 2>&1; then
        fail "accepted $message"
    fi
    grep -Fq -- "$message" "$scratch/rejection.log" || {
        cat "$scratch/rejection.log" >&2
        fail "wrong rejection, expected $message"
    }
    checks=$((checks + 1))
}

# Synthetic package bytes exercise only private shell helpers, never runtime admission.
mkdir -p "$scratch/package/DEBIAN" "$scratch/package/usr/lib/x86_64-linux-gnu" "$scratch/inputs"
printf '%s\n' 'Package: fe2o3-qualification-test' 'Version: 1' 'Architecture: all' \
    'Maintainer: test <test@example.invalid>' 'Description: package stream controls' \
    > "$scratch/package/DEBIAN/control"
readonly TEST_MEMBER=./usr/lib/x86_64-linux-gnu/libc.so.6
printf 'a\000b\377c\n' > "$scratch/package/${TEST_MEMBER#./}"
dpkg-deb --build --root-owner-group "$scratch/package" "$scratch/data.deb" > /dev/null
readonly TEST_PAYLOAD="$scratch/package/${TEST_MEMBER#./}"
TEST_SIZE="$(stat -c %s "$TEST_PAYLOAD")"
TEST_DIGEST="$(sha256_file "$TEST_PAYLOAD")"
WRONG_DIGEST="$(printf '%064d' 0)"
readonly TEST_SIZE TEST_DIGEST WRONG_DIGEST
verify_package_member "$scratch/data.deb" "$TEST_MEMBER" "$TEST_SIZE" "$TEST_DIGEST"
checks=$((checks + 1))
reject 'package member size differs' verify_package_member "$scratch/data.deb" "$TEST_MEMBER" 7 "$TEST_DIGEST"
reject 'package member SHA-256 differs' verify_package_member "$scratch/data.deb" "$TEST_MEMBER" "$TEST_SIZE" "$WRONG_DIGEST"
reject 'cannot measure package member' verify_package_member "$scratch/data.deb" ./missing "$TEST_SIZE" "$TEST_DIGEST"
head -c 128 "$scratch/data.deb" > "$scratch/truncated.deb"
reject 'cannot measure package member' verify_package_member "$scratch/truncated.deb" "$TEST_MEMBER" "$TEST_SIZE" "$TEST_DIGEST"

decoder_fails_after_output() {
    dpkg-deb() { command dpkg-deb "$@"; return 17; }
    verify_package_member "$scratch/data.deb" "$TEST_MEMBER" "$TEST_SIZE" "$TEST_DIGEST"
}
tar_fails_after_output() {
    tar() { command tar "$@"; return 19; }
    verify_package_member "$scratch/data.deb" "$TEST_MEMBER" "$TEST_SIZE" "$TEST_DIGEST"
}
hasher_fails_after_output() {
    sha256sum() { command sha256sum "$@"; return 23; }
    verify_package_member "$scratch/data.deb" "$TEST_MEMBER" "$TEST_SIZE" "$TEST_DIGEST"
}
reject 'cannot measure package member' decoder_fails_after_output
reject 'cannot measure package member' tar_fails_after_output
reject 'cannot hash package member' hasher_fails_after_output

file_hash_command_fails_after_output() {
    # Invoked indirectly through the sourced verify_file/sha256_file helpers.
    # shellcheck disable=SC2329
    sha256sum() { command sha256sum "$@"; return 29; }
    verify_file "$TEST_PAYLOAD" 0444 "$TEST_SIZE" "$TEST_DIGEST" false
}
file_hash_helper_fails_after_output() {
    sha256_file() { printf '%s\n' "$TEST_DIGEST"; return 31; }
    verify_file "$TEST_PAYLOAD" 0444 "$TEST_SIZE" "$TEST_DIGEST" false
}
report_hash_fails_after_output() {
    sha256_file() { printf '%s\n' "$TEST_DIGEST"; return 37; }
    report_success QUALIFICATION_INPUTS
}
inventory_parser_fails_after_output() {
    awk() { command awk "$@"; return 41; }
    expected_inventory
}
reject 'cannot hash source' file_hash_command_fails_after_output
reject 'cannot hash source' file_hash_helper_fails_after_output
reject 'cannot hash manifest' report_hash_fails_after_output
if grep -Fq 'QUALIFICATION_INPUTS_OK' "$scratch/rejection.log"; then fail 'reported a failed hash as success'; fi
reject 'cannot enumerate manifest inventory' inventory_parser_fails_after_output

reject 'qualification package pin differs' qualification_package_digest absent-test-package
[[ $(qualification_package_digest libc6) =~ ^[0-9a-f]{64}$ ]]
checks=$((checks + 1))
printf 'not an archive\n' > "$scratch/inputs/libc.deb"
decoder_must_not_run() {
    dpkg-deb() { touch "$scratch/decoder-called"; return 91; }
    audit_qualification_source /nonexistent /nonexistent "$scratch/inputs"
}
reject 'qualification package SHA-256 differs' decoder_must_not_run
[[ ! -e "$scratch/decoder-called" ]] || fail 'parsed an unpinned package'
rm -- "$scratch/inputs/libc.deb"
cp "$scratch/data.deb" "$scratch/inputs/libc.deb"
TEST_ARCHIVE_DIGEST="$(sha256_file "$scratch/data.deb")"
readonly TEST_ARCHIVE_DIGEST
package_hash_fails_after_output() {
    qualification_package_digest() { printf '%s\n' "$TEST_ARCHIVE_DIGEST"; }
    sha256_file() { printf '%s\n' "$TEST_ARCHIVE_DIGEST"; return 43; }
    dpkg-deb() { touch "$scratch/decoder-called"; return 91; }
    audit_qualification_source /nonexistent /nonexistent "$scratch/inputs"
}
reject 'cannot hash qualification package' package_hash_fails_after_output
[[ ! -e "$scratch/decoder-called" ]] || fail 'parsed a package after failed hashing'
rm -- "$scratch/inputs/libc.deb"
ln -s "$scratch/data.deb" "$scratch/inputs/libc.deb"
reject 'not a no-follow regular file' audit_qualification_source /nonexistent /nonexistent "$scratch/inputs"
rm -- "$scratch/inputs/libc.deb"
ln "$scratch/data.deb" "$scratch/inputs/libc.deb"
reject 'source has multiple hard links' audit_qualification_source /nonexistent /nonexistent "$scratch/inputs"
rm -- "$scratch/inputs/libc.deb"
mkfifo "$scratch/inputs/libc.deb"
reject 'not a no-follow regular file' audit_qualification_source /nonexistent /nonexistent "$scratch/inputs"
rm -- "$scratch/inputs/libc.deb"
reject 'not a no-follow regular file' audit_qualification_source /nonexistent /nonexistent "$scratch/inputs"

# Inspection of script-shaped input must never execute it, even with matching bytes.
printf '#!/bin/sh\ntouch "%s/executed"\n' "$scratch" > "$scratch/candidate"
verify_file "$scratch/candidate" 0555 "$(stat -c %s "$scratch/candidate")" "$(sha256_file "$scratch/candidate")" false
[[ ! -e "$scratch/executed" ]] || fail 'executed a candidate'
checks=$((checks + 1))
reject usage main audit-qualification-source
printf 'qualification package input checks passed: %s\n' "$checks"
