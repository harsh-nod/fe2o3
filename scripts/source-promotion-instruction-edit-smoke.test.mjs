// Pure synthetic acceptance/refusal controls; no compiler, filesystem or child execution.
import test from 'node:test';
import assert from 'node:assert/strict';
import { validateSourceExportLine } from './ordered-program-source-native.mjs';
import { INPUTS, descriptors, options, oracle, parseExport, parsePublication, replaceOnce, request, sha256,
  validateSeedRecord, verifyEmission, verifyInspector, verifySimulation, verifySourceEdit, verifyVariants,
} from './source-promotion-instruction-edit-smoke.mjs';

const H = '12'.repeat(32), J = '34'.repeat(32), K = '56'.repeat(32), L = '78'.repeat(32);
const copy = value => structuredClone(value);
function exported() {
  return { canonical_sha256: H, canonical_bytes: 64, retained_source_inventory: J,
    retained_source_preflight: K, semantic_identity: L };
}
function legacy(value = exported()) {
  return 'fe2o3 diagnostic extraction: actual Rust -> admitted semantic MIR V32 -> semantic SSA -> pre_ranked_diagnostic exact KIR V17; ' +
    'declared_target=gfx942:xnack-, declared_wave=64, 1 kernel(s), canonical_identity ' + value.canonical_sha256 + ', ' +
    value.canonical_bytes + ' byte(s), retained_source_inventory ' + value.retained_source_inventory +
    ', retained_source_preflight ' + value.retained_source_preflight +
    '; ranked_checks=false, functional_proof=false, exported_source_authentication=false, exported_compiler_authentication=false, ' +
    'protected_admission=false, artifact/load/launch/hardware_authority=false, source_variable_map=unavailable, ' +
    'physical_register_values=unavailable, instruction_microsteps=unavailable; raw bytes are diagnostic input, not production resume';
}
function semantic(value = exported()) {
  return 'fe2o3 diagnostic source identities: semantic_mir_v32 ' + value.semantic_identity +
    ', canonical_kir_v17 ' + value.canonical_sha256 + '; observation_only=true, exported_source_authentication=false';
}
function seedRecord() {
  return { case: 'positive', phase: 'baseline', source_directory: 'target/source-bitselect-candidate-roundtrip/prepared/positive',
    args: ['/toolchain/bin/rustc', '--crate-name', 'fe2o3_production_extraction_fixture',
      '/repo/target/source-bitselect-candidate-roundtrip/prepared/positive/original-loader.rs', '--edition=2024', '--crate-type=lib',
      '--target=amdgcn-amd-amdhsa', '--emit=metadata', '-Copt-level=3', '-Cpanic=abort',
      '-Cembed-bitcode=no', '-Cdebug-assertions=off', '-Coverflow-checks=on', '-Ctarget-cpu=gfx942',
      '-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32', '-Zalways-encode-mir', '-Zunstable-options',
      '--sysroot', '/toolchain', '--out-dir', '/prepared/analysis-output', '-L',
      'dependency=/prepared/dependencies/target/release/deps', '-L', 'dependency=/prepared/dependencies/release/deps',
      '--extern', 'fe2o3_device=/prepared/dependencies/target/release/deps/device.rmeta',
      '--extern', 'noprelude:core=/prepared/dependencies/target/release/deps/core.rmeta',
      '--cfg', 'feature="source-bitselect-feasibility"', '-Cmetadata=portable_metadata'],
    crate_binding: H, cargo_observation: J, source_sha256: K, loader_sha256: L,
    fixture_sha256: [H, J, K, L], artifacts_sha256: H, metadata_sha256: J };
}
function inspector(variant = 'default', identity = exported()) {
  return { kind: 'diagnostic_ordered_program_inspection_example', authority: 'observation_only',
    canonical: { wire_version: 17, sha256: identity.canonical_sha256, bytes: identity.canonical_bytes },
    kernel: 'choose_bits', function: 'choose_bits_impl', coordinate: { function_ordinal: 0, block_ordinal: 0, operation_ordinal: 0 },
    raw_block_id: 0, input_value_ids: [1, 2, 3], result_value_id: 4, declared_target: 'gfx942:xnack-',
    declared_wave_width: 64, profile: 'closed_u32_program_e32_v1',
    register_plan: { scratch: 4, output: 5, inputs: [0, 1, 2], vgpr_high_water: 6 },
    declared_program: { count: 3, descriptors: descriptors(variant) },
    declared_instruction_steps: [
      { instruction: 'v_xor_b32_e32', output: 4, inputs: [0, 1] },
      { instruction: 'v_and_b32_e32', output: 4, inputs: [4, 2] },
      { instruction: variant === 'default' ? 'v_xor_b32_e32' : 'v_or_b32_e32', output: 5, inputs: [1, 4] },
    ], declared_source_ids: { frontend_unit: H, function: J, contract: K, statement: L },
    memory_effect: 'NoMemory', ordered_region_effect: true, pure_or_movable: false,
    logical_observation_granularity: 'whole_program_before_after', source_authentication: false,
    source_map_available: false, physical_register_values_available: false, instruction_microsteps_available: false,
    register_lifetime_or_final_allocation_proof: false, proof_authority: false, artifact_authority: false,
    production_resume_authority: false, hardware_execution: false, cpu_preflight_passed: true,
    inspection_counts: { blocks: 5, operations: 8, ssa_definitions: 12, capability_entries: 9, name_bytes: 100 },
    inspection_max_canonical_bytes_after_admission: 65536, cpu_preflight_resident_limit_bytes: 64 * 1024 * 1024,
    output_buffer_bytes: 8192,
    accounting_scope: 'shared canonical admission, CPU resident accounting, and this borrowed structural/output bound are separate; not a combined allocator or RSS cap' };
}
function simulated(query, word, identity = exported()) {
  const elements = query.arguments[0].elements, bytes = Buffer.from(query.shared_buffers[0].bytes.slice(2), 'hex');
  const init = Buffer.alloc(Math.ceil(bytes.length / 8));
  for (let index = 0; index < elements; index++) bytes.writeUInt32LE(word, 4 + index * 4);
  for (let offset = 4; offset < 4 + elements * 4; offset++) init[Math.floor(offset / 8)] |= 2 ** (offset % 8);
  return { schema: 'fe2o3-simulation-result-v1', status: 'ok', authority: 'observation_only', simulated: true,
    hardware_observed: false, hardware_validation: false, performance_prediction: false,
    kir: { sha256: identity.canonical_sha256, canonical_bytes: identity.canonical_bytes },
    arguments: copy(query.arguments), counts: { arguments: 4, shared_buffers: 1,
      invocations_executed: query.grid[0], workgroups_visited: query.grid[0] / 64 },
    target_profile: { identity: 'amdgpu_64_little_endian_v1', index_bits: 64 },
    shared_buffers: [{ id: 1, buffer: { element: 'u32', access: 'read_write', alignment: 4,
      bytes: '0x' + bytes.toString('hex'), initialized: '0x' + init.toString('hex') } }] };
}
function emission(variant, kir, llvm) {
  return { kind: 'diagnostic_ordered_program_llvm_observation', authority: 'observation_only',
    canonical_wire_version: 17, canonical_identity: H, canonical_bytes: kir.length, input_file_sha256: sha256(kir),
    llvm_sha256: sha256(llvm), llvm_bytes: llvm.length, program_count: 3, descriptors: descriptors(variant),
    register_plan: [4, 5, 0, 1, 2], canonical_retained_storage_bytes: 1024, canonical_work_limit: 1 << 26,
    canonical_storage_limit: 64 * 1024 * 1024, max_input_bytes: 65536, max_published_llvm_bytes: 65536,
    emitter_text_limit_bytes: 16 * 1024 * 1024, canonical_and_emitter_accounting_are_separate: true,
    source_authentication: false, compiler_closure_attestation: false, proof_authority: false,
    protected_admission: false, final_artifact_authority: false, production_resume: false,
    physical_register_values: false, hardware_execution: false };
}
function sourcePair() {
  const original = Buffer.from('// λ before byte offsets\n    let selected = b ^ ((a ^ b) & mask);\n// unchanged tail\n');
  const macro = 'fe2o3_device::amdgpu_ordered_program! {\n    gfx942_xnack_off_wave64;\n    scratch(4); out(5);\n' +
    '    in(0) = a;\n    in(1) = b;\n    in(2) = mask;\n' +
    '    xor(scratch, input0, input1);\n    and(scratch, scratch, input2);\n    xor(out, input1, scratch);\n}';
  return { original, candidate: replaceOnce(original, 'b ^ ((a ^ b) & mask)', macro) };
}
function joinedVariants() {
  const first = { label: 'default', source_sha256: H, llvm_sha256: H, kir_file_sha256: J,
    exported: exported(), inspection: inspector() };
  const edited = { label: 'edited', source_sha256: J, llvm_sha256: J, kir_file_sha256: K,
    exported: { canonical_sha256: J, canonical_bytes: 64, retained_source_inventory: J,
      retained_source_preflight: L, semantic_identity: H } };
  edited.inspection = inspector('edited', edited.exported);
  edited.inspection.declared_source_ids.statement = H;
  return [first, edited, { ...copy(edited), label: 'repeat' }];
}

