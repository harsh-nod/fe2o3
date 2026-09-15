// Assemble only closed, replayed local CPU/test evidence. Never overwrite an archive.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', repo = root + 'fe2o3-r61-execution';
const destination = path.join(repo, 'docs/evidence/local-r121-ordinary-data-cleanup-2026-09-15');
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
const captured = new Map();
function bytes(name) {
  const file = path.isAbsolute(name) ? name : root + name;
  assert(fs.lstatSync(file).isFile(), 'regular input');
  if (!captured.has(file)) captured.set(file, fs.readFileSync(file));
  return captured.get(file);
}
const json = name => JSON.parse(bytes(name));
function pin(name, digest) {
  const b = bytes(name); assert.strictEqual(sha(b), digest, name); return b;
}
const q = JSON.parse(pin('r121-development-qualification-inputs-v4.json',
  'b73f84b689968e8a4fe494848e28ff6706f8e03a9552c521ce497a35b53bc7ea'));
for (const [name, digest] of Object.entries(q.inputs)) pin(name, digest);
pin(q.runner.name, q.runner.sha256); pin(q.collector.name, q.collector.sha256);
const p = json('r121-development-mutations-v2.json');
for (const [name, digest] of Object.entries(p.parser_inputs)) pin(name, digest);
const E = require('./r119-integrated-qualification-evidence-v1.js');
const map = json(p.source_map);
assert.strictEqual(sha(JSON.stringify(map)), q.source_map_sha256);
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
function identities() {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), q.source_parent);
  return Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(n => n && !n.startsWith('docs/')).sort().map(n => [n, sha(fs.readFileSync(path.join(repo, n)))]));
}
assert.deepStrictEqual(identities(), map);
const r = json(q.run + '.json');
E.checkRecord(r, q.run, q.command, map, q.predecessor, 0, {read: json, bytes}, {
  head: q.source_parent, cwd: repo, contract: 'r121-development-raw-utc-boot-monotonic-v1',
  runner: q.runner.name, after: map,
});
assert(r.elapsed_seconds >= 0 && r.elapsed_seconds < r.deadline_ms / 1000);
assert.deepStrictEqual(E.liveGroupMembers(r.process_group_cleanup.close.pgid), []);
assert(!fs.existsSync(root + q.run + '-gate-failure.json'));
const report = json(q.run + '.log');
assert.strictEqual(report.development_only, true);
assert.strictEqual(report.publication_pending, true);
assert.strictEqual(report.scope, 'R121 local CPU/test qualification only');
assert.strictEqual(report.source_map_sha256, q.source_map_sha256);
assert.strictEqual(report.source_identities, 5696);
assert.strictEqual(report.gnu_passed, 2818); assert.strictEqual(report.musl_passed, 2818);
assert.strictEqual(report.ignored_each, 5);
assert.strictEqual(report.source_gates, 17); assert.strictEqual(report.auxiliary_checks, 10);
assert.strictEqual(report.compiled_runtime_negatives, 26); assert.strictEqual(report.distinct_mutant_maps, 25);
assert.strictEqual(report.helper_calibrations, 30);
assert.deepStrictEqual(report.restored_suites, [59, 17]);
const artifacts = new Map();
function retain(name, digest, size) {
  assert.strictEqual(path.basename(name), name);
  assert(name.startsWith('r121-development-') || Object.hasOwn(p.parser_inputs, name));
  assert(!artifacts.has(name), 'unique archive artifact');
  const b = bytes(name);
  assert.strictEqual(sha(b), digest, name);
  if (size !== undefined) assert.strictEqual(b.length, size);
  artifacts.set(name, {name, sha256: digest, bytes: b.length});
}
for (const a of report.raw_artifacts) retain(a.name, a.sha256, a.bytes);
assert.strictEqual(artifacts.size, report.raw_artifacts.length);
for (const suffix of ['.json', '.log', '-source.json', '-source-after.json']) {
  const name = q.run + suffix; retain(name, sha(bytes(name)), bytes(name).length);
}
const collectorOutputs = new Set(['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json'].map(s => q.run + s));
const evidenceNames = () => fs.readdirSync(root)
  .filter(n => /^r121-development-.*\.(?:json|log|js|md|rs)$/.test(n) && !collectorOutputs.has(n)).sort();
const expectedNames = report.raw_artifacts.map(a => a.name)
  .filter(n => /^r121-development-.*\.(?:json|log|js|md|rs)$/.test(n) && !collectorOutputs.has(n)).sort();
