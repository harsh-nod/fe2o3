#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root

if [[ $# -ne 2 ]]; then
  printf 'usage: %s <dedicated-service-uid> <dedicated-service-gid>\n' "$0" >&2
  exit 2
fi
readonly service_uid="$1"
readonly service_gid="$2"
if [[ ! "${service_uid}" =~ ^[1-9][0-9]*$ || ! "${service_gid}" =~ ^[1-9][0-9]*$ ]]; then
  printf 'service UID and GID must be nonzero decimal identities\n' >&2
  exit 2
fi

if [[ ${EUID} -ne 0 ]]; then
  readonly target_root="${FE2O3_ROOT_ANCHOR_TARGET_DIR:-${repo_root}/target/root-anchor-qualification}"
  helper_target="${target_root}/helper"
  daemon_target="${target_root}/daemon"
  FE2O3_STATIC_ANCHOR_HELPER_TARGET_DIR="${helper_target}" \
    bash "${repo_root}/scripts/build-static-external-anchor-provisioning-helper.sh" >/dev/null
  FE2O3_STATIC_ANCHOR_TARGET_DIR="${daemon_target}" \
    bash "${repo_root}/scripts/build-static-external-anchor-service.sh" >/dev/null
  readonly helper="${helper_target}/x86_64-unknown-linux-musl/release/fe2o3-external-anchor-provisioning-helper"
  readonly daemon="${daemon_target}/x86_64-unknown-linux-musl/release/fe2o3-external-anchor-service"
  readonly cargo_bin="$(command -v cargo)"
  readonly cargo_home="${CARGO_HOME:-${HOME}/.cargo}"
  readonly rustup_home="${RUSTUP_HOME:-${HOME}/.rustup}"
  exec sudo \
    FE2O3_ROOT_ANCHOR_HELPER="${helper}" \
    FE2O3_ROOT_ANCHOR_DAEMON="${daemon}" \
    FE2O3_ROOT_ANCHOR_UID="${service_uid}" \
    FE2O3_ROOT_ANCHOR_GID="${service_gid}" \
    FE2O3_ROOT_CARGO="${cargo_bin}" \
    CARGO_HOME="${cargo_home}" \
    RUSTUP_HOME="${rustup_home}" \
    RUSTUP_TOOLCHAIN="${RUSTUP_TOOLCHAIN:-nightly-2026-04-03}" \
    FE2O3_ROOT_ANCHOR_TEST_TARGET_DIR="${target_root}/root-test" \
    "${repo_root}/scripts/qualify-root-external-anchor-coordinator.sh" \
    "${service_uid}" "${service_gid}"
fi

: "${FE2O3_ROOT_ANCHOR_HELPER:?root helper image is required}"
: "${FE2O3_ROOT_ANCHOR_DAEMON:?root daemon image is required}"
: "${FE2O3_ROOT_ANCHOR_UID:?root service UID is required}"
: "${FE2O3_ROOT_ANCHOR_GID:?root service GID is required}"
: "${FE2O3_ROOT_CARGO:?root cargo path is required}"
: "${FE2O3_ROOT_ANCHOR_TEST_TARGET_DIR:?root test target directory is required}"
[[ "${FE2O3_ROOT_ANCHOR_UID}" == "${service_uid}" ]]
[[ "${FE2O3_ROOT_ANCHOR_GID}" == "${service_gid}" ]]

cd -- "${repo_root}"
CARGO_TARGET_DIR="${FE2O3_ROOT_ANCHOR_TEST_TARGET_DIR}" \
  "${FE2O3_ROOT_CARGO}" test --locked \
    -p fe2o3-external-anchor-coordinator \
    --test root_distinct_uid \
    real_distinct_uid_helper_daemon_exchange_and_restart \
    -- --exact --ignored --nocapture
