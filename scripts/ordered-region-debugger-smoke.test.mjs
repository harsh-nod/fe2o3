// Pure option/oracle/inspection-validator controls. No compiler, inspector or
// debugger is executed, and no backend response or passing evidence is emitted.
import assert from 'node:assert/strict';
import test from 'node:test';
import { deriveRequestExpectation, parseSmokeArguments, validateInspection, validateSmokeOptions }
  from './ordered-region-debugger-smoke.mjs';

function options() {
  return { debuggerPath: '/example/fe2o3-debug', inspectorPath: '/example/inspector',
    kirPath: '/example/kernel.kir', requestPath: '/example/request.json', outputDirectory: '/example/fresh-output',
    resultMode: 'used', operandOrder: [0, 1, 2], registerPlan: [32, 33, 34, 35, 36] };
}
function args() {
  const o = options();
  return ['--debugger', o.debuggerPath, '--inspector', o.inspectorPath, '--kir', o.kirPath,
    '--request', o.requestPath, '--output', o.outputDirectory, '--result-mode', o.resultMode,
    '--operand-order', '0,1,2', '--register-plan', '32,33,34,35,36'];
}
function request(inputs = [19, 23, 42]) {
  // Synthetic caller input to an independent pure oracle, never captured data.
  return { schema: 'fe2o3-simulation-request-v1', kernel: 'synthetic_oracle_control',
    grid: [64, 1, 1], workgroup: [64, 1, 1],
    arguments: [{ kind: 'buffer_view', backing: 1, element: 'u32', access: 'read_write',
      alignment: 4, byte_offset: 4, elements: 64 },
    ...inputs.map(value => ({ kind: 'scalar', type: 'u32', bits: `0x${value.toString(16).padStart(8, '0')}` }))],
    shared_buffers: [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
      bytes: `0x${'a5'.repeat(264)}`, initialized: `0x${'00'.repeat(33)}` }] };
}

test('eight required options are explicit and caller arrays are not shared', () => {
  assert.deepEqual(parseSmokeArguments(args()), options());
  const original = options(), copy = validateSmokeOptions(original);
  copy.operandOrder.reverse(); copy.registerPlan[0] = 9;
  assert.deepEqual(original, options());
});

test('missing duplicate unknown and noncanonical command options refuse', () => {
  const cases = [args().slice(2), [...args(), '--extra', 'x']];
  for (const [index, value] of [[0, '--unknown'], [2, '--debugger'], [13, '00,1,2'],
    [13, '0,1,1'], [13, '0,1,2,3'], [13, '-1,1,2'], [13, '0,1,2e0'],
    [15, '32,33,34,35,64'], [15, '32,33,34,35,35'], [15, '32,33,34,35'],
    [15, '9007199254740992,33,34,35,36']]) {
    const changed = args(); changed[index] = value; cases.push(changed);
  }
  for (const value of cases) assert.throws(() => parseSmokeArguments(value));
});

test('unsupported path behavior and omitted independent expectations refuse', () => {
  const mutations = [
    o => { o.debuggerPath = 'relative'; }, o => { o.kirPath = '/example/../kernel'; },
    o => { o.requestPath = '/example/request\n.json'; }, o => { o.outputDirectory = '/' + 'a'.repeat(4096); },
    o => { o.resultMode = 'infer'; }, o => { delete o.operandOrder; },
    o => { o.operandOrder = [0, 1, '2']; }, o => { o.registerPlan = [32, 33, 34, 35, 1.5]; },
    o => { o.registerPlan = [32, 33, 34, 35, -1]; }, o => { o.inferFromDebugger = true; },
  ];
  for (const mutate of mutations) { const o = options(); mutate(o); assert.throws(() => validateSmokeOptions(o)); }
});

test('independent U32 oracle preserves canaries and exact initialization across wraparound', () => {
  const cases = [
    [[0, 0, 0], 0], [[0xffffffff, 0, 1], 0], [[0xffffffff, 1, 2], 0],
    [[0x80000000, 0, 0x80000000], 0], [[0xaaaa5555, 0x5555aaaa, 19], 18], [[19, 23, 42], 46],
  ];
  for (const [inputs, result] of cases) {
    for (const resultMode of ['used', 'unused']) {
      const input = request(inputs), original = structuredClone(input);
      const expected = deriveRequestExpectation(input, { resultMode, operandOrder: [0, 1, 2] });
      assert.deepEqual(input, original, 'oracle cannot alter caller input');
      assert.deepEqual(expected.beforeInputs, inputs); assert.equal(expected.afterResult, result);
      const output = resultMode === 'used' ? result : inputs[0];
      assert.equal(expected.outputValue, output);
      const bytes = Buffer.from(expected.outputMemory.bytes.slice(2), 'hex');
      assert.equal(bytes.length, 264);
      assert.deepEqual([...bytes.subarray(0, 4)], [165, 165, 165, 165]);
      assert.deepEqual([...bytes.subarray(260)], [165, 165, 165, 165]);
      for (let lane = 0; lane < 64; lane++) assert.equal(bytes.readUInt32LE(4 + 4 * lane), output);
      assert.equal(expected.outputMemory.initialized, `0xf0${'ff'.repeat(31)}0f`);
      assert.equal(expected.expectedAccessByteOffset, 4); assert.equal(expected.expectedAccessCount, 64);
    }
  }
});

