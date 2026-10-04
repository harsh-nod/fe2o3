// Pure in-memory controls only. No child process, filesystem fixture, compiler,
// native execution, or retained receipt is created by these synthetic shapes.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import path from 'node:path';
import { LLVM_VERSION, LLVM_BUILD_ID, TEST_TARGET, MAX_REPORT, MAX_CACHE_BYTES,
  MIN_RAM_BYTES, EXPECTED_PROGRAMS, validateProgramObservation, parseProgramObservation,
  parseArguments, programBuildMeasurementRoster, parseProgramLinkInputs,
  parseCacheObservation, validateProgramBuildReceipt, validateProgramOutputLayout } from './ordered-program-worker-prototype.mjs';

const CLAIM = `fe2o3-worker-v1-sha256-${'a'.repeat(64)}`;
const HASH = 'b'.repeat(64);
const sha = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const copied = value => structuredClone(value);
function report() {
  const positive_cases = [];
  for (const profile of ['one', 'three', 'sixteen']) for (const optimization of ['O0', 'O3']) for (const result_used of [true, false]) {
    const program = EXPECTED_PROGRAMS[profile].map((expected, index) => ({ ...copied(expected),
      file_offset: 520 + 4 * index, mc_flags: 16, implicit_reads: ['EXEC'], implicit_writes: [] }));
    positive_cases.push({ profile, optimization, result_used, program_count: program.length,
      llvm_text_sha256: HASH, llvm_text_bytes: 512, hsaco_sha256: 'c'.repeat(64), hsaco_bytes: 2048,
      descriptor_sha256: 'd'.repeat(64), entry_file_offset: 512, entry_code_bytes: 256,
      static_instruction_count: program.length + 10, program, complete_exact_sequence_observed: true,
      descriptor_capacity_checked: true, descriptor_resources: {
        descriptor_file_offset: 128, descriptor_bytes: 64, descriptor_sha256: 'd'.repeat(64),
        compute_pgm_rsrc1: 4, compute_pgm_rsrc3: 9, vgpr_capacity: 40, architected_vgpr_boundary: 40,
        required_footprint_high_water: 37, interpretation: 'encoded-capacity-not-metadata-usage-or-lifetime' } });
  }
  return { schema: 'fe2o3-ordered-program-worker-prototype-v1', authority: 'unauthenticated-native-test-fixture',
    source_produced: false, production_exact_program_admission: false, protected_finalizer_admission: false,
    hardware_executed: false, physical_register_allocation_or_lifetime_proof: false,
    whole_kernel_order_or_byte_stability_claim: false, synthetic_worker_request_identity_fields: true,
    runtime_closure_attestation: 'unavailable', target: 'gfx942:xnack-', wave_width: 64,
    workgroup_size: 64, code_object_version: 6, llvm_build_claim: LLVM_BUILD_ID, worker_build_claim: CLAIM,
    positive_cases, inline_asm_guard_controls: { pure_parser_typed_call_positives: 13,
      pure_parser_typed_call_negatives: 11, worker_or_target_machine_invoked: false },
    decoded_observation_negatives: 18, actual_payload_mutation_negatives: 3,
    actual_e64_rejected_by_matcher: true,
    encoding_reference_scope: 'public gfx900 cross-check; actual pinned gfx942 qualification is this run' };
}
function rejects(mutators) {
  validateProgramObservation(report(), CLAIM);
  for (const mutate of mutators) { const value = report(); mutate(value); assert.throws(() => validateProgramObservation(value, CLAIM)); }
}

test('pure: exact twelve-case synthetic shape includes eighty observed instruction sites within report cap', () => {
  const value = report();
  assert.equal(validateProgramObservation(value, CLAIM), value);
  const bytes = Buffer.from(JSON.stringify(value, null, 2));
  assert(bytes.length <= MAX_REPORT);
  assert.deepEqual(parseProgramObservation(bytes, CLAIM), value);
  assert.equal(value.positive_cases.reduce((sum, entry) => sum + entry.program.length, 0), 80);
});

