#!/usr/bin/env bash
set -euo pipefail

usage() {
  printf 'usage: %s TOOL_SOURCE BUILD_DIR STATUS_FILE\n' "$0" >&2
  exit 64
}

die() {
  printf 'fe2o3-static-host-lld-ctest-policy: %s\n' "$*" >&2
  exit 1
}

is_forbidden_trace_line() {
  local line=$1
  local memory_tool_re='"/[^" ]*/(boundscheck|compute-sanitizer|cuda-memcheck|drmemory|purify|valgrind)"'
  local gcov_re='"/[^" ]*/(gcov|gcov-[0-9]+|[^/" ]+-gcov|[^/" ]+-gcov-[0-9]+)"'
  [[ "$line" == *'"/proc/meminfo"'* || "$line" =~ $memory_tool_re ||
    "$line" =~ $gcov_re ]]
}

[[ $# -eq 3 ]] || usage
readonly tool_source=$1
readonly build_dir=$2
readonly status_file=$3
readonly cmake_cache="$build_dir/CMakeCache.txt"
readonly ctest_test_file="$build_dir/CTestTestfile.cmake"
readonly cmake_lists="$tool_source/CMakeLists.txt"
readonly trace_prefix="$build_dir/configure.raw"

[[ "$tool_source" == /* && -d "$tool_source" && ! -L "$tool_source" ]] ||
  die 'tool source must be an absolute non-symlink directory'
[[ "$build_dir" == /* && -d "$build_dir" && ! -L "$build_dir" ]] ||
  die 'build directory must be an absolute non-symlink directory'
[[ "$status_file" == "$build_dir/fe2o3-host-lld.ctest-policy-status.txt" &&
  ! -e "$status_file" && ! -L "$status_file" ]] ||
  die 'status path must be the fresh canonical build status path'
for input in "$cmake_cache" "$ctest_test_file" "$cmake_lists"; do
  [[ -f "$input" && ! -L "$input" ]] || die "required policy input is absent: $input"
done

build_testing_rows=0
build_testing_on_rows=0
while IFS= read -r line; do
  if [[ "$line" == BUILD_TESTING:* ]]; then
    ((build_testing_rows += 1))
    [[ "$line" == BUILD_TESTING:BOOL=ON ]] && ((build_testing_on_rows += 1))
  fi
  [[ ! "$line" =~ ^(MEMORYCHECK_COMMAND|COVERAGE_COMMAND)[^:]*: ]] ||
    die 'configure retained an optional CTest cache key'
done <"$cmake_cache"
[[ $build_testing_rows -eq 1 && $build_testing_on_rows -eq 1 ]] ||
  die 'configure did not retain exactly one BUILD_TESTING:BOOL=ON row'

cmake_line_number=0
cmake_add_test_line=0
cmake_add_test_rows=0
while IFS= read -r line; do
  ((cmake_line_number += 1))
  if [[ "$line" == '  add_test(NAME fe2o3-host-lld-secure-protocol-v2' ]]; then
    ((cmake_add_test_rows += 1))
    cmake_add_test_line=$cmake_line_number
  fi
done <"$cmake_lists"
[[ $cmake_add_test_rows -eq 1 && $cmake_add_test_line -gt 0 ]] ||
  die 'source does not contain one canonical secure protocol registration'

printf -v expected_add_test \
  'add_test([=[fe2o3-host-lld-secure-protocol-v2]=] "/usr/bin/bash" "%s/tests/ctest_secure_protocol.sh" "%s" "%s/fe2o3-host-lld" "%s")' \
  "$tool_source" "$tool_source" "$build_dir" "$build_dir"
printf -v expected_properties \
  'set_tests_properties([=[fe2o3-host-lld-secure-protocol-v2]=] PROPERTIES  LABELS "host-link;security;protocol-v2" TIMEOUT "900" _BACKTRACE_TRIPLES "%s;%s;add_test;%s;0;")' \
  "$cmake_lists" "$cmake_add_test_line" "$cmake_lists"
ctest_add_test_rows=0
ctest_property_rows=0
expected_add_test_rows=0
expected_property_rows=0
while IFS= read -r line; do
  [[ "$line" == add_test\(* ]] && ((ctest_add_test_rows += 1))
  [[ "$line" == set_tests_properties\(* ]] && ((ctest_property_rows += 1))
  [[ "$line" == "$expected_add_test" ]] && ((expected_add_test_rows += 1))
  [[ "$line" == "$expected_properties" ]] && ((expected_property_rows += 1))
done <"$ctest_test_file"
[[ $ctest_add_test_rows -eq 1 && $ctest_property_rows -eq 1 &&
  $expected_add_test_rows -eq 1 && $expected_property_rows -eq 1 ]] ||
  die 'configure emitted a noncanonical secure protocol registration'

shopt -s nullglob
trace_files=("$trace_prefix".*)
shopt -u nullglob
[[ ${#trace_files[@]} -gt 0 ]] || die 'configure omitted its raw traces'
for trace_file in "${trace_files[@]}"; do
  [[ -f "$trace_file" && ! -L "$trace_file" ]] ||
    die 'configure trace is not a regular non-symlink file'
  while IFS= read -r line; do
    ! is_forbidden_trace_line "$line" ||
      die "configure performed optional CTest discovery: $trace_file"
  done <"$trace_file"
done

umask 077
{
  printf 'FORMAT=fe2o3-static-host-lld-ctest-policy-v1\n'
  printf 'STATUS=passed\n'
  printf 'BUILD_TESTING=enabled-exactly-once\n'
  printf 'TEST_REGISTRATION=canonical-exactly-once\n'
  printf 'OPTIONAL_CTEST_CACHE_KEYS=absent\n'
  printf 'OPTIONAL_CTEST_DISCOVERY=absent\n'
  printf 'PROTOCOL_CTEST_EXECUTION=not-executed-by-policy-check\n'
  printf 'TERMINAL=fe2o3-static-host-lld-ctest-policy-v1-end\n'
} >"$status_file"

