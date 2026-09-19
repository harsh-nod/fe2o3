#!/usr/bin/env node
// Reproducible native-test experiment, not a source/compiler authority path.
// No Cargo, GPU, protected finalizer, provider library, or existing build reuse.
import fs from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import crypto from 'node:crypto';
import { spawn } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';

export const LLVM_VERSION = '22.0.0git';
export const LLVM_BUILD_ID = 'rocm7.2.1-packages-sha256:eb02c62693d6697017195f0abf5ebcf7e58f60e4d2acf8356de2e944bceec540';
export const TEST_TARGET = 'fe2o3-worker-ordered-inline-region-prototype-tests';
export const MIN_FREE_BYTES = 40n * 1024n ** 3n;
const HERE = path.dirname(fileURLToPath(import.meta.url));
const MAX_REPORT = 64 * 1024;
const PIN = /^[0-9a-f]{64}$/;
const WORKER_CLAIM = /^fe2o3-worker-v1-sha256-[0-9a-f]{64}$/;
const CLAIM_FALSE = ['source_produced', 'production_exact_region_admission',
  'protected_finalizer_admission', 'hardware_executed',
  'whole_kernel_order_or_byte_stability_claim', 'physical_register_allocation_or_lifetime_proof'];
const ROOT_KEYS = ['schema', 'authority', ...CLAIM_FALSE, 'target', 'wave_width',
  'workgroup_size', 'code_object_version', 'llvm_build_claim', 'worker_build_claim',
  'implicit_exec_reads', 'boundary_operations', 'positive_cases', 'closed_contract_negatives',
  'decoded_observation_negatives', 'actual_payload_mutation_negatives',
  'actual_e64_encoding_rejected_by_matcher', 'module_only_export_rejected_by_worker'];
const CASE_KEYS = ['optimization', 'result_used', 'llvm_text_sha256', 'llvm_text_bytes',
  'hsaco_sha256', 'hsaco_bytes', 'descriptor_sha256', 'entry_file_offset', 'entry_code_bytes',
  'static_instruction_count', 'post_link_inspection_diagnostics', 'region',
  'boundary_register_site_count', 'boundary_v_mov_b32_count', 'boundary_sites',
  'boundary_sites_truncated'];
const SITE_KEYS = ['file_offset', 'opcode', 'bytes_hex', 'mc_flags', 'register_operands',
  'implicit_reads', 'implicit_writes'];
const EXPECTED_UNIT = [
  { opcode: 'V_XOR_B32_e32_vi', bytes: '2247402a', registers: ['VGPR32', 'VGPR34', 'VGPR35'] },
  { opcode: 'V_ADD_U32_e32_gfx9', bytes: '20494268', registers: ['VGPR33', 'VGPR32', 'VGPR36'] },
];
const DIAGNOSTICS = [
  'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
  'post_link.check=exports status=ok symbols=[ordered_region_fixture,ordered_region_fixture.kd]',
  'post_link.check=unresolved status=ok symbols=[]',
  'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-',
  'post_link.kernel name=ordered_region_fixture symbol=ordered_region_fixture.kd kernarg_size=280 group_size=0 private_size=0 kernarg_align=8 wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=[64,1,1]',
];
export const WORKER_FILES = Object.freeze(['CMakeLists.txt', 'include/WorkerProtocol.h', 'include/WorkerPipeline.h',
  'include/WorkerDeviceLibraryPolicy.h', 'include/WorkerBuildConfig.h.in',
  'include/WorkerLldPolicy.h', 'include/WorkerMachineEffect.h', 'src/WorkerProtocol.cpp',
  'src/WorkerPipeline.cpp', 'src/WorkerDeviceLibraryPolicy.cpp', 'src/WorkerMachineEffect.cpp',
  'src/main.cpp', 'tests/OrderedInlineRegionPrototypeTests.cpp',
  'tests/OrderedInlineRegionSourceObservation.inc']);

function demand(value, reason) { if (!value) throw new Error(reason); }
function exactObject(value, keys, label) {
  demand(value !== null && typeof value === 'object' && !Array.isArray(value), `${label}: object required`);
  const actual = Object.keys(value).sort();
  const expected = [...keys].sort();
  demand(actual.length === expected.length && actual.every((key, i) => key === expected[i]), `${label}: unknown/missing fields`);
}
function natural(value, max, label, min = 0) {
  demand(Number.isSafeInteger(value) && value >= min && value <= max, `${label}: bounded safe integer required`);
}
function digest(value, label) {
  demand(typeof value === 'string' && PIN.test(value) && value !== '0'.repeat(64), `${label}: nonzero lowercase SHA256 required`);
}
function sameArray(actual, expected) {
  return Array.isArray(actual) && actual.length === expected.length && actual.every((value, i) => value === expected[i]);
}
function sha256(bytes) { return crypto.createHash('sha256').update(bytes).digest('hex'); }

