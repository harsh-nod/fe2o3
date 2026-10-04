#!/usr/bin/env node
// Closed source-edit qualification: normal public seed, then normal V17 tools.
// No source/IR builder, private fresh callback, native worker or launch is used.
// Importing exposes pure validators only; execution requires the explicit CLI.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { parseBoundedJson } from './ordered-program-source-native.mjs';
import { requireDiskReserve, runBoundedCommand } from './assembly-region-worker-prototype.mjs';

const KiB = 1024, MiB = KiB * KiB;
const MAX_SOURCE = 64 * KiB, MAX_JSON = MiB, MAX_PINS = 384, MAX_PIN_BYTES = 2 * 1024 * MiB;
const NORMAL_PREFIX = 'FE2O3_HEADLESS_NORMAL_PUBLISHED ';
const EXPORT_PREFIX = 'fe2o3 diagnostic extraction:';
const SEMANTIC_PREFIX = 'fe2o3 diagnostic source identities:';
const PACKAGE = 'fe2o3-production-extraction-fixture', CRATE = 'fe2o3_production_extraction_fixture';
const EXPORT_CRATE = 'fe2o3_assembly_authoring_v30_fixture';
const LOADER = '#![no_std]\n#[path = "original.rs"] mod source_bitselect_feasibility;\n';
const EXPORT_LOADER = '#![no_std]\n#[path = "kernel.rs"] mod source_bitselect_feasibility;\n';
const EXPRESSION = 'b ^ ((a ^ b) & mask)';
const LAST_XOR = '    xor(out, input1, scratch);\n';
const LAST_OR = '    or(out, input1, scratch);\n';
const MACRO = 'fe2o3_device::amdgpu_ordered_program! {\n' +
  '    gfx942_xnack_off_wave64;\n    scratch(4); out(5);\n' +
  '    in(0) = a;\n    in(1) = b;\n    in(2) = mask;\n' +
  '    xor(scratch, input0, input1);\n    and(scratch, scratch, input2);\n' + LAST_XOR + '}';
