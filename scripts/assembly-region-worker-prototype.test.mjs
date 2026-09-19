// Synthetic report-shape controls and explicitly mocked child commands only.
// These tests do not compile code, qualify LLVM, or write compilation receipts.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { LLVM_BUILD_ID, parseArguments, parseObservation, readRegular,
  runBoundedCommand, validateObservation, WORKER_FILES, nativeBuildMeasurementRoster,
  validateNativeBuildReceipt, measureNativeBuildInput, requireNativeBuildInputsUnchanged } from './assembly-region-worker-prototype.mjs';
import { syntheticNativeBuildReceipt } from './assembly-region-worker-prototype.test-fixtures.mjs';

const CLAIM = `fe2o3-worker-v1-sha256-${'a'.repeat(64)}`;
const DIAGNOSTICS = [
  'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
  'post_link.check=exports status=ok symbols=[ordered_region_fixture,ordered_region_fixture.kd]',
  'post_link.check=unresolved status=ok symbols=[]',
  'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-',
  'post_link.kernel name=ordered_region_fixture symbol=ordered_region_fixture.kd kernarg_size=280 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]',
];

function site(file_offset, opcode, bytes_hex, register_operands) {
  return { file_offset, opcode, bytes_hex, register_operands, mc_flags: 0,
    implicit_reads: ['EXEC'], implicit_writes: [] };
}

// Clearly synthetic, never retained as execution evidence. Just enough valid
// shape to ensure each negative fails the intended validation check.
function syntheticReportShape() {
  const positive_cases = [];
  for (const optimization of ['O0', 'O3']) for (const result_used of [true, false]) {
    positive_cases.push({ optimization, result_used, llvm_text_sha256: 'b'.repeat(64),
      llvm_text_bytes: 512, hsaco_sha256: 'c'.repeat(64), hsaco_bytes: 512,
      descriptor_sha256: 'd'.repeat(64), entry_file_offset: 128, entry_code_bytes: 64,
      static_instruction_count: 8, post_link_inspection_diagnostics: [...DIAGNOSTICS],
      region: [site(144, 'V_XOR_B32_e32_vi', '2247402a', ['VGPR32', 'VGPR34', 'VGPR35']),
        site(148, 'V_ADD_U32_e32_gfx9', '20494268', ['VGPR33', 'VGPR32', 'VGPR36'])],
      boundary_register_site_count: 3, boundary_v_mov_b32_count: 3,
      boundary_sites: [site(132, 'V_MOV_B32_e32_vi', '0002447e', ['VGPR34', 'SGPR0']),
        site(136, 'V_MOV_B32_e32_vi', '0102467e', ['VGPR35', 'SGPR1']),
        site(140, 'V_MOV_B32_e32_vi', '0202487e', ['VGPR36', 'SGPR2'])],
      boundary_sites_truncated: false });
  }
  return { schema: 'fe2o3-ordered-inline-unit-worker-prototype-v1',
    authority: 'unauthenticated-native-test-fixture', source_produced: false,
    production_exact_region_admission: false, protected_finalizer_admission: false,
    hardware_executed: false, physical_register_allocation_or_lifetime_proof: false,
    whole_kernel_order_or_byte_stability_claim: false, target: 'gfx942:xnack-',
    wave_width: 64, workgroup_size: 64, code_object_version: 6,
    llvm_build_claim: LLVM_BUILD_ID, worker_build_claim: CLAIM,
    implicit_exec_reads: 'required; not clobbered', boundary_operations: 'compiler-owned; reported outside the unit',
    positive_cases, closed_contract_negatives: 12, decoded_observation_negatives: 8,
    actual_payload_mutation_negatives: 2, actual_e64_encoding_rejected_by_matcher: true,
    module_only_export_rejected_by_worker: true };
}

function rejectMutations(mutators) {
  validateObservation(syntheticReportShape(), CLAIM); // Shape, not compilation.
  for (const mutate of mutators) {
    const report = syntheticReportShape();
    mutate(report);
    assert.throws(() => validateObservation(report, CLAIM));
  }
}

