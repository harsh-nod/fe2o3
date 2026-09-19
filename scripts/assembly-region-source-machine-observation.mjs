#!/usr/bin/env node
// TEST-ONLY retained callback -> unchanged LLVM -> final-machine observation.
// Hash joins are consistency checks, not producer authentication or admission.
import fs from 'node:fs';
import path from 'node:path';
import crypto from 'node:crypto';
import { fileURLToPath } from 'node:url';
import { isDeepStrictEqual } from 'node:util';
import { LLVM_BUILD_ID, TEST_TARGET, MIN_FREE_BYTES, readRegular, requireDiskReserve,
  runBoundedCommand, parseObservation, validateNativeBuildReceipt, measureNativeBuildInput,
  requireNativeBuildInputsUnchanged } from './assembly-region-worker-prototype.mjs';

const HERE = path.dirname(fileURLToPath(import.meta.url));
const MAX_REPORT = 64 * 1024;
const FEATURES = ['ordered-region-v31', 'ordered-region-unused-v31', 'ordered-region-alias-v31',
  'ordered-region-dynamic-v31', 'ordered-region-divergent-v31', 'ordered-region-wrong-launch-v31'];
const REJECTIONS = ['ordered region physical roles must be distinct v0..v63',
  'ordered region physical role is not an actual MIR constant',
  'ordered region must precede every conditional source edge',
  'ordered region requires an explicit 64x1x1 workgroup'];
const FALSE_CLAIMS = ['production_exact_region_admission', 'protected_finalizer_admission', 'hardware_executed',
  'physical_register_allocation_or_lifetime_proof', 'whole_kernel_order_or_byte_stability_claim'];
const CASE_KEYS = ['optimization', 'result_used', 'llvm_text_sha256', 'llvm_text_bytes', 'hsaco_sha256', 'hsaco_bytes',
  'descriptor_sha256', 'entry_file_offset', 'entry_code_bytes', 'static_instruction_count', 'post_link_inspection_diagnostics',
  'region', 'boundary_register_site_count', 'boundary_v_mov_b32_count', 'boundary_sites', 'boundary_sites_truncated', 'descriptor_resources'];
const SITE_KEYS = ['file_offset', 'opcode', 'bytes_hex', 'mc_flags', 'register_operands', 'implicit_reads', 'implicit_writes'];
const UNIT = [['V_XOR_B32_e32_vi', '2247402a', ['VGPR32', 'VGPR34', 'VGPR35']],
  ['V_ADD_U32_e32_gfx9', '20494268', ['VGPR33', 'VGPR32', 'VGPR36']]];

function demand(value, reason) { if (!value) throw new Error(reason); }
function exact(value, keys, label) {
  demand(value && typeof value === 'object' && !Array.isArray(value) &&
    Object.keys(value).length === keys.length && keys.every(key => Object.hasOwn(value, key)), `${label}: unexpected fields`);
}
function integer(value, minimum, maximum, label) {
  demand(Number.isSafeInteger(value) && value >= minimum && value <= maximum, `${label}: integer bound`);
}
function digest(value, label) {
  demand(typeof value === 'string' && /^[0-9a-f]{64}$/.test(value) && !/^0+$/.test(value), `${label}: SHA256 required`);
}
function sha(bytes) { return crypto.createHash('sha256').update(bytes).digest('hex'); }
function same(a, b, label) { demand(isDeepStrictEqual(a, b), `${label}: mismatch`); }
function json(bytes, cap = 256 * 1024) {
  demand(Buffer.isBuffer(bytes) && bytes.length > 0 && bytes.length <= cap, 'JSON byte bound');
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(bytes));
}
function absolute(value, label) {
  demand(typeof value === 'string' && path.isAbsolute(value) && value.length <= 4096 && !/[\x00-\x1f\x7f]/.test(value), `${label}: absolute path required`);
  return path.resolve(value);
}
function readMeasured(file, cap) {
  const requested = absolute(file, 'input'), resolved = fs.realpathSync(requested), bytes = readRegular(requested, cap);
  demand(bytes.length > 0, 'empty input');
  return { measurement: { requested, resolved, bytes: bytes.length, sha256: sha(bytes) }, bytes };
}
export function measureInput(file, cap = 16 * 1024 * 1024) {
  return readMeasured(file, cap).measurement;
}
export function requireUnchanged(before) {
  for (const item of before) {
    const after = measureInput(item.requested, Math.max(item.bytes, 1));
    demand(after.resolved === item.resolved && after.bytes === item.bytes && after.sha256 === item.sha256,
      `input changed: ${item.requested}`);
  }
}

