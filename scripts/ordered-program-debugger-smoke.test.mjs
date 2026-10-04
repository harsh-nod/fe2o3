// Pure option/oracle/inspection-validator controls. No compiler, inspector or
// debugger is executed, and no backend response or passing evidence is emitted.

test('one-step descriptor space has exactly48 valid complete programs', () => {
  let count = 0;
  for (let word = 0; word <= 65535; word++) {
    // Independent bit interpretation for the full one-step truth table.
    const opcode = word & 7, output = (word & 8) !== 0;
    const lhs = (word >> 4) & 7, rhs = (word >> 7) & 7;
    const accepted = (word & 0xfc00) === 0 && output && opcode < 6 && lhs < 3
      && (opcode === 0 ? rhs === 0 : rhs < 3);
    let result;
    try { result = validateProgramDescriptors([word]); } catch {}
    assert.equal(Boolean(result), accepted, 'word=' + word.toString(16));
    if (result) count++;
  }
  assert.equal(count, 48);
});

test('descriptor counts, reserved bits, move arity and pre-step initialization refuse', () => {
  for (const active of [[], Array(17).fill(8), [65536], [-1], [1.5], [NaN], [Infinity], [8n],
    ['8'], [0x408], [0x8008], [0x0e], [0x0f], [0x58], [0x388], [0x88], [0x38], [0x48],
    [0], [48,56], [8,0x233]]) assert.throws(() => validateProgramDescriptors(active));
  const active=[0,48,56], checked=validateProgramDescriptors(active);
  assert.deepEqual(checked,active); checked[0]=8; assert.deepEqual(active,[0,48,56]);
  assert.equal(evaluateProgram([8,0x249],[0x80000000,1,2]),0,'output read observes previous step and wraps');
  assert.equal(evaluateProgram([0,0x1b1,56],[19,23,42]),38,'scratch self read is pre-step');
});

test('all six forms and wrapping edges use independent arithmetic without backend values', () => {
  const oracle = [
    v => v[0], v => (v[0]+v[1])>>>0, v => (v[0]-v[1])>>>0,
    v => (v[0]&v[1])>>>0, v => (v[0]|v[1])>>>0, v => (v[0]^v[1])>>>0,
  ];
  for (const values of [[0,0,0],[0xffffffff,1,2],[0,1,0],[0x80000000,0x80000000,0],
    [0xaaaa5555,0x5555aaaa,19],[19,23,42]]) {
    for (const [index,descriptor] of [8,137,138,139,140,141].entries())
      assert.equal(evaluateProgram([descriptor],values),oracle[index](values));
    assert.equal(evaluateProgram([133,307,413],values),(values[1]^((values[0]^values[1])&values[2]))>>>0);
  }
  for(const values of [[],[1,2],[1,2,3,4],[-1,2,3],[0x100000000,2,3],[1.5,2,3],[1n,2,3]])
    assert.throws(()=>evaluateProgram([8],values));
});

test('dead, repeated, self and all16 active steps are retained exactly', () => {
  const sixteen=[0,...Array(14).fill(48),56];
  for (const active of [[8],[0,8],[8,8],[8,72],Array(16).fill(8),sixteen]) {
    const preserved=validateProgramDescriptors(active);
    assert.deepEqual(preserved,active);
    assert.equal(evaluateProgram(active,[19,23,42]),19);
    assert.equal(declaredSteps(active,options().registerPlan).length,active.length);
  }
  const allSix=[8,137,138,139,140,141];
  assert.deepEqual(declaredSteps(allSix,[40,41,42,43,44]),[
    {instruction:'v_mov_b32_e32',output:41,inputs:[42]},
    {instruction:'v_add_u32_e32',output:41,inputs:[42,43]},
    {instruction:'v_sub_u32_e32',output:41,inputs:[42,43]},
    {instruction:'v_and_b32_e32',output:41,inputs:[42,43]},
    {instruction:'v_or_b32_e32',output:41,inputs:[42,43]},
    {instruction:'v_xor_b32_e32',output:41,inputs:[42,43]},
  ]);
  assert.equal(evaluateProgram(allSix,[19,23,42]),4);
});

