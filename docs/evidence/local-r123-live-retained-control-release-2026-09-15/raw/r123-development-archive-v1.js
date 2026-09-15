// Assemble closed, replayed R123 CPU/test evidence without replacing an existing archive.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/', hash = value => crypto.createHash('sha256').update(value).digest('hex');
const inputs = new Map();
function bytes(name) {
  const file = path.isAbsolute(name) ? name : root + name;
  assert(fs.lstatSync(file).isFile(), 'regular input');
  if (!inputs.has(file)) inputs.set(file, fs.readFileSync(file));
  return inputs.get(file);
}
function pin(name, digest) { const value = bytes(name); assert.strictEqual(hash(value), digest, name); return value; }
const q = JSON.parse(pin('r123-development-qualification-plan-v1.json',
  '2b71669ffc7782bb30274f68dd2e651f57fa8efbbd825a0b591b94e657adad93'));
for (const [name, digest] of Object.entries(q.inputs)) pin(name, digest);
pin(q.runner.name, q.runner.sha256);
const collector = 'r123-development-qualification-collect-v1.js';
pin(collector, '46df82559a7a42b2a4e965ab9739c09579e9230813ec6c8e2f407900699220e9');
bytes(__filename);
const M = require(root + q.mutation_checker), C = M.C, {repo, map} = C;
assert.strictEqual(require(root + q.gate_checker).C, C);
assert.deepStrictEqual(C.identities(), map);
const result = C.completed(q.collector_run, ['node', root + collector], q.collector_predecessor);
function requireNoCollectorFailure() {
  assert.throws(() => fs.lstatSync(root + q.collector_run + '-gate-failure.json'),
    error => error.code === 'ENOENT', 'collector has no failure marker, including dangling symlinks');
}
requireNoCollectorFailure();
const report = JSON.parse(result.log);
assert.strictEqual(report.development_only, true);
assert.strictEqual(report.publication_pending, true);
assert.strictEqual(report.scope, q.scope);
assert.strictEqual(report.source_parent, q.source_parent);
assert.strictEqual(report.source_map_sha256, q.source_map_sha256);
assert.strictEqual(report.source_identities, 5703);
assert.strictEqual(report.gnu_passed, 2865); assert.strictEqual(report.musl_passed, 2865);
assert.strictEqual(report.ignored_each, 5);
assert.strictEqual(report.libtest_targets_each, 48); assert.strictEqual(report.csv_targets_each, 1);
assert.strictEqual(report.source_gates, 17); assert.strictEqual(report.auxiliary_checks, 10);
assert.strictEqual(report.compiled_runtime_negatives, 10); assert.strictEqual(report.distinct_mutant_maps, 10);
assert.strictEqual(report.helper_calibrations, 134);
assert.deepStrictEqual(report.source_bundle, {name: q.source_bundle, sha256: hash(bytes(q.source_bundle)), changed: 19, added: 3});
assert.deepStrictEqual(report.restored_suites, [128, 17]);
assert.deepStrictEqual(report.exclusions, q.exclusions);
const artifacts = new Map();
function retain(name, digest, size) {
  assert.strictEqual(path.basename(name), name);
  assert(name.startsWith('r123-development-') || Object.hasOwn(q.inputs, name));
  assert(!artifacts.has(name), 'unique archive artifact');
  const value = bytes(name);
  assert.strictEqual(hash(value), digest, name);
  if (size !== undefined) assert.strictEqual(value.length, size);
  artifacts.set(name, {name, sha256: digest, bytes: value.length});
}
for (const item of report.raw_artifacts) retain(item.name, item.sha256, item.bytes);
assert.strictEqual(artifacts.size, report.raw_artifacts.length);
for (const suffix of ['.json', '.log', '-source.json', '-source-after.json']) {
  const name = q.collector_run + suffix; retain(name, hash(bytes(name)), bytes(name).length);
}
const ownOutputs = new Set(['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']
  .map(suffix => q.collector_run + suffix));
const inventoryNames = () => fs.readdirSync(root)
  .filter(name => /^r123-development-.*\.(?:json|log|js|md|rs)$/.test(name) && !ownOutputs.has(name)).sort();
const expectedNames = report.raw_artifacts.map(item => item.name)
  .filter(name => name.startsWith('r123-development-') && !ownOutputs.has(name)).sort();