function validateSite(site, entry) {
  exact(site, SITE_KEYS, 'instruction');
  integer(site.file_offset, entry.entry_file_offset, entry.entry_file_offset + entry.entry_code_bytes, 'instruction offset');
  demand(typeof site.opcode === 'string' && /^[A-Za-z][A-Za-z0-9_]{0,127}$/.test(site.opcode) &&
    typeof site.bytes_hex === 'string' && /^(?:[0-9a-f]{2}){1,16}$/.test(site.bytes_hex), 'instruction encoding');
  demand(site.file_offset + site.bytes_hex.length / 2 <= entry.entry_file_offset + entry.entry_code_bytes, 'instruction outside entry');
  integer(site.mc_flags, 0, 63, 'instruction flags');
  for (const field of ['register_operands', 'implicit_reads', 'implicit_writes']) {
    demand(Array.isArray(site[field]) && site[field].length <= 64 && site[field].every(item =>
      typeof item === 'string' && /^[A-Za-z][A-Za-z0-9_]{0,127}$/.test(item)), 'instruction register list');
  }
}

export function validateMachineObservation(report, expected) {
  exact(report, ['schema', 'authority', 'source_ancestry', ...FALSE_CLAIMS, 'runtime_closure_attestation',
    'target', 'wave_width', 'workgroup_size', 'code_object_version', 'kernel_symbol', 'result_used',
    'llvm_text_sha256', 'llvm_text_bytes', 'llvm_build_claim', 'worker_build_claim', 'cases'], 'machine report');
  demand(report.schema === 'fe2o3-ordered-region-llvm-machine-observation-v1' &&
    report.authority === 'unauthenticated-test-transport' && report.source_ancestry === 'not-established-by-llvm-file' &&
    report.runtime_closure_attestation === 'unavailable', 'machine observation scope');
  for (const field of FALSE_CLAIMS) demand(report[field] === false, `unsupported ${field}`);
  demand(report.target === 'gfx942:xnack-' && report.wave_width === 64 && report.workgroup_size === 64 &&
    report.code_object_version === 6, 'machine target profile');
  demand(typeof report.kernel_symbol === 'string' && /^[A-Za-z_][A-Za-z0-9_]{0,127}$/.test(report.kernel_symbol), 'entry symbol');
  demand(/^fe2o3-worker-v1-sha256-[0-9a-f]{64}$/.test(expected.workerClaim) && report.worker_build_claim === expected.workerClaim &&
    report.llvm_build_claim === LLVM_BUILD_ID, 'machine build claims');
  digest(report.llvm_text_sha256, 'LLVM'); integer(report.llvm_text_bytes, 1, 65536, 'LLVM length');
  demand(report.llvm_text_sha256 === expected.llvmSha256 && report.llvm_text_bytes === expected.llvmBytes &&
    report.result_used === expected.used, 'source LLVM or mode mismatch');
  demand(Array.isArray(report.cases) && report.cases.length === 2, 'exactly O0 and O3 required');
  const seen = new Set();
  for (const entry of report.cases) {
    exact(entry, CASE_KEYS, 'machine case');
    demand(['O0', 'O3'].includes(entry.optimization) && !seen.has(entry.optimization), 'duplicate/missing optimization');
    seen.add(entry.optimization);
    demand(entry.result_used === expected.used && entry.llvm_text_sha256 === expected.llvmSha256 &&
      entry.llvm_text_bytes === expected.llvmBytes, 'machine case source binding');
    digest(entry.hsaco_sha256, 'HSACO'); digest(entry.descriptor_sha256, 'descriptor');
    integer(entry.hsaco_bytes, 1, 1024 * 1024, 'HSACO size');
    integer(entry.entry_file_offset, 0, entry.hsaco_bytes, 'entry offset');
    integer(entry.entry_code_bytes, 8, entry.hsaco_bytes - entry.entry_file_offset, 'entry size');
    integer(entry.static_instruction_count, 2, 512, 'decoded instruction count');
    const diagnostics = entry.post_link_inspection_diagnostics;
    demand(Array.isArray(diagnostics) && diagnostics.length === 5 && diagnostics.every(value => typeof value === 'string' && value.length <= 4096), 'inspection diagnostics');
    same(diagnostics.slice(0, 4), [
      'post_link.check=target status=ok arch=gfx942 code_object_version=6 e_flags=0x64c',
      `post_link.check=exports status=ok symbols=[${report.kernel_symbol},${report.kernel_symbol}.kd]`,
      'post_link.check=unresolved status=ok symbols=[]',
      'post_link.check=metadata status=ok kernels=1 target=amdgcn-amd-amdhsa--gfx942%3Axnack-',
    ], 'inspection facts');
    demand(new RegExp(`^post_link.kernel name=${report.kernel_symbol} symbol=${report.kernel_symbol}\\.kd kernarg_size=[0-9]{1,10} group_size=[0-9]{1,10} private_size=[0-9]{1,10} kernarg_align=[0-9]{1,10} wavefront_size=64 max_workgroup_size=64 reqd_workgroup_size=\\[64,1,1\\]$`).test(diagnostics[4]), 'kernel inspection');
    demand(Array.isArray(entry.region) && entry.region.length === 2, 'exact instruction pair');
    entry.region.forEach((site, index) => {
      validateSite(site, entry);
      const [opcode, bytes, registers] = UNIT[index];
      demand(site.opcode === opcode && site.bytes_hex === bytes && (site.mc_flags & ~16) === 0, 'ordered e32 opcode/bytes/flags');
      same(site.register_operands, registers, 'physical registers');
      same(site.implicit_reads, ['EXEC'], 'implicit reads'); same(site.implicit_writes, [], 'implicit writes');
    });
    demand(entry.region[0].file_offset + 4 === entry.region[1].file_offset, 'instruction adjacency');
    const resource = entry.descriptor_resources;
    exact(resource, ['descriptor_file_offset', 'descriptor_bytes', 'descriptor_sha256', 'compute_pgm_rsrc1', 'compute_pgm_rsrc3',
      'vgpr_capacity', 'architected_vgpr_boundary', 'required_footprint_high_water', 'interpretation'], 'descriptor resources');
    integer(resource.descriptor_file_offset, 0, entry.hsaco_bytes - 64, 'descriptor offset');
    demand(resource.descriptor_bytes === 64 && resource.descriptor_sha256 === entry.descriptor_sha256 &&
      resource.required_footprint_high_water === 37 && resource.interpretation === 'encoded-capacity-not-metadata-usage-or-lifetime', 'descriptor identity/scope');
    integer(resource.compute_pgm_rsrc1, 0, 0xffff_ffff, 'RSRC1'); integer(resource.compute_pgm_rsrc3, 0, 0xffff_ffff, 'RSRC3');
    demand(resource.vgpr_capacity === ((resource.compute_pgm_rsrc1 & 63) + 1) * 8 &&
      resource.architected_vgpr_boundary === ((resource.compute_pgm_rsrc3 & 63) + 1) * 4 &&
      resource.vgpr_capacity >= resource.architected_vgpr_boundary && resource.architected_vgpr_boundary >= 37,
    'gfx942 descriptor capacity does not cover fixed footprint');
    integer(entry.boundary_register_site_count, 0, entry.static_instruction_count - 2, 'boundary count');
    integer(entry.boundary_v_mov_b32_count, 0, entry.boundary_register_site_count, 'boundary moves');
    demand(Array.isArray(entry.boundary_sites) && entry.boundary_sites.length === Math.min(32, entry.boundary_register_site_count) &&
      entry.boundary_sites_truncated === (entry.boundary_register_site_count > 32), 'boundary completeness');
    let previous = -1, moves = 0;
    for (const site of entry.boundary_sites) {
      validateSite(site, entry);
      demand(site.file_offset > previous && !entry.region.some(value => value.file_offset === site.file_offset) &&
        site.register_operands.some(value => /^VGPR3[2-6]$/.test(value)), 'boundary order/footprint');
      previous = site.file_offset; moves += Number(site.opcode.startsWith('V_MOV_B32_'));
    }
    demand(moves <= entry.boundary_v_mov_b32_count && (entry.boundary_sites_truncated || moves === entry.boundary_v_mov_b32_count), 'boundary move coverage');
  }
  return report;
}
export function parseMachineObservation(bytes, expected) {
  return validateMachineObservation(json(bytes, MAX_REPORT), expected);
}

