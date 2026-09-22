#!/usr/bin/env node
// Task-private, fail-closed observation join. No compiler/worker/GPU is invoked.
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { createHash } from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';
import { parseBoundedJson } from './ordered-program-source-native.mjs';
import { INPUTS, LENGTHS, descriptors, oracle, parseExport, parsePublication, replaceOnce, request, sha256,
  validateSeedRecord, verifyEmission, verifyInspector, verifySimulation, verifySourceEdit, verifyVariants,
} from './source-promotion-instruction-edit-smoke.mjs';

const KiB = 1024, MiB = KiB * KiB, GiB = MiB * KiB;
const SOURCE_LIMIT = 64 * KiB, PAYLOAD_LIMIT = MiB, JSON_LIMIT = MiB;
const SOURCE_PIN_LIMIT = 384, SOURCE_PIN_BYTES = 2 * GiB;
const JOIN_PIN_LIMIT = 400, JOIN_PIN_BYTES = SOURCE_PIN_BYTES + 10 * MiB;
const SOURCE_FALSE = ['ranked_checks', 'protected_proof', 'compiler_closure_attestation',
  'source_authentication', 'native_qualified', 'physical_register_lifetime_proof', 'production_resume', 'hardware_observed'];
const NATIVE_FALSE = ['prefix_source_admitted', 'production_exact_program_admission', 'protected_finalizer_admission',
  'artifact_authority', 'source_authentication', 'compiler_closure_attestation', 'hardware_execution',
  'native_whole_kernel_correctness', 'physical_register_allocation_or_lifetime_proof', 'whole_kernel_order_or_byte_stability_claim'];
const SOURCE_LIMITS = Object.freeze({ source_bytes: SOURCE_LIMIT, command_stream_bytes: MiB, command_ms: 300000,
  stages: 110, retained_pins: SOURCE_PIN_LIMIT, retained_pin_bytes: SOURCE_PIN_BYTES, cargo_jobs: 2,
  task_root_storage_accounting: 'external_root_supervisor_required_not_replaced_by_this_runner' });
const SHAPE = Object.freeze({ typed_shape_positives: 4, typed_shape_negatives: 48,
  input_identity_positives: 1, input_identity_negatives: 3, file_snapshot_positives: 1,
  file_snapshot_negatives: 9, worker_or_target_machine_invoked: false, captured_llvm_mutated: false });
const MUTATIONS = Object.freeze({ decoded_field_refusals: 7, stale_identity_refusals: 1,
  gapped_sequence_refusals: 1, raw_byte_mismatch_refusals: 1, redecoded_opposite_opcode_refusals: 1,
  opposite_profile_mutated_payload_observed: true, original_payload_unchanged: true,
  captured_llvm_mutated: false, mutated_payload_executed_on_hardware: false });
const PIN_KEYS = ['path', 'bytes', 'sha256', 'device', 'inode', 'mode', 'mtime_ns', 'ctime_ns'];
const LOADER = '#![no_std]\n#[path = "original.rs"] mod source_bitselect_feasibility;\n';
const EXPORT_LOADER = '#![no_std]\n#[path = "kernel.rs"] mod source_bitselect_feasibility;\n';
const UTF8 = new TextDecoder('utf-8', { fatal: true });
export const LIMITS = Object.freeze({ source: SOURCE_LIMIT, payload: PAYLOAD_LIMIT, json: JSON_LIMIT,
  sourcePins: SOURCE_PIN_LIMIT, sourcePinBytes: SOURCE_PIN_BYTES, joinPins: JOIN_PIN_LIMIT, joinPinBytes: JOIN_PIN_BYTES });

function exact(value, keys) {
  assert.ok(value && typeof value === 'object' && !Array.isArray(value), 'closed object required');
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'unknown or missing fields');
}
function natural(value, max, min = 0) {
  assert.ok(Number.isSafeInteger(value) && value >= min && value <= max, 'lossless bounded unsigned integer');
  return value;
}
function digest(value) {
  assert.equal(typeof value, 'string'); assert.match(value, /^[0-9a-f]{64}$/u);
  assert.notEqual(value, '0'.repeat(64)); return value;
}
function decimal(value) {
  assert.equal(typeof value, 'string'); assert.match(value, /^(?:0|[1-9][0-9]{0,31})$/u);
  return BigInt(value);
}
function absolute(value) {
  assert.equal(typeof value, 'string'); assert.ok(path.isAbsolute(value) && path.resolve(value) === value);
  assert.ok(Buffer.byteLength(value) <= 4096 && !/[\u0000-\u001f\u007f]/u.test(value)); return value;
}
function within(parent, child) {
  const relative = path.relative(parent, child);
  return relative !== '' && !path.isAbsolute(relative) && relative !== '..' && !relative.startsWith('../');
}
function buffer(bytes, cap, empty = false) {
  assert.ok(Buffer.isBuffer(bytes) && (empty || bytes.length > 0) && bytes.length <= cap, 'bounded raw bytes');
  return bytes;
}
function identity(bytes, expected, cap, empty = false) {
  buffer(bytes, cap, empty); natural(expected.bytes, cap, empty ? 0 : 1); digest(expected.sha256);
  assert.equal(bytes.length, expected.bytes, 'raw byte length join'); assert.equal(sha256(bytes), expected.sha256, 'raw digest join');
}
export function parseJson(bytes, cap = JSON_LIMIT) { return parseBoundedJson(bytes, cap); }