test('report controls reject changed scope, target, identities and missing cases', () => {
  rejectMutations([
    value => { value.source_produced = true; },
    value => { value.protected_finalizer_admission = true; },
    value => { value.hardware_executed = true; },
    value => { value.authority = 'production'; },
    value => { value.extra_authority = true; },
    value => { value.target = 'gfx950:xnack-'; },
    value => { value.wave_width = 32; },
    value => { value.llvm_build_claim = 'ambient'; },
    value => { value.worker_build_claim = `fe2o3-worker-v1-sha256-${'e'.repeat(64)}`; },
    value => { value.positive_cases.pop(); },
    value => { value.positive_cases[1] = value.positive_cases[0]; },
    value => { value.decoded_observation_negatives = 7; },
    value => { value.actual_e64_encoding_rejected_by_matcher = false; },
  ]);
});

test('report controls reject altered order, bytes, widths, registers and implicit state', () => {
  const first = value => value.positive_cases[0].region[0];
  rejectMutations([
    value => { value.positive_cases[0].region.reverse(); },
    value => { first(value).bytes_hex = '2347402a'; },
    value => { first(value).bytes_hex += '00000000'; },
    value => { first(value).opcode = 'V_XOR_B32_e64_vi'; },
    value => { first(value).register_operands[1] = 'VGPR35'; },
    value => { first(value).implicit_reads = []; },
    value => { first(value).implicit_writes = ['EXEC']; },
    value => { first(value).mc_flags = 1; },
    value => { first(value).mc_flags = 32; },
    value => { value.positive_cases[0].region[1].file_offset += 4; },
    value => { first(value).file_offset = 511; },
  ]);
});

test('report controls reject lossy sizes, forged diagnostics and boundary contradictions', () => {
  rejectMutations([
    value => { value.positive_cases[0].hsaco_bytes = Number.MAX_SAFE_INTEGER + 1; },
    value => { value.positive_cases[0].llvm_text_bytes = 0; },
    value => { value.positive_cases[0].descriptor_sha256 = '0'.repeat(64); },
    value => { value.positive_cases[0].descriptor_sha256 = 'A'.repeat(64); },
    value => { value.positive_cases[0].static_instruction_count = 513; },
    value => { value.positive_cases[0].entry_code_bytes = 1000; },
    value => { value.positive_cases[0].post_link_inspection_diagnostics[4] = 'all good'; },
    value => { value.positive_cases[0].boundary_sites_truncated = true; },
    value => { value.positive_cases[0].boundary_register_site_count = 4; },
    value => { value.positive_cases[0].boundary_v_mov_b32_count = 2; },
    value => { value.positive_cases[0].boundary_sites[0].file_offset = 144; },
    value => { value.positive_cases[0].boundary_sites[0].register_operands = ['VGPR1']; },
  ]);
});

test('raw report parser rejects oversized, malformed and invalid UTF8 output', () => {
  assert.throws(() => parseObservation(Buffer.alloc(65537, 32), CLAIM));
  assert.throws(() => parseObservation(Buffer.from([0xff]), CLAIM));
  assert.throws(() => parseObservation(Buffer.from('{}'), CLAIM));
  assert.throws(() => parseObservation(Buffer.from('{'), CLAIM));
});

test('CLI has explicit bounded paths and no duplicate, unknown or shell options', () => {
  for (const args of [[], ['--output', '/tmp/out'], ['--shell', '/bin/sh'], ['--output', 'relative'],
    ['--output', '/tmp/a\ninvalid'], ['--output', '/tmp/one', '--output', '/tmp/two']])
    assert.throws(() => parseArguments(args));
});

test('regular-file reader rejects size, final symlink and FIFO without blocking', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'ordered-unit-file-control-'));
  try {
    const file = path.join(directory, 'regular');
    fs.writeFileSync(file, 'test-only');
    assert.equal(readRegular(file, 9).toString(), 'test-only');
    assert.throws(() => readRegular(file, 8));
    const link = path.join(directory, 'link');
    fs.symlinkSync(file, link);
    assert.throws(() => readRegular(link, 100));
    const fifo = path.join(directory, 'fifo');
    const created = spawnSync('/usr/bin/mkfifo', [fifo]);
    assert.equal(created.status, 0);
    assert.throws(() => readRegular(fifo, 100));
  } finally { fs.rmSync(directory, { recursive: true }); }
});

