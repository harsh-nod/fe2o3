#!/usr/bin/env node
// Normal Bundle V6 source acceptance for the const-u32 scalar helper draft.
// Importing exposes pure controls only. No fabricated KIR, owner or call edge.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { parseBoundedJson } from './ordered-program-source-native.mjs';
import { requireDiskReserve } from './assembly-region-worker-prototype.mjs';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';

const KiB = 1024, MiB = KiB * KiB;
export const LIMITS = Object.freeze({ source_bytes: 64 * KiB, helper_bytes: 4096,
  bundle_bytes: 4 * MiB, json_bytes: MiB, operation_count: 512, page_items: 64,
  stages: 224, pins: 640, pin_bytes: 2 * 1024 * MiB, input_bytes: 512 * MiB,
  command_stream_bytes: MiB, command_ms: 300000, wall_ms: 1200000, cargo_jobs: 2 });
export const INPUTS = Object.freeze([[0, 0], [0xffffffff, 0xffffffff], [0xffffffff, 0],
  [0xaaaaaaaa, 0x55555555], [0xfffffff0, 0x25]].map(Object.freeze));
export const LENGTHS = Object.freeze([0, 1, 65]);
export const LABELS = Object.freeze(['baseline', 'default256', 'edited512', 'repeat', 'two']);
const FIXTURE = 'crates/rustc-codegen-fe2o3/tests/fixtures/ordinary-bitwise-promotion-v1';
const CRATE = 'fe2o3_ordinary_bitwise_promotion_v1_fixture';
const HELPER = 'specialized_or';
const ORIGINAL = '    let result = low | 256;';
const ID_FIELDS = ['bundle_identity', 'canonical_kir_digest', 'semantic_mir_identity',
  'rustc_preflight_plan_receipt_sha256'];
const AUTHORITY = Object.freeze({ observation_only: true, authenticates_compiler_execution: false,
  source_authenticated: false, grants_proof_authority: false, grants_production_resume: false,
  grants_load_or_launch: false });
