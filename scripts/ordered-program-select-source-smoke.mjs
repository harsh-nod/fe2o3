#!/usr/bin/env node
// Normal-source acceptance for bounded compile-time ordered selection.
// Importing exposes pure validators only; this never manufactures executable IR.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { parseBoundedJson } from './ordered-program-source-native.mjs';
import { parseExport, replaceOnce } from './source-promotion-instruction-edit-smoke.mjs';
import { requireDiskReserve } from './assembly-region-worker-prototype.mjs';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';

const KiB = 1024, MiB = KiB * KiB;
export const LIMITS = Object.freeze({ source_bytes: 64 * KiB, json_bytes: MiB,
  kir_bytes: 64 * KiB, command_stream_bytes: MiB, export_ms: 300000, query_ms: 60000,
  wall_ms: 1140000, stages: 136, pins: 512, pin_bytes: 2 * 1024 * MiB,
  selected_file_bytes: 512 * MiB, receipt_bytes: MiB, cargo_messages: 256,
  cargo_jobs: 2, min_available_ram_bytes: 64 * 1024 * MiB });
export const INPUTS = Object.freeze([[0, 0xffffffff, 0], [0xffffffff, 1, 42],
  [0xaaaaaaaa, 0x55555555, 0x0f0f0f0f], [0x80000000, 0x80000000, 0xffffffff],
  [19, 23, 42]].map(Object.freeze));
export const LENGTHS = Object.freeze([0, 1, 65]);
export const LABELS = Object.freeze(['literal-true', 'const-true', 'const-false', 'repeat']);
export const SELECTIONS = Object.freeze([true, true, false, false]);
export const ADDITIONS = Object.freeze([1, 1, 2, 2]);
export const FIXTURE = 'crates/fe2o3-device/tests/fixtures/ordered-select-v1';
const TEMPLATE = 'crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30';
const CRATE = 'fe2o3_assembly_authoring_v30_fixture';
const KERNEL = 'ordered_select_u32';
// All eleven source files are exact full registered kernels authored with the
// device macro draft. Refusals are not manufactured KIR or substituted snippets.
export const SOURCE_FIXTURES = Object.freeze([
  {
    "file": "source-literal-true.rs",
    "bytes": 799,
    "sha256": "cccdb4076ed4404b315ea938d1f9c0feeb297414f36e5c4003f36c1ff852ebd6"
  },
  {
    "file": "source-const-true.rs",
    "bytes": 834,
    "sha256": "2a04e96a8f4b6f8e52ef6319545ae6f56b1ae3b985de9a2e2f77a9b605717f1a"
  },
  {
    "file": "source-const-false.rs",
    "bytes": 834,
    "sha256": "d3d882ac1669b114701f693451dab5b81182b360c2dee8c199a474eb58b03df6"
  }
].map(Object.freeze));
export const REFUSALS = Object.freeze([
  {
    "label": "dynamic",
    "primary_error_count": 1,
    "kind": "rust",
    "compiler_code": "E0435",
    "message": "attempt to use a non-constant value in a constant",
    "file": "refuse-dynamic.rs",
    "bytes": 801,
    "sha256": "b0207d3860a01fce94fcfd428513ff2eb96812df8deeb9c5bd951412038288aa"
  },
  {
    "label": "non-bool",
    "primary_error_count": 5,
    "kind": "rust",
    "compiler_code": "E0308",
    "message": "mismatched types",
    "file": "refuse-non-bool.rs",
    "bytes": 800,
    "sha256": "5bd209d21a2ba3464a656940606904767effe5f8bb253f8ce1e45c5fd030784b"
  },
  {
    "label": "nested",
    "primary_error_count": 1,
    "kind": "macro",
    "compiler_code": null,
    "message": "unsupported amdgpu_ordered_program! syntax; use the closed gfx942 u32 program with literal VGPR bindings",
    "file": "refuse-nested.rs",
    "bytes": 832,
    "sha256": "01e8fdba11b2746572713e9950eaeaacffe7d2ae00e82e336e0cf51fd76b25f3"
  },
  {
    "label": "missing-else",
    "primary_error_count": 1,
    "kind": "macro",
    "compiler_code": null,
    "message": "unsupported amdgpu_ordered_program! syntax; use the closed gfx942 u32 program with literal VGPR bindings",
    "file": "refuse-missing-else.rs",
    "bytes": 682,
    "sha256": "593cbfb364b58bf598b916efaa022f71baf294000160e0474f02d318cd06e29b"
  },
  {
    "label": "unknown-opcode",
    "primary_error_count": 5,
    "kind": "macro",
    "compiler_code": null,
    "message": "ordered program binary opcode or arity is not admitted",
    "file": "refuse-unknown-opcode.rs",
    "bytes": 764,
    "sha256": "ef2a3123ad66714b4d6492fae1a652cfbf8ebaeb213410c476848e68ec0796f8"
  },
  {
    "label": "inactive-undefined",
    "primary_error_count": 5,
    "kind": "const",
    "compiler_code": "E0080",
    "message": "ordered program reads an undefined source",
    "file": "refuse-inactive-undefined.rs",
    "bytes": 734,
    "sha256": "e40704524be650eaa15778b3c7f68c4a0c9688c08d527f3b33f0379c06e6102d"
  },
  {
    "label": "inactive-seventeen",
    "primary_error_count": 5,
    "kind": "const",
    "compiler_code": "E0080",
    "message": "ordered program requires 1..16 steps",
    "file": "refuse-inactive-seventeen.rs",
    "bytes": 1289,
    "sha256": "31af8d48243315a0f10359e24fce04cbdb85639ae2e1aa4e0630ba7eba6401d6"
  },
  {
    "label": "register-alias",
    "primary_error_count": 0,
    "kind": "owner",
    "compiler_code": null,
    "message": "fe2o3 rustc extraction: ordered-program source stages: production compilation semantic importer rejected semantic body construction: semantic body construction rejected inconsistent ordered program physical roles must be distinct v0..v63",
    "file": "refuse-register-alias.rs",
    "bytes": 799,
    "sha256": "6ecc77863c102959579fe3e8d709e5334aafc4abc7a701e5fe52dcb0330c9426"
  }
].map(Object.freeze));
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const decode = bytes => new TextDecoder('utf-8', { fatal: true }).decode(bytes);
export const parseJson = bytes => parseBoundedJson(bytes, LIMITS.json_bytes);
function exact(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'closed object fields');
}
function natural(value, maximum, minimum = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= minimum && value <= maximum, 'bounded integer'); return value;
}
function digest(value) { assert.equal(typeof value, 'string'); assert.match(value, /^[0-9a-f]{64}$/u);
  assert.notEqual(value, '0'.repeat(64)); return value; }
