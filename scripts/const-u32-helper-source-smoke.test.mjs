// Pure synthetic controls only: no filesystem access, compiler or subprocesses.
import test from 'node:test';
import assert from 'node:assert/strict';
import { INPUTS, LABELS, LENGTHS, LIMITS, directConstant, expectedHelper, inspectInstances,
  options, oracle, parseJson, replaceOnce, request, selectBaseline, sha256, sourceVariant,
  validateJoins, validateMaterialization, validatePage, validateRoster, validateSimulation,
  validateSummary } from './const-u32-helper-source-smoke.mjs';
const copy = value => structuredClone(value);
const H = label => sha256(Buffer.from('synthetic:' + label));
const authority = { observation_only: true, authenticates_compiler_execution: false,
  source_authenticated: false, grants_proof_authority: false, grants_production_resume: false,
  grants_load_or_launch: false };
const v = value => ({ value, ty: 'Scalar(U32)' });
const coord = (operation, fn = 0, block = 0) => ({ function: fn, block, operation });
const span = { file_identity: H('source'), display_path: 'src/lib.rs', byte_start: '1',
  byte_end: '4', line: 1, column: 1 };
function operation(index, kind, detail, inputs, results, fn = 0) {
  return { coordinate: coord(index, fn), function_name: 'synthetic_fn_' + fn,
    kind, semantic_detail: detail, mnemonic: null, inline_assembly_source: null,
    inputs: inputs.map(v), results: results.map(v), local_memory_effects: [],
    complete_local_effect_summary: true, convergence: 'not_analyzed', traps: 'not_analyzed',
    physical_resources: 'unavailable_logical_canonical_stage',
    source_binding: 'bundle_content_bound_not_authenticated', source_spans: [copy(span)],
    materialization: 'diagnostic_rust_draft_available' };
}
function baselineOperations() {
  return [operation(0, 'constant', 'U32(255)', [], [2]),
    operation(1, 'binary', 'BitXor', [0, 1], [3]),
    operation(2, 'binary', 'BitAnd', [3, 2], [4]),
    operation(3, 'constant', 'U32(256)', [], [5]),
    operation(4, 'binary', 'BitOr', [4, 5], [6]),
    operation(5, 'store', null, [6], [])];
}
function summary(label = 'baseline', operations = baselineOperations()) {
  return { schema: 'fe2o3-multilevel-authoring-observation-v1', authority: copy(authority),
    bundle_identity: H(label + ':bundle'), bundle_subject_identity: H(label + ':subject'),
    canonical_kir_version: 11, canonical_kir_digest: H(label + ':kir'), canonical_kir_bytes: '512',
    target: 'gfx942:xnack-', source_map_identity: H(label + ':source-map'),
    semantic_mir_identity: H(label + ':semantic'),
    rustc_identity_inventory_receipt_sha256: H(label + ':inventory'),
    rustc_identity_inventory_receipt_bytes: '128',
    rustc_preflight_plan_receipt_sha256: H(label + ':preflight'), rustc_preflight_plan_receipt_bytes: '256',
    compiler_policy_identity: 'unavailable_in_v6',
    final_artifact_identity: 'unavailable_extraction_precedes_final_artifact',
    operation_count: operations.length, eliminated_source_span_count: 0, capabilities: [] };
}
function selected() { return selectBaseline(summary(), baselineOperations()); }
function materialized(selection = selected()) {
  const { selected: op, constant, selector } = selection;
  const runtime = op.inputs.find(input => input.value !== constant.value.value);
  return { schema: 'fe2o3-const-u32-helper-draft-v1', authority: copy(authority),
    region: { authority: copy(authority), selector: copy(selector),
      structural_boundary: 'contiguous_operations_in_one_block_no_terminator_selected',
      source_insertion_boundary: 'unavailable_source_application_not_admitted',
      live_in: copy(op.inputs), live_out: copy(op.results), operations: [copy(op)],
      materialization: 'diagnostic_rust_draft_available' },
    helper_name: 'specialized_or', source: expectedHelper(op, constant),
    runtime_parameters: [copy(runtime)], const_parameter: { name: 'C0', ...copy(constant) },
    original_call_template: 'specialized_or::<256u32>(v' + runtime.value + ')',
    status: 'diagnostic_const_u32_source_draft_only',
    frontend_readmission: 'not_performed_requires_fresh_source_compilation',
    source_application: 'unavailable_requires_explicit_new_source_and_normal_frontend',
    semantic_equivalence: 'unproved', exact_machine_contract: 'unproved',
    physical_register_bindings: 'unavailable_compiler_owned_scalar_values' };
}
function specializedOperations(label) {
  const ops = baselineOperations().slice(0, 3);
  ops.push(operation(3, 'call', null, [4], [6]), operation(4, 'store', null, [6], []));
  const values = label === 'two' ? [256, 512] : [label === 'default256' ? 256 : 512];
  for (const [index, bits] of values.entries()) {
    const fn = index + 1, constant = operation(0, 'constant', 'U32(' + bits + ')', [], [0], fn);
    const assembly = operation(1, 'inline_assembly', null, [1, 0], [2], fn);
    assembly.mnemonic = 'v_or_b32'; assembly.inline_assembly_source = {
      frontend_unit: H(label + ':unit'), function: H('instance:' + bits), contract: H('contract'),
      statement: H(label + ':' + bits + ':statement'), authority: 'inert_references_not_source_authentication' };
    ops.push(constant, assembly);
  }
  return ops;
}
function simulated(query, identity, word) {
  const elements = query.arguments[0].elements, bytes = Buffer.from(query.shared_buffers[0].bytes.slice(2), 'hex');
  const initialized = Buffer.alloc(Math.ceil(bytes.length / 8));
  for (let at = 0; at < elements; at++) bytes.writeUInt32LE(word, 4 + at * 4);
  for (let at = 4; at < 4 + elements * 4; at++) initialized[Math.floor(at / 8)] |= 2 ** (at % 8);
  return { schema: 'fe2o3-simulation-result-v1', status: 'ok', authority: 'observation_only',
    simulated: true, hardware_observed: false, hardware_validation: false, performance_prediction: false,
    target_profile: { identity: 'amdgpu_64_little_endian_v1', index_bits: 64, max_workgroup_invocations: 1024 },
    kir: { sha256: identity.canonical_kir_digest, canonical_bytes: Number(identity.canonical_kir_bytes) },
    counts: { arguments: 3, shared_buffers: 1, invocations_executed: query.grid[0],
      workgroups_visited: query.grid[0] / 64 },
    schedule: { transcript_sha256: H('schedule'), coverage: { complete: true } },
    arguments: copy(query.arguments), shared_buffers: [{ id: 1, buffer: {
      element: 'u32', access: 'read_write', alignment: 4, bytes: '0x' + bytes.toString('hex'),
      initialized: '0x' + initialized.toString('hex') } }] };
}
function variants() {
  return LABELS.map(label => {
    const profile = label === 'repeat' ? 'edited512' : label;
    const operations = profile === 'baseline' ? baselineOperations() : specializedOperations(profile);
    const identity = summary(profile, operations);
    return { label, source_sha256: H(profile + ':source'), bundle_file_sha256: H(profile + ':file'),
      summary: identity, instances: inspectInstances(profile, identity, operations) };
  });
}

