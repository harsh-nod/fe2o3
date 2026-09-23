#!/usr/bin/env node
// Four fresh typed-owner LLVM observations; retained simulation is not rerun.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';
import { parseExport } from './source-promotion-instruction-edit-smoke.mjs';
import { ORIGIN_BYTES_V1, originOutputPathV1, repeatExportArgumentsV1,
  validateRepeatExportProfileV1, validateRepeatOriginV1 } from './ordered-repeat-origin-v1.mjs';
import {
  LIMITS as SOURCE_LIMITS, LABELS, REPETITIONS, INPUTS, LENGTHS, REFUSALS,
  parseJson, validateSources, validateMatrix, validateSimulation, validateRefusal,
  declaredProgram, request, negativeSource,
} from './ordered-repeat-source-smoke.mjs';

const KiB = 1024, MiB = 1024 * KiB;
export const LIMITS = Object.freeze({ calls: 4, command_ms: 60000, wall_ms: 270000,
  stream_bytes: 4096, llvm_bytes: 64 * KiB, receipt_bytes: MiB,
  output_bytes: 64 * MiB, output_files: 32, historical_pins: 512,
  historical_pin_bytes: 2 * 1024 * MiB, selected_file_bytes: 512 * MiB,
  all_pins: 560, all_pin_bytes: 3 * 1024 * MiB });
export const FALSE_FIELDS = Object.freeze(['source_authentication', 'compiler_closure_attestation',
  'proof_authority', 'protected_admission', 'final_artifact_authority', 'production_resume',
  'physical_register_values', 'hardware_execution']);
const SOURCE_FALSE = ['runtime_loop_or_schedule_added', 'native_qualified', 'source_authentication',
  'compiler_closure_attestation', 'protected_proof', 'production_resume', 'hardware_observed',
  'physical_register_lifetime_proof', 'milestone_completion'];
const PIN_KEYS = ['path', 'bytes', 'sha256', 'device', 'inode', 'mode', 'nlink', 'mtime_ns', 'ctime_ns'];
const STAGE_KEYS = ['label', 'executable', 'args', 'code', 'signal', 'reason', 'elapsed_ms',
  'stdout_bytes', 'stdout_sha256', 'stderr_bytes', 'stderr_sha256'];
export const sha256 = bytes => createHash('sha256').update(bytes).digest('hex');
const text = bytes => new TextDecoder('utf-8', { fatal: true }).decode(bytes);
const exact = (value, keys) => {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value));
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'closed object fields');
};
function integer(value, cap, minimum = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= minimum && value <= cap, 'bounded integer'); return value;
}
function digest(value) {
  assert.equal(typeof value, 'string'); assert.match(value, /^[0-9a-f]{64}$/u);
  assert.notEqual(value, '0'.repeat(64)); return value;
}
function absolute(value) {
  assert.equal(typeof value, 'string'); assert.ok(path.isAbsolute(value) && path.resolve(value) === value);
  assert.ok(Buffer.byteLength(value) <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value)); return value;
}
function inside(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (!path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('../'));
}
function pinShape(pin) {
  exact(pin, PIN_KEYS); absolute(pin.path); integer(pin.bytes, LIMITS.selected_file_bytes); digest(pin.sha256);
  for (const field of PIN_KEYS.slice(3)) {
    assert.equal(typeof pin[field], 'string'); assert.match(pin[field], /^(?:0|[1-9][0-9]{0,23})$/u);
  }
}
export function expectedPin(actual, expected) {
  pinShape(actual); pinShape(expected); assert.deepEqual(actual, expected, 'unchanged selected file identity');
}
export function externalIdentity(bytes, expectedBytes, expectedSha) {
  assert.ok(Buffer.isBuffer(bytes)); integer(expectedBytes, LIMITS.selected_file_bytes, 1); digest(expectedSha);
  assert.equal(bytes.length, expectedBytes); assert.equal(sha256(bytes), expectedSha, 'external byte identity');
}

