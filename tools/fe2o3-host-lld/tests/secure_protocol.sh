#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s TOOL_SOURCE_DIR TOOL WORK_ROOT\n' "$0" >&2
  exit 64
}

die() {
  printf 'fe2o3-host-lld-secure-test: %s\n' "$*" >&2
  exit 1
}

expect_exit() {
  local expected=$1 label=$2
  shift 2
  set +e
  "$@" >"$work/reject-$label.stdout" 2>"$work/reject-$label.stderr"
  local status=$?
  set -e
  [[ $status -eq $expected ]] ||
    die "$label returned $status instead of $expected"
  [[ ! -e "$work/out-$label" ]] || die "$label published an output"
}

make_deplibs_object() {
  local output=$1 section=$2 dependency=$3
  local source="$output.s"
  [[ $section != *['"\\']* && $dependency != *['"\\']* ]] ||
    die 'dependent-library fixture contains an unsupported assembly byte'
  {
    printf '%s\n' '.global _start' '.text' '_start:' \
      '  mov $60, %rax' '  xor %rdi, %rdi' '  syscall'
    printf '.section "%s","MS",@llvm_dependent_libraries,1\n' "$section"
    if [[ $dependency == __oversized__ ]]; then
      printf '  .ascii "'
      /usr/bin/head -c 8192 /dev/zero | /usr/bin/tr '\0' x
      printf '"\n  .byte 0\n'
    else
      printf '  .asciz "%s"\n' "$dependency"
    fi
  } >"$source"
  /usr/lib/llvm-18/bin/llvm-mc -filetype=obj \
    -triple=x86_64-unknown-linux-gnu \
    "$source" -o "$output"
}

