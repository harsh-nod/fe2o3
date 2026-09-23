#!/usr/bin/env node
// Closed retained source/LLVM -> fresh native observation. Never execution authority.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';
import { requireDiskReserve } from './assembly-region-worker-prototype.mjs';
import { parseExport } from './source-promotion-instruction-edit-smoke.mjs';
import { LIMITS as SOURCE_LIMITS, LABELS, INPUTS, LENGTHS,
  parseJson, validateSources, validateSimulation, validateRefusal, declaredProgram,
  request, negativeSource } from './ordered-repeat-source-smoke.mjs';
import { LIMITS as LLVM_LIMITS, FALSE_FIELDS as LLVM_FALSE, sha256, expectedPin,
  externalIdentity, validateCapture, validateLowererReport, validateLlvm,
  validateJoinedVariants } from './ordered-repeat-llvm-observation.mjs';

const KiB = 1024, MiB = 1024 * KiB, GiB = 1024 * MiB;
export const LIMITS = Object.freeze({ calls: 4, cases: 8, command_ms: 180000, wall_ms: 1140000,
  stream_bytes: 64 * KiB, payload_bytes: MiB, receipt_bytes: MiB,
  selected_file_bytes: 512 * MiB, pins: 600, pin_bytes: 3 * GiB, read_bytes: 12 * GiB,
  output_bytes: 32 * MiB, output_files: 32, output_directories: 5, min_ram_bytes: 64 * GiB });
export const LLVM_BUILD_ID = 'rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540';
export const NATIVE_FALSE = Object.freeze(['prefix_source_admitted', 'production_exact_program_admission',
  'protected_finalizer_admission', 'artifact_authority', 'source_authentication', 'compiler_closure_attestation',
  'hardware_execution', 'native_whole_kernel_correctness', 'physical_register_allocation_or_lifetime_proof',
  'whole_kernel_order_or_byte_stability_claim']);
export const SHAPE_CONTROLS = Object.freeze({ typed_shape_positives: 6, typed_shape_negatives: 78,
  input_identity_positives: 1, input_identity_negatives: 3, file_snapshot_positives: 1,
  file_snapshot_negatives: 9, count_selector_positives: 3, count_selector_negatives: 4,
  worker_or_target_machine_invoked: false, captured_llvm_mutated: false });
// Finalized together with the separately reviewed C++ reporter before handoff.
export const MUTATION_CONTROLS = Object.freeze({ decoded_field_refusals: 28, stale_identity_refusals: 1,
  synthetic_view_positives: 1, gapped_sequence_refusals: 1, duplicate_sequence_refusals: 1,
  extra_add_refusals: 1, wrong_count_refusals: 1, raw_byte_mismatch_refusals: 1,
  redecoded_opposite_opcode_refusals: 1, opposite_opcode_mutated_payload_observed: true,
  descriptor_capacity_positives: 1, descriptor_capacity_refusals: 3,
  original_payload_unchanged: true, captured_llvm_mutated: false, mutated_payload_executed_on_hardware: false });
const STAGE_KEYS = ['label', 'executable', 'args', 'code', 'signal', 'reason', 'elapsed_ms',
  'stdout_bytes', 'stdout_sha256', 'stderr_bytes', 'stderr_sha256'];
const LLVM_EXTRA_FALSE = ['native_qualified', 'llvm_verified_by_new_parser',
  'physical_register_lifetime_proof', 'runtime_loop_or_schedule_added', 'milestone_completion'];
const LLVM_ACCOUNTING = 'selected file custody and bounded process output only; outer build provenance, process-group and full-root guards remain required';
const CONSTRAINTS = '=&{v33},{v34},{v35},{v36},~{v32}';
const exact = (value, fields) => {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value), 'closed object');
  assert.deepEqual(Object.keys(value).sort(), [...fields].sort(), 'closed object fields');
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
function identity(bytes, pin, cap, empty = false) {
  assert.ok(Buffer.isBuffer(bytes)); integer(bytes.length, cap, empty ? 0 : 1);
  assert.equal(bytes.length, pin.bytes, 'retained complete byte length'); digest(pin.sha256);
  assert.equal(sha256(bytes), pin.sha256, 'retained complete raw hash');
}
function ledger(rows, cap, bytesCap, expectedBytes) {
  assert.ok(Array.isArray(rows) && rows.length > 0 && rows.length <= cap);
  const pins = new Map(); let total = 0;
  for (const pin of rows) {
    expectedPin(pin, pin); assert.ok(!pins.has(pin.path), 'duplicate selected pin');
    pins.set(pin.path, pin); total += pin.bytes; integer(total, bytesCap);
  }
  assert.equal(total, expectedBytes, 'complete selected pin byte census'); return pins;
}
function readFrom(pins, read) {
  return (file, cap = SOURCE_LIMITS.json_bytes) => {
    const pin = pins.get(absolute(file)); assert.ok(pin, 'consumed leaf must have original retained pin');
    const bytes = read(file, cap, pin); identity(bytes, pin, cap, pin.bytes === 0); return bytes;
  };
}

