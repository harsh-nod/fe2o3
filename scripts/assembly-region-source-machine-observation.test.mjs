// Synthetic report-shape controls are not compiled-source observations.
// Optional native parser controls name an already built binary explicitly.
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { LLVM_BUILD_ID } from './assembly-region-worker-prototype.mjs';
import { measureInput, requireUnchanged, parseArguments, parseMachineObservation,
  validateMachineObservation, validateSourceLadder } from './assembly-region-source-machine-observation.mjs';

const CLAIM = `fe2o3-worker-v1-sha256-${'a'.repeat(64)}`;
const expected = { workerClaim: CLAIM, llvmSha256: 'b'.repeat(64), llvmBytes: 512, used: true };
function site(file_offset, opcode, bytes_hex, register_operands) {
  return { file_offset, opcode, bytes_hex, register_operands, mc_flags: 0, implicit_reads: ['EXEC'], implicit_writes: [] };
}
function reportShape() {
  const cases = ['O0', 'O3'].map(optimization => ({ optimization, result_used: true,
    llvm_text_sha256: expected.llvmSha256, llvm_text_bytes: 512, hsaco_sha256: 'c'.repeat(64), hsaco_bytes: 512,
    descriptor_sha256: 'd'.repeat(64), entry_file_offset: 128, entry_code_bytes: 64, static_instruction_count: 8,
    post_link_inspection_diagnostics: [
      'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
      'post_link.check=exports status=ok symbols=[actual_symbol,actual_symbol.kd]',
      'post_link.check=unresolved status=ok symbols=[]',
      'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-',
      'post_link.kernel name=actual_symbol symbol=actual_symbol.kd kernarg_size=280 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]',
    ],
    region: [site(144, 'V_XOR_B32_e32_vi', '2247402a', ['VGPR32', 'VGPR34', 'VGPR35']),
      site(148, 'V_ADD_U32_e32_gfx9', '20494268', ['VGPR33', 'VGPR32', 'VGPR36'])],
    boundary_register_site_count: 3, boundary_v_mov_b32_count: 3,
    boundary_sites: [site(132, 'V_MOV_B32_e32_vi', '0002447e', ['VGPR34', 'SGPR0']),
      site(136, 'V_MOV_B32_e32_vi', '0102467e', ['VGPR35', 'SGPR1']),
      site(140, 'V_MOV_B32_e32_vi', '0202487e', ['VGPR36', 'SGPR2'])], boundary_sites_truncated: false,
    descriptor_resources: { descriptor_file_offset: 64, descriptor_bytes: 64, descriptor_sha256: 'd'.repeat(64),
      compute_pgm_rsrc1: 4, compute_pgm_rsrc3: 9, vgpr_capacity: 40, architected_vgpr_boundary: 40,
      required_footprint_high_water: 37, interpretation: 'encoded-capacity-not-metadata-usage-or-lifetime' },
  }));
  return { schema: 'fe2o3-ordered-region-llvm-machine-observation-v1', authority: 'unauthenticated-test-transport',
    source_ancestry: 'not-established-by-llvm-file', production_exact_region_admission: false, protected_finalizer_admission: false,
    hardware_executed: false, runtime_closure_attestation: 'unavailable', physical_register_allocation_or_lifetime_proof: false,
    whole_kernel_order_or_byte_stability_claim: false, target: 'gfx942:xnack-', wave_width: 64, workgroup_size: 64,
    code_object_version: 6, kernel_symbol: 'actual_symbol', result_used: true, llvm_text_sha256: expected.llvmSha256,
    llvm_text_bytes: 512, llvm_build_claim: LLVM_BUILD_ID, worker_build_claim: CLAIM, cases };
}
function rejectReports(changes) {
  validateMachineObservation(reportShape(), expected);
  for (const change of changes) { const report = reportShape(); change(report); assert.throws(() => validateMachineObservation(report, expected)); }
}
test('synthetic report guards bind exact LLVM bytes, mode, target and two machine cases', () => {
  rejectReports([
    report => { report.llvm_text_sha256 = 'e'.repeat(64); },
    report => { report.llvm_text_bytes++; }, report => { report.result_used = false; },
    report => { report.worker_build_claim = `fe2o3-worker-v1-sha256-${'e'.repeat(64)}`; },
    report => { report.llvm_build_claim = 'ambient'; }, report => { report.target = 'gfx950:xnack-'; },
    report => { report.kernel_symbol = 'injected.*'; }, report => { report.wave_width = 32; },
    report => { report.cases = []; }, report => { report.cases[1] = report.cases[0]; },
    report => { report.cases[1].llvm_text_sha256 = 'e'.repeat(64); },
    report => { report.cases[1].result_used = false; }, report => { report.extra_authority = true; },
  ]);
});
test('synthetic report guards reject production, authentication, physical-lifetime and hardware claims', () => {
  rejectReports([
    report => { report.source_ancestry = 'authenticated'; }, report => { report.authority = 'production'; },
    report => { report.production_exact_region_admission = true; }, report => { report.protected_finalizer_admission = true; },
    report => { report.hardware_executed = true; }, report => { report.runtime_closure_attestation = 'verified'; },
    report => { report.physical_register_allocation_or_lifetime_proof = true; },
    report => { report.whole_kernel_order_or_byte_stability_claim = true; },
  ]);
});
test('synthetic final-byte controls reject changed opcode/order/register/implicit state and descriptor coverage', () => {
  rejectReports([
    report => { report.cases[0].region.reverse(); },
    report => { report.cases[0].region[0].bytes_hex = '2347402a'; },
    report => { report.cases[0].region[0].opcode = 'V_XOR_B32_e64_vi'; },
    report => { report.cases[0].region[0].register_operands[1] = 'VGPR35'; },
    report => { report.cases[0].region[0].implicit_reads = []; },
    report => { report.cases[0].region[0].implicit_writes = ['EXEC']; },
    report => { report.cases[0].region[0].mc_flags = 32; },
    report => { report.cases[0].region[1].file_offset++; },
    report => { report.cases[0].descriptor_resources.vgpr_capacity = 20; },
    report => { report.cases[0].descriptor_resources.architected_vgpr_boundary = 36; },
    report => { report.cases[0].descriptor_resources.compute_pgm_rsrc1 = 3; },
    report => { report.cases[0].descriptor_resources.descriptor_file_offset = 449; },
    report => { report.cases[0].descriptor_resources.descriptor_sha256 = 'e'.repeat(64); },
    report => { report.cases[0].descriptor_resources.interpretation = 'live-register-usage'; },
    report => { report.cases[0].post_link_inspection_diagnostics[4] = 'unrelated entry'; },
    report => { report.cases[0].boundary_sites[0].file_offset = 144; },
    report => { report.cases[0].boundary_register_site_count++; },
    report => { report.cases[0].hsaco_bytes = Number.MAX_SAFE_INTEGER + 1; },
  ]);
});
test('zero/empty success, invalid UTF8, malformed and oversized machine reports are rejected', () => {
  for (const bytes of [Buffer.alloc(0), Buffer.from([0xff]), Buffer.from('{}'), Buffer.from('{'), Buffer.alloc(65537, 32)])
    assert.throws(() => parseMachineObservation(bytes, expected));
  assert.equal(parseMachineObservation(Buffer.from(JSON.stringify(reportShape())), expected).cases.length, 2);
});