const SUMMARY_KEYS = ['schema', 'authority', 'bundle_identity', 'bundle_subject_identity',
  'canonical_kir_version', 'canonical_kir_digest', 'canonical_kir_bytes', 'target', 'source_map_identity',
  'semantic_mir_identity', 'rustc_identity_inventory_receipt_sha256', 'rustc_identity_inventory_receipt_bytes',
  'rustc_preflight_plan_receipt_sha256', 'rustc_preflight_plan_receipt_bytes', 'compiler_policy_identity',
  'final_artifact_identity', 'operation_count', 'eliminated_source_span_count', 'capabilities'];
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
export const parseJson = bytes => parseBoundedJson(bytes, LIMITS.json_bytes);
const decode = bytes => new TextDecoder('utf-8', { fatal: true }).decode(bytes);
function exact(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'closed object fields');
}
function natural(value, max, min = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= min && value <= max, 'bounded integer');
  return value;
}
function digest(value) {
  assert.equal(typeof value, 'string'); assert.match(value, /^[0-9a-f]{64}$/u);
  assert.notEqual(value, '0'.repeat(64)); return value;
}
function decimal(value, max) {
  assert.equal(typeof value, 'string'); assert.match(value, /^(?:0|[1-9][0-9]*)$/u);
  return natural(Number(value), max);
}
function absolute(value) {
  assert.equal(typeof value, 'string'); assert.ok(path.isAbsolute(value) && path.resolve(value) === value);
  assert.ok(Buffer.byteLength(value) <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value)); return value;
}
function within(parent, child) {
  const relative = path.relative(parent, child);
  return relative !== '' && !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('../');
}
const authority = value => assert.deepEqual(value, AUTHORITY);
function coordinate(value) {
  exact(value, ['function', 'block', 'operation']);
  Object.values(value).forEach(item => natural(item, 65535));
}
function valueView(value) { exact(value, ['value', 'ty']); natural(value.value, 65535); assert.equal(typeof value.ty, 'string'); }
function sourceReference(value) {
  exact(value, ['frontend_unit', 'function', 'contract', 'statement', 'authority']);
  for (const field of ['frontend_unit', 'function', 'contract', 'statement']) digest(value[field]);
  assert.equal(value.authority, 'inert_references_not_source_authentication');
}
export function validateSummary(value) {
  exact(value, SUMMARY_KEYS); authority(value.authority);
  assert.equal(value.schema, 'fe2o3-multilevel-authoring-observation-v1');
  assert.equal(value.canonical_kir_version, 11); assert.equal(value.target, 'gfx942:xnack-');
  for (const field of [...ID_FIELDS, 'bundle_subject_identity', 'source_map_identity',
    'rustc_identity_inventory_receipt_sha256']) digest(value[field]);
  for (const field of ['canonical_kir_bytes', 'rustc_identity_inventory_receipt_bytes',
    'rustc_preflight_plan_receipt_bytes']) assert.ok(decimal(value[field], LIMITS.bundle_bytes) > 0);
  assert.equal(value.compiler_policy_identity, 'unavailable_in_v6');
  assert.equal(value.final_artifact_identity, 'unavailable_extraction_precedes_final_artifact');
  natural(value.operation_count, LIMITS.operation_count, 1);
  natural(value.eliminated_source_span_count, 65536);
  assert.ok(Array.isArray(value.capabilities) && value.capabilities.length <= 16);
}
export function validateOperation(value) {
  exact(value, ['coordinate', 'function_name', 'kind', 'semantic_detail', 'mnemonic',
    'inline_assembly_source', 'inputs', 'results', 'local_memory_effects', 'complete_local_effect_summary',
    'convergence', 'traps', 'physical_resources', 'source_binding', 'source_spans', 'materialization']);
  coordinate(value.coordinate);
  for (const field of ['function_name', 'kind', 'convergence', 'traps', 'physical_resources',
    'source_binding', 'materialization']) assert.ok(typeof value[field] === 'string' && Buffer.byteLength(value[field]) <= 4096);
  for (const field of ['semantic_detail', 'mnemonic']) assert.ok(value[field] === null ||
    (typeof value[field] === 'string' && Buffer.byteLength(value[field]) <= 4096));
  for (const field of ['inputs', 'results']) {
    assert.ok(Array.isArray(value[field]) && value[field].length <= 256); value[field].forEach(valueView);
  }
  assert.ok(Array.isArray(value.local_memory_effects) && value.local_memory_effects.length <= 256);
  value.local_memory_effects.forEach(item => assert.ok(typeof item === 'string' && Buffer.byteLength(item) <= 4096));
  assert.equal(typeof value.complete_local_effect_summary, 'boolean');
  assert.ok(Array.isArray(value.source_spans) && value.source_spans.length <= 256);
  for (const span of value.source_spans) {
    exact(span, ['file_identity', 'display_path', 'byte_start', 'byte_end', 'line', 'column']);
    digest(span.file_identity);
    assert.ok(typeof span.display_path === 'string' && Buffer.byteLength(span.display_path) <= 4096);
    assert.ok(decimal(span.byte_start, Number.MAX_SAFE_INTEGER) <= decimal(span.byte_end, Number.MAX_SAFE_INTEGER));
    natural(span.line, 0xffffffff); natural(span.column, 0xffffffff);
  }
  if (value.inline_assembly_source !== null) sourceReference(value.inline_assembly_source);
}
export function validatePage(page, summary, start) {
  exact(page, ['authority', 'bundle_identity', 'canonical_kir_digest', 'target', 'start',
    'next_start', 'total_operations', 'operations']); authority(page.authority);
  for (const field of ['bundle_identity', 'canonical_kir_digest', 'target']) assert.equal(page[field], summary[field]);
  assert.equal(page.start, start); assert.equal(page.total_operations, summary.operation_count);
  assert.ok(Array.isArray(page.operations));
  // The normal operation_page call always requests limit64 and returns exactly
  // min(limit, total-start), not an arbitrary short prefix with a valid cursor.
  const expectedCount = Math.min(LIMITS.page_items, summary.operation_count - start);
  assert.ok(expectedCount > 0);
  assert.equal(page.operations.length, expectedCount, 'exact fixed-limit operation page length');
  page.operations.forEach(validateOperation);
  const next = start + page.operations.length;
  assert.equal(page.next_start, next < summary.operation_count ? next : null);
  return next;
}
export function validateRoster(operations, summary) {
  assert.equal(operations.length, summary.operation_count);
  let prior = null; const results = new Set();
  for (const operation of operations) {
    validateOperation(operation); const current = operation.coordinate;
    if (prior) assert.ok(current.function > prior.function ||
      (current.function === prior.function && (current.block > prior.block ||
      (current.block === prior.block && current.operation > prior.operation))), 'strict canonical roster order');
    prior = current;
    for (const result of operation.results) {
      const key = current.function + ':' + result.value;
      assert.ok(!results.has(key), 'unique function-local definition'); results.add(key);
    }
  }
}
export function directConstant(operations, selected) {
  assert.equal(selected.inputs.length, 2); assert.equal(selected.results.length, 1);
  assert.notEqual(selected.inputs[0].value, selected.inputs[1].value);
  [...selected.inputs, ...selected.results].forEach(value => assert.equal(value.ty, 'Scalar(U32)'));
  const found = [];
  for (const input of selected.inputs) {
    const definitions = operations.filter(item => item.coordinate.function === selected.coordinate.function &&
      item.results.some(result => result.value === input.value));
    assert.ok(definitions.length <= 1, 'one exact SSA definition');
    const definition = definitions[0];
    if (!definition || definition.kind !== 'constant') continue;
    assert.equal(definition.results.length, 1); assert.deepEqual(definition.results[0], input);
    assert.equal(definition.inputs.length, 0);
    const match = /^U32\((0|[1-9][0-9]*)\)$/u.exec(definition.semantic_detail);
    assert.ok(match, 'actual u32 literal definition required');
    assert.equal(definition.coordinate.block, selected.coordinate.block, 'same-block direct constant');
    assert.ok(definition.coordinate.operation < selected.coordinate.operation, 'earlier direct constant');
    found.push({ value: input, definition: definition.coordinate, original_value: natural(Number(match[1]), 0xffffffff) });
  }
  assert.equal(found.length, 1, 'exactly one directly observed constant, no alias folding');
  return found[0];
}
export function selectBaseline(summary, operations) {
  validateSummary(summary); validateRoster(operations, summary);
  assert.ok(operations.every(item => item.mnemonic === null && item.inline_assembly_source === null),
    'unchanged ordinary Rust baseline');
  const chosen = operations.filter(item => item.kind === 'binary' && item.semantic_detail === 'BitOr');
  assert.equal(chosen.length, 1, 'unique actual final ordinary BitOr');
  const selected = chosen[0]; assert.ok(selected.source_spans.length > 0);
  const constant = directConstant(operations, selected);
  assert.equal(constant.original_value, 256);
  return { selected, constant, selector: { bundle_identity: summary.bundle_identity,
    canonical_kir_digest: summary.canonical_kir_digest, target: summary.target, operations: [selected.coordinate] } };
}
export function expectedHelper(selected, constant) {
  const runtime = selected.inputs.find(value => value.value !== constant.value.value);
  assert.ok(runtime); const operands = selected.inputs.map(value =>
    value.value === constant.value.value ? 'C0' : 'v' + value.value).join(', ');
  return '// Diagnostic scalar helper draft; fresh source readmission required.\n' +
    '// gfx942:xnack-; physical allocation remains compiler-owned.\n#[inline(never)]\n' +
    'pub fn specialized_or<const C0: u32>(v' + runtime.value + ': u32) -> (u32,) {\n' +
    '    let v' + selected.results[0].value + ': u32 = fe2o3_device::amdgpu_asm!(v_or_b32(' + operands + '));\n' +
    '    (v' + selected.results[0].value + ',)\n}\n';
}
export function validateMaterialization(value, selection) {
  exact(value, ['schema', 'authority', 'region', 'helper_name', 'source', 'runtime_parameters',
    'const_parameter', 'original_call_template', 'status', 'frontend_readmission', 'source_application',
    'semantic_equivalence', 'exact_machine_contract', 'physical_register_bindings']);
  authority(value.authority); const { selected, constant, selector } = selection;
  assert.equal(value.schema, 'fe2o3-const-u32-helper-draft-v1'); assert.equal(value.helper_name, HELPER);
  assert.equal(value.status, 'diagnostic_const_u32_source_draft_only');
  assert.equal(value.frontend_readmission, 'not_performed_requires_fresh_source_compilation');
  assert.equal(value.source_application, 'unavailable_requires_explicit_new_source_and_normal_frontend');
  assert.equal(value.semantic_equivalence, 'unproved'); assert.equal(value.exact_machine_contract, 'unproved');
  assert.equal(value.physical_register_bindings, 'unavailable_compiler_owned_scalar_values');
  const region = value.region;
  exact(region, ['authority', 'selector', 'structural_boundary', 'source_insertion_boundary',
    'live_in', 'live_out', 'operations', 'materialization']); authority(region.authority);
  assert.deepEqual(region.selector, selector); assert.deepEqual(region.operations, [selected]);
  assert.deepEqual(region.live_in, selected.inputs); assert.deepEqual(region.live_out, selected.results);
  assert.equal(region.structural_boundary, 'contiguous_operations_in_one_block_no_terminator_selected');
  assert.equal(region.source_insertion_boundary, 'unavailable_source_application_not_admitted');
  assert.equal(region.materialization, 'diagnostic_rust_draft_available');
  assert.deepEqual(value.const_parameter, { name: 'C0', ...constant });
  const runtime = selected.inputs.find(input => input.value !== constant.value.value);
  assert.deepEqual(value.runtime_parameters, [runtime]);
  assert.equal(value.original_call_template, HELPER + '::<256u32>(v' + runtime.value + ')');
  assert.equal(value.source, expectedHelper(selected, constant));
  assert.ok(Buffer.byteLength(value.source) <= LIMITS.helper_bytes);
}
export function replaceOnce(bytes, before, after) {
  assert.ok(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= LIMITS.source_bytes); decode(bytes);
  const needle = Buffer.from(before), at = bytes.indexOf(needle);
  assert.ok(needle.length > 0 && at >= 0 && bytes.indexOf(needle, at + 1) < 0, 'one exact source replacement');
  const result = Buffer.concat([bytes.subarray(0, at), Buffer.from(after), bytes.subarray(at + needle.length)]);
  assert.ok(result.length <= LIMITS.source_bytes); return result;
}
export function sourceVariant(original, helper, label) {
  assert.ok(['default256', 'edited512', 'two'].includes(label));
  assert.ok(Buffer.byteLength(helper) > 0 && Buffer.byteLength(helper) <= LIMITS.helper_bytes);
  const expression = label === 'two' ? 'specialized_or::<256u32>(low).0 ^ specialized_or::<512u32>(a).0' :
    'specialized_or::<' + (label === 'default256' ? '256' : '512') + 'u32>(low).0';
  const caller = replaceOnce(original, ORIGINAL, '    let result = ' + expression + ';');
  const result = Buffer.concat([caller, Buffer.from('\n' + helper)]);
  assert.ok(result.length <= LIMITS.source_bytes); return result;
}
export function inspectInstances(label, summary, operations) {
  validateSummary(summary); validateRoster(operations, summary);
  if (label === 'baseline') { selectBaseline(summary, operations); return []; }
  assert.ok(LABELS.includes(label));
  const expected = label === 'two' ? [256, 512] : [label === 'default256' ? 256 : 512];
  const assembly = operations.filter(item => item.mnemonic !== null);
  assert.equal(assembly.length, expected.length, 'exact retained scalar ISA occurrence count');
  assert.ok(!operations.some(item => item.kind === 'binary' && item.semantic_detail === 'BitOr'),
    'no hidden ordinary final OR fallback');
  for (const detail of ['BitXor', 'BitAnd']) assert.ok(operations.some(item => item.kind === 'binary' && item.semantic_detail === detail));
  const observed = assembly.map(item => {
    assert.equal(item.kind, 'inline_assembly'); assert.equal(item.mnemonic, 'v_or_b32');
    assert.ok(item.source_spans.length > 0); sourceReference(item.inline_assembly_source);
    const constant = directConstant(operations, item);
    return { coordinate: item.coordinate, typed_constant: constant, source_reference: item.inline_assembly_source };
  });
  assert.deepEqual(observed.map(item => item.typed_constant.original_value).sort((a, b) => a - b), expected);
  assert.equal(new Set(observed.map(item => item.coordinate.function)).size, expected.length,
    'distinct retained function bodies, not two operations in one body');
  assert.equal(new Set(observed.map(item => item.source_reference.function)).size, expected.length,
    'distinct actual compiler-observed function identities');
  // Public Call observations expose operands but not the callee. Do not infer
  // kernel-to-helper edges from function display names or unrelated call presence.
  return observed;
}
export function oracle(label, inputs) {
  assert.ok(LABELS.includes(label)); assert.equal(inputs.length, 2);
  inputs.forEach(value => natural(value, 0xffffffff));
  const [a, b] = inputs.map(BigInt), low = (a ^ b) & 255n;
  const value = label === 'two' ? ((low | 256n) ^ (a | 512n)) :
    low | ((label === 'edited512' || label === 'repeat') ? 512n : 256n);
  return Number(value & 0xffffffffn);
}
export function request(inputs, elements) {
  assert.equal(inputs.length, 2); inputs.forEach(value => natural(value, 0xffffffff));
  assert.ok(LENGTHS.includes(elements)); const size = 8 + 4 * elements;
  return { schema: 'fe2o3-simulation-request-v1', kernel: 'bitwise_chain',
    grid: [elements > 64 ? 128 : 64, 1, 1], workgroup: [64, 1, 1],
    arguments: [{ kind: 'buffer_view', backing: 1, element: 'u32', access: 'read_write',
      alignment: 4, byte_offset: 4, elements }, ...inputs.map(value =>
      ({ kind: 'scalar', type: 'u32', bits: '0x' + value.toString(16).padStart(8, '0') }))],
    shared_buffers: [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
      bytes: '0x' + 'a5'.repeat(size), initialized: '0x' + '00'.repeat(Math.ceil(size / 8)) }] };
}
export function validateSimulation(value, query, summary, label, inputs, elements) {
  assert.deepEqual(query, request(inputs, elements));
  assert.equal(value.schema, 'fe2o3-simulation-result-v1'); assert.equal(value.status, 'ok');
  assert.equal(value.authority, 'observation_only'); assert.equal(value.simulated, true);
  for (const field of ['hardware_observed', 'hardware_validation', 'performance_prediction']) assert.equal(value[field], false);
  assert.equal(value.kir.sha256, summary.canonical_kir_digest);
  assert.equal(value.kir.canonical_bytes, decimal(summary.canonical_kir_bytes, LIMITS.bundle_bytes));
  assert.deepEqual(value.arguments, query.arguments); assert.equal(value.counts.arguments, 3);
  assert.equal(value.counts.shared_buffers, 1); assert.equal(value.counts.invocations_executed, query.grid[0]);
  assert.equal(value.counts.workgroups_visited, query.grid[0] / 64);
  assert.equal(value.target_profile.identity, 'amdgpu_64_little_endian_v1'); assert.equal(value.target_profile.index_bits, 64);
  assert.equal(value.schedule.coverage.complete, true); digest(value.schedule.transcript_sha256);
  const bytes = Buffer.alloc(8 + elements * 4, 0xa5), initialized = Buffer.alloc(Math.ceil(bytes.length / 8));
  const word = oracle(label, inputs);
  for (let index = 0; index < elements; index++) bytes.writeUInt32LE(word, 4 + index * 4);
  for (let offset = 4; offset < 4 + elements * 4; offset++) initialized[offset >> 3] |= 1 << (offset & 7);
  assert.deepEqual(value.shared_buffers, [{ id: 1, buffer: { element: 'u32', access: 'read_write', alignment: 4,
    bytes: '0x' + bytes.toString('hex'), initialized: '0x' + initialized.toString('hex') } }]);
  return { expected_word: word, output_words: elements, backing_bytes: bytes.length, guard_bytes: 8 };
}
export function validateJoins(variants) {
  assert.deepEqual(variants.map(item => item.label), LABELS);
  const [baseline, first, edited, repeat, two] = variants;
  for (const [a, b] of [[baseline, first], [first, edited], [edited, two]]) {
    for (const field of ['source_sha256', 'bundle_file_sha256']) assert.notEqual(a[field], b[field], field);
    for (const field of ID_FIELDS) assert.notEqual(a.summary[field], b.summary[field], field);
  }
  assert.equal(edited.source_sha256, repeat.source_sha256);
  assert.equal(edited.bundle_file_sha256, repeat.bundle_file_sha256);
  assert.deepEqual(edited.summary, repeat.summary); assert.deepEqual(edited.instances, repeat.instances);
  assert.notEqual(first.instances[0].source_reference.function, edited.instances[0].source_reference.function,
    'const specialization changes the actual function instance');
  // Inventory is retained and rechecked as an identity, not relabeled as a body
  // digest. No arbitrary cross-variant inventory inequality is assumed.
}
export function options(argv) {
  const keys = ['repo', 'bin-dir', 'cargo', 'rustc', 'output'];
  assert.equal(argv.length, keys.length * 2); const value = {};
  for (let at = 0; at < argv.length; at += 2) {
    assert.ok(argv[at].startsWith('--')); const key = argv[at].slice(2);
    assert.ok(keys.includes(key) && !Object.hasOwn(value, key)); value[key] = absolute(argv[at + 1]);
  }
  assert.notEqual(value.repo, value.output); assert.ok(!within(value.repo, value.output));
  assert.notEqual(value['bin-dir'], value.output); assert.ok(!within(value['bin-dir'], value.output));
  return value;
}

