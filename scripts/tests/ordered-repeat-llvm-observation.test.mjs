import test from 'node:test';
import assert from 'node:assert/strict';
import {
  LIMITS, FALSE_FIELDS, sha256, expectedPin, externalIdentity, validateCaptureHeader,
  stageLabels, validateCapture, validateLowererReport, validateLlvm, validateJoinedVariants, healthy, options,
} from '../ordered-repeat-llvm-observation.mjs';
import {
  LIMITS as SOURCE_LIMITS, LABELS, REPETITIONS, INPUTS, LENGTHS, REFUSALS, declaredProgram, declaredSteps, oracle,
} from '../ordered-repeat-source-smoke.mjs';

const id = n => n.toString(16).padStart(64, '0');
const clone = value => structuredClone(value);
const DIR = '/retained/repeat-source';
const inertKir = n => Buffer.from('INERT PURE-TEST BYTES; NEVER AN EXECUTABLE KIR: ' + n);
function variant(n = 2, label = 'two') {
  const kir = inertKir(n), exported = { canonical_sha256: id(100 + n), canonical_bytes: kir.length,
    retained_source_inventory: id(300), retained_source_preflight: id(400 + n), semantic_identity: id(500 + n) };
  const inspection = { kind: 'diagnostic_ordered_program_inspection_example', authority: 'observation_only',
    canonical: { wire_version: 17, sha256: exported.canonical_sha256, bytes: kir.length },
    kernel: 'ordered_repeat_u32', function: 'retained-entry',
    coordinate: { function_ordinal: 0, block_ordinal: 4, operation_ordinal: 2 }, raw_block_id: 41,
    input_value_ids: [7, 8, 12], result_value_id: 19, declared_target: 'gfx942:xnack-',
    declared_wave_width: 64, profile: 'closed_u32_program_e32_v1',
    register_plan: { scratch: 32, output: 33, inputs: [34, 35, 36], vgpr_high_water: 37 },
    declared_program: declaredProgram(n), declared_instruction_steps: declaredSteps(n),
    declared_source_ids: { frontend_unit: id(600), function: id(601), contract: id(602), statement: id(700 + n) },
    memory_effect: 'NoMemory', ordered_region_effect: true, pure_or_movable: false,
    logical_observation_granularity: 'whole_program_before_after', source_authentication: false,
    source_map_available: false, physical_register_values_available: false, instruction_microsteps_available: false,
    register_lifetime_or_final_allocation_proof: false, proof_authority: false, artifact_authority: false,
    production_resume_authority: false, hardware_execution: false, cpu_preflight_passed: true,
    inspection_counts: { blocks: 6, operations: 26, ssa_definitions: 40, capability_entries: 5, name_bytes: 110 },
    inspection_max_canonical_bytes_after_admission: 65536, cpu_preflight_resident_limit_bytes: 67108864,
    output_buffer_bytes: 8192,
    accounting_scope: 'shared canonical admission, CPU resident accounting, and this borrowed structural/output bound are separate; not a combined allocator or RSS cap' };
  const simulations = [];
  for (let input = 0; input < INPUTS.length; input++) for (const elements of LENGTHS) for (let replay = 0; replay < 2; replay++) {
    simulations.push({ input, elements, replay, expected_word: oracle(n, INPUTS[input]),
      output_words: elements, backing_bytes: 8 + 4 * elements, guard_bytes: 8 });
  }
  return { label, repetitions: n, source_path: DIR + '/' + (label === 'repeat' ? 'fifteen' : label) + '-source/src/lib.rs',
    source_sha256: id(800 + n), kir_path: DIR + '/' + label + '.kir', kir_file_sha256: sha256(kir),
    exported, inspection, simulations };
}
// Synthetic emitter text only. It is never assembled, verified, or substituted
// for a real source-owned LLVM file in the executable acceptance path.
function llvm(n = 2) {
  const instructions = ['v_mov_b32_e32 $0, $1'];
  for (let i = 0; i < n; i++) instructions.push('v_add_u32_e32 $0, $0, $2');
  return Buffer.from('target triple = "amdgcn-amd-amdhsa"\n' +
    'define amdgpu_kernel void @ordered_repeat_u32(ptr addrspace(1) %arg0.data, i64 %arg0.len, i32 %arg1, i32 %arg2, i32 %arg3) #0 !reqd_work_group_size !0 {\n' +
    'bb4:\n  ; ordered-program-v17 vgpr-high-water=37 (binding extent; final descriptor unverified)\n' +
    '  %v19 = call i32 asm sideeffect "' + instructions.join('\\0A\\09') +
    '", "=&{v33},{v34},{v35},{v36},~{v32}"(i32 %arg1, i32 %arg2, i32 %arg3)\n' +
    '  store i32 %v19, ptr addrspace(1) %v25, align 4\n  ret void\n}\n' +
    'attributes #0 = { nounwind "amdgpu-flat-work-group-size"="64,64" "target-features"="-wavefrontsize32,+wavefrontsize64,-xnack" "target-cpu"="gfx942" }\n' +
    '!0 = !{i32 64, i32 1, i32 1}\n');
}
function report(n = 2) {
  const v = variant(n), bytes = llvm(n);
  return { kind: 'diagnostic_ordered_program_llvm_observation', authority: 'observation_only',
    canonical_wire_version: 17, canonical_identity: v.exported.canonical_sha256, canonical_bytes: inertKir(n).length,
    input_file_sha256: v.kir_file_sha256, llvm_sha256: sha256(bytes), llvm_bytes: bytes.length,
    program_count: n + 1, descriptors: declaredProgram(n).descriptors, register_plan: [32, 33, 34, 35, 36],
    canonical_retained_storage_bytes: 4000, canonical_work_limit: 1 << 26, canonical_storage_limit: 64 * 1024 * 1024,
    max_input_bytes: 65536, max_published_llvm_bytes: 65536, emitter_text_limit_bytes: 16 * 1024 * 1024,
    canonical_and_emitter_accounting_are_separate: true, ...Object.fromEntries(FALSE_FIELDS.map(key => [key, false])) };
}
function pin(file, bytes = 1, digest = id(999)) {
  return { path: file, bytes, sha256: digest, device: '1', inode: '2', mode: '33152',
    nlink: '1', mtime_ns: '1000000001', ctime_ns: '1000000002' };
}
function capture() {
  const variants = LABELS.map((label, i) => variant(REPETITIONS[i], label));
  const roster = new Map(), add = p => roster.set(p.path, p);
  add(pin('/tools/fe2o3-export-sim'));
  for (const v of variants) {
    add(pin(v.source_path, 20, v.source_sha256));
    add(pin(v.kir_path, v.exported.canonical_bytes, v.kir_file_sha256));
  }
  const stages = stageLabels().map((label, i) => {
    for (const stream of ['stdout', 'stderr']) add(pin(DIR + '/' + label + '.' + stream));
    return { label, executable: '/tools/fe2o3-export-sim', args: [], code: i >= 128 ? 1 : 0,
      signal: null, reason: null, elapsed_ms: 1, stdout_bytes: 1, stdout_sha256: id(999),
      stderr_bytes: 1, stderr_sha256: id(999) };
  });
  const refusals = REFUSALS.map((r, i) => {
    const source_path = DIR + '/refuse-' + r.label + '-source/src/lib.rs';
    add(pin(source_path, 10, id(1000 + i)));
    return { label: r.label, kind: r.kind, diagnostic: r.message, source_path,
      cargo_exit: 101, exporter_exit: 1, kir_absent: true, source_sha256: id(1000 + i) };
  });
  return { schema: 'task-ordered-repeat-source-acceptance-v1', status: 'passed', authority: 'observation_only',
    origin: 'fresh_rust_normal_exporter_v17_inspector_and_cpu_simulator',
    successful_exports: 4, exact_frontend_refusals: 8, whole_kernel_simulations: 120, variants, refusals, stages,
    retained_file_pins: [...roster.values()], retained_pin_bytes: [...roster.values()].reduce((sum, p) => sum + p.bytes, 0),
    limits: { ...SOURCE_LIMITS },
    bundle_identity: 'unavailable_this_normal_export_route_is_raw_diagnostic_kir_v17_not_bundle_v6',
    compiler_identity_source: 'normal_live_exporter_never_inferred_from_preflight_or_file_hash',
    inventory_semantics: 'retained_root_contract_census_not_body_digest_repeat_requires_exact_equality',
    declared_instruction_count: '2_3_16_steps_one_ordered_region_whole_region_logical_observation',
    runtime_loop_or_schedule_added: false, native_qualified: false, source_authentication: false,
    compiler_closure_attestation: false, protected_proof: false, production_resume: false, hardware_observed: false,
    physical_register_lifetime_proof: false, milestone_completion: false,
    task_resource_accounting: 'external_current_scope_supervisor_and_complete_input_census_required' };
}
function joined() {
  return LABELS.map((label, i) => {
    const n = REPETITIONS[i], bytes = llvm(n);
    return { label, repetitions: n, llvm_sha256: sha256(bytes), llvm_bytes: bytes.length,
      report: report(n), observed: validateLlvm(bytes, variant(n, label)) };
  });
}
const argv = () => ['--repo', '/repo', '--receipt', DIR + '/receipt.json', '--receipt-bytes', '100',
  '--receipt-sha256', id(1), '--lowerer', '/tools/lowerer', '--lowerer-bytes', '200',
  '--lowerer-sha256', id(2), '--output', '/new/observation'];