export function validateCaptureHeader(value) {
  exact(value, ['schema', 'status', 'authority', 'origin', 'successful_exports', 'exact_frontend_refusals',
    'whole_kernel_simulations', 'variants', 'refusals', 'stages', 'retained_file_pins', 'retained_pin_bytes',
    'limits', 'bundle_identity', 'compiler_identity_source', 'inventory_semantics',
    'declared_instruction_count', ...SOURCE_FALSE, 'task_resource_accounting']);
  assert.equal(value.schema, 'task-ordered-repeat-source-acceptance-v1');
  assert.equal(value.status, 'passed'); assert.equal(value.authority, 'observation_only');
  assert.equal(value.origin, 'fresh_rust_normal_exporter_v17_inspector_and_cpu_simulator');
  assert.equal(value.successful_exports, 4); assert.equal(value.exact_frontend_refusals, 8);
  assert.equal(value.whole_kernel_simulations, 120);
  assert.deepEqual(value.limits, SOURCE_LIMITS, 'exact R2 source-driver limits, not historical R1');
  assert.equal(value.bundle_identity, 'unavailable_this_normal_export_route_is_raw_diagnostic_kir_v17_not_bundle_v6');
  assert.equal(value.compiler_identity_source, 'normal_live_exporter_never_inferred_from_preflight_or_file_hash');
  assert.equal(value.inventory_semantics, 'retained_root_contract_census_not_body_digest_repeat_requires_exact_equality');
  assert.equal(value.declared_instruction_count, '2_3_16_steps_one_ordered_region_whole_region_logical_observation');
  assert.equal(value.task_resource_accounting, 'external_current_scope_supervisor_and_complete_input_census_required');
  for (const field of SOURCE_FALSE) assert.equal(value[field], false, field);
  for (const [key, count] of [['variants', 4], ['refusals', 8], ['stages', 136]]) {
    assert.ok(Array.isArray(value[key])); assert.equal(value[key].length, count);
  }
  assert.ok(Array.isArray(value.retained_file_pins) && value.retained_file_pins.length > 0
    && value.retained_file_pins.length <= LIMITS.historical_pins);
  const seen = new Set(); let total = 0;
  for (const pin of value.retained_file_pins) {
    pinShape(pin); assert.ok(!seen.has(pin.path), 'duplicate historical pin'); seen.add(pin.path);
    total += pin.bytes; integer(total, LIMITS.historical_pin_bytes);
  }
  assert.equal(value.retained_pin_bytes, total);
}
export function stageLabels() {
  const labels = [];
  for (const label of LABELS) {
    labels.push(label + '-export', label + '-inspect');
    for (let input = 0; input < INPUTS.length; input++) for (const length of LENGTHS) {
      for (let replay = 0; replay < 2; replay++) labels.push(label + '-case-' + input + '-length-' + length + '-replay-' + replay);
    }
  }
  return [...labels, ...REFUSALS.map(item => 'refuse-' + item.label + '-export')];
}
export function validateCapture(value, directory) {
  absolute(directory); validateCaptureHeader(value); validateMatrix(value.variants);
  const pins = new Map(value.retained_file_pins.map(pin => [pin.path, pin]));
  const needed = (file, cap) => {
    absolute(file); const pin = pins.get(file); assert.ok(pin, 'required source-capture pin');
    integer(pin.bytes, cap, 1); return pin;
  };
  value.variants.forEach((variant, index) => {
    exact(variant, ['label', 'repetitions', 'source_path', 'source_sha256', 'kir_path',
      'kir_file_sha256', 'exported', 'inspection', 'simulations']);
    const label = LABELS[index], sourceLabel = LABELS[Math.min(index, 2)];
    assert.equal(variant.source_path, path.join(directory, sourceLabel + '-source/src/lib.rs'));
    assert.equal(variant.kir_path, path.join(directory, label + '.kir'));
    assert.equal(needed(variant.source_path, SOURCE_LIMITS.source_bytes).sha256, variant.source_sha256);
    const kir = needed(variant.kir_path, SOURCE_LIMITS.kir_bytes);
    assert.equal(kir.sha256, variant.kir_file_sha256); assert.equal(kir.bytes, variant.exported.canonical_bytes);
    exact(variant.exported, ['canonical_sha256', 'canonical_bytes', 'retained_source_inventory',
      'retained_source_preflight', 'semantic_identity']);
  });
  const labels = stageLabels(); assert.equal(labels.length, 136);
  value.stages.forEach((stage, index) => {
    exact(stage, STAGE_KEYS); assert.equal(stage.label, labels[index]); absolute(stage.executable);
    assert.ok(Array.isArray(stage.args) && stage.args.length <= 32);
    stage.args.forEach(arg => assert.ok(typeof arg === 'string' && Buffer.byteLength(arg) <= 4096 && !arg.includes('\0')));
    assert.equal(stage.code, index >= 128 ? 1 : 0); assert.equal(stage.signal, null); assert.equal(stage.reason, null);
    integer(stage.elapsed_ms, SOURCE_LIMITS.wall_ms);
    needed(stage.executable, LIMITS.selected_file_bytes);
    for (const stream of ['stdout', 'stderr']) {
      const pin = pins.get(path.join(directory, stage.label + '.' + stream)); assert.ok(pin);
      assert.equal(pin.bytes, stage[stream + '_bytes']); assert.equal(pin.sha256, stage[stream + '_sha256']);
      integer(pin.bytes, SOURCE_LIMITS.command_stream_bytes);
    }
  });
  value.refusals.forEach((refusal, index) => {
    const expected = REFUSALS[index];
    exact(refusal, ['label', 'kind', 'diagnostic', 'source_path', 'cargo_exit', 'exporter_exit', 'kir_absent', 'source_sha256']);
    assert.equal(refusal.label, expected.label); assert.equal(refusal.kind, expected.kind);
    assert.equal(refusal.diagnostic, expected.message); assert.equal(refusal.cargo_exit, 101);
    assert.equal(refusal.exporter_exit, 1); assert.equal(refusal.kir_absent, true);
    assert.equal(refusal.source_path, path.join(directory, 'refuse-' + expected.label + '-source/src/lib.rs'));
    assert.equal(needed(refusal.source_path, SOURCE_LIMITS.source_bytes).sha256, refusal.source_sha256);
  });
  return pins;
}

