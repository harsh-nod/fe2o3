#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd -P)"
readonly repo_root
readonly target_dir="${FE2O3_STATIC_DEPLOYMENT_VERIFIER_TARGET_DIR:-${repo_root}/target/static-deployment-verifier}"
readonly target="x86_64-unknown-linux-musl"
readonly manifest="${target_dir}/${target}/release/fe2o3-compiler-execution-manifest"
readonly verifier="${target_dir}/${target}/release/fe2o3-compiler-execution-deployment-verify"
readonly installer="${target_dir}/${target}/release/fe2o3-compiler-execution-deployment-install"
readonly qualification="${target_dir}/${target}/release/fe2o3-compiler-execution-qualification"

cd -- "${repo_root}"
CARGO_TARGET_DIR="${target_dir}/profile-test" \
  cargo test --locked -p fe2o3-compiler-execution-deployment --all-targets

for binary in \
  fe2o3-compiler-execution-manifest \
  fe2o3-compiler-execution-deployment-verify \
  fe2o3-compiler-execution-deployment-install \
  fe2o3-compiler-execution-qualification; do
  CARGO_TARGET_DIR="${target_dir}" cargo rustc \
    --locked \
    --release \
    --target "${target}" \
    -p fe2o3-compiler-execution-deployment \
    --bin "${binary}" \
    -- \
    -C target-feature=+crt-static \
    -C relocation-model=static \
    -C link-arg=-static \
    -C link-arg=-no-pie

  executable="${target_dir}/${target}/release/${binary}"
  report="${target_dir}/${binary}.readelf.txt"
  /usr/bin/readelf -hW -lW -dW -sW -- "${executable}" >"${report}"
  /usr/bin/grep -Eq 'Class:[[:space:]]+ELF64' "${report}"
  /usr/bin/grep -Eq 'Type:[[:space:]]+EXEC' "${report}"
  if /usr/bin/grep -Eq 'INTERP|DYNAMIC|\(NEEDED\)|\(RPATH\)|\(RUNPATH\)' "${report}"; then
    printf '%s contains a dynamic-loader dependency\n' "${binary}" >&2
    exit 1
  fi
  /usr/bin/grep -Eq 'GNU_STACK.*RW[[:space:]]' "${report}"
  undefined_symbols="$(/usr/bin/nm -u -- "${executable}")"
  if [[ -n "${undefined_symbols}" ]]; then
    printf '%s contains undefined symbols\n' "${binary}" >&2
    exit 1
  fi
done

set +e
manifest_usage="$(/usr/bin/env -i "${manifest}" 2>&1)"
manifest_status=$?
verifier_usage="$(/usr/bin/env -i "${verifier}" 2>&1)"
verifier_status=$?
installer_usage="$(/usr/bin/env -i "${installer}" 2>&1)"
installer_status=$?
qualification_usage="$(/usr/bin/env -i "${qualification}" 2>&1)"
qualification_status=$?
set -e
if [[ ${manifest_status} -ne 2 \
  || "${manifest_usage}" != 'usage: fe2o3-compiler-execution-manifest BUNDLE_ROOT GIT_COMMIT TARGET' ]]; then
  printf 'static deployment manifest generator argument gate changed\n' >&2
  exit 1
fi
if [[ ${verifier_status} -ne 2 \
  || "${verifier_usage}" != 'usage: fe2o3-compiler-execution-deployment-verify BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT' ]]; then
  printf 'static deployment verifier argument gate changed\n' >&2
  exit 1
fi
if [[ ${installer_status} -ne 2 \
  || "${installer_usage}" != 'usage: fe2o3-compiler-execution-deployment-install BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT' ]]; then
  printf 'static deployment installer argument gate changed\n' >&2
  exit 1
