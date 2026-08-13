#!/usr/bin/env bash
set -euo pipefail

repo=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd -- "${repo}"

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"${repo}/target/protected-publisher-conformance"}
export PYTHONDONTWRITEBYTECODE=1

if [[ -n "$(find scripts -type d -name __pycache__ -print -quit)" ]] ||
  [[ -n "$(find scripts -type f -name '*.pyc' -print -quit)" ]]; then
  printf '%s\n' 'protected publisher conformance: preexisting Python bytecode' >&2
  exit 1
fi
checkout_before=$(git status --porcelain=v1 --untracked-files=all)

cargo test --locked -p fe2o3-protected-publisher
cargo build --locked -p fe2o3-protected-publisher
python3 scripts/tests/protected-publisher-secret-memory.py \
  "${CARGO_TARGET_DIR}/debug/fe2o3-protected-publisher"
cargo test --locked --release -p fe2o3-protected-publisher
python3 scripts/tests/parity-publisher-client.py
cargo clippy --locked -p fe2o3-protected-publisher --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check

checkout_after=$(git status --porcelain=v1 --untracked-files=all)
if [[ "${checkout_after}" != "${checkout_before}" ]] ||
  [[ -n "$(find scripts -type d -name __pycache__ -print -quit)" ]] ||
  [[ -n "$(find scripts -type f -name '*.pyc' -print -quit)" ]]; then
  printf '%s\n' 'protected publisher conformance: checkout pollution detected' >&2
  diff -u <(printf '%s\n' "${checkout_before}") <(printf '%s\n' "${checkout_after}") >&2 || true
  exit 1
fi

printf '%s\n' 'protected publisher reference-service conformance: PASS'
