#!/usr/bin/env node
// Retained ordinary-source -> exact diagnostic owner -> LLVM -> native observations.
// No detached file or joined receipt authenticates source, a compiler closure,
// protected admission, a register lifetime, GPU execution, or production resume.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { MIN_FREE_BYTES, runBoundedCommand, requireDiskReserve,
  measureNativeBuildInput, requireNativeBuildInputsUnchanged }
  from './assembly-region-worker-prototype.mjs';
import { LLVM_BUILD_ID, TEST_TARGET, MAX_REPORT, MAX_CACHE_BYTES, MIN_RAM_BYTES,
  EXPECTED_PROGRAMS, validateProgramBuildReceipt, parseProgramObservation,
  parseCacheObservation, validateProgramOutputLayout }
  from './ordered-program-worker-prototype.mjs';

const MiB = 1024 * 1024;
const HERE = path.dirname(fileURLToPath(import.meta.url));
const SHA = /^[0-9a-f]{64}$/;
const CLAIM = /^fe2o3-worker-v1-sha256-[0-9a-f]{64}$/;
const falses = ['source_authentication', 'compiler_closure_attestation', 'proof_authority',
  'protected_admission', 'final_artifact_authority', 'production_resume',
  'physical_register_values', 'hardware_execution'];
export const DESCRIPTORS = Object.freeze({
  one: Object.freeze([8]), three: Object.freeze([133, 307, 413]),
  sixteen: Object.freeze([0, 141, 323, 188, 321, 58, 64, 181, 60, 331, 73, 194, 56, 333, 16, 72]),
});
export const SOURCE_VARIANTS = Object.freeze([
  ['ordered-program-one-v32', 'one'], ['ordered-program-v32', 'three'],
  ['ordered-program-sixteen-v32', 'sixteen'],
].flatMap(([name, profile]) => [true, false].map(used => Object.freeze({
  name: name + (used ? '' : '-unused'), profile, used,
  feature: name + (used ? '' : ',ordered-program-unused-v32'),
}))));
const REFUSALS = Object.freeze([
  ['alias', 'ordered program physical roles must be distinct v0..v63'],
  ['dynamic', 'ordered program physical role is not an actual MIR constant'],
  ['divergent', 'ordered program requires bounded unconditional acyclic source placement'],
  ['wrong-launch', 'ordered program requires required and maximum 64x1x1 workgroup bounds'],
  ...['invalid-count', 'invalid-opcode', 'read-before-init', 'padding'].map(name =>
    [name, 'ordered program descriptors, padding or definite initialization are invalid']),
].map(([name, diagnostic]) => Object.freeze({ name: `ordered-program-${name}-v32`, diagnostic })));
export const SCALAR_CASES = Object.freeze([[0, 0, 0], [4294967295, 0, 1], [4294967295, 1, 2],
  [2147483648, 0, 2147483648], [2863289685, 1431677610, 19], [19, 23, 42]].map(Object.freeze));
const CONTROL_NAMES = ['wrong-wire', 'wrong-launch', 'duplicate-request-field', 'duplicate-input-option',
  'mixed-input-options', ...Array.from({ length: 9 }, (_, i) => `schedule-before-io-${i}`), 'existing-simulator-output'];
const CONTROL_EXPECTATIONS = [
  ['kir_admission', 'kir_v17_wrong_version', 'kir_v17'],
  ['preflight', 'preflight_workgroup_mismatch', null],
  ['request', 'request_json_duplicate_field', null],
  ['arguments', 'invalid_command_line', null], ['arguments', 'invalid_command_line', null],
  ...Array.from({ length: 9 }, () => ['arguments', 'schedule_input_unsupported', null]),
  ['output', 'output_already_exists', null],
];
const RUSTC_COMMIT = '55e86c996809902e8bbad512cfb4d2c18be446d9';
function controlArguments(sourceBatch) {
  const first = path.join(sourceBatch, SOURCE_VARIANTS[0].name), directory = path.join(sourceBatch, 'negative-controls');
  const kir = path.join(first, 'kernel.kir'), request = path.join(first, 'request-case-5.json');
  const base = ['--diagnostic-kir-v17', kir, '--request', request];
  const absent = path.join(directory, 'must-not-be-published');
  const scheduleBase = ['--diagnostic-kir-v17', path.join(directory, 'missing-kir'), '--request', path.join(directory, 'missing-request')];
  return [
    ['--diagnostic-kir-v17', path.join(directory, 'wrong-wire-v12.kir'), '--request', request],
    ['--diagnostic-kir-v17', kir, '--request', path.join(directory, 'wrong-launch.json')],
    ['--diagnostic-kir-v17', kir, '--request', path.join(directory, 'duplicate-request-field.json')],
    [...base, '--diagnostic-kir-v17', kir], [...base, '--bundle-v6', path.join(directory, 'missing-bundle')],
    ...[['--record-canonical-schedule', absent], ['--record-seeded-schedule', absent], ['--replay-schedule', absent],
      ['--explore-seeded-schedules', '1'], ['--reduce-failure'], ['--replay-failure-reduction', absent],
      ['--schedule-seed', '0'], ['--schedule-max-decisions', '1'], ['--exploration-max-retained-decisions', '1']]
      .map(extra => [...scheduleBase, ...extra]),
    [...base, '--output', path.join(first, 'sim-case-5.json')],
  ];
}
const SOURCE_FALSE = ['exported_source_authentication', 'compiler_closure_attestation',
  'protected_admission', 'production_resume', 'hardware_execution', 'physical_register_values', 'persisted_schedule'];
const SITE_KEYS = ['file_offset', 'opcode', 'bytes_hex', 'mc_flags', 'register_operands', 'implicit_reads', 'implicit_writes'];
function demand(condition, message) { if (!condition) throw new Error(message); }
function exact(value, keys, label) {
  demand(value && typeof value === 'object' && !Array.isArray(value)
    && Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key)), `${label}: exact object fields`);
}
function dense(value, count, label) {
  demand(Array.isArray(value) && value.length === count
    && Array.from({ length: count }, (_, i) => Object.hasOwn(value, i)).every(Boolean), `${label}: dense array`);
}
function same(actual, expected, label) {
  demand(JSON.stringify(actual) === JSON.stringify(expected), label);
}
function natural(value, max, label, min = 0) {
  demand(Number.isSafeInteger(value) && value >= min && value <= max, `${label}: integer bound`);
}
function digest(value, label) { demand(typeof value === 'string' && SHA.test(value) && value !== '0'.repeat(64), `${label}: digest`); }
export function sha256(bytes) { return crypto.createHash('sha256').update(bytes).digest('hex'); }
function absolute(value, label = 'path') {
  demand(typeof value === 'string' && Buffer.byteLength(value) <= 4096 && path.isAbsolute(value)
    && path.resolve(value) === value && !/[\x00-\x1f\x7f]/.test(value), `${label}: canonical absolute path`);
  return value;
}
function inside(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
}