test('pure: literal bytes independently obey six GFX9 opcode fields and exact physical operand roles', () => {
  const bases = { V_MOV_B32_e32_vi: 0x7e000200, V_ADD_U32_e32_gfx9: 0x68000000,
    V_SUB_U32_e32_gfx9: 0x6a000000, V_AND_B32_e32_vi: 0x26000000,
    V_OR_B32_e32_vi: 0x28000000, V_XOR_B32_e32_vi: 0x2a000000 };
  const seen = new Set();
  for (const [profile, steps] of Object.entries(EXPECTED_PROGRAMS)) {
    assert.equal(steps.length, { one: 1, three: 3, sixteen: 16 }[profile]);
    assert(Object.isFrozen(steps));
    for (const step of steps) {
      const [destination, left, right] = step.register_operands.map(name => Number(name.slice(4)));
      assert([32, 33].includes(destination));
      assert(step.register_operands.every(name => /^VGPR3[2-6]$/.test(name)));
      const binary = step.opcode !== 'V_MOV_B32_e32_vi';
      assert.equal(step.register_operands.length, binary ? 3 : 2);
      const word = bases[step.opcode] + destination * 2 ** 17 + (binary ? right * 2 ** 9 : 0) + 256 + left;
      assert.equal(Buffer.from(step.bytes_hex, 'hex').readUInt32LE(), word);
      assert(Object.isFrozen(step)); assert(Object.isFrozen(step.register_operands)); seen.add(step.opcode);
    }
  }
  assert.equal(seen.size, 6);
  assert.deepEqual(EXPECTED_PROGRAMS.sixteen[14].register_operands, ['VGPR32', 'VGPR35']);
  assert.deepEqual(EXPECTED_PROGRAMS.sixteen[15].register_operands, ['VGPR33', 'VGPR33']);
});

test('pure: scope, target, schema, control counts and synthetic identities cannot be promoted', () => {
  rejects([
    value => { value.schema = 'fe2o3-ordered-inline-unit-worker-prototype-v1'; },
    value => { value.authority = 'production'; }, value => { value.source_produced = true; },
    value => { value.hardware_executed = true; }, value => { value.production_exact_program_admission = true; },
    value => { value.protected_finalizer_admission = true; },
    value => { value.physical_register_allocation_or_lifetime_proof = true; },
    value => { value.whole_kernel_order_or_byte_stability_claim = true; },
    value => { value.synthetic_worker_request_identity_fields = false; },
    value => { value.runtime_closure_attestation = 'verified'; }, value => { value.target = 'gfx950:xnack-'; },
    value => { value.wave_width = 32; }, value => { value.workgroup_size = 32; }, value => { value.code_object_version = 5; },
    value => { value.worker_build_claim = `fe2o3-worker-v1-sha256-${'e'.repeat(64)}`; },
    value => { value.llvm_build_claim = 'unmeasured'; }, value => { value.extra = true; },
    value => { value.inline_asm_guard_controls.pure_parser_typed_call_positives = 12; },
    value => { value.inline_asm_guard_controls.pure_parser_typed_call_negatives = 10; },
    value => { value.inline_asm_guard_controls.worker_or_target_machine_invoked = true; },
    value => { value.decoded_observation_negatives = 17; }, value => { value.actual_payload_mutation_negatives = 2; },
    value => { value.actual_e64_rejected_by_matcher = false; },
  ]);
});

test('pure: missing, duplicate, sparse and mismatched native case selectors reject', () => {
  rejects([
    value => { value.positive_cases.pop(); }, value => { value.positive_cases[1] = copied(value.positive_cases[0]); },
    value => { delete value.positive_cases[1]; }, value => { value.positive_cases[0].profile = 'constructor'; },
    value => { value.positive_cases[0].optimization = 'O2'; }, value => { value.positive_cases[0].result_used = 1; },
    value => { value.positive_cases[0].program_count = 16; }, value => { value.positive_cases[0].complete_exact_sequence_observed = false; },
    value => { value.positive_cases[2].llvm_text_sha256 = 'e'.repeat(64); },
    value => { value.positive_cases[2].llvm_text_bytes++; },
  ]);
});

test('pure: every authored site rejects changed bytes, opcode, operand or EXEC state independently', () => {
  for (const [caseIndex, entry] of report().positive_cases.entries()) {
    for (const stepIndex of entry.program.keys()) {
      for (const mutate of [site => { site.bytes_hex = '00000000'; }, site => { site.opcode += '_e64'; },
        site => { site.register_operands[0] = 'VGPR34'; }, site => { delete site.register_operands[0]; },
        site => { site.implicit_reads = []; }, site => { site.implicit_writes = ['EXEC']; }, site => { site.mc_flags = 1; }]) {
        const value = report(); mutate(value.positive_cases[caseIndex].program[stepIndex]);
        assert.throws(() => validateProgramObservation(value, CLAIM));
      }
    }
  }
});