export function validateSourceLadder(ladder) {
  exact(ladder, ['schema', 'observations', 'grants_artifact_or_launch_authority', 'hardware_observed'], 'source ladder');
  demand(ladder.schema === 'fe2o3-test-source-ordered-region-ladder-v31' && ladder.grants_artifact_or_launch_authority === false &&
    ladder.hardware_observed === false && Array.isArray(ladder.observations) && ladder.observations.length === 6, 'source ladder scope/count');
  for (const [index, row] of ladder.observations.entries()) {
    exact(row, ['schema', 'feature', 'invocation', 'observation', 'actual_rustc_callback', 'source_unchanged', 'proof_executed',
      'final_production_admitted', 'grants_artifact_or_launch_authority', 'hardware_observed'], 'source observation');
    demand(row.schema === 'fe2o3-test-source-ordered-region-observation-v31' && row.feature === FEATURES[index] &&
      row.actual_rustc_callback === true && row.source_unchanged === true, 'source callback case');
    for (const field of ['proof_executed', 'final_production_admitted', 'grants_artifact_or_launch_authority', 'hardware_observed'])
      demand(row[field] === false, `unsupported source ${field}`);
    const invocation = row.invocation;
    exact(invocation, ['schema', 'feature', 'args', 'crate_binding', 'cargo_observation', 'source_sha256', 'root_source_sha256',
      'manifest_sha256', 'artifacts_sha256', 'metadata_sha256'], 'source invocation');
    demand(invocation.schema === 'fe2o3-test-source-ordered-region-invocation-v31' && invocation.feature === row.feature &&
      Array.isArray(invocation.args) && invocation.args.length > 0 && invocation.args.length <= 128 &&
      invocation.args.every(arg => typeof arg === 'string' && arg.length > 0 && arg.length <= 4096), 'invocation arguments');
    for (const field of ['crate_binding', 'cargo_observation', 'source_sha256', 'root_source_sha256', 'manifest_sha256', 'artifacts_sha256', 'metadata_sha256']) digest(invocation[field], field);
    const result = row.observation;
    if (index < 2) {
      exact(result, ['stage', 'semantic_sha256', 'canonical_v16_identity', 'canonical_bytes_sha256', 'canonical_v16_length', 'llvm_sha256',
        'rustc_identity_inventory_sha256', 'rustc_preflight_plan_sha256', 'region_source_ids', 'region_count', 'cpu_cases', 'lanes_per_case',
        'canaries_unchanged', 'unused_result_retained'], 'positive source observation');
      demand(result.stage === 'actual_source_v31_exact_v16_cpu_and_llvm_observed' && result.region_count === 1 && result.cpu_cases === 6 &&
        result.lanes_per_case === 64 && result.canaries_unchanged === true && result.unused_result_retained === (index === 1), 'positive callback coverage');
      for (const field of ['semantic_sha256', 'canonical_v16_identity', 'canonical_bytes_sha256', 'llvm_sha256', 'rustc_identity_inventory_sha256', 'rustc_preflight_plan_sha256']) digest(result[field], field);
      integer(result.canonical_v16_length, 1, 1024 * 1024, 'canonical length');
      demand(Array.isArray(result.region_source_ids) && result.region_source_ids.length === 4, 'four source IDs');
      for (const id of result.region_source_ids) digest(id, 'source ID');
    } else {
      exact(result, ['stage', 'diagnostic'], 'negative source observation');
      demand(result.stage === 'actual_source_profile_refused' && typeof result.diagnostic === 'string' && result.diagnostic.length <= 65536 &&
        result.diagnostic.includes(REJECTIONS[index - 2]), 'negative callback wrong refusal');
    }
  }
  demand(ladder.observations[0].observation.llvm_sha256 !== ladder.observations[1].observation.llvm_sha256 &&
    ladder.observations[0].observation.canonical_bytes_sha256 !== ladder.observations[1].observation.canonical_bytes_sha256, 'source variants aliased');
  return ladder;
}