// Reject duplicate keys (including escaped spellings), imprecise numbers,
// malformed UTF8, excessive depth/nodes and trailing data before consuming JSON.
export function parseBoundedJson(bytes, cap = MiB) {
  demand(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= cap, 'JSON byte bound');
  const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  let at = 0, nodes = 0;
  const ws = () => { while (at < text.length && /[ \t\r\n]/.test(text[at])) at++; };
  function string() {
    const start = at++;
    while (at < text.length) {
      const char = text[at++];
      if (char === '\\') at++;
      else if (char === '"') return JSON.parse(text.slice(start, at));
    }
    throw new Error('unterminated JSON string');
  }
  function value(depth) {
    demand(depth <= 24 && ++nodes <= 65536, 'JSON structural bound'); ws();
    const start = text[at++];
    if (start === '{' || start === '[') {
      const object = start === '{', end = object ? '}' : ']', keys = new Set();
      ws(); if (text[at] === end) { at++; return; }
      for (let count = 0; ; count++) {
        demand(count < 8192, 'JSON collection bound'); ws();
        if (object) {
          demand(text[at] === '"', 'JSON key'); const key = string();
          demand(!keys.has(key), 'duplicate JSON key'); keys.add(key); ws();
          demand(text[at++] === ':', 'JSON colon');
        }
        value(depth + 1); ws(); if (text[at] === end) { at++; return; }
        demand(text[at++] === ',', 'JSON separator');
      }
    }
    if (start === '"') { at--; string(); return; }
    const begin = at - 1;
    while (at < text.length && !/[\s,\]}]/.test(text[at])) at++;
    const token = text.slice(begin, at);
    demand(token.length > 0 && token.length <= 128, 'JSON scalar bound');
    const scalar = JSON.parse(token);
    if (typeof scalar === 'number') demand(Number.isSafeInteger(scalar) && /^(?:0|[1-9][0-9]*)$/.test(token), 'JSON exact unsigned integer');
  }
  value(0); ws(); demand(at === text.length, 'JSON trailing data');
  return JSON.parse(text);
}