test('program-aware full backing oracle keeps unused output and computed result separate', () => {
  for(const descriptors of [[8],[133,307,413],[0,...Array(14).fill(48),56],[8,137,138,139,140,141]]) {
    for(const resultMode of ['used','unused']) {
      const expected=deriveRequestExpectation(request(),{...options(),descriptors,resultMode,operandOrder:[2,1,0]});
      const expectedValue=evaluateProgram(descriptors,[42,23,19]);
      assert.equal(expected.afterResult,expectedValue);
      assert.equal(expected.outputValue,resultMode==='used'?expectedValue:19);
      const bytes=Buffer.from(expected.outputMemory.bytes.slice(2),'hex');
      assert.equal(bytes.subarray(0,4).toString('hex'),'a5a5a5a5');
      assert.equal(bytes.subarray(260).toString('hex'),'a5a5a5a5');
      for(let lane=0;lane<64;lane++)assert.equal(bytes.readUInt32LE(4+4*lane),expected.outputValue);
      assert.equal(expected.outputMemory.initialized,'0xf0'+'ff'.repeat(31)+'0f');
    }
  }
});

test('explicit program/source expectations cannot be omitted, padded ambiguously or inferred', () => {
  for(const mutate of [
    o=>{delete o.descriptors;},o=>{o.descriptors=[];},o=>{o.descriptors=Array(17).fill(8);},
    o=>{o.descriptors=[8,0x400];},o=>{delete o.sourceIds;},o=>{o.sourceIds=['2'.repeat(64)];},
    o=>{o.sourceIds[0]='0'.repeat(64);},o=>{o.sourceIds[1]='A'.repeat(64);},
    o=>{o.sourceIds[2]='2'.repeat(65);},o=>{o.sourceIds[3]=9;},
    o=>{delete o.sourceIds[0];},o=>{delete o.registerPlan[0];},o=>{delete o.descriptors[0];},
  ]){const o=options();mutate(o);assert.throws(()=>validateSmokeOptions(o));}
  for(const [index,value] of [[17,'0133,313'],[17,'133,313,'],[17,'0x85,313'],
    [17,'133,313e0'],[17,'133,-1'],[19,'0'.repeat(64)+','+options().sourceIds.slice(1).join(',')]]) {
    const a=args();a[index]=value;assert.throws(()=>parseSmokeArguments(a));
  }
  const original=options(),copy=validateSmokeOptions(original);
  copy.descriptors[0]=8;copy.sourceIds[0]='9'.repeat(64);assert.deepEqual(original,options());
});

test('inspection checks exact count/all16 padding/typed steps/source IDs before projection', () => {
  for(const mutate of [
    v=>{v.canonical.wire_version=16;},v=>{v.declared_program.count=1;},
    v=>{v.declared_program.descriptors[2]=1;},v=>{v.declared_program.descriptors.pop();},
    v=>{v.declared_program.descriptors.push(0);},v=>{v.declared_program.descriptors[0]=141;},
    v=>{v.declared_program.extra=true;},v=>{delete v.declared_program;},
    v=>{v.declared_instruction_steps[0].inputs.push(36);},
    v=>{v.declared_source_ids.statement='6'.repeat(64);},
    v=>{v.declared_source_ids.frontend_unit='0'.repeat(64);},
    v=>{v.source_authentication=true;},v=>{v.unreviewed_final_artifact=true;},
  ]){const view=inspectionMetadata();mutate(view);assert.throws(()=>validateInspection(view,inspectionExpectation()));}
});

