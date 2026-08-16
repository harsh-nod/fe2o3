#!/usr/bin/dash
set -eu

usage() {
  printf 'usage: %s TOOL_SOURCE BUILD_DIR STATUS_FILE\n' "$0" >&2
  exit 64
}

die() {
  printf 'fe2o3-static-host-lld-ctest-policy: %s\n' "$*" >&2
  exit 1
}

is_forbidden_trace_line() {
  local line=$1 tool
  case "$line" in
    *'"/proc/meminfo"'*) return 0 ;;
  esac
  for tool in boundscheck compute-sanitizer cuda-memcheck drmemory purify valgrind; do
    case "$line" in
      *'"/'*"/$tool\""*) return 0 ;;
    esac
  done
  case "$line" in
    *'"/'*'/gcov"'*) return 0 ;;
    *'"/'*'/gcov-'[0-9]*'"'*) return 0 ;;
    *'"/'*'/'*'-gcov"'*) return 0 ;;
    *'"/'*'/'*'-gcov-'[0-9]*'"'*) return 0 ;;
  esac
  return 1
}

[ "$#" -eq 3 ] || usage
tool_source=$1
build_dir=$2
status_file=$3
readonly tool_source build_dir status_file
cmake_cache="$build_dir/CMakeCache.txt"
ctest_test_file="$build_dir/CTestTestfile.cmake"
cmake_lists="$tool_source/CMakeLists.txt"
trace_prefix="$build_dir/configure.raw"
readonly cmake_cache ctest_test_file cmake_lists trace_prefix

[ "${tool_source#/}" != "$tool_source" ] && [ -d "$tool_source" ] &&
  [ ! -L "$tool_source" ] ||
  die 'tool source must be an absolute non-symlink directory'
[ "${build_dir#/}" != "$build_dir" ] && [ -d "$build_dir" ] &&
  [ ! -L "$build_dir" ] ||
  die 'build directory must be an absolute non-symlink directory'
[ "$status_file" = "$build_dir/fe2o3-host-lld.ctest-policy-status.txt" ] &&
  [ ! -e "$status_file" ] && [ ! -L "$status_file" ] ||
  die 'status path must be the fresh canonical build status path'
for input in "$cmake_cache" "$ctest_test_file" "$cmake_lists"; do
  [ -f "$input" ] && [ ! -L "$input" ] ||
    die "required policy input is absent: $input"
done

build_testing_rows=0
build_testing_on_rows=0
while IFS= read -r line; do
  case "$line" in
    BUILD_TESTING:*)
      build_testing_rows=$((build_testing_rows + 1))
      [ "$line" = BUILD_TESTING:BOOL=ON ] &&
        build_testing_on_rows=$((build_testing_on_rows + 1))
      ;;
  esac
  case "$line" in
    MEMORYCHECK_COMMAND*:*) die 'configure retained an optional CTest cache key' ;;
    COVERAGE_COMMAND*:*) die 'configure retained an optional CTest cache key' ;;
  esac
done <"$cmake_cache"
[ "$build_testing_rows" -eq 1 ] && [ "$build_testing_on_rows" -eq 1 ] ||
  die 'configure did not retain exactly one BUILD_TESTING:BOOL=ON row'

cmake_line_number=0
cmake_add_test_line=0
cmake_add_test_rows=0
while IFS= read -r line; do
  cmake_line_number=$((cmake_line_number + 1))
  if [ "$line" = '  add_test(NAME fe2o3-host-lld-secure-protocol-v2' ]; then
    cmake_add_test_rows=$((cmake_add_test_rows + 1))
    cmake_add_test_line=$cmake_line_number
  fi
done <"$cmake_lists"
[ "$cmake_add_test_rows" -eq 1 ] && [ "$cmake_add_test_line" -gt 0 ] ||
  die 'source does not contain one canonical secure protocol registration'

expected_add_test=$(printf \
  'add_test([=[fe2o3-host-lld-secure-protocol-v2]=] "/usr/bin/bash" "%s/tests/ctest_secure_protocol.sh" "%s" "%s/fe2o3-host-lld" "%s")' \
  "$tool_source" "$tool_source" "$build_dir" "$build_dir")
expected_properties=$(printf \
  'set_tests_properties([=[fe2o3-host-lld-secure-protocol-v2]=] PROPERTIES  LABELS "host-link;security;protocol-v2" TIMEOUT "900" _BACKTRACE_TRIPLES "%s;%s;add_test;%s;0;")' \
  "$cmake_lists" "$cmake_add_test_line" "$cmake_lists")
readonly expected_add_test expected_properties
ctest_add_test_rows=0
ctest_property_rows=0
expected_add_test_rows=0
expected_property_rows=0
while IFS= read -r line; do
  case "$line" in add_test\(*) ctest_add_test_rows=$((ctest_add_test_rows + 1)) ;; esac
  case "$line" in set_tests_properties\(*) ctest_property_rows=$((ctest_property_rows + 1)) ;; esac
  [ "$line" = "$expected_add_test" ] &&
    expected_add_test_rows=$((expected_add_test_rows + 1))
  [ "$line" = "$expected_properties" ] &&
    expected_property_rows=$((expected_property_rows + 1))
done <"$ctest_test_file"
[ "$ctest_add_test_rows" -eq 1 ] && [ "$ctest_property_rows" -eq 1 ] &&
  [ "$expected_add_test_rows" -eq 1 ] && [ "$expected_property_rows" -eq 1 ] ||
  die 'configure emitted a noncanonical secure protocol registration'

set -- "$trace_prefix".*
[ "$1" != "$trace_prefix.*" ] || die 'configure omitted its raw traces'
for trace_file in "$@"; do
  [ -f "$trace_file" ] && [ ! -L "$trace_file" ] ||
    die 'configure trace is not a regular non-symlink file'
  while IFS= read -r line; do
    if is_forbidden_trace_line "$line"; then
      die "configure performed optional CTest discovery: $trace_file"
    fi
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
