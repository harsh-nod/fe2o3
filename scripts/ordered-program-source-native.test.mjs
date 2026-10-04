// Pure synthetic-shape controls only; no child process or qualification claim.
import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { DESCRIPTORS, SOURCE_VARIANTS, SCALAR_CASES, sha256, parseBoundedJson, validateSourceReceipt,
  programResult, validateSimulationJoin, validateEmission, validateMachineObservation,
  parseArguments, validateSourceExportLine } from './ordered-program-source-native.mjs';
import { EXPECTED_PROGRAMS, LLVM_BUILD_ID, MAX_CACHE_BYTES, MIN_RAM_BYTES } from './ordered-program-worker-prototype.mjs';
import { MIN_FREE_BYTES } from './assembly-region-worker-prototype.mjs';

const H = 'a'.repeat(64), C = 'b'.repeat(64), W = `fe2o3-worker-v1-sha256-${'c'.repeat(64)}`;
const REPO = '/synthetic/compiler', SOURCE = '/synthetic/source', LLVM = Buffer.from('; synthetic LLVM bytes, not execution evidence\n');
const clone = value => structuredClone(value);
function artifact(variant = SOURCE_VARIANTS[0]) {
  const directory = path.join(SOURCE, variant.name);
  return { name: variant.name, directory, kirPath: path.join(directory, 'kernel.kir'), status: 'exported',
    canonical_sha256: C, canonical_bytes: 2048, observed_retained_source_inventory: H,
    observed_retained_source_preflight: H, source_authentication_carried_by_raw_bytes: false,
    file_sha256: H, file_bytes: 2048 };
}
function emission(variant = SOURCE_VARIANTS[0]) {
  return { kind: 'diagnostic_ordered_program_llvm_observation', authority: 'observation_only', canonical_wire_version: 17,
    canonical_identity: C, canonical_bytes: 2048, input_file_sha256: H, llvm_sha256: sha256(LLVM), llvm_bytes: LLVM.length,
    program_count: DESCRIPTORS[variant.profile].length,
    descriptors: [...DESCRIPTORS[variant.profile], ...Array(16 - DESCRIPTORS[variant.profile].length).fill(0)], register_plan: [32, 33, 34, 35, 36],
    canonical_retained_storage_bytes: 4096, canonical_work_limit: 1 << 26, canonical_storage_limit: 64 * 1024 ** 2,
    max_input_bytes: 65536, max_published_llvm_bytes: 65536, emitter_text_limit_bytes: 16 * 1024 ** 2,
    canonical_and_emitter_accounting_are_separate: true, source_authentication: false, compiler_closure_attestation: false,
    proof_authority: false, protected_admission: false, final_artifact_authority: false, production_resume: false,
    physical_register_values: false, hardware_execution: false };
}
function machine(variant = SOURCE_VARIANTS[0]) {
  const expected = EXPECTED_PROGRAMS[variant.profile], e = emission(variant);
  const cases = ['O0', 'O3'].map(optimization => ({ optimization, profile: variant.profile, program_count: expected.length,
    result_used: variant.used, llvm_text_sha256: e.llvm_sha256, llvm_text_bytes: e.llvm_bytes, hsaco_sha256: H, hsaco_bytes: 2048,
    descriptor_sha256: C, entry_file_offset: 512, entry_code_bytes: 256, static_instruction_count: expected.length + 12,
    post_link_inspection_diagnostics: [], program: expected.map((item, i) => ({ ...clone(item), file_offset: 544 + i * 4,
      mc_flags: 16, implicit_reads: ['EXEC'], implicit_writes: [] })), boundary_register_site_count: 0, boundary_sites: [],
    boundary_sites_truncated: false, boundary_observation_is_value_or_lifetime_proof: false,
    descriptor_resources: { descriptor_file_offset: 128, descriptor_bytes: 64, descriptor_sha256: C,
      compute_pgm_rsrc1: 4, compute_pgm_rsrc3: 9, vgpr_capacity: 40, architected_vgpr_boundary: 40,
      required_footprint_high_water: 37, interpretation: 'encoded-capacity-not-metadata-usage-or-lifetime' } }));
  return { schema: 'fe2o3-ordered-program-llvm-machine-observation-v1', authority: 'unauthenticated-test-transport',
    source_ancestry: 'not-established-by-llvm-file', synthetic_worker_request_identity_fields: true,
    production_exact_program_admission: false, protected_finalizer_admission: false, hardware_executed: false,
    runtime_closure_attestation: 'unavailable', physical_register_allocation_or_lifetime_proof: false,
    whole_kernel_order_or_byte_stability_claim: false, target: 'gfx942:xnack-', wave_width: 64, workgroup_size: 64,
    code_object_version: 6, kernel_symbol: 'ordered_u32_program', profile: variant.profile, program_count: expected.length,
    result_used: variant.used, result_use_scope: 'checked SSA use in supplied LLVM; not source authentication',
    expected_program: expected.map(item => ({ mnemonic: item.opcode.replace(/^V_/, 'v_').replace(/_(?:vi|gfx9)$/, '').toLowerCase(), ...clone(item) })),
    llvm_text_sha256: e.llvm_sha256, llvm_text_bytes: e.llvm_bytes, llvm_build_claim: LLVM_BUILD_ID, worker_build_claim: W,
    encoding_reference_scope: 'public gfx900 cross-check; actual pinned gfx942 qualification is this run', cases };
}
function sourceReceipt() {
  const refusals = [['alias', 'ordered program physical roles must be distinct v0..v63'],
    ['dynamic', 'ordered program physical role is not an actual MIR constant'],
    ['divergent', 'ordered program requires bounded unconditional acyclic source placement'],
    ['wrong-launch', 'ordered program requires required and maximum 64x1x1 workgroup bounds'],
    ...['invalid-count', 'invalid-opcode', 'read-before-init', 'padding'].map(name => [name, 'ordered program descriptors, padding or definite initialization are invalid'])]
    .map(([name, diagnostic]) => ({ name: `ordered-program-${name}-v32`, status: 'expected_source_refusal', diagnostic, kir_absent: true }));
  const controls = ['wrong-wire', 'wrong-launch', 'duplicate-request-field', 'duplicate-input-option', 'mixed-input-options',
    ...Array.from({ length: 9 }, (_, i) => `schedule-before-io-${i}`), 'existing-simulator-output'];
  const names = ['rustc-version'];
  for (const item of [...SOURCE_VARIANTS, ...refusals]) {
    names.push(...['before-target-du', 'before-secondary-du', 'export', 'after-target-du', 'after-secondary-du'].map(suffix => `${item.name}-${suffix}`));
    if (item.profile) names.push(...SCALAR_CASES.map((_, i) => `${item.name}-sim-${i}`));
  }
  names.push(...controls);
  const rejected = new Set([...refusals.map(item => item.name + '-export'), ...controls]);
  const input = (requested, bytes = 100, hash = H) => ({ requested, resolved: requested, bytes, sha256: hash });
  const fixture = path.join(REPO, 'crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device');
  const binaries = ['fe2o3-export-sim', 'fe2o3-rustc-extract', 'fe2o3-kir-sim', 'fe2o3-debug', 'librustc_codegen_fe2o3.so']
    .map(name => input('/synthetic/bin/' + name));
  const exports = [...SOURCE_VARIANTS.map(artifact), ...refusals];
  const inputs = [path.join(fixture, 'Cargo.toml'), path.join(fixture, 'src/lib.rs'), path.join(fixture, 'src/ordered_program_v32.rs'),
    path.join(REPO, 'Cargo.toml'), path.join(REPO, 'Cargo.lock')].map(name => input(name));
  const rustc = input('/synthetic/toolchain/bin/rustc'), driver = input('/synthetic/toolchain/lib/librustc_driver-7bb70639c3ace5a4.so');
  inputs.push(...binaries, rustc, driver, ...exports.slice(0, 6).map(item => input(item.kirPath, item.file_bytes, item.file_sha256)));
  const rustcText = 'rustc 1.96.0-nightly\ncommit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9\nrelease: 1.96.0-nightly\n';
  const receipt = { schema: 'task-phase9-ordinary-program-source-qualification-v1', status: 'passed',
    scope: 'actual ordinary source export and logical CPU observation for the closed ordered-program profile', source_export_executed: true,
    exported_source_authentication: false, compiler_closure_attestation: false, protected_admission: false, production_resume: false,
    hardware_execution: false, physical_register_values: false, persisted_schedule: false,
    output: SOURCE, repo: REPO, source_target: '/synthetic/target', node: 'v22.22.3',
    limits: { cargo_ms: 300000, command_output_bytes_per_stream: 1024 ** 2, maximum_root_target_bytes: MAX_CACHE_BYTES.toString(),
      minimum_available_ram_bytes: MIN_RAM_BYTES.toString(), minimum_persistent_free_bytes: MIN_FREE_BYTES.toString(), jobs: 2 },
    stages: names.map((stage, i) => ({ stage, executable: '/synthetic/bin', args: [], code: rejected.has(stage) ? 1 : 0,
      signal: null, reason: null, elapsed_ms: 1, free_bytes_before: MIN_FREE_BYTES.toString(), stdout_sha256: H, stdout_bytes: 10,
      stderr_sha256: H, stderr_bytes: 10, log_stem: `${String(i).padStart(3, '0')}-${stage}` })),
    resource_guards: [...SOURCE_VARIANTS, ...refusals].flatMap(item => ['before', 'after'].map(suffix => ({
      stage: item.name + '-' + suffix, target_resolved: '/synthetic/target', target_bytes: '100', secondary_cache_bytes: '200',
      combined_cache_bytes: '300', available_ram_bytes: MIN_RAM_BYTES.toString(), free_persistent_bytes: MIN_FREE_BYTES.toString() }))),
    exports, simulations: Array.from({ length: 36 }, () => ({})),
    controls: controls.map(name => ({ name, stage: 'arguments', kind: 'invalid_command_line', input: null, status: 'expected_refusal' })), binaries_before: binaries,
    selected_library_measurements: {
      scope: 'explicit backend SONAME and pinned rustc-driver files in the exporter-configured loader search path; not a complete runtime closure',
      backend_soname: 'librustc_codegen_fe2o3.so', backend: clone(binaries[4]), rustc_driver: clone(driver),
      loader_search_directories: ['/synthetic/bin', '/synthetic/toolchain/lib'], runtime_closure_attestation: false },
    rustc_binary: clone(rustc), rustc_version: { text: rustcText, commit: '55e86c996809902e8bbad512cfb4d2c18be446d9' },
    inputs_before: inputs, inputs_after: clone(inputs),
    counts: { source_exports: 6, expected_source_refusals: 8, simulations: 36, ordinary_negative_controls: 15 } };
  const stages = new Map(receipt.stages.map(stage => [stage.stage, stage]));
  Object.assign(stages.get('rustc-version'), { executable: rustc.requested, args: ['-vV'],
    stdout_sha256: sha256(Buffer.from(rustcText)), stdout_bytes: Buffer.byteLength(rustcText), stderr_sha256: sha256(Buffer.alloc(0)), stderr_bytes: 0 });
  for (const variant of [...SOURCE_VARIANTS, ...refusals]) Object.assign(stages.get(`${variant.name}-export`), {
    executable: binaries[0].requested, args: ['--diagnostic-kir-v17', '--crate', 'fe2o3_production_extraction_fixture',
      '--output', path.join(SOURCE, variant.name, 'kernel.kir'), '--target', 'gfx942', '--target-dir', receipt.source_target,
      '--', '--manifest-path', path.join(fixture, 'Cargo.toml'), '-p', 'fe2o3-production-extraction-fixture',
      '--lib', '--no-default-features', '--features', variant.feature ?? variant.name, '--offline'] });
  // Independent explicit fixture commands, not imported from the validator.
  const first = SOURCE + '/ordered-program-one-v32', negative = SOURCE + '/negative-controls';
  const base = ['--diagnostic-kir-v17', first + '/kernel.kir', '--request', first + '/request-case-5.json'];
  const commands = [
    ['kir_admission', 'kir_v17_wrong_version', 'kir_v17', ['--diagnostic-kir-v17', negative + '/wrong-wire-v12.kir', '--request', first + '/request-case-5.json']],
    ['preflight', 'preflight_workgroup_mismatch', null, ['--diagnostic-kir-v17', first + '/kernel.kir', '--request', negative + '/wrong-launch.json']],
    ['request', 'request_json_duplicate_field', null, ['--diagnostic-kir-v17', first + '/kernel.kir', '--request', negative + '/duplicate-request-field.json']],
    ['arguments', 'invalid_command_line', null, [...base, '--diagnostic-kir-v17', first + '/kernel.kir']],
    ['arguments', 'invalid_command_line', null, [...base, '--bundle-v6', negative + '/missing-bundle']],
    ...[['--record-canonical-schedule', negative + '/must-not-be-published'], ['--record-seeded-schedule', negative + '/must-not-be-published'],
      ['--replay-schedule', negative + '/must-not-be-published'], ['--explore-seeded-schedules', '1'], ['--reduce-failure'],
      ['--replay-failure-reduction', negative + '/must-not-be-published'], ['--schedule-seed', '0'], ['--schedule-max-decisions', '1'],
      ['--exploration-max-retained-decisions', '1']].map(extra => ['arguments', 'schedule_input_unsupported', null,
      ['--diagnostic-kir-v17', negative + '/missing-kir', '--request', negative + '/missing-request', ...extra]]),
    ['output', 'output_already_exists', null, [...base, '--output', first + '/sim-case-5.json']],
  ];
  commands.forEach(([stage, kind, inputKind, args], index) => {
    Object.assign(receipt.controls[index], { stage, kind, input: inputKind });
    Object.assign(stages.get(controls[index]), { executable: binaries[2].requested, args, stdout_bytes: 0, stdout_sha256: sha256(Buffer.alloc(0)) });
  });
  return receipt;
}
function simulated(variant, caseIndex) {
  const inputs = SCALAR_CASES[caseIndex], value = programResult(variant.profile, inputs), output = variant.used ? value : inputs[0];
  const argumentsExpected = [{ kind: 'buffer_view', backing: 1, element: 'u32', access: 'read_write', alignment: 4, byte_offset: 4, elements: 64 },
    ...inputs.map(value => ({ kind: 'scalar', type: 'u32', bits: `0x${value.toString(16).padStart(8, '0')}` }))];
  const request = { schema: 'fe2o3-simulation-request-v1', kernel: 'ordered_u32_program', grid: [64, 1, 1], workgroup: [64, 1, 1],
    arguments: argumentsExpected, shared_buffers: [{ id: 1, element: 'u32', access: 'read_write', alignment: 4,
      bytes: `0x${'a5'.repeat(264)}`, initialized: `0x${'00'.repeat(33)}` }] };
  const bytes = Buffer.alloc(264, 0xa5); for (let lane = 0; lane < 64; lane++) bytes.writeUInt32LE(output, 4 + lane * 4);
  const record = { name: variant.name, case: caseIndex, inputs, used: variant.used, program_profile: variant.profile, before_inputs: inputs,
    operand_order: [0, 1, 2], canonical_sha256: C, canonical_bytes: 2048, expected_output: output, expected_program_result: value,
    guard_bytes_unchanged: true, guard_bytes_uninitialized: true, checked_lanes: 64 };
  const result = { schema: 'fe2o3-simulation-result-v1', status: 'ok', authority: 'observation_only', simulated: true,
    hardware_observed: false, hardware_validation: false, performance_prediction: false, kir: { sha256: C, canonical_bytes: 2048 },
    arguments: clone(argumentsExpected), counts: { arguments: 4, shared_buffers: 1, invocations_executed: 64, workgroups_visited: 1 },
    target_profile: { identity: 'amdgpu_64_little_endian_v1', index_bits: 64 }, shared_buffers: [{ id: 1, buffer: {
      element: 'u32', access: 'read_write', alignment: 4, bytes: `0x${bytes.toString('hex')}`, initialized: `0xf0${'ff'.repeat(31)}0f` } }] };
  return { record, request, result };
}

