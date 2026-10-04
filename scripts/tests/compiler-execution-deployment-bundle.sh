#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd -P)"
readonly repo_root
readonly builder="${repo_root}/scripts/build-static-compiler-execution-deployment.sh"

fail() {
  printf 'compiler-execution deployment-bundle contract failed: %s\n' "$*" >&2
  exit 1
}

bash -n "${builder}"
set +e
usage="$(${builder} 2>&1)"
status=$?
set -e
[[ ${status} -eq 2 && "${usage}" == usage:* ]] || fail 'builder argument gate changed'

# Execute the actual argument/jobs prologue only. No Git, Cargo, CMake, output
# directories, or synthetic successful bundle are involved in these controls.
[[ "$(grep -cx 'umask 077' "${builder}")" == 1 ]] ||
  fail 'expected one build side-effect boundary'
readonly job_prologue="$(sed '/^umask 077$/,$d' "${builder}")"
[[ "${job_prologue}" == *'export CMAKE_BUILD_PARALLEL_LEVEL="${jobs}"'* ]] ||
  fail 'job validation must precede the build side-effect boundary'
readonly job_report='[[ $(declare -p CARGO_BUILD_JOBS) == "declare -x "* ]] &&
[[ $(declare -p CMAKE_BUILD_PARALLEL_LEVEL) == "declare -x "* ]] || exit 91
printf "cargo=%s cmake=%s jobs=%s\n" "$CARGO_BUILD_JOBS" "$CMAKE_BUILD_PARALLEL_LEVEL" "$jobs"'
default_jobs="$(env -i PATH=/dev/null CMAKE_BUILD_PARALLEL_LEVEL=0 \
  /bin/bash -c "${job_prologue}"$'\n'"${job_report}" job-control unused-output)"
[[ "${default_jobs}" == 'cargo=1 cmake=1 jobs=1' ]] || fail 'unset jobs must default to one'
for jobs in {1..16}; do
  observed="$(env -i PATH=/dev/null CARGO_BUILD_JOBS="${jobs}" CMAKE_BUILD_PARALLEL_LEVEL=999 \
    /bin/bash -c "${job_prologue}"$'\n'"${job_report}" job-control unused-output)"
  [[ "${observed}" == "cargo=${jobs} cmake=${jobs} jobs=${jobs}" ]] ||
    fail "Cargo/CMake job bound differs for ${jobs}"
done
for jobs in '' 0 -1 +1 01 1.0 1e1 auto 17 64 999999999999999999999999999999 \
  ' 1' '1 ' $'1\n' $'1\n2' '1+1' '$(exit 99)' '1; exit 99'; do
  set +e
  # An empty command search path makes accidental work past validation fail
  # without invoking a build, even when these tests run on a clean checkout.
  rejection="$(env -i PATH=/dev/null CARGO_BUILD_JOBS="${jobs}" \
    /bin/bash "${builder}" unused-output 2>&1)"
  status=$?
  set -e
  [[ ${status} -eq 2 && "${rejection}" == \
    'CARGO_BUILD_JOBS must be a canonical integer from 1 through 16' ]] ||
    fail 'malformed job bound reached work or changed its refusal'
done
grep -Fxq -- 'cmake --build "${target_root}/launcher" --parallel "${jobs}"' "${builder}" ||
  fail 'launcher build must receive the exact explicit job bound'
if grep -Eq -- '--parallel[[:space:]]*$' "${builder}"; then
  fail 'bare unlimited CMake parallelism is forbidden'
fi

for helper in \
  build-static-compiler-execution-coordinator.sh \
  build-static-compiler-execution-supervisor.sh \
  build-static-compiler-execution-issuer.sh \
  build-static-external-anchor-provisioning-helper.sh \
  build-static-external-anchor-service.sh \
  build-static-compiler-execution-provisioner.sh \
  build-static-compiler-execution-deployment-verifier.sh; do
  grep -Fq -- "scripts/${helper}" "${builder}" || fail "missing ${helper}"
done