// This is the unchanged source driver's actual raw-output ladder, not summary
// acceptance or another source compiler. Its exported validators remain owners.
export function revalidateSource(capture, directory, read, exists) {
  const historical = validateCapture(capture, directory), retained = readFrom(historical, read);
  const absent = file => !exists(file);
  const sources = capture.variants.slice(0, 3).map(v => retained(v.source_path, SOURCE_LIMITS.source_bytes));
  validateSources(sources);
  const stageMap = new Map(capture.stages.map(stage => [stage.label, stage]));
  const outputOf = label => {
    const stage = stageMap.get(label); assert.ok(stage);
    return { ...stage, stdout: retained(path.join(directory, label + '.stdout')),
      stderr: retained(path.join(directory, label + '.stderr')) };
  };
  const exportArguments = (label, source, negative) => [
    '--diagnostic-kir-v17', '--crate', 'fe2o3_assembly_authoring_v30_fixture',
    '--output', path.join(directory, label + '.kir'), '--target', 'gfx942',
    '--target-dir', path.join(directory, label + '-extraction'), '--', '--manifest-path',
    path.join(path.dirname(path.dirname(source)), 'Cargo.toml'), '--lib', '--offline',
    ...(negative ? ['--message-format=json'] : []),
  ];
  for (let index = 0; index < LABELS.length; index++) {
    const variant = capture.variants[index], label = variant.label;
    const exported = outputOf(label + '-export');
    assert.equal(path.basename(exported.executable), 'fe2o3-export-sim');
    assert.deepEqual(exported.args, exportArguments(label, variant.source_path, false));
    assert.deepEqual(parseExport(exported.stderr, 'required'), variant.exported);
    const inspected = outputOf(label + '-inspect');
    assert.equal(path.basename(inspected.executable), 'fe2o3-program-inspect');
    assert.deepEqual(parseJson(inspected.stdout), variant.inspection);
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
  }
  return { pins: historical, variants: capture.variants, simulations: 120, refusals: 8, stages: 136 };
}
export function validateOrigin(variant, original) {
  for (const key of ['label', 'repetitions', 'source_path', 'source_sha256', 'kir_path', 'kir_file_sha256'])
    assert.equal(variant[key], original[key], 'same original source variant');
  assert.equal(variant.semantic_identity, original.exported.semantic_identity);
  assert.equal(variant.retained_source_preflight, original.exported.retained_source_preflight);
  assert.equal(variant.retained_source_inventory, original.exported.retained_source_inventory);
  assert.equal(variant.canonical_identity, original.exported.canonical_sha256);
}
export function revalidateLlvm(capture, source, sourcePin, directory, read) {
  exact(capture, ['schema', 'status', 'authority', 'source_capture', 'lowerer',
    'fresh_lowerer_calls', 'retained_simulations_revalidated', 'fresh_simulations',
    'retained_frontend_refusals_revalidated', 'stages', 'variants', 'selected_pins',
    'selected_pin_bytes', 'selected_inputs_unchanged', 'limits', ...LLVM_FALSE, ...LLVM_EXTRA_FALSE, 'accounting']);
  assert.equal(capture.schema, 'task-ordered-repeat-llvm-observation-v1');
  assert.equal(capture.status, 'passed'); assert.equal(capture.authority, 'observation_only');
  expectedPin(capture.source_capture, sourcePin);
  assert.equal(capture.fresh_lowerer_calls, 4); assert.equal(capture.retained_simulations_revalidated, 120);
  assert.equal(capture.fresh_simulations, 0); assert.equal(capture.retained_frontend_refusals_revalidated, 8);
  assert.equal(capture.selected_inputs_unchanged, true); assert.deepEqual(capture.limits, LLVM_LIMITS);
  assert.equal(capture.accounting, LLVM_ACCOUNTING);
  for (const field of [...LLVM_FALSE, ...LLVM_EXTRA_FALSE]) assert.equal(capture[field], false, field);
  const pins = ledger(capture.selected_pins, LLVM_LIMITS.all_pins, LLVM_LIMITS.all_pin_bytes, capture.selected_pin_bytes);
  for (const pin of source.pins.values()) expectedPin(pins.get(pin.path), pin);
  expectedPin(pins.get(sourcePin.path), sourcePin);
  expectedPin(pins.get(capture.lowerer.path), capture.lowerer);
  assert.ok(Array.isArray(capture.stages) && capture.stages.length === 4);
  assert.ok(Array.isArray(capture.variants) && capture.variants.length === 4);
  const retained = readFrom(pins, read);
  for (const [index, variant] of capture.variants.entries()) {
    exact(variant, ['label', 'repetitions', 'source_path', 'source_sha256', 'semantic_identity',
      'retained_source_preflight', 'retained_source_inventory', 'kir_path', 'kir_file_sha256',
      'canonical_identity', 'llvm_path', 'llvm_bytes', 'llvm_sha256', 'report', 'observed']);
    const original = source.variants[index], label = LABELS[index];
    validateOrigin(variant, original);
    assert.equal(variant.llvm_path, path.join(directory, label + '.ll'));
    const stage = capture.stages[index]; exact(stage, STAGE_KEYS);
    assert.equal(stage.label, label); assert.equal(stage.executable, capture.lowerer.path);
    assert.deepEqual(stage.args, [original.kir_path, variant.llvm_path]);
    assert.equal(stage.code, 0); assert.equal(stage.signal, null); assert.equal(stage.reason, null);
    integer(stage.elapsed_ms, LLVM_LIMITS.command_ms);
    const stdout = retained(path.join(directory, label + '.stdout'), LLVM_LIMITS.stream_bytes);
    const stderr = retained(path.join(directory, label + '.stderr'), LLVM_LIMITS.stream_bytes);
    identity(stdout, { bytes: stage.stdout_bytes, sha256: stage.stdout_sha256 }, LLVM_LIMITS.stream_bytes);
    identity(stderr, { bytes: stage.stderr_bytes, sha256: stage.stderr_sha256 }, LLVM_LIMITS.stream_bytes, true);
    assert.equal(stderr.length, 0);
    const report = parseJson(stdout), kir = retained(original.kir_path, SOURCE_LIMITS.kir_bytes);
    const llvm = retained(variant.llvm_path, LLVM_LIMITS.llvm_bytes);
    identity(llvm, { bytes: variant.llvm_bytes, sha256: variant.llvm_sha256 }, LLVM_LIMITS.llvm_bytes);
    validateLowererReport(report, original, kir, llvm); assert.deepEqual(report, variant.report);
    assert.deepEqual(validateLlvm(llvm, original), variant.observed);
  }
  validateJoinedVariants(capture.variants);
  assert.deepEqual(retained(capture.variants[2].llvm_path, LLVM_LIMITS.llvm_bytes),
    retained(capture.variants[3].llvm_path, LLVM_LIMITS.llvm_bytes), 'whole identical-source repeated LLVM bytes');
  return { pins, variants: capture.variants };
}

