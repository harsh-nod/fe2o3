// Synthetic pure join controls only. No filesystem, compiler, child or GPU use.
import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { INPUTS, LENGTHS, descriptors, oracle, replaceOnce, request, sha256, verifySimulation,
} from './source-promotion-instruction-edit-smoke.mjs';
import { LIMITS, options, parseJson, sourcePinLedger, validateNativeReport, validateSourceReceipt,
} from './instruction-edit-native-join.mjs';
const copy = value => structuredClone(value);
const encoded = value => Buffer.from(JSON.stringify(value) + '\n');
const H = label => sha256(Buffer.from('synthetic:' + label));
const LLVM_CLAIM = 'rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540';
const WORKER_CLAIM = 'fe2o3-worker-v1-sha256-' + H('worker');
const CONTEXT = { repo: '/repo', prepared: '/prepared', sourceRoot: '/source-output',
  consumer: '/tools/promote_once', binDir: '/tools', emitter: '/tools/lower' };
function identities(profile, kir) {
  return { canonical_sha256: H(profile + ':canonical'), canonical_bytes: kir.length,
    retained_source_inventory: H('fixed-root-contract-inventory'), retained_source_preflight: H(profile + ':preflight'),
    semantic_identity: H(profile + ':semantic') };
}
function exportLog(value) {
  return Buffer.from('fe2o3 diagnostic extraction: actual Rust -> admitted semantic MIR V32 -> semantic SSA -> pre_ranked_diagnostic exact KIR V17; ' +
    'declared_target=gfx942:xnack-, declared_wave=64, 1 kernel(s), canonical_identity ' + value.canonical_sha256 + ', ' +
    value.canonical_bytes + ' byte(s), retained_source_inventory ' + value.retained_source_inventory +
    ', retained_source_preflight ' + value.retained_source_preflight +
    '; ranked_checks=false, functional_proof=false, exported_source_authentication=false, exported_compiler_authentication=false, ' +
    'protected_admission=false, artifact/load/launch/hardware_authority=false, source_variable_map=unavailable, ' +
    'physical_register_values=unavailable, instruction_microsteps=unavailable; raw bytes are diagnostic input, not production resume\n' +
    'fe2o3 diagnostic source identities: semantic_mir_v32 ' + value.semantic_identity + ', canonical_kir_v17 ' +
    value.canonical_sha256 + '; observation_only=true, exported_source_authentication=false\n');
}
function inspection(profile, exported) {
  return { kind: 'diagnostic_ordered_program_inspection_example', authority: 'observation_only',
    canonical: { wire_version: 17, sha256: exported.canonical_sha256, bytes: exported.canonical_bytes },
    kernel: 'choose_bits', function: 'choose_bits_impl', coordinate: { function_ordinal: 0, block_ordinal: 0, operation_ordinal: 0 },
    raw_block_id: 0, input_value_ids: [1, 2, 3], result_value_id: 4, declared_target: 'gfx942:xnack-', declared_wave_width: 64,
    profile: 'closed_u32_program_e32_v1', register_plan: { scratch: 4, output: 5, inputs: [0, 1, 2], vgpr_high_water: 6 },
    declared_program: { count: 3, descriptors: descriptors(profile) }, declared_instruction_steps: [
      { instruction: 'v_xor_b32_e32', output: 4, inputs: [0, 1] },
      { instruction: 'v_and_b32_e32', output: 4, inputs: [4, 2] },
      { instruction: profile === 'default' ? 'v_xor_b32_e32' : 'v_or_b32_e32', output: 5, inputs: [1, 4] }],
    declared_source_ids: { frontend_unit: H('unit'), function: H('function'), contract: H('contract'), statement: H(profile + ':statement') },
    memory_effect: 'NoMemory', ordered_region_effect: true, pure_or_movable: false,
    logical_observation_granularity: 'whole_program_before_after', source_authentication: false, source_map_available: false,
    physical_register_values_available: false, instruction_microsteps_available: false,
    register_lifetime_or_final_allocation_proof: false, proof_authority: false, artifact_authority: false,
    production_resume_authority: false, hardware_execution: false, cpu_preflight_passed: true,
    inspection_counts: { blocks: 5, operations: 8, ssa_definitions: 12, capability_entries: 9, name_bytes: 100 },
    inspection_max_canonical_bytes_after_admission: 65536, cpu_preflight_resident_limit_bytes: 64 * 1024 * 1024,
    output_buffer_bytes: 8192,
    accounting_scope: 'shared canonical admission, CPU resident accounting, and this borrowed structural/output bound are separate; not a combined allocator or RSS cap' };
}
function emission(profile, exported, kir, llvm) {
  return { kind: 'diagnostic_ordered_program_llvm_observation', authority: 'observation_only',
    canonical_wire_version: 17, canonical_identity: exported.canonical_sha256, canonical_bytes: kir.length,
    input_file_sha256: sha256(kir), llvm_sha256: sha256(llvm), llvm_bytes: llvm.length, program_count: 3,
    descriptors: descriptors(profile), register_plan: [4, 5, 0, 1, 2], canonical_retained_storage_bytes: 1024,
    canonical_work_limit: 1 << 26, canonical_storage_limit: 64 * 1024 * 1024, max_input_bytes: 65536,
    max_published_llvm_bytes: 65536, emitter_text_limit_bytes: 16 * 1024 * 1024,
    canonical_and_emitter_accounting_are_separate: true, source_authentication: false, compiler_closure_attestation: false,
    proof_authority: false, protected_admission: false, final_artifact_authority: false, production_resume: false,
    physical_register_values: false, hardware_execution: false };
}
function simulation(query, exported, word) {
  const elements = query.arguments[0].elements, bytes = Buffer.from(query.shared_buffers[0].bytes.slice(2), 'hex');
  const initialized = Buffer.alloc(Math.ceil(bytes.length / 8));
  for (let index = 0; index < elements; index++) bytes.writeUInt32LE(word, 4 + index * 4);
  for (let offset = 4; offset < 4 + elements * 4; offset++) initialized[offset >> 3] |= 1 << (offset & 7);
  return { schema: 'fe2o3-simulation-result-v1', status: 'ok', authority: 'observation_only', simulated: true,
    hardware_observed: false, hardware_validation: false, performance_prediction: false,
    kir: { sha256: exported.canonical_sha256, canonical_bytes: exported.canonical_bytes }, arguments: copy(query.arguments),
    counts: { arguments: 4, shared_buffers: 1, invocations_executed: query.grid[0], workgroups_visited: query.grid[0] / 64 },
    target_profile: { identity: 'amdgpu_64_little_endian_v1', index_bits: 64 },
    shared_buffers: [{ id: 1, buffer: { element: 'u32', access: 'read_write', alignment: 4,
      bytes: '0x' + bytes.toString('hex'), initialized: '0x' + initialized.toString('hex') } }] };
}
function sourceFixture() {
  const files = new Map(), pins = new Map(), stages = [], variants = [];
  const put = (file, bytes) => {
    const value = Buffer.from(bytes), prior = pins.get(file); files.set(file, value);
    pins.set(file, { path: file, bytes: value.length, sha256: sha256(value), device: '1',
      inode: prior?.inode ?? String(pins.size + 2), mode: '33152', mtime_ns: '1', ctime_ns: '1' }); return value;
  };
  const save = (name, bytes) => put(path.join(CONTEXT.sourceRoot, name), bytes);
  const stage = (label, executable, args, stdout = Buffer.alloc(0), stderr = Buffer.alloc(0)) => {
    save(label + '.stdout', stdout); save(label + '.stderr', stderr);
    stages.push({ label, executable, args, code: 0, signal: null, reason: null, elapsed_ms: 1,
      stdout_bytes: stdout.length, stdout_sha256: sha256(stdout), stderr_bytes: stderr.length, stderr_sha256: sha256(stderr) });
  };
  const original = Buffer.from('// synthetic source bytes\n    let selected = b ^ ((a ^ b) & mask);\n// unchanged tail\n');
  const macro = 'fe2o3_device::amdgpu_ordered_program! {\n    gfx942_xnack_off_wave64;\n    scratch(4); out(5);\n' +
    '    in(0) = a;\n    in(1) = b;\n    in(2) = mask;\n    xor(scratch, input0, input1);\n' +
    '    and(scratch, scratch, input2);\n    xor(out, input1, scratch);\n}';
  const candidate = replaceOnce(original, 'b ^ ((a ^ b) & mask)', macro);
  const edited = replaceOnce(candidate, '    xor(out, input1, scratch);\n', '    or(out, input1, scratch);\n');
  const directory = '/repo/target/source-bitselect-candidate-roundtrip/prepared/positive';
  const loader = Buffer.from('#![no_std]\n#[path = "original.rs"] mod source_bitselect_feasibility;\n');
  put(directory + '/original.rs', original); put(directory + '/original-loader.rs', loader);
  put(directory + '/instruction-default.rs', candidate);
  const fixture = '/repo/crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device';
  const fixtureValues = ['Cargo.toml', 'src/lib.rs', 'src/source_bitselect_feasibility.rs', 'src/source_bitselect_normalized.rs']
    .map(name => put(fixture + '/' + name, name.includes('feasibility') ? original : Buffer.from(name)));
  const metadata = put('/prepared/metadata.stdout', 'synthetic metadata'), artifacts = put('/prepared/dependencies.stdout', 'synthetic artifacts');
  const device = '/prepared/dependencies/target/release/deps/device.rmeta', core = '/prepared/dependencies/target/release/deps/core.rmeta';
  const args = ['/toolchain/bin/rustc', '--crate-name', 'fe2o3_production_extraction_fixture', directory + '/original-loader.rs',
    '--edition=2024', '--crate-type=lib', '--target=amdgcn-amd-amdhsa', '--emit=metadata', '-Copt-level=3', '-Cpanic=abort',
    '-Cembed-bitcode=no', '-Cdebug-assertions=off', '-Coverflow-checks=on', '-Ctarget-cpu=gfx942',
    '-Ctarget-feature=-xnack,+wavefrontsize64,-wavefrontsize32', '-Zalways-encode-mir', '-Zunstable-options',
    '--sysroot', '/toolchain', '--out-dir', '/prepared/analysis-output', '-L', 'dependency=' + path.dirname(device),
    '-L', 'dependency=/prepared/dependencies/release/deps', '--extern', 'fe2o3_device=' + device,
    '--extern', 'noprelude:core=' + core, '--cfg', 'feature="source-bitselect-feasibility"', '-Cmetadata=synthetic'];
  const record = { case: 'positive', phase: 'baseline', source_directory: path.relative('/repo', directory), args,
    crate_binding: H('binding'), cargo_observation: H('cargo'), source_sha256: sha256(original), loader_sha256: sha256(loader),
    fixture_sha256: fixtureValues.map(sha256), artifacts_sha256: sha256(artifacts), metadata_sha256: sha256(metadata) };
  put('/prepared/positive/headless-normal.invocation.json', encoded(record));
  for (const tool of [CONTEXT.consumer, CONTEXT.emitter, device, core, '/toolchain/bin/rustc', '/toolchain/bin/cargo',
    ...['fe2o3-export-sim', 'fe2o3-rustc-extract', 'fe2o3-program-inspect', 'fe2o3-kir-sim'].map(name => '/tools/' + name)]) put(tool, 'synthetic binary');
  const publication = { original_sha256: sha256(original), candidate_sha256: sha256(candidate), candidate_bytes: candidate.length };
  stage('public-seed', CONTEXT.consumer, [path.relative('/repo', directory + '/original.rs'),
    path.relative('/repo', directory + '/instruction-default.rs'), sha256(original), '--', ...args],
  Buffer.from('FE2O3_HEADLESS_NORMAL_PUBLISHED ' + sha256(original) + ' ' + sha256(candidate) + ' ' + candidate.length + '\n'));
  save('original.rs', original); save('public-candidate.rs', candidate); save('instruction-edited.rs', edited);
  const template = '/repo/crates/rustc-codegen-fe2o3/tests/fixtures/assembly-authoring-v30';
  const templateManifest = put(template + '/Cargo.toml',
    'device = "../../../../fe2o3-device"\nhost = "../../../../fe2o3-host"\n[features]\nedited = []\n');
  const lock = put(template + '/Cargo.lock', 'synthetic unchanged lock');
  let manifest = Buffer.from(templateManifest);
  for (const dependency of ['fe2o3-device', 'fe2o3-host'])
    manifest = replaceOnce(manifest, '"../../../../' + dependency + '"', '"/repo/crates/' + dependency + '"');
  manifest = replaceOnce(manifest, 'edited = []',
    'source-bitselect-feasibility = []\nsource-bitselect-ambiguous = []\nsource-bitselect-local-alias = []');
  for (const [label, source] of [['default', candidate], ['edited', edited]]) {
    save(label + '-source/src/kernel.rs', source);
    save(label + '-source/src/lib.rs', '#![no_std]\n#[path = "kernel.rs"] mod source_bitselect_feasibility;\n');
    save(label + '-source/Cargo.toml', manifest); save(label + '-source/Cargo.lock', lock);
  }
  for (const label of ['default', 'edited', 'repeat']) {
    const profile = label === 'default' ? 'default' : 'edited', source = profile === 'default' ? candidate : edited;
    const kir = save(label + '.kir', profile + ' synthetic KIR'), llvm = save(label + '.ll', profile + ' synthetic LLVM');
    const exported = identities(profile, kir), inspected = inspection(profile, exported), emitted = emission(profile, exported, kir, llvm);
    const kirPath = CONTEXT.sourceRoot + '/' + label + '.kir', llvmPath = CONTEXT.sourceRoot + '/' + label + '.ll';
    stage(label + '-export', '/tools/fe2o3-export-sim', ['--diagnostic-kir-v17', '--crate', 'fe2o3_assembly_authoring_v30_fixture',
      '--output', kirPath, '--target', 'gfx942', '--target-dir', CONTEXT.sourceRoot + '/' + label + '-export-target',
      '--', '--manifest-path', CONTEXT.sourceRoot + '/' + profile + '-source/Cargo.toml',
      '--lib', '--offline', '--features', 'source-bitselect-feasibility'], Buffer.alloc(0), exportLog(exported));
    save(label + '-inspect-request.json', encoded(request(INPUTS[0], 1)));
    stage(label + '-inspect', '/tools/fe2o3-program-inspect', [kirPath, CONTEXT.sourceRoot + '/' + label + '-inspect-request.json'], encoded(inspected));
    stage(label + '-llvm', CONTEXT.emitter, [kirPath, llvmPath], encoded(emitted));
    const simulations = [];
    for (let input = 0; input < INPUTS.length; input++) for (const length of LENGTHS) {
      const query = request(INPUTS[input], length), result = simulation(query, exported, oracle(profile, INPUTS[input]));
      const name = label + '-case-' + input + '-length-' + length, requestPath = CONTEXT.sourceRoot + '/' + name + '.request.json';
      put(requestPath, encoded(query));
      for (let replay = 0; replay < 2; replay++) {
        stage(name + '-replay-' + replay, '/tools/fe2o3-kir-sim', ['--diagnostic-kir-v17', kirPath, '--request', requestPath], encoded(result));
        simulations.push({ input, length, replay, ...verifySimulation(result, query, exported, profile, INPUTS[input], length) });
      }
    }
    variants.push({ label, source_sha256: sha256(source), exported, inspection: inspected, emission: emitted,
      kir_file_sha256: sha256(kir), llvm_sha256: sha256(llvm), simulations });
  }
  const receipt = { status: 'passed', kind: 'source_promotion_instruction_edit_diagnostic_v1', normal_public_seed: publication,
    actual_normal_exports: 3, whole_kernel_simulations: 90, variants, stages, retained_file_pins: [], retained_pin_bytes: 0,
    semantic_identity_join_qualified: true, semantic_identity_availability: 'normal_live_exporter_observed',
    source_edit: 'one_exact_final_xor_to_or_original_and_surrounding_bytes_unchanged',
    ranked_checks: false, protected_proof: false, compiler_closure_attestation: false, source_authentication: false,
    native_qualified: false, physical_register_lifetime_proof: false, production_resume: false, hardware_observed: false,
    limits: { source_bytes: 65536, command_stream_bytes: 1048576, command_ms: 300000, stages: 110,
      retained_pins: 384, retained_pin_bytes: 2 * 1024 ** 3, cargo_jobs: 2,
      task_root_storage_accounting: 'external_root_supervisor_required_not_replaced_by_this_runner' } };
  const sync = () => { receipt.retained_file_pins = [...pins.values()]; receipt.retained_pin_bytes = [...pins.values()].reduce((n, pin) => n + pin.bytes, 0); };
  sync();
  const read = (file, cap) => { const bytes = files.get(file); assert.ok(bytes && bytes.length <= cap); return Buffer.from(bytes); };
  return { receipt, files, put, sync, read, context: copy(CONTEXT) };
}