test('byte-exact source edit preserves multibyte prefix and all surrounding bytes', () => {
  const { original, candidate } = sourcePair(), before = Buffer.from(candidate);
  const edited = verifySourceEdit(original, candidate);
  assert.ok(edited.includes(Buffer.from('    or(out, input1, scratch);\n')));
  assert.deepEqual(candidate, before);
  assert.deepEqual(replaceOnce(edited, '    or(out, input1, scratch);\n', '    xor(out, input1, scratch);\n'), candidate);
  assert.throws(() => verifySourceEdit(original, Buffer.concat([candidate, Buffer.from('// extra\n')])));
});
test('exact edit refuses absent, duplicate, oversized and invalid UTF8 input', () => {
  for (const bytes of [Buffer.from('none'), Buffer.from('x x'), Buffer.alloc(65537, 120), Buffer.from([0xff, 120])]) {
    assert.throws(() => replaceOnce(bytes, 'x', 'y'));
  }
});
test('normal public publication must be one exact current-byte observation', () => {
  const { original, candidate } = sourcePair();
  const line = 'FE2O3_HEADLESS_NORMAL_PUBLISHED ' + sha256(original) + ' ' + sha256(candidate) + ' ' + candidate.length + '\n';
  assert.equal(parsePublication(Buffer.from(line), original, candidate).candidate_bytes, candidate.length);
  for (const changed of [line + line, line.replace(sha256(original), H), line.replace(String(candidate.length), '0'), '']) {
    assert.throws(() => parsePublication(Buffer.from(changed), original, candidate));
  }
});
test('root-prepared record preserves exact existing argv, dependency and environment contracts', () => {
  const record = seedRecord(), before = copy(record);
  assert.equal(validateSeedRecord(record, '/repo', '/prepared').device, '/prepared/dependencies/target/release/deps/device.rmeta');
  assert.deepEqual(record, before);
});
test('seed record refuses path escape, old dependency root, argument mutation and extra fields', () => {
  const mutations = [v => v.source_directory = '../escape', v => v.phase = 'fresh',
    v => v.args[26] = 'fe2o3_device=/old/device.rmeta', v => v.args[12] = '-Coverflow-checks=off',
    v => v.args.push('--cfg=unreviewed'), v => v.crate_binding = '0'.repeat(64), v => v.extra = true];
  for (const mutate of mutations) { const value = seedRecord(); mutate(value); assert.throws(() => validateSeedRecord(value, '/repo', '/prepared')); }
});
test('seed source custody refuses coherent old or unrelated in-repo directory substitution', () => {
  for (const directory of ['target/source-bitselect-candidate-roundtrip/old-preparation/positive',
    'target/source-bitselect-candidate-roundtrip/prepared/other', '.instruction-test/positive']) {
    const record = seedRecord(); record.source_directory = directory;
    record.args[3] = '/repo/' + directory + '/original-loader.rs';
    assert.throws(() => validateSeedRecord(record, '/repo', '/prepared'));
  }
  for (const prepared of ['/bad name', '/' + 'a'.repeat(97), '/']) {
    assert.throws(() => validateSeedRecord(seedRecord(), '/repo', prepared),
      /exact existing preparation basename contract/u);
  }
});
test('semantic-unavailable mode does not relabel source preflight as semantic identity', () => {
  const value = parseExport(Buffer.from(legacy() + '\n'), 'unavailable');
  assert.equal(value.semantic_identity, null); assert.equal(value.retained_source_preflight, K);
  assert.throws(() => parseExport(Buffer.from(legacy() + '\n'), 'required'));
});
test('optional live semantic diagnostic must join the same exact canonical subject', () => {
  const bytes = Buffer.from(legacy() + '\n' + semantic() + '\n');
  assert.deepEqual(parseExport(bytes, 'required'), exported());
  assert.throws(() => parseExport(bytes, 'unavailable'));
  assert.throws(() => parseExport(Buffer.from(legacy() + '\n' + semantic().replace('canonical_kir_v17 ' + H, 'canonical_kir_v17 ' + J)), 'required'));
});
test('additive semantic line leaves the existing exact legacy consumer unchanged', () => {
  const bytes = Buffer.from(legacy() + '\n' + semantic() + '\n');
  validateSourceExportLine(bytes, { canonical_sha256: H, canonical_bytes: 64,
    observed_retained_source_inventory: J, observed_retained_source_preflight: K });
  assert.throws(() => validateSourceExportLine(Buffer.from(legacy() + '\n' + legacy() + '\n' + semantic()),
    { canonical_sha256: H, canonical_bytes: 64,
      observed_retained_source_inventory: J, observed_retained_source_preflight: K }));
});
test('export observation refuses duplicates, authority drift and malformed digest', () => {
  for (const text of [legacy() + '\n' + legacy(), legacy().replace('ranked_checks=false', 'ranked_checks=true'),
    legacy().replace(H, '0'.repeat(64)), legacy().replace('64 byte(s)', '65537 byte(s)')]) {
    assert.throws(() => parseExport(Buffer.from(text), 'unavailable'));
  }
  assert.throws(() => parseExport(Buffer.from(legacy() + '\n' + semantic() + '\n' + semantic()), 'required'));
});
test('typed inspection distinguishes genuine opcode edit and retains exact low register plan', () => {
  verifyInspector(inspector(), exported(), 'default'); verifyInspector(inspector('edited'), exported(), 'edited');
  assert.throws(() => verifyInspector(inspector(), exported(), 'edited'));
  for (const mutate of [v => v.register_plan.scratch = 32, v => v.declared_source_ids.statement = '0'.repeat(64),
    v => v.hardware_execution = true, v => v.unknown = true, v => v.declared_program.descriptors[15] = 1]) {
    const value = inspector(); mutate(value); assert.throws(() => verifyInspector(value, exported(), 'default'));
  }
});
test('independent Boolean oracles distinguish the real instruction change', () => {
  assert.equal(oracle('default', INPUTS[0]), 0); assert.equal(oracle('edited', INPUTS[0]), 0xffffffff);
  assert.equal(oracle('default', [0xffffffff, 0, 0xffffffff]), 0xffffffff);
  assert.equal(oracle('edited', [0, 0xffffffff, 0]), 0xffffffff);
  assert.throws(() => oracle('unknown', INPUTS[0]));
  assert.throws(() => oracle('default', [-1, 0, 0]));
});
test('whole-buffer checks cover zero, one and 65 outputs with both uninitialized canaries', () => {
  for (const length of [0, 1, 65]) for (const [variant, word] of [['default', 0], ['edited', 0xffffffff]]) {
    const query = request(INPUTS[0], length), result = simulated(query, word), before = copy(result);
    const checked = verifySimulation(result, query, exported(), variant, INPUTS[0], length);
    assert.equal(checked.checked_output_words, length); assert.equal(checked.checked_guard_bytes, 8);
    assert.deepEqual(result, before);
  }
});
test('simulation rejects wrong computation, stale KIR, mutated guard, initialization and launch', () => {
  const query = request(INPUTS[0], 1);
  assert.throws(() => verifySimulation(simulated(query, 0), query, exported(), 'edited', INPUTS[0], 1));
  const mutations = [v => v.kir.sha256 = J, v => v.shared_buffers[0].buffer.bytes = '0x00' + v.shared_buffers[0].buffer.bytes.slice(4),
    v => v.shared_buffers[0].buffer.initialized = '0xffff', v => v.counts.invocations_executed = 1,
    v => v.hardware_observed = true, v => v.arguments[0].elements = 0];
  for (const mutate of mutations) {
    const value = simulated(query, 0xffffffff); mutate(value);
    assert.throws(() => verifySimulation(value, query, exported(), 'edited', INPUTS[0], 1));
  }
});
test('LLVM observation keeps raw-file hash distinct from canonical identity', () => {
  const kir = Buffer.alloc(64, 3), llvm = Buffer.from('; synthetic only\n');
  assert.notEqual(sha256(kir), H);
  verifyEmission(emission('edited', kir, llvm), exported(), 'edited', kir, llvm);
  for (const mutate of [v => v.canonical_identity = sha256(kir), v => v.input_file_sha256 = H,
    v => v.llvm_sha256 = J, v => v.descriptors[2] = 413, v => v.protected_admission = true,
    v => v.register_plan[0] = 32, v => v.extra = true]) {
    const value = emission('edited', kir, llvm); mutate(value);
    assert.throws(() => verifyEmission(value, exported(), 'edited', kir, llvm));
  }
});
test('baseline/edit/repeat identity join is exact and rejects stale or changed replay', () => {
  verifyVariants(joinedVariants(), 'required');
  for (const mutate of [v => v[1].exported.semantic_identity = v[0].exported.semantic_identity,
    v => v[2].exported.canonical_sha256 = H, v => v[2].source_sha256 = H,
    v => v[1].inspection.declared_source_ids.statement = L]) {
    const value = joinedVariants(); mutate(value); assert.throws(() => verifyVariants(value, 'required'));
  }
  const unavailable = joinedVariants(); unavailable.forEach(value => value.exported.semantic_identity = null);
  verifyVariants(unavailable, 'unavailable'); assert.throws(() => verifyVariants(unavailable, 'required'));
  assert.throws(() => verifyVariants(unavailable, 'auto'));
});
test('opcode-only edit refuses any edited or repeat root/contract inventory drift', () => {
  const unchanged = joinedVariants();
  assert.deepEqual(unchanged.map(value => value.exported.retained_source_inventory), [J, J, J]);
  verifyVariants(unchanged, 'required');
  for (const changed of [[1], [2], [1, 2]]) {
    const value = joinedVariants();
    changed.forEach(index => value[index].exported.retained_source_inventory = K);
    assert.throws(() => verifyVariants(value, 'required'), /root\/contract inventory/u);
  }
});
test('stable inventory never permits stale or changed-repeat body-sensitive identities', () => {
  for (const field of ['retained_source_preflight', 'semantic_identity']) {
    for (const changed of ['stale-edited', 'changed-repeat']) {
      const value = joinedVariants();
      if (changed === 'stale-edited') {
        value[1].exported[field] = value[0].exported[field];
        value[2].exported[field] = value[0].exported[field];
      } else value[2].exported[field] = value[0].exported[field];
      assert.throws(() => verifyVariants(value, 'required'));
    }
  }
});
test('closed CLI requires explicit semantic availability and rejects duplicate or missing options', () => {
  const args = ['--repo', '/repo', '--prepared-root', '/prepared', '--seed-record', '/prepared/positive/headless-normal.invocation.json',
    '--consumer', '/bin/promote_once', '--bin-dir', '/bin', '--emitter', '/bin/emitter', '--output', '/new/run',
    '--semantic', 'unavailable'];
  assert.equal(options(args).semantic, 'unavailable');
  assert.throws(() => options(args.slice(0, -2)));
  assert.throws(() => options([...args.slice(0, -2), '--repo', '/other']));
  assert.throws(() => options([...args.slice(0, -1), 'auto']));
  for (const record of ['/old/positive/headless-normal.invocation.json', '/prepared/record.json',
    '/prepared/positive/other.invocation.json']) {
    const changed = [...args]; changed[5] = record; assert.throws(() => options(changed));
  }
});