function validateSite(site, entry, label) {
  exactObject(site, SITE_KEYS, label);
  natural(site.file_offset, entry.hsaco_bytes, `${label}.file_offset`);
  demand(typeof site.opcode === 'string' && /^[A-Za-z][A-Za-z0-9_]{0,127}$/.test(site.opcode), `${label}: opcode`);
  demand(typeof site.bytes_hex === 'string' && /^(?:[0-9a-f]{2}){1,16}$/.test(site.bytes_hex), `${label}: instruction bytes`);
  const size = site.bytes_hex.length / 2;
  demand(site.file_offset >= entry.entry_file_offset &&
    site.file_offset + size <= entry.entry_file_offset + entry.entry_code_bytes, `${label}: outside entry code`);
  natural(site.mc_flags, 63, `${label}.mc_flags`);
  for (const field of ['register_operands', 'implicit_reads', 'implicit_writes']) {
    const values = site[field];
    demand(Array.isArray(values) && values.length <= 64 && values.every(value =>
      typeof value === 'string' && /^[A-Za-z][A-Za-z0-9_]{0,127}$/.test(value)), `${label}: register list`);
  }
}

// Checks a bounded observation, not source provenance or protected authority.
// The native test independently checks actual returned HSACO bytes. This JS
// checker checks its report structure and fixed reviewed diagnostic contract.
export function validateObservation(value, expectedWorkerClaim) {
  demand(typeof expectedWorkerClaim === 'string' && WORKER_CLAIM.test(expectedWorkerClaim), 'expected worker build claim is malformed');
  exactObject(value, ROOT_KEYS, 'report');
  demand(value.schema === 'fe2o3-ordered-inline-unit-worker-prototype-v1' &&
    value.authority === 'unauthenticated-native-test-fixture', 'wrong native observation schema/scope');
  for (const field of CLAIM_FALSE) demand(value[field] === false, `unsupported authority claim: ${field}`);
  demand(value.target === 'gfx942:xnack-' && value.wave_width === 64 && value.workgroup_size === 64 &&
    value.code_object_version === 6, 'wrong target/wave/workgroup/code object');
  demand(value.llvm_build_claim === LLVM_BUILD_ID && value.worker_build_claim === expectedWorkerClaim, 'build claim mismatch');
  demand(value.implicit_exec_reads === 'required; not clobbered' &&
    value.boundary_operations === 'compiler-owned; reported outside the unit', 'implicit/boundary scope changed');
  demand(value.closed_contract_negatives === 12 && value.decoded_observation_negatives === 8 &&
    value.actual_payload_mutation_negatives === 2 &&
    value.actual_e64_encoding_rejected_by_matcher === true && value.module_only_export_rejected_by_worker === true,
  'negative control coverage missing');
  demand(Array.isArray(value.positive_cases) && value.positive_cases.length === 4, 'four native cases required');
  const seen = new Set();
  for (const entry of value.positive_cases) {
    exactObject(entry, CASE_KEYS, 'case');
    demand(['O0', 'O3'].includes(entry.optimization) && typeof entry.result_used === 'boolean', 'invalid case selector');
    const key = `${entry.optimization}/${entry.result_used}`;
    demand(!seen.has(key), 'duplicate native case');
    seen.add(key);
    for (const field of ['llvm_text_sha256', 'hsaco_sha256', 'descriptor_sha256']) digest(entry[field], field);
    natural(entry.llvm_text_bytes, MAX_REPORT, 'llvm_text_bytes', 1);
    natural(entry.hsaco_bytes, 1024 * 1024, 'hsaco_bytes', 1);
    natural(entry.entry_file_offset, entry.hsaco_bytes, 'entry_file_offset');
    natural(entry.entry_code_bytes, entry.hsaco_bytes - entry.entry_file_offset, 'entry_code_bytes', 8);
    natural(entry.static_instruction_count, 512, 'static_instruction_count', 2);
    demand(sameArray(entry.post_link_inspection_diagnostics, DIAGNOSTICS), 'post-link diagnostic facts changed');
    demand(Array.isArray(entry.region) && entry.region.length === 2, 'two exact instructions required');
    entry.region.forEach((site, index) => {
      validateSite(site, entry, 'unit instruction');
      const expected = EXPECTED_UNIT[index];
      demand(site.opcode === expected.opcode && site.bytes_hex === expected.bytes &&
        sameArray(site.register_operands, expected.registers) && sameArray(site.implicit_reads, ['EXEC']) &&
        sameArray(site.implicit_writes, []) && (site.mc_flags & ~16) === 0, 'exact unit encoding/register/state mismatch');
    });
    demand(entry.region[0].file_offset + 4 === entry.region[1].file_offset, 'unit is not contiguous');
    natural(entry.boundary_register_site_count, entry.static_instruction_count - 2, 'boundary count');
    natural(entry.boundary_v_mov_b32_count, entry.boundary_register_site_count, 'boundary move count');
    demand(Array.isArray(entry.boundary_sites) && entry.boundary_sites.length === Math.min(32, entry.boundary_register_site_count) &&
      entry.boundary_sites_truncated === (entry.boundary_register_site_count > 32), 'boundary truncation/count mismatch');
    let previous = -1;
    let observedMoves = 0;
    for (const site of entry.boundary_sites) {
      validateSite(site, entry, 'boundary instruction');
      demand(site.file_offset > previous && !entry.region.some(region => region.file_offset === site.file_offset) &&
        site.register_operands.some(register => /^VGPR3[2-6]$/.test(register)), 'boundary site overlap/order/footprint mismatch');
      previous = site.file_offset;
      observedMoves += Number(site.opcode.startsWith('V_MOV_B32_'));
    }
    demand(observedMoves <= entry.boundary_v_mov_b32_count &&
      (entry.boundary_sites_truncated || observedMoves === entry.boundary_v_mov_b32_count), 'boundary move count mismatch');
  }
  return value;
}

