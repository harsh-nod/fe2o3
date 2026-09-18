// Synthetic receipt-shape fixtures ONLY. No compilation, actual stage execution,
// package qualification, source callback, or successful join is represented.
import path from 'node:path';
import { LLVM_VERSION, LLVM_BUILD_ID, TEST_TARGET, MIN_FREE_BYTES,
  nativeBuildMeasurementRoster } from './assembly-region-worker-prototype.mjs';

export function syntheticNativeBuildReceipt(repo = '/synthetic-native-test/repo', output = '/synthetic-native-test/native') {
  const source = path.join(repo, 'tools/fe2o3-llvm-link-worker'), build = path.join(output, 'build');
  const cmake = path.join(output, 'tools/cmake'), packageRoot = path.join(output, 'package');
  const configure = { executable: cmake, args: ['-S', source, '-B', build, '-G', 'Unix Makefiles',
    `-DLLVM_DIR=${packageRoot}/lib/cmake/llvm`, `-DLLD_DIR=${packageRoot}/lib/cmake/lld`,
    `-DFE2O3_PINNED_LLVM_VERSION=${LLVM_VERSION}`, `-DFE2O3_EXPECTED_LLVM_BUILD_ID=${LLVM_BUILD_ID}`,
    `-DFE2O3_LLVM_BUILD_ID_FILE=${packageRoot}/build-id.txt`,
    `-DFE2O3_GFX942_DEVICE_LIB_DIR=${output}/no-device-libraries`, `-DFE2O3_GFX950_DEVICE_LIB_DIR=${output}/no-device-libraries`,
    `-Dzstd_INCLUDE_DIR=${packageRoot}/include`, `-Dzstd_LIBRARY=${packageRoot}/libzstd.so`,
    '-DCMAKE_BUILD_TYPE=Release', `-DCMAKE_CXX_COMPILER=${output}/tools/cxx`, '-DBUILD_TESTING=ON'] };
  const stage = (name, executable, args) => ({ stage: name, executable, args, code: 0, signal: null, reason: null,
    elapsed_ms: 1, free_bytes_before: MIN_FREE_BYTES.toString(), stdout_sha256: '1'.repeat(64), stdout_bytes: 1,
    stderr_sha256: '2'.repeat(64), stderr_bytes: 0 });
  const roster = nativeBuildMeasurementRoster({ repo, output, configure });
  const measurement = item => ({ requested: item.requested, resolved: item.requested, bytes: 1, sha256: '3'.repeat(64) });
  return { schema: 'fe2o3-ordered-inline-unit-engineering-receipt-v1', status: 'passed',
    scope: 'native-test-fixture transport/encoding observation only', source_produced: false,
    production_exact_region_admission: false, protected_finalizer_admission: false, hardware_executed: false,
    runtime_closure_attestation: 'unavailable', package_identity_kind: 'existing asserted package build-ID, not runtime closure',
    policy_or_source_gate_changes: false,
    environment: { platform: 'linux', arch: 'x64', node: 'v22.0.0', os_release: 'synthetic-not-executed',
      compiler_repo: repo, output, compiler_head: '4'.repeat(40), compiler_worktree_dirty: true },
    limits: { jobs: 2, minimum_free_bytes: MIN_FREE_BYTES.toString(), configure_ms: 120000,
      build_ms: 900000, test_ms: 120000, test_stdout_bytes: 65536 },
    stages: [stage('git-head', '/usr/bin/git', ['rev-parse', 'HEAD']),
      stage('git-status', '/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal']),
      stage('configure', cmake, configure.args),
      stage('build', cmake, ['--build', build, '--target', TEST_TARGET, 'fe2o3-llvm-link-worker', '--parallel', '2']),
      stage('native-test', path.join(build, TEST_TARGET), [])],
    inputs: roster.inputs.map(measurement), artifacts: roster.artifacts.map(measurement),
    observation: { sha256: '5'.repeat(64), bytes: 1, worker_build_claim: `fe2o3-worker-v1-sha256-${'6'.repeat(64)}`,
      positive_cases: 4, native_test_control_counts: [12, 8, 2] } };
}