export function validateLowererReport(report, variant, kir, llvm) {
  exact(report, ['kind', 'authority', 'canonical_wire_version', 'canonical_identity', 'canonical_bytes',
    'input_file_sha256', 'llvm_sha256', 'llvm_bytes', 'program_count', 'descriptors', 'register_plan',
    'canonical_retained_storage_bytes', 'canonical_work_limit', 'canonical_storage_limit',
    'max_input_bytes', 'max_published_llvm_bytes', 'emitter_text_limit_bytes',
    'canonical_and_emitter_accounting_are_separate', ...FALSE_FIELDS]);
  assert.ok(Buffer.isBuffer(kir) && kir.length > 0 && kir.length <= SOURCE_LIMITS.kir_bytes);
  assert.ok(Buffer.isBuffer(llvm) && llvm.length > 0 && llvm.length <= LIMITS.llvm_bytes);
  assert.equal(report.kind, 'diagnostic_ordered_program_llvm_observation');
  assert.equal(report.authority, 'observation_only'); assert.equal(report.canonical_wire_version, 17);
  digest(report.canonical_identity); assert.equal(report.canonical_identity, variant.exported.canonical_sha256);
  assert.equal(report.canonical_bytes, variant.exported.canonical_bytes); assert.equal(report.canonical_bytes, kir.length);
  assert.equal(report.input_file_sha256, variant.kir_file_sha256); assert.equal(report.input_file_sha256, sha256(kir));
  assert.equal(report.llvm_sha256, sha256(llvm)); assert.equal(report.llvm_bytes, llvm.length);
  const program = declaredProgram(variant.repetitions);
  assert.equal(report.program_count, program.count); assert.deepEqual(report.descriptors, program.descriptors);
  assert.deepEqual(report.register_plan, [32, 33, 34, 35, 36]);
  integer(report.canonical_retained_storage_bytes, 64 * MiB, 1);
  assert.equal(report.canonical_work_limit, 1 << 26); assert.equal(report.canonical_storage_limit, 64 * MiB);
  assert.equal(report.max_input_bytes, 64 * KiB); assert.equal(report.max_published_llvm_bytes, 64 * KiB);
  assert.equal(report.emitter_text_limit_bytes, 16 * MiB);
  assert.equal(report.canonical_and_emitter_accounting_are_separate, true);
  for (const field of FALSE_FIELDS) assert.equal(report[field], false);
}
export function validateLlvm(bytes, variant) {
  assert.ok(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= LIMITS.llvm_bytes);
  declaredProgram(variant.repetitions); // Bound repetition before allocating its text roster.
  const llvm = text(bytes); assert.ok(!llvm.includes('\0') && !llvm.includes('\r'));
  const lines = llvm.split('\n'), result = variant.inspection.result_value_id;
  integer(result, 8192); const value = '%v' + result;
  const program = ['v_mov_b32_e32 $0, $1', ...Array(variant.repetitions).fill('v_add_u32_e32 $0, $0, $2')].join('\\0A\\09');
  const expected = '  ' + value + ' = call i32 asm sideeffect "' + program +
    '", "=&{v33},{v34},{v35},{v36},~{v32}"(i32 %arg1, i32 %arg2, i32 %arg3)';
  assert.deepEqual(lines.filter(line => /\basm\b/u.test(line)), [expected],
    'exact sole asm, full authored order, constraints and original direct arguments');
  assert.deepEqual(lines.filter(line => line.startsWith('define ')),
    ['define amdgpu_kernel void @ordered_repeat_u32(ptr addrspace(1) %arg0.data, i64 %arg0.len, i32 %arg1, i32 %arg2, i32 %arg3) #0 !reqd_work_group_size !0 {']);
  assert.deepEqual(lines.filter(line => line.startsWith('target triple')),
    ['target triple = "amdgcn-amd-amdhsa"']);
  assert.deepEqual(lines.filter(line => line.includes('; ordered-program-v17')),
    ['  ; ordered-program-v17 vgpr-high-water=37 (binding extent; final descriptor unverified)']);
  assert.deepEqual(lines.filter(line => line.startsWith('!0 =')), ['!0 = !{i32 64, i32 1, i32 1}']);
  const attributes = lines.filter(line => line.startsWith('attributes #0 ='));
  assert.equal(attributes.length, 1);
  for (const literal of ['"amdgpu-flat-work-group-size"="64,64"',
    '"target-features"="-wavefrontsize32,+wavefrontsize64,-xnack"', '"target-cpu"="gfx942"']) {
    assert.equal(attributes[0].split(literal).length - 1, 1);
  }
  // Bind the real source marker result to the ordinary emitted store. This is
  // an exact emitter-text observation, not a general LLVM verifier/dataflow proof.
  const stores = lines.filter(line => /^\s*store\b/u.test(line)); assert.equal(stores.length, 1);
  assert.match(stores[0], new RegExp('^  store i32 ' + value +
    ', ptr addrspace\\(1\\) %v[0-9]+, align 4$'));
  return { result_ssa: value, assembly_line: expected, instruction_count: variant.repetitions + 1,
    constraints: '=&{v33},{v34},{v35},{v36},~{v32}', result_store_line: stores[0] };
}
export function validateJoinedVariants(variants) {
  assert.ok(Array.isArray(variants) && variants.length === 4);
  assert.deepEqual(variants.map(item => item.label), LABELS);
  assert.deepEqual(variants.map(item => item.repetitions), REPETITIONS);
  for (const value of variants) {
    digest(value.llvm_sha256); integer(value.llvm_bytes, LIMITS.llvm_bytes, 1);
    assert.equal(value.observed.instruction_count, value.repetitions + 1);
  }
  assert.equal(new Set(variants.slice(0, 3).map(item => item.llvm_sha256)).size, 3);
  assert.equal(variants[2].llvm_sha256, variants[3].llvm_sha256);
  assert.equal(variants[2].llvm_bytes, variants[3].llvm_bytes);
  assert.deepEqual(variants[2].report, variants[3].report, 'repeat actual lowerer report equality');
  assert.deepEqual(variants[2].observed, variants[3].observed);
}
export function healthy(result) {
  assert.equal(result.code, 0); assert.equal(result.reason, null); assert.equal(result.signal, null);
  for (const key of ['stdout', 'stderr']) assert.ok(Buffer.isBuffer(result[key]) && result[key].length <= LIMITS.stream_bytes);
  assert.ok(result.stdout.length > 0); assert.equal(result.stderr.length, 0, 'no unexpected lowerer diagnostic');
  integer(result.elapsed_ms, LIMITS.command_ms);
}
export function options(argv) {
  const keys = ['repo', 'receipt', 'receipt-bytes', 'receipt-sha256', 'lowerer', 'lowerer-bytes', 'lowerer-sha256', 'output'];
  assert.equal(argv.length, keys.length * 2); const opt = {};
  for (let i = 0; i < argv.length; i += 2) {
    assert.ok(argv[i].startsWith('--')); const key = argv[i].slice(2);
    assert.ok(keys.includes(key) && !Object.hasOwn(opt, key)); opt[key] = argv[i + 1];
  }
  for (const key of ['repo', 'receipt', 'lowerer', 'output']) absolute(opt[key]);
  for (const key of ['receipt-bytes', 'lowerer-bytes']) {
    assert.match(opt[key], /^[1-9][0-9]{0,9}$/u); opt[key] = integer(Number(opt[key]), LIMITS.selected_file_bytes, 1);
  }
  integer(opt['receipt-bytes'], LIMITS.receipt_bytes, 1);
  for (const key of ['receipt-sha256', 'lowerer-sha256']) digest(opt[key]);
  assert.equal(path.basename(opt.receipt), 'receipt.json');
  for (const input of [opt.repo, path.dirname(opt.receipt), opt.lowerer]) {
    assert.ok(!inside(input, opt.output) && !inside(opt.output, input), 'separate exclusive output tree');
  }
  return opt;
}