export function parseObservation(bytes, claim) {
  demand(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= MAX_REPORT, 'native report byte cap');
  const text = new TextDecoder('utf-8', { fatal: true }).decode(bytes);
  return validateObservation(JSON.parse(text), claim);
}

function absolute(value, label) {
  demand(typeof value === 'string' && value.length <= 4096 && path.isAbsolute(value) &&
    !/[\x00-\x1f\x7f]/.test(value), `${label}: bounded absolute path required`);
  return path.resolve(value);
}

function exactPath(value, label) {
  const normalized = absolute(value, label);
  demand(value === normalized, `${label}: noncanonical/rewritten path`);
  return normalized;
}

// One immutable measured-file roster for the builder and its retained-receipt
// consumer. This is the declared experiment input set, NOT runtime closure or
// build authentication. Variable paths come only from the fixed configure stage.
export function nativeBuildMeasurementRoster({ repo, output, configure }) {
  exactPath(repo, 'compiler repo'); exactPath(output, 'native output');
  exactObject(configure, ['executable', 'args'], 'native configure');
  const cmake = exactPath(configure.executable, 'CMake tool'), args = configure.args;
  demand(Array.isArray(args) && args.length === 18 && args.every(arg => typeof arg === 'string' &&
    arg.length > 0 && arg.length <= 4096 && !/[\x00-\x1f\x7f]/.test(arg)), 'native configure: bounded fixed arguments');
  const source = path.join(repo, 'tools/fe2o3-llvm-link-worker'), build = path.join(output, 'build');
  demand(sameArray(args.slice(0, 6), ['-S', source, '-B', build, '-G', 'Unix Makefiles']), 'native configure: source/build/generator mismatch');
  function setting(index, prefix) {
    demand(args[index].startsWith(prefix), `native configure: expected ${prefix}`);
    return exactPath(args[index].slice(prefix.length), `native configure ${prefix}`);
  }
  const llvmDir = setting(6, '-DLLVM_DIR='), lldDir = setting(7, '-DLLD_DIR=');
  demand(path.basename(llvmDir) === 'llvm' && path.basename(path.dirname(llvmDir)) === 'cmake' &&
    lldDir === path.join(path.dirname(llvmDir), 'lld'), 'native configure: LLVM/LLD package directories');
  demand(args[8] === `-DFE2O3_PINNED_LLVM_VERSION=${LLVM_VERSION}` &&
    args[9] === `-DFE2O3_EXPECTED_LLVM_BUILD_ID=${LLVM_BUILD_ID}`, 'native configure: reviewed package claims');
  const buildId = setting(10, '-DFE2O3_LLVM_BUILD_ID_FILE=');
  demand(args[11] === `-DFE2O3_GFX942_DEVICE_LIB_DIR=${path.join(output, 'no-device-libraries')}` &&
    args[12] === `-DFE2O3_GFX950_DEVICE_LIB_DIR=${path.join(output, 'no-device-libraries')}` &&
    args[15] === '-DCMAKE_BUILD_TYPE=Release' && args[17] === '-DBUILD_TESTING=ON', 'native configure: fixed test-only policy');
  const zstdInclude = setting(13, '-Dzstd_INCLUDE_DIR='), zstdLibrary = setting(14, '-Dzstd_LIBRARY=');
  const cxx = setting(16, '-DCMAKE_CXX_COMPILER=');
  const spec = (requested, cap) => Object.freeze({ requested: exactPath(requested, 'roster file'), cap });
  const inputs = Object.freeze([
    ...WORKER_FILES.map(file => spec(path.join(source, file), 2 * 1024 * 1024)),
    spec(fileURLToPath(import.meta.url), 256 * 1024), spec(cmake, 512 * 1024 * 1024), spec(cxx, 512 * 1024 * 1024),
    spec(buildId, 256), spec(path.join(llvmDir, 'LLVMConfig.cmake'), 1024 * 1024),
    spec(path.join(lldDir, 'LLDConfig.cmake'), 1024 * 1024), spec(path.join(zstdInclude, 'zstd.h'), 1024 * 1024),
    spec(zstdLibrary, 4 * 1024 * 1024),
  ]);
  const artifacts = Object.freeze([TEST_TARGET, 'fe2o3-llvm-link-worker'].map(file => spec(path.join(build, file), 512 * 1024 * 1024)));
  demand(new Set(inputs.map(item => item.requested)).size === inputs.length, 'native input roster: duplicate configured paths');
  return Object.freeze({ inputs, artifacts });
}

