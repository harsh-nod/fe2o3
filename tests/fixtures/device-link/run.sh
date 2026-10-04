#!/usr/bin/env bash
set -euo pipefail

fixture_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd -P)
temporary=

cleanup() {
  if [[ -n ${temporary:-} && -d $temporary ]]; then
    rm -rf -- "$temporary"
  fi
}
trap cleanup EXIT

fail() {
  printf 'FAIL device-link fixture: %s\n' "$*" >&2
  exit 1
}

resolve_tool() {
  local variable=$1
  local name=$2
  local configured=${!variable:-}
  if [[ -n $configured ]]; then
    [[ -x $configured ]] || fail "$variable is not executable: $configured"
    printf '%s\n' "$configured"
    return
  fi
  if [[ -x /opt/rocm/llvm/bin/$name ]]; then
    printf '/opt/rocm/llvm/bin/%s\n' "$name"
    return
  fi
  command -v "$name" || fail "required tool is unavailable: $name"
}

require_pattern() {
  local pattern=$1
  local path=$2
  local description=$3
  grep -Eq -- "$pattern" "$path" || fail "$description"
}

reject_pattern() {
  local pattern=$1
  local path=$2
  local description=$3
  if grep -Eq -- "$pattern" "$path"; then
    fail "$description"
  fi
}

prepare() {
  temporary=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-device-link-fixture.XXXXXX")
  hipcc=$(resolve_tool FE2O3_FIXTURE_HIPCC hipcc)
  clang=$(resolve_tool FE2O3_FIXTURE_CLANG clang)
  llvm_dis=$(resolve_tool FE2O3_FIXTURE_LLVM_DIS llvm-dis)
  llvm_link=$(resolve_tool FE2O3_FIXTURE_LLVM_LINK llvm-link)
  llc=$(resolve_tool FE2O3_FIXTURE_LLC llc)
  opt=$(resolve_tool FE2O3_FIXTURE_OPT opt)
  llvm_readelf=$(resolve_tool FE2O3_FIXTURE_LLVM_READELF llvm-readelf)
  cxx=$(resolve_tool FE2O3_FIXTURE_CXX c++)
}

compile_hip() {
  local source=$1
  local target=$2
  local output=$3
  "$hipcc" --offload-arch="$target" -fgpu-rdc --cuda-device-only -O1 \
    -mcode-object-version=5 -emit-llvm -c "$source" -o "$output"
}

run_oracle() {
  "$cxx" -std=c++17 -O2 -Wall -Wextra -Werror "$fixture_dir/oracle.cpp" \
    -o "$temporary/oracle"
  "$temporary/oracle"
  printf '%s\n' \
    'PASS oracle: rust->HIP and HIP->Rust literal outputs match modulo-2^32 contracts'
}

build_positive() {
  compile_hip "$fixture_dir/hip/bidirectional.hip" \
    'gfx942:sramecc+:xnack-' "$temporary/hip.bc"
  "$clang" -target amdgcn-amd-amdhsa -mcpu=gfx942 -x ir -emit-llvm -c \
    "$fixture_dir/rust-device/link-surrogate.amdgpu.ll" -o "$temporary/rust.bc"
  "$llvm_dis" "$temporary/hip.bc" -o "$temporary/hip.ll"

  require_pattern \
    '^define [^(]*i32 @external_scale_bias_v1\(i32 [^,]*, i32 ' \
    "$temporary/hip.ll" 'HIP definition has the wrong symbol or ABI'
  require_pattern \
    '^declare [^(]*i32 @rust_accumulate_v1\(i32 [^,]*, i32 ' \
    "$temporary/hip.ll" 'HIP module does not import the Rust C ABI symbol'
  require_pattern \
    '^define [^{]*amdgpu_kernel void @hip_calls_rust_kernel_v1\(' \
    "$temporary/hip.ll" 'HIP kernel entry has the wrong role or symbol'
  require_pattern '"target-cpu"="gfx942"' "$temporary/hip.ll" \
    'HIP definitions are not compiled for gfx942'
  require_pattern '"target-features"="[^"]*\+sramecc[^"]*-xnack' \
    "$temporary/hip.ll" \
    'HIP definitions do not have the required sramecc+/xnack- target features'
  require_pattern '!"amdhsa_code_object_version", i32 500' "$temporary/hip.ll" \
    'HIP module does not request code-object version 5'

  "$llvm_link" "$temporary/rust.bc" "$temporary/hip.bc" \
    -o "$temporary/linked.bc"
  "$opt" -passes=verify -disable-output "$temporary/linked.bc"
  "$llvm_dis" "$temporary/linked.bc" -o "$temporary/linked.ll"

  for symbol in external_scale_bias_v1 rust_accumulate_v1; do
    require_pattern "^define .*@${symbol}\\(" "$temporary/linked.ll" \
      "linked closure does not define $symbol"
    reject_pattern "^declare .*@${symbol}\\(" "$temporary/linked.ll" \
      "linked closure still imports $symbol"
  done
  for kernel in rust_calls_hip_kernel_v1 hip_calls_rust_kernel_v1; do
    require_pattern "^define .*amdgpu_kernel void @${kernel}\\(" \
      "$temporary/linked.ll" "linked closure lost kernel role for $kernel"
  done

  "$llc" -mtriple=amdgcn-amd-amdhsa -mcpu=gfx942 -filetype=obj \
    "$temporary/linked.bc" -o "$temporary/linked.o"
  "$llvm_readelf" -sW "$temporary/linked.o" >"$temporary/symbols.txt"
  for function in external_scale_bias_v1 rust_accumulate_v1; do
    require_pattern "FUNC +GLOBAL +(HIDDEN|PROTECTED).* ${function}$" \
      "$temporary/symbols.txt" "object lacks exact C ABI device symbol $function"
  done
  for kernel in rust_calls_hip_kernel_v1 hip_calls_rust_kernel_v1; do
    require_pattern "FUNC +GLOBAL +PROTECTED.* ${kernel}$" \
      "$temporary/symbols.txt" "object lacks protected kernel symbol $kernel"
    require_pattern "OBJECT +GLOBAL +PROTECTED.* ${kernel}\\.kd$" \
      "$temporary/symbols.txt" "object lacks kernel descriptor ${kernel}.kd"
  done
  printf '%s\n' \
    'PASS positive: exact gfx942 ABI symbols form a verified bidirectional LLVM closure'
}

