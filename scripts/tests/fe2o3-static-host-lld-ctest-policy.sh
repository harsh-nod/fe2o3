#!/usr/bin/env bash
set -euo pipefail

die() {
  printf 'fe2o3-static-host-lld-ctest-policy-test: %s\n' "$*" >&2
  exit 1
}

source_root=$(/usr/bin/readlink -f -- "${BASH_SOURCE[0]%/*}/../..")
readonly source_root
readonly helper="$source_root/scripts/fe2o3-static-host-lld-ctest-policy.sh"
work_root=$(/usr/bin/mktemp -d)
readonly work_root
trap '/usr/bin/rm -rf -- "$work_root"' EXIT

new_fixture() {
  local name=$1 fixture source build add_test_line
  fixture="$work_root/$name"
  source="$fixture/source"
  build="$fixture/build"
  /usr/bin/mkdir -p -- "$source/tests" "$build"
  {
    printf 'cmake_minimum_required(VERSION 3.28)\n'
    printf 'project(fixture LANGUAGES CXX)\n'
    printf 'if(BUILD_TESTING)\n'
    printf '  add_test(NAME fe2o3-host-lld-secure-protocol-v2\n'
    printf '    COMMAND /usr/bin/bash fixture)\n'
    printf 'endif()\n'
  } >"$source/CMakeLists.txt"
  printf 'BUILD_TESTING:BOOL=ON\n' >"$build/CMakeCache.txt"
  add_test_line=4
  {
    printf 'add_test([=[fe2o3-host-lld-secure-protocol-v2]=] "/usr/bin/bash" "%s/tests/ctest_secure_protocol.sh" "%s" "%s/fe2o3-host-lld" "%s")\n' \
      "$source" "$source" "$build" "$build"
    printf 'set_tests_properties([=[fe2o3-host-lld-secure-protocol-v2]=] PROPERTIES  LABELS "host-link;security;protocol-v2" TIMEOUT "900" _BACKTRACE_TRIPLES "%s;%s;add_test;%s;0;")\n' \
      "$source/CMakeLists.txt" "$add_test_line" "$source/CMakeLists.txt"
  } >"$build/CTestTestfile.cmake"
  {
    printf '1 newfstatat(AT_FDCWD, "/usr/bin/gcov-tool", 0x0, 0) = -1 ENOENT\n'
    printf '1 newfstatat(AT_FDCWD, "/usr/bin/valgrind-helper", 0x0, 0) = -1 ENOENT\n'
  } >"$build/configure.raw.100"
  printf '%s\n' "$fixture"
}

run_pass() {
  local fixture=$1 source build status
  source="$fixture/source"
  build="$fixture/build"
  status="$build/fe2o3-host-lld.ctest-policy-status.txt"
  "$helper" "$source" "$build" "$status" >/dev/null
  [[ -f "$status" && ! -L "$status" ]] || die 'passing fixture omitted status'
  /usr/bin/grep -Fxq 'STATUS=passed' "$status" || die 'passing status is invalid'
  /usr/bin/grep -Fxq \
    'PROTOCOL_CTEST_EXECUTION=not-executed-by-policy-check' "$status" ||
    die 'passing status overstates protocol CTest execution'
}

run_reject() {
  local fixture=$1 source build status
  source="$fixture/source"
  build="$fixture/build"
  status="$build/fe2o3-host-lld.ctest-policy-status.txt"
  if "$helper" "$source" "$build" "$status" >/dev/null 2>&1; then
    die "negative fixture unexpectedly passed: ${fixture##*/}"
  fi
  [[ ! -e "$status" && ! -L "$status" ]] ||
    die "negative fixture retained status: ${fixture##*/}"
}

fixture=$(new_fixture positive-neighbor-controls)
run_pass "$fixture"

fixture=$(new_fixture build-testing-off)
/usr/bin/sed -i 's/^BUILD_TESTING:BOOL=ON$/BUILD_TESTING:BOOL=OFF/' \
  "$fixture/build/CMakeCache.txt"
run_reject "$fixture"

fixture=$(new_fixture build-testing-missing)
/usr/bin/sed -i '/^BUILD_TESTING:/d' "$fixture/build/CMakeCache.txt"
run_reject "$fixture"

fixture=$(new_fixture build-testing-duplicate)
printf 'BUILD_TESTING:BOOL=ON\n' >>"$fixture/build/CMakeCache.txt"
run_reject "$fixture"

fixture=$(new_fixture registration-renamed)
/usr/bin/sed -i 's/fe2o3-host-lld-secure-protocol-v2/renamed-protocol-test/g' \
  "$fixture/build/CTestTestfile.cmake"
run_reject "$fixture"

fixture=$(new_fixture registration-duplicate)
duplicate_registration=$(/usr/bin/sed -n '1p' \
  "$fixture/build/CTestTestfile.cmake")
printf '%s\n' "$duplicate_registration" >>"$fixture/build/CTestTestfile.cmake"
run_reject "$fixture"

for cache_case in memory-empty memory-nonempty coverage-empty coverage-nonempty; do
  fixture=$(new_fixture "$cache_case")
  case "$cache_case" in
    memory-empty) printf 'MEMORYCHECK_COMMAND:FILEPATH=\n' ;;
    memory-nonempty) printf 'MEMORYCHECK_COMMAND:FILEPATH=/usr/bin/valgrind\n' ;;
    coverage-empty) printf 'COVERAGE_COMMAND:FILEPATH=\n' ;;
    coverage-nonempty) printf 'COVERAGE_COMMAND:FILEPATH=/usr/bin/gcov\n' ;;
  esac >>"$fixture/build/CMakeCache.txt"
  run_reject "$fixture"
done

fixture=$(new_fixture proc-meminfo)
printf '1 openat(AT_FDCWD, "/proc/meminfo", O_RDONLY) = -1 EACCES\n' \
  >>"$fixture/build/configure.raw.100"
run_reject "$fixture"

for tool in boundscheck compute-sanitizer cuda-memcheck drmemory purify valgrind; do
  fixture=$(new_fixture "memory-tool-$tool")
  printf '1 access("/usr/local/bin/%s", X_OK) = -1 ENOENT\n' "$tool" \
    >>"$fixture/build/configure.raw.100"
  run_reject "$fixture"
done

for tool in gcov gcov-13 x86_64-linux-gnu-gcov x86_64-linux-gnu-gcov-13; do
  fixture=$(new_fixture "coverage-tool-$tool")
  printf '1 access("/usr/bin/%s", X_OK) = -1 ENOENT\n' "$tool" \
    >>"$fixture/build/configure.raw.100"
  run_reject "$fixture"
done

printf 'fe2o3 static host LLD CTest policy tests passed\n'
