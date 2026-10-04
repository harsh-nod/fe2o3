#!/usr/bin/env node
// Synthetic native observations only; no source/production/GPU authority.
// Imports have no process or filesystem side effects. The V16 runner is unchanged.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { LLVM_VERSION, LLVM_BUILD_ID, MIN_FREE_BYTES, runBoundedCommand,
  requireDiskReserve, readRegular, measureNativeBuildInput,
  requireNativeBuildInputsUnchanged, nativeBuildMeasurementRoster }
  from './assembly-region-worker-prototype.mjs';

export { LLVM_VERSION, LLVM_BUILD_ID };
export const TEST_TARGET = 'fe2o3-worker-ordered-program-prototype-tests';
export const MAX_REPORT = 64 * 1024;
export const MAX_CACHE_BYTES = 20n * 1024n ** 3n;
export const MIN_RAM_BYTES = 64n * 1024n ** 3n;
const MiB = 1024 * 1024;
const HERE = path.dirname(fileURLToPath(import.meta.url));
const CLAIM = /^fe2o3-worker-v1-sha256-[0-9a-f]{64}$/;
const FALSE_FIELDS = ['source_produced', 'production_exact_program_admission',
  'protected_finalizer_admission', 'hardware_executed',
  'physical_register_allocation_or_lifetime_proof', 'whole_kernel_order_or_byte_stability_claim'];
const ROOT_KEYS = ['schema', 'authority', ...FALSE_FIELDS,
  'synthetic_worker_request_identity_fields', 'runtime_closure_attestation',
  'target', 'wave_width', 'workgroup_size', 'code_object_version',
  'llvm_build_claim', 'worker_build_claim', 'positive_cases', 'inline_asm_guard_controls',
  'decoded_observation_negatives', 'actual_payload_mutation_negatives',
  'actual_e64_rejected_by_matcher', 'encoding_reference_scope'];
const CASE_KEYS = ['optimization', 'profile', 'program_count', 'result_used',
  'llvm_text_sha256', 'llvm_text_bytes', 'hsaco_sha256', 'hsaco_bytes',
  'descriptor_sha256', 'entry_file_offset', 'entry_code_bytes', 'static_instruction_count',
  'program', 'descriptor_resources', 'complete_exact_sequence_observed', 'descriptor_capacity_checked'];
const SITE_KEYS = ['file_offset', 'opcode', 'bytes_hex', 'mc_flags',
  'register_operands', 'implicit_reads', 'implicit_writes'];
const RESOURCE_KEYS = ['descriptor_file_offset', 'descriptor_bytes', 'descriptor_sha256',
  'compute_pgm_rsrc1', 'compute_pgm_rsrc3', 'vgpr_capacity',
  'architected_vgpr_boundary', 'required_footprint_high_water', 'interpretation'];
function demand(value, reason) { if (!value) throw new Error(reason); }
function exact(value, keys, label) {
  demand(value && typeof value === 'object' && !Array.isArray(value), `${label}: object required`);
  demand(Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key)),
    `${label}: unknown/missing fields`);
}
function dense(value, count, label) {
  demand(Array.isArray(value) && value.length === count
    && Array.from({ length: count }, (_, index) => Object.hasOwn(value, index)).every(Boolean), `${label}: exact dense array`);
}
function same(actual, expected, label) {
  dense(actual, expected.length, label);
  demand(actual.every((value, index) => value === expected[index]), label);
}
function natural(value, maximum, label, minimum = 0) {
  demand(Number.isSafeInteger(value) && value >= minimum && value <= maximum, `${label}: integer bound`);
}
function digest(value, label) {
  demand(typeof value === 'string' && /^[0-9a-f]{64}$/.test(value) && value !== '0'.repeat(64), `${label}: digest`);
}
function sha256(bytes) { return crypto.createHash('sha256').update(bytes).digest('hex'); }
function freeze(value) {
  if (value && typeof value === 'object') { Object.values(value).forEach(freeze); Object.freeze(value); }
  return value;
}
function literal(opcode, bytes_hex, ...registers) {
  return { opcode, bytes_hex, register_operands: registers.map(register => `VGPR${register}`) };
}
// Independently fixed GFX9 field/byte expectations, not populated from LLVM,
// the Rust descriptors, or a candidate response. These are not native evidence.
export const EXPECTED_PROGRAMS = freeze({
  one: [literal('V_MOV_B32_e32_vi', '2203427e', 33, 34)],
  three: [literal('V_XOR_B32_e32_vi', '2247402a', 32, 34, 35),
    literal('V_AND_B32_e32_vi', '20494026', 32, 32, 36),
    literal('V_XOR_B32_e32_vi', '2341422a', 33, 35, 32)],
  sixteen: [literal('V_MOV_B32_e32_vi', '2203407e', 32, 34),
    literal('V_XOR_B32_e32_vi', '2247422a', 33, 34, 35),
    literal('V_AND_B32_e32_vi', '21494026', 32, 33, 36),
    literal('V_OR_B32_e32_vi', '20474228', 33, 32, 35),
    literal('V_ADD_U32_e32_gfx9', '21494068', 32, 33, 36),
    literal('V_SUB_U32_e32_gfx9', '2045426a', 33, 32, 34),
    literal('V_MOV_B32_e32_vi', '2103407e', 32, 33),
    literal('V_XOR_B32_e32_vi', '2047402a', 32, 32, 35),
    literal('V_OR_B32_e32_vi', '20454228', 33, 32, 34),
    literal('V_AND_B32_e32_vi', '21494226', 33, 33, 36),
    literal('V_ADD_U32_e32_gfx9', '21454268', 33, 33, 34),
    literal('V_SUB_U32_e32_gfx9', '2147406a', 32, 33, 35),
    literal('V_MOV_B32_e32_vi', '2003427e', 33, 32),
    literal('V_XOR_B32_e32_vi', '2149422a', 33, 33, 36),
    literal('V_MOV_B32_e32_vi', '2303407e', 32, 35),
    literal('V_MOV_B32_e32_vi', '2103427e', 33, 33)],
});