function measured(value, label) {
  exact(value, ['requested', 'resolved', 'bytes', 'sha256'], label);
  absolute(value.requested); absolute(value.resolved); natural(value.bytes, 512 * MiB, label, 1); digest(value.sha256, label);
}
function validateArtifact(value, variant, sourceBatch) {
  exact(value, ['name', 'directory', 'kirPath', 'status', 'canonical_sha256', 'canonical_bytes',
    'observed_retained_source_inventory', 'observed_retained_source_preflight',
    'source_authentication_carried_by_raw_bytes', 'file_sha256', 'file_bytes'], 'source export');
  const directory = path.join(sourceBatch, variant.name);
  demand(value.name === variant.name && value.directory === directory && value.kirPath === path.join(directory, 'kernel.kir')
    && value.status === 'exported' && value.source_authentication_carried_by_raw_bytes === false, 'source export identity/scope');
  for (const key of ['canonical_sha256', 'file_sha256', 'observed_retained_source_inventory', 'observed_retained_source_preflight']) digest(value[key], key);
  natural(value.file_bytes, MAX_REPORT, 'source KIR bytes', 10);
  demand(value.canonical_bytes === value.file_bytes, 'source canonical length');
}
export function validateSourceReceipt(receipt, { repo, sourceBatch }) {
  absolute(repo); absolute(sourceBatch);
  exact(receipt, ['schema', 'status', 'scope', 'source_export_executed', ...SOURCE_FALSE, 'output', 'repo', 'source_target', 'node',
    'limits', 'stages', 'resource_guards', 'exports', 'simulations', 'controls', 'binaries_before', 'selected_library_measurements',
    'rustc_binary', 'rustc_version', 'inputs_before', 'inputs_after', 'counts'], 'source receipt');
  demand(receipt?.schema === 'task-phase9-ordinary-program-source-qualification-v1' && receipt.status === 'passed'
    && receipt.scope === 'actual ordinary source export and logical CPU observation for the closed ordered-program profile'
    && receipt.output === sourceBatch && receipt.repo === repo && receipt.source_export_executed === true, 'source receipt schema/scope/status');
  for (const field of SOURCE_FALSE) demand(receipt[field] === false, `source authority: ${field}`);
  exact(receipt.counts, ['source_exports', 'expected_source_refusals', 'simulations', 'ordinary_negative_controls'], 'source counts');
  same(receipt.counts, { source_exports: 6, expected_source_refusals: 8, simulations: 36, ordinary_negative_controls: 15 }, 'complete source batch counts');
  dense(receipt.exports, 14, 'source exports'); dense(receipt.simulations, 36, 'source simulations');
  absolute(receipt.source_target); demand(typeof receipt.node === 'string' && /^v22\./.test(receipt.node), 'source runner Node version');
  exact(receipt.limits, ['cargo_ms', 'command_output_bytes_per_stream', 'maximum_root_target_bytes', 'minimum_available_ram_bytes',
    'minimum_persistent_free_bytes', 'jobs'], 'source limits');
  same(receipt.limits, { cargo_ms: 300000, command_output_bytes_per_stream: MiB, maximum_root_target_bytes: MAX_CACHE_BYTES.toString(),
    minimum_available_ram_bytes: MIN_RAM_BYTES.toString(), minimum_persistent_free_bytes: MIN_FREE_BYTES.toString(), jobs: 2 }, 'source limits changed');
  SOURCE_VARIANTS.forEach((variant, index) => validateArtifact(receipt.exports[index], variant, sourceBatch));
  REFUSALS.forEach((expected, index) => {
    const value = receipt.exports[6 + index]; exact(value, ['name', 'status', 'diagnostic', 'kir_absent'], 'source refusal');
    demand(value.name === expected.name && value.status === 'expected_source_refusal'
      && value.diagnostic === expected.diagnostic && value.kir_absent === true, 'source refusal mismatch');
  });
  const names = ['rustc-version'];
  for (const variant of [...SOURCE_VARIANTS, ...REFUSALS]) {
    for (const suffix of ['before-target-du', 'before-secondary-du', 'export', 'after-target-du', 'after-secondary-du']) names.push(`${variant.name}-${suffix}`);
    if (Object.hasOwn(variant, 'profile')) for (let i = 0; i < 6; i++) names.push(`${variant.name}-sim-${i}`);
  }
  names.push(...CONTROL_NAMES); dense(receipt.stages, names.length, 'source stages');
  const refusals = new Set([...REFUSALS.map(item => `${item.name}-export`), ...CONTROL_NAMES]);
  receipt.stages.forEach((stage, index) => {
    exact(stage, ['stage', 'executable', 'args', 'code', 'signal', 'reason', 'elapsed_ms', 'free_bytes_before',
      'stdout_sha256', 'stdout_bytes', 'stderr_sha256', 'stderr_bytes', 'log_stem'], 'source stage');
    demand(stage.stage === names[index] && stage.log_stem === `${String(index).padStart(3, '0')}-${stage.stage}`
      && stage.signal === null && stage.reason === null, 'source stage order/transport');
    natural(stage.code, 255, 'source exit'); demand((stage.code !== 0) === refusals.has(stage.stage), 'source exit expectation');
    absolute(stage.executable); demand(Array.isArray(stage.args) && stage.args.length <= 128
      && stage.args.every(arg => typeof arg === 'string' && arg.length <= 4096 && !arg.includes('\0')), 'source args');
    natural(stage.elapsed_ms, 300000, 'source duration');
    demand(typeof stage.free_bytes_before === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(stage.free_bytes_before)
      && BigInt(stage.free_bytes_before) >= MIN_FREE_BYTES, 'source disk reserve');
    for (const stream of ['stdout', 'stderr']) { digest(stage[`${stream}_sha256`], 'source stream digest'); natural(stage[`${stream}_bytes`], MiB, 'source stream bytes'); }
  });
  dense(receipt.controls, 15, 'source controls');
  receipt.controls.forEach((control, index) => {
    exact(control, ['name', 'stage', 'kind', 'input', 'status'], 'source control');
    demand(control.name === CONTROL_NAMES[index] && control.status === 'expected_refusal', 'source control status/order');
    same([control.stage, control.kind, control.input], CONTROL_EXPECTATIONS[index], 'source control exact refusal');
  });
  dense(receipt.resource_guards, 28, 'source resource guards');
  const guardNames = [...SOURCE_VARIANTS, ...REFUSALS].flatMap(item => [item.name + '-before', item.name + '-after']);
  receipt.resource_guards.forEach((guard, index) => {
    exact(guard, ['stage', 'target_resolved', 'target_bytes', 'secondary_cache_bytes', 'combined_cache_bytes',
      'available_ram_bytes', 'free_persistent_bytes'], 'source resource guard');
    demand(guard.stage === guardNames[index], 'source resource guard order'); absolute(guard.target_resolved);
    for (const key of ['target_bytes', 'secondary_cache_bytes', 'combined_cache_bytes', 'available_ram_bytes', 'free_persistent_bytes'])
      demand(typeof guard[key] === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(guard[key]), 'source resource decimal');
    demand(BigInt(guard.combined_cache_bytes) === BigInt(guard.target_bytes) + BigInt(guard.secondary_cache_bytes)
      && BigInt(guard.combined_cache_bytes) <= MAX_CACHE_BYTES && BigInt(guard.available_ram_bytes) >= MIN_RAM_BYTES
      && BigInt(guard.free_persistent_bytes) >= MIN_FREE_BYTES, 'source resource guard bounds');
  });
  demand(Array.isArray(receipt.inputs_before) && receipt.inputs_before.length > 0 && receipt.inputs_before.length <= 128, 'source input roster');
  dense(receipt.inputs_after, receipt.inputs_before.length, 'source after roster');
  const seen = new Set();
  receipt.inputs_before.forEach((item, index) => {
    measured(item, 'source input'); demand(!seen.has(item.requested), 'duplicate source input'); seen.add(item.requested);
    same(item, receipt.inputs_after[index], 'source input changed during qualification');
  });
  dense(receipt.binaries_before, 5, 'source binary roster');
  receipt.binaries_before.forEach((item, index) => {
    measured(item, 'source binary');
    demand(path.basename(item.requested) === ['fe2o3-export-sim', 'fe2o3-rustc-extract', 'fe2o3-kir-sim', 'fe2o3-debug', 'librustc_codegen_fe2o3.so'][index], 'source binary role');
    same(item, receipt.inputs_before.find(input => input.requested === item.requested), 'source binary measurement binding');
  });
  measured(receipt.rustc_binary, 'source rustc binary');
  demand(path.basename(receipt.rustc_binary.requested) === 'rustc', 'source rustc binary role');
  same(receipt.rustc_binary, receipt.inputs_before.find(input => input.requested === receipt.rustc_binary.requested), 'source rustc measurement binding');
  const libraries = receipt.selected_library_measurements;
  exact(libraries, ['scope', 'backend_soname', 'backend', 'rustc_driver', 'loader_search_directories', 'runtime_closure_attestation'], 'source selected libraries');
  demand(libraries.scope === 'explicit backend SONAME and pinned rustc-driver files in the exporter-configured loader search path; not a complete runtime closure'
    && libraries.backend_soname === 'librustc_codegen_fe2o3.so' && libraries.runtime_closure_attestation === false, 'source selected library scope');
  same(libraries.backend, receipt.binaries_before[4], 'source backend library binding');
  measured(libraries.rustc_driver, 'source rustc driver');
  demand(/^librustc_driver-[0-9a-f]{16}\.so$/.test(path.basename(libraries.rustc_driver.requested))
    && path.dirname(libraries.rustc_driver.requested) === path.resolve(path.dirname(receipt.rustc_binary.requested), '../lib'), 'source rustc driver role');
  same(libraries.rustc_driver, receipt.inputs_before.find(input => input.requested === libraries.rustc_driver.requested), 'source rustc driver measurement binding');
  same(libraries.loader_search_directories, [path.dirname(libraries.backend.resolved), path.dirname(libraries.rustc_driver.requested)], 'source loader search binding');
  exact(receipt.rustc_version, ['text', 'commit'], 'source rustc version');
  demand(typeof receipt.rustc_version.text === 'string' && Buffer.byteLength(receipt.rustc_version.text) <= 4096
    && receipt.rustc_version.commit === RUSTC_COMMIT, 'source rustc version bound/commit');
  same([...receipt.rustc_version.text.matchAll(/^commit-hash: (.+)$/gm)].map(match => match[1]), [RUSTC_COMMIT], 'source rustc version commit');
  same([...receipt.rustc_version.text.matchAll(/^release: (.+)$/gm)].map(match => match[1]), ['1.96.0-nightly'], 'source rustc release');
  const rustcStage = receipt.stages[0];
  demand(rustcStage.executable === receipt.rustc_binary.requested && rustcStage.stderr_bytes === 0
    && rustcStage.stderr_sha256 === sha256(Buffer.alloc(0)), 'source rustc command');
  same(rustcStage.args, ['-vV'], 'source rustc command arguments');
  const versionBytes = Buffer.from(receipt.rustc_version.text);
  demand(rustcStage.stdout_bytes === versionBytes.length && rustcStage.stdout_sha256 === sha256(versionBytes), 'source rustc retained version binding');
  const fixture = path.join(repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device');
  const stages = new Map(receipt.stages.map(stage => [stage.stage, stage]));
  for (const variant of [...SOURCE_VARIANTS, ...REFUSALS]) {
    const stage = stages.get(`${variant.name}-export`);
    demand(stage.executable === receipt.binaries_before[0].requested, 'measured source exporter command');
    same(stage.args, ['--diagnostic-kir-v17', '--crate', 'fe2o3_production_extraction_fixture', '--output', path.join(sourceBatch, variant.name, 'kernel.kir'),
      '--target', 'gfx942', '--target-dir', receipt.source_target, '--', '--manifest-path', path.join(fixture, 'Cargo.toml'),
      '-p', 'fe2o3-production-extraction-fixture', '--lib', '--no-default-features', '--features', variant.feature ?? variant.name, '--offline'], 'source command exact feature binding');
  }
  controlArguments(sourceBatch).forEach((args, index) => {
    const stage = stages.get(CONTROL_NAMES[index]);
    demand(stage.executable === receipt.binaries_before[2].requested && stage.stdout_bytes === 0
      && stage.stdout_sha256 === sha256(Buffer.alloc(0)), 'measured source control command');
    same(stage.args, args, 'source control exact command binding');
  });
  for (const file of [path.join(fixture, 'Cargo.toml'), path.join(fixture, 'src/lib.rs'), path.join(fixture, 'src/ordered_program_v32.rs'),
    path.join(repo, 'Cargo.toml'), path.join(repo, 'Cargo.lock')]) demand(seen.has(file), 'missing source/compiler input measurement');
  for (const item of receipt.exports.slice(0, 6)) demand(receipt.inputs_before.some(input => input.requested === item.kirPath
    && input.bytes === item.file_bytes && input.sha256 === item.file_sha256), 'source KIR measurement absent');
  return receipt.exports.slice(0, 6);
}

export function programResult(profile, inputs) {
  demand(Object.hasOwn(DESCRIPTORS, profile), 'program profile'); dense(inputs, 3, 'inputs'); inputs.forEach(value => natural(value, 0xffffffff, 'u32 input'));
  const [a, b, c] = inputs.map(BigInt), mask = 0xffffffffn;
  if (profile === 'one') return Number(a);
  if (profile === 'three') return Number(b ^ ((a ^ b) & c));
  // Direct imperative oracle, independently transcribed from the source, not
  // evaluated through the descriptor or machine-byte encoders being checked.
  let scratch = a, out = a ^ b;
  scratch = out & c; out = scratch | b; scratch = (out + c) & mask;
  out = (scratch - a) & mask; scratch = out; scratch ^= b; out = scratch | a;
  out &= c; out = (out + a) & mask; scratch = (out - b) & mask;
  out = scratch; out ^= c; scratch = b; out = out;
  return Number(out & mask);
}
export function validateSimulationJoin(record, request, result, artifact, variant, caseIndex) {
  const inputs = SCALAR_CASES[caseIndex], program = programResult(variant.profile, inputs), output = variant.used ? program : inputs[0];
  demand(record.name === variant.name && record.case === caseIndex && record.used === variant.used
    && record.program_profile === variant.profile && record.canonical_sha256 === artifact.canonical_sha256
    && record.canonical_bytes === artifact.canonical_bytes, 'simulation selector/canonical join');
  same(record.inputs, inputs, 'simulation inputs'); same(record.before_inputs, inputs, 'logical inputs'); same(record.operand_order, [0, 1, 2], 'operand order');
  demand(record.expected_output === output && record.expected_program_result === program && record.guard_bytes_unchanged === true
    && record.guard_bytes_uninitialized === true && record.checked_lanes === 64, 'simulation oracle claims');
  const argumentsExpected = [{ kind: 'buffer_view', backing: 1, element: 'u32', access: 'read_write', alignment: 4, byte_offset: 4, elements: 64 },
    ...inputs.map(value => ({ kind: 'scalar', type: 'u32', bits: `0x${value.toString(16).padStart(8, '0')}` }))];
  same(request, { schema: 'fe2o3-simulation-request-v1', kernel: 'ordered_u32_program', grid: [64, 1, 1], workgroup: [64, 1, 1],
    arguments: argumentsExpected, shared_buffers: [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
      bytes: `0x${'a5'.repeat(264)}`, initialized: `0x${'00'.repeat(33)}` }] }, 'actual simulation request');
  demand(result.schema === 'fe2o3-simulation-result-v1' && result.status === 'ok' && result.authority === 'observation_only'
    && result.simulated === true && result.hardware_observed === false && result.hardware_validation === false
    && result.performance_prediction === false, 'simulation scope');
  demand(result.kir?.sha256 === artifact.canonical_sha256 && result.kir?.canonical_bytes === artifact.canonical_bytes, 'simulation admitted identity');
  same(result.arguments, argumentsExpected, 'simulation output ABI');
  demand(result.counts?.arguments === 4 && result.counts?.shared_buffers === 1 && result.counts?.invocations_executed === 64
    && result.counts?.workgroups_visited === 1 && result.target_profile?.identity === 'amdgpu_64_little_endian_v1'
    && result.target_profile?.index_bits === 64, 'simulation execution profile');
  dense(result.shared_buffers, 1, 'simulation output backing');
  demand(result.shared_buffers[0].id === 1, 'simulation backing id');
  const actual = result.shared_buffers[0].buffer, bytes = Buffer.alloc(264, 0xa5);
  for (let lane = 0; lane < 64; lane++) bytes.writeUInt32LE(output, 4 + lane * 4);
  demand(actual.element === 'u32' && actual.access === 'read_write' && actual.alignment === 4
    && actual.bytes === `0x${bytes.toString('hex')}` && actual.initialized === `0xf0${'ff'.repeat(31)}0f`, 'actual all-lane output and guards');
  return { case: caseIndex, inputs, program_result: program, stored_output: output, checked_lanes: 64 };
}