assert.deepStrictEqual(inventoryNames(), expectedNames);
const replay = JSON.parse(cp.execFileSync('node', [root + collector], {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString());
assert.deepStrictEqual(replay, report, 'complete closed collector replay');
const stable = () => {
  requireNoCollectorFailure();
  for (const [file, value] of inputs) assert.deepStrictEqual(fs.readFileSync(file), value, 'stable input: ' + file);
  C.stable(); assert.deepStrictEqual(C.identities(), map);
  assert.deepStrictEqual(inventoryNames(), expectedNames);
};
stable();
const artifactList = [...artifacts.values()].sort((a, b) => a.name.localeCompare(b.name));
const summary = {
  source_head: q.source_parent, accepted_parent: q.source_parent,
  source_map_sha256: q.source_map_sha256, source_identities: 5703,
  scope: 'Live retained-control release and runtime ownership: local CPU/test qualification',
  full: {gnu: {harnesses: 48, passed: 2865, failed: 0, ignored: 5},
    musl: {harnesses: 48, passed: 2865, failed: 0, ignored: 5}},
  harnessless_csv_each: 1,
  full_runs: ['r123-development-gnu-all-v1', 'r123-development-musl-all-v1'],
  source_gates: 17, auxiliary_gates: 10, focused: {kfd: 145, rosters: [128, 17]},
  compiled_negatives: 10, distinct_mutations: 10, helper_calibration_tests: 134,
  source_bundle: report.source_bundle,
  collector: {run: q.collector_run, record_sha256: hash(bytes(q.collector_run + '.json')),
    log_sha256: hash(bytes(q.collector_run + '.log'))},
  retained_other_artifacts: report.retained_other_artifacts,
  historical_runs: report.historical_runs, closed_owned_groups: report.closed_owned_groups,
  formal_qualification: false, native_qualification: false, performance_qualification: false,
  total_memory_qualification: false, production_semantics_changed: true,
  archive_review_pending: true, artifacts: artifactList,
};
const readme = [
  '# R123 Live Retained-Control Release Evidence', '',
  'Prepared local CPU/test evidence; independent archive review and publication are pending.',
  'GNU and musl each pass 2,865 tests with five ignored across 48 libtest harnesses and one separately checked CSV target.',
  'All seventeen source gates, ten auxiliary checks, restored disjoint 128/17 KFD suites and 134 final checker-calibration cases pass.',
  'Ten compiled production mutations reach their declared behavioral assertions with normal Cargo 101 exits, then restore all 5,703 source identities.',
  'The immutable source bundle retains the exact nineteen changed/new source files, including three added files; unchanged sources belong to the accepted parent.',
  'The collector replays exactly. Its four closed outputs are retained alongside every report-listed, hash-checked raw artifact.',
  'Older source cohorts, initial compile/test/lint failures and the superseded 30-case gate checker remain separate unqualified development history.',
  'No recursive archive import is used. Explicit parser dependencies and a historical Linux split-output fixture are retained; accepted R122 baselines are bound to committed paths and hashes.',
  'Endpoint equality is not continuous source immutability. CPU fixtures and checker calibrations do not establish live Linux/KFD execution, authenticated formal refinement, aggregate retained-memory bounds, multi-device acceptance or HIP/HSA performance parity.',
  '',
].join('\n');
const destination = path.join(repo, 'docs/evidence/local-r123-live-retained-control-release-2026-09-15');
assert(!fs.existsSync(destination), 'archive must not already exist');
fs.mkdirSync(destination); fs.mkdirSync(path.join(destination, 'raw'));
for (const item of artifactList) fs.writeFileSync(path.join(destination, 'raw', item.name), bytes(item.name), {flag: 'wx'});
fs.writeFileSync(path.join(destination, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
fs.writeFileSync(path.join(destination, 'README.md'), readme, {flag: 'wx'});
assert.deepStrictEqual(fs.readdirSync(path.join(destination, 'raw')).sort(), artifactList.map(item => item.name).sort());
for (const item of artifactList) assert.deepStrictEqual(fs.readFileSync(path.join(destination, 'raw', item.name)), bytes(item.name));
assert.deepStrictEqual(JSON.parse(fs.readFileSync(path.join(destination, 'summary.json'))), summary);
assert.strictEqual(fs.readFileSync(path.join(destination, 'README.md'), 'utf8'), readme);
stable();
console.log(JSON.stringify({archive_prepared: true, archive_review_pending: true, destination,
  artifacts: artifactList.length, source_map_sha256: q.source_map_sha256,
  summary_sha256: hash(fs.readFileSync(path.join(destination, 'summary.json')))}));