test('pure: retention, order, extent, identity and integer precision changes reject', () => {
  rejects([
    value => { value.positive_cases[8].program.pop(); }, value => { value.positive_cases[8].program.reverse(); },
    value => { value.positive_cases[8].program[1] = copied(value.positive_cases[8].program[0]); },
    value => { value.positive_cases[8].program[15].file_offset += 4; },
    value => { value.positive_cases[0].program[0].file_offset = 2047; },
    value => { value.positive_cases[0].entry_code_bytes = 0; },
    value => { value.positive_cases[0].hsaco_bytes = Number.MAX_SAFE_INTEGER + 1; },
    value => { value.positive_cases[0].static_instruction_count = 513; },
    value => { value.positive_cases[0].descriptor_sha256 = '0'.repeat(64); },
    value => { value.positive_cases[0].llvm_text_sha256 = 'A'.repeat(64); },
  ]);
});

test('pure: each descriptor joins exact identity/range and eight-versus-four granule arithmetic', () => {
  const resources = value => value.positive_cases[0].descriptor_resources;
  rejects([
    value => { resources(value).descriptor_sha256 = 'e'.repeat(64); },
    value => { resources(value).descriptor_file_offset = 2040; }, value => { resources(value).descriptor_file_offset = 512; },
    value => { resources(value).descriptor_bytes = 63; }, value => { resources(value).compute_pgm_rsrc1 = 3; },
    value => { resources(value).compute_pgm_rsrc3 = 8; }, value => { resources(value).vgpr_capacity = 20; },
    value => { resources(value).architected_vgpr_boundary = 80; }, value => { resources(value).required_footprint_high_water = 36; },
    value => { resources(value).interpretation = 'verified-register-lifetime'; },
    value => { resources(value).compute_pgm_rsrc1 = 2 ** 32; },
    value => { value.positive_cases[0].descriptor_capacity_checked = false; },
  ]);
});

test('pure: raw report byte, UTF8, duplicate-key, structural and trailing-data controls', () => {
  const valid = JSON.stringify(report());
  for (const bytes of [Buffer.alloc(MAX_REPORT + 1), Buffer.alloc(0), Buffer.from([0xff]), Buffer.from('{}'),
    Buffer.from(valid + '{}'), Buffer.from(valid.replace('{', '{"schema":"duplicate",')),
    Buffer.from(valid.replace('"profile":"one"', '"profile":"one","pro\\u0066ile":"one"')),
    Buffer.from('['.repeat(17) + '0' + ']'.repeat(17)), Buffer.from('[' + '0,'.repeat(9000) + '0]')]) {
    assert.throws(() => parseProgramObservation(bytes, CLAIM));
  }
});