// Independent literal references are already reviewed in
// ordered-program-worker-prototype.mjs and its GFX9 encoding-field tests.
// These are expected encodings, not claimed actual native observations.
export function expectedProgram(repetitions) {
  declaredProgram(repetitions);
  return [{ opcode: 'V_MOV_B32_e32_vi', bytes_hex: '2203427e', register_operands: ['VGPR33', 'VGPR34'] },
    ...Array.from({ length: repetitions }, () => ({
      opcode: 'V_ADD_U32_e32_gfx9', bytes_hex: '21474268', register_operands: ['VGPR33', 'VGPR33', 'VGPR35'] }))];
}
export function validateNative(report, expected, llvm, readPayload) {
  exact(expected, ['repetitions', 'llvm_sha256', 'llvm_bytes', 'payload_directory', 'llvm_build_id', 'worker_build_id']);
  declaredProgram(expected.repetitions); absolute(expected.payload_directory);
  assert.equal(expected.llvm_build_id, LLVM_BUILD_ID);
  assert.match(expected.worker_build_id, /^fe2o3-worker-v1-sha256-[a-f0-9]{64}$/u);
  assert.notEqual(expected.worker_build_id, 'fe2o3-worker-v1-sha256-' + '0'.repeat(64));
  identity(llvm, { bytes: expected.llvm_bytes, sha256: expected.llvm_sha256 }, LLVM_LIMITS.llvm_bytes);
  exact(report, ['report_kind', 'authority', 'repetitions', 'program_count', 'kernel_symbol', 'register_plan',
    'constraints', 'required_binding_extent', 'result_use', 'llvm_sha256', 'llvm_bytes',
    'expected_input_identity_matched', 'retained_file_identity_and_bytes_rechecked', 'llvm_build_claim',
    'worker_build_claim', 'target', 'wave_width', 'workgroup_size', 'code_object_version', 'shape_controls',
    'cases', 'source_ancestry', 'synthetic_worker_request_identity_fields', 'runtime_closure_attestation', ...NATIVE_FALSE]);
  assert.equal(report.report_kind, 'task-ordered-repeat-native-observation-v1');
  assert.equal(report.authority, 'unauthenticated-test-transport');
  assert.equal(report.repetitions, expected.repetitions); assert.equal(report.program_count, expected.repetitions + 1);
  assert.equal(report.kernel_symbol, 'ordered_repeat_u32'); assert.deepEqual(report.register_plan, [32, 33, 34, 35, 36]);
  assert.equal(report.constraints, CONSTRAINTS); assert.equal(report.required_binding_extent, 37);
  assert.equal(report.result_use, 'sole-direct-nonvolatile-nonatomic-global-store');
  assert.equal(report.llvm_sha256, expected.llvm_sha256); assert.equal(report.llvm_bytes, expected.llvm_bytes);
  assert.equal(report.expected_input_identity_matched, true); assert.equal(report.retained_file_identity_and_bytes_rechecked, true);
  assert.equal(report.llvm_build_claim, expected.llvm_build_id); assert.equal(report.worker_build_claim, expected.worker_build_id);
  assert.equal(report.target, 'gfx942:xnack-'); assert.equal(report.wave_width, 64);
  assert.equal(report.workgroup_size, 64); assert.equal(report.code_object_version, 6);
  assert.equal(report.source_ancestry, 'not-established-by-llvm-file');
  assert.equal(report.synthetic_worker_request_identity_fields, true);
  assert.equal(report.runtime_closure_attestation, 'unavailable');
  for (const key of NATIVE_FALSE) assert.equal(report[key], false, key);
  assert.deepEqual(report.shape_controls, SHAPE_CONTROLS);
  assert.ok(Array.isArray(report.cases) && report.cases.length === 2);
  const program = expectedProgram(expected.repetitions), summaries = [];
  for (const [index, wrapper] of report.cases.entries()) {
    exact(wrapper, ['machine_observation', 'mutation_controls', 'retained_payload']);
    assert.deepEqual(wrapper.mutation_controls, MUTATION_CONTROLS);
    const value = wrapper.machine_observation, payload = wrapper.retained_payload, optimization = ['O0', 'O3'][index];
    exact(value, ['optimization', 'llvm_sha256', 'llvm_bytes', 'hsaco_sha256', 'hsaco_bytes',
      'entry_file_offset', 'entry_code_bytes', 'static_instruction_count', 'program', 'descriptor',
      'post_link_checks', 'derivation_identity', 'boundary_value_or_lifetime_proof', 'repetitions', 'unique_sequence_matches']);
    assert.equal(value.optimization, optimization); assert.equal(value.repetitions, expected.repetitions);
    assert.equal(value.unique_sequence_matches, 1, 'one unique decoded contiguous sequence');
    assert.equal(value.llvm_sha256, expected.llvm_sha256); assert.equal(value.llvm_bytes, expected.llvm_bytes);
    assert.equal(value.boundary_value_or_lifetime_proof, false); digest(value.derivation_identity);
    exact(payload, ['path', 'bytes', 'sha256', 'matches_linked_worker_payload', 'create_new_only', 'production_artifact_authority']);
    assert.equal(payload.path, path.join(expected.payload_directory, optimization + '.hsaco'));
    assert.equal(payload.bytes, value.hsaco_bytes); assert.equal(payload.sha256, value.hsaco_sha256);
    assert.equal(payload.matches_linked_worker_payload, true); assert.equal(payload.create_new_only, true);
    assert.equal(payload.production_artifact_authority, false);
    const bytes = readPayload(payload.path, LIMITS.payload_bytes); identity(bytes, payload, LIMITS.payload_bytes);
    integer(value.entry_file_offset, bytes.length); integer(value.entry_code_bytes, bytes.length - value.entry_file_offset, program.length * 4);
    integer(value.static_instruction_count, 512, program.length);
    assert.ok(Array.isArray(value.program) && value.program.length === program.length, 'exact MOV plus N ADD count');
    for (const [at, site] of value.program.entries()) {
      exact(site, ['file_offset', 'opcode', 'bytes_hex', 'mc_flags', 'register_operands', 'implicit_reads', 'implicit_writes']);
      assert.deepEqual({ opcode: site.opcode, bytes_hex: site.bytes_hex, register_operands: site.register_operands }, program[at]);
      integer(site.file_offset, value.entry_file_offset + value.entry_code_bytes - 4, value.entry_file_offset);
      integer(site.mc_flags, 65535); assert.equal(site.mc_flags & ~16, 0);
      assert.deepEqual(site.implicit_reads, ['EXEC']); assert.deepEqual(site.implicit_writes, []);
      assert.equal(site.file_offset, value.program[0].file_offset + at * 4, 'contiguous full-payload instruction offsets');
      assert.equal(bytes.subarray(site.file_offset, site.file_offset + 4).toString('hex'), site.bytes_hex,
        'exact instruction bytes at worker ELF file offsets, never byte-search offsets');
    }
    const descriptor = value.descriptor;
    exact(descriptor, ['file_offset', 'bytes', 'sha256', 'compute_pgm_rsrc1', 'compute_pgm_rsrc3',
      'vgpr_capacity', 'architected_vgpr_boundary', 'required_footprint_high_water', 'interpretation']);
    integer(descriptor.file_offset, bytes.length - 64); assert.equal(descriptor.bytes, 64);
    assert.equal(descriptor.required_footprint_high_water, 37);
    assert.equal(descriptor.interpretation, 'encoded-capacity-not-metadata-usage-or-lifetime');
    assert.ok(descriptor.file_offset + 64 <= value.entry_file_offset ||
      value.entry_file_offset + value.entry_code_bytes <= descriptor.file_offset, 'entry and descriptor disjoint');
    const raw = bytes.subarray(descriptor.file_offset, descriptor.file_offset + 64); identity(raw, descriptor, 64);
    integer(descriptor.compute_pgm_rsrc1, 0xffffffff); integer(descriptor.compute_pgm_rsrc3, 0xffffffff);
    assert.equal(raw.readUInt32LE(48), descriptor.compute_pgm_rsrc1);
    assert.equal(raw.readUInt32LE(44), descriptor.compute_pgm_rsrc3);
    assert.equal(descriptor.vgpr_capacity, ((descriptor.compute_pgm_rsrc1 & 63) + 1) * 8);
    assert.equal(descriptor.architected_vgpr_boundary, ((descriptor.compute_pgm_rsrc3 & 63) + 1) * 4);
    assert.ok(descriptor.vgpr_capacity >= descriptor.architected_vgpr_boundary &&
      descriptor.architected_vgpr_boundary >= 37, 'encoded capacity must cover declared v0..v36 extent');
    assert.ok(Array.isArray(value.post_link_checks) && value.post_link_checks.length === 5);
    value.post_link_checks.forEach(line => assert.ok(typeof line === 'string' && Buffer.byteLength(line) <= 1024));
    assert.deepEqual(value.post_link_checks.slice(0, 4), [
      'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
      'post_link.check=exports status=ok symbols=[ordered_repeat_u32,ordered_repeat_u32.kd]',
      'post_link.check=unresolved status=ok symbols=[]',
      'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-']);
    const launch = /^post_link.kernel name=ordered_repeat_u32 symbol=ordered_repeat_u32\.kd kernarg_size=([0-9]{1,10}) group_size=([0-9]{1,10}) private_size=([0-9]{1,10}) kernarg_align=([1-9][0-9]{0,4}) wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=\[64,1,1\]$/u.exec(value.post_link_checks[4]);
    assert.ok(launch, 'exact worker post-link target/kernel launch reporting contract');
    launch.slice(1, 4).forEach(n => { assert.match(n, /^(?:0|[1-9][0-9]*)$/u); integer(Number(n), 0xffffffff); });
    const alignment = integer(Number(launch[4]), 32768, 1); assert.equal(alignment & (alignment - 1), 0);
    summaries.push({ optimization, repetitions: expected.repetitions, llvm_sha256: expected.llvm_sha256,
      llvm_bytes: expected.llvm_bytes, payload: { ...payload }, instruction_offsets: value.program.map(site => site.file_offset),
      descriptor: { ...descriptor }, unique_sequence_matches: 1, boundary_value_or_lifetime_proof: false });
  }
  return summaries;
}
export function healthy(result) {
  assert.equal(result.code, 0); assert.equal(result.signal, null); assert.equal(result.reason, null);
  integer(result.elapsed_ms, LIMITS.command_ms);
  for (const key of ['stdout', 'stderr']) assert.ok(Buffer.isBuffer(result[key]) && result[key].length <= LIMITS.stream_bytes);
  assert.ok(result.stdout.length > 0); assert.equal(result.stderr.length, 0, 'unexpected native stderr');
}
export function options(argv) {
  const paths = ['repo', 'source-receipt', 'llvm-receipt', 'observer', 'output'];
  const sizes = ['source-receipt-bytes', 'llvm-receipt-bytes', 'observer-bytes'];
  const hashes = ['source-receipt-sha256', 'llvm-receipt-sha256', 'observer-sha256'];
  const keys = [...paths, ...sizes, ...hashes, 'llvm-build-id', 'worker-build-id'], out = {};
  assert.equal(argv.length, keys.length * 2, 'closed argument count');
  for (let i = 0; i < argv.length; i += 2) {
    assert.ok(argv[i].startsWith('--')); const key = argv[i].slice(2);
    assert.ok(keys.includes(key) && !Object.hasOwn(out, key)); out[key] = argv[i + 1];
  }
  paths.forEach(key => absolute(out[key])); hashes.forEach(key => digest(out[key]));
  for (const key of sizes) {
    assert.match(out[key], /^[1-9][0-9]{0,9}$/u);
    out[key] = integer(Number(out[key]), key === 'observer-bytes' ? LIMITS.selected_file_bytes : LIMITS.receipt_bytes, 1);
  }
  assert.equal(out['llvm-build-id'], LLVM_BUILD_ID);
  assert.match(out['worker-build-id'], /^fe2o3-worker-v1-sha256-[a-f0-9]{64}$/u);
  assert.notEqual(out['worker-build-id'], 'fe2o3-worker-v1-sha256-' + '0'.repeat(64));
  for (const key of ['source-receipt', 'llvm-receipt']) assert.equal(path.basename(out[key]), 'receipt.json');
  assert.notEqual(path.dirname(out['source-receipt']), path.dirname(out['llvm-receipt']));
  for (const input of [out.repo, out.observer, path.dirname(out['source-receipt']), path.dirname(out['llvm-receipt'])])
    assert.ok(!inside(input, out.output) && !inside(out.output, input), 'exclusive new output outside all retained inputs');
  return out;
}