for image in \
  fe2o3-compiler-execution-coordinator \
  'fe2o3-compiler-execution-supervisor${image_suffix}' \
  fe2o3-static-preexec-launcher \
  'fe2o3-compiler-execution-issuer${issuer_suffix}' \
  'fe2o3-external-anchor-provisioning-helper${image_suffix}' \
  'fe2o3-external-anchor-service${image_suffix}' \
  fe2o3-compiler-execution-provision; do
  grep -Fq -- "${image}\"" "${builder}" || fail "missing image ${image}"
done

for selection in \
  'scripts/build-static-compiler-execution-supervisor.sh" "${family}"' \
  'scripts/build-static-compiler-execution-issuer.sh" "${issuer_arguments[@]}"' \
  'scripts/build-static-external-anchor-provisioning-helper.sh" "${family}"' \
  'scripts/build-static-external-anchor-service.sh" "${family}"' \
  '"${manifest_generator}" "${manifest_arguments[@]}"' \
  '"${deployment_verifier}" "${manifest_arguments[@]}"' \
  'content_file_count=12' \
  'schema_version=3'; do
  grep -Fq -- "${selection}" "${builder}" || fail "missing V3 selection ${selection}"
done
# Executes both exact family selectors and verifies the V1-only client guards.
# No compiler, provisioner, verifier image, or privileged service is launched.
python3 -B "${repo_root}/scripts/tests/compiler_execution_deployment_families.py"

grep -Fq -- 'ctest --test-dir' "${builder}" || fail 'launcher CTest qualification is missing'
grep -Fq -- 'sha256sum --check --strict SHA256SUMS' "${builder}" ||
  fail 'strict bundle hash verification is missing'
grep -Fq -- 'readonly usr_dir=' "${builder}" || fail 'explicit usr directory custody is missing'
grep -Fq -- 'readonly libexec_dir=' "${builder}" ||
  fail 'explicit libexec directory custody is missing'
grep -Fq -- 'install -d -m 0700' "${builder}" || fail 'exact directory mode creation is missing'
grep -Fq -- 'fe2o3-compiler-execution-manifest' "${builder}" ||
  fail 'pinned install manifest generation is missing'
grep -Fq -- 'fe2o3-compiler-execution-deployment-verify' "${builder}" ||
  fail 'sealed deployment verification is missing'
grep -Fq -- 'manifest_sha256=%s' "${builder}" ||
  fail 'out-of-band manifest digest publication is missing'

readonly verifier_builder="${repo_root}/scripts/build-static-compiler-execution-deployment-verifier.sh"
readonly qualification_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/bin/qualification.rs"
readonly qualification_supervisor_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/supervisor.rs"
readonly qualification_fault_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/fault.rs"
readonly qualification_preflight_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/preflight.rs"
readonly qualification_provision_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/provision.rs"
readonly qualification_boot_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/boot.rs"
readonly qualification_client_transaction_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/client_transaction.rs"
readonly qualification_cgroup_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/cgroup.rs"
readonly qualification_run_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/run.rs"
bash -n "${verifier_builder}"
for binary in \
  fe2o3-compiler-execution-manifest \
  fe2o3-compiler-execution-deployment-verify \
  fe2o3-compiler-execution-deployment-install \
  fe2o3-compiler-runtime-deployment \
  fe2o3-compiler-execution-qualification; do
  grep -Fq -- "${binary}" "${verifier_builder}" || fail "missing static image ${binary}"
done
grep -Fq -- 'package RECIPE SOURCE_ROOT PROFILE_ROOT DESTINATION WORK STORAGE' "${verifier_builder}" ||
  fail 'runtime package command argument contract is missing'
grep -Fq -- 'runtime_package_status' "${verifier_builder}" ||
  fail 'runtime package missing/extra argument gates are missing'
for boot_contract in \
  '/proc/self/fd/' \
  'usr/lib/x86_64-linux-gnu/ld-linux-x86-64.so.2' \
  'usr/bin/systemd-nspawn' \
  'inherit_exec_descriptor' \
  'pidfd_open' \
  'pidfd_send_signal' \
  'getpgid' \
  'getpgrp' \
  '--private-network' \
  '--bind=+/run/fe2o3:/run/fe2o3:norbind,noidmap' \
  'COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1' \
  'MachineSocketReadinessV1' \
  'try_admit_client_transaction_report_v1' \
  'await_client_transaction' \
  'boot_and_stop_systemd_machine_v1'; do
  grep -Fq -- "${boot_contract}" "${qualification_boot_source}" ||
    fail "missing isolated systemd boot contract ${boot_contract}"
