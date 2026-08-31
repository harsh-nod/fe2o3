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
  fe2o3-compiler-execution-supervisor \
  fe2o3-static-preexec-launcher \
  fe2o3-compiler-execution-issuer \
  fe2o3-external-anchor-provisioning-helper \
  fe2o3-external-anchor-service \
  fe2o3-compiler-execution-provision; do
  grep -Fq -- "${image}\"" "${builder}" || fail "missing image ${image}"
done

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
readonly qualification_boot_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/boot.rs"
readonly qualification_cgroup_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/cgroup.rs"
readonly qualification_run_source="${repo_root}/crates/fe2o3-compiler-execution-deployment/src/run.rs"
bash -n "${verifier_builder}"
for binary in \
  fe2o3-compiler-execution-manifest \
  fe2o3-compiler-execution-deployment-verify \
  fe2o3-compiler-execution-deployment-install \
  fe2o3-compiler-execution-qualification; do
  grep -Fq -- "${binary}" "${verifier_builder}" || fail "missing static image ${binary}"
done
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
  'compiler-execution-supervisor.sock' \
  'boot_and_stop_systemd_machine_v1'; do
  grep -Fq -- "${boot_contract}" "${qualification_boot_source}" ||
    fail "missing isolated systemd boot contract ${boot_contract}"
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
for fault_contract in \
  QualificationFaultPointV1 \
  SystemdVersionComplete \
  SystemdSysusersComplete \
  SystemdTmpfilesComplete \
  SystemdUnitVerifyComplete \
  SystemdPostconditionsAdmitted \
  InstalledLowerRevalidated \
  SystemdMachineSpawned \
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

printf 'compiler-execution deployment-bundle inputs are complete\n'
