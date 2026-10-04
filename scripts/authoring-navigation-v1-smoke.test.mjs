// Synthetic diagnostic controls only. No compiler, filesystem capture, or GPU runs.
import assert from 'node:assert/strict';
import { test } from 'node:test';
import { EventEmitter } from 'node:events';
import { PassThrough, Writable } from 'node:stream';
import path from 'node:path';
import { validateNavigationCapture, validateConfiguration, parseArguments, measurementRoster,
  exportArguments, sourceTarget } from './authoring-navigation-v1-smoke.mjs';
import { runNavigationCommand } from './authoring-navigation-v1-process.mjs';
import { AUTHORITY, AVAILABILITY, CAPABILITIES, LIMITS, FIXTURE, TARGET, RUSTC_COMMIT,
  artifact, sha256, coordinate } from './authoring-navigation-v1-data.mjs';

const json = value => Buffer.from(JSON.stringify(value) + '\n');
const clone = value => JSON.parse(JSON.stringify(value));
const pin = label => sha256(Buffer.from(label));
const empty = () => artifact(Buffer.alloc(0));
const SOURCE = '#![no_std]\npub fn bitwise_chain(a: u32, b: u32) { let low = a ^ b; let result = low | 256; }\n';
const shape = value => artifact(json(value));
function makeCapture(count = 2, source = SOURCE) {
  const configuration = { repo: '/task/compiler', output: '/task/navigation-r1', bin_dir: '/task/bin',
    rustc: '/toolchain/bin/rustc', cargo: '/toolchain/bin/cargo', rustc_driver: '/toolchain/lib/librustc_driver-123abc.so',
    cargo_home: '/task/cargo-home', node: '/usr/bin/node', cache_roots: ['/task/cache1', '/task/cache2'] };
  const file = pin('stable-file-not-source-hash'), sourceBytes = Buffer.from(source);
  const start = sourceBytes.indexOf(Buffer.from('low | 256'));
  const span = { file_identity: file, display_path: 'src/lib.rs', byte_start: String(start),
    byte_end: String(start + 9), line: 2, column: 76 };
  const input = [{ value: 14, ty: 'Scalar(U32)' }, { value: 15, ty: 'Scalar(U32)' }];
  const operations = Array.from({ length: count }, (_, i) => ({
    coordinate: { function: 0, block: 0, operation: i }, function_name: 'bitwise_chain',
    kind: i === count - 1 ? 'binary' : 'constant', semantic_detail: i === count - 1 ? 'BitOr' : 'U32(256)',
    mnemonic: null, inline_assembly_source: null, inputs: i === count - 1 ? input : [],
    results: [{ value: i === count - 1 ? 16 : 100 + i, ty: 'Scalar(U32)' }],
    local_memory_effects: [], complete_local_effect_summary: true, convergence: 'not_analyzed',
    traps: 'not_analyzed', physical_resources: 'unavailable_logical_canonical_stage',
    source_binding: 'bundle_content_bound_not_authenticated', source_spans: [clone(span)],
    materialization: i === count - 1 ? 'diagnostic_rust_draft_available' : 'unavailable_unsupported_operation_or_contract',
  }));
  const snapshot = { schema: 'fe2o3-multilevel-authoring-observation-v1', authority: clone(AUTHORITY),
    bundle_identity: pin('bundle-domain'), bundle_subject_identity: pin('bundle-subject'),
    canonical_kir_version: 11, canonical_kir_digest: pin('canonical-kir'), canonical_kir_bytes: '400',
    target: TARGET, source_map_identity: pin('debug-map'), semantic_mir_identity: pin('semantic-mir'),
    rustc_identity_inventory_receipt_sha256: pin('inventory'), rustc_identity_inventory_receipt_bytes: '544',
    rustc_preflight_plan_receipt_sha256: pin('preflight'), rustc_preflight_plan_receipt_bytes: '1700',
    compiler_policy_identity: 'unavailable_in_v6', final_artifact_identity: 'unavailable_extraction_precedes_final_artifact',
    operation_count: count, eliminated_source_span_count: 0, capabilities: clone(CAPABILITIES) };
  const selector = { bundle_identity: snapshot.bundle_identity, canonical_kir_digest: snapshot.canonical_kir_digest,
    target: TARGET, operations: [operations.at(-1).coordinate] };
  const region = { authority: clone(AUTHORITY), selector, structural_boundary: 'contiguous_operations_in_one_block_no_terminator_selected',
    source_insertion_boundary: 'unavailable_source_application_not_admitted', live_in: input,
    live_out: operations.at(-1).results, operations: [operations.at(-1)], materialization: 'diagnostic_rust_draft_available' };
  const run_id = pin('synthetic-test-run');
  const census = { schema: 'fe2o3-diagnostic-source-census-v1', diagnosticOnly: true, qualified: false,
    authenticatesCompilerExecution: false, extractionSucceeded: true, arguments: [
      configuration.rustc, '--crate-name', 'fe2o3_ordinary_bitwise_promotion_v1_fixture', 'src/lib.rs'],
    workingDirectory: path.join(configuration.repo, FIXTURE), extractionMode: { kind: 'simulation-bundle', version: 6 },
    runId: run_id, selection: { status: 'available', value: { target: TARGET,
      functions: [{ functionIdentity: pin('function'), definitionIdentity: pin('definition'),
        monomorphizationIdentity: pin('instance'), role: 'kernel-entry', exportName: 'bitwise_chain',
        logicalName: null, definition: { status: 'unavailable', value: 'synthetic control' },
        identifier: { status: 'unavailable', value: 'synthetic control' } }],
      files: [{ identity: file, displayPath: 'src/lib.rs', compiledSourceHash: 'synthetic:' + pin('compiler-source'),
        originalSha256: sha256(sourceBytes), originalBytes: sourceBytes.length, normalizedBytes: sourceBytes.length }] } } };
  const pages = [];
  for (let start = 0; start < count; start += LIMITS.page_items) {
    const pageOps = operations.slice(start, start + LIMITS.page_items), next = start + pageOps.length;
    pages.push(shape({ authority: clone(AUTHORITY), bundle_identity: snapshot.bundle_identity,
      canonical_kir_digest: snapshot.canonical_kir_digest, target: TARGET, start, total_operations: count,
      operations: pageOps, next_start: next < count ? next : null }));
  }
  const a = { source: artifact(sourceBytes), manifest: artifact(Buffer.from('[workspace]\n')),
    lock: artifact(Buffer.from('version = 4\n')), bundle: { bytes: 20, sha256: pin('synthetic-bundle') },
    census: shape(census), snapshot: shape(snapshot), pages, selector: shape(selector), region: shape(region) };
  const result = { schema: 'task-authoring-navigation-capture-v1', status: 'passed',
    scope: 'retained actual ordinary-source read-only navigation; not compiler or source authority',
    authority: clone(AUTHORITY), run_id, configuration, limits: clone(LIMITS), measurements_before: [], measurements_after: [],
    stages: [], resource_guards: [], artifacts: a, availability: clone(AVAILABILITY) };
  result.measurements_before = measurementRoster(configuration).map(([role, requested]) => ({
    role, requested, resolved: requested, bytes: a[role]?.bytes ?? 32, sha256: a[role]?.sha256 ?? pin(role) }));
  result.measurements_after = clone(result.measurements_before);
  const commands = [
    ['git-head', '/usr/bin/git', ['rev-parse', 'HEAD'], artifact(Buffer.from('a'.repeat(40) + '\n'))],
    ['git-status', '/usr/bin/git', ['status', '--porcelain=v1', '--untracked-files=normal'], empty()],
    ['rustc-version', configuration.rustc, ['-vV'],
      artifact(Buffer.from('rustc 1.96.0-nightly\ncommit-hash: ' + RUSTC_COMMIT + '\nrelease: 1.96.0-nightly\n'))],
    ['export', path.join(configuration.bin_dir, 'fe2o3-export-sim'), exportArguments(configuration), empty()],
    ['inspect', path.join(configuration.bin_dir, 'fe2o3-author'), ['inspect'], a.snapshot],
    ...pages.map(page => ['operations-' + JSON.parse(page.utf8).start, path.join(configuration.bin_dir, 'fe2o3-author'),
      ['operations', '--bundle-identity', snapshot.bundle_identity, '--start', String(JSON.parse(page.utf8).start),
        '--limit', String(LIMITS.page_items)], page]),
    ['select', path.join(configuration.bin_dir, 'fe2o3-author'), ['select', '--selector', JSON.stringify(selector)], a.region],
  ];
  result.stages = commands.map(([name, executable, args, stdout], i) => ({
    name, executable, args, cwd: configuration.repo, env_delta: name === 'export' ? {
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_PATH_V1: path.join(configuration.output, 'census.json'),
      FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1: run_id } : {}, input: i >= 4 ? a.bundle : null,
    code: 0, signal: null, reason: null, elapsed_ms: 1, free_bytes_before: LIMITS.minimum_free_bytes,
    ram_bytes_before: LIMITS.minimum_ram_bytes, ram_bytes_after: LIMITS.minimum_ram_bytes, stdout, stderr: empty(),
  }));
  result.resource_guards = ['before-export', 'after-export'].map(name => ({
    name, executable: '/usr/bin/du', args: ['-sb', '--', ...configuration.cache_roots, configuration.output],
    code: 0, signal: null, reason: null, elapsed_ms: 1,
    stdout: artifact(Buffer.from([...configuration.cache_roots, configuration.output].map(root => '0\t' + root + '\n').join(''))),
    stderr: empty(), free_bytes: LIMITS.minimum_free_bytes, ram_bytes: LIMITS.minimum_ram_bytes,
  }));
  return clone(result);
}
function editArtifact(c, key, edit) {
  const value = JSON.parse(c.artifacts[key].utf8); edit(value); c.artifacts[key] = shape(value);
  const name = { snapshot: 'inspect', region: 'select' }[key];
  if (name) c.stages.find(row => row.name === name).stdout = clone(c.artifacts[key]);
}
function editPage(c, index, edit) {
  const value = JSON.parse(c.artifacts.pages[index].utf8); edit(value);
  c.artifacts.pages[index] = shape(value);
  c.stages.filter(row => row.name.startsWith('operations-'))[index].stdout = clone(c.artifacts.pages[index]);
}
const validate = c => validateNavigationCapture(json(c));
test('synthetic control retains both source-range occurrences and exact inspection-only boundary', () => {
  const { navigation } = validate(makeCapture());
  assert.equal(navigation.operations.length, 2);
  assert.equal(navigation.selected_range_occurrences.length, 2);
  assert.notEqual(navigation.source.sha256, navigation.source.file_identity);
  assert.equal(navigation.region.live_out[0].value, 16);
  assert.equal(navigation.availability.source_edit_boundary, 'unavailable');
  assert.equal(navigation.authority.source_authenticated, false);
  assert.ok(Object.isFrozen(navigation.operations[0].source_spans));
  assert.throws(() => { navigation.selector.operations[0].operation = 0; }, TypeError);
});
test('complete multi-page observations preserve exact roster occurrences', () => {
  const { navigation } = validate(makeCapture(17));
  assert.equal(navigation.operations.length, 17);
  assert.equal(navigation.selected_range_occurrences.length, 17);
  assert.equal(coordinate(navigation.selector.operations[0]), '0:0:16');
});
test('unattributed unselected operation remains inspectable without a guessed span', () => {
  const c = makeCapture(3);
  editPage(c, 0, page => { page.operations[0].source_spans = []; page.operations[0].source_binding = 'unavailable_no_source_span'; });
  assert.equal(validate(c).navigation.attributions.length, 2);
});
const censusControls = [
  ['wrong run', x => { x.runId = pin('stale'); }],
  ['failed extraction', x => { x.extractionSucceeded = false; }],
  ['wrong mode', x => { x.extractionMode.version = 5; }],
  ['unavailable selection', x => { x.selection = { status: 'unavailable', value: 'not collected' }; }],
  ['wrong crate', x => { x.arguments[2] = 'different_crate'; }],
  ['wrong source argument', x => { x.arguments[3] = 'other.rs'; }],
  ['qualified census', x => { x.qualified = true; }],
  ['wrong source hash', x => { x.selection.value.files[0].originalSha256 = pin('different bytes'); }],
  ['wrong source length', x => { x.selection.value.files[0].originalBytes++; }],
  ['normalization difference', x => { x.selection.value.files[0].normalizedBytes--; }],
  ['duplicate file identity', x => { x.selection.value.files.push(clone(x.selection.value.files[0])); }],
  ['path does not substitute for file identity', x => { x.selection.value.files[0].identity = pin('different file'); }],
  ['ambiguous source association', x => { const f = clone(x.selection.value.files[0]); f.identity = pin('other'); x.selection.value.files.push(f); }],
];
for (const [name, edit] of censusControls) test('census refusal: ' + name, () => {
  const c = makeCapture(); editArtifact(c, 'census', edit); assert.throws(() => validate(c));
});
const pageControls = [
  ['wrong target', p => { p.target = 'gfx950:xnack-'; }],
  ['stale identity', p => { p.bundle_identity = pin('different snapshot'); }],
  ['hole', p => { p.start = 1; }],
  ['premature terminal', p => { p.next_start = 1; }],
  ['duplicate occurrence', p => { p.operations[1].coordinate = clone(p.operations[0].coordinate); }],
  ['wrong function', p => { p.operations[1].coordinate.function = 1; }],
  ['out-of-source endpoint', p => { p.operations[0].source_spans[0].byte_end = '65535'; }],
  ['wrong file ID', p => { p.operations[0].source_spans[0].file_identity = pin('caller-supplied'); }],
  ['unproved physical fact', p => { p.operations[0].physical_resources = 'available'; }],
  ['inexact coordinate', p => { p.operations[0].coordinate.operation = 0.5; }],
];
for (const [name, edit] of pageControls) test('operation refusal: ' + name, () => {
  const c = makeCapture(); editPage(c, 0, edit); assert.throws(() => validate(c));
});
test('missing/reordered pages and stale selected boundary refuse', () => {
  for (const edit of [
    c => c.artifacts.pages.pop(), c => c.artifacts.pages.reverse(),
    c => editArtifact(c, 'selector', x => { x.target = 'gfx950:xnack-'; }),
    c => editArtifact(c, 'region', x => { x.live_in.reverse(); }),
    c => editArtifact(c, 'region', x => { x.live_out[0].value = 99; }),
    c => editArtifact(c, 'region', x => { x.source_insertion_boundary = 'available'; }),
    c => editArtifact(c, 'region', x => { x.operations[0].complete_local_effect_summary = false; }),
  ]) { const c = makeCapture(17); edit(c); assert.throws(() => validate(c)); }
});
test('bytes, Unicode normalization, surrogate and duplicate-key bounds refuse', () => {
  for (const prefix of ['\ufeff', '\r']) assert.throws(() => validate(makeCapture(2, prefix + SOURCE)));
  const c = makeCapture(2, 'é' + SOURCE);
  editPage(c, 0, page => { page.operations[0].source_spans[0].byte_start = '1'; });
  assert.throws(() => validate(c), /UTF-8/);
  const changed = makeCapture(); changed.artifacts.source.utf8 += '\n';
  assert.throws(() => validate(changed), /artifact bytes/);
  const surrogate = makeCapture(); surrogate.artifacts.source.utf8 = '\ud800';
  assert.throws(() => validate(surrogate), /surrogate/);
  const oversized = makeCapture(); oversized.artifacts.source.utf8 = 'x'.repeat(LIMITS.source_bytes + 1);
  assert.throws(() => validate(oversized), /bound/);
  assert.throws(() => validateNavigationCapture(Buffer.from('{"schema":"x","schema":"y"}')), /duplicate/);
  assert.throws(() => validateNavigationCapture(Buffer.alloc(2 * 1024 * 1024 + 1)), /bound/);
});
test('command, measurement, authority and resource substitutions refuse', () => {
  for (const edit of [
    c => { c.authority.source_authenticated = true; },
    c => { c.availability.semantic_mir = 'available'; },
    c => { c.stages[3].env_delta.FE2O3_DIAGNOSTIC_SOURCE_CENSUS_RUN_ID_V1 = pin('old'); },
    c => { c.stages[3].args[1] = 'different_crate'; },
    c => { c.stages[3].code = 1; },
    c => { c.stages[3].reason = 'timeout'; },
    c => { c.stages[3].stderr = artifact(Buffer.from('fe2o3 diagnostic source census unavailable: failed\n')); },
    c => { c.stages[4].input.sha256 = pin('wrong stdin'); },
    c => { c.stages[2].executable = '/unmeasured/rustc'; },
    c => { c.stages[2].stdout = artifact(Buffer.from('release: wrong\n')); },
    c => { c.measurements_after[0].sha256 = pin('changed'); },
    c => { c.measurements_before[0].sha256 = pin('forged source'); c.measurements_after = clone(c.measurements_before); },
    c => { c.resource_guards[0].stdout = artifact(Buffer.from('0\t/other\n0\t/task/cache2\n0\t/task/navigation-r1\n')); },
    c => { c.resource_guards[1].ram_bytes = '1'; },
    c => { c.stages[5].stdout = artifact(Buffer.from('{}')); },
  ]) { const c = makeCapture(); edit(c); assert.throws(() => validate(c)); }
});
test('closed command layout rejects overlap, broad output and option substitution', () => {
  const c = makeCapture().configuration;
  assert.equal(sourceTarget(c), '/task/cache1/navigation-navigation-r1');
  assert.throws(() => validateConfiguration({ ...c, output: '/task/compiler/capture' }), /overlap/);
  assert.throws(() => validateConfiguration({ ...c, cache_roots: ['/task/cache1', '/task/cache1/nested'] }), /overlap/);
  assert.throws(() => validateConfiguration({ ...c, output: '/' }));
  assert.throws(() => parseArguments(['--output', '/task/out']));
});

