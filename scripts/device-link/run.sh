#!/usr/bin/env bash
set -euo pipefail

repo_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)
example_manifest="$repo_root/examples/device-link-ffi/Cargo.toml"
device_manifest="$repo_root/tests/fixtures/device-link/rust-device/Cargo.toml"
export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"$repo_root/target/device-link-ffi"}

usage() {
  printf 'usage: %s cpu|compile|hardware|all\n' "$0" >&2
  exit 64
}

run_cpu() {
  cargo test --manifest-path "$example_manifest" --offline independent_cpu_oracle
  cargo run --manifest-path "$example_manifest" --offline --quiet
}

run_compile() {
  cargo check --manifest-path "$example_manifest" --offline --all-targets
  cargo check --manifest-path "$device_manifest" --offline --lib
  printf '%s\n' 'COMPILE_ONLY Rust FFI macros and fixture contracts checked; no device link or execution occurred'
}

run_hardware() {
  printf '%s\n' \
    'UNAVAILABLE hardware: authenticated G5/G6 publication and typed runtime loading are not integrated' >&2
  return 77
}

case ${1:-} in
  cpu)
    run_cpu
    ;;
  compile)
    run_compile
    ;;
  hardware)
    run_hardware
    ;;
  all)
    run_cpu
    run_compile
    run_hardware
    ;;
  *)
    usage
    ;;
esac