// Structural/roster consistency only. The consumer must also remeasure EVERY
// required path and compare requested/resolved paths, byte counts and hashes.
export function validateNativeBuildReceipt(receipt, { repo, output }) {
  exactObject(receipt, ['schema', 'status', 'scope', 'source_produced', 'production_exact_region_admission',
    'protected_finalizer_admission', 'hardware_executed', 'runtime_closure_attestation', 'package_identity_kind',
    'policy_or_source_gate_changes', 'environment', 'limits', 'stages', 'inputs', 'artifacts', 'observation'], 'native build receipt');
  demand(receipt.schema === 'fe2o3-ordered-inline-unit-engineering-receipt-v1' && receipt.status === 'passed' &&
    receipt.scope === 'native-test-fixture transport/encoding observation only' && receipt.source_produced === false &&
    receipt.production_exact_region_admission === false && receipt.protected_finalizer_admission === false &&
    receipt.hardware_executed === false && receipt.policy_or_source_gate_changes === false &&
    receipt.runtime_closure_attestation === 'unavailable' &&
    receipt.package_identity_kind === 'existing asserted package build-ID, not runtime closure', 'native build receipt scope');
  const env = receipt.environment;
  exactObject(env, ['platform', 'arch', 'node', 'os_release', 'compiler_repo', 'output', 'compiler_head', 'compiler_worktree_dirty'], 'native build environment');
  demand(env.compiler_repo === repo && env.output === output && typeof env.compiler_head === 'string' &&
    /^[0-9a-f]{40}$/.test(env.compiler_head) && typeof env.compiler_worktree_dirty === 'boolean', 'native build receipt scope/path');
  for (const field of ['platform', 'arch', 'node', 'os_release']) demand(typeof env[field] === 'string' &&
    env[field].length > 0 && env[field].length <= 128 && !/[\x00-\x1f\x7f]/.test(env[field]), 'native environment bound');
  exactObject(receipt.limits, ['jobs', 'minimum_free_bytes', 'configure_ms', 'build_ms', 'test_ms', 'test_stdout_bytes'], 'native build limits');
  demand(receipt.limits.jobs === 2 && receipt.limits.minimum_free_bytes === MIN_FREE_BYTES.toString() &&
    receipt.limits.configure_ms === 120000 && receipt.limits.build_ms === 900000 && receipt.limits.test_ms === 120000 &&
    receipt.limits.test_stdout_bytes === MAX_REPORT, 'native build fixed limits');
  demand(Array.isArray(receipt.stages) && receipt.stages.length === 5, 'native build: exact five stages required');
  const names = ['git-head', 'git-status', 'configure', 'build', 'native-test'];
  for (const [index, stage] of receipt.stages.entries()) {
    exactObject(stage, ['stage', 'executable', 'args', 'code', 'signal', 'reason', 'elapsed_ms', 'free_bytes_before',
      'stdout_sha256', 'stdout_bytes', 'stderr_sha256', 'stderr_bytes'], 'native build stage');
    demand(stage.stage === names[index] && stage.code === 0 && stage.signal === null && stage.reason === null, 'native build stage order/success');
    exactPath(stage.executable, 'native stage executable');
    demand(Array.isArray(stage.args) && stage.args.length <= 18 && stage.args.every(arg => typeof arg === 'string' &&
      arg.length > 0 && arg.length <= 4096 && !/[\x00-\x1f\x7f]/.test(arg)), 'native stage arguments bound');
    natural(stage.elapsed_ms, 1_000_000, 'native stage duration');
    demand(typeof stage.free_bytes_before === 'string' && /^(0|[1-9][0-9]{0,19})$/.test(stage.free_bytes_before) &&
      BigInt(stage.free_bytes_before) >= MIN_FREE_BYTES, 'native stage disk reserve');
    for (const stream of ['stdout', 'stderr']) { digest(stage[`${stream}_sha256`], 'native stage output'); natural(stage[`${stream}_bytes`], 1024 * 1024, 'native stage output cap'); }
  }
  const configure = { executable: receipt.stages[2].executable, args: receipt.stages[2].args };
  const roster = nativeBuildMeasurementRoster({ repo, output, configure });
  const commands = [
    ['/usr/bin/git', ['rev-parse', 'HEAD']],
    ['/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal']],
    [configure.executable, configure.args],
    [configure.executable, ['--build', path.join(output, 'build'), '--target', TEST_TARGET, 'fe2o3-llvm-link-worker', '--parallel', '2']],
    [path.join(output, 'build', TEST_TARGET), []],
  ];
  for (const [index, [executable, args]] of commands.entries()) demand(receipt.stages[index].executable === executable &&
    sameArray(receipt.stages[index].args, args), 'native stage command mismatch');
  for (const kind of ['inputs', 'artifacts']) {
    const entries = receipt[kind], expected = roster[kind];
    demand(Array.isArray(entries) && entries.length === expected.length, `native ${kind} roster: exact count required`);
    for (const [index, item] of entries.entries()) {
      exactObject(item, ['requested', 'resolved', 'bytes', 'sha256'], `native ${kind} measurement`);
      exactPath(item.requested, `native ${kind} requested`); exactPath(item.resolved, `native ${kind} resolved`);
      demand(item.requested === expected[index].requested, `native ${kind} roster: missing/duplicate/extra/rewritten path`);
      if (kind === 'artifacts') demand(item.resolved === item.requested, 'native artifacts roster: redirected path');
      natural(item.bytes, expected[index].cap, `native ${kind} measurement byte cap`, 1); digest(item.sha256, `native ${kind} measurement`);
    }
  }
  exactObject(receipt.observation, ['sha256', 'bytes', 'worker_build_claim', 'positive_cases', 'native_test_control_counts'], 'native baseline measurement');
  digest(receipt.observation.sha256, 'native baseline'); natural(receipt.observation.bytes, MAX_REPORT, 'native baseline size', 1);
  demand(typeof receipt.observation.worker_build_claim === 'string' && WORKER_CLAIM.test(receipt.observation.worker_build_claim) &&
    receipt.observation.positive_cases === 4 && sameArray(receipt.observation.native_test_control_counts, [12, 8, 2]), 'native baseline coverage');
  return roster;
}

