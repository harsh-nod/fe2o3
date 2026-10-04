import test from 'node:test';
import assert from 'node:assert/strict';
import {
  LIMITS, INPUTS, LENGTHS, LABELS, SELECTIONS, ADDITIONS, SOURCE_FIXTURES, REFUSALS, sha256, parseJson,
  validateSources, validateNegativeSource, selectedAdditions, declaredProgram, declaredSteps, oracle, request,
  validateInspection, validateSimulation, validateMatrix, validateTransport, validateRefusal, options,
} from '../ordered-program-select-source-smoke.mjs';

// Exact reviewed source fixture bytes only. All diagnostic, SSA and simulation
// objects below are explicitly synthetic pure controls, never actual captures.
const POSITIVES = [
  Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(true) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            add(out, out, input1);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n"),
  Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\nconst SELECT: bool = 3_u32 < 4;\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(SELECT) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            add(out, out, input1);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n"),
  Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\nconst SELECT: bool = 4_u32 < 3;\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(SELECT) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            add(out, out, input1);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n"),
];
const NEGATIVES = new Map([
  ["dynamic", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(a != 0) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            add(out, out, input1);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
  ["non-bool", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(1_u32) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            add(out, out, input1);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
  ["nested", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(true) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            const_if(false) {\n                mov(out, input0);\n            } else {\n                mov(out, input1);\n            }\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
  ["missing-else", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(true) {\n            mov(out, input0);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
  ["unknown-opcode", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(true) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            mul(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
  ["inactive-undefined", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(true) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
  ["inactive-seventeen", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(32); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(true) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
  ["register-alias", Buffer.from("// Future normal-source fixture, not a claim of completed frontend admission.\n#![no_std]\nuse fe2o3_device::{DisjointSlice, amdgpu_ordered_program, kernel, thread};\n\n#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]\npub fn ordered_select_u32(\n    mut output: DisjointSlice<u32>,\n    a: u32,\n    b: u32,\n    c: u32,\n) {\n    let result = amdgpu_ordered_program! {\n        gfx942_xnack_off_wave64;\n        scratch(33); out(33); in(34) = a; in(35) = b; in(36) = c;\n        const_if(true) {\n            mov(out, input0);\n            add(out, out, input1);\n        } else {\n            mov(out, input0);\n            add(out, out, input1);\n            add(out, out, input1);\n        }\n    };\n    if let Some(element) = output.get_mut(thread::index_1d()) {\n        *element = result;\n    }\n}\n")],
]);
const ONE = POSITIVES[0];
const sources = () => POSITIVES.map(source => Buffer.from(source));
const copy = value => structuredClone(value);
const id = number => number.toString(16).padStart(64, '0');
function exported(count) {
  return { canonical_sha256: id(100 + count), canonical_bytes: 1000 + count,
    semantic_identity: id(200 + count), retained_source_inventory: id(300),
    retained_source_preflight: id(400 + count) };
}
function inspection(count) {
  const e = exported(count);
  return { kind: 'diagnostic_ordered_program_inspection_example', authority: 'observation_only',
    canonical: { wire_version: 17, sha256: e.canonical_sha256, bytes: e.canonical_bytes },
    kernel: 'ordered_select_u32', function: 'retained-entry-id', coordinate: { function_ordinal: 0, block_ordinal: 3, operation_ordinal: 5 },
    raw_block_id: 41, input_value_ids: [7, 8, 12], result_value_id: 19, declared_target: 'gfx942:xnack-',
    declared_wave_width: 64, profile: 'closed_u32_program_e32_v1',
    register_plan: { scratch: 32, output: 33, inputs: [34, 35, 36], vgpr_high_water: 37 },
    declared_program: { count: count + 1, descriptors: [8, ...Array(count).fill(201), ...Array(15 - count).fill(0)] },
    declared_instruction_steps: [{ instruction: 'v_mov_b32_e32', output: 33, inputs: [34] },
      ...Array.from({ length: count }, () => ({ instruction: 'v_add_u32_e32', output: 33, inputs: [33, 35] }))],
    declared_source_ids: { frontend_unit: id(500), function: id(501), contract: id(502), statement: id(600 + count) },
    memory_effect: 'NoMemory', ordered_region_effect: true, pure_or_movable: false,
    logical_observation_granularity: 'whole_program_before_after', source_authentication: false,
    source_map_available: false, physical_register_values_available: false, instruction_microsteps_available: false,
    register_lifetime_or_final_allocation_proof: false, proof_authority: false, artifact_authority: false,
    production_resume_authority: false, hardware_execution: false, cpu_preflight_passed: true,
    inspection_counts: { blocks: 6, operations: 26, ssa_definitions: 40, capability_entries: 5, name_bytes: 110 },
    inspection_max_canonical_bytes_after_admission: 65536, cpu_preflight_resident_limit_bytes: 67108864,
    output_buffer_bytes: 8192,
    accounting_scope: 'shared canonical admission, CPU resident accounting, and this borrowed structural/output bound are separate; not a combined allocator or RSS cap' };
}
// Independent synthetic data constructor uses iterative wrapping addition, not
// the production acceptance oracle's BigInt multiplication.
function syntheticWord(count, [a, b]) { let out = a; for (let i = 0; i < count; i++) out = (out + b) >>> 0; return out; }
function simulation(count, inputs, elements) {
  const query = request(inputs, elements), e = exported(count), bytes = Buffer.alloc(8 + 4 * elements, 0xa5);
  const initialized = Buffer.alloc(Math.ceil(bytes.length / 8)), word = syntheticWord(count, inputs);
  for (let i = 0; i < elements; i++) {
    bytes.writeUInt32LE(word, 4 * (i + 1));
    for (let j = 0; j < 4; j++) { const offset = 4 * (i + 1) + j; initialized[offset >> 3] |= 1 << (offset % 8); }
  }
  return { schema: 'fe2o3-simulation-result-v1', status: 'ok', authority: 'observation_only',
    simulated: true, hardware_observed: false, hardware_validation: false, performance_prediction: false,
    kir: { sha256: e.canonical_sha256, canonical_bytes: e.canonical_bytes },
    target_profile: { identity: 'amdgpu_64_little_endian_v1', index_bits: 64, max_workgroup_invocations: 1024 },
    counts: { arguments: 4, shared_buffers: 1, invocations_executed: query.grid[0],
      workgroups_visited: query.grid[0] / 64, scheduled_slots_visited: query.grid[0],
      steps_executed: query.grid[0] * 12, events_emitted: 0 },
    schedule: { identity: 'workgroup_major_local_zyx_cooperative_v1',
      coverage: { decisions: query.grid[0], workgroups: query.grid[0] / 64,
        barrier_releases: 0, complete: true }, transcript_sha256: id(700) },
    conflict_assessment: { status: 'no_conflicts_observed' },
    arguments: query.arguments, shared_buffers: [{ id: 1, buffer: { element: 'u32', access: 'read_write', alignment: 4,
      bytes: '0x' + bytes.toString('hex'), initialized: '0x' + initialized.toString('hex') } }] };
}
function matrix() {
  return LABELS.map((label, index) => {
    const count = ADDITIONS[index], simulations = [];
    for (let input = 0; input < INPUTS.length; input++) for (const elements of LENGTHS) for (let replay = 0; replay < 2; replay++) {
      simulations.push({ input, elements, replay, expected_word: syntheticWord(count, INPUTS[input]),
        output_words: elements, backing_bytes: 8 + 4 * elements, guard_bytes: 8 });
    }
    return { label, selected_condition: SELECTIONS[index], selected_additions: count,
      source_path: '/fresh/' + LABELS[Math.min(index, 2)] + '-source/src/lib.rs',
      kir_path: '/fresh/' + label + '.kir',
      source_sha256: sha256(sources()[Math.min(index, 2)]), kir_file_sha256: id(800 + count),
      exported: exported(count), inspection: inspection(count), simulations };
  });
}
const SOURCE = '/fresh/refuse-source/src/lib.rs';
function refusal(label) {
  const expected = REFUSALS.find(item => item.label === label); assert.ok(expected);
  const records = [];
  for (let index = 0; index < expected.primary_error_count; index++) records.push({ reason: 'compiler-message',
    package_id: 'path+file:///fresh/refuse-source#fe2o3-assembly-authoring-v30-fixture@0.0.0',
    manifest_path: '/fresh/refuse-source/Cargo.toml',
    target: { name: 'fe2o3_assembly_authoring_v30_fixture', src_path: SOURCE, kind: ['lib'] },
    message: { level: 'error', code: expected.compiler_code === null ? null : { code: expected.compiler_code, explanation: null },
      message: expected.kind === 'const' ? 'evaluation panicked: ' + expected.message : expected.message,
      spans: [{ is_primary: true, file_name: expected.kind === 'const' ? '/repo/device/ordered_program_select_v1.rs' : SOURCE }],
      children: [], rendered: 'inert display only' } });
  records.push({ reason: 'build-finished', success: false });
  return { code: 1, signal: null, reason: null, stdout: Buffer.from(records.map(row => JSON.stringify(row)).join('\n') + '\n'),
    stderr: Buffer.from((expected.kind === 'owner' ? expected.message + '\n' : '') +
      'error: could not compile ' + "'fe2o3-assembly-authoring-v30-fixture' (lib)" + '\n' +
      'fe2o3-export-sim: Cargo extraction failed with exit status: 101\n') };
}
function editRecords(result, change) {
  const records = result.stdout.toString().trimEnd().split('\n').map(JSON.parse); change(records);
  return { ...result, stdout: Buffer.from(records.map(row => JSON.stringify(row)).join('\n') + '\n') };
}

test('literal and named concrete bool sources are pinned and differ only in their reviewed spelling', () => {
  validateSources(sources()); assert.equal(ONE.length, 799);
  for (const changed of [Buffer.concat([ONE, Buffer.from('\n')]), Buffer.from(ONE.toString().replace('*element = result;', '*element = a;'))]) {
    assert.throws(() => validateSources([changed, ...sources().slice(1)]));
  }
  assert.throws(() => validateSources(sources().reverse()));
});
test('all eight refusal bodies are exact complete registered source fixtures, including inactive arms', () => {
  assert.equal(NEGATIVES.size, 8);
  for (const row of REFUSALS) {
    const source = NEGATIVES.get(row.label); validateNegativeSource(source, row.label);
    assert.ok(source.includes(Buffer.from('#[kernel(typed, launch(')));
    assert.ok(source.includes(Buffer.from('*element = result;')));
    assert.notEqual(sha256(source), sha256(ONE));
    assert.throws(() => validateNegativeSource(Buffer.concat([source, Buffer.from(' ')]), row.label));
    assert.throws(() => validateNegativeSource(ONE, row.label));
  }
  assert.throws(() => validateNegativeSource(ONE, 'unknown'));
  for (const label of ['nested', 'unknown-opcode', 'inactive-undefined', 'inactive-seventeen'])
    assert.ok(NEGATIVES.get(label).includes(Buffer.from('const_if(true)')));
  const inactive = NEGATIVES.get('inactive-seventeen').toString().split('} else {')[1].split('    };')[0];
  assert.equal((inactive.match(/mov\(out, input0\);/gu) ?? []).length, 1);
  assert.equal((inactive.match(/add\(out, out, input1\);/gu) ?? []).length, 16);
});
test('condition selection is concrete bool only; both source kinds select the expected finite profile', () => {
  assert.equal(selectedAdditions(true), 1); assert.equal(selectedAdditions(false), 2);
  for (const bad of [0, 1, 'true', null, undefined, {}, []]) assert.throws(() => selectedAdditions(bad));
  assert.deepEqual(SELECTIONS.map(selectedAdditions), ADDITIONS);
  assert.deepEqual(SOURCE_FIXTURES.map(row => row.bytes), [799, 834, 834]);
});
test('independent mathematical oracle covers wraparound and ignores the third unused scalar', () => {
  for (const [count, want] of [[1, 0], [2, 1]]) assert.equal(oracle(count, [0xffffffff, 1, 123]), want);
  for (const [count, want] of [[1, 42], [2, 65]]) assert.equal(oracle(count, [19, 23, 0]), want);
  for (const count of [1, 2]) for (const inputs of INPUTS) assert.equal(oracle(count, inputs), syntheticWord(count, inputs));
  assert.equal(oracle(2, [19, 23, 0]), oracle(2, [19, 23, 0xffffffff]));
  for (const bad of [0, 3, 15, 16, -1, 1.5, '2']) assert.throws(() => oracle(bad, INPUTS[0]));
  assert.throws(() => oracle(1, [0, 0x100000000, 0]));
});
test('selected descriptor and step profiles retain exact order and zero padding', () => {
  assert.deepEqual(declaredProgram(1), { count: 2, descriptors: [8, 201, ...Array(14).fill(0)] });
  assert.equal(declaredProgram(2).descriptors.length, 16); assert.deepEqual(declaredProgram(2).descriptors.slice(3), Array(13).fill(0));
  assert.throws(() => declaredProgram(15));
  assert.deepEqual(declaredSteps(2), [{ instruction: 'v_mov_b32_e32', output: 33, inputs: [34] },
    { instruction: 'v_add_u32_e32', output: 33, inputs: [33, 35] }, { instruction: 'v_add_u32_e32', output: 33, inputs: [33, 35] }]);
});
test('normal inspection accepts actual coordinate observations, exact resources, and both selected counts', () => {
  for (const count of [1, 2]) validateInspection(inspection(count), exported(count), count);
});
test('inspection rejects collapsed, reordered, repadded or changed instructions and resource bindings', () => {
  const mutations = [
    value => value.declared_instruction_steps.pop(),
    value => value.declared_instruction_steps.reverse(),
    value => value.declared_instruction_steps[1].instruction = 'v_sub_u32_e32',
    value => value.declared_instruction_steps[1].inputs.reverse(),
    value => value.declared_program.count--,
    value => value.declared_program.descriptors[15] = 201,
    value => value.register_plan.output = 32,
    value => value.register_plan.vgpr_high_water = 36,
    value => value.register_plan.inputs.reverse(),
  ];
  for (const change of mutations) { const value = inspection(2); change(value); assert.throws(() => validateInspection(value, exported(2), 2)); }
});
test('inspection rejects stale owner identity, wrong source reference and manufactured authority', () => {
  const mutations = [
    value => value.canonical.sha256 = id(999), value => value.canonical.bytes++,
    value => value.declared_source_ids.statement = '0'.repeat(64), value => value.declared_source_ids.injected = id(4),
    value => value.input_value_ids[1] = value.input_value_ids[0], value => value.result_value_id = value.input_value_ids[0],
    value => value.coordinate.operation_ordinal = -1, value => value.declared_wave_width = 32,
    value => value.declared_target = 'gfx950', value => value.kernel = 'other',
    value => value.source_authentication = true, value => value.proof_authority = true,
    value => value.physical_register_values_available = true, value => value.pure_or_movable = true,
    value => value.instruction_microsteps_available = true, value => value.extra = false,
  ];
  for (const change of mutations) { const value = inspection(1); change(value); assert.throws(() => validateInspection(value, exported(1), 1)); }
});
test('120 synthetic complete-buffer results meet the exact matrix interface, not a real-run claim', () => {
  let seen = 0;
  for (const count of ADDITIONS) for (const inputs of INPUTS) for (const elements of LENGTHS) for (let replay = 0; replay < 2; replay++) {
    validateSimulation(simulation(count, inputs, elements), request(inputs, elements), exported(count), count, inputs, elements); seen++;
  }
  assert.equal(seen, 120);
});
test('complete data, initialization and both canaries reject one-byte drift and extra backing', () => {
  const inputs = INPUTS[4], count = 2, elements = 65, query = request(inputs, elements);
  for (const offset of [0, 3, 4, 4 + 63 * 4, 4 + 65 * 4, 7 + 65 * 4]) {
    const value = simulation(count, inputs, elements), bytes = Buffer.from(value.shared_buffers[0].buffer.bytes.slice(2), 'hex');
    bytes[offset] ^= 1; value.shared_buffers[0].buffer.bytes = '0x' + bytes.toString('hex');
    assert.throws(() => validateSimulation(value, query, exported(count), count, inputs, elements));
  }
  for (const offset of [0, 4, 263, 267, 271]) {
    const value = simulation(count, inputs, elements), bits = Buffer.from(value.shared_buffers[0].buffer.initialized.slice(2), 'hex');
    bits[offset >> 3] ^= 1 << (offset & 7); value.shared_buffers[0].buffer.initialized = '0x' + bits.toString('hex');
    assert.throws(() => validateSimulation(value, query, exported(count), count, inputs, elements));
  }
  const value = simulation(count, inputs, elements); value.shared_buffers.push(copy(value.shared_buffers[0]));
  assert.throws(() => validateSimulation(value, query, exported(count), count, inputs, elements));
});
test('simulation requires current identities, exact launch/arguments and complete observation-only coverage', () => {
  const mutations = [
    value => value.kir.sha256 = id(7), value => value.arguments[1].bits = '0x00000000',
    value => value.counts.invocations_executed = 64, value => value.counts.workgroups_visited = 1,
    value => value.target_profile.index_bits = 32, value => value.schedule.coverage.complete = false,
    value => value.schedule.transcript_sha256 = 'not-a-digest', value => value.hardware_observed = true,
    value => value.performance_prediction = true, value => value.status = 'error', value => value.simulated = false,
  ];
  for (const change of mutations) { const value = simulation(2, INPUTS[4], 65); change(value);
    assert.throws(() => validateSimulation(value, request(INPUTS[4], 65), exported(2), 2, INPUTS[4], 65)); }
  const query = request(INPUTS[4], 65); query.grid[0] = 64;
  assert.throws(() => validateSimulation(simulation(2, INPUTS[4], 65), query, exported(2), 2, INPUTS[4], 65));
});
test('simulation rejects omitted normal fields and unrequested schema or authority extensions', () => {
  const check = value => validateSimulation(value, request(INPUTS[4], 65), exported(2), 2, INPUTS[4], 65);
  const original = simulation(2, INPUTS[4], 65);
  // --race-evidence is absent from this driver's exact command. No fabricated
  // optional field is needed for the complete ordinary success contract.
  assert.equal(Object.hasOwn(original, 'race_assessment'), false); check(original);
  const required = [
    [[], ['schema', 'status', 'authority', 'simulated', 'hardware_observed', 'hardware_validation',
      'performance_prediction', 'target_profile', 'kir', 'counts', 'schedule', 'conflict_assessment',
      'arguments', 'shared_buffers']],
    [['target_profile'], ['identity', 'index_bits', 'max_workgroup_invocations']],
    [['kir'], ['sha256', 'canonical_bytes']],
    [['counts'], ['arguments', 'shared_buffers', 'invocations_executed', 'workgroups_visited',
      'scheduled_slots_visited', 'steps_executed', 'events_emitted']],
    [['schedule'], ['identity', 'transcript_sha256', 'coverage']],
    [['schedule', 'coverage'], ['decisions', 'workgroups', 'barrier_releases', 'complete']],
    [['conflict_assessment'], ['status']],
  ];
  for (const [route, keys] of required) for (const key of keys) {
    const value = copy(original), target = route.reduce((object, name) => object[name], value);
    delete target[key]; assert.throws(() => check(value), [...route, key].join('.'));
  }
  for (const change of [
    value => value.proof_authority = true,
    value => value.source_authentication = false,
    value => value.race_assessment = { status: 'no_races_observed', first_ordered_conflict: null },
    value => value.target_profile.extra = false,
    value => value.kir.extra = false,
    value => value.counts.extra = 0,
    value => value.schedule.extra = false,
    value => value.schedule.coverage.extra = 0,
    value => value.conflict_assessment.extra = false,
  ]) { const value = copy(original); change(value); assert.throws(() => check(value)); }
});
test('simulation rejects changed schedule, incomplete conflicts and impossible normal counters', () => {
  const check = value => validateSimulation(value, request(INPUTS[4], 65), exported(2), 2, INPUTS[4], 65);
  for (const change of [
    value => value.target_profile.max_workgroup_invocations = 1023,
    value => value.counts.scheduled_slots_visited--,
    value => value.counts.events_emitted = 1,
    value => value.counts.steps_executed = 0,
    value => value.counts.steps_executed = -1,
    value => value.counts.steps_executed = 1.5,
    value => value.counts.steps_executed = (1 << 27) + 1,
    value => value.schedule.identity = 'workgroup_major_local_zyx_serial_v1',
    value => value.schedule.identity = 'workgroup_major_seeded_runnable_cooperative_v1',
    value => value.schedule.coverage.decisions--,
    value => value.schedule.coverage.workgroups--,
    value => value.schedule.coverage.barrier_releases = 1,
    value => value.conflict_assessment = { status: 'conflicts_observed', conflicting_bytes: 4, first: {} },
    value => value.conflict_assessment = { status: 'incomplete', conflicting_bytes: 0,
      record_limit: 0, first: null },
  ]) { const value = simulation(2, INPUTS[4], 65); change(value); assert.throws(() => check(value)); }
  // Step totals are observations, not guessed from the source spelling.
  for (const steps of [1, 1 << 27]) {
    const value = simulation(2, INPUTS[4], 65); value.counts.steps_executed = steps; check(value);
  }
});
test('same selected arm preserves observed identities without inventing equality or inequality', () => {
  validateMatrix(matrix());
  const distinct = matrix();
  for (const field of ['canonical_sha256', 'semantic_identity', 'retained_source_preflight']) distinct[1].exported[field] = id(950);
  distinct[1].kir_file_sha256 = id(951); distinct[1].inspection.canonical.sha256 = id(950);
  distinct[1].inspection.declared_source_ids.statement = id(952); validateMatrix(distinct);
  const sameSelected = matrix(); sameSelected[2].kir_file_sha256 = sameSelected[1].kir_file_sha256;
  assert.throws(() => validateMatrix(sameSelected));
  for (const field of ['source_sha256', 'kir_file_sha256', 'source_path']) {
    const drift = matrix(); drift[3][field] = field === 'source_path' ? '/other/source.rs' : id(999);
    assert.throws(() => validateMatrix(drift));
  }
  for (const field of ['canonical_sha256', 'semantic_identity', 'retained_source_preflight']) {
    const drift = matrix(); drift[3].exported[field] = id(999); assert.throws(() => validateMatrix(drift));
  }
});
test('selected condition, arm count, source order and retained source IDs cannot be relabeled', () => {
  for (const change of [
    value => value[0].selected_condition = false, value => value[1].selected_additions = 2,
    value => value[2].source_sha256 = value[1].source_sha256,
    value => value[3].inspection.declared_source_ids.statement = id(999),
    value => value[0].source_authentication = true,
    value => value[1].source_path = undefined,
    value => value[2].exported.proof_authority = true,
  ]) { const value = matrix(); change(value); assert.throws(() => validateMatrix(value)); }
});
test('root/contract inventory is never treated as a generic body digest; exact repeat remains mandatory', () => {
  validateMatrix(matrix());
  const changedCensus = matrix(); changedCensus[0].exported.retained_source_inventory = id(333);
  validateMatrix(changedCensus);
  const repeatDrift = matrix(); repeatDrift[3].exported.retained_source_inventory = id(334);
  assert.throws(() => validateMatrix(repeatDrift));
});
test('missing, duplicate, relabeled or stale numeric summary rows cannot satisfy complete-matrix joins', () => {
  for (const change of [
    value => value[0].simulations.pop(), value => value[0].simulations.push(copy(value[0].simulations[0])),
    value => value[0].simulations[1].replay = 0, value => value[0].simulations[0].input = 3,
    value => value[0].simulations[2].expected_word ^= 1, value => value[3].label = 'other',
  ]) { const value = matrix(); change(value); assert.throws(() => validateMatrix(value)); }
});
test('eight typed synthetic frontend refusal contracts match exact compiler or existing-owner causes', () => {
  for (const item of REFUSALS) {
    const value = validateRefusal(refusal(item.label), item.label, SOURCE, false);
    assert.equal(value.kir_absent, true); assert.equal(value.kind, item.kind); assert.equal(value.diagnostic, item.message);
    assert.equal(value.compiler_code, item.compiler_code);
    assert.equal(value.primary_error_count, item.primary_error_count);
  }
});
test('refusals require the exact preview roster, including strict one-primary cases', () => {
  assert.deepEqual(REFUSALS.map(row => [row.label, row.primary_error_count]), [
    ['dynamic', 1], ['non-bool', 5], ['nested', 1], ['missing-else', 1],
    ['unknown-opcode', 5], ['inactive-undefined', 5], ['inactive-seventeen', 5],
    ['register-alias', 0],
  ]);
  for (const item of REFUSALS.filter(row => row.kind !== 'owner')) {
    const original = refusal(item.label);
    assert.equal(validateRefusal(original, item.label, SOURCE, false).primary_error_count,
      item.primary_error_count);
    for (const delta of [-1, 1]) {
      const changed = editRecords(original, rows => {
        if (delta === -1) rows.splice(item.primary_error_count - 1, 1);
        else rows.splice(item.primary_error_count, 0, copy(rows[0]));
      });
      assert.throws(() => validateRefusal(changed, item.label, SOURCE, false),
        item.label + ' count ' + (item.primary_error_count + delta));
    }
  }
});
test('every one of five primary records must preserve its exact typed cause and source target', () => {
  const changes = [
    record => record.message.message = 'unrelated compiler cause',
    record => record.message.code = { code: 'E9999', explanation: null },
    record => record.target.name = 'other_crate',
    record => record.target.src_path = '/other/lib.rs',
    record => record.target.kind = ['bin'],
    record => record.manifest_path = '/other/Cargo.toml',
    record => record.message.spans = [],
    record => record.message.spans[0].is_primary = false,
    record => delete record.message.spans,
  ];
  for (const item of REFUSALS.filter(row => row.primary_error_count === 5)) {
    for (let index = 0; index < 5; index++) for (const change of changes) {
      const changed = editRecords(refusal(item.label), rows => change(rows[index]));
      assert.throws(() => validateRefusal(changed, item.label, SOURCE, false),
        item.label + ' primary slot ' + index);
    }
  }
});
test('timeout, stream cap, signal, wrong exit and any published leaf never count as a refusal', () => {
  for (const reason of ['timeout', 'stdout_cap', 'stderr_cap', 'spawn:ENOENT', 'resource_guard', 'stdin:EPIPE']) {
    const value = refusal('inactive-undefined'); value.reason = reason; assert.throws(() => validateRefusal(value, 'inactive-undefined', SOURCE, false));
  }
  for (const code of [0, 101, null]) { const value = refusal('inactive-undefined'); value.code = code; assert.throws(() => validateRefusal(value, 'inactive-undefined', SOURCE, false)); }
  const signaled = refusal('inactive-undefined'); signaled.signal = 'SIGKILL'; assert.throws(() => validateRefusal(signaled, 'inactive-undefined', SOURCE, false));
  assert.throws(() => validateRefusal(refusal('inactive-undefined'), 'inactive-undefined', SOURCE, true));
  const cap = refusal('inactive-undefined'); cap.stdout = Buffer.alloc(LIMITS.command_stream_bytes + 1);
  assert.throws(() => validateTransport(cap, 1));
});
test('missing-entry/setup and unrelated first errors cannot be qualified by diagnostic quotes', () => {
  let value = editRecords(refusal('inactive-undefined'), records => {
    records[0].message.message = 'cannot find crate fe2o3_device'; records[0].message.code.code = 'E0463';
    records[0].message.rendered = 'evaluation panicked: ordered program reads an undefined source';
  });
  assert.throws(() => validateRefusal(value, 'inactive-undefined', SOURCE, false));
  value = editRecords(refusal('inactive-undefined'), records => records.unshift(copy(records[0])));
  assert.throws(() => validateRefusal(value, 'inactive-undefined', SOURCE, false));
  for (const text of ['internal compiler error', 'failed to execute Cargo', 'No space left on device',
    'fe2o3 diagnostic extraction: success', 'fe2o3 diagnostic source identities: fake']) {
    value = refusal('inactive-undefined'); value.stderr = Buffer.concat([Buffer.from(text + '\n'), value.stderr]);
    assert.throws(() => validateRefusal(value, 'inactive-undefined', SOURCE, false));
  }
});
test('refusal binds exact generated target and manifest, typed diagnostic and terminal failed build', () => {
  const mutations = [
    rows => rows[0].manifest_path = '/other/Cargo.toml', rows => rows[0].target.src_path = '/other/lib.rs',
    rows => rows[0].target.name = 'other_crate', rows => rows[0].message.code.code = 'E0308',
    rows => rows[0].message.message = 'ordered program reads an undefined source',
    rows => rows[0].message.spans[0].is_primary = false, rows => rows.at(-1).success = true,
    rows => rows.pop(), rows => rows.push(copy(rows.at(-1))),
  ];
  for (const change of mutations) assert.throws(() => validateRefusal(editRecords(refusal('inactive-undefined'), change), 'inactive-undefined', SOURCE, false));
  const wrong = refusal('inactive-undefined'); wrong.stderr = Buffer.from(wrong.stderr.toString().replace('exit status: 101', 'signal: 9'));
  assert.throws(() => validateRefusal(wrong, 'inactive-undefined', SOURCE, false));
});
test('physical alias refusal requires the actual first owner error with no prior Rust errors', () => {
  const valid = refusal('register-alias'); validateRefusal(valid, 'register-alias', SOURCE, false);
  const wrong = refusal('register-alias'); wrong.stderr = Buffer.concat([Buffer.from('error: missing crate\n'), wrong.stderr]);
  assert.throws(() => validateRefusal(wrong, 'register-alias', SOURCE, false));
  const quoted = refusal('register-alias'); quoted.stderr = Buffer.from(quoted.stderr.toString().replace('fe2o3 rustc extraction:', 'note: quoted'));
  assert.throws(() => validateRefusal(quoted, 'register-alias', SOURCE, false));
  const other = editRecords(refusal('register-alias'), rows => rows.unshift(JSON.parse(refusal('inactive-undefined').stdout.toString().split('\n')[0])));
  assert.throws(() => validateRefusal(other, 'register-alias', SOURCE, false));
});
test('dynamic and non-bool predicates require their exact typed Rust diagnostics, not generic failure text', () => {
  for (const label of ['dynamic', 'non-bool']) {
    validateRefusal(refusal(label), label, SOURCE, false);
    const wrongCode = editRecords(refusal(label), rows => { rows[0].message.code.code = 'E0401'; });
    assert.throws(() => validateRefusal(wrongCode, label, SOURCE, false));
    const wrongCause = editRecords(refusal(label), rows => { rows[0].message.message = 'cannot find value'; });
    assert.throws(() => validateRefusal(wrongCause, label, SOURCE, false));
  }
  const inactive = editRecords(refusal('inactive-seventeen'), rows => { rows[0].message.message = 'evaluation panicked: unrelated panic'; });
  assert.throws(() => validateRefusal(inactive, 'inactive-seventeen', SOURCE, false));
});
test('strict JSON parsing refuses duplicate keys, malformed UTF8 and over-bound collections', () => {
  assert.throws(() => parseJson(Buffer.from('{"reason":"build-finished","reason":"compiler-message"}')));
  assert.throws(() => parseJson(Buffer.from([0xff])));
  assert.throws(() => parseJson(Buffer.alloc(LIMITS.json_bytes + 1, 32)));
});
test('CLI rejects duplicate/unknown options, noncanonical paths and output overlapping selected inputs', () => {
  const argv = ['--repo', '/repo', '--bin-dir', '/bin/tools', '--cargo', '/toolchain/bin/cargo', '--rustc', '/toolchain/bin/rustc', '--output', '/new/run'];
  assert.equal(options(argv).output, '/new/run');
  for (const mutate of [
    value => value[0] = '--other', value => value[2] = '--repo', value => value[1] = '/repo/../repo',
    value => value[9] = '/repo/run', value => value[9] = '/bin', value => value[9] = '/',
  ]) { const value = [...argv]; mutate(value); assert.throws(() => options(value)); }
});
test('bounded stage/pin accounting is explicit; receipt flags are not test authority', () => {
  assert.equal(4 * (1 + 1 + INPUTS.length * LENGTHS.length * 2) + REFUSALS.length, LIMITS.stages);
  assert.equal(LIMITS.stages, 136); assert.equal(REFUSALS.length, 8); assert.equal(LIMITS.pins, 512);
  assert.ok(LIMITS.command_stream_bytes <= 1024 * 1024); assert.equal(LIMITS.cargo_jobs, 2);
  assert.equal(LIMITS.wall_ms, 19 * 60 * 1000);
  assert.ok(LIMITS.wall_ms + 60000 <= 20 * 60 * 1000, 'inner deadline leaves outer20min margin');
});
