// Pure synthetic refusal controls. Inert payload/LLVM buffers below are NOT
// executable IR/ELF, source captures, native observations or qualification results.
// No subprocess, filesystem mutation, compiler, worker or GPU is invoked.
import assert from 'node:assert/strict';
import test from 'node:test';
import path from 'node:path';
import { LIMITS, LLVM_BUILD_ID, NATIVE_FALSE, SHAPE_CONTROLS, MUTATION_CONTROLS,
  expectedProgram, validateNative, validateOrigin, nativeArguments, healthy, options } from '../ordered-repeat-native-observation.mjs';
import { expectedPin, externalIdentity, sha256 } from '../ordered-repeat-llvm-observation.mjs';
import { parseJson } from '../ordered-repeat-source-smoke.mjs';

const clone = value => structuredClone(value);
const H = value => sha256(Buffer.from(value));
const CLAIM = 'fe2o3-worker-v1-sha256-' + H('synthetic worker claim');
function fixture(count = 2) {
  const llvm = Buffer.from('synthetic identity bytes; not executable LLVM\n');
  const expected = { repetitions: count, llvm_sha256: sha256(llvm), llvm_bytes: llvm.length,
    payload_directory: '/synthetic-only/native-' + count, llvm_build_id: LLVM_BUILD_ID, worker_build_id: CLAIM };
  const payloads = new Map();
  const cases = ['O0', 'O3'].map(optimization => {
    const bytes = Buffer.alloc(4096, 0), descriptorOffset = 128, start = 512;
    bytes.writeUInt32LE(4, descriptorOffset + 48); bytes.writeUInt32LE(9, descriptorOffset + 44);
    const steps = [{ opcode: 'V_MOV_B32_e32_vi', bytes_hex: '2203427e', register_operands: ['VGPR33', 'VGPR34'] },
      ...Array.from({ length: count }, () => ({ opcode: 'V_ADD_U32_e32_gfx9', bytes_hex: '21474268',
        register_operands: ['VGPR33', 'VGPR33', 'VGPR35'] }))];
    const program = steps.map((step, at) => {
      Buffer.from(step.bytes_hex, 'hex').copy(bytes, start + at * 4);
      return { ...step, file_offset: start + at * 4, mc_flags: 16, implicit_reads: ['EXEC'], implicit_writes: [] };
    });
    const retained_payload = { path: path.join(expected.payload_directory, optimization + '.hsaco'),
      bytes: bytes.length, sha256: sha256(bytes), matches_linked_worker_payload: true,
      create_new_only: true, production_artifact_authority: false };
    payloads.set(retained_payload.path, bytes);
    return { retained_payload, mutation_controls: clone(MUTATION_CONTROLS), machine_observation: {
      optimization, llvm_sha256: expected.llvm_sha256, llvm_bytes: expected.llvm_bytes,
      hsaco_sha256: sha256(bytes), hsaco_bytes: bytes.length, entry_file_offset: 512, entry_code_bytes: 256,
      static_instruction_count: 64, program, descriptor: { file_offset: descriptorOffset, bytes: 64,
        sha256: sha256(bytes.subarray(descriptorOffset, descriptorOffset + 64)),
        compute_pgm_rsrc1: 4, compute_pgm_rsrc3: 9, vgpr_capacity: 40, architected_vgpr_boundary: 40,
        required_footprint_high_water: 37, interpretation: 'encoded-capacity-not-metadata-usage-or-lifetime' },
      post_link_checks: [
        'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
        'post_link.check=exports status=ok symbols=[ordered_repeat_u32,ordered_repeat_u32.kd]',
        'post_link.check=unresolved status=ok symbols=[]',
        'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-',
        'post_link.kernel name=ordered_repeat_u32 symbol=ordered_repeat_u32.kd kernarg_size=32 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]'],
      derivation_identity: H('synthetic derivation'), boundary_value_or_lifetime_proof: false,
      repetitions: count, unique_sequence_matches: 1,
    } };
  });
  const report = { report_kind: 'task-ordered-repeat-native-observation-v1', authority: 'unauthenticated-test-transport',
    repetitions: count, program_count: count + 1, kernel_symbol: 'ordered_repeat_u32',
    register_plan: [32, 33, 34, 35, 36], constraints: '=&{v33},{v34},{v35},{v36},~{v32}', required_binding_extent: 37,
    result_use: 'sole-direct-nonvolatile-nonatomic-global-store', llvm_sha256: expected.llvm_sha256,
    llvm_bytes: expected.llvm_bytes, expected_input_identity_matched: true, retained_file_identity_and_bytes_rechecked: true,
    llvm_build_claim: LLVM_BUILD_ID, worker_build_claim: CLAIM, target: 'gfx942:xnack-', wave_width: 64,
    workgroup_size: 64, code_object_version: 6, shape_controls: clone(SHAPE_CONTROLS), cases,
    source_ancestry: 'not-established-by-llvm-file', synthetic_worker_request_identity_fields: true,
    runtime_closure_attestation: 'unavailable', ...Object.fromEntries(NATIVE_FALSE.map(key => [key, false])) };
  const read = (file, cap) => { const bytes = payloads.get(file); assert.ok(bytes); assert.ok(bytes.length <= cap); return bytes; };
  return { report, expected, llvm, payloads, read };
}
const validate = f => validateNative(f.report, f.expected, f.llvm, f.read);
function refuse(mutate, count = 2) {
  const f = fixture(count); validate(f); mutate(f); assert.throws(() => validate(f));
}
function repin(f, index = 0) {
  const wrapper = f.report.cases[index], bytes = f.payloads.get(wrapper.retained_payload.path);
  wrapper.retained_payload.bytes = bytes.length; wrapper.retained_payload.sha256 = sha256(bytes);
  wrapper.machine_observation.hsaco_bytes = bytes.length; wrapper.machine_observation.hsaco_sha256 = sha256(bytes);
}
test('pure: all three finite report shapes have two ordered cases and all exact synthetic byte-offset joins', () => {
  for (const count of [1, 2, 15]) {
    const f = fixture(count), result = validate(f);
    assert.equal(result.length, 2); assert.deepEqual(result.map(x => x.optimization), ['O0', 'O3']);
    assert.equal(result[0].instruction_offsets.length, count + 1);
    assert.equal(result[0].descriptor.required_footprint_high_water, 37);
    assert.ok(Buffer.byteLength(JSON.stringify(f.report)) < LIMITS.stream_bytes);
  }
});
test('pure: expected MOV/ADD words independently encode GFX9 destination and explicit operands', () => {
  for (const count of [1, 2, 15]) {
    const steps = expectedProgram(count); assert.equal(steps.length, count + 1);
    for (const [index, step] of steps.entries()) {
      const base = index === 0 ? 0x7e000200 : 0x68000000;
      const word = base + 33 * 2 ** 17 + (index === 0 ? 0 : 35 * 2 ** 9) + 256 + (index === 0 ? 34 : 33);
      assert.equal(Buffer.from(step.bytes_hex, 'hex').readUInt32LE(), word);
    }
  }
});
test('pure: unsupported counts and N minus/plus one report counts refuse', () => {
  for (const count of [0, 3, 14, 16, -1, 1.5, '2', Number.MAX_SAFE_INTEGER]) assert.throws(() => expectedProgram(count));
  for (const delta of [-1, 1]) refuse(f => { f.report.program_count += delta; });
  refuse(f => { f.report.repetitions = 1; });
  refuse(f => { f.report.cases[0].machine_observation.repetitions = 1; });
});
test('pure: missing/extra decoded ADD and duplicate cases refuse', () => {
  refuse(f => { f.report.cases[0].machine_observation.program.pop(); });
  refuse(f => { const p = f.report.cases[0].machine_observation.program; p.push(clone(p.at(-1))); });
  refuse(f => { f.report.cases[1] = clone(f.report.cases[0]); });
  refuse(f => { f.report.cases.pop(); });
});
test('pure: unique decoded sequence zero or duplicate, gapped/reordered offsets refuse', () => {
  for (const n of [0, 2]) refuse(f => { f.report.cases[0].machine_observation.unique_sequence_matches = n; });
  refuse(f => { f.report.cases[0].machine_observation.program[1].file_offset += 4; });
  refuse(f => { f.report.cases[0].machine_observation.program.reverse(); });
});
test('pure: e64, opposite opcode, wrong physical roles and hidden implicit writes refuse independently', () => {
  for (const mutate of [
    s => { s.opcode = 'V_ADD_U32_e64_gfx9'; },
    s => { s.opcode = 'V_SUB_U32_e32_gfx9'; },
    s => { s.register_operands[0] = 'VGPR32'; },
    s => { s.register_operands[1] = 'VGPR34'; },
    s => { s.register_operands[2] = 'VGPR36'; },
    s => { s.implicit_reads = []; },
    s => { s.implicit_writes = ['VCC']; },
    s => { s.mc_flags |= 1; },
  ]) refuse(f => mutate(f.report.cases[0].machine_observation.program[1]));
});
test('pure: coherent raw last-ADD-to-SUB mutation still refuses the closed ADD expectation', () => {
  refuse(f => {
    const w = f.report.cases[0], s = w.machine_observation.program.at(-1), bytes = f.payloads.get(w.retained_payload.path);
    s.opcode = 'V_SUB_U32_e32_gfx9'; s.bytes_hex = '2147426a';
    Buffer.from(s.bytes_hex, 'hex').copy(bytes, s.file_offset); repin(f);
  });
});
test('pure: exact scratch clobber, unused declared input and binding extent cannot disappear', () => {
  refuse(f => { f.report.constraints = '=&{v33},{v34},{v35},{v36}'; });
  refuse(f => { f.report.register_plan.pop(); });
  refuse(f => { f.report.required_binding_extent = 36; });
  refuse(f => { f.report.result_use = 'unused'; });
});
test('pure: stale LLVM hash/size at top, case and selected external identity refuse', () => {
  refuse(f => { f.report.llvm_sha256 = H('stale'); });
  refuse(f => { f.report.cases[0].machine_observation.llvm_sha256 = H('stale'); });
  refuse(f => { f.report.llvm_bytes++; });
  refuse(f => { f.expected.llvm_sha256 = H('preflight is not raw LLVM'); });
  refuse(f => { f.llvm[0] ^= 1; });
});
test('pure: swapped payloads, changed complete bytes and truncated payloads refuse', () => {
  refuse(f => { f.report.cases[0].retained_payload.path = f.report.cases[1].retained_payload.path; });
  refuse(f => { f.payloads.get(f.report.cases[0].retained_payload.path)[4095] ^= 1; });
  refuse(f => { const p = f.report.cases[0].retained_payload.path; f.payloads.set(p, f.payloads.get(p).subarray(0, 520)); });
});
test('pure: repinned instruction bytes cannot be replaced by report-only decoded facts', () => {
  refuse(f => { const w = f.report.cases[0]; f.payloads.get(w.retained_payload.path)[w.machine_observation.program[0].file_offset] ^= 1; repin(f); });
});
test('pure: entry/descriptor overlap, bounds and whole64-byte descriptor identity refuse', () => {
  refuse(f => { f.report.cases[0].machine_observation.entry_file_offset = 4095; });
  refuse(f => { f.report.cases[0].machine_observation.entry_code_bytes = 4; });
  refuse(f => { f.report.cases[0].machine_observation.descriptor.file_offset = 512; });
  refuse(f => { f.report.cases[0].machine_observation.descriptor.bytes = 63; });
  refuse(f => { f.report.cases[0].machine_observation.descriptor.sha256 = H('stale descriptor'); });
});
test('pure: coherently repinned undersized descriptor cannot pass capacity coverage', () => {
  refuse(f => {
    const w = f.report.cases[0], d = w.machine_observation.descriptor, bytes = f.payloads.get(w.retained_payload.path);
    bytes.writeUInt32LE(0, d.file_offset + 48); bytes.writeUInt32LE(0, d.file_offset + 44);
    Object.assign(d, { compute_pgm_rsrc1: 0, compute_pgm_rsrc3: 0, vgpr_capacity: 8, architected_vgpr_boundary: 4,
      sha256: sha256(bytes.subarray(d.file_offset, d.file_offset + 64)) }); repin(f);
  });
});
test('pure: decoded resource numbers must match exact descriptor words and formulas', () => {
  for (const field of ['compute_pgm_rsrc1', 'compute_pgm_rsrc3', 'vgpr_capacity', 'architected_vgpr_boundary', 'required_footprint_high_water'])
    refuse(f => { f.report.cases[0].machine_observation.descriptor[field]++; });
});
test('pure: target, wave, kernel/export, ABI launch and build claims stay closed', () => {
  refuse(f => { f.report.target = 'gfx950:xnack-'; });
  refuse(f => { f.report.wave_width = 32; });
  refuse(f => { f.report.kernel_symbol = 'choose_bits'; });
  refuse(f => { f.report.worker_build_claim = 'fe2o3-worker-v1-sha256-' + H('other'); });
  refuse(f => { f.report.cases[0].machine_observation.post_link_checks[4] = f.report.cases[0].machine_observation.post_link_checks[4].replace('kernarg_align=8', 'kernarg_align=3'); });
  refuse(f => { f.report.cases[0].machine_observation.post_link_checks[4] = f.report.cases[0].machine_observation.post_link_checks[4].replace('wavefront_size=64', 'wavefront_size=32'); });
});
test('pure: fabricated authority and unknown report/site/descriptor fields refuse', () => {
  for (const key of NATIVE_FALSE) refuse(f => { f.report[key] = true; });
  refuse(f => { f.report.authority = 'production'; });
  refuse(f => { f.report.new_proof = true; });
  refuse(f => { f.report.cases[0].machine_observation.program[0].trusted = true; });
  refuse(f => { f.report.cases[0].machine_observation.descriptor.lifetime = 'proven'; });
  refuse(f => { f.report.cases[0].retained_payload.production_artifact_authority = true; });
});
test('pure: native controls cannot be omitted, changed or relabeled as a hardware run', () => {
  refuse(f => { f.report.shape_controls.typed_shape_negatives--; });
  refuse(f => { f.report.cases[0].mutation_controls.decoded_field_refusals--; });
  refuse(f => { f.report.shape_controls.worker_or_target_machine_invoked = true; });
  refuse(f => { f.report.cases[0].mutation_controls.mutated_payload_executed_on_hardware = true; });
});
test('pure: source-to-LLVM identity join never reconstructs semantic/source IDs from raw LLVM', () => {
  const original = { label: 'one', repetitions: 1, source_path: '/capture/one-source/src/lib.rs',
    source_sha256: H('source'), kir_path: '/capture/one.kir', kir_file_sha256: H('raw kir'),
    exported: { semantic_identity: H('semantic'), retained_source_preflight: H('preflight'),
      retained_source_inventory: H('root census'), canonical_sha256: H('canonical kir') } };
  const variant = { ...original, semantic_identity: original.exported.semantic_identity,
    retained_source_preflight: original.exported.retained_source_preflight,
    retained_source_inventory: original.exported.retained_source_inventory, canonical_identity: original.exported.canonical_sha256 };
  delete variant.exported; validateOrigin(variant, original);
  for (const key of ['source_path', 'source_sha256', 'kir_path', 'kir_file_sha256', 'semantic_identity',
    'retained_source_preflight', 'retained_source_inventory', 'canonical_identity']) {
    const changed = clone(variant); changed[key] += '-stale'; assert.throws(() => validateOrigin(changed, original));
  }
});
test('pure: imported full file identity refuses every retained-file metadata/hash mutation', () => {
  const pin = { path: '/synthetic/input', bytes: 4, sha256: H('data'), device: '1', inode: '2',
    mode: '33152', nlink: '1', mtime_ns: '3', ctime_ns: '4' };
  expectedPin(pin, clone(pin));
  for (const key of ['bytes', 'sha256', 'device', 'inode', 'mode', 'nlink', 'mtime_ns', 'ctime_ns']) {
    const changed = clone(pin); changed[key] = key === 'bytes' ? 5 : key === 'sha256' ? H('other') : '99';
    assert.throws(() => expectedPin(changed, pin));
  }
  externalIdentity(Buffer.from('data'), 4, pin.sha256);
  assert.throws(() => externalIdentity(Buffer.from('dat'), 4, pin.sha256));
});
test('pure: exact count/LLVM/native argument order never substitutes a preflight hash', () => {
  const v = { repetitions: 15, llvm_path: '/capture/fifteen.ll', llvm_sha256: H('llvm'), llvm_bytes: 2393 };
  assert.deepEqual(nativeArguments(v, '/output/fifteen-payloads'), ['15', '/capture/fifteen.ll', H('llvm'), '2393', '/output/fifteen-payloads']);
  assert.throws(() => nativeArguments({ ...v, repetitions: 16 }, '/output/fifteen-payloads'));
});
test('pure: command timeout, signal, stderr, truncation and nonzero exit refuse', () => {
  const good = { code: 0, signal: null, reason: null, elapsed_ms: 10, stdout: Buffer.from('{}'), stderr: Buffer.alloc(0) };
  healthy(good);
  for (const change of [{ code: 1 }, { signal: 'SIGKILL' }, { reason: 'timeout' }, { reason: 'stdout_cap' },
    { stderr: Buffer.from('warning') }, { stdout: Buffer.alloc(LIMITS.stream_bytes + 1) }])
    assert.throws(() => healthy({ ...good, ...change }));
});
function args() {
  const value = { repo: '/repo', 'source-receipt': '/capture/source/receipt.json',
    'llvm-receipt': '/capture/llvm/receipt.json', observer: '/tools/observer', output: '/new-output',
    'source-receipt-bytes': '100', 'llvm-receipt-bytes': '100', 'observer-bytes': '100',
    'source-receipt-sha256': H('source receipt'), 'llvm-receipt-sha256': H('llvm receipt'),
    'observer-sha256': H('observer'), 'llvm-build-id': LLVM_BUILD_ID, 'worker-build-id': CLAIM };
  return Object.entries(value).flatMap(([key, item]) => ['--' + key, item]);
}
test('pure: CLI rejects missing/duplicate/unknown args and output aliasing retained roots', () => {
  options(args());
  assert.throws(() => options(args().slice(2)));
  const duplicate = args(); duplicate[2] = duplicate[0]; assert.throws(() => options(duplicate));
  const unknown = args(); unknown[0] = '--unknown'; assert.throws(() => options(unknown));
  for (const output of ['/repo', '/repo/new', '/capture', '/capture/source/new', '/tools/observer']) {
    const a = args(); a[a.indexOf('--output') + 1] = output; assert.throws(() => options(a));
  }
});
test('pure: duplicate-key and invalid UTF-8 report parsing is rejected by unchanged bounded parser', () => {
  assert.throws(() => parseJson(Buffer.from('{"x":1,"x":2}')));
  assert.throws(() => parseJson(Buffer.from([0xc3, 0x28])));
  assert.throws(() => parseJson(Buffer.alloc(1024 * 1024 + 1, 32)));
});