function mockedChild(code, overrides = {}) {
  return runBoundedCommand({ executable: process.execPath, args: ['-e', code], cwd: os.tmpdir(),
    env: { PATH: '/usr/bin:/bin' }, timeoutMs: 3000, stdoutCap: 1024, stderrCap: 1024, ...overrides });
}

test('mocked nonzero and empty-success children never constitute a native observation', async () => {
  const failed = await mockedChild('process.exit(7)');
  assert.equal(failed.code, 7);
  const empty = await mockedChild('process.exit(0)');
  assert.equal(empty.code, 0);
  assert.throws(() => parseObservation(empty.stdout, CLAIM));
});

test('mocked child stdout and stderr caps stop capture at exact bounds', async () => {
  for (const stream of ['stdout', 'stderr']) {
    const result = await mockedChild(`process.${stream}.write('x'.repeat(100000));setTimeout(()=>{},10000)`);
    assert.equal(result.reason, `${stream}_cap`);
    assert.equal(result[stream].length, 1024);
  }
});

test('mocked descendant holding inherited pipes is killed by the process-group timeout', async () => {
  const result = await mockedChild(
    "require('node:child_process').spawn(process.execPath,['-e','setTimeout(()=>{},10000)'],{stdio:'inherit'});process.exit(0)",
    { timeoutMs: 150 });
  assert.equal(result.reason, 'timeout');
  assert.ok(result.elapsed_ms < 3000);
});

test('mocked nonexistent command returns a bounded spawn failure', async () => {
  const result = await mockedChild('', { executable: '/this-command-does-not-exist/ordered-unit-test' });
  assert.match(result.reason, /^spawn:/);
  assert.equal(result.stdout.length, 0);
});

function validateBuild(receipt) {
  return validateNativeBuildReceipt(receipt, { repo: receipt.environment.compiler_repo, output: receipt.environment.output });
}
test('native measured roster is immutable, complete and shared across builder and consumer', () => {
  const receipt = syntheticNativeBuildReceipt(), roster = validateBuild(receipt);
  assert.equal(WORKER_FILES.length, 14); assert.equal(roster.inputs.length, 22); assert.equal(roster.artifacts.length, 2);
  assert.ok(WORKER_FILES.includes('tests/OrderedInlineRegionSourceObservation.inc'));
  for (const value of [WORKER_FILES, roster, roster.inputs, roster.artifacts, ...roster.inputs, ...roster.artifacts]) assert.ok(Object.isFrozen(value));
  assert.throws(() => WORKER_FILES.push('unmeasured.cpp'));
  assert.throws(() => { roster.inputs[0].requested = '/rewritten'; });
  assert.deepEqual(roster.inputs.map(item => item.requested), receipt.inputs.map(item => item.requested));
});

test('native roster rejects omission of every required source, script, tool, config and zstd input', () => {
  const original = syntheticNativeBuildReceipt();
  for (let index = 0; index < original.inputs.length; index++) {
    const receipt = structuredClone(original); receipt.inputs.splice(index, 1);
    assert.throws(() => validateBuild(receipt), /native inputs roster: exact count required/);
  }
});

test('native roster rejects duplicate, extra, reordered, rewritten and unbounded input measurements', () => {
  for (const change of [
    receipt => { receipt.inputs[1] = receipt.inputs[0]; },
    receipt => { receipt.inputs.push(receipt.inputs[0]); },
    receipt => { receipt.inputs.reverse(); },
    receipt => { receipt.inputs[0].requested = '/different-checkout/CMakeLists.txt'; },
    receipt => { receipt.inputs[0].requested = `${receipt.inputs[0].requested}/../CMakeLists.txt`; },
    receipt => { receipt.inputs[0].resolved = 'relative'; },
    receipt => { receipt.inputs[0].requested = `/${'x'.repeat(4096)}`; },
    receipt => { receipt.inputs[0].bytes = 2 * 1024 * 1024 + 1; },
    receipt => { receipt.inputs[0].sha256 = '0'.repeat(64); },
    receipt => { receipt.inputs[0].extra = true; },
  ]) { const receipt = syntheticNativeBuildReceipt(); change(receipt); assert.throws(() => validateBuild(receipt)); }
});

