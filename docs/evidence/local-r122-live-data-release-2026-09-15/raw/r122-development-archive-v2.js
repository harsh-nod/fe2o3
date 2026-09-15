// Assemble closed, replayed R122 CPU/test evidence without replacing an existing archive.
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
const q = JSON.parse(pin('r122-development-qualification-plan-v2.json',
  'bfad57f39ae3bf77ccc360685ea559675032ccd2a83b0488554ac24061ea6cf5'));
for (const [name, digest] of Object.entries(q.inputs)) pin(name, digest);
pin(q.runner.name, q.runner.sha256);
const collector = 'r122-development-qualification-collect-v2.js';
pin(collector, 'b796c42e7c3558886c7eba8dfbb45d0346e8a5f641a71fd5f8203c36859476d8');
bytes(__filename);
const C = require(root + q.mutation_checker), {repo, map} = C;
assert.deepStrictEqual(C.identities(), map);
const result = C.completed(q.collector_run, ['node', root + collector], q.collector_predecessor);
assert(!fs.existsSync(root + q.collector_run + '-gate-failure.json'));
const report = JSON.parse(result.log);
assert.strictEqual(report.development_only, true);
assert.strictEqual(report.publication_pending, true);
assert.strictEqual(report.scope, q.scope);
assert.strictEqual(report.source_parent, q.source_parent);
assert.strictEqual(report.source_map_sha256, q.source_map_sha256);
assert.strictEqual(report.source_identities, 5700);
assert.strictEqual(report.gnu_passed, 2846); assert.strictEqual(report.musl_passed, 2846);
assert.strictEqual(report.ignored_each, 5);
assert.strictEqual(report.libtest_targets_each, 48); assert.strictEqual(report.csv_targets_each, 1);
assert.strictEqual(report.source_gates, 17); assert.strictEqual(report.auxiliary_checks, 10);
assert.strictEqual(report.compiled_runtime_negatives, 12); assert.strictEqual(report.distinct_mutant_maps, 12);
assert.strictEqual(report.helper_calibrations, 89);
assert.deepStrictEqual(report.restored_suites, [35, 10]);
assert.deepStrictEqual(report.exclusions, q.exclusions);
const artifacts = new Map();
function retain(name, digest, size) {
  assert.strictEqual(path.basename(name), name);
  assert(name.startsWith('r122-development-') || Object.hasOwn(q.inputs, name));
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
  .filter(name => /^r122-development-.*\.(?:json|log|js|md|rs)$/.test(name) && !ownOutputs.has(name)).sort();
const expectedNames = report.raw_artifacts.map(item => item.name)
  .filter(name => name.startsWith('r122-development-') && !ownOutputs.has(name)).sort();
assert.deepStrictEqual(inventoryNames(), expectedNames);
const replay = JSON.parse(cp.execFileSync('node', [root + collector], {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString());
assert.deepStrictEqual(replay, report, 'complete closed collector replay');
const stable = () => {
  for (const [file, value] of inputs) assert.deepStrictEqual(fs.readFileSync(file), value, 'stable input: ' + file);
  C.stable(); assert.deepStrictEqual(C.identities(), map);
  assert.deepStrictEqual(inventoryNames(), expectedNames);
};
stable();
const artifactList = [...artifacts.values()].sort((a, b) => a.name.localeCompare(b.name));
const summary = {
  source_head: q.source_parent, accepted_parent: q.source_parent,
  source_map_sha256: q.source_map_sha256, source_identities: 5700,
  scope: 'Live detached-data release and runtime ownership: local CPU/test qualification',
  full: {gnu: {harnesses: 48, passed: 2846, failed: 0, ignored: 5},
    musl: {harnesses: 48, passed: 2846, failed: 0, ignored: 5}},
  harnessless_csv_each: 1,
  full_runs: ['r122-development-gnu-all-v1', 'r122-development-musl-all-v1'],
  source_gates: 17, auxiliary_gates: 10, focused: {kfd: 35, runtime: 10},
  compiled_negatives: 12, distinct_mutations: 12, helper_calibration_tests: 89,
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
  '# R122 Live Detached-Data Release Evidence', '',
  'Prepared local CPU/test evidence; independent archive review and publication are pending.',
  'GNU and musl each pass 2,846 tests with five ignored across 48 libtest harnesses and one separately checked CSV target.',
  'All seventeen source gates, ten auxiliary checks, restored 35/10 KFD/runtime suites and 89 checker-calibration cases pass.',
  'Twelve compiled production mutations reach their declared behavioral assertions with normal Cargo 101 exits, then restore all 5,700 source identities.',
  'The immutable source bundle retains the exact fifteen changed/new source files, including four added files; unchanged sources belong to the accepted parent.',
  'The collector replays exactly. Its four closed outputs are retained alongside every report-listed, hash-checked raw artifact.',
  'Older source cohorts, initial compile/test/lint failures and earlier forwarding negatives remain separate unqualified development history.',
  'No recursive archive import is used. Explicit parser dependencies and a historical Linux split-output fixture are retained; accepted R121 baselines are bound to committed paths and hashes.',
  'Endpoint equality is not continuous source immutability. CPU fixtures and checker calibrations do not establish live Linux/KFD execution, authenticated formal refinement, aggregate retained-memory bounds, multi-device acceptance or HIP/HSA performance parity.',
  '',
].join('\n');
const destination = path.join(repo, 'docs/evidence/local-r122-live-data-release-2026-09-15');
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