test('operand ordering is explicit and unused result stays logically computed', () => {
  const used = deriveRequestExpectation(request(), { resultMode: 'used', operandOrder: [2, 1, 0] });
  const unused = deriveRequestExpectation(request(), { resultMode: 'unused', operandOrder: [2, 1, 0] });
  assert.deepEqual(used.beforeInputs, [42, 23, 19]);
  assert.equal(used.afterResult, 80); assert.equal(used.outputValue, 80);
  assert.equal(unused.afterResult, 80); assert.equal(unused.outputValue, 19);
});

test('unsupported request topologies and buffer initialization refuse', () => {
  const mutations = [
    r => { r.extra = true; }, r => { r.schema = 'other'; }, r => { r.kernel = ''; },
    r => { r.grid = [128, 1, 1]; }, r => { r.workgroup = [32, 1, 1]; },
    r => { r.arguments.pop(); }, r => { r.arguments.push(r.arguments[1]); },
    r => { r.arguments[0].kind = 'buffer'; }, r => { r.arguments[0].backing = 2; },
    r => { r.arguments[0].byte_offset = 0; }, r => { r.arguments[0].elements = 63; },
    r => { r.arguments[0].access = 'write_only'; }, r => { r.arguments[0].alignment = 8; },
    r => { r.arguments[1].type = 'i32'; }, r => { r.arguments[1].bits = '0xfffffffff'; },
    r => { r.arguments[1].bits = '0xFFFFFFFF'; }, r => { r.arguments[1].bits = '-1'; },
    r => { r.arguments[1].bits = '1e3'; }, r => { r.arguments[1].value = 19; },
    r => { r.shared_buffers.push(r.shared_buffers[0]); }, r => { r.shared_buffers[0].id = 2; },
    r => { r.shared_buffers[0].bytes += 'a5'; }, r => { r.shared_buffers[0].bytes = '0x00' + 'a5'.repeat(263); },
    r => { r.shared_buffers[0].initialized = `0x${'ff'.repeat(33)}`; },
    r => { r.shared_buffers[0].initialized = `0x${'00'.repeat(32)}`; },
  ];
  for (const mutate of mutations) { const r = request(); mutate(r); assert.throws(() => deriveRequestExpectation(r, options())); }
});

function inspectionMetadata() {
  // Synthetic metadata used ONLY to mutate individual validator inputs below.
  // It is not emitted by a process, persisted as evidence, or fed to the client.
  return { kind: 'diagnostic_ordered_region_inspection_example', authority: 'observation_only',
    canonical: { wire_version: 16, sha256: '1'.repeat(64), bytes: 1352 },
    kernel: 'synthetic_oracle_control', function: 'synthetic_function',
    coordinate: { function_ordinal: 0, block_ordinal: 1, operation_ordinal: 0 }, raw_block_id: 89,
    input_value_ids: [101, 202, 303], result_value_id: 404,
    declared_target: 'gfx942:xnack-', declared_wave_width: 64, profile: 'xor_add_u32_e32',
    register_plan: { scratch: 32, output: 33, inputs: [34, 35, 36], vgpr_high_water: 37 },
    declared_instruction_steps: [{ instruction: 'v_xor_b32_e32', output: 32, inputs: [34, 35] },
      { instruction: 'v_add_u32_e32', output: 33, inputs: [32, 36] }],
    declared_source_ids: { frontend_unit: '2'.repeat(64), function: '3'.repeat(64), contract: '4'.repeat(64), statement: '5'.repeat(64) },
    memory_effect: 'NoMemory', ordered_region_effect: true, pure_or_movable: false,
    logical_observation_granularity: 'whole_region_before_after', cpu_preflight_passed: true,
    source_authentication: false, source_map_available: false, physical_register_values_available: false,
    instruction_microsteps_available: false, register_lifetime_or_final_allocation_proof: false,
    proof_authority: false, artifact_authority: false, production_resume_authority: false, hardware_execution: false,
    inspection_counts: { blocks: 2, operations: 8, ssa_definitions: 16, capability_entries: 1, name_bytes: 80 },
    inspection_max_canonical_bytes_after_admission: 65536, cpu_preflight_resident_limit_bytes: 67108864,
    output_buffer_bytes: 8192, accounting_scope: 'Synthetic validator-only control; not an execution or evidence receipt.' };
}
const inspectionExpectation = () => ({ kirBytes: 1352, kernel: 'synthetic_oracle_control', registerPlan: [32, 33, 34, 35, 36] });

