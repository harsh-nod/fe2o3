#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
example_manifest="$repo_root/examples/device-link-ffi/Cargo.toml"
device_manifest="$repo_root/tests/fixtures/device-link/rust-device/Cargo.toml"
external_ir="$repo_root/tests/fixtures/device-link/external.amdgpu.ll"
rust_toolchain=${FE2O3_RUST_TOOLCHAIN:-nightly-2026-04-03}
rustup_bin=${FE2O3_RUSTUP:-rustup}
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$repo_root/target/device-link-ffi"}

usage() {
  printf 'usage: %s cpu-source-model|source-check|llvm-verify|hardware|all\n' "$0" >&2
  exit 64
}

require_pinned_rust() {
  if ! command -v "$rustup_bin" >/dev/null 2>&1; then
    printf 'ERROR source-check: rustup executable not found: %s\n' "$rustup_bin" >&2
    return 69
  fi
  if ! "$rustup_bin" run "$rust_toolchain" rustc --version >/dev/null 2>&1; then
    printf 'ERROR source-check: required Rust toolchain is unavailable: %s\n' \
      "$rust_toolchain" >&2
    return 69
  fi
}

pinned_cargo() {
  "$rustup_bin" run "$rust_toolchain" cargo "$@"
}

run_cpu_source_model() {
  require_pinned_rust
  pinned_cargo test --manifest-path "$example_manifest" --locked --offline \
    independent_cpu_oracle
  pinned_cargo run --manifest-path "$example_manifest" --locked --offline --quiet
}

run_source_check() {
  require_pinned_rust
  pinned_cargo check --manifest-path "$example_manifest" --locked --offline --all-targets
  pinned_cargo check --manifest-path "$device_manifest" --locked --offline --lib
  printf '%s\n' \
    'SOURCE_CHECK Rust source and FFI declarations checked; no GPU compilation, link, load, or execution occurred'
}

resolve_llvm_tool() {
  local configured=$1
  local fallback=$2
  if [[ -n $configured ]]; then
    if [[ ! -x $configured ]]; then
      return 1
    fi
    printf '%s\n' "$configured"
    return 0
  fi
  command -v "$fallback"
}

run_llvm_verify() {
  local llvm_as
  local opt
  if ! llvm_as=$(resolve_llvm_tool "${FE2O3_LLVM_AS:-}" llvm-as) \
    || ! opt=$(resolve_llvm_tool "${FE2O3_OPT:-}" opt); then
    printf '%s\n' \
      'UNAVAILABLE llvm-verify: llvm-as and opt were not both found; set FE2O3_LLVM_AS and FE2O3_OPT' >&2
    return 77
  fi

  local temporary
  temporary=$(mktemp -d "${TMPDIR:-/tmp}/fe2o3-device-link-llvm.XXXXXX")
  if ! "$llvm_as" "$external_ir" -o "$temporary/external.bc"; then
    rm -rf "$temporary"
    return 1
  fi
  if ! "$opt" -passes=verify -disable-output "$temporary/external.bc"; then
    rm -rf "$temporary"
    return 1
  fi
  rm -rf "$temporary"
  printf '%s\n' \
    'LLVM_VERIFIED external source assembled and verified; no GPU compilation, link, load, or execution occurred'
}

run_hardware() {
  printf '%s\n' \
    'UNAVAILABLE hardware: no compiler-derived closure, production loader, or hardware execution is implemented' >&2
  return 77
}

run_all() {
  run_cpu_source_model
  run_source_check

  local unavailable=0
  local status
  if run_llvm_verify; then
    :
  else
    status=$?
    if [[ $status -eq 77 ]]; then
      unavailable=1
    else
      return "$status"
    fi
  fi
  if run_hardware; then
    :
  else
    status=$?
    if [[ $status -eq 77 ]]; then
      unavailable=1
    else
      return "$status"
    fi
  fi
  if [[ $unavailable -ne 0 ]]; then
    return 77
  fi
}

case ${1:-} in
  cpu-source-model)
    run_cpu_source_model
    ;;
  source-check)
    run_source_check
    ;;
  llvm-verify)
    run_llvm_verify
    ;;
  hardware)
    run_hardware
    ;;
  all)
    run_all
    ;;
  *)
    usage
    ;;
esac