function ladderShape() {
  const features = ['ordered-region-v31', 'ordered-region-unused-v31', 'ordered-region-alias-v31', 'ordered-region-dynamic-v31',
    'ordered-region-divergent-v31', 'ordered-region-wrong-launch-v31'];
  const reasons = ['ordered region physical roles must be distinct v0..v63', 'ordered region physical role is not an actual MIR constant',
    'ordered region must precede every conditional source edge', 'ordered region requires an explicit 64x1x1 workgroup'];
  return { schema: 'fe2o3-test-source-ordered-region-ladder-v31', grants_artifact_or_launch_authority: false, hardware_observed: false,
    observations: features.map((feature, index) => ({ schema: 'fe2o3-test-source-ordered-region-observation-v31', feature,
      actual_rustc_callback: true, source_unchanged: true, proof_executed: false, final_production_admitted: false,
      grants_artifact_or_launch_authority: false, hardware_observed: false,
      invocation: { schema: 'fe2o3-test-source-ordered-region-invocation-v31', feature, args: ['explicitly-synthetic-test-only'],
        crate_binding: 'a'.repeat(64), cargo_observation: 'b'.repeat(64), source_sha256: 'c'.repeat(64), root_source_sha256: 'd'.repeat(64),
        manifest_sha256: 'e'.repeat(64), artifacts_sha256: 'f'.repeat(64), metadata_sha256: '1'.repeat(64) },
      observation: index < 2 ? { stage: 'actual_source_v31_exact_v16_cpu_and_llvm_observed', semantic_sha256: '1'.repeat(64),
        canonical_v16_identity: '2'.repeat(64), canonical_bytes_sha256: String(index + 3).repeat(64), canonical_v16_length: 100,
        llvm_sha256: String(index + 5).repeat(64), rustc_identity_inventory_sha256: '7'.repeat(64), rustc_preflight_plan_sha256: '8'.repeat(64),
        region_source_ids: ['1', '2', '3', '4'].map(value => value.repeat(64)), region_count: 1, cpu_cases: 6, lanes_per_case: 64,
        canaries_unchanged: true, unused_result_retained: index === 1 } : { stage: 'actual_source_profile_refused', diagnostic: reasons[index - 2] },
    })) };
}
test('source ladder requires six exact callback cases, independent variants and no elevated authority', () => {
  validateSourceLadder(ladderShape());
  for (const change of [
    value => { value.observations.pop(); }, value => { value.observations[1] = value.observations[0]; },
    value => { value.observations[0].actual_rustc_callback = false; }, value => { value.observations[0].source_unchanged = false; },
    value => { value.observations[0].proof_executed = true; }, value => { value.observations[0].hardware_observed = true; },
    value => { value.observations[0].observation.region_count = 0; }, value => { value.observations[0].observation.cpu_cases = 0; },
    value => { value.observations[0].observation.lanes_per_case = 32; }, value => { value.observations[0].observation.region_source_ids.pop(); },
    value => { value.observations[0].observation.canaries_unchanged = false; },
    value => { value.observations[0].observation.unused_result_retained = true; },
    value => { value.observations[1].observation.llvm_sha256 = value.observations[0].observation.llvm_sha256; },
    value => { value.observations[2].observation.diagnostic = 'unrelated unsupported opcode'; },
    value => { value.observations[2].observation.stage = 'compiler_failed'; },
  ]) { const value = ladderShape(); change(value); assert.throws(() => validateSourceLadder(value)); }
});
test('input measurement rejects changed same-size bytes, symlinks, deleted files and growth', () => {
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'source-machine-join-control-'));
  try {
    const file = path.join(directory, 'input'); fs.writeFileSync(file, 'aaa');
    const measured = measureInput(file, 3); requireUnchanged([measured]);
    fs.writeFileSync(file, 'bbb'); assert.throws(() => requireUnchanged([measured]));
    fs.writeFileSync(file, 'aaaa'); assert.throws(() => requireUnchanged([measured]));
    const link = path.join(directory, 'link'); fs.symlinkSync(file, link); assert.throws(() => measureInput(link));
    fs.unlinkSync(file); assert.throws(() => requireUnchanged([measured]));
  } finally { fs.rmSync(directory, { recursive: true }); }
});
test('CLI rejects relative, duplicate, unknown and incomplete paths', () => {
  for (const args of [[], ['--source-run', 'relative'], ['--shell', '/bin/sh'], ['--output'],
    ['--output', '/tmp/a', '--output', '/tmp/b'], ['--output', '/tmp/a\ninvalid']]) assert.throws(() => parseArguments(args));
});