// NOFOLLOW/NONBLOCK applies to the final canonical path. Callers explicitly
// record both the requested path and its resolved tool/package dependency.
export function readRegular(file, cap) {
  const fd = fs.openSync(file, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd);
    demand(before.isFile() && before.size <= cap, 'regular file byte cap');
    const bytes = Buffer.alloc(before.size);
    let offset = 0;
    while (offset < bytes.length) {
      const count = fs.readSync(fd, bytes, offset, bytes.length - offset, offset);
      demand(count > 0, 'file shrank while reading');
      offset += count;
    }
    const after = fs.fstatSync(fd);
    demand(after.size === before.size && after.mtimeMs === before.mtimeMs && after.ino === before.ino,
      'file changed while reading');
    return bytes;
  } finally { fs.closeSync(fd); }
}

function measure(file, cap = 512 * 1024 * 1024) {
  const requested = absolute(file, 'file');
  const resolved = fs.realpathSync(requested);
  const fd = fs.openSync(resolved, fs.constants.O_RDONLY | fs.constants.O_NOFOLLOW | fs.constants.O_NONBLOCK);
  try {
    const before = fs.fstatSync(fd);
    demand(before.isFile() && before.size > 0 && before.size <= cap, 'measured regular file byte cap');
    const hash = crypto.createHash('sha256');
    const chunk = Buffer.alloc(64 * 1024);
    let offset = 0;
    while (offset < before.size) {
      const count = fs.readSync(fd, chunk, 0, Math.min(chunk.length, before.size - offset), offset);
      demand(count > 0, 'measured file shrank');
      hash.update(chunk.subarray(0, count));
      offset += count;
    }
    const after = fs.fstatSync(fd);
    demand(after.size === before.size && after.mtimeMs === before.mtimeMs && after.ino === before.ino,
      'measured file changed');
    return { requested, resolved, bytes: before.size, sha256: hash.digest('hex') };
  } finally { fs.closeSync(fd); }
}
export { measure as measureNativeBuildInput };