export function validateProgramObservation(value, workerClaim) {
  demand(typeof workerClaim === 'string' && CLAIM.test(workerClaim), 'expected worker claim');
  exact(value, ROOT_KEYS, 'program report');
  demand(value.schema === 'fe2o3-ordered-program-worker-prototype-v1'
    && value.authority === 'unauthenticated-native-test-fixture', 'program report schema/scope');
  for (const field of FALSE_FIELDS) demand(value[field] === false, `unsupported authority: ${field}`);
  demand(value.synthetic_worker_request_identity_fields === true
    && value.runtime_closure_attestation === 'unavailable', 'synthetic request/closure scope');
  demand(value.target === 'gfx942:xnack-' && value.wave_width === 64
    && value.workgroup_size === 64 && value.code_object_version === 6, 'program target profile');
  demand(value.llvm_build_claim === LLVM_BUILD_ID && value.worker_build_claim === workerClaim, 'program build claims');
  demand(value.encoding_reference_scope === 'public gfx900 cross-check; actual pinned gfx942 qualification is this run', 'encoding reference scope');
  const controls = value.inline_asm_guard_controls;
  exact(controls, ['pure_parser_typed_call_positives', 'pure_parser_typed_call_negatives', 'worker_or_target_machine_invoked'], 'parser controls');
  demand(controls.pure_parser_typed_call_positives === 13 && controls.pure_parser_typed_call_negatives === 11
    && controls.worker_or_target_machine_invoked === false, 'parser controls counts/scope');
  demand(value.decoded_observation_negatives === 18 && value.actual_payload_mutation_negatives === 3
    && value.actual_e64_rejected_by_matcher === true, 'native control counts');
  dense(value.positive_cases, 12, 'native cases');
  const seen = new Set(), llvmByProfile = new Map();
  for (const entry of value.positive_cases) {
    exact(entry, CASE_KEYS, 'native case');
    demand(Object.hasOwn(EXPECTED_PROGRAMS, entry.profile) && ['O0', 'O3'].includes(entry.optimization)
      && typeof entry.result_used === 'boolean', 'native case selector');
    const key = `${entry.profile}/${entry.optimization}/${entry.result_used}`;
    demand(!seen.has(key), 'duplicate native case'); seen.add(key);
    const expected = EXPECTED_PROGRAMS[entry.profile];
    demand(entry.program_count === expected.length && entry.complete_exact_sequence_observed === true
      && entry.descriptor_capacity_checked === true, 'native sequence/count flags');
    for (const field of ['llvm_text_sha256', 'hsaco_sha256', 'descriptor_sha256']) digest(entry[field], field);
    natural(entry.llvm_text_bytes, MAX_REPORT, 'LLVM bytes', 1);
    natural(entry.hsaco_bytes, MiB, 'HSACO bytes', 1);
    natural(entry.entry_file_offset, entry.hsaco_bytes, 'entry offset');
    natural(entry.entry_code_bytes, entry.hsaco_bytes - entry.entry_file_offset, 'entry bytes', expected.length * 4);
    natural(entry.static_instruction_count, 512, 'instruction count', expected.length);
    const llvmKey = `${entry.profile}/${entry.result_used}`;
    const llvmIdentity = `${entry.llvm_text_sha256}/${entry.llvm_text_bytes}`;
    demand(!llvmByProfile.has(llvmKey) || llvmByProfile.get(llvmKey) === llvmIdentity,
      'O0/O3 must start from identical synthetic LLVM bytes');
    llvmByProfile.set(llvmKey, llvmIdentity);
    dense(entry.program, expected.length, 'program steps');
    for (const [index, site] of entry.program.entries()) {
      exact(site, SITE_KEYS, 'program site');
      natural(site.file_offset, entry.hsaco_bytes, 'program offset');
      demand(site.file_offset >= entry.entry_file_offset && site.file_offset + 4 <= entry.entry_file_offset + entry.entry_code_bytes,
        'program site outside exact entry');
      demand(index === 0 || site.file_offset === entry.program[index - 1].file_offset + 4, 'program is not contiguous');
      demand(site.opcode === expected[index].opcode && site.bytes_hex === expected[index].bytes_hex, 'literal program opcode/bytes');
      same(site.register_operands, expected[index].register_operands, 'literal physical operand roles');
      same(site.implicit_reads, ['EXEC'], 'program EXEC read'); same(site.implicit_writes, [], 'program implicit writes');
      natural(site.mc_flags, 16, 'program flags'); demand((site.mc_flags & ~16) === 0, 'program unexpected flags');
    }
    const resource = entry.descriptor_resources;
    exact(resource, RESOURCE_KEYS, 'descriptor resources');
    natural(resource.descriptor_file_offset, entry.hsaco_bytes, 'descriptor offset');
    demand(resource.descriptor_bytes === 64 && resource.descriptor_file_offset + 64 <= entry.hsaco_bytes
      && resource.descriptor_sha256 === entry.descriptor_sha256, 'descriptor extent/identity');
    demand(resource.descriptor_file_offset + 64 <= entry.entry_file_offset
      || resource.descriptor_file_offset >= entry.entry_file_offset + entry.entry_code_bytes, 'descriptor overlaps entry code');
    natural(resource.compute_pgm_rsrc1, 0xffffffff, 'RSRC1'); natural(resource.compute_pgm_rsrc3, 0xffffffff, 'RSRC3');
    const capacity = ((resource.compute_pgm_rsrc1 & 63) + 1) * 8;
    const boundary = ((resource.compute_pgm_rsrc3 & 63) + 1) * 4;
    demand(resource.vgpr_capacity === capacity && resource.architected_vgpr_boundary === boundary
      && capacity >= boundary && boundary >= 37 && resource.required_footprint_high_water === 37
      && resource.interpretation === 'encoded-capacity-not-metadata-usage-or-lifetime', 'descriptor capacity interpretation');
  }
  return value;
}