test('pure metadata projection keeps current roster and raw block distinct without changing input', () => {
  // Establish the negative controls' baseline only. This projects caller data;
  // it does not execute the smoke, verify bytes, or create a passing receipt.
  const view = inspectionMetadata(), original = structuredClone(view);
  const projected = validateInspection(view, inspectionExpectation());
  assert.deepEqual(projected, { region: { functionOrdinal: 0, blockOrdinal: 1, operationOrdinal: 0 },
    rawBlockId: 89, canonicalIdentity: '1'.repeat(64), inputValueIds: [101, 202, 303], resultValueId: 404 });
  projected.inputValueIds[0] = 9;
  assert.deepEqual(view, original);
  assert.notEqual(projected.rawBlockId, projected.region.blockOrdinal);
});

test('changed inspector owner/profile/current SSA/declared plan metadata refuses', () => {
  const mutations = [
    v => { v.kind = 'other'; }, v => { v.authority = 'trusted'; },
    v => { v.canonical.wire_version = 7; }, v => { v.canonical.bytes++; }, v => { v.canonical.sha256 = 'bad'; },
    v => { v.kernel = 'another_owner'; }, v => { v.function = ''; },
    v => { v.declared_target = 'gfx1100'; }, v => { v.declared_wave_width = 32; }, v => { v.profile = 'other'; },
    v => { v.memory_effect = 'ReadsMemory'; }, v => { v.ordered_region_effect = false; },
    v => { v.cpu_preflight_passed = false; }, v => { v.logical_observation_granularity = 'instruction'; },
    v => { v.coordinate.function_ordinal = 1; }, v => { v.coordinate.block_ordinal = 128; },
    v => { v.coordinate.operation_ordinal = 4096; }, v => { v.raw_block_id = -1; },
    v => { v.input_value_ids = [101, 101, 303]; }, v => { v.input_value_ids = [101, 202]; },
    v => { v.result_value_id = 303; }, v => { v.register_plan.scratch = 40; },
    v => { v.register_plan.vgpr_high_water = 36; }, v => { v.declared_instruction_steps.reverse(); },
    v => { v.declared_instruction_steps[1].inputs.reverse(); },
    v => { v.declared_source_ids.statement = 'bad'; },
  ];
  for (const mutate of mutations) { const view = inspectionMetadata(); mutate(view); assert.throws(() => validateInspection(view, inspectionExpectation())); }
});

test('inspector authority upgrades and exceeded accounting limits refuse', () => {
  const flags = ['source_authentication', 'source_map_available', 'physical_register_values_available',
    'instruction_microsteps_available', 'register_lifetime_or_final_allocation_proof', 'proof_authority',
    'artifact_authority', 'production_resume_authority', 'hardware_execution', 'pure_or_movable'];
  for (const flag of flags) {
    const view = inspectionMetadata(); view[flag] = true;
    assert.throws(() => validateInspection(view, inspectionExpectation()));
  }
  const mutations = [
    v => { v.inspection_counts.blocks = 129; }, v => { v.inspection_counts.operations = 4097; },
    v => { v.inspection_counts.ssa_definitions = 8193; }, v => { v.inspection_counts.capability_entries = 257; },
    v => { v.inspection_counts.name_bytes = 16385; }, v => { v.inspection_counts.blocks = 1; },
    v => { v.inspection_counts.operations = 0; }, v => { v.inspection_counts.ssa_definitions = 3; },
    v => { v.inspection_max_canonical_bytes_after_admission++; }, v => { v.cpu_preflight_resident_limit_bytes++; },
    v => { v.output_buffer_bytes++; }, v => { v.accounting_scope = 'x'.repeat(1025); },
  ];
  for (const mutate of mutations) { const view = inspectionMetadata(); mutate(view); assert.throws(() => validateInspection(view, inspectionExpectation())); }
});