// Only the CLI performs file/process work. Selected-file custody is not an
// attestation of every dynamic library, compiler build input or privileged peer.
function observe(file, cap, keep = false, empty = false) {
  absolute(file); assert.equal(fs.realpathSync(file), file);
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd, { bigint: true });
    assert.ok(before.isFile() && before.size <= BigInt(cap) && (before.size > 0n || empty));
    const scratch = Buffer.alloc(64 * KiB), hash = createHash('sha256'), chunks = []; let size = 0;
    for (;;) {
      const n = fs.readSync(fd, scratch, 0, scratch.length, null); if (!n) break;
      size += n; assert.ok(size <= cap); hash.update(scratch.subarray(0, n));
      if (keep) chunks.push(Buffer.from(scratch.subarray(0, n)));
    }
    const after = fs.fstatSync(fd, { bigint: true }), named = fs.lstatSync(file, { bigint: true });
    for (const key of ['dev', 'ino', 'mode', 'nlink', 'size', 'mtimeNs', 'ctimeNs']) {
      assert.equal(after[key], before[key]); assert.equal(named[key], before[key]);
    }
    assert.equal(BigInt(size), before.size);
    return { pin: { path: file, bytes: size, sha256: hash.digest('hex'), device: String(before.dev),
      inode: String(before.ino), mode: String(before.mode), nlink: String(before.nlink),
      mtime_ns: String(before.mtimeNs), ctime_ns: String(before.ctimeNs) },
      bytes: keep ? Buffer.concat(chunks) : undefined };
  } finally { fs.closeSync(fd); }
}
function absent(file) {
  try { fs.lstatSync(file); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; }
}
async function run(opt) {
  const start = performance.now(), pins = new Map(), stages = [], variants = [];
  let totalPinBytes = 0, outputCreated = false;
  for (const dir of [opt.repo, path.dirname(opt.receipt), path.dirname(opt.output)]) assert.equal(fs.realpathSync(dir), dir);
  const guard = () => {
    assert.ok(performance.now() - start <= LIMITS.wall_ms, '270s cooperative deadline under5min outer');
    if (!outputCreated) return;
    assert.equal(fs.realpathSync(opt.output), opt.output);
    const names = fs.readdirSync(opt.output); assert.ok(names.length <= LIMITS.output_files); let total = 0;
    for (const name of names) {
      const stat = fs.lstatSync(path.join(opt.output, name)); assert.ok(stat.isFile() && !stat.isSymbolicLink());
      total += stat.size; integer(total, LIMITS.output_bytes);
    }
  };
  const pin = (file, cap, keep = false, empty = false, expected = null) => {
    guard(); const got = observe(file, cap, keep, empty), prior = pins.get(file);
    if (expected) expectedPin(got.pin, expected);
    if (prior) expectedPin(got.pin, prior);
    else {
      assert.ok(pins.size < LIMITS.all_pins); totalPinBytes += got.pin.bytes;
      integer(totalPinBytes, LIMITS.all_pin_bytes); pins.set(file, got.pin);
    }
    return got;
  };
  const save = (name, bytes) => {
    guard(); assert.ok(Buffer.isBuffer(bytes) && bytes.length <= LIMITS.receipt_bytes);
    assert.equal(path.basename(name), name);
    const file = path.join(opt.output, name); fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 });
    pin(file, Math.max(bytes.length, 1), false, bytes.length === 0); guard(); return file;
  };
  const json = (name, value) => save(name, Buffer.from(JSON.stringify(value, null, 2) + '\n'));
  try {
    const retainedReceipt = pin(opt.receipt, LIMITS.receipt_bytes, true);
    externalIdentity(retainedReceipt.bytes, opt['receipt-bytes'], opt['receipt-sha256']);
    const capture = parseJson(retainedReceipt.bytes), directory = path.dirname(opt.receipt);
    const historical = validateCapture(capture, directory);
    for (const expected of historical.values()) pin(expected.path, Math.max(expected.bytes, 1), false, expected.bytes === 0, expected);
    const retained = (file, cap = SOURCE_LIMITS.json_bytes) => {
      const expected = historical.get(file); assert.ok(expected, 'capture must pin every consumed raw leaf');
      return pin(file, cap, true, expected.bytes === 0, expected).bytes;
    };
    const sources = capture.variants.slice(0, 3).map(v => retained(v.source_path, SOURCE_LIMITS.source_bytes));
    validateSources(sources);
    const stageMap = new Map(capture.stages.map(stage => [stage.label, stage]));
    const outputOf = label => {
      const stage = stageMap.get(label); assert.ok(stage);
      return { ...stage, stdout: retained(path.join(directory, label + '.stdout')),
        stderr: retained(path.join(directory, label + '.stderr')) };
    };
    const exportProfile = validateRepeatExportProfileV1(capture, directory);
    const exportArguments = (label, source, negative) =>
      repeatExportArgumentsV1(exportProfile, directory, label, source, negative);
    for (let index = 0; index < LABELS.length; index++) {
      const variant = capture.variants[index], label = variant.label;
      const exported = outputOf(label + '-export');
      assert.equal(path.basename(exported.executable), 'fe2o3-export-sim');
      assert.deepEqual(exported.args, exportArguments(label, variant.source_path, false));
      assert.deepEqual(parseExport(exported.stderr, 'required'), variant.exported);
      const inspected = outputOf(label + '-inspect');
      assert.equal(path.basename(inspected.executable), 'fe2o3-program-inspect');
      assert.deepEqual(parseJson(inspected.stdout), variant.inspection);
      if (exportProfile === 'origin-v1') validateRepeatOriginV1(
        retained(originOutputPathV1(directory, label), ORIGIN_BYTES_V1), variant.exported,
        variant.inspection, retained(variant.kir_path, SOURCE_LIMITS.kir_bytes));
      assert.deepEqual(inspected.args, [variant.kir_path, path.join(directory, label + '-inspect-request.json')]);
      assert.deepEqual(parseJson(retained(inspected.args[1])), request(INPUTS[0], 1));
      for (let input = 0; input < INPUTS.length; input++) for (const elements of LENGTHS) {
        const stem = label + '-case-' + input + '-length-' + elements;
        const requestPath = path.join(directory, stem + '.request.json');
        const query = parseJson(retained(requestPath)); assert.deepEqual(query, request(INPUTS[input], elements));
        for (let replay = 0; replay < 2; replay++) {
          const sim = outputOf(stem + '-replay-' + replay);
          assert.deepEqual(sim.args, ['--diagnostic-kir-v17', variant.kir_path, '--request', requestPath]);
          assert.equal(path.basename(sim.executable), 'fe2o3-kir-sim');
          validateSimulation(parseJson(sim.stdout), query, variant.exported, variant.repetitions, INPUTS[input], elements);
        }
      }
    }
    for (const refusal of capture.refusals) {
      const label = 'refuse-' + refusal.label, source = retained(refusal.source_path, SOURCE_LIMITS.source_bytes);
      assert.deepEqual(source, negativeSource(sources[0], refusal.label));
      const expected = { ...refusal }; delete expected.source_sha256;
      const exported = outputOf(label + '-export');
      assert.equal(path.basename(exported.executable), 'fe2o3-export-sim');
      assert.deepEqual(exported.args, exportArguments(label, refusal.source_path, true));
      assert.deepEqual(validateRefusal(exported, refusal.label, refusal.source_path,
        !absent(path.join(directory, label + '.kir'))), expected);
      assert.ok(absent(originOutputPathV1(directory, label)), 'negative origin remains absent');
    }
    const lowerer = pin(opt.lowerer, LIMITS.selected_file_bytes);
    assert.equal(lowerer.pin.bytes, opt['lowerer-bytes']); assert.equal(lowerer.pin.sha256, opt['lowerer-sha256']);
    assert.ok((BigInt(lowerer.pin.mode) & 0o111n) !== 0n, 'executable lowerer');
    pin(process.execPath, LIMITS.selected_file_bytes);
    const here = path.dirname(fileURLToPath(import.meta.url));
    assert.equal(here, path.join(opt.repo, 'scripts'), 'one current checkout owns imported validators and lowerer sources');
    for (const name of [path.basename(fileURLToPath(import.meta.url)), 'ordered-repeat-source-smoke.mjs',
      'authoring-navigation-v1-process.mjs', 'source-promotion-instruction-edit-smoke.mjs', 'ordered-repeat-origin-v1.mjs',
      'ordered-program-source-native.mjs', 'ordered-program-worker-prototype.mjs', 'assembly-region-worker-prototype.mjs']) {
      pin(path.join(here, name), MiB);
    }
    for (const name of ['Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml',
      'crates/fe2o3-amdgcn-model/Cargo.toml', 'crates/fe2o3-amdgcn-model/src/lowering.rs',
      'crates/fe2o3-amdgcn-model/src/lowering/ordered_program_v17.rs',
      'crates/fe2o3-amdgcn-model/examples/lower_diagnostic_ordered_program_v17.rs']) pin(path.join(opt.repo, name), 8 * MiB);
    // The imported R2 producer must be one of the actually retained capture inputs.
    assert.ok(historical.has(path.join(here, 'ordered-repeat-source-smoke.mjs')));
    if (exportProfile === 'origin-v1') assert.ok(historical.has(path.join(here, 'ordered-repeat-origin-v1.mjs')));
    assert.ok(absent(opt.output)); fs.mkdirSync(opt.output, { mode: 0o700 }); outputCreated = true;
    const env = { ...process.env }; delete env.LD_PRELOAD;
    for (const key of Object.keys(env)) if (key.startsWith('FE2O3_')) delete env[key];
    for (const variant of capture.variants) {
      guard(); assert.ok(stages.length < LIMITS.calls);
      const llvmPath = path.join(opt.output, variant.label + '.ll');
      assert.ok(absent(llvmPath)); const kir = retained(variant.kir_path, SOURCE_LIMITS.kir_bytes);
      pin(opt.lowerer, LIMITS.selected_file_bytes);
      const args = [variant.kir_path, llvmPath];
      const result = await runNavigationCommand({ executable: opt.lowerer, args, cwd: opt.repo, env,
        timeoutMs: LIMITS.command_ms, outputCap: LIMITS.stream_bytes, guard });
      save(variant.label + '.stdout', result.stdout); save(variant.label + '.stderr', result.stderr);
      stages.push({ label: variant.label, executable: opt.lowerer, args, code: result.code,
        signal: result.signal, reason: result.reason, elapsed_ms: result.elapsed_ms,
        stdout_bytes: result.stdout.length, stdout_sha256: sha256(result.stdout),
        stderr_bytes: result.stderr.length, stderr_sha256: sha256(result.stderr) });
      healthy(result); pin(opt.lowerer, LIMITS.selected_file_bytes);
      retained(variant.kir_path, SOURCE_LIMITS.kir_bytes);
      const llvm = pin(llvmPath, LIMITS.llvm_bytes, true).bytes, report = parseJson(result.stdout);
      validateLowererReport(report, variant, kir, llvm);
      const observed = validateLlvm(llvm, variant);
      variants.push({ label: variant.label, repetitions: variant.repetitions,
        source_path: variant.source_path, source_sha256: variant.source_sha256,
        semantic_identity: variant.exported.semantic_identity, retained_source_preflight: variant.exported.retained_source_preflight,
        retained_source_inventory: variant.exported.retained_source_inventory,
        kir_path: variant.kir_path, kir_file_sha256: variant.kir_file_sha256,
        canonical_identity: variant.exported.canonical_sha256, llvm_path: llvmPath,
        llvm_bytes: llvm.length, llvm_sha256: sha256(llvm), report, observed });
    }
    validateJoinedVariants(variants); assert.equal(stages.length, 4);
    assert.deepEqual(pin(variants[2].llvm_path, LIMITS.llvm_bytes, true).bytes,
      pin(variants[3].llvm_path, LIMITS.llvm_bytes, true).bytes, 'whole repeated LLVM bytes, not just hashes');
    for (const expected of pins.values()) {
      guard(); expectedPin(observe(expected.path, Math.max(expected.bytes, 1), false, expected.bytes === 0).pin, expected);
    }
    json('receipt.json', { schema: 'task-ordered-repeat-llvm-observation-v1', status: 'passed',
      authority: 'observation_only', source_capture: retainedReceipt.pin, lowerer: lowerer.pin,
      fresh_lowerer_calls: 4, retained_simulations_revalidated: 120, fresh_simulations: 0,
      retained_frontend_refusals_revalidated: 8, stages, variants, selected_pins: [...pins.values()],
      selected_pin_bytes: totalPinBytes, selected_inputs_unchanged: true, limits: LIMITS,
      ...Object.fromEntries(FALSE_FIELDS.map(key => [key, false])), native_qualified: false,
      llvm_verified_by_new_parser: false, physical_register_lifetime_proof: false,
      runtime_loop_or_schedule_added: false, milestone_completion: false,
      accounting: 'selected file custody and bounded process output only; outer build provenance, process-group and full-root guards remain required' });
  } catch (error) {
    if (outputCreated) {
      try { json('failure.json', { status: 'failed', error: String(error).slice(0, 4096), stages,
        completed_variants: variants, selected_inputs_unchanged: false,
        manufactured_fallback: false, cleanup_or_rollback_claimed: false }); }
      catch (secondary) { process.stderr.write('Failure receipt unavailable: ' + String(secondary).slice(0, 1024) + '\n'); }
    }
    throw error;
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run(options(process.argv.slice(2))).catch(error => {
    process.stderr.write(String(error).slice(0, 4096) + '\n'); process.exitCode = 1;
  });
}