// Reject duplicate object keys before ordinary parsing. Bounded, data-only JSON
// grammar scan: depth<=16, <=8192 values, <=64KiB bytes; no eval/reviver hooks.
function uniqueJson(text) {
  let index = 0, nodes = 0;
  const whitespace = () => { while (/\s/.test(text[index] ?? '') && index < text.length) index++; };
  function string() {
    const begin = index++;
    while (index < text.length) {
      const byte = text[index++];
      if (byte === '\\') index++;
      else if (byte === '"') return JSON.parse(text.slice(begin, index));
    }
    throw new Error('unterminated JSON string');
  }
  function value(depth) {
    demand(depth <= 16 && ++nodes <= 8192, 'JSON structural cap'); whitespace();
    const start = text[index++];
    if (start === '{' || start === '[') {
      const object = start === '{', end = object ? '}' : ']', keys = new Set();
      whitespace(); if (text[index] === end) { index++; return; }
      for (;;) {
        whitespace();
        if (object) {
          demand(text[index] === '"', 'JSON object key');
          const key = string(); demand(!keys.has(key), 'duplicate JSON key'); keys.add(key);
          whitespace(); demand(text[index++] === ':', 'JSON colon');
        }
        value(depth + 1); whitespace();
        if (text[index] === end) { index++; return; }
        demand(text[index++] === ',', 'JSON separator');
      }
    }
    if (start === '"') { index--; string(); return; }
    const begin = index - 1;
    while (index < text.length && !/[\s,\]}]/.test(text[index])) index++;
    demand(index > begin && text.slice(begin, index).length <= 128, 'JSON scalar bound');
    JSON.parse(text.slice(begin, index));
  }
  value(0); whitespace(); demand(index === text.length, 'JSON trailing data');
  return JSON.parse(text);
}
export function parseProgramObservation(bytes, workerClaim) {
  demand(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_REPORT, 'program report byte cap');
  return validateProgramObservation(uniqueJson(new TextDecoder('utf-8', { fatal: true }).decode(bytes)), workerClaim);
}

function absolute(value, label) {
  demand(typeof value === 'string' && Buffer.byteLength(value) <= 4096 && path.isAbsolute(value)
    && path.resolve(value) === value && !/[\x00-\x1f\x7f]/.test(value), `${label}: canonical absolute path`);
  return value;
}
function inside(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (relative !== '..' && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative));
}
export function validateProgramOutputLayout({ repo, output, cacheRoots, repoDevice, outputParentDevice }) {
  absolute(repo, 'compiler repo'); absolute(output, 'output'); dense(cacheRoots, 2, 'cache roots');
  demand(!inside(repo, output) && !inside(output, repo), 'output and compiler checkout overlap');
  for (const root of cacheRoots) {
    absolute(root, 'cache root');
    demand(!inside(root, output) && !inside(output, root), 'output and shared cache trees overlap');
  }
  decimal(repoDevice, 0n, 99999999999999999999n, 'compiler filesystem device');
  decimal(outputParentDevice, 0n, 99999999999999999999n, 'output filesystem device');
  demand(repoDevice === outputParentDevice, 'output must share the monitored compiler filesystem device');
  return Object.freeze({ policy: 'output_and_compiler_repo_same_device_no_output_cache_overlap',
    compiler_repo_device: repoDevice, output_parent_device: outputParentDevice });
}
export function parseArguments(argv) {
  demand(Array.isArray(argv) && argv.length <= 20 && argv.length % 2 === 0, 'option count');
  const allowed = ['llvm-root', 'llvm-build-id-file', 'zstd-include-dir', 'zstd-library', 'output',
    'compiler-repo', 'cmake', 'cxx', 'cargo-cache-root', 'secondary-cache-root'];
  const values = new Map();
  for (let index = 0; index < argv.length; index += 2) {
    const key = argv[index]; demand(allowed.some(name => key === `--${name}`) && !values.has(key), 'unknown/duplicate option');
    values.set(key, absolute(argv[index + 1], key));
  }
  for (const key of ['llvm-root', 'llvm-build-id-file', 'zstd-include-dir', 'zstd-library', 'output', 'cargo-cache-root', 'secondary-cache-root'])
    demand(values.has(`--${key}`), `missing --${key}`);
  return { llvm: values.get('--llvm-root'), buildIdFile: values.get('--llvm-build-id-file'),
    zstdInclude: values.get('--zstd-include-dir'), zstdLibrary: values.get('--zstd-library'), output: values.get('--output'),
    repo: values.get('--compiler-repo') ?? path.resolve(HERE, '..'), cmake: values.get('--cmake') ?? '/usr/local/bin/cmake',
    cxx: values.get('--cxx') ?? '/usr/bin/g++', cacheRoots: [values.get('--cargo-cache-root'), values.get('--secondary-cache-root')] };
}