fi
if [[ ${qualification_status} -ne 2 \
  || "${qualification_usage}" != 'usage: fe2o3-compiler-execution-qualification probe'$'\n''       fe2o3-compiler-execution-qualification fault-points'$'\n''       fe2o3-compiler-execution-qualification recover QUALIFICATION_PARENT'$'\n''       fe2o3-compiler-execution-qualification recover-install EXPECTED_MANIFEST_SHA256 INSTALL_PARENT'$'\n''       fe2o3-compiler-execution-qualification run BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT'$'\n''       fe2o3-compiler-execution-qualification fault POINT BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT'$'\n''       fe2o3-compiler-execution-qualification campaign BUNDLE_ROOT EXPECTED_MANIFEST_SHA256 EXPECTED_GIT_COMMIT EMPTY_INSTALL_PARENT BASE_IMAGE EXPECTED_BASE_IMAGE_SHA256 QUALIFICATION_PARENT' ]]; then
  printf 'static qualification harness argument gate changed\n' >&2
  exit 1
fi

set +e
preflight_helper_failure="$(/usr/bin/env -i "${qualification}" \
  __systemd-preflight-tool-v1 systemd-sysusers </dev/null 2>&1)"
preflight_helper_status=$?
set -e
if [[ ${preflight_helper_status} -ne 1 \
  || "${preflight_helper_failure}" != \
    'compiler-execution systemd preflight boundary failed: expected parent PID is missing' ]]; then
  printf 'static systemd preflight helper parent boundary changed\n' >&2
  exit 1
fi

set +e
machine_helper_failure="$(/usr/bin/env -i "${qualification}" \
  __systemd-machine-tool-v1 \
  .compiler-execution-qualification-v1-0123456789abcdef0123456789abcdef \
  </dev/null 2>&1)"
machine_helper_status=$?
set -e
if [[ ${machine_helper_status} -ne 1 \
  || "${machine_helper_failure}" != \
    'compiler-execution systemd machine boundary failed: expected parent PID is missing' ]]; then
  printf 'static systemd machine helper parent boundary changed\n' >&2
  exit 1
fi

qualification_fault_points="$(/usr/bin/env -i "${qualification}" fault-points)"
readonly qualification_fault_points
if [[ "${qualification_fault_points}" != 'loop-attached'$'\n''base-mounted'$'\n''overlay-mounted'$'\n''projection-revalidated'$'\n''systemd-version-complete'$'\n''systemd-version-revalidated'$'\n''systemd-sysusers-complete'$'\n''systemd-sysusers-revalidated'$'\n''systemd-tmpfiles-complete'$'\n''systemd-tmpfiles-revalidated'$'\n''systemd-unit-verify-complete'$'\n''systemd-unit-verify-revalidated'$'\n''systemd-postconditions-admitted'$'\n''installed-lower-revalidated'$'\n''systemd-machine-spawned'$'\n''systemd-machine-ready'$'\n''systemd-machine-stopped'$'\n''post-boot-lower-revalidated'$'\n''overlay-unmounted'$'\n''base-unmounted'$'\n''loop-released'$'\n''staging-cleaned' ]]; then
  printf 'static qualification fault set changed\n' >&2
  exit 1
fi

qualification_probe="$(/usr/bin/env -i "${qualification}" probe)"
readonly qualification_probe
if [[ "$(printf '%s\n' "${qualification_probe}" | /usr/bin/awk 'END { print NR }')" -ne 13 ]] \
  || ! /usr/bin/grep -Eq '^probe_schema=fe2o3-compiler-execution-qualification-host-probe-v1$' <<<"${qualification_probe}" \
  || ! /usr/bin/grep -Eq '^effective_uid=[0-9]+$' <<<"${qualification_probe}" \
  || ! /usr/bin/grep -Eq '^task_count=[1-9][0-9]*$' <<<"${qualification_probe}" \
  || ! /usr/bin/grep -Eq '^mount_ready=(true|false)$' <<<"${qualification_probe}" \
  || ! /usr/bin/grep -Eq '^isolated_systemd_ready=(true|false)$' <<<"${qualification_probe}"; then
  printf 'static qualification host probe report changed\n' >&2
  exit 1
fi

printf 'manifest_generator=%s\n' "${manifest}"
printf 'deployment_verifier=%s\n' "${verifier}"
printf 'deployment_installer=%s\n' "${installer}"
printf 'qualification_harness=%s\n' "${qualification}"