assert.deepStrictEqual(evidenceNames(), expectedNames, 'exact report-derived evidence namespace');
// The collector excludes its own run outputs, so a closed replay is deterministic.
const replay = JSON.parse(cp.execFileSync('node', [root + q.collector.name],
  {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString());
assert.deepStrictEqual(replay, report, 'closed collector report replays exactly');
for (const [file, b] of captured) assert.deepStrictEqual(fs.readFileSync(file), b, 'stable source artifact');
assert.deepStrictEqual(identities(), map);
const artifactList = [...artifacts.values()].sort((a, b) => a.name.localeCompare(b.name));
const full = {harnesses: 48, passed: 2818, failed: 0, ignored: 5};
const summary = {
  source_head: q.source_parent, accepted_parent: q.source_parent,
  source_map_sha256: q.source_map_sha256, source_identities: 5696,
  scope: 'Ordinary dispatch and typed-data cleanup: local CPU/test qualification',
  full: {gnu: {...full}, musl: {...full}}, harnessless_csv_each: 1,
  full_runs: ['r121-development-gnu-all-v2', 'r121-development-musl-retry1'],
  source_gates: 17, auxiliary_gates: 10, focused: {controls: 59, data: 17},
  compiled_negatives: 26, distinct_mutations: 25, new_behavioral_tests: 21, new_source_guards: 2,
  helper_calibration_tests: 30,
  collector: {run: q.run, record_sha256: sha(bytes(q.run + '.json')), log_sha256: sha(bytes(q.run + '.log'))},
  retained_other_artifacts: report.retained_other_artifacts,
  rejected_attempts: ['r121-development-gnu-all-v1', 'r121-development-musl-all-v1', 'r121-development-gate-check-standalone-v3-rejected'],
  historical_source_map_sha256: '19b8774019ea6c5f70aa90cfdaa5458282c389f31201a584d098156b9e2e7332',
  formal_qualification: false, native_qualification: false, performance_qualification: false,
  total_memory_qualification: false, production_semantics_changed: true,
  archive_review_pending: true, artifacts: artifactList,
};
const readme = [
  '# R121 Ordinary Dispatch and Typed Data Cleanup Evidence', '',
  'Prepared local CPU/test evidence; independent archive review and publication are pending.',
  'GNU and the separately declared musl retry each pass 2,818 tests with five ignored across 48 libtest harnesses and one checked harnessless CSV target.',
  'All seventeen source gates, ten auxiliary checks, restored 59/17 cleanup suites and thirty transcript-parser calibrations pass.',
  'The 26 compiled runtime mutation executions cover 25 distinct production source maps. Each reaches its independently located behavioral assertion and restores all 5,696 source identities.',
  'The closed collector replays exactly. Its four closed run outputs are retained alongside every hash-checked raw artifact.',
  'The interrupted GNU attempt, musl timeout and original development cohort remain separate unaccepted history; none substitutes for a current complete gate.',
  'The original standalone-lockfile checker rejection is retained. The corrected parser permits only the exact Cargo package-cache wait line while requiring the unchanged complete 32-file transcript; raw gate evidence was not replaced.',
  'Endpoint equality is not continuous source immutability. These records do not establish live queue composition or teardown, generated native execution, authenticated formal refinement, aggregate memory bounds, multi-device acceptance or HIP/HSA performance parity.',
  '',
].join('\n');
assert(!fs.existsSync(destination), 'archive must not already exist');
fs.mkdirSync(destination);
fs.mkdirSync(path.join(destination, 'raw'));
for (const a of artifactList) fs.writeFileSync(path.join(destination, 'raw', a.name), bytes(a.name), {flag: 'wx'});
fs.writeFileSync(path.join(destination, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
fs.writeFileSync(path.join(destination, 'README.md'), readme, {flag: 'wx'});
assert.deepStrictEqual(fs.readdirSync(path.join(destination, 'raw')).sort(), artifactList.map(a => a.name).sort());
for (const a of artifactList) assert.strictEqual(sha(fs.readFileSync(path.join(destination, 'raw', a.name))), a.sha256);
assert.deepStrictEqual(JSON.parse(fs.readFileSync(path.join(destination, 'summary.json'))), summary);
assert.strictEqual(fs.readFileSync(path.join(destination, 'README.md'), 'utf8'), readme);
assert.deepStrictEqual(identities(), map);
for (const [file, b] of captured) assert.deepStrictEqual(fs.readFileSync(file), b, 'stable input after assembly');
assert.deepStrictEqual(evidenceNames(), expectedNames, 'stable evidence namespace after assembly');
assert(!fs.existsSync(root + q.run + '-gate-failure.json'));
console.log(JSON.stringify({archive_prepared: true, archive_review_pending: true, destination,
  artifacts: artifactList.length, source_map_sha256: q.source_map_sha256,
  summary_sha256: sha(fs.readFileSync(path.join(destination, 'summary.json')))}));



