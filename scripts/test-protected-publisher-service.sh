#!/usr/bin/env bash
set -euo pipefail

repo=$(CDPATH= cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)
cd -- "${repo}"

export CARGO_TARGET_DIR=${CARGO_TARGET_DIR:-"${repo}/target/protected-publisher-conformance"}

cargo test --locked -p fe2o3-protected-publisher
cargo test --locked --release -p fe2o3-protected-publisher
python3 scripts/tests/parity-publisher-client.py
cargo clippy --locked -p fe2o3-protected-publisher --all-targets --all-features -- -D warnings
cargo fmt --all -- --check
git diff --check

printf '%s\n' 'protected publisher reference-service conformance: PASS'