export function parseArguments(argv) {
  const values = new Map(), allowed = ['--source-run', '--native-build-run', '--output', '--compiler-repo'];
  for (let i = 0; i < argv.length; i += 2) {
    demand(allowed.includes(argv[i]) && !values.has(argv[i]) && i + 1 < argv.length, 'unknown/duplicate/incomplete option');
    values.set(argv[i], absolute(argv[i + 1], argv[i]));
  }
  for (const key of allowed.slice(0, 3)) demand(values.has(key), `missing ${key}`);
  return { source: values.get('--source-run'), native: values.get('--native-build-run'), output: values.get('--output'),
    repo: values.get('--compiler-repo') ?? path.resolve(HERE, '..') };
}

function inside(parent, child) { const relative = path.relative(parent, child); return relative === '' || (!relative.startsWith('../') && relative !== '..' && !path.isAbsolute(relative)); }
function writeNew(file, bytes) { fs.writeFileSync(file, bytes, { flag: 'wx', mode: 0o600 }); }
export async function main(argv) {
  if (argv.length === 1 && argv[0] === '--help') {
    console.log('Usage: node scripts/assembly-region-source-machine-observation.mjs --source-run ABS --native-build-run ABS --output NEW_ABS [--compiler-repo ABS]\nRequires a successful six-case actual callback ladder and a fresh qualified native prototype build. No source edits, GPU, protected finalizer or execution authority.'); return;
  }
  const options = parseArguments(argv), repo = fs.realpathSync(options.repo), source = fs.realpathSync(options.source), native = fs.realpathSync(options.native);
  const output = path.join(fs.realpathSync(path.dirname(options.output)), path.basename(options.output));
  demand(!inside(repo, output) && !inside(source, output) && !inside(native, output) && path.dirname(output) !== output, 'new output must be outside input trees');
  requireDiskReserve(path.dirname(output)); fs.mkdirSync(output, { mode: 0o700 });
  const inputs = [], reports = [], stages = [];
  const capture = (file, cap) => { const item = measureInput(file, cap); inputs.push(item); return item; };
  const read = (file, cap) => { const observed = readMeasured(file, cap); inputs.push(observed.measurement); return observed.bytes; };
  const receipt = { schema: 'fe2o3-source-ordered-region-machine-join-v1', status: 'failed',
    scope: 'retained actual callback byte joins and unauthenticated test transport final-machine observations',
    metadata_authority: 'inert-consistency-not-source-authentication', source_authentication: false,
    production_exact_region_admission: false, protected_finalizer_admission: false, grants_artifact_or_launch_authority: false,
    hardware_executed: false, runtime_closure_attestation: 'unavailable', compiler_repo: repo, source_run: source, native_build_run: native,
    limits: { minimum_free_bytes: MIN_FREE_BYTES.toString(), native_timeout_ms: 120000, native_stdout_bytes: MAX_REPORT, llvm_bytes: 65536 },
    inputs, stages, machine_reports: reports };
  try {
    capture(fileURLToPath(import.meta.url), 256 * 1024);
    capture(path.join(HERE, 'assembly-region-worker-prototype.mjs'), 256 * 1024);
    const build = json(read(path.join(native, 'receipt.json'), 256 * 1024));
    // Validate the COMPLETE fixed roster before opening any claimed build-input
    // path. A nonempty caller-selected subset is not a measured experiment.
    const roster = validateNativeBuildReceipt(build, { repo, output: native });
    const nativeMeasurements = [];
    for (const kind of ['inputs', 'artifacts']) for (const [index, expected] of roster[kind].entries()) {
      const measured = measureNativeBuildInput(expected.requested, expected.cap);
      same(measured, build[kind][index], 'stale native build input');
      nativeMeasurements.push(measured);
      // Keep the existing regular-file-only source-input fence, while separately
      // checking the original package/tool requested->resolved binding below.
      inputs.push({ ...measured, requested: measured.resolved });
    }
    const claim = read(path.join(native, 'build/fe2o3-worker-build-id.txt'), 256).toString('utf8').trim();
    const nativeBaseline = read(path.join(native, 'observation.json'), MAX_REPORT);
    parseObservation(nativeBaseline, claim);
    demand(build.observation.sha256 === sha(nativeBaseline) && build.observation.bytes === nativeBaseline.length &&
      build.observation.worker_build_claim === claim, 'native baseline receipt join');
    const executable = path.join(native, 'build', TEST_TARGET);
    const ladder = validateSourceLadder(json(read(path.join(source, 'observation.json'), 256 * 1024)));
    const fixture = path.join(repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device');
    const bound = {
      source_sha256: capture(path.join(fixture, 'src/ordered_region_v31.rs'), 1024 * 1024).sha256,
      root_source_sha256: capture(path.join(fixture, 'src/lib.rs'), 1024 * 1024).sha256,
      manifest_sha256: capture(path.join(fixture, 'Cargo.toml'), 1024 * 1024).sha256,
      artifacts_sha256: capture(path.join(source, 'dependencies.stdout'), 16 * 1024 * 1024).sha256,
      metadata_sha256: capture(path.join(source, 'metadata.stdout'), 16 * 1024 * 1024).sha256,
    };
    for (const [index, row] of ladder.observations.entries()) {
      same(json(read(path.join(source, `${row.feature}.invocation.json`), 65536)), row.invocation, 'independent invocation record');
      for (const [field, expected] of Object.entries(bound)) demand(row.invocation[field] === expected, `stale source ${field}`);
      const stdout = new TextDecoder('utf-8', { fatal: true }).decode(read(path.join(source, `${row.feature}.stdout`), 16 * 1024 * 1024));
      const lines = stdout.split(/\r?\n/), prefix = 'FE2O3_ORDERED_REGION_OBSERVATION_V31 ';
      const observations = lines.filter(line => line.startsWith(prefix));
      demand(observations.length === 1 && lines.filter(line => line.startsWith('test result: ok. 1 passed; 0 failed; 0 ignored;')).length === 1,
        'missing actual callback result or zero-test success');
      same(JSON.parse(observations[0].slice(prefix.length)), row, 'callback stdout join');
      if (index >= 2) continue;
      const llvmFile = path.join(source, row.feature, 'observation.ll');
      const llvm = capture(llvmFile, 65536), kir = capture(path.join(source, row.feature, 'canonical-v16.bin'), 1024 * 1024);
      demand(llvm.sha256 === row.observation.llvm_sha256 && kir.sha256 === row.observation.canonical_bytes_sha256 &&
        kir.bytes === row.observation.canonical_v16_length, 'source/canonical/LLVM byte join');
      requireDiskReserve(output);
      const result = await runBoundedCommand({ executable, args: [index === 0 ? '--observe-llvm-used' : '--observe-llvm-unused', llvmFile],
        cwd: repo, env: { PATH: '/usr/bin:/bin', LANG: 'C', LC_ALL: 'C' }, timeoutMs: 120000,
        stdoutCap: MAX_REPORT, stderrCap: 128 * 1024, diskDirectory: output });
      for (const stream of ['stdout', 'stderr']) writeNew(path.join(output, `${row.feature}.${stream}`), result[stream]);
      stages.push({ feature: row.feature, code: result.code, signal: result.signal, reason: result.reason, elapsed_ms: result.elapsed_ms,
        stdout_sha256: sha(result.stdout), stdout_bytes: result.stdout.length, stderr_sha256: sha(result.stderr), stderr_bytes: result.stderr.length });
      demand(result.code === 0 && result.signal === null && result.reason === null, `machine process failed for ${row.feature}`);
      const report = parseMachineObservation(result.stdout, { workerClaim: claim, llvmSha256: llvm.sha256, llvmBytes: llvm.bytes, used: index === 0 });
      reports.push({ feature: row.feature, source_sha256: row.invocation.source_sha256, canonical_bytes_sha256: kir.sha256,
        llvm_sha256: llvm.sha256, report_sha256: sha(result.stdout), kernel_symbol: report.kernel_symbol, cases: report.cases.length });
    }
    demand(reports.length === 2 && reports.reduce((sum, report) => sum + report.cases, 0) === 4, 'missing source machine cases');
    requireNativeBuildInputsUnchanged(nativeMeasurements);
    requireUnchanged(inputs); requireDiskReserve(output);
    receipt.source_callback_cases = 6; receipt.machine_cases = 4; receipt.status = 'passed';
  } catch (error) { receipt.failure = String(error.message).slice(0, 4096); throw error; }
  finally { writeNew(path.join(output, 'receipt.json'), `${JSON.stringify(receipt, null, 2)}\n`); }
  console.log(JSON.stringify({ status: receipt.status, output, source_callback_cases: 6, machine_cases: 4, scope: receipt.scope }));
}
if (process.argv[1] && path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  main(process.argv.slice(2)).catch(error => { console.error(String(error.message).slice(0, 4096)); process.exitCode = 1; });
}