test('pure: six variants join descriptor/role/LLVM identities to twelve cases and eighty sites', () => {
  let cases = 0, sites = 0;
  for (const variant of SOURCE_VARIANTS) {
    const e = validateEmission(emission(variant), artifact(variant), variant, LLVM);
    const m = validateMachineObservation(machine(variant), e, variant, W);
    cases += m.cases.length; sites += m.cases.reduce((sum, item) => sum + item.program.length, 0);
  }
  assert.equal(cases, 12); assert.equal(sites, 80);
});
test('pure: descriptor literals independently retain all authored roles and dead/self steps', () => {
  const opcodes = ['V_MOV_B32_e32_vi', 'V_ADD_U32_e32_gfx9', 'V_SUB_U32_e32_gfx9', 'V_AND_B32_e32_vi', 'V_OR_B32_e32_vi', 'V_XOR_B32_e32_vi'];
  for (const [profile, words] of Object.entries(DESCRIPTORS)) words.forEach((word, i) => {
    const opcode = word & 7, dst = word & 8 ? 33 : 32, roles = [34, 35, 36, 32, 33];
    assert.equal(EXPECTED_PROGRAMS[profile][i].opcode, opcodes[opcode]);
    assert.deepEqual(EXPECTED_PROGRAMS[profile][i].register_operands,
      [dst, roles[(word >> 4) & 7], ...(opcode ? [roles[(word >> 7) & 7]] : [])].map(v => `VGPR${v}`));
  });
  assert.deepEqual(DESCRIPTORS.sixteen.slice(-2), [16, 72]);
});
test('pure: all thirty-six retained logical CPU results join exact inputs and full guarded backing', () => {
  let count = 0;
  for (const variant of SOURCE_VARIANTS) for (let i = 0; i < 6; i++) {
    const { record, request, result } = simulated(variant, i);
    assert.equal(validateSimulationJoin(record, request, result, artifact(variant), variant, i).checked_lanes, 64); count++;
  }
  assert.equal(count, 36);
  assert.equal(programResult('three', [19, 23, 42]), 23);
  assert.equal(programResult('one', [4294967295, 1, 2]), 4294967295);
});
test('pure: source receipt requires actual complete ordered commands and immutable measured inputs', () => {
  const r = sourceReceipt(); assert.equal(r.stages.length, 122);
  assert.equal(validateSourceReceipt(r, { repo: REPO, sourceBatch: SOURCE }).length, 6);
  for (const mutate of [v => { v.status = 'failed'; }, v => { v.source_export_executed = false; }, v => { v.exported_source_authentication = true; },
    v => { v.hardware_execution = true; }, v => { v.extra = true; }, v => { v.exports.pop(); }, v => { v.simulations.pop(); },
    v => { v.stages[0].signal = 'SIGKILL'; }, v => { v.stages[0].reason = 'disk_reserve'; }, v => { v.stages[0].code = 1; },
    v => { v.stages[3].stage = 'copied-stage'; }, v => { v.stages[3].log_stem = '../escape'; },
    v => { v.stages[0].free_bytes_before = '0'; }, v => { v.counts.simulations = 35; },
    v => { v.inputs_after[0].sha256 = C; }, v => { v.inputs_before[1] = clone(v.inputs_before[0]); },
    v => { v.binaries_before[0].sha256 = C; }, v => { v.exports[0].kirPath = '/synthetic/substitution'; },
    v => { v.exports[0].source_authentication_carried_by_raw_bytes = true; }, v => { v.exports[6].kir_absent = false; },
    v => { v.exports[7].diagnostic = 'some unrelated refusal'; }, v => { v.limits.jobs = 16; },
    v => { v.resource_guards[0].combined_cache_bytes = '200'; }, v => { v.resource_guards[1].available_ram_bytes = '0'; }]) {
    const value = sourceReceipt(); mutate(value); assert.throws(() => validateSourceReceipt(value, { repo: REPO, sourceBatch: SOURCE }));
  }
});
test('pure: emitter canonical domain, file hash, LLVM bytes and declaration identities are not interchangeable', () => {
  for (const mutate of [v => { v.canonical_identity = H; }, v => { v.input_file_sha256 = C; }, v => { v.canonical_bytes++; },
    v => { v.llvm_sha256 = H; }, v => { v.llvm_bytes++; }, v => { v.canonical_wire_version = 16; },
    v => { v.program_count = 3; }, v => { v.descriptors[1] = 1; }, v => { v.register_plan.reverse(); },
    v => { v.source_authentication = true; }, v => { v.protected_admission = true; }, v => { v.hardware_execution = true; },
    v => { v.canonical_and_emitter_accounting_are_separate = false; }, v => { v.canonical_work_limit++; }, v => { v.extra = true; }]) {
    const value = emission(); mutate(value); assert.throws(() => validateEmission(value, artifact(), SOURCE_VARIANTS[0], LLVM));
  }
  assert.throws(() => validateEmission(emission(), artifact(), SOURCE_VARIANTS[0], Buffer.from('stale LLVM')));
});
test('pure: every negative export binds its exact measured command and feature, even when diagnostics coincide', () => {
  const baseline = sourceReceipt();
  for (const refusal of baseline.exports.slice(6)) for (const mutate of [
    stage => { stage.executable = '/synthetic/unmeasured-exporter'; },
    stage => { stage.args[stage.args.indexOf('--features') + 1] = 'ordered-program-one-v32'; },
    stage => { stage.args[stage.args.indexOf('--output') + 1] = SOURCE + '/another/kernel.kir'; },
  ]) {
    const receipt = sourceReceipt(); mutate(receipt.stages.find(stage => stage.stage === `${refusal.name}-export`));
    assert.throws(() => validateSourceReceipt(receipt, { repo: REPO, sourceBatch: SOURCE }));
  }
  for (const feature of ['invalid-count', 'invalid-opcode', 'read-before-init', 'padding']) {
    const receipt = sourceReceipt(), stage = receipt.stages.find(item => item.stage === `ordered-program-${feature}-v32-export`);
    stage.args[stage.args.indexOf('--features') + 1] = `ordered-program-${feature === 'padding' ? 'invalid-count' : 'padding'}-v32`;
    assert.throws(() => validateSourceReceipt(receipt, { repo: REPO, sourceBatch: SOURCE }));
  }
});
test('pure: all fifteen controls bind known refusal triples and exact measured argument sequences', () => {
  for (let index = 0; index < 15; index++) for (const mutate of [
    (control, stage) => { stage.executable = '/synthetic/unmeasured-simulator'; },
    (control, stage) => { stage.args = []; }, (control, stage) => { stage.args[0] = '--bundle-v6'; },
    (control, stage) => { stage.stdout_bytes = 1; }, (control, stage) => { stage.stdout_sha256 = H; },
    control => { control.stage = 'unrelated'; }, control => { control.kind = 'unrelated'; },
    control => { control.input = control.input === null ? 'kir_v17' : null; },
  ]) {
    const receipt = sourceReceipt(), control = receipt.controls[index], stage = receipt.stages.find(item => item.stage === control.name);
    mutate(control, stage); assert.throws(() => validateSourceReceipt(receipt, { repo: REPO, sourceBatch: SOURCE }));
  }
  const receipt = sourceReceipt(), stage = receipt.stages.find(item => item.stage === 'schedule-before-io-0');
  stage.args = clone(receipt.stages.find(item => item.stage === 'schedule-before-io-1').args);
  assert.throws(() => validateSourceReceipt(receipt, { repo: REPO, sourceBatch: SOURCE }));
});
test('pure: compiler and selected libraries join bounded measured inputs and retained rustc version without closure authority', () => {
  for (const mutate of [v => { v.rustc_binary = {}; }, v => { v.rustc_binary.sha256 = C; },
    v => { v.selected_library_measurements = {}; }, v => { v.selected_library_measurements.extra = true; },
    v => { v.selected_library_measurements.scope = 'runtime closure'; }, v => { v.selected_library_measurements.backend_soname = 'other.so'; },
    v => { v.selected_library_measurements.backend.sha256 = C; }, v => { v.selected_library_measurements.rustc_driver.sha256 = C; },
    v => { v.selected_library_measurements.rustc_driver.requested = '/synthetic/toolchain/lib/not-a-driver.so'; },
    v => { v.selected_library_measurements.loader_search_directories.reverse(); },
    v => { v.selected_library_measurements.runtime_closure_attestation = true; },
    v => { v.rustc_version = {}; }, v => { v.rustc_version.text = 'x'.repeat(4097); }, v => { v.rustc_version.commit = '0'.repeat(40); },
    v => { v.rustc_version.text += 'commit-hash: 55e86c996809902e8bbad512cfb4d2c18be446d9\n'; },
    v => { v.rustc_version.text = v.rustc_version.text.replace('release: 1.96.0-nightly', 'release: 1.95.0'); },
    v => { v.rustc_version.text += 'unretained text\n'; }, v => { v.stages[0].executable = '/synthetic/other-rustc'; },
    v => { v.stages[0].args = ['--version']; }, v => { v.stages[0].stdout_sha256 = H; }, v => { v.stages[0].stdout_bytes++; },
    v => { v.stages[0].stderr_bytes = 1; }, v => { v.stages[0].stderr_sha256 = H; },
    v => { v.inputs_before = v.inputs_before.filter(item => item.requested !== v.rustc_binary.requested); v.inputs_after = clone(v.inputs_before); },
    v => { v.inputs_before = v.inputs_before.filter(item => item.requested !== v.selected_library_measurements.rustc_driver.requested); v.inputs_after = clone(v.inputs_before); },
  ]) {
    const receipt = sourceReceipt(); mutate(receipt); assert.throws(() => validateSourceReceipt(receipt, { repo: REPO, sourceBatch: SOURCE }));
  }
});
test('pure: native input ancestry, source use, profile, build claims and O0/O3 cannot be promoted or substituted', () => {
  for (const mutate of [v => { v.authority = 'production'; }, v => { v.source_ancestry = 'authenticated'; },
    v => { v.synthetic_worker_request_identity_fields = false; }, v => { v.hardware_executed = true; },
    v => { v.runtime_closure_attestation = 'verified'; }, v => { v.physical_register_allocation_or_lifetime_proof = true; },
    v => { v.profile = 'three'; }, v => { v.result_used = false; }, v => { v.llvm_text_sha256 = H; },
    v => { v.worker_build_claim = `fe2o3-worker-v1-sha256-${H}`; }, v => { v.llvm_build_claim = 'unknown'; },
    v => { v.cases.reverse(); }, v => { v.cases[1] = clone(v.cases[0]); }, v => { v.cases[1].llvm_text_sha256 = H; },
    v => { v.expected_program[0].register_operands[0] = 'VGPR32'; }, v => { v.kernel_symbol = '../bad'; }]) {
    const value = machine(); mutate(value); assert.throws(() => validateMachineObservation(value, emission(), SOURCE_VARIANTS[0], W));
  }
});
test('pure: each of eighty observed authored sites rejects opcode bytes roles EXEC or order mutation', () => {
  for (const variant of SOURCE_VARIANTS) for (const optimization of [0, 1]) for (let i = 0; i < DESCRIPTORS[variant.profile].length; i++) {
    for (const mutate of [v => { v.bytes_hex = '00000000'; }, v => { v.opcode += '_e64'; }, v => { v.register_operands[0] = 'VGPR36'; },
      v => { v.implicit_reads = []; }, v => { v.implicit_writes = ['EXEC']; }, v => { v.mc_flags = 1; }, v => { v.file_offset = 2047; }]) {
      const m = machine(variant); mutate(m.cases[optimization].program[i]);
      assert.throws(() => validateMachineObservation(m, emission(variant), variant, W));
    }
  }
});
test('pure: native descriptor joins reject stale identity extent granules footprint and lifetime promotion', () => {
  for (const mutate of [v => { v.descriptor_sha256 = H; }, v => { v.descriptor_file_offset = 2040; }, v => { v.descriptor_file_offset = 512; },
    v => { v.descriptor_bytes = 63; }, v => { v.compute_pgm_rsrc1 = 3; }, v => { v.compute_pgm_rsrc3 = 8; },
    v => { v.vgpr_capacity = 20; }, v => { v.architected_vgpr_boundary = 44; }, v => { v.required_footprint_high_water = 36; },
    v => { v.interpretation = 'lifetime-proof'; }]) {
    const m = machine(); mutate(m.cases[0].descriptor_resources);
    assert.throws(() => validateMachineObservation(m, emission(), SOURCE_VARIANTS[0], W));
  }
});
test('pure: native boundary observations remain bounded independent observations, not register values', () => {
  const m = machine(), entry = m.cases[0];
  entry.boundary_register_site_count = 1;
  entry.boundary_sites.push({ ...clone(entry.program[0]), file_offset: 520 });
  validateMachineObservation(m, emission(), SOURCE_VARIANTS[0], W);
  for (const mutate of [v => { v.boundary_sites_truncated = true; }, v => { v.boundary_observation_is_value_or_lifetime_proof = true; },
    v => { v.boundary_sites[0].file_offset = 544; }, v => { v.boundary_register_site_count = 2; },
    v => { v.boundary_sites[0].bytes_hex = 'aa'.repeat(17); },
    v => { v.static_instruction_count = v.program_count; },
    v => { v.boundary_sites[0].register_operands = ['VGPR31', 'VGPR37']; },
    v => { v.boundary_sites[0].register_operands = []; }]) {
    const changed = clone(m); mutate(changed.cases[0]); assert.throws(() => validateMachineObservation(changed, emission(), SOURCE_VARIANTS[0], W));
  }
});
test('pure: CPU join rejects copied expected values, input permutation, wrong domain and guard corruption', () => {
  const variant = SOURCE_VARIANTS[2];
  for (const mutate of [v => { v.record.expected_output++; }, v => { v.record.inputs = [23, 19, 42]; },
    v => { v.record.operand_order = [1, 0, 2]; }, v => { v.request.workgroup = [32, 1, 1]; },
    v => { v.result.kir.sha256 = H; }, v => { v.result.counts.invocations_executed = 63; },
    v => { v.result.hardware_observed = true; }, v => { v.result.shared_buffers[0].buffer.initialized = '0x' + 'ff'.repeat(33); },
    v => { v.result.shared_buffers[0].buffer.bytes = v.result.shared_buffers[0].buffer.bytes.replace('a5', '00'); },
    v => { v.result.arguments[1].bits = '0x00000000'; }]) {
    const value = simulated(variant, 5); mutate(value);
    assert.throws(() => validateSimulationJoin(value.record, value.request, value.result, artifact(variant), variant, 5));
  }
});
test('pure: source export ancestry requires exact actual-export line and unpromoted retained identities', () => {
  const a = artifact();
  const line = `fe2o3 diagnostic extraction: actual Rust -> admitted semantic MIR V32 -> semantic SSA -> pre_ranked_diagnostic exact KIR V17; declared_target=gfx942:xnack-, declared_wave=64, 1 kernel(s), canonical_identity ${C}, 2048 byte(s), retained_source_inventory ${H}, retained_source_preflight ${H}; ranked_checks=false, functional_proof=false, exported_source_authentication=false, exported_compiler_authentication=false, protected_admission=false, artifact/load/launch/hardware_authority=false, source_variable_map=unavailable, physical_register_values=unavailable, instruction_microsteps=unavailable; raw bytes are diagnostic input, not production resume`;
  validateSourceExportLine(Buffer.from(line + '\n'), a);
  for (const changed of [line + '\n' + line, line.replace('actual Rust', 'synthetic Rust'), line.replace('KIR V17', 'KIR V16'),
    line.replace('source_authentication=false', 'source_authentication=true'), line.replace(C, H)]) assert.throws(() => validateSourceExportLine(Buffer.from(changed), a));
});
test('pure: strict JSON rejects duplicate escaped keys, loss of integer precision, malformed UTF8 and truncation', () => {
  assert.deepEqual(parseBoundedJson(Buffer.from('{"value":4294967295}')), { value: 4294967295 });
  for (const bytes of [Buffer.alloc(0), Buffer.alloc(1024 ** 2 + 1), Buffer.from([255]), Buffer.from('{"a":1,"a":2}'),
    Buffer.from('{"a":1,"\\u0061":2}'), Buffer.from('{"a":9007199254740993}'), Buffer.from('{"a":1.5}'),
    Buffer.from('{"a":-1}'), Buffer.from('{"a":1e1}'), Buffer.from('{"a":1}{}'), Buffer.from('{"a":'),
    Buffer.from('['.repeat(26) + '0' + ']'.repeat(26)), Buffer.from('[' + '0,'.repeat(8192) + '0]')]) assert.throws(() => parseBoundedJson(bytes));
});
test('pure: CLI requires explicit fresh-layout inputs and rejects unknown duplicate relative or control paths', () => {
  const args = ['--source-batch', SOURCE, '--native-batch', '/synthetic/native', '--emitter', '/synthetic/emitter',
    '--output', '/synthetic/new-output', '--cargo-cache-root', '/synthetic/cargo', '--secondary-cache-root', '/synthetic/secondary'];
  assert.equal(parseArguments(args).sourceBatch, SOURCE);
  for (const changed of [args.slice(2), [...args, '--source-batch', SOURCE], [...args, '--unknown', '/synthetic/x'],
    args.map((value, i) => i === 1 ? 'relative' : value), args.map((value, i) => i === 1 ? '/synthetic/../escape' : value),
    args.map((value, i) => i === 1 ? '/synthetic/\nline' : value)]) assert.throws(() => parseArguments(changed));
});