function additions(value) { assert.ok([1, 2].includes(value), 'closed selected-arm profile'); return value; }
export function selectedAdditions(condition) { assert.equal(typeof condition, 'boolean'); return condition ? 1 : 2; }
function absolute(value) {
  assert.equal(typeof value, 'string'); assert.ok(path.isAbsolute(value) && path.resolve(value) === value);
  assert.ok(Buffer.byteLength(value) <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value)); return value;
}
function within(parent, child) { const relative = path.relative(parent, child);
  return relative !== '' && !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('../'); }
function validateFixture(source, row) {
  assert.ok(Buffer.isBuffer(source) && source.length > 0 && source.length <= LIMITS.source_bytes);
  decode(source); assert.equal(source.length, row.bytes, 'exact reviewed source length');
  assert.equal(sha256(source), row.sha256, 'exact reviewed whole source fixture');
  return source;
}
export function validateSources(sources) {
  assert.ok(Array.isArray(sources) && sources.length === SOURCE_FIXTURES.length);
  sources.forEach((source, index) => validateFixture(source, SOURCE_FIXTURES[index]));
  assert.deepEqual(sources[1], replaceOnce(replaceOnce(sources[0],
    '#[kernel', 'const SELECT: bool = 3_u32 < 4;\n\n#[kernel'), 'const_if(true)', 'const_if(SELECT)'));
  assert.deepEqual(sources[2], replaceOnce(sources[1], '3_u32 < 4', '4_u32 < 3'));
}
export function validateNegativeSource(source, label) {
  const row = REFUSALS.find(item => item.label === label); assert.ok(row, 'closed refusal fixture');
  return validateFixture(source, row);
}
export function declaredProgram(count) {
  additions(count);
  return { count: count + 1, descriptors: [8, ...Array(count).fill(201), ...Array(15 - count).fill(0)] };
}
export function declaredSteps(count) {
  additions(count);
  return [{ instruction: 'v_mov_b32_e32', output: 33, inputs: [34] },
    ...Array.from({ length: count }, () => ({ instruction: 'v_add_u32_e32', output: 33, inputs: [33, 35] }))];
}
export function validateInspection(value, exported, count) {
  additions(count);
  exact(value, ['kind', 'authority', 'canonical', 'kernel', 'function', 'coordinate', 'raw_block_id',
    'input_value_ids', 'result_value_id', 'declared_target', 'declared_wave_width', 'profile', 'register_plan',
    'declared_program', 'declared_instruction_steps', 'declared_source_ids', 'memory_effect', 'ordered_region_effect',
    'pure_or_movable', 'logical_observation_granularity', 'source_authentication', 'source_map_available',
    'physical_register_values_available', 'instruction_microsteps_available', 'register_lifetime_or_final_allocation_proof',
    'proof_authority', 'artifact_authority', 'production_resume_authority', 'hardware_execution', 'cpu_preflight_passed',
    'inspection_counts', 'inspection_max_canonical_bytes_after_admission', 'cpu_preflight_resident_limit_bytes',
    'output_buffer_bytes', 'accounting_scope']);
  assert.equal(value.kind, 'diagnostic_ordered_program_inspection_example'); assert.equal(value.authority, 'observation_only');
  digest(exported.canonical_sha256); natural(exported.canonical_bytes, LIMITS.kir_bytes, 1);
  assert.deepEqual(value.canonical, { wire_version: 17, sha256: exported.canonical_sha256, bytes: exported.canonical_bytes });
  assert.equal(value.kernel, KERNEL); assert.equal(value.declared_target, 'gfx942:xnack-');
  assert.equal(value.declared_wave_width, 64); assert.equal(value.profile, 'closed_u32_program_e32_v1');
  assert.deepEqual(value.register_plan, { scratch: 32, output: 33, inputs: [34, 35, 36], vgpr_high_water: 37 });
  assert.deepEqual(value.declared_program, declaredProgram(count));
  assert.deepEqual(value.declared_instruction_steps, declaredSteps(count));
  exact(value.declared_source_ids, ['frontend_unit', 'function', 'contract', 'statement']);
  Object.values(value.declared_source_ids).forEach(digest);
  assert.ok(typeof value.function === 'string' && value.function.length > 0 && Buffer.byteLength(value.function) <= 1024);
  exact(value.coordinate, ['function_ordinal', 'block_ordinal', 'operation_ordinal']);
  // These coordinates are observations, never caller-selected assumed ordinals.
  Object.values(value.coordinate).forEach(item => natural(item, 8192));
  natural(value.raw_block_id, 8192); natural(value.result_value_id, 8192);
  assert.ok(Array.isArray(value.input_value_ids) && value.input_value_ids.length === 3);
  value.input_value_ids.forEach(item => natural(item, 8192));
  assert.equal(new Set([...value.input_value_ids, value.result_value_id]).size, 4);
  assert.equal(value.memory_effect, 'NoMemory'); assert.equal(value.ordered_region_effect, true);
  assert.equal(value.pure_or_movable, false); assert.equal(value.cpu_preflight_passed, true);
  assert.equal(value.logical_observation_granularity, 'whole_program_before_after');
  for (const field of ['source_authentication', 'source_map_available', 'physical_register_values_available',
    'instruction_microsteps_available', 'register_lifetime_or_final_allocation_proof', 'proof_authority',
    'artifact_authority', 'production_resume_authority', 'hardware_execution']) assert.equal(value[field], false);
  exact(value.inspection_counts, ['blocks', 'operations', 'ssa_definitions', 'capability_entries', 'name_bytes']);
  for (const [key, cap] of [['blocks', 128], ['operations', 4096], ['ssa_definitions', 8192],
    ['capability_entries', 256], ['name_bytes', 16384]]) natural(value.inspection_counts[key], cap);
  assert.equal(value.inspection_max_canonical_bytes_after_admission, LIMITS.kir_bytes);
  assert.equal(value.cpu_preflight_resident_limit_bytes, 64 * MiB); assert.equal(value.output_buffer_bytes, 8192);
  assert.equal(value.accounting_scope, 'shared canonical admission, CPU resident accounting, and this borrowed structural/output bound are separate; not a combined allocator or RSS cap');
}
export function oracle(count, inputs) {
  additions(count); assert.ok(Array.isArray(inputs) && inputs.length === 3);
  inputs.forEach(value => natural(value, 0xffffffff));
  return Number((BigInt(inputs[0]) + BigInt(count) * BigInt(inputs[1])) & 0xffffffffn);
}
export function request(inputs, elements) {
  assert.ok(Array.isArray(inputs) && inputs.length === 3); inputs.forEach(value => natural(value, 0xffffffff));
  assert.ok(LENGTHS.includes(elements)); const bytes = 8 + 4 * elements;
  return { schema: 'fe2o3-simulation-request-v1', kernel: KERNEL,
    grid: [elements > 64 ? 128 : 64, 1, 1], workgroup: [64, 1, 1],
    arguments: [{ kind: 'buffer_view', backing: 1, element: 'u32', access: 'read_write', alignment: 4, byte_offset: 4, elements },
      ...inputs.map(value => ({ kind: 'scalar', type: 'u32', bits: '0x' + value.toString(16).padStart(8, '0') }))],
    shared_buffers: [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
      bytes: '0x' + 'a5'.repeat(bytes), initialized: '0x' + '00'.repeat(Math.ceil(bytes / 8)) }] };
}
export function validateSimulation(value, query, exported, count, inputs, elements) {
  assert.deepEqual(query, request(inputs, elements), 'complete request identity');
  // Exact no-race-evidence output of linux.rs::write_success. This CLI does not
  // pass --race-evidence; its optional race_assessment is neither required nor
  // accepted here. Unknown fields cannot introduce unreviewed authority claims.
  exact(value, ['schema', 'status', 'authority', 'simulated', 'hardware_observed',
    'hardware_validation', 'performance_prediction', 'target_profile', 'kir',
    'counts', 'schedule', 'conflict_assessment', 'arguments', 'shared_buffers']);
  assert.equal(value.schema, 'fe2o3-simulation-result-v1'); assert.equal(value.status, 'ok');
  assert.equal(value.authority, 'observation_only'); assert.equal(value.simulated, true);
  for (const key of ['hardware_observed', 'hardware_validation', 'performance_prediction']) assert.equal(value[key], false);
  assert.deepEqual(value.kir, { sha256: exported.canonical_sha256, canonical_bytes: exported.canonical_bytes });
  assert.deepEqual(value.arguments, query.arguments);
  assert.deepEqual(value.target_profile, { identity: 'amdgpu_64_little_endian_v1',
    index_bits: 64, max_workgroup_invocations: 1024 });
  exact(value.counts, ['arguments', 'shared_buffers', 'invocations_executed',
    'workgroups_visited', 'scheduled_slots_visited', 'steps_executed', 'events_emitted']);
  for (const [key, expected] of Object.entries({ arguments: 4, shared_buffers: 1,
    invocations_executed: query.grid[0], workgroups_visited: query.grid[0] / 64,
    scheduled_slots_visited: query.grid[0], events_emitted: 0 })) assert.equal(value.counts[key], expected, key);
  // Keep the actual step count, not an invented per-MIR-shape total. The current
  // normal CLI's simulation budget is 2^27 steps; this nonempty launch uses >0.
  natural(value.counts.steps_executed, 1 << 27, 1);
  exact(value.schedule, ['identity', 'transcript_sha256', 'coverage']);
  assert.equal(value.schedule.identity, 'workgroup_major_local_zyx_cooperative_v1');
  digest(value.schedule.transcript_sha256);
  assert.deepEqual(value.schedule.coverage, { decisions: query.grid[0],
    workgroups: query.grid[0] / 64, barrier_releases: 0, complete: true });
  assert.deepEqual(value.conflict_assessment, { status: 'no_conflicts_observed' },
    'complete selected-run conflict observation, not a general race proof');
  const bytes = Buffer.alloc(8 + elements * 4, 0xa5), initialized = Buffer.alloc(Math.ceil(bytes.length / 8));
  const word = oracle(count, inputs);
  for (let index = 0; index < elements; index++) bytes.writeUInt32LE(word, 4 + index * 4);
  for (let offset = 4; offset < 4 + elements * 4; offset++) initialized[offset >> 3] |= 1 << (offset & 7);
  assert.deepEqual(value.shared_buffers, [{ id: 1, buffer: { element: 'u32', access: 'read_write', alignment: 4,
    bytes: '0x' + bytes.toString('hex'), initialized: '0x' + initialized.toString('hex') } }]);
  return { expected_word: word, output_words: elements, backing_bytes: bytes.length, guard_bytes: 8 };
}
export function validateMatrix(variants) {
  assert.ok(Array.isArray(variants) && variants.length === 4);
  assert.deepEqual(variants.map(item => item.label), LABELS);
  for (let index = 0; index < variants.length; index++) {
    const value = variants[index];
    exact(value, ['label', 'selected_condition', 'selected_additions', 'source_path',
      'source_sha256', 'kir_path', 'kir_file_sha256', 'exported', 'inspection', 'simulations']);
    absolute(value.source_path); absolute(value.kir_path); assert.notEqual(value.source_path, value.kir_path);
    exact(value.exported, ['canonical_sha256', 'canonical_bytes', 'semantic_identity',
      'retained_source_preflight', 'retained_source_inventory']);
    assert.equal(value.selected_condition, SELECTIONS[index]);
    assert.equal(value.selected_additions, selectedAdditions(value.selected_condition));
    assert.equal(value.selected_additions, ADDITIONS[index]);
    for (const key of ['source_sha256', 'kir_file_sha256']) digest(value[key]);
    assert.equal(value.source_sha256, SOURCE_FIXTURES[Math.min(index, 2)].sha256);
    for (const key of ['canonical_sha256', 'semantic_identity', 'retained_source_preflight', 'retained_source_inventory']) digest(value.exported[key]);
    validateInspection(value.inspection, value.exported, value.selected_additions);
    const expected = [];
    for (let input = 0; input < INPUTS.length; input++) for (const elements of LENGTHS) for (let replay = 0; replay < 2; replay++) {
      expected.push({ input, elements, replay, expected_word: oracle(value.selected_additions, INPUTS[input]),
        output_words: elements, backing_bytes: 8 + 4 * elements, guard_bytes: 8 });
    }
    assert.deepEqual(value.simulations, expected, 'complete ordered30-case matrix');
  }
  assert.equal(new Set(variants.slice(0, 3).map(item => item.source_sha256)).size, 3);
  // Same selected arm through different source spelling need not have equal or
  // unequal semantic/source identities. Preserve observations without guessing.
  assert.deepEqual(variants[0].inspection.declared_program, variants[1].inspection.declared_program);
  assert.deepEqual(variants[0].inspection.declared_instruction_steps, variants[1].inspection.declared_instruction_steps);
  assert.deepEqual(variants[0].simulations, variants[1].simulations);
  assert.notEqual(variants[1].kir_file_sha256, variants[2].kir_file_sha256, 'different selected program raw KIR');
  assert.notEqual(variants[1].exported.canonical_sha256, variants[2].exported.canonical_sha256, 'different selected program owner identity');
  const original = variants[2], repeat = variants[3];
  assert.equal(original.source_path, repeat.source_path, 'same retained source for fresh repeat export');
  assert.equal(original.source_sha256, repeat.source_sha256); assert.equal(original.kir_file_sha256, repeat.kir_file_sha256);
  assert.deepEqual(original.exported, repeat.exported); assert.deepEqual(original.inspection, repeat.inspection);
  assert.deepEqual(original.simulations, repeat.simulations);
}
export function validateTransport(result, code) {
  assert.equal(result.reason, null, 'healthy command transport'); assert.equal(result.signal, null, 'no signal');
  assert.equal(result.code, code, 'exact child exit');
  for (const field of ['stdout', 'stderr']) assert.ok(Buffer.isBuffer(result[field]) && result[field].length <= LIMITS.command_stream_bytes);
}
export function validateRefusal(result, label, sourceFile, outputExists) {
  const expected = REFUSALS.find(item => item.label === label); assert.ok(expected); absolute(sourceFile);
  validateTransport(result, 1); assert.equal(outputExists, false, 'no published KIR, including dangling leaf');
  const stderr = decode(result.stderr);
  assert.ok(!stderr.includes('fe2o3 diagnostic extraction:') && !stderr.includes('fe2o3 diagnostic source identities:'));
  assert.ok(!/internal compiler error|panicked at|failed to execute|cannot locate|No space left|out of memory/iu.test(stderr),
    'infrastructure/ICE failure is never a qualification');
  const exporterErrors = stderr.split(/\r?\n/u).filter(line => line.startsWith('fe2o3-export-sim:'));
  assert.deepEqual(exporterErrors, ['fe2o3-export-sim: Cargo extraction failed with exit status: 101']);
  const lines = decode(result.stdout).trimEnd().split('\n');
  assert.ok(lines.length > 0 && lines.length <= LIMITS.cargo_messages);
  const records = lines.map(line => parseJson(Buffer.from(line)));
  assert.ok(records.every(item => ['compiler-artifact', 'build-script-executed', 'compiler-message', 'build-finished'].includes(item.reason)));
  assert.deepEqual(records.filter(item => item.reason === 'build-finished'), [{ reason: 'build-finished', success: false }]);
  assert.equal(records.at(-1).reason, 'build-finished', 'one terminal failed Cargo record');
  const errors = records.filter(item => item.reason === 'compiler-message' && item.message?.level === 'error');
  const extractionErrors = stderr.split(/\r?\n/u).filter(line => line.startsWith('fe2o3 rustc extraction:'));
  if (expected.kind === 'owner') {
    assert.equal(errors.length, 0, 'no unrelated Rust error before the source-owner check');
    assert.deepEqual(extractionErrors, [expected.message], 'exact existing physical-role owner refusal');
    const first = stderr.split(/\r?\n/u).find(line => /^(?:error(?:\[|:)|fe2o3 rustc extraction:)/u.test(line));
    assert.equal(first, expected.message, 'first failure is the owner diagnostic');
  } else {
    // The pinned host-only R2 preview observed an exact one- or five-primary
    // roster, not arbitrary cascades. Full normal frontend acceptance remains
    // mandatory: any different count or any unrelated record stops this run.
    assert.equal(errors.length, expected.primary_error_count, 'exact reviewed primary Rust diagnostic count');
    for (const record of errors) {
      const message = record.message;
      assert.equal(record.manifest_path, path.join(path.dirname(path.dirname(sourceFile)), 'Cargo.toml'));
      assert.equal(record.target.name, CRATE); assert.equal(record.target.src_path, sourceFile);
      assert.ok(Array.isArray(record.target.kind) && record.target.kind.includes('lib'));
      if (expected.kind === 'const') {
        assert.equal(message.code?.code, 'E0080');
        assert.equal(message.message, 'evaluation panicked: ' + expected.message);
      } else if (expected.kind === 'rust') {
        assert.equal(message.code?.code, expected.compiler_code);
        assert.equal(message.message, expected.message);
      } else { assert.equal(message.code, null); assert.equal(message.message, expected.message); }
      assert.ok(Array.isArray(message.spans) && message.spans.length > 0);
      // Const panic spans may be inside the actual device helper; Cargo's typed
      // target+manifest above bind every affected generated registered kernel.
      assert.ok(message.spans.some(span => span.is_primary === true));
    }
    assert.deepEqual(extractionErrors, [], 'Rust diagnostics must not hide an independent extraction error');
  }
  return { label, kind: expected.kind, compiler_code: expected.compiler_code, diagnostic: expected.message, source_path: sourceFile,
    primary_error_count: errors.length, cargo_exit: 101, exporter_exit: 1, kir_absent: true };
}
export function options(argv) {
  const keys = ['repo', 'bin-dir', 'cargo', 'rustc', 'output']; assert.equal(argv.length, keys.length * 2); const result = {};
  for (let at = 0; at < argv.length; at += 2) {
    assert.ok(argv[at].startsWith('--')); const key = argv[at].slice(2);
    assert.ok(keys.includes(key) && !Object.hasOwn(result, key)); result[key] = absolute(argv[at + 1]);
  }
  for (const input of [result.repo, result['bin-dir'], result.cargo, result.rustc]) {
    assert.notEqual(input, result.output); assert.ok(!within(input, result.output) && !within(result.output, input));
  }
  return result;
}

// Filesystem and process work is reachable only through this file's CLI.
function observe(file, cap, keep = false, allowEmpty = false) {
  absolute(file); assert.equal(fs.realpathSync(file), file, 'canonical non-symlink input');
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd, { bigint: true });
    assert.ok(before.isFile() && before.size <= BigInt(cap) && (before.size > 0n || allowEmpty));
    const hash = createHash('sha256'), chunks = [], scratch = Buffer.alloc(64 * KiB); let bytes = 0;
    for (;;) {
      const count = fs.readSync(fd, scratch, 0, scratch.length, null); if (!count) break;
      bytes += count; assert.ok(bytes <= cap); hash.update(scratch.subarray(0, count));
      if (keep) chunks.push(Buffer.from(scratch.subarray(0, count)));
    }
    const after = fs.fstatSync(fd, { bigint: true }), named = fs.lstatSync(file, { bigint: true });
    for (const field of ['dev', 'ino', 'mode', 'nlink', 'size', 'mtimeNs', 'ctimeNs']) {
      assert.equal(after[field], before[field]); assert.equal(named[field], before[field]);
    }
    assert.equal(BigInt(bytes), before.size);
    return { pin: { path: file, bytes, sha256: hash.digest('hex'), device: String(before.dev), inode: String(before.ino),
      mode: String(before.mode), nlink: String(before.nlink), mtime_ns: String(before.mtimeNs), ctime_ns: String(before.ctimeNs) },
      bytes: keep ? Buffer.concat(chunks) : undefined };
  } finally { fs.closeSync(fd); }
}
function environment(opt) {
  const env = { ...process.env };
  for (const key of Object.keys(env)) if (key.startsWith('FE2O3_') ||
    (key.startsWith('CARGO_TARGET_') && key.endsWith('_RUSTFLAGS')) ||
    ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
      'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER'].includes(key)) delete env[key];
  return { ...env, CARGO: opt.cargo, RUSTC: opt.rustc, CARGO_INCREMENTAL: '0', CARGO_BUILD_JOBS: '2',
    CARGO_NET_OFFLINE: 'true', CARGO_TERM_COLOR: 'never',
    LD_LIBRARY_PATH: path.join(path.dirname(path.dirname(opt.rustc)), 'lib') + ':' + opt['bin-dir'] };
}
async function run(opt) {
  const output = opt.output, start = Date.now(), pins = new Map(), stages = [], variants = [], refusals = [];
  let pinBytes = 0; const env = environment(opt), tool = name => path.join(opt['bin-dir'], name);
  for (const directory of [opt.repo, opt['bin-dir'], path.dirname(output)]) assert.equal(fs.realpathSync(directory), directory);
  requireDiskReserve(path.dirname(output)); fs.mkdirSync(output, { mode: 0o700 });
  const guard = () => {
    assert.ok(Date.now() - start <= LIMITS.wall_ms, 'cooperative overall deadline'); requireDiskReserve(output);
    const meminfo = fs.readFileSync('/proc/meminfo', 'utf8'); assert.ok(Buffer.byteLength(meminfo) <= 64 * KiB);
    const match = /^MemAvailable:\s+([0-9]+) kB$/mu.exec(meminfo); assert.ok(match);
    assert.ok(BigInt(match[1]) * 1024n >= BigInt(LIMITS.min_available_ram_bytes), '64 GiB available RAM reserve');
  };
  const pin = (file, cap, keep = false, empty = false) => {
    guard(); const value = observe(file, cap, keep, empty), prior = pins.get(file);
    if (prior) assert.deepEqual(value.pin, prior, 'retained file changed');
    else { assert.ok(pins.size < LIMITS.pins); pinBytes += value.pin.bytes;
      assert.ok(pinBytes <= LIMITS.pin_bytes); pins.set(file, value.pin); }
    return value;
  };
  const save = (name, bytes) => {
    assert.ok(Buffer.byteLength(bytes) <= LIMITS.json_bytes);
    const file = path.join(output, name); assert.ok(within(output, file));
    fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 });
    pin(file, Math.max(Buffer.byteLength(bytes), 1), false, Buffer.byteLength(bytes) === 0); return file;
  };
  const saveJson = (name, value) => save(name, Buffer.from(JSON.stringify(value, null, 2) + '\n'));
  const command = async (label, executable, args, expectedCode = 0) => {
    guard(); assert.ok(stages.length < LIMITS.stages);
    assert.ok(args.length <= 32 && args.every(arg => typeof arg === 'string' && Buffer.byteLength(arg) <= 4096));
    pin(executable, LIMITS.selected_file_bytes);
    const result = await runNavigationCommand({ executable, args, cwd: opt.repo, env,
      timeoutMs: label.endsWith('-export') ? LIMITS.export_ms : LIMITS.query_ms,
      outputCap: LIMITS.command_stream_bytes, guard });
    save(label + '.stdout', result.stdout); save(label + '.stderr', result.stderr);
    stages.push({ label, executable, args, code: result.code, signal: result.signal, reason: result.reason,
      elapsed_ms: result.elapsed_ms, stdout_bytes: result.stdout.length, stdout_sha256: sha256(result.stdout),
      stderr_bytes: result.stderr.length, stderr_sha256: sha256(result.stderr) });
    validateTransport(result, expectedCode); pin(executable, LIMITS.selected_file_bytes); return result;
  };
  const exists = file => { try { fs.lstatSync(file); return true; } catch (error) { if (error.code === 'ENOENT') return false; throw error; } };
  try {
    for (const file of [opt.cargo, opt.rustc, process.execPath, tool('librustc_codegen_fe2o3.so'),
      tool('fe2o3-rustc-extract'), tool('fe2o3-export-sim'), tool('fe2o3-program-inspect'), tool('fe2o3-kir-sim')]) pin(file, LIMITS.selected_file_bytes);
    const here = path.dirname(fileURLToPath(import.meta.url));
    assert.equal(here, path.join(opt.repo, 'scripts'), 'driver must belong to the selected source checkout');
    for (const name of [path.basename(fileURLToPath(import.meta.url)), 'authoring-navigation-v1-process.mjs',
      'source-promotion-instruction-edit-smoke.mjs', 'ordered-program-source-native.mjs',
      'ordered-program-worker-prototype.mjs', 'assembly-region-worker-prototype.mjs']) pin(path.join(here, name), LIMITS.json_bytes);
    for (const name of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml', 'crates/fe2o3-device/Cargo.toml',
      'crates/fe2o3-device/src/lib.rs', 'crates/fe2o3-device/src/ordered_program.rs',
      'crates/fe2o3-device/src/ordered_program_repeat_v1.rs',
      'crates/fe2o3-device/src/ordered_program_select_v1.rs', 'crates/fe2o3-device/src/diagnostics.rs']) pin(path.join(opt.repo, name), LIMITS.json_bytes);
    const originals = SOURCE_FIXTURES.map(row =>
      pin(path.join(opt.repo, FIXTURE, row.file), LIMITS.source_bytes, true).bytes);
    validateSources(originals); originals.forEach((bytes, index) => save('original-' + LABELS[index] + '.rs', bytes));
    const negativeOriginals = new Map();
    for (const row of REFUSALS) {
      const bytes = pin(path.join(opt.repo, FIXTURE, row.file), LIMITS.source_bytes, true).bytes;
      validateNegativeSource(bytes, row.label); negativeOriginals.set(row.label, bytes);
      save('original-' + row.file, bytes);
    }
    let manifest = pin(path.join(opt.repo, TEMPLATE, 'Cargo.toml'), LIMITS.source_bytes, true).bytes;
    const lock = pin(path.join(opt.repo, TEMPLATE, 'Cargo.lock'), LIMITS.json_bytes, true).bytes;
    save('original-Cargo.toml', manifest); save('original-Cargo.lock', lock);
    for (const dependency of ['fe2o3-device', 'fe2o3-host']) manifest = replaceOnce(manifest,
      '"../../../../' + dependency + '"', JSON.stringify(path.join(opt.repo, 'crates', dependency)));
    const sourceDirectory = (label, source) => {
      const directory = path.join(output, label + '-source');
      fs.mkdirSync(directory, { mode: 0o700 }); fs.mkdirSync(path.join(directory, 'src'), { mode: 0o700 });
      for (const [name, bytes] of [['Cargo.toml', manifest], ['Cargo.lock', lock], ['src/lib.rs', source]]) save(label + '-source/' + name, bytes);
      return { manifest: path.join(directory, 'Cargo.toml'), source: path.join(directory, 'src/lib.rs') };
    };
    const sources = originals.map((source, index) => sourceDirectory(LABELS[index], source));
    const exportArgs = (label, current, negative = false) => ['--diagnostic-kir-v17', '--crate', CRATE,
      '--output', path.join(output, label + '.kir'), '--target', 'gfx942',
      '--target-dir', path.join(output, label + '-extraction'), '--', '--manifest-path', current.manifest,
      '--lib', '--offline', ...(negative ? ['--message-format=json'] : [])];
    saveJson('matrix.json', { labels: LABELS, selections: SELECTIONS, selected_additions: ADDITIONS, inputs: INPUTS, lengths: LENGTHS,
      replays: 2, successful_exports: 4, intended_refusals: REFUSALS.map(item => item.label), simulations: 120, stages: LIMITS.stages });
    for (let index = 0; index < LABELS.length; index++) {
      const label = LABELS[index], condition = SELECTIONS[index], count = selectedAdditions(condition),
        sourceIndex = Math.min(index, 2), current = sources[sourceIndex];
      const kirPath = path.join(output, label + '.kir'); assert.equal(exists(kirPath), false);
      const produced = await command(label + '-export', tool('fe2o3-export-sim'), exportArgs(label, current));
      const exported = parseExport(produced.stderr, 'required'), kir = pin(kirPath, LIMITS.kir_bytes, true).bytes;
      assert.equal(kir.length, exported.canonical_bytes);
      const inspectionRequest = saveJson(label + '-inspect-request.json', request(INPUTS[0], 1));
      const inspected = await command(label + '-inspect', tool('fe2o3-program-inspect'), [kirPath, inspectionRequest]);
      const inspection = parseJson(inspected.stdout); validateInspection(inspection, exported, count);
      const simulations = [];
      for (let input = 0; input < INPUTS.length; input++) for (const elements of LENGTHS) {
        const query = request(INPUTS[input], elements), name = label + '-case-' + input + '-length-' + elements;
        const requestPath = saveJson(name + '.request.json', query);
        for (let replay = 0; replay < 2; replay++) {
          const captured = await command(name + '-replay-' + replay, tool('fe2o3-kir-sim'),
            ['--diagnostic-kir-v17', kirPath, '--request', requestPath]);
          simulations.push({ input, elements, replay,
            ...validateSimulation(parseJson(captured.stdout), query, exported, count, INPUTS[input], elements) });
        }
      }
      variants.push({ label, selected_condition: condition, selected_additions: count, source_path: current.source, source_sha256: sha256(originals[sourceIndex]),
        kir_path: kirPath, kir_file_sha256: sha256(kir), exported, inspection, simulations });
    }
    validateMatrix(variants);
    for (const expected of REFUSALS) {
      const label = 'refuse-' + expected.label, source = negativeOriginals.get(expected.label);
      validateNegativeSource(source, expected.label);
      const current = sourceDirectory(label, source), kirPath = path.join(output, label + '.kir');
      assert.equal(exists(kirPath), false);
      const captured = await command(label + '-export', tool('fe2o3-export-sim'), exportArgs(label, current, true), 1);
      refusals.push({ ...validateRefusal(captured, expected.label, current.source, exists(kirPath)), source_sha256: sha256(source) });
    }
    assert.equal(stages.length, LIMITS.stages); assert.equal(refusals.length, 8);
    for (const [file, expected] of pins) {
      guard(); assert.deepEqual(observe(file, Math.max(expected.bytes, 1), false, expected.bytes === 0).pin, expected, 'final selected input/output custody');
    }
    for (const row of REFUSALS) assert.equal(exists(path.join(output, 'refuse-' + row.label + '.kir')), false,
      'frontend refusal output must remain absent at final qualification');
    saveJson('receipt.json', { schema: 'task-ordered-select-source-acceptance-v1', status: 'passed',
      authority: 'observation_only', origin: 'fresh_rust_normal_exporter_v17_inspector_and_cpu_simulator',
      successful_exports: 4, exact_frontend_refusals: 8, whole_kernel_simulations: 120, variants, refusals, stages,
      retained_file_pins: [...pins.values()], retained_pin_bytes: pinBytes, limits: LIMITS,
      bundle_identity: 'unavailable_this_normal_export_route_is_raw_diagnostic_kir_v17_not_bundle_v6',
      compiler_identity_source: 'normal_live_exporter_never_inferred_from_preflight_or_file_hash',
      inventory_semantics: 'retained_root_contract_census_not_body_digest_repeat_requires_exact_equality',
      declared_instruction_count: '2_2_3_3_selected_steps_one_ordered_region_whole_region_logical_observation',
      selection_scope: 'literal_or_named_concrete_bool_const; flat_arms_validated_before_selection; no_runtime_or_generic_predicate',
      inactive_arm_refusals: ['nested', 'unknown-opcode', 'inactive-undefined', 'inactive-seventeen'],
      same_arm_source_identity_relation: 'observed_not_assumed_equal_or_unequal',
      runtime_selection_control_flow_added: false, llvm_observed: false, final_native_observed: false,
      runtime_loop_or_schedule_added: false, native_qualified: false, source_authentication: false,
      compiler_closure_attestation: false, protected_proof: false, production_resume: false,
      hardware_observed: false, physical_register_lifetime_proof: false, milestone_completion: false,
      task_resource_accounting: 'external_current_scope_supervisor_and_complete_input_census_required' });
  } catch (error) {
    try {
      const bytes = Buffer.from(JSON.stringify({ status: 'failed', error: String(error).slice(0, 4096), stages,
        completed_variants: variants, completed_refusals: refusals, manufactured_fallback: false,
        after_pin_verification_completed: false, cleanup_or_rollback_claimed: false }, null, 2) + '\n');
      assert.ok(bytes.length <= LIMITS.receipt_bytes);
      fs.writeFileSync(path.join(output, 'failure.json'), bytes, { flag: 'wx', mode: 0o600 });
    } catch (reportError) { process.stderr.write('Failure receipt unavailable: ' + String(reportError).slice(0, 1024) + '\n'); }
    throw error;
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run(options(process.argv.slice(2))).catch(error => { process.stderr.write(String(error).slice(0, 4096) + '\n'); process.exitCode = 1; });
}