const REPO = '/synthetic/compiler', OUTPUT = '/synthetic/new-native', LLVM = '/synthetic/llvm';
const CXX = '/usr/bin/g++', ZSTD = '/synthetic/libzstd.so', ROOTS = ['/synthetic/cargo-cache', '/synthetic/secondary-cache'];
function configure() {
  return { executable: '/usr/local/bin/cmake', args: ['-S', path.join(REPO, 'tools/fe2o3-llvm-link-worker'),
    '-B', path.join(OUTPUT, 'build'), '-G', 'Unix Makefiles', `-DLLVM_DIR=${LLVM}/lib/cmake/llvm`, `-DLLD_DIR=${LLVM}/lib/cmake/lld`,
    `-DFE2O3_PINNED_LLVM_VERSION=${LLVM_VERSION}`, `-DFE2O3_EXPECTED_LLVM_BUILD_ID=${LLVM_BUILD_ID}`,
    '-DFE2O3_LLVM_BUILD_ID_FILE=/synthetic/build-id', `-DFE2O3_GFX942_DEVICE_LIB_DIR=${OUTPUT}/no-device-libraries`,
    `-DFE2O3_GFX950_DEVICE_LIB_DIR=${OUTPUT}/no-device-libraries`, '-Dzstd_INCLUDE_DIR=/synthetic/include',
    `-Dzstd_LIBRARY=${ZSTD}`, '-DCMAKE_BUILD_TYPE=Release', `-DCMAKE_CXX_COMPILER=${CXX}`, '-DBUILD_TESTING=ON'] };
}
function linkText(target) {
  const object = target === TEST_TARGET ? 'tests/OrderedProgramPrototypeTests.cpp.o' : 'src/main.cpp.o';
  return `${CXX} -O3 -DNDEBUG "CMakeFiles/${target}.dir/${object}" -o ${target} libfe2o3_worker_pipeline.a `
    + `libfe2o3_worker_protocol.a ${LLVM}/lib/liblldELF.a ${LLVM}/lib/liblldCommon.a `
    + `${LLVM}/lib/libLLVMCore.a ${LLVM}/lib/libLLVMAsmParser.a ${ZSTD} /usr/lib/x86_64-linux-gnu/libz.so -lrt -ldl -lm\n`;
}
const measured = (requested, bytes = 32, digest = HASH) => ({ requested, resolved: requested, bytes, sha256: digest });
function buildReceipt() {
  const setup = configure(), roster = programBuildMeasurementRoster({ repo: REPO, output: OUTPUT, configure: setup });
  const cacheText = ROOTS.map(root => `1024\t${root}\n`).join('');
  const observation = Buffer.from(JSON.stringify(report()));
  const names = ['git-head', 'git-status', 'cache-before-configure', 'configure', 'cache-after-configure',
    'cache-before-build', 'build', 'cache-after-build', 'cache-before-native', 'native-test', 'cache-after-native'];
  const commands = new Map([['git-head', ['/usr/bin/git', ['rev-parse', 'HEAD']]],
    ['git-status', ['/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal']]],
    ['configure', [setup.executable, setup.args]], ['build', [setup.executable,
      ['--build', `${OUTPUT}/build`, '--target', TEST_TARGET, 'fe2o3-llvm-link-worker', '--parallel', '2']]],
    ['native-test', [`${OUTPUT}/build/${TEST_TARGET}`, []]]]);
  const stages = names.map(stage => {
    const [executable, args] = commands.get(stage) ?? ['/usr/bin/du', ['-sb', '--', ...ROOTS]];
    const stdout = stage.startsWith('cache-') ? Buffer.from(cacheText) : stage === 'native-test' ? observation : Buffer.alloc(0);
    return { stage, executable, args, code: 0, signal: null, reason: null, elapsed_ms: 1,
      free_bytes_before: String(40n * 1024n ** 3n), ram_bytes_before: String(MIN_RAM_BYTES), ram_bytes_after: String(MIN_RAM_BYTES),
      stdout_sha256: sha(stdout), stdout_bytes: stdout.length, stderr_sha256: sha(Buffer.alloc(0)), stderr_bytes: 0,
      cache_observation: stage.startsWith('cache-') ? parseCacheObservation(stdout, ROOTS) : null };
  });
  const linkCommands = [TEST_TARGET, 'fe2o3-llvm-link-worker'].map(target => {
    const text = linkText(target);
    return { ...measured(`${OUTPUT}/build/CMakeFiles/${target}.dir/link.txt`, Buffer.byteLength(text), sha(Buffer.from(text))), text };
  });
  const libraries = parseProgramLinkInputs(Buffer.from(linkText(TEST_TARGET)), { target: TEST_TARGET, llvm: LLVM, cxx: CXX, zstdLibrary: ZSTD });
  return { schema: 'fe2o3-ordered-program-engineering-receipt-v1', status: 'passed',
    scope: 'synthetic native program/encoding/resource observation only', source_produced: false,
    production_exact_program_admission: false, protected_finalizer_admission: false, hardware_executed: false,
    runtime_closure_attestation: 'unavailable', policy_or_source_gate_changes: false,
    environment: { platform: 'linux', arch: 'x64', node: 'v22.0.0', os_release: 'synthetic', compiler_repo: REPO,
      output: OUTPUT, compiler_head: 'a'.repeat(40), compiler_worktree_dirty: true, cache_roots: ROOTS,
      filesystem_restriction: validateProgramOutputLayout({ repo: REPO, output: OUTPUT, cacheRoots: ROOTS, repoDevice: '7', outputParentDevice: '7' }),
      child_environment: { PATH: '/usr/local/bin:/usr/bin:/bin', LANG: 'C', LC_ALL: 'C', HOME: '/synthetic/user', TMPDIR: `${OUTPUT}/tmp` } },
    limits: { jobs: 2, minimum_free_bytes: String(40n * 1024n ** 3n), minimum_available_ram_bytes: String(MIN_RAM_BYTES),
      maximum_combined_cache_bytes: String(MAX_CACHE_BYTES), configure_ms: 120000, build_ms: 900000,
      test_ms: 120000, test_stdout_bytes: MAX_REPORT, disk_reserve_directory: REPO }, stages,
    inputs: roster.inputs.map(item => measured(item.requested)), artifacts: roster.artifacts.map(item => measured(item.requested)),
    link_inputs: { scope: 'selected generated static link inputs; not complete compiler, system-library or runtime closure',
      commands: linkCommands, libraries: libraries.map(file => measured(file)) },
    observation: { sha256: sha(observation), bytes: observation.length, worker_build_claim: CLAIM,
      positive_cases: 12, program_sites: 80, parser_controls: [13, 11], native_controls: [18, 3, 1] } };
}
const validateBuild = value => validateProgramBuildReceipt(value, { repo: REPO, output: OUTPUT });

