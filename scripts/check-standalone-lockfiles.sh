#!/usr/bin/env bash

set -Eeuo pipefail

SCRIPT_DIR="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
readonly SCRIPT_DIR
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/.." && pwd)"
readonly REPO_ROOT
readonly CARGO_BIN="${CARGO:-cargo}"

mapfile -d '' LOCKFILES < <(
  git -C "${REPO_ROOT}" ls-files -z -- '*/Cargo.lock' | sort -z
)

for lockfile in "${LOCKFILES[@]}"; do
  manifest="${lockfile%Cargo.lock}Cargo.toml"
  if [[ ! -f "${REPO_ROOT}/${manifest}" ]]; then
    printf 'tracked standalone lockfile has no manifest: %s\n' "${lockfile}" >&2
    exit 2
  fi

  printf 'checking standalone lockfile: %s\n' "${lockfile}"
  "${CARGO_BIN}" metadata \
    --locked \
    --format-version 1 \
    --manifest-path "${REPO_ROOT}/${manifest}" \
    >/dev/null
done

printf 'standalone lockfiles: OK (%d checked)\n' "${#LOCKFILES[@]}"