done
if grep -Eq -- 'connect\(|getpeername\(|socket_peercred' "${qualification_boot_source}"; then
  fail 'host readiness must not consume the production supervisor session'
fi
for transaction_contract in \
  'compiler-execution-client-check.report' \
  'complete=true' \
  'profile.identity()' \
  'profile.policy().identity()' \
  'client_uid == 0' \
  'report_identity' \
  'read_exact_at' \
  'ResolveFlags::NO_XDEV'; do
  grep -Fq -- "${transaction_contract}" "${qualification_client_transaction_source}" ||
    fail "missing client transaction evidence contract ${transaction_contract}"
done
if grep -Fq -- '.process_group(0)' "${qualification_boot_source}"; then
  fail 'systemd machine helper escapes the supervised worker process group'
fi
for cgroup_contract in \
  '/proc/self/cgroup' \
  '/sys/fs/cgroup' \
  create_compiler_execution_qualification_cgroup_v1 \
  attach_worker \
  'cgroup.procs' \
  'cgroup.events' \
  'cgroup.kill' \
  'accessat' \
  'remove_descendant_cgroups' \
  'CGROUP_MAX_DEPTH_V1' \
  'CGROUP_MAX_DESCENDANTS_V1'; do
  grep -Fq -- "${cgroup_contract}" "${qualification_cgroup_source}" ||
    fail "missing qualification cgroup contract ${cgroup_contract}"
done
grep -Fq -- 'cgroup_v2_scope_writable' "${verifier_builder}" ||
  fail 'static qualification cgroup-writability probe is missing'
if grep -Eq -- 'PINNED_NSPAWN_PATH[^=]*=[[:space:]]*"/usr/bin|Command::new\("/usr/bin/systemd-nspawn"' \
  "${qualification_boot_source}"; then
  fail 'systemd machine launcher trusts a host systemd-nspawn path'
fi
grep -Fq -- 'qualification-host-probe-v1' "${verifier_builder}" ||
  fail 'static qualification prerequisite probe is missing'
grep -Fq -- 'fault-points' "${verifier_builder}" ||
  fail 'static qualification fault set is missing'
for verifier_fault_contract in \
  supervisor-socket-metadata-admitted \
  client-transaction-complete \
  client-transaction-revalidated; do
  grep -Fq -- "${verifier_fault_contract}" "${verifier_builder}" ||
    fail "static qualification fault set is missing ${verifier_fault_contract}"
done
grep -Fq -- 'campaign BUNDLE_ROOT' "${verifier_builder}" ||
  fail 'static qualification campaign is missing'
grep -Fq -- 'recover QUALIFICATION_PARENT' "${verifier_builder}" ||
  fail 'static qualification recovery command is missing'
grep -Fq -- 'recover-install EXPECTED_MANIFEST_SHA256 INSTALL_PARENT' "${verifier_builder}" ||
  fail 'static installer recovery command is missing'
for supervisor_contract in \
  acquire_compiler_execution_qualification_supervisor_lease_v1 \
  wait_for_compiler_execution_qualification_supervisor_lease_v1 \
  wait_for_qualification_worker_v1 \
  set_parent_process_death_signal \
  signal_hook::flag::register_usize \
  WorkerOutputCaptureV1; do
  grep -Fq -- "${supervisor_contract}" "${qualification_source}" ||
    fail "missing qualification supervisor contract ${supervisor_contract}"
done
for preflight_contract in \
  '/proc/self/exe' \
  '/usr/bin/systemd-sysusers' \
  '/usr/bin/systemd-tmpfiles' \
  '/usr/bin/systemd-analyze' \
  run_compiler_execution_systemd_preflight_with_hooks_v1 \
  'rustix::process::chroot' \
  'Resource::Fsize' \
  admit_systemd_version \
  validate_account_databases \
  validate_tmpfiles_projection; do
  grep -Fq -- "${preflight_contract}" "${qualification_preflight_source}" ||
    fail "missing composed-root preflight contract ${preflight_contract}"