export function programBuildMeasurementRoster({ repo, output, configure }) {
  exact(configure, ['executable', 'args'], 'program configure');
  dense(configure.args, 18, 'program configure arguments');
  const inherited = nativeBuildMeasurementRoster({ repo, output, configure });
  const source = path.join(repo, 'tools/fe2o3-llvm-link-worker');
  const llvmDir = configure.args[6].slice('-DLLVM_DIR='.length);
  const spec = (requested, cap) => Object.freeze({ requested: absolute(requested, 'roster path'), cap });
  const inputs = Object.freeze([...inherited.inputs,
    ...['OrderedProgramPrototypeTests.cpp', 'OrderedProgramWorkerSupport.inc', 'OrderedProgramSourceObservation.inc']
      .map(name => spec(path.join(source, 'tests', name), 2 * MiB)),
    ...['ordered-program-worker-prototype.mjs', 'ordered-program-worker-prototype.test.mjs']
      .map(name => spec(path.join(HERE, name), MiB)),
    ...['/usr/bin/git', '/usr/bin/make', '/usr/bin/ld', '/usr/bin/du', process.execPath].map(file => spec(file, 512 * MiB)),
    ...['LLVMExports.cmake', 'LLVMExports-release.cmake'].map(name => spec(path.join(llvmDir, name), MiB)),
    ...['LLDTargets.cmake', 'LLDTargets-release.cmake'].map(name => spec(path.join(path.dirname(llvmDir), 'lld', name), MiB)),
  ]);
  demand(inputs.length <= 64 && new Set(inputs.map(item => item.requested)).size === inputs.length, 'input roster duplicate/count');
  const artifacts = Object.freeze([TEST_TARGET, 'fe2o3-llvm-link-worker', 'fe2o3-worker-build-id.txt', 'fe2o3-llvm-build-id.txt']
    .map(name => spec(path.join(output, 'build', name), name.endsWith('.txt') ? 256 : 512 * MiB)));
  return Object.freeze({ inputs, artifacts });
}

// The reviewed package is static LLVM. Parse, never execute, its generated
// Release link command; unknown flags/libraries/quoting fail closed. Compiler
// builtins and system -l dependencies remain outside runtime-closure claims.
export function parseProgramLinkInputs(bytes, { target, llvm, cxx, zstdLibrary }) {
  demand(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_REPORT, 'link command byte cap');
  demand([TEST_TARGET, 'fe2o3-llvm-link-worker'].includes(target), 'link target');
  for (const value of [llvm, cxx, zstdLibrary]) absolute(value, 'link path');
  const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes).trim();
  const tokens = [], expression = /[ \t]*(?:"([^"\\\r\n$`]+)"|([^ \t"\\\r\n$`;|&<>]+))/gy;
  let end = 0, match;
  while ((match = expression.exec(text)) !== null) { tokens.push(match[1] ?? match[2]); end = expression.lastIndex; }
  demand(end === text.length && tokens.length >= 12 && tokens.length <= 256, 'closed link token grammar');
  const object = target === TEST_TARGET ? 'tests/OrderedProgramPrototypeTests.cpp.o' : 'src/main.cpp.o';
  same(tokens.slice(0, 6), [cxx, '-O3', '-DNDEBUG', `CMakeFiles/${target}.dir/${object}`, '-o', target], 'Release link prefix');
  const libraries = new Set();
  const generated = new Set();
  for (const token of tokens.slice(6)) {
    if (['libfe2o3_worker_pipeline.a', 'libfe2o3_worker_protocol.a'].includes(token)) { generated.add(token); continue; }
    if (['-lrt', '-ldl', '-lm'].includes(token)) continue;
    absolute(token, 'linked library');
    demand((path.dirname(token) === path.join(llvm, 'lib') && /^lib(?:LLVM[A-Za-z0-9]+|lldELF|lldCommon)\.a$/.test(path.basename(token)))
      || token === zstdLibrary || token === '/usr/lib/x86_64-linux-gnu/libz.so', 'unreviewed linked library');
    libraries.add(token);
  }
  demand(generated.size === 2 && libraries.has(path.join(llvm, 'lib/liblldELF.a'))
    && libraries.has(path.join(llvm, 'lib/liblldCommon.a')) && libraries.has(zstdLibrary)
    && libraries.size <= 128, 'required static library graph');
  return Object.freeze([...libraries].sort());
}

export function parseCacheObservation(bytes, roots) {
  dense(roots, 2, 'cache roots'); roots.forEach(root => absolute(root, 'cache root'));
  demand(!inside(roots[0], roots[1]) && !inside(roots[1], roots[0]), 'cache roots overlap');
  demand(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= 16 * 1024, 'cache report byte cap');
  const lines = new TextDecoder('utf-8', { fatal: true }).decode(bytes).split('\n');
  demand(lines.length === 3 && lines[2] === '', 'exact cache report lines');
  const sizes = roots.map((root, index) => {
    const match = /^(0|[1-9][0-9]{0,19})\t([^\n]+)$/.exec(lines[index]);
    demand(match !== null && match[2] === root, 'cache report root/order');
    return BigInt(match[1]);
  });
  demand(sizes[0] + sizes[1] <= MAX_CACHE_BYTES, 'combined Cargo/secondary caches exceed20GiB');
  return sizes.map((bytes, index) => ({ root: roots[index], logical_bytes: bytes.toString() }));
}
function measurement(value, specification, label, direct = false) {
  exact(value, ['requested', 'resolved', 'bytes', 'sha256'], label);
  demand(value.requested === specification.requested, `${label}: roster path/order`);
  absolute(value.requested, label); absolute(value.resolved, label);
  if (direct) demand(value.resolved === value.requested, `${label}: redirected generated file`);
  natural(value.bytes, specification.cap, label, 1); digest(value.sha256, label);
}
function decimal(value, minimum, maximum, label) {
  demand(typeof value === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(value), `${label}: decimal`);
  const parsed = BigInt(value); demand(parsed >= minimum && parsed <= maximum, `${label}: bound`);
}
const STAGES = ['git-head', 'git-status', 'cache-before-configure', 'configure', 'cache-after-configure',
  'cache-before-build', 'build', 'cache-after-build', 'cache-before-native', 'native-test', 'cache-after-native'];