test('pure: expanded immutable measured roster includes all new leaves, pipeline closure, tools and configs', () => {
  const value = buildReceipt(), roster = validateBuild(value);
  for (const name of ['OrderedProgramPrototypeTests.cpp', 'OrderedProgramWorkerSupport.inc', 'OrderedProgramSourceObservation.inc',
    'CMakeLists.txt', 'WorkerPipeline.cpp', 'WorkerProtocol.cpp', 'WorkerMachineEffect.cpp']) {
    assert(roster.inputs.some(item => path.basename(item.requested) === name));
  }
  assert(roster.inputs.some(item => item.requested === '/usr/bin/make'));
  assert(roster.inputs.some(item => item.requested === '/usr/bin/ld'));
  assert.equal(roster.artifacts.length, 4);
  assert(Object.isFrozen(roster)); assert(Object.isFrozen(roster.inputs));
  for (const item of [...roster.inputs, ...roster.artifacts]) assert(Object.isFrozen(item));
});

test('pure: omission of every source/tool/config/archive or artifact and altered link bytes rejects', () => {
  const baseline = buildReceipt();
  for (const kind of ['inputs', 'artifacts']) for (let index = 0; index < baseline[kind].length; index++) {
    const value = copied(baseline); value[kind].splice(index, 1); assert.throws(() => validateBuild(value));
  }
  for (let index = 0; index < baseline.link_inputs.libraries.length; index++) {
    const value = copied(baseline); value.link_inputs.libraries.splice(index, 1); assert.throws(() => validateBuild(value));
  }
  for (const mutate of [value => { value.inputs.reverse(); }, value => { value.inputs[1] = value.inputs[0]; },
    value => { value.artifacts[0].resolved = '/redirected'; }, value => { value.inputs[0].sha256 = '0'.repeat(64); },
    value => { value.link_inputs.commands[0].text += ' '; }, value => { value.link_inputs.commands.pop(); },
    value => { value.link_inputs.libraries[0].requested = '/unreviewed.a'; }]) {
    const value = copied(baseline); mutate(value); assert.throws(() => validateBuild(value));
  }
});

test('pure: build receipt refuses authority, lowered guards, environment and command drift', () => {
  for (const mutate of [value => { value.schema = 'fe2o3-ordered-inline-unit-engineering-receipt-v1'; },
    value => { value.hardware_executed = true; }, value => { value.environment.child_environment.LD_PRELOAD = '/evil'; },
    value => { value.environment.filesystem_restriction.output_parent_device = '8'; },
    value => { value.environment.filesystem_restriction.policy = 'any_filesystem'; },
    value => { value.limits.jobs = 16; }, value => { value.limits.minimum_free_bytes = '1'; },
    value => { value.limits.maximum_combined_cache_bytes = String(MAX_CACHE_BYTES + 1n); },
    value => { value.stages.reverse(); }, value => { value.stages[6].args[6] = '16'; },
    value => { value.stages[3].args[11] = '-DFE2O3_GFX942_DEVICE_LIB_DIR=/enabled'; },
    value => { value.stages[3].args[8] = '-DFE2O3_PINNED_LLVM_VERSION=23'; },
    value => { delete value.stages[3].args[1]; },
    value => { value.stages[3].reason = 'timeout'; }, value => { value.stages[6].ram_bytes_after = '1'; },
    value => { value.stages[2].cache_observation[0].logical_bytes = '0'; },
    value => { value.stages[9].stdout_sha256 = 'e'.repeat(64); },
    value => { value.observation.program_sites = 79; }, value => { value.observation.parser_controls = [0, 0]; }]) {
    const value = copied(buildReceipt()); mutate(value); assert.throws(() => validateBuild(value));
  }
});