test('one three and sixteen step inspector-only metadata controls never fabricate a session', () => {
  for(const descriptors of [[8],[133,307,413],[0,...Array(14).fill(48),56]]) {
    const view=inspectionMetadata();
    view.declared_program={count:descriptors.length,descriptors:[...descriptors,...Array(16-descriptors.length).fill(0)]};
    view.declared_instruction_steps=declaredSteps(descriptors,options().registerPlan);
    const original=structuredClone(view);
    const actual=validateInspection(view,{...inspectionExpectation(),descriptors});
    assert.equal(actual.canonicalIdentity,'1'.repeat(64));
    assert.equal(actual.rawBlockId,89);assert.equal(actual.region.blockOrdinal,1);
    assert.deepEqual(view,original);
  }
});
import assert from 'node:assert/strict';
import test from 'node:test';
import { deriveRequestExpectation, parseSmokeArguments, validateInspection, validateSmokeOptions, validateProgramDescriptors, evaluateProgram, declaredSteps }
  from './ordered-program-debugger-smoke.mjs';

function options() {
  return { debuggerPath: '/example/fe2o3-debug', inspectorPath: '/example/inspector',
    kirPath: '/example/kernel.kir', requestPath: '/example/request.json', outputDirectory: '/example/fresh-output',
    resultMode: 'used', operandOrder: [0, 1, 2], registerPlan: [32, 33, 34, 35, 36], descriptors: [133,313], sourceIds: ['2','3','4','5'].map(v=>v.repeat(64)) };
}
function args() {
  const o = options();
  return ['--debugger', o.debuggerPath, '--inspector', o.inspectorPath, '--kir', o.kirPath,
    '--request', o.requestPath, '--output', o.outputDirectory, '--result-mode', o.resultMode,
    '--operand-order', '0,1,2', '--register-plan', '32,33,34,35,36', '--descriptors','133,313','--source-ids',o.sourceIds.join(',')];
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

test('ten required options are explicit and caller arrays are not shared', () => {
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
      const expected = deriveRequestExpectation(input, { ...options(), resultMode, operandOrder: [0, 1, 2] });
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
  const used = deriveRequestExpectation(request(), { ...options(), resultMode: 'used', operandOrder: [2, 1, 0] });
  const unused = deriveRequestExpectation(request(), { ...options(), resultMode: 'unused', operandOrder: [2, 1, 0] });
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
  return { kind: 'diagnostic_ordered_program_inspection_example', authority: 'observation_only',
    canonical: { wire_version: 17, sha256: '1'.repeat(64), bytes: 1352 },
    kernel: 'synthetic_oracle_control', function: 'synthetic_function',
    coordinate: { function_ordinal: 0, block_ordinal: 1, operation_ordinal: 0 }, raw_block_id: 89,
    input_value_ids: [101, 202, 303], result_value_id: 404,
    declared_target: 'gfx942:xnack-', declared_wave_width: 64, profile: 'closed_u32_program_e32_v1',
    register_plan: { scratch: 32, output: 33, inputs: [34, 35, 36], vgpr_high_water: 37 },
    declared_program: {count:2,descriptors:[133,313,...Array(14).fill(0)]},
    declared_instruction_steps: [{ instruction: 'v_xor_b32_e32', output: 32, inputs: [34, 35] },
      { instruction: 'v_add_u32_e32', output: 33, inputs: [32, 36] }],
    declared_source_ids: { frontend_unit: '2'.repeat(64), function: '3'.repeat(64), contract: '4'.repeat(64), statement: '5'.repeat(64) },
    memory_effect: 'NoMemory', ordered_region_effect: true, pure_or_movable: false,
    logical_observation_granularity: 'whole_program_before_after', cpu_preflight_passed: true,
    source_authentication: false, source_map_available: false, physical_register_values_available: false,
    instruction_microsteps_available: false, register_lifetime_or_final_allocation_proof: false,
    proof_authority: false, artifact_authority: false, production_resume_authority: false, hardware_execution: false,
    inspection_counts: { blocks: 2, operations: 8, ssa_definitions: 16, capability_entries: 1, name_bytes: 80 },
    inspection_max_canonical_bytes_after_admission: 65536, cpu_preflight_resident_limit_bytes: 67108864,
    output_buffer_bytes: 8192, accounting_scope: 'Synthetic validator-only control; not an execution or evidence receipt.' };
}
function inspectionExpectation() { return { kirBytes: 1352, kernel: 'synthetic_oracle_control', registerPlan: [32, 33, 34, 35, 36], descriptors: options().descriptors, sourceIds: options().sourceIds }; }

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