[[ $# -eq 3 ]] || usage
source_dir=$(/usr/bin/readlink -f -- "$1")
tool=$(/usr/bin/readlink -f -- "$2")
work=$3
readonly source_dir tool work

[[ -d "$source_dir" && ! -L "$1" ]] ||
  die 'tool source directory must be canonical and non-symlink'
[[ -x "$tool" && -f "$tool" && ! -L "$2" ]] ||
  die 'tool must be a canonical executable regular file'
[[ -x /usr/lib/llvm-18/bin/llvm-mc && -f /usr/lib/llvm-18/bin/llvm-mc &&
   ! -L /usr/lib/llvm-18/bin/llvm-mc ]] ||
  die 'canonical LLVM 18 llvm-mc is required for attack fixtures'
[[ "$work" == /* && ! -e "$work" && ! -L "$work" ]] ||
  die 'work root must be an absolute nonexistent path'
/usr/bin/mkdir --mode=700 -- "$work"

harness="$work/fd-link-harness"
readonly harness
/usr/bin/g++-13 -std=c++20 -O2 -Wall -Wextra -Wpedantic -Werror \
  -I"$source_dir/include" -o "$harness" \
  "$source_dir/tests/fd_link_harness.cpp"

assembly="$work/minimal.s"
object="$work/minimal.o"
readonly assembly object
# shellcheck disable=SC2016
printf '%s\n' '.global _start' '.text' '_start:' '  mov $60, %rax' \
  '  xor %rdi, %rdi' '  syscall' >"$assembly"
/usr/bin/as --64 -o "$object" "$assembly"

"$harness" "$tool" "$object" "$work/out-minimal-a" normal
"$harness" "$tool" "$object" "$work/out-minimal-b" normal
/usr/bin/cmp -s -- "$work/out-minimal-a" "$work/out-minimal-b" ||
  die 'fresh minimal links are not byte-identical'
env -i "$work/out-minimal-a" || die 'minimal linked output did not execute'

rust_object="$work/rust-host-smoke.o"
rust_rlib="$work/libfe2o3_rust_rlib_symbol.rlib"
readonly rust_object rust_rlib
env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
  /usr/bin/rustc --crate-type=rlib -C panic=abort -C opt-level=2 \
  -o "$rust_rlib" "$source_dir/tests/rust_rlib_symbol.rs"
env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
  /usr/bin/rustc --emit=obj -C panic=abort -C opt-level=2 \
  -o "$rust_object" "$source_dir/tests/rust_host_smoke.rs"
rust_sysroot=$(/usr/bin/rustc --print sysroot)
core_rlib=$(/usr/bin/find \
  "$rust_sysroot/lib/rustlib/x86_64-unknown-linux-gnu/lib" \
  -name 'libcore-*.rlib' -type f -print -quit)
readonly rust_sysroot core_rlib
[[ -n "$core_rlib" && -f "$core_rlib" ]] || die 'pinned libcore rlib is absent'
"$harness" "$tool" "$rust_object" "$work/out-rust" rust-link "$rust_rlib"
env -i "$work/out-rust" || die 'Rust host output did not execute'
"$harness" "$tool" "$object" "$work/out-core-validation" rust-link \
  "$core_rlib"
/usr/bin/cmp -s -- "$work/out-minimal-a" "$work/out-core-validation" ||
  die 'deep validation of real libcore changed the unreferenced link result'

script="$work/hostile.ld"
thin="$work/hostile-thin.a"
nested="$work/hostile-nested.a"
archive="$work/regular.a"
bitcode="$work/hostile.bc"
deplibs_target="$work/undeclared-target.a"
deplibs_fifo="$work/undeclared-blocking-fifo"
deplibs_absolute="$work/deplibs-absolute.o"
deplibs_relative="$work/deplibs-relative.o"
deplibs_archive_object="$work/deplibs-archive-member.o"
deplibs_rlib_object="$work/deplibs-rlib-member.o"
deplibs_malformed="$work/deplibs-malformed.o"
deplibs_oversized="$work/deplibs-oversized.o"
deplibs_blocking="$work/deplibs-blocking.o"
deplibs_archive="$work/deplibs-archive.a"
deplibs_rlib="$work/deplibs-member.rlib"
deplibs_nested="$work/deplibs-nested.a"
readonly script thin nested archive bitcode deplibs_target deplibs_fifo
readonly deplibs_absolute deplibs_relative deplibs_archive_object
readonly deplibs_rlib_object deplibs_malformed deplibs_oversized
readonly deplibs_blocking deplibs_archive deplibs_rlib deplibs_nested
printf 'INPUT(%s)\n' "$object" >"$script"
/usr/bin/ar crT "$thin" "$object"
/usr/bin/ar cr "$archive" "$object"
/usr/bin/ar cr "$nested" "$archive"
env -i LC_ALL=C LANG=C PATH=/usr/bin:/bin \
  /usr/bin/rustc --emit=llvm-bc -C panic=abort \
  -o "$bitcode" "$source_dir/tests/rust_host_smoke.rs"
/usr/bin/ar cr "$deplibs_target" "$object"
/usr/bin/mkfifo --mode=600 "$deplibs_fifo"
make_deplibs_object "$deplibs_absolute" .deplibs "$deplibs_target"
make_deplibs_object "$deplibs_relative" .not_deplibs undeclared-target.a
make_deplibs_object "$deplibs_archive_object" .archive_deplibs \
  "$deplibs_target"
make_deplibs_object "$deplibs_rlib_object" .rlib_deplibs "$deplibs_target"
make_deplibs_object "$deplibs_malformed" .malformed_deplibs "$deplibs_target"
make_deplibs_object "$deplibs_oversized" .oversized_deplibs __oversized__
make_deplibs_object "$deplibs_blocking" .blocking_deplibs "$deplibs_fifo"
/usr/bin/ar cr "$deplibs_archive" "$object" "$deplibs_archive_object"
/usr/bin/ar cr "$deplibs_rlib" "$object" "$deplibs_rlib_object"
/usr/bin/ar cr "$deplibs_nested" "$deplibs_archive"

expect_exit 64 script "$harness" "$tool" "$script" "$work/out-script" \
  content-attack "$object"
expect_exit 64 thin "$harness" "$tool" "$thin" "$work/out-thin" \
  thin-attack "$object"
expect_exit 64 nested "$harness" "$tool" "$nested" "$work/out-nested" \
  nested-attack
expect_exit 64 bitcode "$harness" "$tool" "$bitcode" "$work/out-bitcode" \
  bitcode-attack
expect_exit 64 deplibs-absolute "$harness" "$tool" "$deplibs_absolute" \
  "$work/out-deplibs-absolute" content-attack "$deplibs_target"
expect_exit 64 deplibs-relative "$harness" "$tool" "$deplibs_relative" \
  "$work/out-deplibs-relative" deplibs-relative-attack "$deplibs_target"
expect_exit 64 deplibs-archive "$harness" "$tool" "$deplibs_archive" \
  "$work/out-deplibs-archive" content-attack "$deplibs_target"
expect_exit 64 deplibs-rlib "$harness" "$tool" "$deplibs_rlib" \
  "$work/out-deplibs-rlib" deplibs-rlib-attack "$deplibs_target"
expect_exit 64 deplibs-nested "$harness" "$tool" "$deplibs_nested" \
  "$work/out-deplibs-nested" content-attack "$deplibs_target"
expect_exit 64 deplibs-malformed-name "$harness" "$tool" \
  "$deplibs_malformed" "$work/out-deplibs-malformed-name" \
  deplibs-malformed-name "$deplibs_target"
expect_exit 64 deplibs-malformed-shstr "$harness" "$tool" \
  "$deplibs_malformed" "$work/out-deplibs-malformed-shstr" \
  deplibs-malformed-shstr "$deplibs_target"
expect_exit 64 deplibs-oversized "$harness" "$tool" "$deplibs_oversized" \
  "$work/out-deplibs-oversized" content-attack
expect_exit 64 deplibs-blocking "$harness" "$tool" "$deplibs_blocking" \
  "$work/out-deplibs-blocking" content-attack "$deplibs_fifo"

for mode in wrong-hash wrong-size wrong-kind bare-fd extra-fd \
    wrong-socket-type wrong-socket-identity user-mmap-option \
    user-threads-option conflicting-duplicate oversized-input-metadata \
    total-input-metadata caller-dependent-libraries \
    caller-no-dependent-libraries; do
  expect_exit 64 "$mode" "$harness" "$tool" "$object" \
    "$work/out-$mode" "$mode"
done
expect_exit 66 duplicate-input "$harness" "$tool" "$object" \
  "$work/out-duplicate-input" duplicate-input
expect_exit 70 no-result-reader "$harness" "$tool" "$object" \
  "$work/out-no-result-reader" no-result-reader
expect_exit 70 prefilled-result-queue "$harness" "$tool" "$object" \
  "$work/out-prefilled-result-queue" prefilled-result-queue
for profile in state rtmin rtmax kernel-reserved-32 kernel-reserved-33; do
  expect_exit 70 "hostile-signal-$profile-kill" "$harness" "$tool" \
    "$object" "$work/out-hostile-signal-$profile-kill" \
    "hostile-signal-$profile-kill"
done
expect_exit 64 archive-member-flood "$harness" "$tool" "$object" \
  "$work/out-archive-member-flood" archive-member-flood

"$harness" "$tool" "$object" "$work/out-stdio-result-alias" \
  stdio-result-alias
"$harness" "$tool" "$object" "$work/out-stdio-input-alias" \
  stdio-input-alias
"$harness" "$tool" "$object" "$work/out-stdio-blocked-pipe" \
  stdio-blocked-pipe
"$harness" "$tool" "$object" "$work/out-blocking-result-socket" \
  blocking-result-socket
"$harness" "$tool" "$object" "$work/out-hostile-signal-state" \
  hostile-signal-state
for profile in rtmin rtmax kernel-reserved-32 kernel-reserved-33; do
  "$harness" "$tool" "$object" "$work/out-hostile-signal-$profile" \
    "hostile-signal-$profile"
done
for output in "$work/out-stdio-result-alias" \
    "$work/out-stdio-input-alias" "$work/out-stdio-blocked-pipe" \
    "$work/out-blocking-result-socket" "$work/out-hostile-signal-state" \
    "$work/out-hostile-signal-rtmin" "$work/out-hostile-signal-rtmax" \
    "$work/out-hostile-signal-kernel-reserved-32" \
    "$work/out-hostile-signal-kernel-reserved-33"; do
  /usr/bin/cmp -s -- "$work/out-minimal-a" "$output" ||
    die "descriptor neutralization changed output bytes: $output"
done

"$harness" "$tool" "$object" "$work/out-proc-attack" proc-attack
"$harness" "$tool" "$object" "$work/out-replacement-race" replacement-race
/usr/bin/cmp -s -- "$work/out-minimal-a" "$work/out-proc-attack" ||
  die 'nondumpable probe changed output bytes'
/usr/bin/cmp -s -- "$work/out-minimal-a" "$work/out-replacement-race" ||
  die 'replacement-race probe changed output bytes'

canonical_env=(env -i LC_ALL=C LANG=C TZ=UTC SOURCE_DATE_EPOCH=0)
expect_exit 64 response "${canonical_env[@]}" "$tool" \
  --fe2o3-host-lld-elf-v2 @/tmp/response
expect_exit 64 user-output "${canonical_env[@]}" "$tool" \
  --fe2o3-host-lld-elf-v2 -o /tmp/output
for assignment in GLIBC_TUNABLES=glibc.malloc.check=3 MALLOC_CHECK_=3 \
    MALLOC_PERTURB_=41 GCONV_PATH=/tmp LOCPATH=/tmp NLSPATH=/tmp; do
  label=${assignment%%=*}
  expect_exit 65 "environment-$label" /usr/bin/env -i LC_ALL=C LANG=C \
    TZ=UTC SOURCE_DATE_EPOCH=0 "$assignment" "$tool" --fe2o3-identity-v1
done

identity="$work/identity.txt"
readonly identity
"${canonical_env[@]}" "$tool" --fe2o3-identity-v1 >"$identity"
for field in max_argument_count=4096 max_argument_bytes=4096 \
    max_total_argument_bytes=1048576 max_input_count=2048 \
    max_input_bytes=268435456 max_total_input_bytes=2147483648 \
    max_output_bytes=536870912 max_address_space_bytes=4294967296 \
    max_archive_members=262144 max_cpu_seconds=60 \
    dependent_libraries=forbidden \
    signal_state=linux-x86_64-kernel-1-64-main-v2; do
  /usr/bin/grep -Fxq "$field" "$identity" ||
    die "identity omitted the operational bound $field"
done

if /usr/bin/unshare --user --map-root-user --mount /usr/bin/true \
    >/dev/null 2>&1; then
  expect_exit 65 user-namespace /usr/bin/unshare --user --map-root-user \
    --mount "$harness" "$tool" "$object" "$work/out-user-namespace" \
    userns-attack
  expect_exit 65 fake-proc /usr/bin/unshare --user --map-root-user --mount \
    /usr/bin/bash -c \
    '/usr/bin/mount -t tmpfs tmpfs /proc && exec "$@"' bash \
    "$harness" "$tool" "$object" "$work/out-fake-proc" fake-proc-attack
else
  printf 'user namespace and fake procfs negative tests skipped: unavailable\n'
fi

trace_prefix="$work/secure.trace"
readonly trace_prefix
/usr/bin/strace -ff -qq \
  -e trace=open,openat,openat2,rename,renameat,renameat2,unlink,unlinkat,\
memfd_create,sendmsg,shutdown \
  -o "$trace_prefix" "$harness" "$tool" "$object" "$work/out-trace" normal
tool_trace=''
for candidate in "$trace_prefix".*; do
  if /usr/bin/grep -q 'sendmsg(91' "$candidate"; then
    tool_trace=$candidate
    break
  fi
done
[[ -n "$tool_trace" ]] || die 'trace did not identify the static tool process'
[[ $(/usr/bin/grep -c 'memfd_create(' "$tool_trace") -eq 1 ]] ||
  die 'tool did not create exactly one private output memfd'
[[ $(/usr/bin/grep -c 'sendmsg(91' "$tool_trace") -eq 1 ]] ||
  die 'tool did not send exactly one result packet'
[[ $(/usr/bin/grep -c 'shutdown(91, SHUT_WR)' "$tool_trace") -eq 1 ]] ||
  die 'tool did not half-close the result socket'
[[ $(/usr/bin/grep -c 'O_WRONLY|O_CREAT|O_TRUNC' "$tool_trace") -eq 1 ]] ||
  die 'tool did not commit directly to the private memfd'
! /usr/bin/grep -Eq 'rename|unlink|O_CREAT\|O_EXCL|\.tmp' "$tool_trace" ||
  die 'tool used a temp, rename, or unlink output path'

! /usr/bin/readelf -lW "$tool" | /usr/bin/grep -q 'INTERP' ||
  die 'tool has a dynamic interpreter'
! /usr/bin/readelf -dW "$tool" 2>&1 | /usr/bin/grep -q 'NEEDED' ||
  die 'tool has a dynamic dependency'
! /usr/bin/strings -a "$tool" | /usr/bin/grep -Eqi \
  '(^|[^[:alnum:]_])(amd_)?comgr([^[:alnum:]_]|$)' ||
  die 'tool contains a COMGR reference'
/usr/bin/strings -a "$tool" | /usr/bin/grep -Fx -- \
  '--no-dependent-libraries' >/dev/null ||
  die 'tool does not contain the forced dependent-library policy'

printf 'secure static host LLD protocol tests passed\n'
printf 'tool_sha256=%s\n' \
  "$(/usr/bin/sha256sum -- "$tool" | /usr/bin/cut -d ' ' -f 1)"
printf 'minimal_sha256=%s\n' \
  "$(/usr/bin/sha256sum -- "$work/out-minimal-a" | /usr/bin/cut -d ' ' -f 1)"
printf 'rust_sha256=%s\n' \
  "$(/usr/bin/sha256sum -- "$work/out-rust" | /usr/bin/cut -d ' ' -f 1)"
printf 'rust_rlib=%s\n' "$rust_rlib"
printf 'core_rlib=%s\n' "$core_rlib"
printf 'core_validation=deep-validation-only;no-extraction-claim\n'
printf 'trace=%s\n' "$tool_trace"