export function validateEmission(value, artifact, variant, llvmBytes) {
  exact(value, ['kind', 'authority', 'canonical_wire_version', 'canonical_identity', 'canonical_bytes', 'input_file_sha256',
    'llvm_sha256', 'llvm_bytes', 'program_count', 'descriptors', 'register_plan', 'canonical_retained_storage_bytes',
    'canonical_work_limit', 'canonical_storage_limit', 'max_input_bytes', 'max_published_llvm_bytes', 'emitter_text_limit_bytes',
    'canonical_and_emitter_accounting_are_separate', ...falses], 'emitter report');
  demand(value.kind === 'diagnostic_ordered_program_llvm_observation' && value.authority === 'observation_only'
    && value.canonical_wire_version === 17 && value.canonical_identity === artifact.canonical_sha256
    && value.canonical_bytes === artifact.canonical_bytes && value.input_file_sha256 === artifact.file_sha256, 'emitter source-owner identity join');
  demand(Buffer.isBuffer(llvmBytes) && llvmBytes.length > 0 && llvmBytes.length <= MAX_REPORT
    && value.llvm_bytes === llvmBytes.length && value.llvm_sha256 === sha256(llvmBytes), 'exact emitted LLVM bytes');
  demand(value.program_count === DESCRIPTORS[variant.profile].length, 'emitter program count');
  same(value.descriptors, [...DESCRIPTORS[variant.profile], ...Array(16 - value.program_count).fill(0)], 'exact descriptors/padding');
  same(value.register_plan, [32, 33, 34, 35, 36], 'declared physical role plan');
  demand(value.canonical_work_limit === 1 << 26 && value.canonical_storage_limit === 64 * MiB
    && value.max_input_bytes === MAX_REPORT && value.max_published_llvm_bytes === MAX_REPORT
    && value.emitter_text_limit_bytes === 16 * MiB && value.canonical_and_emitter_accounting_are_separate === true, 'emitter accounting scope');
  natural(value.canonical_retained_storage_bytes, 64 * MiB, 'retained canonical storage', 1);
  for (const key of falses) demand(value[key] === false, `emitter authority: ${key}`);
  return value;
}