test('pure: generated static link grammar rejects injected commands, unmeasured libraries and changed build mode', () => {
  const options = { target: TEST_TARGET, llvm: LLVM, cxx: CXX, zstdLibrary: ZSTD }, valid = linkText(TEST_TARGET);
  assert.equal(parseProgramLinkInputs(Buffer.from(valid), options).length, 6);
  for (const text of [valid.replace('-O3', '-O0'), valid + '; touch /tmp/x', valid.replace(' -o ', ' -shared '),
    valid.replace('libLLVMCore.a', 'libLLVM.so'), valid.replace(`${LLVM}/lib/libLLVMCore.a`, '/other/libLLVMCore.a'),
    valid.replace('libfe2o3_worker_pipeline.a ', ''), valid.replace(`${LLVM}/lib/liblldELF.a `, ''),
    valid + ' -lunknown', valid.replace(' -lm', ' $(command)'), valid.replace(' -lm', ' @response-file')]) {
    assert.throws(() => parseProgramLinkInputs(Buffer.from(text), options));
  }
});

test('pure: two-cache combined logical bound is exact and rejects overlaps, omissions, duplicates or lossy numbers', () => {
  assert.equal(parseCacheObservation(Buffer.from(`${MAX_CACHE_BYTES}\t${ROOTS[0]}\n0\t${ROOTS[1]}\n`), ROOTS).length, 2);
  for (const text of [`${MAX_CACHE_BYTES}\t${ROOTS[0]}\n1\t${ROOTS[1]}\n`,
    `0\t${ROOTS[0]}\n0\t${ROOTS[0]}\n`, `0\t${ROOTS[0]}\n`,
    `1e3\t${ROOTS[0]}\n0\t${ROOTS[1]}\n`, `01\t${ROOTS[0]}\n0\t${ROOTS[1]}\n`]) {
    assert.throws(() => parseCacheObservation(Buffer.from(text), ROOTS));
  }
  assert.throws(() => parseCacheObservation(Buffer.from('0\t/cache\n0\t/cache/sub\n'), ['/cache', '/cache/sub']));
});

test('pure: CLI requires two explicit cache roots and bounded canonical unique options', () => {
  const args = ['--llvm-root', LLVM, '--llvm-build-id-file', '/synthetic/build-id', '--zstd-include-dir', '/synthetic/include',
    '--zstd-library', ZSTD, '--output', OUTPUT, '--cargo-cache-root', ROOTS[0], '--secondary-cache-root', ROOTS[1]];
  assert.deepEqual(parseArguments(args).cacheRoots, ROOTS);
  for (const bad of [[], args.slice(0, -2), [...args, '--output', '/duplicate'], [...args, '--shell', '/bin/sh'],
    args.map(value => value === OUTPUT ? '/tmp/../rewritten' : value),
    args.map(value => value === OUTPUT ? 'relative' : value), [...args, '--cxx']]) {
    assert.throws(() => parseArguments(bad));
  }
});

test('pure: output is disjoint from caches/checkout and shares the actively monitored filesystem', () => {
  const options = { repo: REPO, output: OUTPUT, cacheRoots: ROOTS, repoDevice: '7', outputParentDevice: '7' };
  assert.equal(validateProgramOutputLayout(options).compiler_repo_device, '7');
  for (const output of [REPO, `${REPO}/build`, '/synthetic', ROOTS[0], `${ROOTS[1]}/native`]) {
    assert.throws(() => validateProgramOutputLayout({ ...options, output }), /overlap/);
  }
  assert.throws(() => validateProgramOutputLayout({ ...options, outputParentDevice: '8' }), /filesystem device/);
  assert.throws(() => validateProgramOutputLayout({ ...options, repoDevice: '7e0' }), /decimal/);
});
