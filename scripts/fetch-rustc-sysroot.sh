#!/usr/bin/env bash
# Resolve the selected rustc's complete rust-src lockfile before offline
# build-std consumers. This downloads dependencies; it does not build a sysroot.
set -Eeuo pipefail

sysroot="$(rustc --print sysroot)"
if [[ -z "${sysroot}" || "${sysroot}" != /* ||
      "${sysroot}" == *$'\n'* || "${sysroot}" == *$'\r'* ]]; then
  printf 'rustc returned an invalid sysroot path\n' >&2
  exit 2
fi
library="${sysroot}/lib/rustlib/src/rust/library"
if [[ ! -f "${library}/Cargo.toml" || ! -f "${library}/Cargo.lock" ]]; then
  printf 'selected rustc requires its rust-src manifest and lockfile\n' >&2
  exit 2
fi
cargo fetch --locked --manifest-path "${library}/Cargo.toml"