done
for provision_contract in \
  '/usr/libexec/fe2o3/fe2o3-compiler-execution-provision' \
  run_compiler_execution_provisioning_with_hooks_v1 \
  execute_compiler_execution_provisioning_tool_v1 \
  admit_provisioned_state \
  require_current_provisioned_state \
  CompilerExecutionIssuerPolicyV1::decode \
  CompilerExecutionClientProfileV1::decode \
  CompilerExecutionSupervisorDeploymentV1::decode \
  CompilerExecutionExternalAnchorDeploymentV1::decode \
  CompilerExecutionExternalAnchorProvisioningV1::decode \
  sealed_static_issuer_runtime_measurement_v1 \
  SigningKey::from_bytes \
  measure_static_image \
  'rustix::process::chroot' \
  'Resource::Fsize'; do
  grep -Fq -- "${provision_contract}" "${qualification_provision_source}" ||
    fail "missing composed-root provisioning contract ${provision_contract}"
done
for fault_contract in \
  QualificationFaultPointV1 \
  SystemdVersionComplete \
  SystemdSysusersComplete \
  SystemdTmpfilesComplete \
  SystemdUnitVerifyComplete \
  SystemdPostconditionsAdmitted \
  InstalledLowerRevalidated \
  CompilerExecutionProvisioningComplete \
  CompilerExecutionProvisioningRevalidated \
  CompilerExecutionProvisioningAdmitted \
  SystemdMachineSpawned \
  SupervisorSocketMetadataAdmitted \
  ClientTransactionComplete \
  ClientTransactionRevalidated \
  SystemdMachineReady \
  SystemdMachineStopped \
  PostBootLowerRevalidated \
  StagingCleaned; do
  grep -Fq -- "${fault_contract}" "${qualification_fault_source}" ||
    fail "missing unified qualification fault contract ${fault_contract}"
done
grep -Fq -- 'run_compiler_execution_qualification_request_v1' "${qualification_run_source}" ||
  fail 'unified qualification run path is missing'
grep -Fq -- 'execute_staged_qualification_with_hooks' "${qualification_run_source}" ||
  fail 'shared normal/fault qualification transaction is missing'
grep -Fq -- 'run_compiler_execution_provisioning_with_hooks_v1' "${qualification_run_source}" ||
  fail 'production provisioning is missing from the unified qualification transaction'
grep -Fq -- 'revalidate_qualification_inputs_after_fault' "${qualification_run_source}" ||
  fail 'post-fault installed-lower revalidation is missing'
if grep -Fq -- 'run_compiler_execution_mount_qualification_request_v1' "${qualification_run_source}"; then
  fail 'legacy mount-only qualification run path remains'
fi
if grep -Eq -- 'run_compiler_execution_mount_(fault|campaign)_v1|QualificationMountFaultPointV1' \
  "${qualification_run_source}" "${qualification_source}"; then
  fail 'legacy mount-only fault path remains'
fi
grep -Fq -- '.process_group(0)' "${qualification_source}" ||
  fail 'qualification worker process-group isolation is missing'
for process_tree_contract in \
  pidfd_open \
  'WaitIdOptions::NOWAIT' \
  kill_process_group; do
  grep -Fq -- "${process_tree_contract}" "${qualification_supervisor_source}" ||
    fail "missing qualification process-tree contract ${process_tree_contract}"
done
grep -Fq -- "--target \"\${target}\"" "${verifier_builder}" ||
  fail 'static verifier target is not pinned'
grep -Fq -- '-C link-arg=-static' "${verifier_builder}" ||
  fail 'static verifier link contract is missing'
grep -Fq -- "'INTERP|DYNAMIC|\\(NEEDED\\)|\\(RPATH\\)|\\(RUNPATH\\)'" "${verifier_builder}" ||
  fail 'static verifier loader-independence gate is missing'

printf 'V1/V3 deployment-bundle source contracts and frozen V1 qualification contract checked\n'