function nativeFixture(profile = 'default') {
  const llvm = Buffer.from(profile + ' synthetic LLVM'), files = new Map(), payloadDirectory = '/native/' + profile;
  const expected = { profile, llvm_sha256: sha256(llvm), llvm_bytes: llvm.length, payloadDirectory,
    llvmClaim: LLVM_CLAIM, workerClaim: WORKER_CLAIM };
  const cases = ['O0', 'O3'].map((optimization, index) => {
    // Deliberately NOT an ELF. This is synthetic raw-byte matcher evidence only.
    const payload = Buffer.alloc(192, index + 1), descriptor = Buffer.alloc(64);
    descriptor.writeUInt32LE(0, 48); descriptor.writeUInt32LE(1, 44); descriptor.copy(payload, 64);
    const words = ['0003082a', '04050826', profile === 'default' ? '01090a2a' : '01090a28'];
    const names = ['V_XOR_B32_e32_vi', 'V_AND_B32_e32_vi', profile === 'default' ? 'V_XOR_B32_e32_vi' : 'V_OR_B32_e32_vi'];
    const registers = [['VGPR4', 'VGPR0', 'VGPR1'], ['VGPR4', 'VGPR4', 'VGPR2'], ['VGPR5', 'VGPR1', 'VGPR4']];
    const program = words.map((word, at) => {
      Buffer.from(word, 'hex').copy(payload, 128 + 4 * at);
      return { file_offset: 128 + 4 * at, opcode: names[at], bytes_hex: word, mc_flags: 16,
        register_operands: registers[at], implicit_reads: ['EXEC'], implicit_writes: [] };
    });
    const file = payloadDirectory + '/' + optimization + '.hsaco'; files.set(file, payload);
    return { machine_observation: { optimization, llvm_sha256: expected.llvm_sha256, llvm_bytes: llvm.length,
      hsaco_sha256: sha256(payload), hsaco_bytes: payload.length, entry_file_offset: 128, entry_code_bytes: 20,
      static_instruction_count: 5, program, descriptor: { file_offset: 64, bytes: 64, sha256: sha256(descriptor),
        compute_pgm_rsrc1: 0, compute_pgm_rsrc3: 1, vgpr_capacity: 8, architected_vgpr_boundary: 8,
        required_footprint_high_water: 6, interpretation: 'encoded-capacity-not-metadata-usage-or-lifetime' },
      post_link_checks: [
        'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
        'post_link.check=exports status=ok symbols=[choose_bits,choose_bits.kd]',
        'post_link.check=unresolved status=ok symbols=[]',
        'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-',
        'post_link.kernel name=choose_bits symbol=choose_bits.kd kernarg_size=28 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]'],
      derivation_identity: H(profile + optimization + ':derivation'), boundary_value_or_lifetime_proof: false },
    mutation_controls: { decoded_field_refusals: 7, stale_identity_refusals: 1, gapped_sequence_refusals: 1,
      raw_byte_mismatch_refusals: 1, redecoded_opposite_opcode_refusals: 1, opposite_profile_mutated_payload_observed: true,
      original_payload_unchanged: true, captured_llvm_mutated: false, mutated_payload_executed_on_hardware: false },
    retained_payload: { path: file, bytes: payload.length, sha256: sha256(payload), matches_linked_worker_payload: true,
      create_new_only: true, production_artifact_authority: false } };
  });
  const report = { report_kind: 'private-instruction-edit-native-observation-v1', authority: 'unauthenticated-test-transport',
    profile, kernel_symbol: 'choose_bits', register_plan: [4, 5, 0, 1, 2], descriptors: descriptors(profile).slice(0, 3),
    result_use: 'sole-direct-nonvolatile-nonatomic-global-store', prefix_source_admitted: false,
    llvm_sha256: sha256(llvm), llvm_bytes: llvm.length, expected_input_identity_matched: true,
    retained_file_identity_and_bytes_rechecked: true, llvm_build_claim: LLVM_CLAIM, worker_build_claim: WORKER_CLAIM,
    target: 'gfx942:xnack-', wave_width: 64, workgroup_size: 64, code_object_version: 6,
    shape_controls: { typed_shape_positives: 4, typed_shape_negatives: 48, input_identity_positives: 1,
      input_identity_negatives: 3, file_snapshot_positives: 1, file_snapshot_negatives: 9,
      worker_or_target_machine_invoked: false, captured_llvm_mutated: false }, cases,
    source_ancestry: 'not-established-by-llvm-file', synthetic_worker_request_identity_fields: true,
    production_exact_program_admission: false, protected_finalizer_admission: false, artifact_authority: false,
    source_authentication: false, compiler_closure_attestation: false, runtime_closure_attestation: 'unavailable',
    hardware_execution: false, native_whole_kernel_correctness: false,
    physical_register_allocation_or_lifetime_proof: false, whole_kernel_order_or_byte_stability_claim: false };
  const read = (file, cap) => { const bytes = files.get(file); assert.ok(bytes && bytes.length <= cap); return Buffer.from(bytes); };
  return { report, expected, llvm, files, read };
}
const validateSource = fixture => validateSourceReceipt(fixture.receipt, fixture.context, fixture.read);
const validateNative = fixture => validateNativeReport(fixture.report, fixture.expected, fixture.llvm, fixture.read);
function replaceStageOutput(fixture, label, name, bytes) {
  fixture.put(CONTEXT.sourceRoot + '/' + label + '.' + name, bytes);
  const stage = fixture.receipt.stages.find(value => value.label === label);
  stage[name + '_bytes'] = bytes.length; stage[name + '_sha256'] = sha256(bytes); fixture.sync();
}
function refreshPayloadIdentity(fixture, index) {
  const wrapper = fixture.report.cases[index], payload = fixture.files.get(wrapper.retained_payload.path);
  wrapper.retained_payload.sha256 = sha256(payload); wrapper.retained_payload.bytes = payload.length;
  wrapper.machine_observation.hsaco_sha256 = sha256(payload); wrapper.machine_observation.hsaco_bytes = payload.length;
}