test('native artifact roster requires both exact regular output paths once each', () => {
  for (const change of [
    receipt => { receipt.artifacts.pop(); }, receipt => { receipt.artifacts.push(receipt.artifacts[0]); },
    receipt => { receipt.artifacts[1] = receipt.artifacts[0]; }, receipt => { receipt.artifacts.reverse(); },
    receipt => { receipt.artifacts[1].requested = '/other/fe2o3-llvm-link-worker'; },
    receipt => { receipt.artifacts[0].resolved = '/other/test-binary'; },
  ]) { const receipt = syntheticNativeBuildReceipt(); change(receipt); assert.throws(() => validateBuild(receipt), /native artifacts/); }
});

test('native roster derives variable paths only from the fixed bounded configure/build stages', () => {
  for (const change of [
    receipt => { receipt.stages.pop(); }, receipt => { receipt.stages.reverse(); },
    receipt => { receipt.stages[2].args.push('-DUNMEASURED=ON'); },
    receipt => { receipt.stages[2].args[1] = '/different/source'; },
    receipt => { receipt.stages[2].args[3] = '/different/output'; },
    receipt => { receipt.stages[2].args[6] = '-DLLVM_DIR=relative'; },
    receipt => { receipt.stages[2].args[7] = '-DLLD_DIR=/other/lib/cmake/lld'; },
    receipt => { receipt.stages[2].args[8] = '-DFE2O3_PINNED_LLVM_VERSION=23'; },
    receipt => { receipt.stages[2].args[10] = '-DFE2O3_LLVM_BUILD_ID_FILE=/changed/id'; },
    receipt => { receipt.stages[2].args[11] = '-DFE2O3_GFX942_DEVICE_LIB_DIR=/enabled'; },
    receipt => { receipt.stages[2].args[13] = '-Dzstd_INCLUDE_DIR=/changed/include'; },
    receipt => { receipt.stages[2].args[14] = '-Dzstd_LIBRARY=/changed/libzstd.so'; },
    receipt => { receipt.stages[2].args[16] = '-DCMAKE_CXX_COMPILER=/changed/cxx'; },
    receipt => { receipt.stages[3].executable = '/different/cmake'; },
    receipt => { receipt.stages[3].args[6] = '16'; },
    receipt => { receipt.stages[4].args = ['--unexpected']; },
    receipt => { receipt.stages[2].reason = 'timeout'; },
    receipt => { receipt.stages[2].free_bytes_before = '9'.repeat(21); },
    receipt => { receipt.runtime_closure_attestation = 'verified'; },
    receipt => { receipt.observation.native_test_control_counts = [0, 0, 0]; },
  ]) { const receipt = syntheticNativeBuildReceipt(); change(receipt); assert.throws(() => validateBuild(receipt)); }
  const receipt = syntheticNativeBuildReceipt(), configure = receipt.stages[2];
  configure.args[16] = `-DCMAKE_CXX_COMPILER=${configure.executable}`;
  assert.throws(() => nativeBuildMeasurementRoster({ repo: receipt.environment.compiler_repo,
    output: receipt.environment.output, configure: { executable: configure.executable, args: configure.args } }), /duplicate configured paths/);
});

test('native file measurements recheck requested-to-resolved tool bindings and byte bounds', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'native-roster-measurement-'));
  try {
    const first = path.join(directory, 'first'), second = path.join(directory, 'second'), link = path.join(directory, 'tool');
    fs.writeFileSync(first, 'same'); fs.writeFileSync(second, 'same'); fs.symlinkSync(first, link);
    const measured = measureNativeBuildInput(link, 4); requireNativeBuildInputsUnchanged([measured]);
    assert.equal(measured.requested, link); assert.equal(measured.resolved, first);
    fs.unlinkSync(link); fs.symlinkSync(second, link);
    assert.throws(() => requireNativeBuildInputsUnchanged([measured]), /input changed/);
    assert.throws(() => measureNativeBuildInput(first, 3), /byte cap/);
  } finally { fs.rmSync(directory, { recursive: true }); }
});
