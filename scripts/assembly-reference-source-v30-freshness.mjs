#!/usr/bin/env node
// Test-only actual-source stale receipt pair; never a production proof bypass.
// Positive receipt is captured from the normal protected producer/import. Negative
// feature children replay only inert bytes, then run the unchanged false proof.
// Prepare with the ignored prepare_actual_source_reference_inputs Rust test.
// Then pass --prepared, --harness, --image sha256:..., --native-volume and
// --output (a new absolute directory). No Cargo or installation runs here.
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import {
  constants, closeSync, chmodSync, fstatSync, mkdirSync, openSync, readSync,
  realpathSync, statfsSync, writeFileSync,
} from 'node:fs';
import { basename, dirname, isAbsolute, join, resolve } from 'node:path';
import { spawn } from 'node:child_process';
import { fileURLToPath, pathToFileURL } from 'node:url';

export const FEATURES = [
  'assembly-reference-positive',
  'assembly-reference-wrong-opcode',
  'assembly-reference-wrong-constant',
];
export const CHILD = 'production_rustc_driver_v1::gfx942_inline_reference_qualification_v30_tests::stale_proof::actual_source_reference_freshness_child';
export const PREFIX = 'FE2O3_ASSEMBLY_REFERENCE_FRESHNESS_V30 ';
const CAP = 16 * 1024 * 1024;
const TASK = 'authoring-280-282';
const VOLUME = 'fe2o3-authoring-proof-native-20260917-r1';
const RUNTIME = '/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5';
const HASH = /^[0-9a-f]{64}$/;
const utf8 = new TextDecoder('utf-8', { fatal: true });
const sha = bytes => createHash('sha256').update(bytes).digest('hex');
const object = value => value !== null && typeof value === 'object' && !Array.isArray(value);
function exactKeys(value, keys) {
  assert(object(value), 'expected an object');
  assert.deepEqual(Object.keys(value).sort(), [...keys].sort(), 'unexpected or missing fields');
}
function normalizedPath(path) {
  assert(typeof path === 'string' && isAbsolute(path) && resolve(path) === path
    && !/[\x00-\x1f,]/.test(path), 'expected a normalized absolute mount-safe path');
  return path;
}
export function readBounded(path, cap = CAP) {
  const fd = openSync(path, constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
  try {
    const info = fstatSync(fd);
    assert(info.isFile() && info.size <= cap, 'expected a bounded regular file');
    const buffer = Buffer.alloc(Math.min(info.size + 1, cap + 1));
    let used = 0;
    while (used < buffer.length) {
      const count = readSync(fd, buffer, used, buffer.length - used, null);
      if (count === 0) break;
      used += count;
    }
    assert.equal(used, info.size, 'file size changed during read');
    const after = fstatSync(fd);
    assert.equal(after.size, info.size, 'file changed during read');
    return buffer.subarray(0, used);
  } finally { closeSync(fd); }
}
const readJson = path => JSON.parse(utf8.decode(readBounded(path, 64 * 1024)));
const writeJson = (path, value) => writeFileSync(path, JSON.stringify(value, null, 2) + '\n', { flag: 'wx' });

export function parseOptions(args) {
  const keys = ['prepared', 'harness', 'image', 'native-volume', 'output'];
  const out = {};
  for (let i = 0; i < args.length; i += 2) {
    const key = args[i]?.slice(2);
    assert(args[i]?.startsWith('--') && keys.includes(key) && !(key in out)
      && typeof args[i + 1] === 'string' && !args[i + 1].startsWith('--'), 'invalid CLI arguments');
    out[key] = args[i + 1];
  }
  exactKeys(out, keys);
  for (const key of ['prepared', 'harness', 'output']) normalizedPath(out[key]);
  assert(/^sha256:[0-9a-f]{64}$/.test(out.image), 'immutable image ID required');
  assert.equal(out['native-volume'], VOLUME, 'native volume is outside this task');
  assert(/^[a-z0-9][a-z0-9-]{1,64}$/.test(basename(out.output)), 'bounded Docker-safe output basename required');
  return out;
}

export function validatePreparation(preparation, hashes) {
  exactKeys(preparation, [
    'schema', 'features', 'metadata_sha256', 'artifacts_sha256', 'source_sha256',
    'root_source_sha256', 'manifest_sha256', 'rustc_callback_executed', 'proof_executed',
    'grants_artifact_or_launch_authority',
  ]);
  assert.equal(preparation.schema, 'fe2o3-test-source-isa-reference-preparation-v30');
  assert.deepEqual(preparation.features, FEATURES);
  for (const key of ['metadata_sha256', 'artifacts_sha256', 'source_sha256', 'root_source_sha256', 'manifest_sha256']) {
    assert(HASH.test(preparation[key]));
    assert.equal(preparation[key], hashes[key], 'stale preparation ' + key);
  }
  for (const key of ['rustc_callback_executed', 'proof_executed', 'grants_artifact_or_launch_authority']) {
    assert.equal(preparation[key], false);
  }
}
export function validateInvocation(record, feature, preparation) {
  exactKeys(record, ['schema', 'feature', 'args', 'crate_binding', 'cargo_observation',
    'source_sha256', 'root_source_sha256', 'manifest_sha256']);
  assert.equal(record.schema, 'fe2o3-test-source-isa-reference-invocation-v30');
  assert(FEATURES.includes(feature));
  assert.equal(record.feature, feature);
  assert(Array.isArray(record.args) && record.args.length > 10 && record.args.length < 128
    && record.args.every(arg => typeof arg === 'string' && arg.length < 4096 && !arg.includes('\0')));
  for (const key of ['crate_binding', 'cargo_observation']) assert(HASH.test(record[key]));
  for (const key of ['source_sha256', 'root_source_sha256', 'manifest_sha256']) {
    assert.equal(record[key], preparation[key]);
  }
  // The child independently derives the complete ordered argv and both IDs.
  // This transport check never authorizes arbitrary retained arguments.
  assert(record.args.includes('-Coverflow-checks=on'));
  assert(record.args.includes('feature="' + feature + '"'));
}

export function expectedStage(feature, diagnostic) {
  assert(FEATURES.includes(feature));
  assert(typeof diagnostic === 'string' && Buffer.byteLength(diagnostic) <= 64 * 1024);
  assert(!diagnostic.includes('proof runtime unavailable')
    && !diagnostic.includes('source-to-proof V2 effect mismatch')
    && !diagnostic.includes('source-to-proof V2 cannot normalize'), 'setup failure is not a proof observation');
  if (feature === FEATURES[0]) {
    assert(diagnostic.includes('source-to-proof V2 ranked admission failed: error[FE2O3-OWN-002]')
      && diagnostic.includes('launch dimension 0 is dynamic')
      && !diagnostic.includes('proof execution failed'), 'positive did not reach the later ownership gate');
    return 'protected_proof_completed_then_dynamic_total_view_refused';
  }
  assert(diagnostic.includes('functional-refinement proof execution failed:')
    && diagnostic.includes('UnexpectedProofResult')
    && diagnostic.includes('exit=Some(1), signal=None')
    && diagnostic.includes('verified, 1 errors\\n')
    && diagnostic.includes('assertion failed')
    && !diagnostic.includes('FE2O3-OWN-002'), 'mutation did not fail the actual Verus assertion');
  return 'protected_verus_assertion_rejected_before_ownership';
}
function byteArray(value, length, nonzero = false) {
  assert(Array.isArray(value) && value.length === length
    && value.every(byte => Number.isInteger(byte) && byte >= 0 && byte <= 255));
  if (nonzero) assert(value.some(byte => byte !== 0));
}
function binding(value) {
  exactKeys(value, ['reference_identity', 'reference_mir', 'kernel_identity',
    'kernel_mir', 'normalized_obligation']);
  for (const bytes of Object.values(value)) byteArray(bytes, 32, true);
}
export function validateOriginal(value, record) {
  exactKeys(value, ['schema', 'invocation', 'receipt']);
  assert.equal(value.schema, 'fe2o3-test-source-proof-original-v1');
  assert.equal(record.feature, FEATURES[0]);
  assert.deepEqual(value.invocation, record);
  exactKeys(value.receipt, ['schema', 'binding', 'wire', 'verifying_key']);
  assert.equal(value.receipt.schema, 'fe2o3-test-source-proof-inert-receipt-v1');
  binding(value.receipt.binding);
  byteArray(value.receipt.wire, 524);
  byteArray(value.receipt.verifying_key, 32);
  // This is framing only. The isolated Rust child performs cryptographic import
  // using the current normal protected lease and exact compiler-derived binding.
  return value;
}
export function parseObservation(stdout, record, original = null, originalHash = null) {
  assert(Buffer.byteLength(stdout) <= CAP);
  const lines = stdout.split('\n');
  const receipts = lines.filter(line => line.startsWith(PREFIX));
  assert.equal(receipts.length, 1);
  assert(Buffer.byteLength(receipts[0]) <= 64 * 1024 + PREFIX.length);
  assert.equal(lines.filter(line => /^test result: ok\. 1 passed; 0 failed; 0 ignored;/.test(line)).length, 1);
  const value = JSON.parse(receipts[0].slice(PREFIX.length));
  exactKeys(value, ['schema', 'feature', 'stage', 'invocation', 'diagnostic', 'observed',
    'original', 'original_transport_sha256', 'actual_rustc_callback',
    'source_file_bytes_unchanged', 'semantic_change_is_feature_selected', 'proof_subject_kind',
    'source_hash_is_proof_subject', 'source_admission_complete', 'v17_proof_support',
    'grants_artifact_or_launch_authority', 'hardware_observed']);
  assert.equal(value.schema, 'fe2o3-test-source-proof-freshness-observation-v1');
  assert.equal(value.feature, record.feature);
  assert.deepEqual(value.invocation, record);
  assert.equal(value.stage, expectedStage(record.feature, value.diagnostic));
  for (const key of ['actual_rustc_callback', 'source_file_bytes_unchanged',
    'semantic_change_is_feature_selected']) assert.equal(value[key], true);
  for (const key of ['source_hash_is_proof_subject', 'source_admission_complete',
    'v17_proof_support', 'grants_artifact_or_launch_authority', 'hardware_observed']) {
    assert.equal(value[key], false);
  }
  assert.equal(value.proof_subject_kind, 'mir');
  const seen = value.observed;
  exactKeys(seen, ['request_count', 'normal_import_count', 'binding', 'original_receipt',
    'stale_error', 'rejected_import_count', 'original_reimport_count',
    'normalized_obligation_changed', 'kernel_mir_changed']);
  binding(seen.binding);
  assert.equal(seen.request_count, 1);
  assert.equal(seen.original_receipt, null);
  assert.equal(seen.rejected_import_count, 0);
  if (record.feature === FEATURES[0]) {
    assert.equal(original, null);
    assert.equal(originalHash, null);
    assert.equal(value.original_transport_sha256, null);
    validateOriginal(value.original, record);
    assert.deepEqual(seen.binding, value.original.receipt.binding);
    assert.equal(seen.normal_import_count, 1);
    assert.equal(seen.stale_error, null);
    assert.equal(seen.original_reimport_count, 0);
    assert.equal(seen.normalized_obligation_changed, false);
    assert.equal(seen.kernel_mir_changed, false);
  } else {
    assert(original && HASH.test(originalHash));
    validateOriginal(original, original.invocation);
    assert.equal(sha(Buffer.from(JSON.stringify(original) + '\n')), originalHash);
    assert.notEqual(record.crate_binding, original.invocation.crate_binding);
    assert.notEqual(record.cargo_observation, original.invocation.cargo_observation);
    for (const key of ['source_sha256', 'root_source_sha256', 'manifest_sha256']) {
      assert.equal(record[key], original.invocation[key]);
    }
    assert.equal(value.original, null);
    assert.equal(value.original_transport_sha256, originalHash);
    assert.equal(seen.normal_import_count, 0);
    assert.equal(seen.stale_error, 'StaleSafeReferenceIdentity');
    assert.equal(seen.original_reimport_count, 1);
    assert.equal(seen.normalized_obligation_changed, true);
    assert.equal(seen.kernel_mir_changed, true);
    for (const key of ['reference_identity', 'kernel_identity', 'kernel_mir', 'normalized_obligation']) {
      assert.notDeepEqual(seen.binding[key], original.receipt.binding[key]);
    }
  }
  return value;
}

const children = new Set();
let activeContainer;
async function command(program, args, prefix, timeout = 300_000) {
  const result = await new Promise(resolveResult => {
    const child = spawn(program, args, { detached: true, stdio: ['ignore', 'pipe', 'pipe'],
      env: { PATH: '/usr/bin:/bin', DOCKER_HOST: 'unix:///var/run/docker.sock' } });
    children.add(child);
    const streams = [[], []];
    let total = 0;
    let error;
    const terminate = reason => {
      error ??= reason;
      if (child.pid) { try { process.kill(-child.pid, 'SIGKILL'); } catch { child.kill('SIGKILL'); } }
    };
    const timer = setTimeout(() => terminate('command exceeded deadline'), timeout);
    [child.stdout, child.stderr].forEach((stream, i) => stream.on('data', chunk => {
      total += chunk.length;
      if (total > CAP) terminate('command exceeded output cap');
      else streams[i].push(chunk);
    }));
    child.on('error', failure => { error = failure.message; });
    child.on('close', (status, signal) => {
      clearTimeout(timer);
      children.delete(child);
      resolveResult({ status, signal, error, stdout: Buffer.concat(streams[0]), stderr: Buffer.concat(streams[1]) });
    });
  });
  if (prefix) {
    writeFileSync(prefix + '.stdout', result.stdout, { flag: 'wx' });
    writeFileSync(prefix + '.stderr', result.stderr, { flag: 'wx' });
    writeJson(prefix + '.status.json', { status: result.status, signal: result.signal, error: result.error ?? null });
  }
  assert(!result.error && result.status === 0 && result.signal === null,
    program + ' failed: ' + (result.error ?? result.status) + '; ' + utf8.decode(result.stderr).slice(-4096));
  return utf8.decode(result.stdout);
}
async function docker(args, prefix, timeout = 30_000) { return command('docker', args, prefix, timeout); }
async function inspect(kind, name, prefix) {
  const values = JSON.parse(await docker([kind, 'inspect', name], prefix));
  assert(Array.isArray(values) && values.length === 1);
  return values[0];
}

async function main(args) {
  const options = parseOptions(args);
  const repo = realpathSync(join(dirname(fileURLToPath(import.meta.url)), '..'));
  const fixture = join(repo, 'crates/rustc-codegen-fe2o3/tests/fixtures/production-extraction-device');
  const prepared = realpathSync(options.prepared);
  assert.equal(prepared, options.prepared);
  assert.equal(realpathSync(dirname(options.output)), dirname(options.output));
  const space = statfsSync(dirname(options.output), { bigint: true });
  assert(space.bavail * space.bsize >= 40n * 1024n ** 3n, '40 GiB reserve unavailable');
  const preparation = readJson(join(prepared, 'preparation.json'));
  const selectedPaths = [
    ...['preparation.json', 'metadata.stdout', 'dependencies.stdout', 'sysroot.stdout',
      ...FEATURES.map(feature => feature + '.invocation.json')].map(path => join(prepared, path)),
    ...['src/assembly_reference_v30.rs', 'src/lib.rs', 'Cargo.toml'].map(path => join(fixture, path)),
  ];
  const selectedPins = selectedPaths.map(path => ({ path, sha256: sha(readBounded(path)) }));
  const hashes = {
    metadata_sha256: sha(readBounded(join(prepared, 'metadata.stdout'))),
    artifacts_sha256: sha(readBounded(join(prepared, 'dependencies.stdout'))),
    source_sha256: sha(readBounded(join(fixture, 'src/assembly_reference_v30.rs'))),
    root_source_sha256: sha(readBounded(join(fixture, 'src/lib.rs'))),
    manifest_sha256: sha(readBounded(join(fixture, 'Cargo.toml'))),
  };
  validatePreparation(preparation, hashes);
  const records = FEATURES.map(feature => {
    const record = readJson(join(prepared, feature + '.invocation.json'));
    validateInvocation(record, feature, preparation);
    return record;
  });
  const sysroot = normalizedPath(utf8.decode(readBounded(join(prepared, 'sysroot.stdout'), 4096)).trimEnd());
  assert.equal(realpathSync(sysroot), sysroot);
  assert(basename(sysroot).startsWith('nightly-2026-04-03-'), 'pinned nightly required');
  const harness = readBounded(options.harness, 512 * 1024 * 1024);
  const uidInventory = await command('ps', ['-eo', 'uid='], undefined, 10_000);
  assert(!uidInventory.split('\n').some(line => line.trim() === '61075'), 'private proof UID already active');
  mkdirSync(options.output, { mode: 0o700 });
  const output = options.output;
  const image = await inspect('image', options.image, join(output, 'image'));
  assert.equal(image.Id, options.image);
  assert.equal(image.Config.Labels['org.fe2o3.task'], TASK);
  const volume = await inspect('volume', options['native-volume'], join(output, 'native-volume'));
  assert.equal(volume.Name, VOLUME);
  assert.equal(volume.Driver, 'local');
  assert.equal(volume.Labels['org.fe2o3.task'], TASK);
  assert.equal(volume.Mountpoint, '/var/lib/docker/volumes/' + VOLUME + '/_data');
  const frozen = join(output, 'harness');
  mkdirSync(frozen, { mode: 0o755 });
  writeFileSync(join(frozen, 'backend-tests'), harness, { flag: 'wx', mode: 0o555 });
  chmodSync(join(frozen, 'backend-tests'), 0o555);
  const harnessHash = sha(harness);
  writeJson(join(output, 'inputs.json'), { preparation, records, harness_sha256: harnessHash,
    image: options.image, native_volume: VOLUME, sysroot, repository: repo,
    compiler_and_input_mounts_read_only: true, outer_seccomp_unconfined_for_controller_owned_filter: true });

  let original = null;
  let originalHash = null;
  async function runContainer(label, record) {
    const env = record ? [
      'FE2O3_TEST_ISA_REFERENCE_INPUTS_V30=' + prepared,
      'FE2O3_TEST_ISA_REFERENCE_FEATURE_V30=' + record.feature,
      'FE2O3_CRATE_BINDING_ID_V1=' + record.crate_binding,
      'FE2O3_CARGO_METADATA_BUILD_OBSERVATION_V2=' + record.cargo_observation,
      'CARGO_MANIFEST_DIR=' + fixture,
      'CARGO_PKG_NAME=fe2o3-production-extraction-fixture',
      'CARGO_PKG_VERSION=0.1.0',
      'CARGO_CRATE_NAME=fe2o3_production_extraction_fixture',
      'CARGO_PRIMARY_PACKAGE=1',
    ] : [];
    if (record && record.feature !== FEATURES[0]) {
      assert(original && HASH.test(originalHash));
      env.push('FE2O3_TEST_ISA_REFERENCE_ORIGINAL_V30=/tests/original.json',
        'FE2O3_TEST_ISA_REFERENCE_ORIGINAL_SHA256_V30=' + originalHash);
    }
    env.push('LD_LIBRARY_PATH=' + join(sysroot, 'lib'));
    const mounts = [
      [frozen, '/tests'], [repo, repo], [prepared, prepared], [sysroot, sysroot],
      [join(volume.Mountpoint, 'runtime'), RUNTIME],
      [join(volume.Mountpoint, 'interpreter'), '/usr/lib/x86_64-linux-gnu'],
    ].flatMap(([source, destination]) => ['--mount', 'type=bind,src=' + source + ',dst=' + destination + ',readonly']);
    const name = 'fe2o3-' + basename(output) + '-' + label;
    let id;
    let success = false;
    try {
      id = (await docker(['create', '--name', name, '--label', 'org.fe2o3.task=' + TASK,
        '--user', '61075:61075', '--cap-drop', 'ALL', '--security-opt', 'no-new-privileges=true',
        ...(record ? ['--security-opt', 'seccomp=unconfined'] : []),
        '--network', 'none', '--read-only', '--init', '--workdir', repo, '--pids-limit', '128',
        '--cpus', '2', '--memory', '12g', '--memory-swap', '12g',
        '--ulimit', 'nproc=4096', '--ulimit', 'nofile=1024', '--ulimit', 'core=0',
        '--log-driver', 'local', '--log-opt', 'max-size=1m', '--log-opt', 'max-file=2',
        '--tmpfs', '/tmp:rw,nosuid,nodev,noexec,size=64m,mode=1777',
        ...mounts, ...env.flatMap(value => ['--env', value]), '--entrypoint', '/tests/backend-tests',
        options.image, ...(record ? [CHILD, '--ignored', '--exact', '--test-threads=1', '--nocapture'] : ['--list'])],
      join(output, label + '-create'))).trim();
      assert(HASH.test(id), 'invalid container ID');
      activeContainer = id;
      const before = await inspect('container', id, join(output, label + '-container'));
      assert.equal(before.Image, options.image);
      assert.equal(before.Config.Labels['org.fe2o3.task'], TASK);
      const stdout = await docker(['start', '--attach', id], join(output, label), 300_000);
      const after = await inspect('container', id, join(output, label + '-terminal'));
      assert.equal(after.State.Running, false);
      assert.equal(after.State.ExitCode, 0);
      assert.equal(after.State.OOMKilled, false);
      assert.equal(after.State.Error, '');
      const result = record ? parseObservation(stdout, record, original, originalHash) : null;
      if (!record) assert.equal(stdout.split('\n').filter(line => line === CHILD + ': test').length, 1,
        'exact child not uniquely listed');
      success = true;
      return result;
    } finally {
      if (id && HASH.test(id)) {
        if (success) await docker(['rm', id], join(output, label + '-remove'));
        else {
          await docker(['stop', '--time', '5', id], join(output, label + '-stop'), 15_000).catch(() => {});
          await inspect('container', id, join(output, label + '-failed')).catch(() => {});
          console.error('Failed owned container retained for diagnosis: ' + id);
        }
        activeContainer = undefined;
      }
    }
  }
  await runContainer('list', null);
  const observations = [];
  for (const record of records) {
    const observation = await runContainer(record.feature, record);
    observations.push(observation);
    if (record.feature === FEATURES[0]) {
      original = validateOriginal(observation.original, record);
      const bytes = Buffer.from(JSON.stringify(original) + '\n');
      assert(bytes.length <= 64 * 1024);
      originalHash = sha(bytes);
      writeFileSync(join(frozen, 'original.json'), bytes, { flag: 'wx', mode: 0o444 });
      chmodSync(join(frozen, 'original.json'), 0o444);
    }
  }
  assert.equal(sha(readBounded(join(frozen, 'original.json'), 64 * 1024)), originalHash);
  assert.equal(sha(readBounded(join(frozen, 'backend-tests'), 512 * 1024 * 1024)), harnessHash);
  validatePreparation(readJson(join(prepared, 'preparation.json')), {
    ...hashes,
    source_sha256: sha(readBounded(join(fixture, 'src/assembly_reference_v30.rs'))),
    root_source_sha256: sha(readBounded(join(fixture, 'src/lib.rs'))),
    manifest_sha256: sha(readBounded(join(fixture, 'Cargo.toml'))),
  });
  for (const pin of selectedPins) assert.equal(sha(readBounded(pin.path)), pin.sha256,
    'selected preparation/source bytes changed during protected observation');
  const receipt = { schema: 'fe2o3-source-isa-reference-proof-freshness-v1',
    observations, original_transport_sha256: originalHash, selected_pins: selectedPins,
    image: options.image, native_volume: VOLUME, harness_sha256: harnessHash,
    semantic_change_is_feature_selected: true, source_file_bytes_unchanged: true,
    source_hash_is_proof_subject: false, v17_proof_support: false,
    source_admission_complete: false, grants_artifact_or_launch_authority: false, hardware_observed: false };
  writeJson(join(output, 'observation.json'), receipt);
  console.log(JSON.stringify(receipt, null, 2));
}

if (process.argv[1] && pathToFileURL(resolve(process.argv[1])).href === import.meta.url) {
  let interrupted = false;
  for (const signal of ['SIGINT', 'SIGTERM']) process.on(signal, async () => {
    if (interrupted) return;
    interrupted = true;
    for (const child of children) if (child.pid) { try { process.kill(-child.pid, 'SIGKILL'); } catch {} }
    if (activeContainer) await docker(['stop', '--time', '5', activeContainer], undefined, 15_000).catch(() => {});
    process.exit(130);
  });
  main(process.argv.slice(2)).catch(error => { console.error(error.stack ?? error); process.exitCode = 1; });
}