export function requireDiskReserve(directory) {
  const stat = fs.statfsSync(directory, { bigint: true });
  const available = stat.bavail * stat.bsize;
  demand(available >= MIN_FREE_BYTES, 'disk reserve below40GiB');
  return available;
}

// Exported for explicitly mocked child-process controls. This routine conveys
// no compiler success or receipt authority and never invokes a shell.
export function runBoundedCommand({ executable, args = [], cwd, env, timeoutMs,
  stdoutCap, stderrCap, diskDirectory }) {
  demand(Number.isInteger(timeoutMs) && timeoutMs > 0 && timeoutMs <= 900_000, 'process timeout bound');
  demand(Number.isInteger(stdoutCap) && stdoutCap > 0 && stdoutCap <= 1024 * 1024 &&
    Number.isInteger(stderrCap) && stderrCap > 0 && stderrCap <= 1024 * 1024, 'process output bounds');
  return new Promise(resolve => {
    const start = performance.now();
    let reason = null;
    let stdoutBytes = 0, stderrBytes = 0;
    const stdout = [], stderr = [];
    const child = spawn(executable, args, { cwd, env, detached: true, stdio: ['ignore', 'pipe', 'pipe'] });
    const stop = message => {
      reason ??= message;
      if (child.pid) {
        try { process.kill(-child.pid, 'SIGKILL'); } catch (error) {
          if (error.code !== 'ESRCH') reason = `kill:${error.code}`;
        }
      }
    };
    const timer = setTimeout(() => stop('timeout'), timeoutMs);
    const monitor = diskDirectory ? setInterval(() => {
      try { requireDiskReserve(diskDirectory); } catch { stop('disk_reserve'); }
    }, 1000) : null;
    child.stdout.on('data', bytes => {
      const remaining = stdoutCap - stdoutBytes;
      if (remaining > 0) { stdout.push(bytes.subarray(0, remaining)); stdoutBytes += Math.min(bytes.length, remaining); }
      if (bytes.length > remaining) stop('stdout_cap');
    });
    child.stderr.on('data', bytes => {
      const remaining = stderrCap - stderrBytes;
      if (remaining > 0) { stderr.push(bytes.subarray(0, remaining)); stderrBytes += Math.min(bytes.length, remaining); }
      if (bytes.length > remaining) stop('stderr_cap');
    });
    child.once('error', error => { reason ??= `spawn:${error.code ?? 'error'}`; });
    child.once('close', (code, signal) => {
      clearTimeout(timer);
      if (monitor) clearInterval(monitor);
      resolve({ code, signal, reason, elapsed_ms: Math.ceil(performance.now() - start),
        stdout: Buffer.concat(stdout), stderr: Buffer.concat(stderr) });
    });
  });
}

export function parseArguments(argv) {
  const allowed = new Set(['--llvm-root', '--llvm-build-id-file', '--zstd-include-dir', '--zstd-library',
    '--output', '--compiler-repo', '--cmake', '--cxx']);
  const values = new Map();
  for (let i = 0; i < argv.length; i += 2) {
    demand(allowed.has(argv[i]) && !values.has(argv[i]) && i + 1 < argv.length, 'unknown/duplicate/incomplete option');
    values.set(argv[i], absolute(argv[i + 1], argv[i]));
  }
  for (const required of ['--llvm-root', '--llvm-build-id-file', '--zstd-include-dir', '--zstd-library', '--output'])
    demand(values.has(required), `missing ${required}`);
  return { llvm: values.get('--llvm-root'), buildIdFile: values.get('--llvm-build-id-file'),
    zstdInclude: values.get('--zstd-include-dir'), zstdLibrary: values.get('--zstd-library'),
    output: values.get('--output'), repo: values.get('--compiler-repo') ?? path.resolve(HERE, '..'),
    cmake: values.get('--cmake') ?? '/usr/local/bin/cmake', cxx: values.get('--cxx') ?? '/usr/bin/g++' };
}