// Runtime adapter deliberately retains the source driver's full stat/hash schema.
// Existing expectedPin/externalIdentity own comparison; no shared export is changed.
class Custody {
  pins = new Map(); total = 0; readBytes = 0;
  constructor(guard) { this.guard = guard; }
  observe(file, cap, keep = false, empty = false, expected = null) {
    this.guard(); absolute(file); integer(cap, LIMITS.selected_file_bytes, 1);
    assert.equal(fs.realpathSync(file), file, 'canonical retained input');
    const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    try {
      const before = fs.fstatSync(fd, { bigint: true });
      assert.ok(before.isFile() && before.size <= BigInt(cap) && (empty || before.size > 0n));
      const hash = createHash('sha256'), chunks = [], block = Buffer.alloc(64 * KiB); let size = 0;
      for (;;) {
        this.guard();
        const n = fs.readSync(fd, block, 0, block.length, null); if (n === 0) break;
        size += n; this.readBytes += n; integer(size, cap); integer(this.readBytes, LIMITS.read_bytes);
        hash.update(block.subarray(0, n)); if (keep) chunks.push(Buffer.from(block.subarray(0, n)));
      }
      const after = fs.fstatSync(fd, { bigint: true }), named = fs.lstatSync(file, { bigint: true });
      for (const key of ['dev', 'ino', 'mode', 'nlink', 'size', 'mtimeNs', 'ctimeNs']) {
        assert.equal(after[key], before[key]); assert.equal(named[key], before[key]);
      }
      assert.equal(fs.realpathSync(file), file); assert.equal(BigInt(size), before.size);
      const pin = { path: file, bytes: size, sha256: hash.digest('hex'), device: String(before.dev),
        inode: String(before.ino), mode: String(before.mode), nlink: String(before.nlink),
        mtime_ns: String(before.mtimeNs), ctime_ns: String(before.ctimeNs) };
      if (expected) expectedPin(pin, expected);
      const prior = this.pins.get(file);
      if (prior) expectedPin(pin, prior);
      else { assert.ok(this.pins.size < LIMITS.pins); this.total += size;
        integer(this.total, LIMITS.pin_bytes); this.pins.set(file, pin); }
      return { pin, bytes: keep ? Buffer.concat(chunks) : undefined };
    } finally { fs.closeSync(fd); }
  }
  read(file, cap, expected) { return this.observe(file, cap, true, expected?.bytes === 0, expected).bytes; }
  recheck() { for (const pin of this.pins.values()) this.observe(pin.path, Math.max(pin.bytes, 1), false, pin.bytes === 0, pin); }
}
function absent(file) {
  try { fs.lstatSync(file); return false; } catch (error) { if (error.code === 'ENOENT') return true; throw error; }
}
function directory(file) {
  absolute(file); assert.equal(fs.realpathSync(file), file); const st = fs.lstatSync(file, { bigint: true });
  assert.ok(st.isDirectory()); return { dev: st.dev, ino: st.ino, mode: st.mode };
}
function tree(root) {
  let files = 0, directories = 0, bytes = 0;
  const visit = (dir, depth) => {
    assert.ok(depth <= 1); directories++; integer(directories, LIMITS.output_directories);
    for (const name of fs.readdirSync(dir)) {
      const full = path.join(dir, name), st = fs.lstatSync(full);
      assert.ok(!st.isSymbolicLink(), 'no output symlink');
      if (st.isDirectory()) visit(full, depth + 1);
      else { assert.ok(st.isFile(), 'regular output only'); files++; bytes += st.size;
        integer(files, LIMITS.output_files); integer(bytes, LIMITS.output_bytes); }
    }
  };
  visit(root, 0); return { files, directories, bytes };
}
export function nativeArguments(variant, payloadDirectory) {
  declaredProgram(variant.repetitions); absolute(variant.llvm_path); absolute(payloadDirectory);
  digest(variant.llvm_sha256); integer(variant.llvm_bytes, LLVM_LIMITS.llvm_bytes, 1);
  return [String(variant.repetitions), variant.llvm_path, variant.llvm_sha256, String(variant.llvm_bytes), payloadDirectory];
}
async function run(opt) {
  const start = performance.now(), stages = [], native = [], directories = new Map();
  let created = false;
  for (const dir of [opt.repo, path.dirname(opt['source-receipt']), path.dirname(opt['llvm-receipt']), path.dirname(opt.output)])
    directories.set(dir, directory(dir));
  const guard = () => {
    assert.ok(performance.now() - start <= LIMITS.wall_ms, '19 minute cooperative deadline');
    requireDiskReserve(path.dirname(opt.output));
    const mem = fs.readFileSync('/proc/meminfo', 'utf8'); assert.ok(Buffer.byteLength(mem) <= 64 * KiB);
    const match = /^MemAvailable:\s+([0-9]+) kB$/mu.exec(mem); assert.ok(match);
    assert.ok(BigInt(match[1]) * 1024n >= BigInt(LIMITS.min_ram_bytes), '64GiB available RAM reserve');
    for (const [dir, prior] of directories) assert.deepEqual(directory(dir), prior, 'same selected directory owner');
    if (created) tree(opt.output);
  };
  const custody = new Custody(guard);
  const save = (leaf, bytes) => {
    guard(); assert.equal(path.basename(leaf), leaf); assert.ok(Buffer.isBuffer(bytes));
    integer(bytes.length, LIMITS.receipt_bytes);
    const file = path.join(opt.output, leaf);
    fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 });
    custody.observe(file, Math.max(bytes.length, 1), false, bytes.length === 0); guard(); return file;
  };
  const json = (leaf, value) => save(leaf, Buffer.from(JSON.stringify(value, null, 2) + '\n'));
  try {
    const inputs = {};
    for (const role of ['source', 'llvm']) {
      const key = role + '-receipt', got = custody.observe(opt[key], LIMITS.receipt_bytes, true);
      externalIdentity(got.bytes, opt[key + '-bytes'], opt[key + '-sha256']); inputs[role] = { ...got, value: parseJson(got.bytes) };
      assert.ok(absent(path.join(path.dirname(opt[key]), 'failure.json')), 'failed historical capture is not a positive');
    }
    const sourceLedger = validateCapture(inputs.source.value, path.dirname(opt['source-receipt']));
    const llvmValue = inputs.llvm.value;
    const llvmLedger = ledger(llvmValue.selected_pins, LLVM_LIMITS.all_pins, LLVM_LIMITS.all_pin_bytes, llvmValue.selected_pin_bytes);
    for (const pins of [sourceLedger, llvmLedger]) for (const pin of pins.values())
      custody.observe(pin.path, Math.max(pin.bytes, 1), false, pin.bytes === 0, pin);
    // Do not transplant historical ownership to a renamed checkout. Integration
    // must stage here under explicit scope, or capture fresh source+LLVM there.
    const here = path.dirname(fileURLToPath(import.meta.url));
    assert.equal(here, path.join(opt.repo, 'scripts'), 'driver and retained capture repo must share exact original owner');
    for (const name of ['ordered-repeat-source-smoke.mjs', 'source-promotion-instruction-edit-smoke.mjs',
      'ordered-program-source-native.mjs', 'ordered-program-worker-prototype.mjs',
      'assembly-region-worker-prototype.mjs', 'authoring-navigation-v1-process.mjs']) {
      const file = path.join(here, name); assert.ok(sourceLedger.has(file) && llvmLedger.has(file));
      expectedPin(sourceLedger.get(file), llvmLedger.get(file));
    }
    assert.ok(llvmLedger.has(path.join(here, 'ordered-repeat-llvm-observation.mjs')));
    custody.observe(fileURLToPath(import.meta.url), MiB);
    custody.observe(process.execPath, LIMITS.selected_file_bytes);
    const read = (file, cap, expected) => custody.read(file, cap, expected);
    const source = revalidateSource(inputs.source.value, path.dirname(opt['source-receipt']), read, file => !absent(file));
    const lowered = revalidateLlvm(llvmValue, source, inputs.source.pin, path.dirname(opt['llvm-receipt']), read);
    const observer = custody.observe(opt.observer, LIMITS.selected_file_bytes);
    assert.equal(observer.pin.bytes, opt['observer-bytes']); assert.equal(observer.pin.sha256, opt['observer-sha256']);
    assert.ok((BigInt(observer.pin.mode) & 0o111n) !== 0n);
    custody.recheck(); guard(); assert.ok(absent(opt.output), 'new-only output directory');
    fs.mkdirSync(opt.output, { mode: 0o700 }); created = true; directories.set(opt.output, directory(opt.output));
    const env = { ...process.env }; delete env.LD_PRELOAD;
    for (const key of Object.keys(env)) if (key.startsWith('FE2O3_')) delete env[key];
    for (const variant of lowered.variants) {
      guard(); assert.ok(stages.length < LIMITS.calls);
      const payloadDirectory = path.join(opt.output, variant.label + '-payloads'); assert.ok(absent(payloadDirectory));
      const args = nativeArguments(variant, payloadDirectory);
      const llvm = custody.read(variant.llvm_path, LLVM_LIMITS.llvm_bytes, lowered.pins.get(variant.llvm_path));
      custody.observe(opt.observer, LIMITS.selected_file_bytes, false, false, observer.pin);
      const result = await runNavigationCommand({ executable: opt.observer, args, cwd: opt.repo, env,
        timeoutMs: LIMITS.command_ms, outputCap: LIMITS.stream_bytes, guard });
      save(variant.label + '.stdout', result.stdout); save(variant.label + '.stderr', result.stderr);
      stages.push({ label: variant.label, executable: opt.observer, args, code: result.code, signal: result.signal,
        reason: result.reason, elapsed_ms: result.elapsed_ms, stdout_bytes: result.stdout.length,
        stdout_sha256: sha256(result.stdout), stderr_bytes: result.stderr.length, stderr_sha256: sha256(result.stderr) });
      healthy(result); directories.set(payloadDirectory, directory(payloadDirectory));
      assert.deepEqual(fs.readdirSync(payloadDirectory).sort(), ['O0.hsaco', 'O3.hsaco'], 'exact complete payload census');
      const report = parseJson(result.stdout), expected = { repetitions: variant.repetitions,
        llvm_sha256: variant.llvm_sha256, llvm_bytes: variant.llvm_bytes, payload_directory: payloadDirectory,
        llvm_build_id: opt['llvm-build-id'], worker_build_id: opt['worker-build-id'] };
      native.push({ label: variant.label, report, observations: validateNative(report, expected, llvm,
        (file, cap) => custody.read(file, cap)) });
      custody.observe(opt.observer, LIMITS.selected_file_bytes, false, false, observer.pin);
      custody.observe(variant.llvm_path, LLVM_LIMITS.llvm_bytes, false, false, lowered.pins.get(variant.llvm_path));
    }
    assert.equal(stages.length, 4); assert.equal(native.flatMap(v => v.observations).length, 8);
    // Count15 and its fresh identical-source replay were both actually compiled.
    // Whole-payload equality is checked for this finite pair, never generalized.
    for (const index of [0, 1]) {
      const a = native[2].observations[index].payload, b = native[3].observations[index].payload;
      assert.deepEqual(custody.read(a.path, LIMITS.payload_bytes), custody.read(b.path, LIMITS.payload_bytes),
        'same exact repeated LLVM produces same selected complete payload in this run');
    }
    for (const variant of lowered.variants) assert.deepEqual(fs.readdirSync(path.join(opt.output, variant.label + '-payloads')).sort(),
      ['O0.hsaco', 'O3.hsaco']);
    custody.recheck();
    for (const key of ['source-receipt', 'llvm-receipt'])
      assert.ok(absent(path.join(path.dirname(opt[key]), 'failure.json')));
    for (const refusal of inputs.source.value.refusals)
      assert.ok(absent(path.join(path.dirname(opt['source-receipt']), 'refuse-' + refusal.label + '.kir')),
        'retained frontend refusal output remains absent');
    guard();
    json('receipt.json', { schema: 'task-ordered-repeat-source-native-join-v1', status: 'passed',
      authority: 'observation_only', source_capture: inputs.source.pin, llvm_capture: inputs.llvm.pin,
      observer: observer.pin, source_variants: source.variants.map(v => ({ label: v.label, repetitions: v.repetitions,
        source_sha256: v.source_sha256, exported: v.exported, kir_file_sha256: v.kir_file_sha256 })),
      llvm_variants: lowered.variants, retained_source_exports: 4, retained_cpu_simulations_revalidated: 120,
      retained_frontend_refusals_revalidated: 8, retained_llvm_lowerings: 4,
      fresh_source_exports: 0, fresh_llvm_lowerings: 0, fresh_cpu_simulations: 0,
      fresh_native_invocations: 4, native_optimization_cases: 8, complete_hsaco_payloads: 8, stages, native,
      selected_inputs_unchanged: true, selected_pins: [...custody.pins.values()], selected_pin_bytes: custody.total,
      actual_read_bytes: custody.readBytes, limits: LIMITS, full_payload_offset_join: 'worker ELF file offsets and whole bytes; no byte scanning',
      native_repeat_scope: 'fresh O0/O3 repeated exact LLVM; finite whole-payload equality, not global stability',
      ...Object.fromEntries(NATIVE_FALSE.map(key => [key, false])), proof_authority: false,
      runtime_loop_or_schedule_added: false, production_resume: false, performance_prediction: false,
      native_qualified: false, milestone_completion: false, runtime_closure_attestation: 'unavailable',
      accounting: 'outer pinned build/SDK/source census, task-parent custody, process-group and full-root guards required' });
  } catch (error) {
    if (created) {
      try { json('failure.json', { status: 'failed', error: String(error).slice(0, 4096), stages,
        completed_native: native, selected_inputs_unchanged: false, manufactured_fallback: false, cleanup_or_rollback_claimed: false }); }
      catch (secondary) { process.stderr.write('Failure receipt unavailable: ' + String(secondary).slice(0, 1024) + '\n'); }
    }
    throw error;
  }
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  run(options(process.argv.slice(2))).catch(error => { process.stderr.write(String(error).slice(0, 4096) + '\n'); process.exitCode = 1; });
}