// All I/O and subprocesses are reachable only through the explicit CLI.
function observe(file, cap, keep = false, allowEmpty = false) {
  absolute(file); assert.equal(fs.realpathSync(file), file, 'canonical non-symlink input');
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd, { bigint: true });
    assert.ok(before.isFile() && before.size <= BigInt(cap) && (before.size > 0n || allowEmpty));
    const chunks = [], hash = createHash('sha256'), scratch = Buffer.alloc(64 * KiB); let bytes = 0;
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
    return { pin: { path: file, bytes, sha256: hash.digest('hex'), device: String(before.dev),
      inode: String(before.ino), mode: String(before.mode), nlink: String(before.nlink),
      mtime_ns: String(before.mtimeNs), ctime_ns: String(before.ctimeNs) },
      bytes: keep ? Buffer.concat(chunks) : undefined };
  } finally { fs.closeSync(fd); }
}
function environment(opt) {
  const env = { ...process.env };
  for (const key of Object.keys(env)) if (key.startsWith('FE2O3_') ||
    (key.startsWith('CARGO_TARGET_') && key.endsWith('_RUSTFLAGS')) ||
    ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER',
      'RUSTC_WORKSPACE_WRAPPER', 'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER'].includes(key)) delete env[key];
  return { ...env, CARGO: opt.cargo, RUSTC: opt.rustc, CARGO_INCREMENTAL: '0', CARGO_BUILD_JOBS: '2',
    CARGO_NET_OFFLINE: 'true', LD_LIBRARY_PATH: path.join(path.dirname(path.dirname(opt.rustc)), 'lib') + ':' + opt['bin-dir'] };
}
async function run(opt) {
  const output = opt.output, start = Date.now(), pins = new Map(), stages = [], variants = [];
  let pinBytes = 0; const env = environment(opt), tool = name => path.join(opt['bin-dir'], name);
  for (const dir of [opt.repo, opt['bin-dir'], path.dirname(output)]) assert.equal(fs.realpathSync(dir), dir);
  requireDiskReserve(path.dirname(output)); fs.mkdirSync(output, { mode: 0o700 });
  const guard = () => {
    assert.ok(Date.now() - start <= LIMITS.wall_ms, 'overall cooperative deadline');
    requireDiskReserve(output);
    const text = fs.readFileSync('/proc/meminfo', 'utf8'); assert.ok(Buffer.byteLength(text) <= 64 * KiB);
    const match = /^MemAvailable:\s+([0-9]+) kB$/mu.exec(text); assert.ok(match);
    assert.ok(BigInt(match[1]) * 1024n >= 64n * 1024n ** 3n, '64 GiB available RAM reserve');
  };
  const pin = (file, cap, keep = false, allowEmpty = false) => {
    guard(); const value = observe(file, cap, keep, allowEmpty), prior = pins.get(file);
    if (prior) assert.deepEqual(value.pin, prior, 'retained file changed');
    else { assert.ok(pins.size < LIMITS.pins); pinBytes += value.pin.bytes;
      assert.ok(pinBytes <= LIMITS.pin_bytes); pins.set(file, value.pin); }
    return value;
  };
  const save = (name, bytes) => {
    const file = path.join(output, name); fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 });
    pin(file, Math.max(Buffer.byteLength(bytes), 1), false, Buffer.byteLength(bytes) === 0);
  };
  const saveJson = (name, value) => {
    const bytes = Buffer.from(JSON.stringify(value, null, 2) + '\n');
    assert.ok(bytes.length <= LIMITS.json_bytes); save(name, bytes);
  };
  const command = async (label, executable, args, input = Buffer.alloc(0), refuse = null) => {
    guard(); assert.ok(stages.length < LIMITS.stages);
    assert.ok(args.length <= 32 && args.every(arg => typeof arg === 'string' && Buffer.byteLength(arg) <= 16384));
    if (!pins.has(executable)) pin(executable, LIMITS.input_bytes);
    const result = await runNavigationCommand({ executable, args, cwd: opt.repo, env, input,
      timeoutMs: label.endsWith('-export') ? LIMITS.command_ms : 60000,
      outputCap: LIMITS.command_stream_bytes, guard });
    save(label + '.stdout', result.stdout); save(label + '.stderr', result.stderr);
    stages.push({ label, executable, args, stdin_bytes: input.length, stdin_sha256: sha256(input),
      code: result.code, signal: result.signal, reason: result.reason, elapsed_ms: result.elapsed_ms,
      stdout_bytes: result.stdout.length, stdout_sha256: sha256(result.stdout),
      stderr_bytes: result.stderr.length, stderr_sha256: sha256(result.stderr) });
    assert.equal(result.reason, null, label); assert.equal(result.signal, null, label);
    assert.equal(result.code, refuse ? 1 : 0, label);
    if (refuse) { assert.equal(result.stdout.length, 0); assert.match(decode(result.stderr), refuse); }
    return result.stdout;
  };
  try {
    for (const file of [opt.cargo, opt.rustc, process.execPath, tool('librustc_codegen_fe2o3.so'),
      tool('fe2o3-rustc-extract'), tool('fe2o3-export-sim'), tool('fe2o3-author'), tool('fe2o3-kir-sim')]) pin(file, LIMITS.input_bytes);
    const here = path.dirname(fileURLToPath(import.meta.url));
    for (const name of [path.basename(fileURLToPath(import.meta.url)), 'authoring-navigation-v1-process.mjs',
      'ordered-program-source-native.mjs', 'ordered-program-worker-prototype.mjs', 'assembly-region-worker-prototype.mjs']) {
      pin(path.join(here, name), LIMITS.json_bytes);
    }
    const fixture = path.join(opt.repo, FIXTURE), original = pin(path.join(fixture, 'src/lib.rs'), LIMITS.source_bytes, true).bytes;
    const manifest = pin(path.join(fixture, 'Cargo.toml'), LIMITS.source_bytes, true).bytes;
    const lock = pin(path.join(fixture, 'Cargo.lock'), LIMITS.json_bytes, true).bytes;
    assert.ok(!decode(original).includes('amdgpu_asm'));
    save('original-source.rs', original); save('original-Cargo.toml', manifest); save('original-Cargo.lock', lock);
    saveJson('matrix.json', { labels: LABELS, inputs: INPUTS, lengths: LENGTHS, replays: 2,
      planned_exports: 5, planned_simulations: 150, expected_refusals: 3 });
    const exportSource = async (label, source, sourceManifest) => {
      const bundlePath = path.join(output, label + '.fe2sim');
      await command(label + '-export', tool('fe2o3-export-sim'), ['--crate', CRATE, '--output', bundlePath,
        '--bundle-version', '6', '--target', 'gfx942', '--target-dir', path.join(output, label + '-extraction'),
        '--', '--manifest-path', sourceManifest, '--lib', '--offline']);
      const bundle = pin(bundlePath, LIMITS.bundle_bytes, true).bytes;
      const summary = parseJson(await command(label + '-inspect', tool('fe2o3-author'), ['inspect'], bundle));
      validateSummary(summary); const operations = [];
      for (let at = 0; at < summary.operation_count;) {
        const page = parseJson(await command(label + '-operations-' + at, tool('fe2o3-author'),
          ['operations', '--bundle-identity', summary.bundle_identity, '--start', String(at), '--limit', '64'], bundle));
        at = validatePage(page, summary, at); operations.push(...page.operations);
      }
      validateRoster(operations, summary); saveJson(label + '-operations.json', operations);
      const instances = inspectInstances(label, summary, operations), simulations = [];
      for (let input = 0; input < INPUTS.length; input++) for (const elements of LENGTHS) {
        const query = request(INPUTS[input], elements), name = label + '-case-' + input + '-length-' + elements;
        saveJson(name + '.request.json', query);
        for (let replay = 0; replay < 2; replay++) {
          const result = parseJson(await command(name + '-replay-' + replay, tool('fe2o3-kir-sim'),
            ['--bundle-v6', bundlePath, '--request', path.join(output, name + '.request.json')]));
          simulations.push({ input, elements, replay, ...validateSimulation(result, query, summary, label, INPUTS[input], elements) });
        }
      }
      const observation = { label, source_sha256: sha256(source), bundle_file_sha256: sha256(bundle),
        bundle_bytes: bundle.length, summary, instances, simulations,
        exact_kernel_to_helper_call_edges: 'unavailable_in_current_public_operation_projection' };
      variants.push(observation); return { bundle, summary, operations, observation };
    };
    const baseline = await exportSource('baseline', original, path.join(fixture, 'Cargo.toml'));
    const selection = selectBaseline(baseline.summary, baseline.operations), encoded = JSON.stringify(selection.selector);
    saveJson('selector.json', selection.selector);
    const generated = parseJson(await command('materialize', tool('fe2o3-author'),
      ['materialize-const-u32', '--selector', encoded, '--helper', HELPER], baseline.bundle));
    validateMaterialization(generated, selection); saveJson('materialized.json', generated); save('generated-helper.rs', generated.source);
    // These two argument-parser refusals occur before stdin is read; send empty
    // stdin intentionally, avoiding an EPIPE being mistaken for CLI rejection.
    await command('refuse-asserted-constant-argv', tool('fe2o3-author'),
      ['materialize-const-u32', '--selector', encoded, '--helper', HELPER, '--const-value', '512'],
      Buffer.alloc(0), /^fe2o3-author: usage: fe2o3-author inspect\n/u);
    await command('refuse-asserted-constant-selector', tool('fe2o3-author'),
      ['materialize-const-u32', '--selector', JSON.stringify({ ...selection.selector, constant_value: 512 }), '--helper', HELPER],
      Buffer.alloc(0), /invalid selector: unknown field .constant_value./u);
    await command('refuse-stale-canonical-selector', tool('fe2o3-author'),
      ['materialize-const-u32', '--selector', JSON.stringify({ ...selection.selector, canonical_kir_digest: '0'.repeat(64) }),
        '--helper', HELPER], baseline.bundle, /stale or malformed exact canonical V11 identity/u);
    let namedManifest = manifest;
    for (const dependency of ['fe2o3-device', 'fe2o3-host']) namedManifest = replaceOnce(namedManifest,
      '"../../../../' + dependency + '"', JSON.stringify(path.join(opt.repo, 'crates', dependency)));
    const sources = new Map();
    for (const label of ['default256', 'edited512', 'two']) {
      const source = sourceVariant(original, generated.source, label), dir = path.join(output, label + '-source');
      fs.mkdirSync(dir, { mode: 0o700 }); fs.mkdirSync(path.join(dir, 'src'), { mode: 0o700 });
      for (const [name, bytes] of [['src/lib.rs', source], ['Cargo.toml', namedManifest], ['Cargo.lock', lock]]) {
        const file = path.join(dir, name); fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 });
        pin(file, LIMITS.json_bytes);
      }
      sources.set(label, { source, manifest: path.join(dir, 'Cargo.toml') });
    }
    for (const label of ['default256', 'edited512', 'repeat', 'two']) {
      const current = sources.get(label === 'repeat' ? 'edited512' : label);
      await exportSource(label, current.source, current.manifest);
    }
    validateJoins(variants);
    for (const [file, expected] of pins) {
      guard(); assert.deepEqual(observe(file, Math.max(expected.bytes, 1), false, expected.bytes === 0).pin, expected);
    }
    saveJson('receipt.json', { schema: 'task-const-u32-source-acceptance-v1', status: 'passed',
      origin: 'current_ordinary_source_normal_bundle_v6_and_author_cli', authority: 'observation_only',
      fixture: FIXTURE, original_source_sha256: sha256(original), selector: selection.selector,
      materialized_helper_sha256: sha256(generated.source), actual_exports: 5,
      whole_kernel_simulations: 150, exact_cli_refusals: 3, variants, stages,
      retained_file_pins: [...pins.values()], retained_pin_bytes: pinBytes,
      distinct_scalar_function_instances: 'observed_typed_u32_constants_and_inert_function_references',
      exact_kernel_to_helper_call_edges: 'unavailable_in_current_public_operation_projection',
      source_authentication: false, compiler_closure_attestation: false, protected_proof: false,
      production_resume: false, hardware_observed: false, native_qualified: false,
      physical_register_or_helper_abi_qualified: false, milestone_completion: false,
      limits: LIMITS, task_resource_accounting: 'external_current_scope_supervisor_required' });
  } catch (error) {
    try {
      const bytes = Buffer.from(JSON.stringify({ status: 'failed', error: String(error).slice(0, 4096),
        completed_variants: variants, stages, manufactured_fallback: false,
        after_pin_verification_completed: false, cleanup_or_rollback_claimed: false,
        source_readmission_may_have_refused: true, hardware_observed: false }, null, 2) + '\n');
      assert.ok(bytes.length <= LIMITS.json_bytes);
      fs.writeFileSync(path.join(output, 'failure.json'), bytes, { flag: 'wx', mode: 0o600 });
    } catch (reportError) { process.stderr.write('Failure receipt unavailable: ' + String(reportError).slice(0, 1024) + '\n'); }
    throw error;
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run(options(process.argv.slice(2))).catch(error => {
    process.stderr.write(String(error).slice(0, 4096) + '\n'); process.exitCode = 1;
  });
}