export const INPUTS = Object.freeze([
  [0, 0xffffffff, 0xffffffff], [0xffffffff, 0, 0xffffffff],
  [0xaaaaaaaa, 0x55555555, 0x0f0f0f0f], [0x80000000, 0x80000000, 0],
  [19, 23, 42],
].map(Object.freeze));
export const LENGTHS = Object.freeze([0, 1, 65]);
export const descriptors = variant => [133, 307, profile(variant) === 'default' ? 413 : 412, ...Array(13).fill(0)];
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const decode = bytes => new TextDecoder('utf-8', { fatal: true }).decode(bytes);
const json = bytes => parseBoundedJson(bytes, MAX_JSON);
function exact(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'closed object fields');
}
function natural(value, maximum, minimum = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= minimum && value <= maximum, 'bounded integer');
  return value;
}
function digest(value) {
  assert.equal(typeof value, 'string'); assert.match(value, /^[0-9a-f]{64}$/u);
  assert.notEqual(value, '0'.repeat(64)); return value;
}
function profile(value) { assert.ok(value === 'default' || value === 'edited'); return value; }
function absolute(value) {
  assert.equal(typeof value, 'string'); assert.ok(path.isAbsolute(value) && path.resolve(value) === value);
  assert.ok(Buffer.byteLength(value) <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value)); return value;
}
function within(parent, child) {
  const relative = path.relative(parent, child);
  return relative !== '' && !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('../');
}
export function replaceOnce(bytes, before, after) {
  assert.ok(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_SOURCE);
  decode(bytes);
  const needle = Buffer.from(before), replacement = Buffer.from(after), offset = bytes.indexOf(needle);
  assert.ok(needle.length > 0 && offset >= 0 && bytes.indexOf(needle, offset + 1) < 0, 'one exact byte edit');
  const result = Buffer.concat([bytes.subarray(0, offset), replacement, bytes.subarray(offset + needle.length)]);
  assert.ok(result.length <= MAX_SOURCE); return result;
}
export function verifySourceEdit(original, candidate) {
  // Anchor only the selected binding; other inactive fixture branches remain byte-identical.
  const before = '    let selected = ' + EXPRESSION + ';\n';
  assert.deepEqual(candidate, replaceOnce(original, before, '    let selected = ' + MACRO + ';\n'),
    'public seed changes exactly the original initializer and no surrounding byte');
  const edited = replaceOnce(candidate, LAST_XOR, LAST_OR);
  assert.notEqual(sha256(candidate), sha256(edited));
  return edited;
}
export function parsePublication(stdout, original, candidate) {
  const lines = decode(stdout).split(/\r?\n/u).filter(line => line.startsWith(NORMAL_PREFIX));
  const expected = NORMAL_PREFIX + sha256(original) + ' ' + sha256(candidate) + ' ' + candidate.length;
  assert.deepEqual(lines, [expected], 'exact successful normal public-driver return');
  return { original_sha256: sha256(original), candidate_sha256: sha256(candidate), candidate_bytes: candidate.length };
}
export function validateSeedRecord(record, repo, prepared) {
  absolute(repo); absolute(prepared);
  const basename = path.basename(prepared);
  assert.ok(basename !== '.' && basename !== '..' && /^[A-Za-z0-9_.-]{1,96}$/u.test(basename),
    'exact existing preparation basename contract');
  exact(record, ['case', 'phase', 'source_directory', 'args', 'crate_binding', 'cargo_observation',
    'source_sha256', 'loader_sha256', 'fixture_sha256', 'artifacts_sha256', 'metadata_sha256']);
  assert.equal(record.case, 'positive'); assert.equal(record.phase, 'baseline');
  const source = record.source_directory;
  assert.equal(typeof source, 'string'); assert.ok(source.length > 0 && Buffer.byteLength(source) <= 950);
  assert.ok(!path.isAbsolute(source) && source.split('/').every(part => /^[A-Za-z0-9_.-]+$/u.test(part) && part !== '.' && part !== '..'));
  assert.equal(source, path.join('target/source-bitselect-candidate-roundtrip', basename, 'positive'),
    'source directory must descend from this exact fresh preparation, not another in-repo source');
  const directory = path.join(repo, source); assert.ok(within(repo, directory));
  assert.ok(Array.isArray(record.args) && record.args.length === 32);
  for (const arg of record.args) assert.ok(typeof arg === 'string' && Buffer.byteLength(arg) <= 4096 && !arg.includes('\0'));
  assert.ok(record.args.reduce((n, arg) => n + Buffer.byteLength(arg), 0) <= MiB);
  const args = record.args, sysroot = absolute(args[18]), analysis = path.join(prepared, 'analysis-output');
  const device = args[26].replace(/^fe2o3_device=/u, ''), core = args[28].replace(/^noprelude:core=/u, '');
  assert.ok(args[26].startsWith('fe2o3_device=') && args[28].startsWith('noprelude:core='));
  for (const file of [device, core]) assert.ok(within(path.join(prepared, 'dependencies'), absolute(file)));
  assert.deepEqual(args.slice(0, 26), [
    path.join(sysroot, 'bin/rustc'), '--crate-name', CRATE, path.join(directory, 'original-loader.rs'),
    '--edition=2024', '--crate-type=lib', '--target=amdgcn-amd-amdhsa', '--emit=metadata',
    '-Copt-level=3', '-Cpanic=abort', '-Cembed-bitcode=no', '-Cdebug-assertions=off',
    '-Coverflow-checks=on', '-Ctarget-cpu=gfx942', '-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32',
    '-Zalways-encode-mir', '-Zunstable-options', '--sysroot', sysroot, '--out-dir', analysis,
    '-L', 'dependency=' + path.dirname(device), '-L', 'dependency=' + path.join(prepared, 'dependencies/release/deps'), '--extern',
  ]);
  assert.equal(args[27], '--extern'); assert.deepEqual(args.slice(29, 31), ['--cfg', 'feature="source-bitselect-feasibility"']);
  assert.match(args[31], /^-Cmetadata=[A-Za-z0-9_-]{1,256}$/u);
  for (const key of ['crate_binding', 'cargo_observation', 'source_sha256', 'loader_sha256', 'artifacts_sha256', 'metadata_sha256']) digest(record[key]);
  assert.ok(Array.isArray(record.fixture_sha256) && record.fixture_sha256.length === 4);
  record.fixture_sha256.forEach(digest);
  return { directory, sysroot, analysis, device, core };
}
export function parseExport(stderr, semanticMode) {
  assert.ok(semanticMode === 'required' || semanticMode === 'unavailable');
  const lines = decode(stderr).split(/\r?\n/u), exports = lines.filter(line => line.startsWith(EXPORT_PREFIX));
  assert.equal(exports.length, 1, 'exactly one fresh normal exporter observation');
  const match = /^fe2o3 diagnostic extraction: actual Rust -> admitted semantic MIR V32 -> semantic SSA -> pre_ranked_diagnostic exact KIR V17; declared_target=gfx942:xnack-, declared_wave=64, 1 kernel\(s\), canonical_identity ([0-9a-f]{64}), ([1-9][0-9]{0,5}) byte\(s\), retained_source_inventory ([0-9a-f]{64}), retained_source_preflight ([0-9a-f]{64}); ranked_checks=false, functional_proof=false, exported_source_authentication=false, exported_compiler_authentication=false, protected_admission=false, artifact\/load\/launch\/hardware_authority=false, source_variable_map=unavailable, physical_register_values=unavailable, instruction_microsteps=unavailable; raw bytes are diagnostic input, not production resume$/u.exec(exports[0]);
  assert.ok(match, 'exact existing normal V17 diagnostic contract');
  const result = { canonical_sha256: digest(match[1]), canonical_bytes: natural(Number(match[2]), MAX_SOURCE, 1),
    retained_source_inventory: digest(match[3]), retained_source_preflight: digest(match[4]), semantic_identity: null };
  const semantic = lines.filter(line => line.startsWith(SEMANTIC_PREFIX));
  if (semanticMode === 'required') {
    assert.equal(semantic.length, 1, 'semantic digest must come from the normal live exporter');
    const joined = /^fe2o3 diagnostic source identities: semantic_mir_v32 ([0-9a-f]{64}), canonical_kir_v17 ([0-9a-f]{64}); observation_only=true, exported_source_authentication=false$/u.exec(semantic[0]);
    assert.ok(joined); assert.equal(joined[2], result.canonical_sha256);
    result.semantic_identity = digest(joined[1]);
  } else {
    assert.equal(semantic.length, 0, 'use required mode when the semantic diagnostic exists');
  }
  return result;
}
export function verifyInspector(value, exported, variant) {
  profile(variant);
  exact(value, ['kind', 'authority', 'canonical', 'kernel', 'function', 'coordinate', 'raw_block_id',
    'input_value_ids', 'result_value_id', 'declared_target', 'declared_wave_width', 'profile', 'register_plan',
    'declared_program', 'declared_instruction_steps', 'declared_source_ids', 'memory_effect', 'ordered_region_effect',
    'pure_or_movable', 'logical_observation_granularity', 'source_authentication', 'source_map_available',
    'physical_register_values_available', 'instruction_microsteps_available', 'register_lifetime_or_final_allocation_proof',
    'proof_authority', 'artifact_authority', 'production_resume_authority', 'hardware_execution', 'cpu_preflight_passed',
    'inspection_counts', 'inspection_max_canonical_bytes_after_admission', 'cpu_preflight_resident_limit_bytes',
    'output_buffer_bytes', 'accounting_scope']);
  assert.equal(value.kind, 'diagnostic_ordered_program_inspection_example');
  assert.equal(value.authority, 'observation_only');
  assert.deepEqual(value.canonical, { wire_version: 17, sha256: exported.canonical_sha256, bytes: exported.canonical_bytes });
  assert.equal(value.kernel, 'choose_bits'); assert.equal(value.declared_target, 'gfx942:xnack-');
  assert.equal(value.declared_wave_width, 64); assert.equal(value.profile, 'closed_u32_program_e32_v1');
  assert.deepEqual(value.register_plan, { scratch: 4, output: 5, inputs: [0, 1, 2], vgpr_high_water: 6 });
  assert.deepEqual(value.declared_program, { count: 3, descriptors: descriptors(variant) });
  assert.deepEqual(value.declared_instruction_steps, [
    { instruction: 'v_xor_b32_e32', output: 4, inputs: [0, 1] },
    { instruction: 'v_and_b32_e32', output: 4, inputs: [4, 2] },
    { instruction: variant === 'default' ? 'v_xor_b32_e32' : 'v_or_b32_e32', output: 5, inputs: [1, 4] },
  ]);
  exact(value.declared_source_ids, ['frontend_unit', 'function', 'contract', 'statement']);
  Object.values(value.declared_source_ids).forEach(digest);
  assert.ok(typeof value.function === 'string' && value.function.length > 0 && Buffer.byteLength(value.function) <= 1024);
  exact(value.coordinate, ['function_ordinal', 'block_ordinal', 'operation_ordinal']);
  assert.equal(value.coordinate.function_ordinal, 0);
  for (const item of [value.coordinate.block_ordinal, value.coordinate.operation_ordinal, value.raw_block_id,
    value.result_value_id, ...value.input_value_ids]) natural(item, 8192);
  assert.equal(value.input_value_ids.length, 3); assert.equal(new Set([...value.input_value_ids, value.result_value_id]).size, 4);
  assert.equal(value.memory_effect, 'NoMemory'); assert.equal(value.ordered_region_effect, true);
  assert.equal(value.pure_or_movable, false); assert.equal(value.cpu_preflight_passed, true);
  assert.equal(value.logical_observation_granularity, 'whole_program_before_after');
  for (const field of ['source_authentication', 'source_map_available', 'physical_register_values_available',
    'instruction_microsteps_available', 'register_lifetime_or_final_allocation_proof', 'proof_authority',
    'artifact_authority', 'production_resume_authority', 'hardware_execution']) assert.equal(value[field], false);
  assert.equal(value.output_buffer_bytes, 8192);
  exact(value.inspection_counts, ['blocks', 'operations', 'ssa_definitions', 'capability_entries', 'name_bytes']);
  for (const [field, maximum] of [['blocks', 128], ['operations', 4096], ['ssa_definitions', 8192],
    ['capability_entries', 256], ['name_bytes', 16384]]) natural(value.inspection_counts[field], maximum);
  assert.equal(value.inspection_max_canonical_bytes_after_admission, MAX_SOURCE);
  assert.equal(value.cpu_preflight_resident_limit_bytes, 64 * MiB);
  assert.equal(value.accounting_scope, 'shared canonical admission, CPU resident accounting, and this borrowed structural/output bound are separate; not a combined allocator or RSS cap');
}
export function oracle(variant, inputs) {
  profile(variant); assert.ok(Array.isArray(inputs) && inputs.length === 3);
  inputs.forEach(value => natural(value, 0xffffffff));
  const [a, b, mask] = inputs.map(BigInt), word = 0xffffffffn;
  return Number((variant === 'default' ? (a & mask) | (b & (~mask & word)) : b | (a & mask)) & word);
}
export function request(inputs, elements) {
  inputs.forEach(value => natural(value, 0xffffffff)); assert.equal(inputs.length, 3);
  assert.ok(LENGTHS.includes(elements));
  const bytes = 8 + elements * 4;
  return { schema: 'fe2o3-simulation-request-v1', kernel: 'choose_bits',
    grid: [elements > 64 ? 128 : 64, 1, 1], workgroup: [64, 1, 1],
    arguments: [{ kind: 'buffer_view', backing: 1, element: 'u32', access: 'read_write', alignment: 4, byte_offset: 4, elements },
      ...inputs.map(value => ({ kind: 'scalar', type: 'u32', bits: '0x' + value.toString(16).padStart(8, '0') }))],
    shared_buffers: [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
      bytes: '0x' + 'a5'.repeat(bytes), initialized: '0x' + '00'.repeat(Math.ceil(bytes / 8)) }] };
}
export function verifySimulation(result, query, exported, variant, inputs, elements) {
  assert.deepEqual(query, request(inputs, elements), 'exact immutable simulation request');
  assert.equal(result.schema, 'fe2o3-simulation-result-v1'); assert.equal(result.status, 'ok');
  assert.equal(result.authority, 'observation_only'); assert.equal(result.simulated, true);
  for (const field of ['hardware_observed', 'hardware_validation', 'performance_prediction']) assert.equal(result[field], false);
  assert.equal(result.kir.sha256, exported.canonical_sha256); assert.equal(result.kir.canonical_bytes, exported.canonical_bytes);
  assert.deepEqual(result.arguments, query.arguments);
  assert.equal(result.counts.arguments, 4); assert.equal(result.counts.shared_buffers, 1);
  assert.equal(result.counts.invocations_executed, query.grid[0]);
  assert.equal(result.counts.workgroups_visited, query.grid[0] / 64);
  assert.equal(result.target_profile.identity, 'amdgpu_64_little_endian_v1'); assert.equal(result.target_profile.index_bits, 64);
  const expected = Buffer.alloc(8 + elements * 4, 0xa5), initialized = Buffer.alloc(Math.ceil(expected.length / 8));
  const word = oracle(variant, inputs);
  for (let index = 0; index < elements; index++) {
    expected.writeUInt32LE(word, 4 + index * 4);
    for (let byte = 4 + index * 4; byte < 8 + index * 4; byte++) initialized[byte >> 3] |= 1 << (byte & 7);
  }
  assert.equal(result.shared_buffers.length, 1); assert.equal(result.shared_buffers[0].id, 1);
  const actual = result.shared_buffers[0].buffer;
  for (const key of ['element', 'access', 'alignment']) assert.equal(actual[key], query.shared_buffers[0][key]);
  assert.equal(actual.bytes, '0x' + expected.toString('hex')); assert.equal(actual.initialized, '0x' + initialized.toString('hex'));
  return { expected_word: word, checked_output_words: elements, checked_backing_bytes: expected.length, checked_guard_bytes: 8 };
}
export function verifyEmission(value, exported, variant, kir, llvm) {
  exact(value, ['kind', 'authority', 'canonical_wire_version', 'canonical_identity', 'canonical_bytes', 'input_file_sha256',
    'llvm_sha256', 'llvm_bytes', 'program_count', 'descriptors', 'register_plan', 'canonical_retained_storage_bytes',
    'canonical_work_limit', 'canonical_storage_limit', 'max_input_bytes', 'max_published_llvm_bytes',
    'emitter_text_limit_bytes', 'canonical_and_emitter_accounting_are_separate', 'source_authentication',
    'compiler_closure_attestation', 'proof_authority', 'protected_admission', 'final_artifact_authority',
    'production_resume', 'physical_register_values', 'hardware_execution']);
  assert.equal(value.kind, 'diagnostic_ordered_program_llvm_observation'); assert.equal(value.authority, 'observation_only');
  assert.equal(value.canonical_wire_version, 17); assert.equal(value.canonical_identity, exported.canonical_sha256);
  assert.equal(kir.length, exported.canonical_bytes);
  assert.equal(value.canonical_bytes, kir.length); assert.equal(value.input_file_sha256, sha256(kir));
  assert.equal(value.llvm_sha256, sha256(llvm)); assert.equal(value.llvm_bytes, llvm.length);
  assert.ok(llvm.length > 0 && llvm.length <= MAX_SOURCE); assert.equal(value.program_count, 3);
  assert.deepEqual(value.descriptors, descriptors(profile(variant))); assert.deepEqual(value.register_plan, [4, 5, 0, 1, 2]);
  for (const field of ['source_authentication', 'compiler_closure_attestation', 'proof_authority', 'protected_admission',
    'final_artifact_authority', 'production_resume', 'physical_register_values', 'hardware_execution']) assert.equal(value[field], false);
  assert.equal(value.max_input_bytes, MAX_SOURCE); assert.equal(value.max_published_llvm_bytes, MAX_SOURCE);
  assert.equal(value.canonical_work_limit, 1 << 26); assert.equal(value.canonical_storage_limit, 64 * MiB);
  natural(value.canonical_retained_storage_bytes, 64 * MiB);
  assert.equal(value.emitter_text_limit_bytes, 16 * MiB); assert.equal(value.canonical_and_emitter_accounting_are_separate, true);
}
export function verifyVariants(variants, semanticMode) {
  assert.ok(semanticMode === 'required' || semanticMode === 'unavailable');
  assert.deepEqual(variants.map(value => value.label), ['default', 'edited', 'repeat']);
  const [first, edited, repeat] = variants;
  for (const field of ['source_sha256', 'llvm_sha256', 'kir_file_sha256']) {
    assert.notEqual(first[field], edited[field], field); assert.equal(edited[field], repeat[field], field);
  }
  // This opcode-only edit preserves the retained root/contract inventory, whose
  // encoder hashes function identities and contracts, not body instructions
  // (collector/production_importer_v1.rs: identity_inventory_identity_and_transcript_v1).
  // Body-sensitive preflight, semantic, source-occurrence and KIR identities must
  // still change below; inventory equality cannot stand in for any of them.
  assert.equal(first.exported.retained_source_inventory, edited.exported.retained_source_inventory,
    'unchanged root/contract inventory');
  assert.equal(edited.exported.retained_source_inventory, repeat.exported.retained_source_inventory,
    'unchanged repeat root/contract inventory');
  for (const field of ['canonical_sha256', 'retained_source_preflight']) {
    assert.notEqual(first.exported[field], edited.exported[field], field);
    assert.equal(edited.exported[field], repeat.exported[field], field);
  }
  if (semanticMode === 'required') {
    variants.forEach(value => digest(value.exported.semantic_identity));
    assert.notEqual(first.exported.semantic_identity, edited.exported.semantic_identity);
    assert.equal(edited.exported.semantic_identity, repeat.exported.semantic_identity);
  } else variants.forEach(value => assert.equal(value.exported.semantic_identity, null));
  assert.deepEqual(edited.inspection, repeat.inspection, 'same actual source repeat has identical inspected owner');
  assert.notEqual(first.inspection.declared_source_ids.statement, edited.inspection.declared_source_ids.statement);
}