function inside(parent, child) {
  const relative = path.relative(parent, child);
  return relative === '' || (!relative.startsWith(`..${path.sep}`) && relative !== '..' && !path.isAbsolute(relative));
}
function freshOutput(options) {
  const repo = fs.realpathSync(options.repo);
  const parent = fs.realpathSync(path.dirname(options.output));
  const output = path.join(parent, path.basename(options.output));
  demand(!inside(repo, output) && output !== os.homedir() && path.dirname(output) !== output,
    'output must be a new task directory outside compiler checkout');
  requireDiskReserve(parent);
  fs.mkdirSync(output, { mode: 0o700 }); // Intentionally no recursive/existing reuse.
  for (const name of ['build', 'logs', 'tmp', 'no-device-libraries']) fs.mkdirSync(path.join(output, name), { mode: 0o700 });
  return { repo, output };
}
function writeNew(file, bytes) { fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 }); }
function unchanged(before) {
  for (const item of before) {
    const after = measure(item.requested, item.bytes);
    demand(item.sha256 === after.sha256 && item.bytes === after.bytes && item.resolved === after.resolved,
      `input changed during experiment: ${item.requested}`);
  }
}
export { unchanged as requireNativeBuildInputsUnchanged };

async function main(argv) {
  if (argv.length === 1 && argv[0] === '--help') {
    console.log('Usage: node scripts/assembly-region-worker-prototype.mjs --llvm-root ABS --llvm-build-id-file ABS --zstd-include-dir ABS --zstd-library ABS --output NEW_ABS [--compiler-repo ABS] [--cmake ABS] [--cxx ABS]\nFixed LLVM22.0.0git/ROCm7.2.1 package claim, Release/jobs2,40GiB reserve; no hardware or production admission.');
    return;
  }
  const options = parseArguments(argv);
  const llvm = fs.realpathSync(options.llvm);
  const llvmConfig = path.join(llvm, 'lib/cmake/llvm/LLVMConfig.cmake');
  const lldConfig = path.join(llvm, 'lib/cmake/lld/LLDConfig.cmake');
  const idBytes = readRegular(fs.realpathSync(options.buildIdFile), 256);
  demand(new TextDecoder('utf-8', { fatal: true }).decode(idBytes).trim() === LLVM_BUILD_ID, 'package build-ID mismatch');
  const configText = readRegular(llvmConfig, 1024 * 1024).toString('utf8');
  demand(configText.includes('set(LLVM_PACKAGE_VERSION 22.0.0git)'), 'LLVM package version is not the reviewed22.0.0git');
  readRegular(lldConfig, 1024 * 1024);
  const { repo, output } = freshOutput(options);
  const source = path.join(repo, 'tools/fe2o3-llvm-link-worker');
  const build = path.join(output, 'build');
  const disabled = path.join(output, 'no-device-libraries');
  const env = { PATH: '/usr/local/bin:/usr/bin:/bin', LANG: 'C', LC_ALL: 'C',
    HOME: os.homedir(), TMPDIR: path.join(output, 'tmp') };
  const records = [];
  const receipt = { schema: 'fe2o3-ordered-inline-unit-engineering-receipt-v1', status: 'failed',
    scope: 'native-test-fixture transport/encoding observation only', source_produced: false,
    production_exact_region_admission: false, protected_finalizer_admission: false, hardware_executed: false,
    runtime_closure_attestation: 'unavailable', package_identity_kind: 'existing asserted package build-ID, not runtime closure',
    policy_or_source_gate_changes: false, environment: { platform: process.platform, arch: process.arch,
      node: process.version, os_release: os.release(), compiler_repo: repo, output },
    limits: { jobs: 2, minimum_free_bytes: MIN_FREE_BYTES.toString(), configure_ms: 120000,
      build_ms: 900000, test_ms: 120000, test_stdout_bytes: MAX_REPORT }, stages: records };
  try {
    const configure = { executable: options.cmake, args: ['-S', source, '-B', build, '-G', 'Unix Makefiles',
      `-DLLVM_DIR=${path.dirname(llvmConfig)}`, `-DLLD_DIR=${path.dirname(lldConfig)}`,
      `-DFE2O3_PINNED_LLVM_VERSION=${LLVM_VERSION}`, `-DFE2O3_EXPECTED_LLVM_BUILD_ID=${LLVM_BUILD_ID}`,
      `-DFE2O3_LLVM_BUILD_ID_FILE=${fs.realpathSync(options.buildIdFile)}`,
      `-DFE2O3_GFX942_DEVICE_LIB_DIR=${disabled}`, `-DFE2O3_GFX950_DEVICE_LIB_DIR=${disabled}`,
      `-Dzstd_INCLUDE_DIR=${fs.realpathSync(options.zstdInclude)}`, `-Dzstd_LIBRARY=${fs.realpathSync(options.zstdLibrary)}`,
      '-DCMAKE_BUILD_TYPE=Release', `-DCMAKE_CXX_COMPILER=${options.cxx}`, '-DBUILD_TESTING=ON'] };
    const roster = nativeBuildMeasurementRoster({ repo, output, configure });
    const inputs = roster.inputs.map(item => measure(item.requested, item.cap));
    receipt.inputs = inputs;
    const run = async (stage, executable, args, timeoutMs, stdoutCap, stderrCap) => {
      const disk = requireDiskReserve(output).toString();
      const result = await runBoundedCommand({ executable, args, cwd: repo, env, timeoutMs,
        stdoutCap, stderrCap, diskDirectory: output });
      for (const stream of ['stdout', 'stderr']) writeNew(path.join(output, 'logs', `${stage}.${stream}`), result[stream]);
      records.push({ stage, executable, args, code: result.code, signal: result.signal, reason: result.reason,
        elapsed_ms: result.elapsed_ms, free_bytes_before: disk, stdout_sha256: sha256(result.stdout),
        stdout_bytes: result.stdout.length, stderr_sha256: sha256(result.stderr), stderr_bytes: result.stderr.length });
      demand(result.code === 0 && result.reason === null && result.signal === null, `${stage} failed: ${result.reason ?? result.signal ?? result.code}`);
      requireDiskReserve(output);
      return result;
    };
    const head = await run('git-head', '/usr/bin/git', ['rev-parse', 'HEAD'], 10000, 256, 4096);
    receipt.environment.compiler_head = head.stdout.toString('utf8').trim();
    demand(/^[0-9a-f]{40}$/.test(receipt.environment.compiler_head), 'git HEAD is malformed');
    const dirty = await run('git-status', '/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal'], 10000, 64 * 1024, 4096);
    receipt.environment.compiler_worktree_dirty = dirty.stdout.length !== 0;
    await run('configure', configure.executable, configure.args, 120000, 256 * 1024, 256 * 1024);
    await run('build', options.cmake, ['--build', build, '--target', TEST_TARGET,
      'fe2o3-llvm-link-worker', '--parallel', '2'], 900000, 1024 * 1024, 1024 * 1024);
    const claim = readRegular(path.join(build, 'fe2o3-worker-build-id.txt'), 256).toString('utf8').trim();
    demand(WORKER_CLAIM.test(claim), 'generated worker claim is malformed');
    demand(readRegular(path.join(build, 'fe2o3-llvm-build-id.txt'), 256).toString('utf8').trim() === LLVM_BUILD_ID,
      'configured LLVM build-ID drifted');
    const artifacts = roster.artifacts.map(item => measure(item.requested, item.cap));
    receipt.artifacts = artifacts;
    const tested = await run('native-test', path.join(build, TEST_TARGET), [], 120000, MAX_REPORT, 128 * 1024);
    const observation = parseObservation(tested.stdout, claim);
    unchanged(inputs);
    unchanged(artifacts);
    writeNew(path.join(output, 'observation.json'), tested.stdout);
    receipt.observation = { sha256: sha256(tested.stdout), bytes: tested.stdout.length,
      worker_build_claim: claim, positive_cases: observation.positive_cases.length,
      native_test_control_counts: [observation.closed_contract_negatives, observation.decoded_observation_negatives,
        observation.actual_payload_mutation_negatives] };
    receipt.status = 'passed';
    validateNativeBuildReceipt(receipt, { repo, output });
  } catch (error) {
    receipt.status = 'failed';
    receipt.failure = String(error.message).slice(0, 4096);
    throw error;
  } finally {
    writeNew(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`);
  }
  console.log(JSON.stringify({ status: 'passed', output, scope: receipt.scope,
    observation_sha256: receipt.observation.sha256 }));
}

if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch(error => { console.error(String(error.message).slice(0, 4096)); process.exitCode = 1; });
}