function site(value, entry, exactProgram) {
  exact(value, SITE_KEYS, 'native site'); natural(value.file_offset, entry.hsaco_bytes, 'native site offset');
  demand(typeof value.opcode === 'string' && /^[A-Za-z0-9_]{1,128}$/.test(value.opcode), 'native opcode');
  demand(typeof value.bytes_hex === 'string' && /^(?:[0-9a-f]{2}){1,16}$/.test(value.bytes_hex), 'native instruction bytes');
  demand(value.file_offset >= entry.entry_file_offset && value.file_offset + value.bytes_hex.length / 2 <= entry.entry_file_offset + entry.entry_code_bytes, 'native site extent');
  natural(value.mc_flags, 0xffff, 'native flags');
  for (const key of ['register_operands', 'implicit_reads', 'implicit_writes']) {
    demand(Array.isArray(value[key]) && value[key].length <= 32 && value[key].every(name => typeof name === 'string'
      && /^[A-Za-z0-9_]{1,128}$/.test(name)), 'native register list');
  }
  if (exactProgram) {
    demand(value.opcode === exactProgram.opcode && value.bytes_hex === exactProgram.bytes_hex
      && (value.mc_flags & ~16) === 0, 'exact authored opcode/bytes/flags');
    same(value.register_operands, exactProgram.register_operands, 'exact authored operand roles');
    same(value.implicit_reads, ['EXEC'], 'authored EXEC read'); same(value.implicit_writes, [], 'authored implicit writes');
  }
}
export function validateMachineObservation(value, emission, variant, workerClaim) {
  demand(CLAIM.test(workerClaim), 'worker claim');
  exact(value, ['schema', 'authority', 'source_ancestry', 'synthetic_worker_request_identity_fields',
    'production_exact_program_admission', 'protected_finalizer_admission', 'hardware_executed', 'runtime_closure_attestation',
    'physical_register_allocation_or_lifetime_proof', 'whole_kernel_order_or_byte_stability_claim', 'target', 'wave_width',
    'workgroup_size', 'code_object_version', 'kernel_symbol', 'profile', 'program_count', 'result_used', 'result_use_scope',
    'expected_program', 'llvm_text_sha256', 'llvm_text_bytes', 'llvm_build_claim', 'worker_build_claim', 'encoding_reference_scope', 'cases'], 'machine report');
  demand(value.schema === 'fe2o3-ordered-program-llvm-machine-observation-v1' && value.authority === 'unauthenticated-test-transport'
    && value.source_ancestry === 'not-established-by-llvm-file' && value.synthetic_worker_request_identity_fields === true
    && value.runtime_closure_attestation === 'unavailable', 'machine observation scope');
  for (const key of ['production_exact_program_admission', 'protected_finalizer_admission', 'hardware_executed',
    'physical_register_allocation_or_lifetime_proof', 'whole_kernel_order_or_byte_stability_claim']) demand(value[key] === false, 'machine authority');
  demand(value.target === 'gfx942:xnack-' && value.wave_width === 64 && value.workgroup_size === 64 && value.code_object_version === 6, 'machine target');
  demand(value.profile === variant.profile && value.result_used === variant.used && value.program_count === emission.program_count
    && typeof value.kernel_symbol === 'string' && /^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(value.kernel_symbol), 'machine selector');
  demand(value.llvm_text_sha256 === emission.llvm_sha256 && value.llvm_text_bytes === emission.llvm_bytes
    && value.llvm_build_claim === LLVM_BUILD_ID && value.worker_build_claim === workerClaim, 'machine exact LLVM/build join');
  demand(value.result_use_scope === 'checked SSA use in supplied LLVM; not source authentication'
    && value.encoding_reference_scope === 'public gfx900 cross-check; actual pinned gfx942 qualification is this run', 'machine use/reference scope');
  const expected = EXPECTED_PROGRAMS[variant.profile]; dense(value.expected_program, expected.length, 'expected native program');
  value.expected_program.forEach((item, index) => {
    exact(item, ['mnemonic', 'opcode', 'bytes_hex', 'register_operands'], 'literal native expectation');
    const mnemonic = expected[index].opcode.replace(/^V_/, 'v_').replace(/_(?:vi|gfx9)$/, '').toLowerCase();
    demand(item.mnemonic === mnemonic && item.opcode === expected[index].opcode && item.bytes_hex === expected[index].bytes_hex, 'native expectation opcode/bytes');
    same(item.register_operands, expected[index].register_operands, 'native expectation operands');
  });
  dense(value.cases, 2, 'O0/O3 cases');
  value.cases.forEach((entry, index) => {
    exact(entry, ['optimization', 'profile', 'program_count', 'result_used', 'llvm_text_sha256', 'llvm_text_bytes',
      'hsaco_sha256', 'hsaco_bytes', 'descriptor_sha256', 'entry_file_offset', 'entry_code_bytes', 'static_instruction_count',
      'post_link_inspection_diagnostics', 'program', 'boundary_register_site_count', 'boundary_sites', 'boundary_sites_truncated',
      'boundary_observation_is_value_or_lifetime_proof', 'descriptor_resources'], 'native case');
    demand(entry.optimization === ['O0', 'O3'][index] && entry.profile === variant.profile && entry.result_used === variant.used
      && entry.program_count === expected.length && entry.llvm_text_sha256 === emission.llvm_sha256
      && entry.llvm_text_bytes === emission.llvm_bytes, 'native case exact LLVM/profile');
    for (const key of ['hsaco_sha256', 'descriptor_sha256']) digest(entry[key], key);
    natural(entry.hsaco_bytes, MiB, 'native payload bytes', 1); natural(entry.entry_file_offset, entry.hsaco_bytes, 'native entry offset');
    natural(entry.entry_code_bytes, entry.hsaco_bytes - entry.entry_file_offset, 'native entry bytes', expected.length * 4);
    natural(entry.static_instruction_count, 512, 'native instruction count', expected.length);
    dense(entry.program, expected.length, 'native authored steps');
    entry.program.forEach((item, i) => { site(item, entry, expected[i]); demand(i === 0 || item.file_offset === entry.program[i - 1].file_offset + 4, 'native program contiguity'); });
    demand(Array.isArray(entry.post_link_inspection_diagnostics) && entry.post_link_inspection_diagnostics.length <= 32
      && entry.post_link_inspection_diagnostics.every(text => typeof text === 'string' && text.length <= 4096), 'native diagnostic bounds');
    natural(entry.boundary_register_site_count, entry.static_instruction_count - expected.length, 'native boundary sites');
    dense(entry.boundary_sites, Math.min(entry.boundary_register_site_count, 32), 'native retained boundary sites');
    demand(entry.boundary_sites_truncated === (entry.boundary_register_site_count > 32)
      && entry.boundary_observation_is_value_or_lifetime_proof === false, 'native boundary scope');
    let previous = -1;
    entry.boundary_sites.forEach(item => {
      site(item, entry); demand(item.file_offset > previous && !entry.program.some(step => step.file_offset === item.file_offset), 'native boundary order/overlap'); previous = item.file_offset;
      demand(item.register_operands.some(register => /^VGPR3[2-6]$/.test(register)), 'native boundary authored-register footprint');
    });
    const resource = entry.descriptor_resources;
    exact(resource, ['descriptor_file_offset', 'descriptor_bytes', 'descriptor_sha256', 'compute_pgm_rsrc1', 'compute_pgm_rsrc3',
      'vgpr_capacity', 'architected_vgpr_boundary', 'required_footprint_high_water', 'interpretation'], 'native descriptor');
    natural(resource.descriptor_file_offset, entry.hsaco_bytes, 'descriptor offset');
    demand(resource.descriptor_bytes === 64 && resource.descriptor_file_offset + 64 <= entry.hsaco_bytes
      && resource.descriptor_sha256 === entry.descriptor_sha256
      && (resource.descriptor_file_offset + 64 <= entry.entry_file_offset || resource.descriptor_file_offset >= entry.entry_file_offset + entry.entry_code_bytes), 'descriptor extent/identity');
    natural(resource.compute_pgm_rsrc1, 0xffffffff, 'RSRC1'); natural(resource.compute_pgm_rsrc3, 0xffffffff, 'RSRC3');
    const capacity = ((resource.compute_pgm_rsrc1 & 63) + 1) * 8, boundary = ((resource.compute_pgm_rsrc3 & 63) + 1) * 4;
    demand(resource.vgpr_capacity === capacity && resource.architected_vgpr_boundary === boundary && capacity >= boundary && boundary >= 37
      && resource.required_footprint_high_water === 37 && resource.interpretation === 'encoded-capacity-not-metadata-usage-or-lifetime', 'descriptor encoded capacity');
  });
  return value;
}