// EventEmitter/stream child stand-ins exercise transport, never real processes.
function mockSpawn(behavior) {
  const killed = [], chunks = []; let child;
  const spawnImpl = (_exe, _args, options) => {
    assert.deepEqual(options.stdio, ['pipe', 'pipe', 'pipe']); assert.equal(options.detached, true);
    child = new EventEmitter(); child.pid = 123;
    child.stdout = new PassThrough(); child.stderr = new PassThrough();
    child.stdin = new Writable({ highWaterMark: 1, write(chunk, _encoding, next) {
      chunks.push(Buffer.from(chunk)); setImmediate(next);
    } });
    child.stdin.on('finish', () => behavior(child));
    return child;
  };
  const close = (code = 0, signal = null) => {
    child.stdout.end(); child.stderr.end(); child.emit('close', code, signal);
  };
  return { spawnImpl, killImpl(pid, signal) {
    killed.push([pid, signal]); setImmediate(() => close(null, 'SIGKILL'));
  }, killed, chunks, close, get child() { return child; } };
}
const command = extra => ({ executable: '/not-launched/compiler', args: [], cwd: '/task', env: {},
  timeoutMs: 1000, outputCap: 65536, ...extra });
test('bounded stdin waits for drain and delivers exact binary input before success', async () => {
  const fake = mockSpawn(child => {
    child.stdout.write('ok'); child.stderr.write('note'); fake.close();
  });
  const input = Buffer.alloc(70000, 0xa5);
  const result = await runNavigationCommand(command({ input }), fake);
  assert.equal(result.code, 0); assert.equal(result.reason, null);
  assert.deepEqual(Buffer.concat(fake.chunks), input);
  assert.equal(result.stdout.toString(), 'ok'); assert.equal(result.stderr.toString(), 'note');
  assert.equal(fake.killed.length, 0);
});
test('stdout/stderr caps terminate the process group and retain only bounded bytes', async () => {
  for (const stream of ['stdout', 'stderr']) {
    const fake = mockSpawn(child => child[stream].write(Buffer.alloc(33)));
    const result = await runNavigationCommand(command({ outputCap: 32 }), fake);
    assert.equal(result.reason, stream + '_cap'); assert.equal(result[stream].length, 32);
    assert.deepEqual(fake.killed, [[-123, 'SIGKILL']]); assert.notEqual(result.code, 0);
  }
});
test('timeout and failed stdin are transport failures, never semantic refusals', async () => {
  const slow = mockSpawn(() => {});
  const timed = await runNavigationCommand(command({ timeoutMs: 5 }), slow);
  assert.equal(timed.reason, 'timeout'); assert.deepEqual(slow.killed, [[-123, 'SIGKILL']]);
  const broken = mockSpawn(child => child.stdin.emit('error', Object.assign(new Error('closed'), { code: 'EPIPE' })));
  const result = await runNavigationCommand(command({ input: Buffer.from('binary') }), broken);
  assert.equal(result.reason, 'stdin:EPIPE'); assert.notEqual(result.code, 0);
});
test('spawn/setup, stream and ordinary nonzero exits never become successful evidence', async () => {
  const failure = await runNavigationCommand(command({}), { spawnImpl() { throw Object.assign(new Error('missing'), { code: 'ENOENT' }); } });
  assert.equal(failure.reason, 'setup:ENOENT'); assert.equal(failure.code, null);
  const guarded = await runNavigationCommand(command({ guard() { throw new Error('reserve'); } }));
  assert.match(guarded.reason, /setup:reserve/);
  const stream = mockSpawn(child => child.stdout.emit('error', Object.assign(new Error('read'), { code: 'EIO' })));
  assert.equal((await runNavigationCommand(command({}), stream)).reason, 'stdout:EIO');
  const exit = mockSpawn(() => exit.close(17));
  const result = await runNavigationCommand(command({}), exit);
  assert.equal(result.code, 17); assert.equal(result.reason, null);
  assert.throws(() => runNavigationCommand(command({ input: Buffer.alloc(4 * 1024 * 1024 + 1) })), /bounds/);
});