const native = process.env.FE2O3_ORDERED_REGION_NATIVE_TEST_EXE;
test('optional actual native reader rejects links, FIFO, oversized IR, helpers and module assembly', { skip: !native }, () => {
  assert.ok(path.isAbsolute(native));
  const directory = fs.mkdtempSync(path.join(os.tmpdir(), 'source-machine-native-controls-'));
  const reject = (file, reason) => {
    const result = spawnSync(native, ['--observe-llvm-used', file], { timeout: 10000, maxBuffer: 65536, encoding: 'utf8' });
    assert.equal(result.error, undefined); assert.equal(result.signal, null, result.stderr); assert.equal(result.status, 1, result.stderr);
    assert.equal(result.stdout, ''); assert.match(result.stderr, reason);
  };
  try {
    const file = path.join(directory, 'input.ll'); fs.writeFileSync(file, 'not LLVM');
    reject(file, /not valid textual IR/);
    const link = path.join(directory, 'link.ll'); fs.symlinkSync(file, link); reject(link, /non-symlink LLVM input/);
    const fifo = path.join(directory, 'fifo.ll'); assert.equal(spawnSync('/usr/bin/mkfifo', [fifo]).status, 0); reject(fifo, /bounded regular file/);
    fs.writeFileSync(file, 'x'.repeat(65537)); reject(file, /bounded regular file/);
    fs.writeFileSync(file, 'target triple = "amdgcn-amd-amdhsa"\ndefine amdgpu_kernel void @kernel() { ret void }\ndefine void @helper() { ret void }\n');
    reject(file, /exactly one defined AMD kernel and no helpers/);
    fs.writeFileSync(file, 'target triple = "amdgcn-amd-amdhsa"\nmodule asm "s_nop 0"\ndefine amdgpu_kernel void @kernel() { ret void }\n');
    reject(file, /no module asm/);
    fs.writeFileSync(file, 'target triple = "amdgcn-amd-amdhsa"\ndeclare amdgpu_kernel void @kernel()\n');
    reject(file, /kernel symbol/);
    fs.writeFileSync(file, 'target triple = "amdgcn-amd-amdhsa"\ndefine amdgpu_kernel void @kernel() { ret void }\n');
    reject(file, /explicit data layout/);
  } finally { fs.rmSync(directory, { recursive: true }); }
});