test('all three counts and the repeated maximum accept exact report and emitter syntax', () => {
  for (const n of [1, 2, 15]) {
    validateLowererReport(report(n), variant(n), inertKir(n), llvm(n));
    const observed = validateLlvm(llvm(n), variant(n));
    assert.equal(observed.instruction_count, n + 1);
    assert.equal(observed.result_ssa, '%v19');
  }
  validateJoinedVariants(joined());
});
test('lowerer report requires every genuine field and refuses unknown authority fields', () => {
  for (const key of Object.keys(report())) {
    const value = report(); delete value[key];
    assert.throws(() => validateLowererReport(value, variant(), inertKir(2), llvm()), key);
  }
  for (const key of ['native_qualified', 'hardware_observed', 'fabricated']) {
    const value = report(); value[key] = false;
    assert.throws(() => validateLowererReport(value, variant(), inertKir(2), llvm()));
  }
  for (const key of FALSE_FIELDS) {
    const value = report(); value[key] = true;
    assert.throws(() => validateLowererReport(value, variant(), inertKir(2), llvm()));
  }
});
test('raw and canonical KIR identities are separate exact current-input joins', () => {
  for (const mutate of [
    v => v.canonical_identity = id(777), v => v.input_file_sha256 = id(777),
    v => v.canonical_bytes++, v => v.canonical_wire_version = 16,
  ]) {
    const value = report(); mutate(value);
    assert.throws(() => validateLowererReport(value, variant(), inertKir(2), llvm()));
  }
  const changed = Buffer.from(inertKir(2)); changed[changed.length - 1] ^= 1;
  assert.throws(() => validateLowererReport(report(), variant(), changed, llvm()));
});
test('whole LLVM byte identity refuses stale reports, equal-length edits and length drift', () => {
  const changed = Buffer.from(llvm()); changed[0] ^= 1;
  assert.throws(() => validateLowererReport(report(), variant(), inertKir(2), changed));
  for (const mutate of [r => r.llvm_sha256 = id(7), r => r.llvm_bytes++]) {
    const r = report(); mutate(r); assert.throws(() => validateLowererReport(r, variant(), inertKir(2), llvm()));
  }
});
test('report rejects collapsed or reordered descriptors, wrong count and register bindings', () => {
  for (const mutate of [
    r => r.program_count--, r => r.descriptors.pop(), r => r.descriptors.reverse(),
    r => r.descriptors[2] = 193, r => r.descriptors[15] = 201,
    r => r.register_plan[0] = 4, r => r.register_plan[1] = 32,
    r => r.register_plan.reverse(), r => r.register_plan.push(37),
  ]) {
    const r = report(); mutate(r); assert.throws(() => validateLowererReport(r, variant(), inertKir(2), llvm()));
  }
});
test('report accounting is bounded and explicitly separate, never aggregate proof', () => {
  for (const mutate of [
    r => r.canonical_retained_storage_bytes = 0,
    r => r.canonical_retained_storage_bytes = 64 * 1024 * 1024 + 1,
    r => r.canonical_work_limit--, r => r.canonical_storage_limit--,
    r => r.max_input_bytes++, r => r.max_published_llvm_bytes++,
    r => r.emitter_text_limit_bytes--, r => r.canonical_and_emitter_accounting_are_separate = false,
  ]) {
    const r = report(); mutate(r); assert.throws(() => validateLowererReport(r, variant(), inertKir(2), llvm()));
  }
});
test('text refuses missing/extra/reordered instructions and changed opcode or operand roles', () => {
  const source = llvm().toString();
  for (const changed of [
    source.replace('\\0A\\09v_add_u32_e32 $0, $0, $2', ''),
    source.replace('v_mov_b32_e32 $0, $1', 'v_add_u32_e32 $0, $0, $2'),
    source.replace('v_mov_b32_e32 $0, $1\\0A\\09v_add_u32_e32 $0, $0, $2',
      'v_add_u32_e32 $0, $0, $2\\0A\\09v_mov_b32_e32 $0, $1'),
    source.replace('v_add_u32_e32', 'v_sub_u32_e32'),
    source.replace('v_add_u32_e32', 'v_add_u32_e64'),
    source.replace('$0, $0, $2', '$0, $2, $0'),
    source.replace('$0, $0, $2', '$0, $0, $3'),
    source.replace('v_mov_b32_e32 $0, $1', 'v_mov_b32_e32 $0, $1\\0A\\09v_mov_b32_e32 $0, $1'),
  ]) assert.throws(() => validateLlvm(Buffer.from(changed), variant()));
});
test('text refuses changed register constraints, missing clobber, sideeffect or direct argument order', () => {
  const source = llvm().toString();
  for (const [before, after] of [
    ['=&{v33}', '={v33}'], ['{v34}', '{v35}'], [',~{v32}', ''],
    ['asm sideeffect', 'asm'], ['i32 %arg1, i32 %arg2, i32 %arg3)', 'i32 %arg2, i32 %arg1, i32 %arg3)'],
    ['vgpr-high-water=37', 'vgpr-high-water=36'],
  ]) assert.throws(() => validateLlvm(Buffer.from(source.replace(before, after)), variant()));
});
test('text refuses duplicate/hidden module asm and wrong source result store', () => {
  const source = llvm().toString(), selected = source.split('\n').find(line => line.includes(' asm '));
  for (const changed of [
    source.replace(selected, ''), source.replace(selected, selected + '\n' + selected),
    source + 'module asm "s_nop 0"\n', source.replace('store i32 %v19,', 'store i32 %arg1,'),
    source.replace('store i32 %v19,', 'store i32 %v20,'), source.replace('ptr addrspace(1) %v25', 'ptr addrspace(0) %v25'),
    source.replace('  store i32 %v19, ptr addrspace(1) %v25, align 4\n', ''),
  ]) assert.throws(() => validateLlvm(Buffer.from(changed), variant()));
  const v = variant(); v.inspection.result_value_id++;
  assert.throws(() => validateLlvm(llvm(), v));
});
test('text refuses changed kernel ABI, launch, target and second function', () => {
  const source = llvm().toString();
  for (const changed of [
    source.replace('@ordered_repeat_u32', '@different'), source.replace('i64 %arg0.len', 'i32 %arg0.len'),
    source.replace('amdgcn-amd-amdhsa', 'x86_64-unknown-linux-gnu'),
    source.replace('"gfx942"', '"gfx950"'), source.replace('"-wavefrontsize32,+wavefrontsize64,-xnack"', '"+wavefrontsize32"'),
    source.replace('64,64', '1,1024'), source.replace('!0 = !{i32 64', '!0 = !{i32 32'),
    source + 'define void @hidden() { ret void }\n',
  ]) assert.throws(() => validateLlvm(Buffer.from(changed), variant()));
});
test('LLVM byte parser refuses malformed UTF8, NUL, CR and oversize', () => {
  for (const bytes of [Buffer.from([255]), Buffer.concat([llvm(), Buffer.from([0])]),
    Buffer.from(llvm().toString().replace('\n', '\r\n')), Buffer.alloc(LIMITS.llvm_bytes + 1), Buffer.alloc(0)]) {
    assert.throws(() => validateLlvm(bytes, variant()));
  }
  for (const count of [0, 16, 1000000000, -1, 1.5, '2']) {
    const value = variant(); value.repetitions = count;
    assert.throws(() => validateLlvm(llvm(), value), 'count must refuse before array allocation');
  }
});
test('repeat requires exact whole LLVM and reports, distinct counts require distinct LLVM', () => {
  for (const mutate of [
    v => v.reverse(), v => v.pop(), v => v[3].llvm_sha256 = id(7),
    v => v[3].llvm_bytes++, v => v[3].report.canonical_retained_storage_bytes++,
    v => v[3].observed.result_store_line += ' ', v => v[1].llvm_sha256 = v[0].llvm_sha256,
    v => v[2].observed.instruction_count--,
  ]) { const values = joined(); mutate(values); assert.throws(() => validateJoinedVariants(values)); }
});
test('successful R2 capture exact shape admits complete four-variant ordered roster', () => {
  const value = capture(); validateCaptureHeader(value); const pins = validateCapture(value, DIR);
  assert.equal(pins.size, value.retained_file_pins.length); assert.equal(stageLabels().length, 136);
  assert.equal(value.variants[3].source_path, value.variants[2].source_path);
});
test('capture refuses R1, missing stages, lossy simulation counts and authority additions', () => {
  for (const mutate of [
    v => v.status = 'failed', v => v.limits.wall_ms = 1500000, v => v.stages.pop(),
    v => v.variants.pop(), v => v.refusals.pop(), v => v.whole_kernel_simulations--,
    v => v.variants[0].simulations.pop(), v => v.native_qualified = true,
    v => v.source_authentication = true, v => v.proof_authority = true,
    v => delete v.compiler_identity_source,
  ]) { const value = capture(); mutate(value); assert.throws(() => validateCapture(value, DIR)); }
});
test('capture refuses stale KIR/source pins, missing required leaf and wrong source path', () => {
  for (const mutate of [
    v => v.variants[0].kir_file_sha256 = id(55),
    v => v.variants[0].source_sha256 = id(55),
    v => v.variants[0].source_path = DIR + '/two-source/src/lib.rs',
    v => v.variants[0].kir_path = DIR + '/two.kir',
    v => v.retained_file_pins.find(p => p.path.endsWith('/one.kir')).bytes++,
    v => { v.retained_file_pins = v.retained_file_pins.filter(p => !p.path.endsWith('/one.kir'));
      v.retained_pin_bytes = v.retained_file_pins.reduce((sum, p) => sum + p.bytes, 0); },
  ]) { const value = capture(); mutate(value); assert.throws(() => validateCapture(value, DIR)); }
});
test('capture stage health, order and exact raw-stream custody cannot be substituted', () => {
  for (const mutate of [
    v => v.stages.reverse(), v => v.stages[0].signal = 'SIGKILL',
    v => v.stages[0].reason = 'timeout', v => v.stages[0].code = 1,
    v => v.stages[128].code = 0, v => v.stages[1].stdout_sha256 = id(55),
    v => v.stages[1].stderr_bytes++, v => v.stages[0].extra = true,
  ]) { const value = capture(); mutate(value); assert.throws(() => validateCapture(value, DIR)); }
});
test('capture pins are unique, exact bounded identities with complete sum', () => {
  for (const mutate of [
    v => v.retained_file_pins.push(clone(v.retained_file_pins[0])),
    v => v.retained_pin_bytes++, v => v.retained_file_pins[0].bytes = LIMITS.selected_file_bytes + 1,
    v => v.retained_file_pins[0].path = '/retained/../other',
    v => v.retained_file_pins[0].ctime_ns = '-1',
    v => v.retained_file_pins[0].sha256 = '0'.repeat(64),
  ]) { const value = capture(); mutate(value); assert.throws(() => validateCaptureHeader(value)); }
});
test('external receipt pin rejects byte-length and equal-length byte/hash substitutions', () => {
  const bytes = Buffer.from('retained receipt'); externalIdentity(bytes, bytes.length, sha256(bytes));
  assert.throws(() => externalIdentity(bytes, bytes.length + 1, sha256(bytes)));
  const changed = Buffer.from(bytes); changed[0] ^= 1;
  assert.throws(() => externalIdentity(changed, bytes.length, sha256(bytes)));
  assert.throws(() => externalIdentity(bytes, bytes.length, id(77)));
});
test('before/after pin comparison rejects replacement and same-size changed bytes or timestamps', () => {
  const value = pin('/retained/input', 7); expectedPin(value, clone(value));
  for (const key of ['path', 'bytes', 'sha256', 'device', 'inode', 'mode', 'nlink', 'mtime_ns', 'ctime_ns']) {
    const changed = clone(value);
    changed[key] = key === 'path' ? '/retained/other' : key === 'bytes' ? 8 : key === 'sha256' ? id(77) : '9';
    assert.throws(() => expectedPin(changed, value), key);
  }
});
test('fresh transport refuses diagnostics, timeout, signals, nonzero exit and truncated stream', () => {
  const value = { code: 0, signal: null, reason: null, stdout: Buffer.from('{}\n'), stderr: Buffer.alloc(0), elapsed_ms: 1 };
  healthy(value);
  for (const extra of [{ code: 1 }, { signal: 'SIGKILL' }, { reason: 'timeout' }, { reason: 'stdout_cap' },
    { stderr: Buffer.from('warning') }, { stdout: Buffer.alloc(0) },
    { stdout: Buffer.alloc(LIMITS.stream_bytes + 1) }, { elapsed_ms: LIMITS.command_ms + 1 }]) {
    assert.throws(() => healthy({ ...value, ...extra }));
  }
});
test('CLI requires independent explicit receipt/tool hashes and disjoint new output', () => {
  const valid = options(argv()); assert.equal(valid['receipt-bytes'], 100);
  for (const mutate of [
    v => v.pop(), v => v[2] = '--repo', v => v[3] = '/retained/other.json',
    v => v[5] = '0', v => v[7] = 'A'.repeat(64),
    v => v[15] = DIR + '/new', v => v[15] = '/repo/new', v => v[15] = '/tools/lowerer',
    v => v[1] = '/repo/../other', v => v.push('--extra', '1'),
  ]) { const values = argv(); mutate(values); assert.throws(() => options(values)); }
});
test('four commands remain below5min and resource limits are closed', () => {
  assert.equal(LIMITS.calls, 4); assert.equal(LIMITS.command_ms, 60000);
  assert.ok(LIMITS.calls * LIMITS.command_ms < LIMITS.wall_ms);
  assert.ok(LIMITS.wall_ms + 30000 <= 300000); assert.equal(LIMITS.output_bytes, 64 * 1024 * 1024);
  assert.equal(LIMITS.llvm_bytes, 65536); assert.equal(LIMITS.stream_bytes, 4096);
  assert.ok(LIMITS.all_pins >= LIMITS.historical_pins + 32);
});