export function sourcePinLedger(receipt) {
  assert.ok(Array.isArray(receipt.retained_file_pins) && receipt.retained_file_pins.length > 0 &&
    receipt.retained_file_pins.length <= SOURCE_PIN_LIMIT, 'producer pin census');
  const pins = new Map(); let bytes = 0;
  for (const pin of receipt.retained_file_pins) {
    exact(pin, PIN_KEYS); absolute(pin.path); digest(pin.sha256); natural(pin.bytes, 512 * MiB);
    for (const key of ['device', 'inode', 'mode', 'mtime_ns', 'ctime_ns']) decimal(pin[key]);
    assert.ok(!pins.has(pin.path), 'duplicate retained input path'); pins.set(pin.path, pin);
    bytes += pin.bytes; natural(bytes, SOURCE_PIN_BYTES);
  }
  assert.equal(receipt.retained_pin_bytes, bytes, 'lossless complete producer pin accounting');
  return pins;
}

// Pure join: read(file, cap) returns retained raw bytes; runtime adds fd/stat custody.
// It does not accept facts reconstructed from the summary instead of actual logs.
export function validateSourceReceipt(receipt, context, read) {
  exact(context, ['repo', 'prepared', 'sourceRoot', 'consumer', 'binDir', 'emitter']);
  Object.values(context).forEach(absolute);
  const { repo, prepared, sourceRoot, consumer, binDir, emitter } = context;
  exact(receipt, ['status', 'kind', 'normal_public_seed', 'actual_normal_exports', 'whole_kernel_simulations',
    'variants', 'stages', 'retained_file_pins', 'retained_pin_bytes', 'semantic_identity_join_qualified',
    'semantic_identity_availability', 'source_edit', ...SOURCE_FALSE, 'limits']);
  assert.equal(receipt.status, 'passed'); assert.equal(receipt.kind, 'source_promotion_instruction_edit_diagnostic_v1');
  assert.equal(receipt.actual_normal_exports, 3); assert.equal(receipt.whole_kernel_simulations, 90);
  assert.equal(receipt.semantic_identity_join_qualified, true);
  assert.equal(receipt.semantic_identity_availability, 'normal_live_exporter_observed');
  assert.equal(receipt.source_edit, 'one_exact_final_xor_to_or_original_and_surrounding_bytes_unchanged');
  SOURCE_FALSE.forEach(key => assert.equal(receipt[key], false));
  assert.deepEqual(receipt.limits, SOURCE_LIMITS);
  const pins = sourcePinLedger(receipt);
  const raw = (file, cap = SOURCE_LIMIT, empty = false) => {
    const pin = pins.get(absolute(file)); assert.ok(pin, 'required file absent from full producer pin ledger');
    const bytes = read(file, cap); identity(bytes, pin, cap, empty); return bytes;
  };
  const fromOutput = (name, cap = SOURCE_LIMIT, empty = false) => raw(path.join(sourceRoot, name), cap, empty);
  const record = parseJson(raw(path.join(prepared, 'positive/headless-normal.invocation.json'), JSON_LIMIT));
  const seed = validateSeedRecord(record, repo, prepared);
  const original = raw(path.join(seed.directory, 'original.rs'));
  assert.equal(sha256(original), record.source_sha256);
  assert.deepEqual(fromOutput('original.rs'), original);
  const loader = raw(path.join(seed.directory, 'original-loader.rs'));
  assert.equal(sha256(loader), record.loader_sha256); assert.equal(UTF8.decode(loader), LOADER);
  const fixture = path.join(repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device');
  const fixtureNames = ['Cargo.toml', 'src/lib.rs', 'src/source_bitselect_feasibility.rs', 'src/source_bitselect_normalized.rs'];
  fixtureNames.forEach((name, index) => assert.equal(sha256(raw(path.join(fixture, name))), record.fixture_sha256[index]));
  assert.deepEqual(original, raw(path.join(fixture, fixtureNames[2])), 'closed no-prefix original');
  assert.equal(sha256(raw(path.join(prepared, 'metadata.stdout'), JSON_LIMIT)), record.metadata_sha256);
  assert.equal(sha256(raw(path.join(prepared, 'dependencies.stdout'), JSON_LIMIT)), record.artifacts_sha256);
  for (const file of [consumer, emitter, seed.device, seed.core, record.args[0], path.join(seed.sysroot, 'bin/cargo'),
    ...['fe2o3-export-sim', 'fe2o3-rustc-extract', 'fe2o3-program-inspect', 'fe2o3-kir-sim'].map(name => path.join(binDir, name))]) {
    assert.ok(pins.has(file), 'actual tool/dependency absent from producer pins');
    natural(pins.get(file).bytes, 512 * MiB, 1);
  }
  const candidate = raw(path.join(seed.directory, 'instruction-default.rs'));
  assert.deepEqual(candidate, fromOutput('public-candidate.rs'));
  const edited = verifySourceEdit(original, candidate);
  assert.deepEqual(edited, fromOutput('instruction-edited.rs'));
  const template = path.join(repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30');
  let manifest = raw(path.join(template, 'Cargo.toml'));
  for (const dependency of ['fe2o3-device', 'fe2o3-host']) manifest = replaceOnce(manifest,
    '"../../../../' + dependency + '"', JSON.stringify(path.join(repo, 'crates', dependency)));
  manifest = replaceOnce(manifest, 'edited = []',
    'source-bitselect-feasibility = []\nsource-bitselect-ambiguous = []\nsource-bitselect-local-alias = []');
  const lock = raw(path.join(template, 'Cargo.lock'), JSON_LIMIT);
  for (const [label, source] of [['default', candidate], ['edited', edited]]) {
    assert.deepEqual(fromOutput(label + '-source/src/kernel.rs'), source);
    assert.equal(UTF8.decode(fromOutput(label + '-source/src/lib.rs')), EXPORT_LOADER);
    assert.deepEqual(fromOutput(label + '-source/Cargo.toml'), manifest, 'exact current template/dependency/features clone');
    assert.deepEqual(fromOutput(label + '-source/Cargo.lock', JSON_LIMIT), lock);
  }
  assert.deepEqual(fromOutput('default-source/Cargo.toml'), fromOutput('edited-source/Cargo.toml'));
  assert.deepEqual(fromOutput('default-source/Cargo.lock', JSON_LIMIT), fromOutput('edited-source/Cargo.lock', JSON_LIMIT));
  assert.ok(Array.isArray(receipt.stages) && receipt.stages.length === 100, 'one public seed plus three complete 33-stage lanes');
  let next = 0;
  const stage = (label, executable, args) => {
    const value = receipt.stages[next++];
    exact(value, ['label', 'executable', 'args', 'code', 'signal', 'reason', 'elapsed_ms',
      'stdout_bytes', 'stdout_sha256', 'stderr_bytes', 'stderr_sha256']);
    assert.equal(value.label, label); assert.equal(value.executable, executable); assert.deepEqual(value.args, args);
    assert.equal(value.code, 0); assert.equal(value.signal, null); assert.equal(value.reason, null);
    natural(value.elapsed_ms, 310000);
    const streams = {};
    for (const name of ['stdout', 'stderr']) {
      const bytes = fromOutput(label + '.' + name, MiB, true);
      identity(bytes, { bytes: value[name + '_bytes'], sha256: value[name + '_sha256'] }, MiB, true);
      streams[name] = bytes;
    }
    return streams;
  };
  const published = stage('public-seed', consumer, [
    path.relative(repo, path.join(seed.directory, 'original.rs')),
    path.relative(repo, path.join(seed.directory, 'instruction-default.rs')), record.source_sha256, '--', ...record.args]);
  assert.deepEqual(receipt.normal_public_seed, parsePublication(published.stdout, original, candidate));
  assert.ok(Array.isArray(receipt.variants) && receipt.variants.length === 3);
  const summaries = [];
  for (const [index, label] of ['default', 'edited', 'repeat'].entries()) {
    const variant = receipt.variants[index], profile = label === 'default' ? 'default' : 'edited';
    exact(variant, ['label', 'source_sha256', 'exported', 'inspection', 'emission', 'kir_file_sha256', 'llvm_sha256', 'simulations']);
    assert.equal(variant.label, label); assert.equal(variant.source_sha256, sha256(profile === 'default' ? candidate : edited));
    const kirPath = path.join(sourceRoot, label + '.kir'), llvmPath = path.join(sourceRoot, label + '.ll');
    const exportedLog = stage(label + '-export', path.join(binDir, 'fe2o3-export-sim'), [
      '--diagnostic-kir-v17', '--crate', 'fe2o3_assembly_authoring_v30_fixture', '--output', kirPath,
      '--target', 'gfx942', '--target-dir', path.join(sourceRoot, label + '-export-target'), '--', '--manifest-path',
      path.join(sourceRoot, profile + '-source/Cargo.toml'), '--lib', '--offline', '--features', 'source-bitselect-feasibility']);
    const exported = parseExport(exportedLog.stderr, 'required'); assert.deepEqual(variant.exported, exported);
    const kir = raw(kirPath); assert.equal(kir.length, exported.canonical_bytes);
    assert.equal(variant.kir_file_sha256, sha256(kir));
    assert.deepEqual(parseJson(fromOutput(label + '-inspect-request.json', JSON_LIMIT)), request(INPUTS[0], 1));
    const inspect = stage(label + '-inspect', path.join(binDir, 'fe2o3-program-inspect'),
      [kirPath, path.join(sourceRoot, label + '-inspect-request.json')]);
    const inspection = parseJson(inspect.stdout); verifyInspector(inspection, exported, profile);
    assert.deepEqual(variant.inspection, inspection);
    const lowered = stage(label + '-llvm', emitter, [kirPath, llvmPath]);
    const llvm = raw(llvmPath), emission = parseJson(lowered.stdout);
    verifyEmission(emission, exported, profile, kir, llvm); assert.deepEqual(variant.emission, emission);
    assert.equal(variant.llvm_sha256, sha256(llvm));
    const simulations = [];
    for (let input = 0; input < INPUTS.length; input++) for (const length of LENGTHS) {
      const name = label + '-case-' + input + '-length-' + length, requestPath = path.join(sourceRoot, name + '.request.json');
      const query = parseJson(raw(requestPath, JSON_LIMIT));
      for (let replay = 0; replay < 2; replay++) {
        const output = stage(name + '-replay-' + replay, path.join(binDir, 'fe2o3-kir-sim'),
          ['--diagnostic-kir-v17', kirPath, '--request', requestPath]);
        const result = parseJson(output.stdout);
        const checked = verifySimulation(result, query, exported, profile, INPUTS[input], length);
        simulations.push({ input, length, replay, ...checked });
      }
    }
    assert.deepEqual(variant.simulations, simulations, 'all 30 per-variant raw results join to complete census');
    summaries.push({ label, profile, source_sha256: variant.source_sha256, semantic_sha256: exported.semantic_identity,
      canonical_kir_sha256: exported.canonical_sha256, canonical_kir_bytes: exported.canonical_bytes,
      kir_file_sha256: sha256(kir), llvm_path: llvmPath, llvm_sha256: sha256(llvm), llvm_bytes: llvm.length,
      retained_source_inventory: exported.retained_source_inventory, retained_source_preflight: exported.retained_source_preflight,
      declared_source_ids: inspection.declared_source_ids, simulations: simulations.length });
  }
  assert.equal(next, 100); verifyVariants(receipt.variants, 'required');
  assert.notEqual(oracle('default', INPUTS[0]), oracle('edited', INPUTS[0]), 'distinguishing whole-kernel oracle');
  return { variants: summaries, stages: next, simulations: 90, pins };
}

const COMMON_STEPS = [
  ['V_XOR_B32_e32_vi', '0003082a', ['VGPR4', 'VGPR0', 'VGPR1']],
  ['V_AND_B32_e32_vi', '04050826', ['VGPR4', 'VGPR4', 'VGPR2']],
];
const POST_LINK = [
  'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
  'post_link.check=exports status=ok symbols=[choose_bits,choose_bits.kd]',
  'post_link.check=unresolved status=ok symbols=[]',
  'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-',
];
function nativeExpectations(expected) {
  exact(expected, ['profile', 'llvm_sha256', 'llvm_bytes', 'payloadDirectory', 'llvmClaim', 'workerClaim']);
  assert.ok(expected.profile === 'default' || expected.profile === 'edited');
  absolute(expected.payloadDirectory); digest(expected.llvm_sha256); natural(expected.llvm_bytes, SOURCE_LIMIT, 1);
  assert.equal(expected.llvmClaim, 'rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540');
  assert.match(expected.workerClaim, /^fe2o3-worker-v1-sha256-[0-9a-f]{64}$/u);
  assert.notEqual(expected.workerClaim, 'fe2o3-worker-v1-sha256-' + '0'.repeat(64));
}
export function validateNativeReport(report, expected, llvmBytes, readPayload) {
  nativeExpectations(expected); identity(llvmBytes, { bytes: expected.llvm_bytes, sha256: expected.llvm_sha256 }, SOURCE_LIMIT);
  exact(report, ['report_kind', 'authority', 'profile', 'kernel_symbol', 'register_plan', 'descriptors',
    'result_use', 'llvm_sha256', 'llvm_bytes', 'expected_input_identity_matched',
    'retained_file_identity_and_bytes_rechecked', 'llvm_build_claim', 'worker_build_claim', 'target',
    'wave_width', 'workgroup_size', 'code_object_version', 'shape_controls', 'cases', 'source_ancestry',
    'synthetic_worker_request_identity_fields', 'runtime_closure_attestation', ...NATIVE_FALSE]);
  assert.equal(report.report_kind, 'private-instruction-edit-native-observation-v1');
  assert.equal(report.authority, 'unauthenticated-test-transport'); assert.equal(report.profile, expected.profile);
  assert.equal(report.kernel_symbol, 'choose_bits'); assert.deepEqual(report.register_plan, [4, 5, 0, 1, 2]);
  assert.deepEqual(report.descriptors, descriptors(expected.profile).slice(0, 3));
  assert.equal(report.result_use, 'sole-direct-nonvolatile-nonatomic-global-store');
  assert.equal(report.expected_input_identity_matched, true); assert.equal(report.retained_file_identity_and_bytes_rechecked, true);
  assert.equal(report.llvm_sha256, expected.llvm_sha256); assert.equal(report.llvm_bytes, expected.llvm_bytes);
  assert.equal(report.llvm_build_claim, expected.llvmClaim); assert.equal(report.worker_build_claim, expected.workerClaim);
  assert.equal(report.target, 'gfx942:xnack-'); assert.equal(report.wave_width, 64);
  assert.equal(report.workgroup_size, 64); assert.equal(report.code_object_version, 6);
  assert.equal(report.source_ancestry, 'not-established-by-llvm-file');
  assert.equal(report.synthetic_worker_request_identity_fields, true);
  assert.equal(report.runtime_closure_attestation, 'unavailable');
  NATIVE_FALSE.forEach(key => assert.equal(report[key], false)); assert.deepEqual(report.shape_controls, SHAPE);
  assert.ok(Array.isArray(report.cases) && report.cases.length === 2, 'exact complete O0/O3 cases');
  const steps = [...COMMON_STEPS, expected.profile === 'default'
    ? ['V_XOR_B32_e32_vi', '01090a2a', ['VGPR5', 'VGPR1', 'VGPR4']]
    : ['V_OR_B32_e32_vi', '01090a28', ['VGPR5', 'VGPR1', 'VGPR4']]];
  const summaries = [];
  for (const [index, wrapper] of report.cases.entries()) {
    exact(wrapper, ['machine_observation', 'mutation_controls', 'retained_payload']);
    assert.deepEqual(wrapper.mutation_controls, MUTATIONS);
    const value = wrapper.machine_observation, retained = wrapper.retained_payload, optimization = ['O0', 'O3'][index];
    exact(value, ['optimization', 'llvm_sha256', 'llvm_bytes', 'hsaco_sha256', 'hsaco_bytes',
      'entry_file_offset', 'entry_code_bytes', 'static_instruction_count', 'program', 'descriptor',
      'post_link_checks', 'derivation_identity', 'boundary_value_or_lifetime_proof']);
    assert.equal(value.optimization, optimization);
    assert.equal(value.llvm_sha256, expected.llvm_sha256); assert.equal(value.llvm_bytes, expected.llvm_bytes);
    assert.equal(value.boundary_value_or_lifetime_proof, false); digest(value.derivation_identity);
    exact(retained, ['path', 'bytes', 'sha256', 'matches_linked_worker_payload', 'create_new_only', 'production_artifact_authority']);
    assert.equal(retained.path, path.join(expected.payloadDirectory, optimization + '.hsaco'));
    assert.equal(retained.bytes, value.hsaco_bytes); assert.equal(retained.sha256, value.hsaco_sha256);
    assert.equal(retained.matches_linked_worker_payload, true); assert.equal(retained.create_new_only, true);
    assert.equal(retained.production_artifact_authority, false);
    const payload = readPayload(retained.path, PAYLOAD_LIMIT);
    identity(payload, retained, PAYLOAD_LIMIT);
    natural(value.entry_file_offset, payload.length);
    natural(value.entry_code_bytes, payload.length - value.entry_file_offset, 12);
    natural(value.static_instruction_count, 512, 3);
    assert.ok(Array.isArray(value.program) && value.program.length === 3);
    for (const [at, site] of value.program.entries()) {
      exact(site, ['file_offset', 'opcode', 'bytes_hex', 'mc_flags', 'register_operands', 'implicit_reads', 'implicit_writes']);
      assert.deepEqual([site.opcode, site.bytes_hex, site.register_operands], steps[at]);
      natural(site.file_offset, value.entry_file_offset + value.entry_code_bytes - 4, value.entry_file_offset);
      natural(site.mc_flags, 65535); assert.equal(site.mc_flags & ~16, 0);
      assert.deepEqual(site.implicit_reads, ['EXEC']); assert.deepEqual(site.implicit_writes, []);
      assert.equal(site.file_offset, value.program[0].file_offset + 4 * at, 'exact contiguous instruction order');
      // The unchanged worker maps symbol virtual addresses through ELF section
      // sh_offset, then functionFileOffset. These are FULL PAYLOAD offsets.
      // Never scan for matching bytes or reinterpret them as section-relative.
      assert.equal(payload.subarray(site.file_offset, site.file_offset + 4).toString('hex'),
        site.bytes_hex, 'instruction bytes at exact complete-HSACO file offset');
    }
    const descriptor = value.descriptor;
    exact(descriptor, ['file_offset', 'bytes', 'sha256', 'compute_pgm_rsrc1', 'compute_pgm_rsrc3',
      'vgpr_capacity', 'architected_vgpr_boundary', 'required_footprint_high_water', 'interpretation']);
    natural(descriptor.file_offset, payload.length - 64);
    assert.equal(descriptor.bytes, 64); assert.equal(descriptor.required_footprint_high_water, 6);
    assert.equal(descriptor.interpretation, 'encoded-capacity-not-metadata-usage-or-lifetime');
    assert.ok(descriptor.file_offset + 64 <= value.entry_file_offset ||
      value.entry_file_offset + value.entry_code_bytes <= descriptor.file_offset, 'descriptor and entry disjoint');
    const rawDescriptor = payload.subarray(descriptor.file_offset, descriptor.file_offset + 64);
    identity(rawDescriptor, descriptor, 64);
    natural(descriptor.compute_pgm_rsrc1, 0xffffffff); natural(descriptor.compute_pgm_rsrc3, 0xffffffff);
    assert.equal(rawDescriptor.readUInt32LE(48), descriptor.compute_pgm_rsrc1);
    assert.equal(rawDescriptor.readUInt32LE(44), descriptor.compute_pgm_rsrc3);
    assert.equal(descriptor.vgpr_capacity, ((descriptor.compute_pgm_rsrc1 & 63) + 1) * 8);
    assert.equal(descriptor.architected_vgpr_boundary, ((descriptor.compute_pgm_rsrc3 & 63) + 1) * 4);
    assert.ok(descriptor.vgpr_capacity >= descriptor.architected_vgpr_boundary &&
      descriptor.architected_vgpr_boundary >= 6, 'descriptor capacity covers authored v0..v5');
    assert.ok(Array.isArray(value.post_link_checks) && value.post_link_checks.length === 5 &&
      value.post_link_checks.every(text => typeof text === 'string' && Buffer.byteLength(text) <= 1024));
    assert.deepEqual(value.post_link_checks.slice(0, 4), POST_LINK);
    const launch = /^post_link.kernel name=choose_bits symbol=choose_bits.kd kernarg_size=([0-9]{1,10}) group_size=([0-9]{1,10}) private_size=([0-9]{1,10}) kernarg_align=([1-9][0-9]{0,4}) wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=\[64,1,1\]$/u.exec(value.post_link_checks[4]);
    assert.ok(launch, 'exact existing native launch reporting contract');
    launch.slice(1, 4).forEach(number => {
      assert.match(number, /^(?:0|[1-9][0-9]*)$/u); natural(Number(number), 0xffffffff);
    });
    const alignment = natural(Number(launch[4]), 32768, 1);
    assert.equal(alignment & (alignment - 1), 0);
    summaries.push({ profile: expected.profile, optimization, llvm_sha256: expected.llvm_sha256,
      llvm_bytes: expected.llvm_bytes, payload_path: retained.path, hsaco_sha256: retained.sha256, hsaco_bytes: retained.bytes,
      exact_payload_instruction_offsets: value.program.map(site => site.file_offset),
      descriptor_file_offset: descriptor.file_offset, descriptor_sha256: descriptor.sha256,
      descriptor_bytes: 64, source_authentication: false, boundary_value_or_lifetime_proof: false });
  }
  return summaries;
}

export function options(argv) {
  const paths = ['repo', 'prepared-root', 'source-output', 'consumer', 'bin-dir', 'emitter',
    'default-report', 'default-payload-dir', 'edited-report', 'edited-payload-dir', 'output'];
  const hashes = ['source-receipt-sha256', 'default-report-sha256', 'edited-report-sha256'];
  const sizes = ['source-receipt-bytes', 'default-report-bytes', 'edited-report-bytes'];
  const keys = [...paths, ...hashes, ...sizes, 'llvm-build-id', 'worker-build-id'], out = {};
  assert.equal(argv.length, keys.length * 2, 'closed CLI key count');
  for (let at = 0; at < argv.length; at += 2) {
    assert.ok(typeof argv[at] === 'string' && argv[at].startsWith('--'));
    const key = argv[at].slice(2); assert.ok(keys.includes(key) && !Object.hasOwn(out, key));
    out[key] = argv[at + 1];
  }
  paths.forEach(key => absolute(out[key])); hashes.forEach(key => digest(out[key]));
  sizes.forEach(key => {
    assert.match(out[key], /^[1-9][0-9]{0,6}$/u);
    out[key] = natural(Number(out[key]), key === 'source-receipt-bytes' ? JSON_LIMIT : SOURCE_LIMIT, 1);
  });
  nativeExpectations({ profile: 'default', llvm_sha256: out['source-receipt-sha256'], llvm_bytes: 1,
    payloadDirectory: out['default-payload-dir'], llvmClaim: out['llvm-build-id'], workerClaim: out['worker-build-id'] });
  assert.notEqual(out['default-report'], out['edited-report'], 'distinct selected native reports');
  assert.notEqual(out['default-payload-dir'], out['edited-payload-dir'], 'distinct profile payload directories');
  for (const root of ['repo', 'prepared-root', 'source-output', 'default-payload-dir', 'edited-payload-dir'])
    assert.ok(out.output !== out[root] && !within(out[root], out.output), 'new report must be outside retained input trees');
  assert.ok(out['source-output'] !== out.repo && !within(out.repo, out['source-output']));
  assert.ok(out['prepared-root'] !== out['source-output'] &&
    !within(out['prepared-root'], out['source-output']) && !within(out['source-output'], out['prepared-root']));
  return out;
}

function snapshot(stat) {
  return { device: String(stat.dev), inode: String(stat.ino), mode: String(stat.mode), nlink: String(stat.nlink),
    bytes: Number(stat.size), mtime_ns: String(stat.mtimeNs), ctime_ns: String(stat.ctimeNs) };
}
function sameStat(before, after) {
  for (const key of ['dev', 'ino', 'mode', 'nlink', 'size', 'mtimeNs', 'ctimeNs']) assert.equal(after[key], before[key], 'same retained file');
}
// Runtime-only adapter: pure validators above never select files or execute tools.
class RetainedInputs {
  pins = new Map(); charged = 0; readBytes = 0; started = performance.now();
  checkpoint() { assert.ok(performance.now() - this.started <= 120000, 'join wall-clock budget exceeded'); }
  observe(file, cap, keep = false, empty = false) {
    this.checkpoint(); absolute(file); natural(cap, 512 * MiB, 1);
    assert.equal(fs.realpathSync(file), file, 'no input symlink traversal');
    const descriptor = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
    try {
      const before = fs.fstatSync(descriptor, { bigint: true });
      assert.ok(before.isFile() && before.size <= BigInt(cap) && (empty || before.size > 0n));
      const hash = createHash('sha256'), chunks = [], scratch = Buffer.alloc(64 * KiB); let length = 0;
      for (;;) {
        this.checkpoint();
        const count = fs.readSync(descriptor, scratch, 0, scratch.length, null); if (count === 0) break;
        length += count; this.readBytes += count; natural(length, cap); natural(this.readBytes, 6 * GiB);
        hash.update(scratch.subarray(0, count));
        if (keep) chunks.push(Buffer.from(scratch.subarray(0, count)));
      }
      assert.equal(BigInt(length), before.size);
      sameStat(before, fs.fstatSync(descriptor, { bigint: true }));
      sameStat(before, fs.lstatSync(file, { bigint: true }));
      assert.equal(fs.realpathSync(file), file);
      const pin = { path: file, ...snapshot(before), sha256: hash.digest('hex') }, prior = this.pins.get(file);
      if (prior) assert.deepEqual(pin, prior, 'retained input changed between observations');
      else {
        assert.ok(this.pins.size < JOIN_PIN_LIMIT); this.charged += length; natural(this.charged, JOIN_PIN_BYTES);
        this.pins.set(file, pin);
      }
      return { pin, bytes: keep ? Buffer.concat(chunks) : undefined };
    } finally { fs.closeSync(descriptor); }
  }
  read(file, cap) { return this.observe(file, cap, true, true).bytes; }
  recheck() {
    for (const pin of this.pins.values()) this.observe(pin.path, Math.max(pin.bytes, 1), false, pin.bytes === 0);
  }
}

function canonicalDirectory(directory) {
  absolute(directory); assert.equal(fs.realpathSync(directory), directory);
  const stat = fs.lstatSync(directory, { bigint: true }); assert.ok(stat.isDirectory()); return stat;
}
function sameDirectory(directory, before) {
  const after = canonicalDirectory(directory);
  for (const key of ['dev', 'ino', 'mode']) assert.equal(after[key], before[key], 'selected directory changed');
}
function createReport(file, bytes, directoryStat) {
  buffer(bytes, JSON_LIMIT); const parent = path.dirname(file); sameDirectory(parent, directoryStat);
  const descriptor = fs.openSync(file, fs.constants.O_RDWR | fs.constants.O_CREAT | fs.constants.O_EXCL | fs.constants.O_NOFOLLOW, 0o600);
  try {
    let at = 0;
    while (at < bytes.length) {
      const count = fs.writeSync(descriptor, bytes, at, bytes.length - at);
      assert.ok(count > 0); at += count;
    }
    fs.fsyncSync(descriptor);
    const actual = fs.fstatSync(descriptor, { bigint: true }), retained = Buffer.alloc(bytes.length);
    assert.ok(actual.isFile()); assert.equal(actual.size, BigInt(bytes.length));
    let offset = 0;
    while (offset < retained.length) {
      const count = fs.readSync(descriptor, retained, offset, retained.length - offset, offset);
      assert.ok(count > 0); offset += count;
    }
    assert.deepEqual(retained, bytes, 'new report reread differs from exact joined JSON');
    sameStat(actual, fs.fstatSync(descriptor, { bigint: true }));
    sameStat(actual, fs.lstatSync(file, { bigint: true }));
    assert.equal(fs.realpathSync(file), file);
  } finally { fs.closeSync(descriptor); }
  sameDirectory(parent, directoryStat);
  const parentFd = fs.openSync(parent, fs.constants.O_RDONLY | fs.constants.O_DIRECTORY | fs.constants.O_NOFOLLOW);
  try { fs.fsyncSync(parentFd); } finally { fs.closeSync(parentFd); }
}

function run(opt) {
  const retained = new RetainedInputs(), parent = path.dirname(opt.output), parentStat = canonicalDirectory(parent);
  assert.ok(!fs.existsSync(opt.output), 'new-only report output required');
  const directories = new Map();
  for (const key of ['repo', 'prepared-root', 'source-output', 'bin-dir', 'default-payload-dir', 'edited-payload-dir'])
    directories.set(opt[key], canonicalDirectory(opt[key]));
  assert.ok(!fs.existsSync(path.join(opt['source-output'], 'failure.json')), 'producer failure marker forbids qualification');
  for (const profile of ['default', 'edited']) assert.deepEqual(fs.readdirSync(opt[profile + '-payload-dir']).sort(),
    ['O0.hsaco', 'O3.hsaco'], 'complete exact payload-directory census');
  const sourceReceiptPath = path.join(opt['source-output'], 'receipt.json');
  const sourceBytes = retained.read(sourceReceiptPath, JSON_LIMIT);
  identity(sourceBytes, { bytes: opt['source-receipt-bytes'], sha256: opt['source-receipt-sha256'] }, JSON_LIMIT);
  const receipt = parseJson(sourceBytes), ledger = sourcePinLedger(receipt);
  // Stream ALL selected producer pins, not only the attractive successful leaves.
  for (const pin of ledger.values()) {
    const actual = retained.observe(pin.path, Math.max(pin.bytes, 1), false, pin.bytes === 0).pin;
    for (const key of PIN_KEYS) assert.equal(actual[key], pin[key], 'unchanged complete producer pin ledger');
  }
  const source = validateSourceReceipt(receipt, { repo: opt.repo, prepared: opt['prepared-root'],
    sourceRoot: opt['source-output'], consumer: opt.consumer, binDir: opt['bin-dir'], emitter: opt.emitter },
  (file, cap) => retained.read(file, cap));
  const native = [], reportPins = [];
  for (const profile of ['default', 'edited']) {
    const selected = source.variants.find(variant => variant.label === profile);
    const reportPath = opt[profile + '-report'], bytes = retained.read(reportPath, SOURCE_LIMIT);
    identity(bytes, { bytes: opt[profile + '-report-bytes'], sha256: opt[profile + '-report-sha256'] }, SOURCE_LIMIT);
    const report = parseJson(bytes, SOURCE_LIMIT), llvm = retained.read(selected.llvm_path, SOURCE_LIMIT);
    native.push(...validateNativeReport(report, { profile, llvm_sha256: selected.llvm_sha256, llvm_bytes: selected.llvm_bytes,
      payloadDirectory: opt[profile + '-payload-dir'], llvmClaim: opt['llvm-build-id'], workerClaim: opt['worker-build-id'] },
    llvm, (file, cap) => retained.read(file, cap)));
    reportPins.push(retained.pins.get(reportPath));
  }
  assert.equal(native.length, 4); retained.recheck();
  for (const [directory, stat] of directories) sameDirectory(directory, stat);
  assert.ok(!fs.existsSync(path.join(opt['source-output'], 'failure.json')), 'producer failed during retained join');
  for (const profile of ['default', 'edited']) assert.deepEqual(fs.readdirSync(opt[profile + '-payload-dir']).sort(), ['O0.hsaco', 'O3.hsaco']);
  assert.ok(!retained.pins.has(opt.output)); retained.checkpoint();
  const report = {
    kind: 'private_instruction_edit_source_native_payload_join_v1', status: 'joined_observation',
    authority: 'observation_only', source_receipt: retained.pins.get(sourceReceiptPath), native_reports: reportPins,
    source_variants: source.variants, normal_public_seed: receipt.normal_public_seed,
    actual_normal_exports: 3, producer_stages: source.stages, whole_kernel_simulations: source.simulations,
    native_optimization_cases: 4, complete_hsaco_payloads: 4, native_observations: native,
    native_repeat_execution: false, repeat_scope: 'same edited source semantic KIR LLVM identities; no separate native execution claimed',
    full_payload_offset_join: 'worker ELF file offsets and complete payload bytes; no byte scanning',
    source_edit: receipt.source_edit, changed_oracle: 'default bitselect; edited b | (a & mask); independent BigInt whole-kernel simulation',
    source_authentication: false, compiler_closure_attestation: false, runtime_closure_attestation: 'unavailable',
    protected_admission: false, ranked_checks: false, proof_authority: false, artifact_authority: false,
    production_resume: false, hardware_execution: false, native_whole_kernel_correctness: false,
    physical_register_allocation_or_lifetime_proof: false, whole_kernel_order_or_byte_stability_claim: false,
    retained_input_pins: [...retained.pins.values()], retained_input_bytes: retained.charged,
    limits: { ...LIMITS, total_read_bytes: 6 * GiB, wall_clock_ms: 120000, actual_read_bytes: retained.readBytes,
      input_directory_custody: 'task-controlled parents; outer supervisor and post-exit snapshot required',
      compiler_worker_execution: false, total_RSS_or_task_storage_attestation: false },
  };
  const output = Buffer.from(JSON.stringify(report, null, 2) + '\n'); buffer(output, JSON_LIMIT);
  createReport(opt.output, output, parentStat);
  process.stdout.write('Joined observation: ' + opt.output + ' ' + output.length + ' ' + sha256(output) + '\n');
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try { run(options(process.argv.slice(2))); }
  catch (error) { process.stderr.write(String(error).slice(0, 4096) + '\n'); process.exitCode = 1; }
}