export function parseArguments(argv) {
  demand(Array.isArray(argv) && argv.length >= 12 && argv.length <= 14 && argv.length % 2 === 0, 'option count');
  const allowed = ['source-batch', 'native-batch', 'emitter', 'output', 'cargo-cache-root', 'secondary-cache-root', 'compiler-repo'], values = new Map();
  for (let i = 0; i < argv.length; i += 2) {
    demand(allowed.some(key => argv[i] === `--${key}`) && !values.has(argv[i]), 'unknown/duplicate option'); values.set(argv[i], absolute(argv[i + 1]));
  }
  for (const key of allowed.slice(0, 6)) demand(values.has(`--${key}`), `missing --${key}`);
  return { sourceBatch: values.get('--source-batch'), nativeBatch: values.get('--native-batch'), emitter: values.get('--emitter'),
    output: values.get('--output'), repo: values.get('--compiler-repo') ?? path.resolve(HERE, '..'),
    cacheRoots: [values.get('--cargo-cache-root'), values.get('--secondary-cache-root')] };
}

function readBounded(file, cap, allowEmpty = false) {
  absolute(file); demand(fs.realpathSync(file) === file, 'retained path redirects');
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd, { bigint: true });
    demand(before.isFile() && before.size <= BigInt(cap) && (allowEmpty || before.size > 0n), 'retained regular-file bound');
    const bytes = Buffer.alloc(Number(before.size)); let offset = 0;
    while (offset < bytes.length) { const got = fs.readSync(fd, bytes, offset, bytes.length - offset); demand(got > 0, 'retained file shrank'); offset += got; }
    const after = fs.fstatSync(fd, { bigint: true });
    for (const field of ['dev', 'ino', 'size', 'mtimeNs', 'ctimeNs']) demand(before[field] === after[field], 'retained file changed');
    return bytes;
  } finally { fs.closeSync(fd); }
}
function pin(ctx, file, cap = MiB, allowEmpty = false) {
  const bytes = readBounded(file, cap, allowEmpty), value = { path: file, bytes: bytes.length, sha256: sha256(bytes) };
  const prior = ctx.files.get(file);
  if (prior) same(value, prior, 'retained input changed between reads');
  else { demand(ctx.files.size < 768, 'retained-file roster bound'); ctx.files.set(file, value); }
  return bytes;
}
function stream(ctx, batch, stage, name, stem = stage.log_stem) {
  const bytes = pin(ctx, path.join(batch, 'logs', `${stem}.${name}`), MiB, true);
  demand(bytes.length === stage[`${name}_bytes`] && sha256(bytes) === stage[`${name}_sha256`], 'retained stage stream mismatch');
  return bytes;
}
function measure(ctx, file, cap = 512 * MiB) {
  const value = measureNativeBuildInput(file, cap), prior = ctx.measured.get(file);
  if (prior) same(value, prior, 'measured input substitution');
  else { demand(ctx.measured.size < 384, 'measurement roster bound'); ctx.measured.set(file, value); }
  return value;
}
function remeasureDeclared(ctx, values) {
  for (const value of values) {
    const { text: _text, ...expected } = value;
    same(measure(ctx, value.requested, value.bytes), expected, 'stale retained measurement');
  }
}
function checkPin(bytes, hash, length, label) { demand(bytes.length === length && sha256(bytes) === hash, `${label}: exact retained bytes`); }
export function validateSourceExportLine(stderr, artifact) {
  const lines = new TextDecoder('utf-8', { fatal: true }).decode(stderr).split(/\r?\n/).filter(line => line.startsWith('fe2o3 diagnostic extraction:'));
  const expected = `fe2o3 diagnostic extraction: actual Rust -> admitted semantic MIR V32 -> semantic SSA -> pre_ranked_diagnostic exact KIR V17; declared_target=gfx942:xnack-, declared_wave=64, 1 kernel(s), canonical_identity ${artifact.canonical_sha256}, ${artifact.canonical_bytes} byte(s), retained_source_inventory ${artifact.observed_retained_source_inventory}, retained_source_preflight ${artifact.observed_retained_source_preflight}; ranked_checks=false, functional_proof=false, exported_source_authentication=false, exported_compiler_authentication=false, protected_admission=false, artifact/load/launch/hardware_authority=false, source_variable_map=unavailable, physical_register_values=unavailable, instruction_microsteps=unavailable; raw bytes are diagnostic input, not production resume`;
  same(lines, [expected], 'retained genuine source-export observation');
}
function loadSourceBatch(ctx, options) {
  const receipt = parseBoundedJson(pin(ctx, path.join(options.sourceBatch, 'receipt.json')));
  const artifacts = validateSourceReceipt(receipt, options);
  remeasureDeclared(ctx, receipt.inputs_before);
  const stages = new Map(receipt.stages.map(stage => [stage.stage, stage]));
  for (const stage of receipt.stages) {
    stream(ctx, options.sourceBatch, stage, 'stdout'); stream(ctx, options.sourceBatch, stage, 'stderr');
    same(parseBoundedJson(pin(ctx, path.join(options.sourceBatch, 'logs', `${stage.log_stem}.json`))), stage, 'source stage record substitution');
  }
  for (const refusal of receipt.exports.slice(6)) {
    const directory = path.join(options.sourceBatch, refusal.name);
    same(parseBoundedJson(pin(ctx, path.join(directory, 'export-observation.json'))), refusal, 'retained source refusal record');
    demand(!fs.existsSync(path.join(directory, 'kernel.kir')), 'refused source export acquired an output');
    demand(stream(ctx, options.sourceBatch, stages.get(refusal.name + '-export'), 'stderr').toString('utf8').includes(refusal.diagnostic), 'retained source refusal diagnostic');
  }
  for (const control of receipt.controls) {
    const stage = stages.get(control.name);
    demand(stage.stdout_bytes === 0, 'source control emitted unexpected stdout');
    const error = parseBoundedJson(stream(ctx, options.sourceBatch, stage, 'stderr'));
    demand(error.schema === 'fe2o3-simulation-error-v1' && error.status === 'error'
      && error.stage === control.stage && error.kind === control.kind
      && (control.input === null || error.input === control.input), 'retained structured simulator refusal');
  }
  const results = [];
  artifacts.forEach((artifact, variantIndex) => {
    const variant = SOURCE_VARIANTS[variantIndex], bytes = pin(ctx, artifact.kirPath, MAX_REPORT);
    checkPin(bytes, artifact.file_sha256, artifact.file_bytes, 'source KIR');
    demand(bytes.readUInt16LE(8) === 17, 'source KIR wire version');
    same(parseBoundedJson(pin(ctx, path.join(artifact.directory, 'export-observation.json'))), artifact, 'source export record substitution');
    const stage = stages.get(`${variant.name}-export`);
    demand(path.basename(stage.executable) === 'fe2o3-export-sim' && receipt.binaries_before.some(item => item.requested === stage.executable), 'measured exporter command');
    const fixture = path.join(options.repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device');
    same(stage.args, ['--diagnostic-kir-v17', '--crate', 'fe2o3_production_extraction_fixture', '--output', artifact.kirPath,
      '--target', 'gfx942', '--target-dir', receipt.source_target, '--', '--manifest-path', path.join(fixture, 'Cargo.toml'),
      '-p', 'fe2o3-production-extraction-fixture', '--lib', '--no-default-features', '--features', variant.feature, '--offline'], 'actual source command selector');
    validateSourceExportLine(stream(ctx, options.sourceBatch, stage, 'stderr'), artifact);
    const simulations = [];
    for (let i = 0; i < 6; i++) {
      const record = receipt.simulations[variantIndex * 6 + i];
      const requestPath = path.join(artifact.directory, `request-case-${i}.json`), resultPath = path.join(artifact.directory, `sim-case-${i}.json`);
      demand(record.request_path === requestPath && record.result_path === resultPath, 'simulation path substitution');
      const requestBytes = pin(ctx, requestPath), resultBytes = pin(ctx, resultPath);
      checkPin(requestBytes, record.request_sha256, record.request_bytes, 'simulation request');
      checkPin(resultBytes, record.result_sha256, record.result_bytes, 'simulation result');
      const simulated = stages.get(`${variant.name}-sim-${i}`);
      demand(path.basename(simulated.executable) === 'fe2o3-kir-sim' && receipt.binaries_before.some(item => item.requested === simulated.executable)
        && simulated.stdout_bytes === 0 && simulated.stderr_bytes === 0, 'actual simulator command');
      same(simulated.args, ['--diagnostic-kir-v17', artifact.kirPath, '--request', requestPath, '--output', resultPath], 'simulation command binding');
      simulations.push(validateSimulationJoin(record, parseBoundedJson(requestBytes), parseBoundedJson(resultBytes), artifact, variant, i));
    }
    results.push({ artifact, variant, simulations });
  });
  return results;
}
function loadNativeBatch(ctx, options) {
  const receipt = parseBoundedJson(pin(ctx, path.join(options.nativeBatch, 'receipt.json')));
  validateProgramBuildReceipt(receipt, { repo: options.repo, output: options.nativeBatch });
  for (const values of [receipt.inputs, receipt.link_inputs.commands, receipt.link_inputs.libraries, receipt.artifacts]) remeasureDeclared(ctx, values);
  for (const stage of receipt.stages) for (const name of ['stdout', 'stderr']) stream(ctx, options.nativeBatch, stage, name, stage.stage);
  const observationBytes = pin(ctx, path.join(options.nativeBatch, 'observation.json'), MAX_REPORT);
  checkPin(observationBytes, receipt.observation.sha256, receipt.observation.bytes, 'synthetic native qualification');
  parseProgramObservation(observationBytes, receipt.observation.worker_build_claim);
  const claim = pin(ctx, path.join(options.nativeBatch, 'build/fe2o3-worker-build-id.txt'), 256).toString('utf8').trim();
  demand(claim === receipt.observation.worker_build_claim, 'measured worker claim file');
  return { executable: path.join(options.nativeBatch, 'build', TEST_TARGET), workerClaim: claim };
}
function writeNew(file, bytes) { fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 }); }
function jsonNew(file, value) { const bytes = Buffer.from(JSON.stringify(value, null, 2) + '\n'); demand(bytes.length <= 2 * MiB, 'aggregate receipt byte bound'); writeNew(file, bytes); }
function availableRam() {
  const text = fs.readFileSync('/proc/meminfo', 'utf8'); demand(text.length <= MAX_REPORT, 'meminfo byte bound');
  const matches = [...text.matchAll(/^MemAvailable:\s+([0-9]+) kB$/gm)]; demand(matches.length === 1, 'unique MemAvailable');
  const bytes = BigInt(matches[0][1]) * 1024n; demand(bytes >= MIN_RAM_BYTES, 'available RAM below64GiB'); return bytes.toString();
}