const SCOPE = 'synthetic native program/encoding/resource observation only';
const LIBRARY_SCOPE = 'selected generated static link inputs; not complete compiler, system-library or runtime closure';

// Structural consistency only. Consumers must remeasure each declared input,
// link command, selected library and artifact, and parse the exact retained report.
export function validateProgramBuildReceipt(receipt, { repo, output }) {
  exact(receipt, ['schema', 'status', 'scope', 'source_produced', 'production_exact_program_admission',
    'protected_finalizer_admission', 'hardware_executed', 'runtime_closure_attestation', 'policy_or_source_gate_changes',
    'environment', 'limits', 'stages', 'inputs', 'link_inputs', 'artifacts', 'observation'], 'program build receipt');
  demand(receipt.schema === 'fe2o3-ordered-program-engineering-receipt-v1' && receipt.status === 'passed'
    && receipt.scope === SCOPE && receipt.runtime_closure_attestation === 'unavailable', 'build receipt schema/scope');
  for (const key of ['source_produced', 'production_exact_program_admission', 'protected_finalizer_admission',
    'hardware_executed', 'policy_or_source_gate_changes']) demand(receipt[key] === false, 'build authority');
  const environment = receipt.environment;
  exact(environment, ['platform', 'arch', 'node', 'os_release', 'compiler_repo', 'output', 'compiler_head',
    'compiler_worktree_dirty', 'cache_roots', 'child_environment', 'filesystem_restriction'], 'build environment');
  demand(environment.compiler_repo === repo && environment.output === output && /^[0-9a-f]{40}$/.test(environment.compiler_head)
    && typeof environment.compiler_worktree_dirty === 'boolean', 'build repo/head');
  for (const key of ['platform', 'arch', 'node', 'os_release']) demand(typeof environment[key] === 'string'
    && environment[key].length > 0 && environment[key].length <= 128 && !/[\x00-\x1f\x7f]/.test(environment[key]), 'environment bound');
  dense(environment.cache_roots, 2, 'cache roots');
  parseCacheObservation(Buffer.from(environment.cache_roots.map(root => `0\t${root}\n`).join('')), environment.cache_roots);
  const filesystem = environment.filesystem_restriction;
  exact(filesystem, ['policy', 'compiler_repo_device', 'output_parent_device'], 'filesystem restriction');
  const expectedFilesystem = validateProgramOutputLayout({ repo, output, cacheRoots: environment.cache_roots,
    repoDevice: filesystem.compiler_repo_device, outputParentDevice: filesystem.output_parent_device });
  demand(filesystem.policy === expectedFilesystem.policy, 'filesystem restriction policy');
  const env = environment.child_environment;
  exact(env, ['PATH', 'LANG', 'LC_ALL', 'HOME', 'TMPDIR'], 'child environment');
  demand(env.PATH === '/usr/local/bin:/usr/bin:/bin' && env.LANG === 'C' && env.LC_ALL === 'C'
    && env.TMPDIR === path.join(output, 'tmp'), 'child environment values'); absolute(env.HOME, 'child home');
  exact(receipt.limits, ['jobs', 'minimum_free_bytes', 'minimum_available_ram_bytes', 'maximum_combined_cache_bytes',
    'configure_ms', 'build_ms', 'test_ms', 'test_stdout_bytes', 'disk_reserve_directory'], 'build limits');
  demand(receipt.limits.jobs === 2 && receipt.limits.minimum_free_bytes === MIN_FREE_BYTES.toString()
    && receipt.limits.minimum_available_ram_bytes === MIN_RAM_BYTES.toString()
    && receipt.limits.maximum_combined_cache_bytes === MAX_CACHE_BYTES.toString()
    && receipt.limits.configure_ms === 120000 && receipt.limits.build_ms === 900000
    && receipt.limits.test_ms === 120000 && receipt.limits.test_stdout_bytes === MAX_REPORT
    && receipt.limits.disk_reserve_directory === repo, 'fixed build limits');
  dense(receipt.stages, STAGES.length, 'build stages');
  const configure = { executable: receipt.stages[3].executable, args: receipt.stages[3].args };
  const roster = programBuildMeasurementRoster({ repo, output, configure });
  const commands = new Map([
    ['git-head', ['/usr/bin/git', ['rev-parse', 'HEAD']]],
    ['git-status', ['/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal']]],
    ['configure', [configure.executable, configure.args]],
    ['build', [configure.executable, ['--build', path.join(output, 'build'), '--target', TEST_TARGET,
      'fe2o3-llvm-link-worker', '--parallel', '2']]],
    ['native-test', [path.join(output, 'build', TEST_TARGET), []]],
  ]);
  for (const [index, stage] of receipt.stages.entries()) {
    exact(stage, ['stage', 'executable', 'args', 'code', 'signal', 'reason', 'elapsed_ms', 'free_bytes_before',
      'ram_bytes_before', 'ram_bytes_after', 'stdout_sha256', 'stdout_bytes', 'stderr_sha256', 'stderr_bytes', 'cache_observation'], 'build stage');
    demand(stage.stage === STAGES[index] && stage.code === 0 && stage.signal === null && stage.reason === null, 'stage order/success');
    const expected = commands.get(stage.stage) ?? ['/usr/bin/du', ['-sb', '--', ...environment.cache_roots]];
    demand(stage.executable === expected[0], 'stage executable'); same(stage.args, expected[1], 'stage arguments');
    natural(stage.elapsed_ms, 1000000, 'stage duration');
    decimal(stage.free_bytes_before, MIN_FREE_BYTES, 99999999999999999999n, 'stage free bytes');
    for (const key of ['ram_bytes_before', 'ram_bytes_after']) decimal(stage[key], MIN_RAM_BYTES, 99999999999999999999n, 'stage RAM');
    for (const stream of ['stdout', 'stderr']) { digest(stage[`${stream}_sha256`], 'stage stream'); natural(stage[`${stream}_bytes`], MiB, 'stage stream'); }
    if (stage.stage.startsWith('cache-')) {
      dense(stage.cache_observation, 2, 'cache observations');
      const encoded = stage.cache_observation.map(item => {
        exact(item, ['root', 'logical_bytes'], 'cache observation'); return `${item.logical_bytes}\t${item.root}\n`;
      }).join('');
      parseCacheObservation(Buffer.from(encoded), environment.cache_roots);
      demand(stage.stderr_bytes === 0 && stage.stdout_bytes === Buffer.byteLength(encoded)
        && stage.stdout_sha256 === sha256(Buffer.from(encoded)), 'cache stream/observation binding');
    } else demand(stage.cache_observation === null, 'noncache observation');
  }
  for (const kind of ['inputs', 'artifacts']) {
    dense(receipt[kind], roster[kind].length, kind);
    receipt[kind].forEach((item, index) => measurement(item, roster[kind][index], kind, kind === 'artifacts'));
  }
  const links = receipt.link_inputs;
  exact(links, ['scope', 'commands', 'libraries'], 'link inputs'); demand(links.scope === LIBRARY_SCOPE, 'link scope');
  dense(links.commands, 2, 'link command measurements');
  const llvm = path.dirname(path.dirname(path.dirname(configure.args[6].slice('-DLLVM_DIR='.length))));
  const cxx = configure.args[16].slice('-DCMAKE_CXX_COMPILER='.length);
  const zstdLibrary = configure.args[14].slice('-Dzstd_LIBRARY='.length);
  const expectedLibraries = new Set();
  links.commands.forEach((item, index) => {
    exact(item, ['requested', 'resolved', 'bytes', 'sha256', 'text'], 'link command');
    const { text, ...measured } = item;
    const target = [TEST_TARGET, 'fe2o3-llvm-link-worker'][index];
    measurement(measured, { requested: path.join(output, 'build/CMakeFiles', `${target}.dir/link.txt`), cap: MAX_REPORT }, 'link command', true);
    demand(typeof text === 'string' && Buffer.byteLength(text) === item.bytes && sha256(Buffer.from(text)) === item.sha256,
      'link command measured bytes');
    for (const file of parseProgramLinkInputs(Buffer.from(text), { target, llvm, cxx, zstdLibrary })) expectedLibraries.add(file);
  });
  demand(expectedLibraries.size <= 128, 'combined library count');
  const orderedLibraries = [...expectedLibraries].sort();
  dense(links.libraries, orderedLibraries.length, 'selected library roster');
  links.libraries.forEach((item, index) => measurement(item,
    { requested: orderedLibraries[index], cap: 512 * MiB }, 'selected library'));
  demand(links.libraries.reduce((sum, item) => sum + item.bytes, 0) <= 2 * 1024 * MiB, 'combined library bytes cap');
  exact(receipt.observation, ['sha256', 'bytes', 'worker_build_claim', 'positive_cases', 'program_sites', 'parser_controls', 'native_controls'], 'observation receipt');
  digest(receipt.observation.sha256, 'observation'); natural(receipt.observation.bytes, MAX_REPORT, 'observation bytes', 1);
  demand(CLAIM.test(receipt.observation.worker_build_claim) && receipt.observation.positive_cases === 12
    && receipt.observation.program_sites === 80, 'observation coverage');
  same(receipt.observation.parser_controls, [13, 11], 'parser coverage'); same(receipt.observation.native_controls, [18, 3, 1], 'native coverage');
  const native = receipt.stages[9];
  demand(native.stdout_sha256 === receipt.observation.sha256 && native.stdout_bytes === receipt.observation.bytes
    && native.stderr_bytes === 0, 'native report/stage binding');
  return roster;
}