test('complete bounded current summary is authority-free and exact', () => {
  const current = summary(), before = copy(current); validateSummary(current); assert.deepEqual(current, before);
  for (const mutate of [x => x.canonical_kir_version = 17, x => x.target = 'gfx950',
    x => x.authority.grants_production_resume = true, x => x.operation_count = 513,
    x => x.semantic_mir_identity = '0'.repeat(64), x => x.canonical_kir_bytes = '01',
    x => x.extra = true]) {
    const value = summary(); mutate(value); assert.throws(() => validateSummary(value));
  }
});
test('operation paging requires current identities and exact complete boundaries', () => {
  const identity = summary(), ops = baselineOperations();
  const page = { authority: copy(authority), bundle_identity: identity.bundle_identity,
    canonical_kir_digest: identity.canonical_kir_digest, target: identity.target, start: 0,
    next_start: null, total_operations: ops.length, operations: ops };
  assert.equal(validatePage(page, identity, 0), ops.length);
  for (const mutate of [x => x.start = 1, x => x.next_start = 6, x => x.operations = [],
    x => x.bundle_identity = H('stale'), x => x.total_operations++, x => x.extra = true]) {
    const changed = copy(page); mutate(changed); assert.throws(() => validatePage(changed, identity, 0));
  }
});
test('fixed-limit paging rejects coherent short pages and accepts complete multipage tails', () => {
  const identity = summary(), ops = baselineOperations();
  const short = { authority: copy(authority), bundle_identity: identity.bundle_identity,
    canonical_kir_digest: identity.canonical_kir_digest, target: identity.target, start: 0,
    next_start: 2, total_operations: ops.length, operations: ops.slice(0, 2) };
  assert.throws(() => validatePage(short, identity, 0), /exact fixed-limit operation page length/u);
  const complete = Array.from({ length: 67 }, (_, index) => operation(index, 'constant', 'U32(1)', [], [index + 2]));
  const current = summary('multipage', complete);
  const first = { authority: copy(authority), bundle_identity: current.bundle_identity,
    canonical_kir_digest: current.canonical_kir_digest, target: current.target, start: 0,
    next_start: 64, total_operations: complete.length, operations: complete.slice(0, 64) };
  assert.equal(validatePage(first, current, 0), 64);
  const tail = { ...copy(first), start: 64, next_start: null, operations: complete.slice(64) };
  assert.equal(validatePage(tail, current, 64), 67);
  validateRoster([...first.operations, ...tail.operations], current);
});
test('roster rejects repeated coordinates or function-local SSA definitions', () => {
  for (const mutate of [ops => ops[4].coordinate = copy(ops[3].coordinate),
    ops => ops[4].results[0] = copy(ops[3].results[0]), ops => ops.reverse()]) {
    const ops = baselineOperations(); mutate(ops); assert.throws(() => validateRoster(ops, summary()));
  }
});
test('baseline selects only actual ordinary OR and derives its real constant definition', () => {
  const ops = baselineOperations(), before = copy(ops), selection = selectBaseline(summary(), ops);
  assert.deepEqual(selection.constant, { value: v(5), definition: coord(3), original_value: 256 });
  assert.deepEqual(selection.selector.operations, [coord(4)]); assert.deepEqual(ops, before);
  for (const mutate of [rows => rows[4].semantic_detail = 'BitXor',
    rows => rows[2].semantic_detail = 'BitOr', rows => rows[4].source_spans = [],
    rows => rows[3].semantic_detail = 'U32(512)']) {
    const rows = baselineOperations(); mutate(rows); assert.throws(() => selectBaseline(summary(), rows));
  }
});
test('constant observation rejects absent, aliased, cross-block and later definitions', () => {
  const mutations = [rows => rows[3].kind = 'cast',
    rows => rows[3].coordinate.block = 1, rows => rows[3].coordinate.operation = 5,
    rows => rows[3].semantic_detail = 'U64(256)', rows => rows[3].semantic_detail = 'U32(0256)',
    rows => rows[3].semantic_detail = 'U32(4294967296)',
    rows => rows[3].results[0].ty = 'Scalar(U64)', rows => rows[3].inputs = [v(0)],
    rows => rows[2].kind = 'constant', rows => rows[4].inputs[1] = v(4)];
  for (const mutate of mutations) {
    const rows = baselineOperations(); mutate(rows); assert.throws(() => directConstant(rows, rows[4]));
  }
});
test('both direct constants refuse rather than treating one as a runtime parameter', () => {
  const rows = baselineOperations(); rows[2].kind = 'constant'; rows[2].semantic_detail = 'U32(255)'; rows[2].inputs = [];
  assert.throws(() => directConstant(rows, rows[4]), /exactly one directly observed constant/u);
});
test('materialization joins actual owner-derived value, coordinate, source and boundary', () => {
  const selection = selected(), generated = materialized(selection), before = copy(generated);
  validateMaterialization(generated, selection); assert.deepEqual(generated, before);
  for (const mutate of [x => x.const_parameter.original_value = 512,
    x => x.const_parameter.definition.operation = 2, x => x.runtime_parameters[0].value = 5,
    x => x.region.selector.canonical_kir_digest = H('old'), x => x.region.live_out = [],
    x => x.source += '// injected\n', x => x.authority.source_authenticated = true,
    x => x.source_application = 'applied', x => x.extra = 256]) {
    const value = copy(generated); mutate(value); assert.throws(() => validateMaterialization(value, selection));
  }
});
test('exact generated source respects observed operand order', () => {
  const rows = baselineOperations(); rows[4].inputs.reverse();
  const selection = selectBaseline(summary(), rows), generated = materialized(selection);
  assert.match(generated.source, /v_or_b32\(C0, v4\)/u); validateMaterialization(generated, selection);
});
test('fresh variants alter only the known line and append the exact helper', () => {
  const original = Buffer.from('// λ prefix\n    let result = low | 256;\n// tail\n'), before = Buffer.from(original);
  const helper = materialized().source;
  for (const label of ['default256', 'edited512', 'two']) {
    const value = sourceVariant(original, helper, label);
    assert.ok(value.subarray(0, Buffer.byteLength('// λ prefix\n')).equals(Buffer.from('// λ prefix\n')));
    assert.ok(value.toString().endsWith('// tail\n\n' + helper));
  }
  assert.match(sourceVariant(original, helper, 'two').toString(),
    /specialized_or::<256u32>\(low\)\.0 \^ specialized_or::<512u32>\(a\)\.0/u);
  assert.deepEqual(original, before);
  for (const bytes of [Buffer.from('absent'), Buffer.from('x x'), Buffer.from([0xff, 120]), Buffer.alloc(65537, 120)]) {
    assert.throws(() => replaceOnce(bytes, 'x', 'y'));
  }
  assert.throws(() => sourceVariant(original, helper, 'repeat'));
});
test('retained specializations expose actual constants and distinct function identities', () => {
  for (const label of ['default256', 'edited512', 'two']) {
    const ops = specializedOperations(label), current = summary(label, ops), before = copy(ops);
    const seen = inspectInstances(label, current, ops); assert.deepEqual(ops, before);
    assert.deepEqual(seen.map(item => item.typed_constant.original_value), label === 'two' ? [256, 512] :
      [label === 'default256' ? 256 : 512]);
  }
});
test('two-specialization observation refuses same function, source identity or stale constant', () => {
  for (const mutate of [
    ops => ops.at(-1).inline_assembly_source.function = ops.at(-3).inline_assembly_source.function,
    ops => ops.at(-2).semantic_detail = 'U32(256)',
    ops => ops.at(-1).mnemonic = 'v_and_b32',
    ops => ops.at(-1).source_spans = [],
    ops => { ops.at(-2).coordinate.function = 1; ops.at(-1).coordinate.function = 1; }]) {
    const ops = specializedOperations('two'); mutate(ops);
    assert.throws(() => inspectInstances('two', summary('two', ops), ops));
  }
});
test('public call projection is never treated as an exact callee edge', () => {
  const ops = specializedOperations('two');
  // No callee field is available; removing the synthetic call observation does
  // not manufacture a claimed edge or change the retained-instance check.
  const rows = ops.filter(item => item.kind !== 'call');
  assert.equal(inspectInstances('two', summary('two', rows), rows).length, 2);
});
test('independent constant-specialization Boolean oracles have distinguishing values', () => {
  const inputs = [0xfffffff0, 0x25];
  assert.equal(oracle('baseline', inputs), 469); assert.equal(oracle('default256', inputs), 469);
  assert.equal(oracle('edited512', inputs), 725); assert.equal(oracle('repeat', inputs), 725);
  assert.equal(oracle('two', [0, 0]), 768);
  assert.equal(oracle('two', [0xffffffff, 0xffffffff]), 0xfffffeff);
  assert.throws(() => oracle('unknown', inputs)); assert.throws(() => oracle('baseline', [-1, 0]));
});
test('whole-buffer controls cover every planned input, length and source profile', () => {
  const identity = summary();
  for (const label of LABELS) for (const inputs of INPUTS) for (const length of LENGTHS) {
    const query = request(inputs, length), result = simulated(query, identity, oracle(label, inputs)), before = copy(result);
    const checked = validateSimulation(result, query, identity, label, inputs, length);
    assert.equal(checked.guard_bytes, 8); assert.equal(checked.output_words, length);
    assert.deepEqual(result, before);
  }
});
test('edited output fails the old oracle and scalar/canary/init/identity drift refuses', () => {
  const identity = summary(), query = request(INPUTS[4], 1);
  assert.throws(() => validateSimulation(simulated(query, identity, 725), query, identity, 'default256', INPUTS[4], 1));
  for (const mutate of [x => x.kir.sha256 = H('stale'), x => x.arguments[1].bits = '0x00000000',
    x => x.shared_buffers[0].buffer.initialized = '0xffff',
    x => x.shared_buffers[0].buffer.bytes = '0x00' + x.shared_buffers[0].buffer.bytes.slice(4),
    x => x.counts.invocations_executed = 1, x => x.hardware_observed = true,
    x => x.schedule.coverage.complete = false]) {
    const value = simulated(query, identity, 725); mutate(value);
    assert.throws(() => validateSimulation(value, query, identity, 'edited512', INPUTS[4], 1));
  }
});
test('variant joins demand fresh body identities and exact same-source repeat', () => {
  const current = variants(), before = copy(current); validateJoins(current); assert.deepEqual(current, before);
  for (const mutate of [x => x[2].summary.semantic_mir_identity = x[1].summary.semantic_mir_identity,
    x => x[3].source_sha256 = H('changed'), x => x[3].summary.source_map_identity = H('stale'),
    x => x[3].instances[0].typed_constant.original_value = 256,
    x => x[2].instances[0].source_reference.function = x[1].instances[0].source_reference.function]) {
    const value = variants(); mutate(value); assert.throws(() => validateJoins(value));
  }
});
test('bounded JSON rejects duplicate fields and imprecise numbers', () => {
  for (const text of ['{"a":1,"a":2}', '{"a":1,"\\u0061":2}', '{"a":9007199254740993}',
    '{"a":1e3}', '{} trailing']) assert.throws(() => parseJson(Buffer.from(text)));
  assert.deepEqual(parseJson(Buffer.from('{"value":256}')), { value: 256 });
  assert.throws(() => parseJson(Buffer.alloc(LIMITS.json_bytes + 1, 32)));
});
test('closed absolute CLI refuses in-repo output, aliases, missing and duplicate options', () => {
  const args = ['--repo', '/repo', '--bin-dir', '/bin', '--cargo', '/tool/cargo',
    '--rustc', '/tool/rustc', '--output', '/new/output'];
  assert.equal(options(args).output, '/new/output');
  assert.throws(() => options(args.slice(0, -2)));
  for (const [at, text] of [[8, '--repo'], [9, '/repo/target/out'], [9, '/bin/out'],
    [9, 'relative'], [1, '/repo/../repo']]) {
    const value = [...args]; value[at] = text; assert.throws(() => options(value));
  }
});