run_adversarial() {
  compile_hip "$fixture_dir/adversarial/missing_definition.hip" \
    'gfx942:sramecc+:xnack-' \
    "$temporary/missing.bc"
  "$llvm_link" "$temporary/rust.bc" "$temporary/missing.bc" \
    -o "$temporary/missing-linked.bc"
  "$llvm_dis" "$temporary/missing-linked.bc" -o "$temporary/missing-linked.ll"
  require_pattern '^declare .*@external_scale_bias_v1\(' \
    "$temporary/missing-linked.ll" \
    'missing-definition case did not retain an unresolved import'
  printf '%s\n' 'PASS reject missing: unresolved external_scale_bias_v1 remains'

  compile_hip "$fixture_dir/adversarial/duplicate_definition.hip" \
    'gfx942:sramecc+:xnack-' \
    "$temporary/duplicate.bc"
  if "$llvm_link" "$temporary/rust.bc" "$temporary/hip.bc" \
      "$temporary/duplicate.bc" -o "$temporary/duplicate-linked.bc" \
      >"$temporary/duplicate.stdout" 2>"$temporary/duplicate.stderr"; then
    fail 'duplicate strong definition was accepted by llvm-link'
  fi
  require_pattern '(multiply defined|symbol multiply defined)' \
    "$temporary/duplicate.stderr" \
    'duplicate case failed without a duplicate-definition diagnostic'
  printf '%s\n' 'PASS reject duplicate: llvm-link rejects the second strong definition'

  compile_hip "$fixture_dir/adversarial/wrong_role_definition.hip" \
    'gfx942:sramecc+:xnack-' \
    "$temporary/wrong-role.bc"
  "$llvm_dis" "$temporary/wrong-role.bc" -o "$temporary/wrong-role.ll"
  require_pattern \
    '^define .*amdgpu_kernel void @external_scale_bias_v1\(' \
    "$temporary/wrong-role.ll" \
    'wrong-role case is not an AMDGPU kernel definition'
  reject_pattern '^define [^(]*i32 @external_scale_bias_v1\(' \
    "$temporary/wrong-role.ll" \
    'wrong-role case unexpectedly provides the scalar device ABI'
  printf '%s\n' 'PASS reject wrong-role: external symbol is a kernel, not a device function'

  compile_hip "$fixture_dir/adversarial/abi_mismatched_definition.hip" \
    'gfx942:sramecc+:xnack-' \
    "$temporary/abi-mismatch.bc"
  "$llvm_dis" "$temporary/abi-mismatch.bc" -o "$temporary/abi-mismatch.ll"
  require_pattern '^define [^(]*i64 @external_scale_bias_v1\(i64 [^,]*, i32 ' \
    "$temporary/abi-mismatch.ll" \
    'ABI-mismatch case does not expose its intended u64 ABI'
  reject_pattern '^define [^(]*i32 @external_scale_bias_v1\(i32 [^,]*, i32 ' \
    "$temporary/abi-mismatch.ll" \
    'ABI-mismatch case unexpectedly provides the required ABI'
  printf '%s\n' 'PASS reject ABI: definition is (u64,u32)->u64, import is (u32,u32)->u32'

  compile_hip "$fixture_dir/adversarial/wrong_target_definition.hip" \
    'gfx90a:sramecc+:xnack-' \
    "$temporary/wrong-target.bc"
  "$llvm_dis" "$temporary/wrong-target.bc" -o "$temporary/wrong-target.ll"
  require_pattern '^define [^(]*i32 @external_scale_bias_v1\(i32 [^,]*, i32 ' \
    "$temporary/wrong-target.ll" \
    'wrong-target case changed the physical ABI'
  require_pattern '"target-cpu"="gfx90a"' "$temporary/wrong-target.ll" \
    'wrong-target case was not compiled for gfx90a'
  reject_pattern '"target-cpu"="gfx942"' "$temporary/wrong-target.ll" \
    'wrong-target definition unexpectedly contains gfx942 code'
  printf '%s\n' 'PASS reject target: valid ABI definition is gfx90a rather than gfx942'
}

usage() {
  printf 'usage: %s oracle|positive|adversarial|all\n' "$0" >&2
  exit 64
}

prepare
case ${1:-all} in
  oracle)
    run_oracle
    ;;
  positive)
    build_positive
    ;;
  adversarial)
    build_positive
    run_adversarial
    ;;
  all)
    run_oracle
    build_positive
    run_adversarial
    ;;
  *)
    usage
    ;;
esac