// Filesystem/process operations below are unreachable when this module is imported.
function observe(file, cap, keep = false, allowEmpty = false) {
  absolute(file); assert.equal(fs.realpathSync(file), file, 'no symlink traversal in retained input');
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd, { bigint: true });
    assert.ok(before.isFile() && (before.size > 0n || allowEmpty) && before.size <= BigInt(cap));
    const hash = createHash('sha256'), chunks = [], scratch = Buffer.alloc(64 * KiB); let length = 0;
    for (;;) {
      const count = fs.readSync(fd, scratch, 0, scratch.length, null); if (!count) break;
      length += count; assert.ok(length <= cap); hash.update(scratch.subarray(0, count));
      if (keep) chunks.push(Buffer.from(scratch.subarray(0, count)));
    }
    const after = fs.fstatSync(fd, { bigint: true }), named = fs.lstatSync(file, { bigint: true });
    for (const field of ['dev', 'ino', 'mode', 'nlink', 'size', 'mtimeNs', 'ctimeNs']) {
      assert.equal(after[field], before[field]); assert.equal(named[field], before[field]);
    }
    assert.equal(BigInt(length), before.size);
    return { pin: { path: file, bytes: length, sha256: hash.digest('hex'),
      device: String(before.dev), inode: String(before.ino), mode: String(before.mode),
      mtime_ns: String(before.mtimeNs), ctime_ns: String(before.ctimeNs) },
      bytes: keep ? Buffer.concat(chunks) : undefined };
  } finally { fs.closeSync(fd); }
}
function sanitized() {
  const env = { ...process.env };
  for (const key of Object.keys(env)) if (key.startsWith('FE2O3_') ||
    (key.startsWith('CARGO_TARGET_') && key.endsWith('_RUSTFLAGS')) ||
    ['RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'CARGO_BUILD_TARGET', 'RUSTC_WRAPPER', 'RUSTC_WORKSPACE_WRAPPER',
      'CARGO_BUILD_RUSTC_WRAPPER', 'CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER'].includes(key)) delete env[key];
  return { ...env, CARGO_INCREMENTAL: '0', CARGO_BUILD_JOBS: '2', CARGO_NET_OFFLINE: 'true' };
}
export function options(argv) {
  const keys = ['repo', 'prepared-root', 'seed-record', 'consumer', 'bin-dir', 'emitter', 'output', 'semantic'];
  assert.equal(argv.length, keys.length * 2); const out = {};
  for (let at = 0; at < argv.length; at += 2) {
    assert.ok(argv[at].startsWith('--')); const key = argv[at].slice(2);
    assert.ok(keys.includes(key) && !Object.hasOwn(out, key)); out[key] = argv[at + 1];
  }
  keys.filter(key => key !== 'semantic').forEach(key => absolute(out[key]));
  assert.equal(out['seed-record'], path.join(out['prepared-root'], 'positive/headless-normal.invocation.json'),
    'record must be the exact prepare-only factory output');
  assert.ok(['required', 'unavailable'].includes(out.semantic)); return out;
}
async function run(opt) {
  const repo = opt.repo, output = opt.output, prepared = opt['prepared-root'], pins = new Map(), stages = [], variants = [];
  for (const directory of [repo, prepared, opt['bin-dir'], path.dirname(output)]) assert.equal(fs.realpathSync(directory), directory);
  assert.ok(within(path.dirname(output), output) && !within(repo, output) && output !== repo);
  requireDiskReserve(path.dirname(output)); fs.mkdirSync(output, { mode: 0o700 });
  const save = (name, bytes) => {
    fs.writeFileSync(path.join(output, name), bytes, { flag: 'wx', mode: 0o600 });
    pin(path.join(output, name), Math.max(Buffer.byteLength(bytes), 1), false, Buffer.byteLength(bytes) === 0);
  };
  const saveJson = (name, value) => { const bytes = Buffer.from(JSON.stringify(value, null, 2) + '\n'); assert.ok(bytes.length <= MAX_JSON); save(name, bytes); };
  let charged = 0;
  const pin = (file, cap, keep = false, allowEmpty = false) => {
    const observed = observe(file, cap, keep, allowEmpty), prior = pins.get(file);
    if (prior) assert.deepEqual(observed.pin, prior);
    else { assert.ok(pins.size < MAX_PINS); charged += observed.pin.bytes; assert.ok(charged <= MAX_PIN_BYTES); pins.set(file, observed.pin); }
    return observed;
  };
  const runTool = async (label, executable, args, env = sanitized(), timeoutMs = 300000) => {
    assert.ok(stages.length < 110);
    if (!pins.has(executable)) pin(executable, 512 * MiB);
    requireDiskReserve(output);
    const captured = await runBoundedCommand({ executable, args, cwd: repo, env, timeoutMs,
      stdoutCap: MiB, stderrCap: MiB, diskDirectory: output });
    save(label + '.stdout', captured.stdout); save(label + '.stderr', captured.stderr);
    stages.push({ label, executable, args, code: captured.code, signal: captured.signal, reason: captured.reason,
      elapsed_ms: captured.elapsed_ms, stdout_bytes: captured.stdout.length, stdout_sha256: sha256(captured.stdout),
      stderr_bytes: captured.stderr.length, stderr_sha256: sha256(captured.stderr) });
    assert.equal(captured.reason, null, label); assert.equal(captured.signal, null, label); assert.equal(captured.code, 0, label);
    return captured;
  };
  try {
    const record = json(pin(opt['seed-record'], MAX_JSON, true).bytes), seed = validateSeedRecord(record, repo, prepared);
    const fixture = path.join(repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device');
    const fixtureFiles = ['Cargo.toml', 'src/lib.rs', 'src/source_bitselect_feasibility.rs', 'src/source_bitselect_normalized.rs'];
    fixtureFiles.forEach((name, index) => assert.equal(pin(path.join(fixture, name), MAX_SOURCE).pin.sha256, record.fixture_sha256[index]));
    for (const [name, expected] of [['metadata.stdout', record.metadata_sha256], ['dependencies.stdout', record.artifacts_sha256]]) {
      assert.equal(pin(path.join(prepared, name), MAX_JSON).pin.sha256, expected);
    }
    for (const file of [seed.device, seed.core, record.args[0], path.join(seed.sysroot, 'bin/cargo')]) pin(file, 512 * MiB);
    assert.equal(fs.realpathSync(seed.analysis), seed.analysis, 'exact preparation analysis directory');
    assert.equal(fs.readdirSync(seed.analysis).length, 0);
    assert.equal(fs.realpathSync(seed.directory), seed.directory);
    const originalPath = path.join(seed.directory, 'original.rs'), original = pin(originalPath, MAX_SOURCE, true).bytes;
    assert.equal(sha256(original), record.source_sha256);
    assert.deepEqual(original, pin(path.join(fixture, fixtureFiles[2]), MAX_SOURCE, true).bytes, 'closed no-prefix source fixture');
    const loader = pin(path.join(seed.directory, 'original-loader.rs'), MAX_SOURCE, true);
    assert.equal(loader.pin.sha256, record.loader_sha256); assert.equal(decode(loader.bytes), LOADER);
    const candidatePath = path.join(seed.directory, 'instruction-default.rs');
    assert.ok(!fs.existsSync(candidatePath), 'fresh public seed destination required');
    const seedEnv = { ...sanitized(), FE2O3_CRATE_BINDING_ID_V1: record.crate_binding,
      FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2: record.cargo_observation,
      CARGO_MANIFEST_DIR: fixture, CARGO_PKG_NAME: PACKAGE, CARGO_PKG_VERSION: '0.1.0', CARGO_CRATE_NAME: CRATE };
    const publicRun = await runTool('public-seed', opt.consumer, [
      path.relative(repo, originalPath), path.relative(repo, candidatePath), record.source_sha256, '--', ...record.args,
    ], seedEnv);
    const candidate = pin(candidatePath, MAX_SOURCE, true).bytes;
    const publication = parsePublication(publicRun.stdout, original, candidate), edited = verifySourceEdit(original, candidate);
    assert.equal(fs.readdirSync(seed.analysis).length, 0);
    save('original.rs', original); save('public-candidate.rs', candidate); save('instruction-edited.rs', edited);
    const template = path.join(repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30');
    let manifest = pin(path.join(template, 'Cargo.toml'), MAX_SOURCE, true).bytes;
    for (const dependency of ['fe2o3-device', 'fe2o3-host']) manifest = replaceOnce(manifest,
      '"../../../../' + dependency + '"', JSON.stringify(path.join(repo, 'crates', dependency)));
    manifest = replaceOnce(manifest, 'edited = []', 'source-bitselect-feasibility = []\nsource-bitselect-ambiguous = []\nsource-bitselect-local-alias = []');
    const lock = pin(path.join(template, 'Cargo.lock'), MAX_JSON, true).bytes;
    const tool = name => path.join(opt['bin-dir'], name);
    pin(tool('fe2o3-rustc-extract'), 512 * MiB);
    const env = { ...sanitized(), RUSTC: record.args[0], CARGO: path.join(seed.sysroot, 'bin/cargo') };
    for (const [label, source] of [['default', candidate], ['edited', edited]]) {
      const directory = path.join(output, label + '-source'); fs.mkdirSync(directory, { mode: 0o700 });
      fs.mkdirSync(path.join(directory, 'src'), { mode: 0o700 });
      for (const [name, bytes] of [['Cargo.toml', manifest], ['Cargo.lock', lock], ['src/lib.rs', Buffer.from(EXPORT_LOADER)], ['src/kernel.rs', source]]) {
        fs.writeFileSync(path.join(directory, name), bytes, { flag: 'wx', mode: 0o600 }); pin(path.join(directory, name), MAX_JSON);
      }
    }
    for (const label of ['default', 'edited', 'repeat']) {
      const variant = label === 'default' ? 'default' : 'edited', source = variant === 'default' ? candidate : edited;
      const kirPath = path.join(output, label + '.kir'), llvmPath = path.join(output, label + '.ll');
      const produced = await runTool(label + '-export', tool('fe2o3-export-sim'), [
        '--diagnostic-kir-v17', '--crate', EXPORT_CRATE, '--output', kirPath, '--target', 'gfx942',
        '--target-dir', path.join(output, label + '-export-target'), '--', '--manifest-path',
        path.join(output, variant + '-source/Cargo.toml'), '--lib', '--offline', '--features', 'source-bitselect-feasibility',
      ], env);
      const exported = parseExport(produced.stderr, opt.semantic), kir = pin(kirPath, MAX_SOURCE, true).bytes;
      // Canonical identity has its own domain; it is not the raw-file SHA256.
      assert.equal(kir.length, exported.canonical_bytes);
      const probe = request(INPUTS[0], 1); saveJson(label + '-inspect-request.json', probe);
      const inspected = await runTool(label + '-inspect', tool('fe2o3-program-inspect'), [kirPath, path.join(output, label + '-inspect-request.json')], env, 60000);
      const inspection = json(inspected.stdout); verifyInspector(inspection, exported, variant);
      const lowered = await runTool(label + '-llvm', opt.emitter, [kirPath, llvmPath], env, 60000);
      const llvm = pin(llvmPath, MAX_SOURCE, true).bytes, emission = json(lowered.stdout);
      verifyEmission(emission, exported, variant, kir, llvm);
      const simulations = [];
      for (let input = 0; input < INPUTS.length; input++) for (const length of LENGTHS) {
        const query = request(INPUTS[input], length), name = label + '-case-' + input + '-length-' + length;
        saveJson(name + '.request.json', query);
        for (let replay = 0; replay < 2; replay++) {
          const result = await runTool(name + '-replay-' + replay, tool('fe2o3-kir-sim'),
            ['--diagnostic-kir-v17', kirPath, '--request', path.join(output, name + '.request.json')], env, 60000);
          simulations.push({ input, length, replay, ...verifySimulation(json(result.stdout), query, exported, variant, INPUTS[input], length) });
        }
      }
      variants.push({ label, source_sha256: sha256(source), exported, inspection, emission,
        kir_file_sha256: sha256(kir), llvm_sha256: sha256(llvm), simulations });
    }
    verifyVariants(variants, opt.semantic);
    for (const [file, expected] of pins) assert.deepEqual(observe(file, expected.bytes, false, expected.bytes === 0).pin, expected, 'retained file changed');
    saveJson('receipt.json', { status: 'passed', kind: 'source_promotion_instruction_edit_diagnostic_v1',
      normal_public_seed: publication, actual_normal_exports: 3, whole_kernel_simulations: 90,
      variants, stages, retained_file_pins: [...pins.values()], retained_pin_bytes: charged,
      semantic_identity_join_qualified: opt.semantic === 'required',
      semantic_identity_availability: opt.semantic === 'required' ? 'normal_live_exporter_observed' : 'unavailable_not_inferred_from_preflight',
      source_edit: 'one_exact_final_xor_to_or_original_and_surrounding_bytes_unchanged',
      ranked_checks: false, protected_proof: false, compiler_closure_attestation: false, source_authentication: false,
      native_qualified: false, physical_register_lifetime_proof: false, production_resume: false, hardware_observed: false,
      limits: { source_bytes: MAX_SOURCE, command_stream_bytes: MiB, command_ms: 300000, stages: 110,
        retained_pins: MAX_PINS, retained_pin_bytes: MAX_PIN_BYTES, cargo_jobs: 2,
        task_root_storage_accounting: 'external_root_supervisor_required_not_replaced_by_this_runner' } });
  } catch (error) {
    // Preserve the original error even if a full pin ledger or exhausted disk
    // prevents a last diagnostic write; prior create-new logs remain untouched.
    try {
      const failure = Buffer.from(JSON.stringify({ status: 'failed', error: String(error).slice(0, 4096), stages,
        candidate_may_exist: true, rollback_or_cleanup_claimed: false, manufactured_fallback: false }, null, 2) + '\n');
      assert.ok(failure.length <= MAX_JSON);
      fs.writeFileSync(path.join(output, 'failure.json'), failure, { flag: 'wx', mode: 0o600 });
    } catch (reportError) {
      process.stderr.write('Failure receipt unavailable: ' + String(reportError).slice(0, 1024) + '\n');
    }
    throw error;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run(options(process.argv.slice(2))).catch(error => { process.stderr.write(String(error).slice(0, 4096) + '\n'); process.exitCode = 1; });
}