test('complete synthetic producer join: three variants, 100 stages, 90 raw oracle results; immutable inputs', () => {
  const fixture = sourceFixture(), before = encoded(fixture.receipt);
  const pinsBefore = [...fixture.files].map(([file, bytes]) => [file, sha256(bytes)]);
  const joined = validateSource(fixture);
  assert.equal(joined.stages, 100); assert.equal(joined.simulations, 90);
  assert.deepEqual(joined.variants.map(value => value.label), ['default', 'edited', 'repeat']);
  assert.deepEqual(joined.variants.map(value => value.retained_source_inventory),
    Array(3).fill(H('fixed-root-contract-inventory')));
  assert.notEqual(joined.variants[0].retained_source_preflight, joined.variants[1].retained_source_preflight);
  assert.equal(joined.variants[1].retained_source_preflight, joined.variants[2].retained_source_preflight);
  assert.deepEqual(encoded(fixture.receipt), before);
  assert.deepEqual([...fixture.files].map(([file, bytes]) => [file, sha256(bytes)]), pinsBefore);
});
test('source exact flags, semantic availability, counts and complete stage order refuse drift', () => {
  const mutations = [
    value => { value.whole_kernel_simulations = 89; }, value => { value.actual_normal_exports = 2; },
    value => { value.semantic_identity_join_qualified = false; }, value => { value.semantic_identity_availability = 'preflight'; },
    value => { value.hardware_observed = true; }, value => { value.source_authentication = true; },
    value => { value.extra_authority = true; }, value => { value.stages.pop(); },
    value => { [value.stages[4], value.stages[5]] = [value.stages[5], value.stages[4]]; },
    value => { value.stages[1].code = 1; }, value => { value.stages[1].signal = 'SIGTERM'; },
    value => { value.stages[1].reason = 'timeout'; }, value => { value.stages[1].elapsed_ms = 9007199254740992; },
    value => { value.variants[2].simulations.pop(); }, value => { value.variants[1].simulations[0].expected_word ^= 1; },
  ];
  for (const mutate of mutations) { const fixture = sourceFixture(); mutate(fixture.receipt); assert.throws(() => validateSource(fixture)); }
});
test('stale repeat/source/semantic/KIR/emission/statement joins and misplaced fresh targets refuse', () => {
  for (const mutate of [
    fixture => { fixture.receipt.variants[2].source_sha256 = H('stale'); },
    fixture => { fixture.receipt.variants[2].llvm_sha256 = H('stale'); },
    fixture => { fixture.receipt.variants[2].exported.semantic_identity = H('stale'); },
    fixture => { fixture.receipt.variants[1].inspection.declared_source_ids.statement = H('stale'); },
    fixture => { fixture.receipt.variants[1].emission.input_file_sha256 = fixture.receipt.variants[1].exported.canonical_sha256; },
    fixture => { fixture.receipt.stages.find(value => value.label === 'repeat-export').args[8] = '/old-target'; },
    fixture => { fixture.context.sourceRoot = '/old-source-output'; },
    fixture => { fixture.receipt.normal_public_seed.candidate_bytes++; },
  ]) { const fixture = sourceFixture(); mutate(fixture); assert.throws(() => validateSource(fixture)); }
});
test('coherently rehashed export inventories still refuse any fixed-root census drift', () => {
  for (const labels of [['default'], ['edited'], ['repeat'], ['edited', 'repeat']]) {
    const fixture = sourceFixture();
    for (const label of labels) {
      const variant = fixture.receipt.variants.find(value => value.label === label);
      variant.exported.retained_source_inventory = H('changed-root-contract-inventory');
      // Join the changed observation to its actual synthetic raw log, stage and pin first.
      // Rejection must then come from the cross-variant census, not stale byte custody.
      replaceStageOutput(fixture, label + '-export', 'stderr', exportLog(variant.exported));
    }
    assert.throws(() => validateSource(fixture), /unchanged (?:repeat )?root\/contract inventory/u);
  }
});
test('retained raw source, LLVM, stderr and stdout cannot be replaced by matching summary claims', () => {
  for (const filename of ['original.rs', 'public-candidate.rs', 'instruction-edited.rs', 'edited.ll',
    'repeat.kir', 'default-export.stderr', 'edited-case-0-length-1-replay-0.stdout']) {
    const fixture = sourceFixture(); fixture.files.get(CONTEXT.sourceRoot + '/' + filename)[0] ^= 1;
    assert.throws(() => validateSource(fixture));
  }
});
test('coherently rehashed wrong edited oracle, canary and initialization still refuse', () => {
  for (const kind of ['old-oracle', 'leading-guard', 'trailing-guard', 'initialization']) {
    const fixture = sourceFixture(), label = 'edited-case-0-length-1-replay-0';
    const result = JSON.parse(fixture.files.get(CONTEXT.sourceRoot + '/' + label + '.stdout'));
    const view = result.shared_buffers[0].buffer, bytes = Buffer.from(view.bytes.slice(2), 'hex');
    if (kind === 'old-oracle') bytes.writeUInt32LE(oracle('default', INPUTS[0]), 4);
    if (kind === 'leading-guard') bytes[0] ^= 1;
    if (kind === 'trailing-guard') bytes[bytes.length - 1] ^= 1;
    if (kind === 'initialization') view.initialized = '0x0000';
    view.bytes = '0x' + bytes.toString('hex'); replaceStageOutput(fixture, label, 'stdout', encoded(result));
    assert.throws(() => validateSource(fixture));
  }
});
test('coherently retained duplicate exporter observation and missing semantic diagnostic refuse', () => {
  const fixture = sourceFixture(), name = 'edited-export', prior = fixture.files.get(CONTEXT.sourceRoot + '/' + name + '.stderr');
  replaceStageOutput(fixture, name, 'stderr', Buffer.concat([prior, prior]));
  assert.throws(() => validateSource(fixture));
  const absent = sourceFixture(), bytes = exportLog(absent.receipt.variants[1].exported);
  replaceStageOutput(absent, name, 'stderr', Buffer.from(bytes.toString().split('\n')[0] + '\n'));
  assert.throws(() => validateSource(absent));
});
test('pin census forbids omission, duplicates, unsafe integers, fabricated totals and noncanonical paths', () => {
  for (const mutate of [
    value => { value.retained_file_pins.push(value.retained_file_pins[0]); },
    value => { value.retained_file_pins[0].bytes = 9007199254740992; },
    value => { value.retained_file_pins[0].inode = '1e3'; },
    value => { value.retained_file_pins[0].path = '/repo/../wrong'; },
    value => { value.retained_file_pins[0].extra = true; },
    value => { value.retained_pin_bytes++; },
  ]) { const fixture = sourceFixture(); mutate(fixture.receipt); assert.throws(() => sourcePinLedger(fixture.receipt)); }
  const missing = sourceFixture();
  missing.receipt.retained_file_pins = missing.receipt.retained_file_pins.filter(pin => pin.path !== '/source-output/repeat.ll');
  missing.receipt.retained_pin_bytes = missing.receipt.retained_file_pins.reduce((n, pin) => n + pin.bytes, 0);
  assert.throws(() => validateSource(missing));
});
test('both closed native profiles join complete synthetic payloads at full file offsets without mutation', () => {
  for (const profile of ['default', 'edited']) {
    const fixture = nativeFixture(profile), before = encoded(fixture.report), bytes = [...fixture.files].map(([file, value]) => [file, sha256(value)]);
    const cases = validateNative(fixture); assert.deepEqual(cases.map(value => value.optimization), ['O0', 'O3']);
    assert.deepEqual(cases[0].exact_payload_instruction_offsets, [128, 132, 136]);
    assert.deepEqual(encoded(fixture.report), before); assert.deepEqual([...fixture.files].map(([file, value]) => [file, sha256(value)]), bytes);
  }
});
test('source-to-native end-to-end pure join binds the actual selected LLVM from all three source lanes', () => {
  const source = sourceFixture(), joined = validateSource(source); let cases = 0;
  for (const profile of ['default', 'edited']) {
    const fixture = nativeFixture(profile), selected = joined.variants.find(value => value.label === profile);
    const expected = { ...fixture.expected, llvm_sha256: selected.llvm_sha256, llvm_bytes: selected.llvm_bytes };
    cases += validateNativeReport(fixture.report, expected, source.read(selected.llvm_path, LIMITS.source), fixture.read).length;
    const opposite = joined.variants.find(value => value.label === (profile === 'default' ? 'edited' : 'default'));
    assert.throws(() => validateNativeReport(fixture.report,
      { ...expected, llvm_sha256: opposite.llvm_sha256, llvm_bytes: opposite.llvm_bytes },
      source.read(opposite.llvm_path, LIMITS.source), fixture.read));
  }
  assert.equal(cases, 4);
  assert.equal(joined.variants[1].llvm_sha256, joined.variants[2].llvm_sha256);
  assert.notEqual(joined.variants[0].semantic_sha256, joined.variants[1].semantic_sha256);
  assert.equal(joined.variants[1].semantic_sha256, joined.variants[2].semantic_sha256);
});
test('coherently retained manifest dependency drift cannot bypass the current-source clone boundary', () => {
  const fixture = sourceFixture();
  for (const profile of ['default', 'edited']) {
    const file = CONTEXT.sourceRoot + '/' + profile + '-source/Cargo.toml';
    fixture.put(file, Buffer.from(fixture.files.get(file).toString().replace('/repo/crates/fe2o3-device', '/old/crates/fe2o3-device')));
  }
  fixture.sync(); assert.throws(() => validateSource(fixture));
});
test('native exact top-level scope, profile, build, LLVM, flags and complete optimization counts', () => {
  for (const mutate of [
    value => { value.profile = 'edited'; }, value => { value.llvm_sha256 = H('stale'); },
    value => { value.llvm_bytes++; }, value => { value.worker_build_claim = 'fe2o3-worker-v1-sha256-' + H('wrong'); },
    value => { value.llvm_build_claim = 'wrong'; }, value => { value.cases.pop(); },
    value => { value.cases.reverse(); }, value => { value.cases.push(copy(value.cases[0])); },
    value => { value.shape_controls.typed_shape_negatives = 47; }, value => { value.shape_controls.extra = true; },
    value => { value.result_use = 'unused'; }, value => { value.authority = 'proof'; },
    value => { value.runtime_closure_attestation = true; }, value => { value.extra = true; },
    value => { value.register_plan[0] = 6; }, value => { value.descriptors[2] = 412; },
  ]) { const fixture = nativeFixture(); mutate(fixture.report); assert.throws(() => validateNative(fixture)); }
  for (const key of ['prefix_source_admitted', 'production_exact_program_admission', 'protected_finalizer_admission',
    'artifact_authority', 'source_authentication', 'compiler_closure_attestation', 'hardware_execution',
    'native_whole_kernel_correctness', 'physical_register_allocation_or_lifetime_proof', 'whole_kernel_order_or_byte_stability_claim']) {
    const fixture = nativeFixture(); fixture.report[key] = true; assert.throws(() => validateNative(fixture));
  }
});
test('native e64/opcode/register/effect, missing controls and offset/descriptor extent refusals', () => {
  for (const mutate of [
    value => { value.machine_observation.program[2].opcode = 'V_OR_B32_e32_vi'; },
    value => { value.machine_observation.program[0].bytes_hex += '00'; },
    value => { value.machine_observation.program[0].register_operands[0] = 'VGPR6'; },
    value => { value.machine_observation.program[0].implicit_reads = []; },
    value => { value.machine_observation.program[0].implicit_writes = ['VCC']; },
    value => { value.machine_observation.program[0].mc_flags = 2; },
    value => { value.machine_observation.program[1].file_offset++; },
    value => { value.machine_observation.entry_code_bytes = 8; },
    value => { value.machine_observation.static_instruction_count = 9007199254740992; },
    value => { value.machine_observation.descriptor.file_offset = 128; },
    value => { value.machine_observation.descriptor.vgpr_capacity = 4; },
    value => { value.machine_observation.boundary_value_or_lifetime_proof = true; },
    value => { value.retained_payload.production_artifact_authority = true; },
    value => { value.retained_payload.path = '/native/edited/O0.hsaco'; },
    value => { value.mutation_controls.gapped_sequence_refusals = 0; },
    value => { value.machine_observation.post_link_checks[0] += ' extra'; },
  ]) { const fixture = nativeFixture(); mutate(fixture.report.cases[0]); assert.throws(() => validateNative(fixture)); }
});
test('whole payload SHA catches an unselected-byte mutation, not only attractive selected bytes', () => {
  const fixture = nativeFixture(); fixture.files.get('/native/default/O0.hsaco')[191] ^= 1;
  assert.throws(() => validateNative(fixture));
});
test('coherently rehashed payload still rejects wrong actual instruction bytes and wrong descriptor fields', () => {
  for (const offset of [128, 136, 64 + 48]) {
    const fixture = nativeFixture(); fixture.files.get('/native/default/O0.hsaco')[offset] ^= 1; refreshPayloadIdentity(fixture, 0);
    // Update descriptor SHA too, isolating actual resource word vs report.
    if (offset === 112) fixture.report.cases[0].machine_observation.descriptor.sha256 =
      sha256(fixture.files.get('/native/default/O0.hsaco').subarray(64, 128));
    assert.throws(() => validateNative(fixture));
  }
});
test('section-relative offset substitution refuses despite exact bytes existing elsewhere', () => {
  const fixture = nativeFixture(), value = fixture.report.cases[0].machine_observation;
  value.entry_file_offset = 0; value.program.forEach(site => { site.file_offset -= 128; });
  assert.throws(() => validateNative(fixture), 'must directly slice reported whole-file offset; no search fallback');
});
test('exact selected LLVM bytes, complete payload paths and raw JSON reject stale or lossy input', () => {
  const fixture = nativeFixture(); fixture.llvm[0] ^= 1; assert.throws(() => validateNative(fixture));
  for (const text of ['{"count":9007199254740993}', '{"count":1.0}', '{"count":1e2}',
    '{"count":1,"count":2}', '{"count":1,"\\u0063ount":2}', '{"count":1} trailing']) assert.throws(() => parseJson(Buffer.from(text)));
  assert.throws(() => parseJson(Buffer.from([0xff])));
  assert.throws(() => parseJson(Buffer.alloc(LIMITS.json + 1)));
});
test('closed CLI root selections enforce exact identity sizes, unique paths and new output scope', () => {
  const selected = { repo: '/repo', 'prepared-root': '/prepared', 'source-output': '/source-output', consumer: '/tools/promote_once',
    'bin-dir': '/tools', emitter: '/tools/lower', 'default-report': '/reports/default.json', 'default-payload-dir': '/native/default',
    'edited-report': '/reports/edited.json', 'edited-payload-dir': '/native/edited', output: '/joins/new.json',
    'source-receipt-sha256': H('source'), 'default-report-sha256': H('default'), 'edited-report-sha256': H('edited'),
    'source-receipt-bytes': '100', 'default-report-bytes': '200', 'edited-report-bytes': '200',
    'llvm-build-id': LLVM_CLAIM, 'worker-build-id': WORKER_CLAIM };
  const args = value => Object.entries(value).flatMap(([key, value]) => ['--' + key, value]);
  assert.equal(options(args(selected))['source-receipt-bytes'], 100);
  for (const [key, bad] of [['source-receipt-bytes', '1e2'], ['default-report-bytes', '65537'],
    ['source-receipt-sha256', '0'.repeat(64)], ['output', '/source-output/new.json'],
    ['edited-report', '/reports/default.json'], ['edited-payload-dir', '/native/default'],
    ['repo', '/repo/../repo'], ['worker-build-id', 'fake']]) assert.throws(() => options(args({ ...selected, [key]: bad })));
  assert.throws(() => options([...args(selected), '--extra', 'value']));
});