export async function runSourceNative(options) {
  const repo = fs.realpathSync(absolute(options.repo)), outputParent = fs.realpathSync(path.dirname(absolute(options.output)));
  demand(repo === options.repo && outputParent === path.dirname(options.output), 'repo/output parent redirects');
  const output = options.output, cacheRoots = options.cacheRoots.map(root => fs.realpathSync(absolute(root)));
  for (const key of ['sourceBatch', 'nativeBatch']) {
    absolute(options[key]); demand(fs.realpathSync(options[key]) === options[key]
      && !inside(options[key], output) && !inside(output, options[key]), 'batch/output overlap or redirect');
  }
  demand(output !== os.homedir() && path.dirname(output) !== output, 'nonbroad output required');
  const filesystem = validateProgramOutputLayout({ repo, output, cacheRoots,
    repoDevice: String(fs.statSync(repo, { bigint: true }).dev), outputParentDevice: String(fs.statSync(outputParent, { bigint: true }).dev) });
  requireDiskReserve(repo); requireDiskReserve(outputParent); availableRam();
  fs.mkdirSync(output, { mode: 0o700 }); for (const name of ['logs', 'tmp']) fs.mkdirSync(path.join(output, name), { mode: 0o700 });
  const ctx = { files: new Map(), measured: new Map() };
  const receipt = { schema: 'fe2o3-ordered-program-source-native-observation-v1', status: 'failed',
    authority: 'joined-executed-process-observations-not-authentication', executed_source_export_ancestry_observed: false,
    native_report_source_ancestry: 'not-established-by-llvm-file', runtime_closure_attestation: 'unavailable',
    final_payload_retention: 'digest-and-decoded-observations-only', whole_kernel_order_or_byte_stability_claim: false,
    physical_register_allocation_or_lifetime_proof: false, ...Object.fromEntries(falses.map(key => [key, false])),
    repo, output, source_batch: options.sourceBatch, native_batch: options.nativeBatch, emitter: options.emitter,
    filesystem, cache_roots: cacheRoots, limits: { minimum_free_bytes: MIN_FREE_BYTES.toString(),
      minimum_available_ram_bytes: MIN_RAM_BYTES.toString(), maximum_combined_cache_bytes: MAX_CACHE_BYTES.toString(),
      emitter_ms: 30000, native_ms: 120000, stdout_bytes: MAX_REPORT, stderr_bytes: 128 * 1024 },
    stages: [], cache_observations: [], joins: [] };
  let activeStage = 'validate-retained-inputs';
  try {
    const source = loadSourceBatch(ctx, { ...options, repo }), native = loadNativeBatch(ctx, { ...options, repo });
    const emitter = measure(ctx, absolute(options.emitter));
    demand(path.basename(emitter.resolved) === 'lower_diagnostic_ordered_program_v17', 'exact emitter binary name');
    for (const file of [fileURLToPath(import.meta.url), path.join(HERE, 'ordered-program-source-native.test.mjs'),
      path.join(HERE, 'ordered-program-worker-prototype.mjs'), path.join(HERE, 'assembly-region-worker-prototype.mjs'),
      path.join(repo, 'crates/fe2o3-amdgcn-model/examples/lower_diagnostic_ordered_program_v17.rs'), process.execPath, '/usr/bin/du']) measure(ctx, file);
    const env = { PATH: '/usr/local/bin:/usr/bin:/bin', LANG: 'C', LC_ALL: 'C', HOME: os.homedir(), TMPDIR: path.join(output, 'tmp') };
    async function cache(stage) {
      demand(options.cacheRoots.every((root, i) => fs.realpathSync(root) === cacheRoots[i]), 'cache root changed');
      requireDiskReserve(repo); availableRam();
      const result = await runBoundedCommand({ executable: '/usr/bin/du', args: ['-sb', '--', ...cacheRoots], cwd: repo, env,
        timeoutMs: 30000, stdoutCap: 16 * 1024, stderrCap: 4096, diskDirectory: repo });
      writeNew(path.join(output, 'logs', `${stage}.stdout`), result.stdout); writeNew(path.join(output, 'logs', `${stage}.stderr`), result.stderr);
      demand(result.code === 0 && result.signal === null && result.reason === null && result.stderr.length === 0, 'cache measurement failed');
      receipt.cache_observations.push({ stage, elapsed_ms: result.elapsed_ms, values: parseCacheObservation(result.stdout, cacheRoots),
        stdout_sha256: sha256(result.stdout), stdout_bytes: result.stdout.length });
    }
    async function command(stage, executable, args, timeoutMs) {
      activeStage = stage; await cache(`${stage}-cache-before`);
      const free = requireDiskReserve(repo), ram = availableRam();
      const result = await runBoundedCommand({ executable, args, cwd: repo, env, timeoutMs,
        stdoutCap: MAX_REPORT, stderrCap: 128 * 1024, diskDirectory: repo });
      for (const name of ['stdout', 'stderr']) writeNew(path.join(output, 'logs', `${stage}.${name}`), result[name]);
      receipt.stages.push({ stage, executable, args, code: result.code, signal: result.signal, reason: result.reason,
        elapsed_ms: result.elapsed_ms, free_bytes_before: free.toString(), available_ram_bytes_before: ram,
        stdout_sha256: sha256(result.stdout), stdout_bytes: result.stdout.length,
        stderr_sha256: sha256(result.stderr), stderr_bytes: result.stderr.length });
      demand(result.code === 0 && result.signal === null && result.reason === null && result.stderr.length === 0, `${stage}: child did not finish cleanly`);
      requireDiskReserve(repo); availableRam(); await cache(`${stage}-cache-after`); return result.stdout;
    }
    for (const { artifact, variant, simulations } of source) {
      const directory = path.join(output, variant.name); fs.mkdirSync(directory, { mode: 0o700 });
      const llvmPath = path.join(directory, 'kernel.ll');
      const emitterBytes = await command(`${variant.name}-emit`, emitter.resolved, [artifact.kirPath, llvmPath], 30000);
      const llvmBytes = pin(ctx, llvmPath, MAX_REPORT), emission = validateEmission(parseBoundedJson(emitterBytes, MAX_REPORT), artifact, variant, llvmBytes);
      writeNew(path.join(directory, 'emission.json'), emitterBytes);
      const machineBytes = await command(`${variant.name}-native`, native.executable, [variant.profile, variant.used ? 'used' : 'unused', llvmPath], 120000);
      const machine = validateMachineObservation(parseBoundedJson(machineBytes, MAX_REPORT), emission, variant, native.workerClaim);
      writeNew(path.join(directory, 'native-observation.json'), machineBytes);
      checkPin(pin(ctx, artifact.kirPath, MAX_REPORT), artifact.file_sha256, artifact.file_bytes, 'unchanged source KIR');
      checkPin(pin(ctx, llvmPath, MAX_REPORT), emission.llvm_sha256, emission.llvm_bytes, 'unchanged emitted LLVM');
      receipt.joins.push({ name: variant.name, profile: variant.profile, result_used: variant.used, source_export: artifact,
        declared_descriptors: emission.descriptors, declared_register_plan: emission.register_plan, program_count: emission.program_count,
        logical_cpu_cases: simulations, emission_sha256: sha256(emitterBytes), emission_bytes: emitterBytes.length,
        llvm_path: llvmPath, llvm_sha256: emission.llvm_sha256, llvm_bytes: emission.llvm_bytes,
        native_observation_sha256: sha256(machineBytes), native_observation_bytes: machineBytes.length,
        native_kernel_symbol: machine.kernel_symbol, native_cases: machine.cases });
    }
    activeStage = 'remeasure-all-inputs';
    requireNativeBuildInputsUnchanged([...ctx.measured.values()]);
    for (const item of ctx.files.values()) checkPin(readBounded(item.path, Math.max(item.bytes, 1), true), item.sha256, item.bytes, 'retained input remeasurement');
    receipt.executed_source_export_ancestry_observed = true;
    receipt.counts = { source_exports_joined: receipt.joins.length, retained_cpu_cases_checked: receipt.joins.reduce((n, item) => n + item.logical_cpu_cases.length, 0),
      emitter_subprocesses: receipt.stages.filter(item => item.stage.endsWith('-emit')).length,
      native_subprocesses: receipt.stages.filter(item => item.stage.endsWith('-native')).length,
      observation_subprocesses: receipt.stages.length, cache_subprocesses: receipt.cache_observations.length,
      native_compilation_cases: receipt.joins.reduce((n, item) => n + item.native_cases.length, 0),
      authored_instruction_sites: receipt.joins.reduce((n, item) => n + item.native_cases.reduce((m, entry) => m + entry.program.length, 0), 0) };
    same(receipt.counts, { source_exports_joined: 6, retained_cpu_cases_checked: 36, emitter_subprocesses: 6, native_subprocesses: 6,
      observation_subprocesses: 12, cache_subprocesses: 24, native_compilation_cases: 12, authored_instruction_sites: 80 }, 'aggregate actual coverage');
    receipt.retained_files = [...ctx.files.values()]; receipt.measured_inputs = [...ctx.measured.values()];
    requireDiskReserve(repo); availableRam(); receipt.status = 'passed'; jsonNew(path.join(output, 'receipt.json'), receipt); return receipt;
  } catch (error) {
    receipt.failure = { stage: activeStage, message: String(error?.message ?? error).slice(0, 4096) };
    try { jsonNew(path.join(output, 'failure.json'), receipt); } catch { /* Original failure remains primary; no destructive cleanup. */ }
    throw error;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length === 3 && process.argv[2] === '--help') console.log('Usage: node scripts/ordered-program-source-native.mjs --source-batch ABS --native-batch ABS --emitter ABS --output NEW_ABS --cargo-cache-root ABS --secondary-cache-root ABS [--compiler-repo ABS]\nClosed six-variant retained-source observation join; no builds, protected admission or GPU execution.');
  else Promise.resolve().then(() => runSourceNative(parseArguments(process.argv.slice(2)))).then(receipt => {
    console.log(JSON.stringify({ status: receipt.status, output: receipt.output, counts: receipt.counts, authority: receipt.authority }));
  }).catch(error => { console.error(String(error?.message ?? error).slice(0, 4096)); process.exitCode = 1; });
}