function availableRam() {
  const text = fs.readFileSync('/proc/meminfo', 'utf8');
  demand(text.length <= MAX_REPORT, 'kernel meminfo cap');
  const matches = [...text.matchAll(/^MemAvailable:\s+([0-9]+) kB$/gm)];
  demand(matches.length === 1, 'unique MemAvailable');
  const bytes = BigInt(matches[0][1]) * 1024n;
  demand(bytes >= MIN_RAM_BYTES, 'available RAM below64GiB'); return bytes.toString();
}
function writeNew(file, bytes) { fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 }); }
function jsonNew(file, value) {
  const bytes = Buffer.from(`${JSON.stringify(value, null, 2)}\n`);
  demand(bytes.length <= 256 * 1024, 'build receipt byte cap'); writeNew(file, bytes);
}

export async function runProgramPrototype(options) {
  const repo = fs.realpathSync(absolute(options.repo, 'compiler repo'));
  const parent = fs.realpathSync(path.dirname(absolute(options.output, 'output')));
  const output = path.join(parent, path.basename(options.output));
  demand(!inside(repo, output) && !inside(output, repo) && output !== os.homedir()
    && path.dirname(output) !== output, 'fresh output must be outside checkout and nonbroad');
  const llvm = fs.realpathSync(absolute(options.llvm, 'LLVM root'));
  const build = path.join(output, 'build'), source = path.join(repo, 'tools/fe2o3-llvm-link-worker');
  const cacheRoots = options.cacheRoots.map(root => fs.realpathSync(absolute(root, 'cache root')));
  parseCacheObservation(Buffer.from(cacheRoots.map(root => `0\t${root}\n`).join('')), cacheRoots);
  const filesystem = validateProgramOutputLayout({ repo, output, cacheRoots,
    repoDevice: String(fs.statSync(repo, { bigint: true }).dev), outputParentDevice: String(fs.statSync(parent, { bigint: true }).dev) });
  requireDiskReserve(repo); requireDiskReserve(parent); availableRam();
  fs.mkdirSync(output, { mode: 0o700 });
  for (const name of ['build', 'logs', 'tmp', 'no-device-libraries']) fs.mkdirSync(path.join(output, name), { mode: 0o700 });
  const env = { PATH: '/usr/local/bin:/usr/bin:/bin', LANG: 'C', LC_ALL: 'C', HOME: os.homedir(), TMPDIR: path.join(output, 'tmp') };
  const receipt = { schema: 'fe2o3-ordered-program-engineering-receipt-v1', status: 'failed', scope: SCOPE,
    source_produced: false, production_exact_program_admission: false, protected_finalizer_admission: false,
    hardware_executed: false, runtime_closure_attestation: 'unavailable', policy_or_source_gate_changes: false,
    environment: { platform: process.platform, arch: process.arch, node: process.version, os_release: os.release(),
      compiler_repo: repo, output, cache_roots: cacheRoots, child_environment: env, filesystem_restriction: filesystem },
    limits: { jobs: 2, minimum_free_bytes: MIN_FREE_BYTES.toString(), minimum_available_ram_bytes: MIN_RAM_BYTES.toString(),
      maximum_combined_cache_bytes: MAX_CACHE_BYTES.toString(), configure_ms: 120000, build_ms: 900000,
      test_ms: 120000, test_stdout_bytes: MAX_REPORT, disk_reserve_directory: repo }, stages: [] };
  let activeStage = 'initialize';
  try {
    const llvmDir = path.join(llvm, 'lib/cmake/llvm'), lldDir = path.join(llvm, 'lib/cmake/lld');
    const buildId = fs.realpathSync(options.buildIdFile), zstdInclude = fs.realpathSync(options.zstdInclude), zstdLibrary = fs.realpathSync(options.zstdLibrary);
    demand(new TextDecoder('utf-8', { fatal: true }).decode(readRegular(buildId, 256)).trim() === LLVM_BUILD_ID, 'LLVM package claim mismatch');
    demand(readRegular(path.join(llvmDir, 'LLVMConfig.cmake'), MiB).toString('utf8').includes('set(LLVM_PACKAGE_VERSION 22.0.0git)'), 'LLVM package version');
    const configure = { executable: absolute(options.cmake, 'cmake'), args: ['-S', source, '-B', build, '-G', 'Unix Makefiles',
      `-DLLVM_DIR=${llvmDir}`, `-DLLD_DIR=${lldDir}`, `-DFE2O3_PINNED_LLVM_VERSION=${LLVM_VERSION}`,
      `-DFE2O3_EXPECTED_LLVM_BUILD_ID=${LLVM_BUILD_ID}`, `-DFE2O3_LLVM_BUILD_ID_FILE=${buildId}`,
      `-DFE2O3_GFX942_DEVICE_LIB_DIR=${path.join(output, 'no-device-libraries')}`,
      `-DFE2O3_GFX950_DEVICE_LIB_DIR=${path.join(output, 'no-device-libraries')}`,
      `-Dzstd_INCLUDE_DIR=${zstdInclude}`, `-Dzstd_LIBRARY=${zstdLibrary}`, '-DCMAKE_BUILD_TYPE=Release',
      `-DCMAKE_CXX_COMPILER=${absolute(options.cxx, 'C++ compiler')}`, '-DBUILD_TESTING=ON'] };
    const roster = programBuildMeasurementRoster({ repo, output, configure });
    receipt.inputs = roster.inputs.map(item => measureNativeBuildInput(item.requested, item.cap));
    async function run(stage, executable, args, timeoutMs, stdoutCap = MiB, stderrCap = MiB) {
      demand(receipt.stages.length < STAGES.length && stage === STAGES[receipt.stages.length], 'fixed stage order');
      activeStage = stage;
      demand(String(fs.statSync(repo, { bigint: true }).dev) === filesystem.compiler_repo_device
        && String(fs.statSync(output, { bigint: true }).dev) === filesystem.output_parent_device, 'observed build filesystem changed');
      const free = requireDiskReserve(repo); requireDiskReserve(output);
      const ram = availableRam();
      const result = await runBoundedCommand({ executable, args, cwd: repo, env, timeoutMs, stdoutCap, stderrCap, diskDirectory: repo });
      for (const stream of ['stdout', 'stderr']) writeNew(path.join(output, 'logs', `${stage}.${stream}`), result[stream]);
      const record = { stage, executable, args, code: result.code, signal: result.signal, reason: result.reason,
        elapsed_ms: result.elapsed_ms, free_bytes_before: free.toString(), ram_bytes_before: ram, ram_bytes_after: null,
        stdout_sha256: sha256(result.stdout), stdout_bytes: result.stdout.length, stderr_sha256: sha256(result.stderr),
        stderr_bytes: result.stderr.length, cache_observation: null };
      receipt.stages.push(record);
      demand(result.code === 0 && result.reason === null && result.signal === null, `${stage}: ${result.reason ?? result.signal ?? result.code}`);
      demand(String(fs.statSync(repo, { bigint: true }).dev) === filesystem.compiler_repo_device
        && String(fs.statSync(output, { bigint: true }).dev) === filesystem.output_parent_device, 'observed build filesystem changed');
      requireDiskReserve(repo); requireDiskReserve(output); record.ram_bytes_after = availableRam();
      if (stage.startsWith('cache-')) {
        demand(result.stderr.length === 0, 'cache measurement stderr');
        record.cache_observation = parseCacheObservation(result.stdout, cacheRoots);
      }
      return result;
    }
    async function cache(stage) {
      demand(options.cacheRoots.every((root, index) => fs.realpathSync(root) === cacheRoots[index]), 'cache root binding changed');
      return run(stage, '/usr/bin/du', ['-sb', '--', ...cacheRoots], 30000, 16 * 1024, 4096);
    }
    const head = await run('git-head', '/usr/bin/git', ['rev-parse', 'HEAD'], 10000, 256, 4096);
    receipt.environment.compiler_head = head.stdout.toString('utf8').trim();
    demand(/^[0-9a-f]{40}$/.test(receipt.environment.compiler_head) && head.stderr.length === 0, 'git HEAD observation');
    const status = await run('git-status', '/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal'], 10000, MAX_REPORT, 4096);
    demand(status.stderr.length === 0, 'git status stderr'); receipt.environment.compiler_worktree_dirty = status.stdout.length !== 0;
    await cache('cache-before-configure');
    await run('configure', configure.executable, configure.args, 120000, 256 * 1024, 256 * 1024);
    await cache('cache-after-configure');
    activeStage = 'capture-generated-link-inputs';
    const libraries = new Set(), commands = [];
    for (const target of [TEST_TARGET, 'fe2o3-llvm-link-worker']) {
      const file = path.join(build, 'CMakeFiles', `${target}.dir/link.txt`);
      const bytes = readRegular(file, MAX_REPORT);
      const measured = measureNativeBuildInput(file, MAX_REPORT);
      demand(measured.bytes === bytes.length && measured.sha256 === sha256(bytes), 'link command changed during capture');
      commands.push({ ...measured, text: new TextDecoder('utf-8', { fatal: true }).decode(bytes) });
      for (const library of parseProgramLinkInputs(bytes, { target, llvm, cxx: options.cxx, zstdLibrary })) libraries.add(library);
    }
    demand(libraries.size <= 128, 'combined library roster cap');
    receipt.link_inputs = { scope: LIBRARY_SCOPE, commands,
      libraries: [...libraries].sort().map(file => measureNativeBuildInput(file, 512 * MiB)) };
    demand(receipt.link_inputs.libraries.reduce((sum, item) => sum + item.bytes, 0) <= 2 * 1024 * MiB, 'combined library bytes cap');
    await cache('cache-before-build');
    await run('build', configure.executable, ['--build', build, '--target', TEST_TARGET,
      'fe2o3-llvm-link-worker', '--parallel', '2'], 900000);
    await cache('cache-after-build');
    receipt.artifacts = roster.artifacts.map(item => measureNativeBuildInput(item.requested, item.cap));
    const claim = readRegular(path.join(build, 'fe2o3-worker-build-id.txt'), 256).toString('utf8').trim();
    demand(CLAIM.test(claim) && readRegular(path.join(build, 'fe2o3-llvm-build-id.txt'), 256).toString('utf8').trim() === LLVM_BUILD_ID,
      'configured build claims');
    await cache('cache-before-native');
    const result = await run('native-test', path.join(build, TEST_TARGET), [], 120000, MAX_REPORT, 128 * 1024);
    demand(result.stderr.length === 0, 'native test stderr');
    const observation = parseProgramObservation(result.stdout, claim);
    await cache('cache-after-native');
    for (const items of [receipt.inputs, commands, receipt.link_inputs.libraries, receipt.artifacts]) requireNativeBuildInputsUnchanged(items);
    writeNew(path.join(output, 'observation.json'), result.stdout);
    receipt.observation = { sha256: sha256(result.stdout), bytes: result.stdout.length, worker_build_claim: claim,
      positive_cases: observation.positive_cases.length, program_sites: observation.positive_cases.reduce((sum, entry) => sum + entry.program.length, 0),
      parser_controls: [13, 11], native_controls: [18, 3, 1] };
    receipt.status = 'passed'; validateProgramBuildReceipt(receipt, { repo, output });
    jsonNew(path.join(output, 'receipt.json'), receipt);
    return receipt;
  } catch (error) {
    receipt.status = 'failed'; receipt.failure = { stage: activeStage, message: String(error?.message ?? error).slice(0, 4096) };
    try { jsonNew(path.join(output, 'failure.json'), receipt); } catch { /* Original failure stays primary; retained stage logs remain. */ }
    throw error;
  }
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  if (process.argv.length === 3 && process.argv[2] === '--help') {
    console.log('Usage: node scripts/ordered-program-worker-prototype.mjs --llvm-root ABS --llvm-build-id-file ABS --zstd-include-dir ABS --zstd-library ABS --output NEW_ABS --cargo-cache-root ABS --secondary-cache-root ABS [--compiler-repo ABS] [--cmake ABS] [--cxx ABS]\nSynthetic native test only; LLVM22/ROCm7.2.1 static package, Release/jobs2,40GiB disk,64GiB RAM,20GiB combined caches; no GPU/production authority.');
  } else {
    Promise.resolve().then(() => runProgramPrototype(parseArguments(process.argv.slice(2)))).then(receipt => {
      console.log(JSON.stringify({ status: receipt.status, output: receipt.environment.output, scope: receipt.scope,
        observation_sha256: receipt.observation.sha256, hardware_executed: false }));
    }).catch(error => { console.error(String(error?.message ?? error).slice(0, 4096)); process.exitCode = 1; });
  }
}
