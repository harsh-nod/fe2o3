// Assemble closed, replayed R125 CPU/test evidence without replacing an archive.
// Create this helper before collection, then execute it only after collector closure.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const inputs = new Map();
function bytes(name) {
  const file = path.isAbsolute(name) ? name : root + name;
  assert(fs.lstatSync(file).isFile(), 'regular input');
  if (!inputs.has(file)) inputs.set(file, fs.readFileSync(file));
  return inputs.get(file);
}
function pin(name, digest) {
  const value = bytes(name); assert.strictEqual(hash(value), digest, name); return value;
}
const q = JSON.parse(pin('r125-development-qualification-plan-v1.json',
  '9760e1c22a227b52d90df87d99aeeaad6cc38803ccf57669b23af00a3de0be07'));
for (const [name, digest] of Object.entries(q.inputs)) pin(name, digest);
pin(q.runner.name, q.runner.sha256);
const collector = 'r125-development-qualification-collect-v1.js';
assert.strictEqual(q.collector, collector);
pin(collector, 'd928eb37b1a551e13691e38c48f44fbdaee2758cf13089e7d394149f85b8299a');
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
assert.strictEqual(report.source_identities, 5709);
assert.strictEqual(report.gnu_passed, 2900); assert.strictEqual(report.musl_passed, 2900);
assert.strictEqual(report.ignored_each, 5);
assert.strictEqual(report.libtest_targets_each, 48); assert.strictEqual(report.csv_targets_each, 1);
assert.strictEqual(report.source_gates, 17); assert.strictEqual(report.auxiliary_checks, 10);
assert.strictEqual(report.compiled_runtime_negatives, 18); assert.strictEqual(report.distinct_mutant_maps, 18);
assert.strictEqual(report.helper_calibrations, 205);
assert.strictEqual(report.execution_boot, C.p.admitted_boot);
assert(report.owned_group_observations.every(item =>
  item.boot_id === C.p.admitted_boot && item.closure === 'current_boot_observed_absent'));
assert.deepStrictEqual(report.source_bundle, {
  name: q.source_bundle, sha256: hash(bytes(q.source_bundle)), changed: 16, added: 3,
});
assert.deepStrictEqual(report.restored_suites, [46, 744]);
assert.deepStrictEqual(report.exclusions, q.exclusions);
const runNames = [q.source_plan_run.name, q.source_snapshot.name,
  ...q.calibrations.map(spec => spec.name), ...C.p.full_runs.map(spec => spec.name),
  q.gnu_check.name, q.full_check.name, ...C.p.gates.map(spec => spec.run), q.gate_check.name,
  ...q.mutations.flatMap(item => [item.negative, item.check, item.restored]),
  ...q.restored.map(spec => spec.name)].sort();
assert.strictEqual(runNames.length, 91);
assert.strictEqual(new Set(runNames).size, runNames.length);
assert.deepStrictEqual(report.runs.map(run => run.name).sort(), runNames);
const artifacts = new Map();
function retain(name, digest, size) {
  assert.strictEqual(path.basename(name), name);
  assert(name.startsWith('r125-development-') || Object.hasOwn(q.inputs, name));
  assert(!artifacts.has(name), 'unique archive artifact');
  const value = bytes(name);
  assert.strictEqual(hash(value), digest, name);
  if (size !== undefined) assert.strictEqual(value.length, size);
  artifacts.set(name, {name, sha256: digest, bytes: value.length});
}
for (const item of report.raw_artifacts) retain(item.name, item.sha256, item.bytes);
assert.strictEqual(artifacts.size, report.raw_artifacts.length);
assert(artifacts.has(path.basename(__filename)), 'archive builder retained before collection');
for (const name of ['r125-development-qualification-plan-v1.json', collector,
  q.runner.name, ...Object.keys(q.inputs)]) assert(artifacts.has(name), 'retained required input: ' + name);
for (const name of runNames) {
  for (const suffix of ['.json', '.log', '-source.json', '-source-after.json'])
    assert(artifacts.has(name + suffix), 'complete qualifying run: ' + name);
}
for (const suffix of ['.json', '.log', '-source.json', '-source-after.json']) {
  const name = q.collector_run + suffix; retain(name, hash(bytes(name)), bytes(name).length);
}
const ownOutputs = new Set(['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']
  .map(suffix => q.collector_run + suffix));
const inventoryNames = () => fs.readdirSync(root)
  .filter(name => /^r125-development-.*\.(?:json|log|js|md|rs)$/.test(name) && !ownOutputs.has(name)).sort();
const expectedNames = report.raw_artifacts.map(item => item.name)
  .filter(name => name.startsWith('r125-development-') && !ownOutputs.has(name)).sort();
assert.deepStrictEqual(inventoryNames(), expectedNames);
const replay = JSON.parse(cp.execFileSync('node', [root + collector],
  {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString());
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
  source_map_sha256: q.source_map_sha256, source_identities: 5709,
  scope: 'Live persistent cancellation and boxed-ledger repair: local CPU/test qualification',
  full: {gnu: {harnesses: 48, passed: 2900, failed: 0, ignored: 5},
    musl: {harnesses: 48, passed: 2900, failed: 0, ignored: 5}},
  harnessless_csv_each: 1, full_runs: C.p.full_runs.map(spec => spec.name),
  source_gates: 17, auxiliary_gates: 10, focused: {kfd: 46, runtime: 744, rosters: [46, 744]},
  compiled_negatives: 18, distinct_mutations: 18, helper_calibration_tests: 205,
  execution_boot: report.execution_boot, source_bundle: report.source_bundle,
  collector: {run: q.collector_run, record_sha256: hash(bytes(q.collector_run + '.json')),
    log_sha256: hash(bytes(q.collector_run + '.log'))},
  retained_other_artifacts: report.retained_other_artifacts,
  historical_runs: report.historical_runs, owned_group_observations: report.owned_group_observations,
  formal_qualification: false, native_qualification: false, performance_qualification: false,
  total_memory_qualification: false, production_semantics_changed: true,
  archive_review_pending: true, artifacts: artifactList,
};
const readme = [
  '# R125 Live Persistent Cancellation Evidence', '',
  'Prepared local CPU/test evidence; independent archive review and publication are pending.',
  'GNU and musl each pass 2,900 tests with five ignored across 48 libtest harnesses and one separately checked CSV target.',
  'All seventeen source gates, ten auxiliary checks, restored 46-test KFD and 744-test runtime suites, and 205 fresh checker-calibration cases pass.',
  'Eighteen compiled production mutations reach their declared behavioral assertions with normal Cargo 101 exits, then restore all 5,709 source identities.',
  'The mutation inventory does not claim mutation coverage of the added boxed-ledger layout and owner-move tests.',
  'The immutable source bundle retains the exact sixteen changed/new source files, including three added files; unchanged sources belong to the accepted R124 parent.',
  'The GNU and full-musl interstitial checker records are explicitly validated and replayed, not accepted merely through predecessor hashes.',
  'The collector replays exactly. Its four closed outputs are retained alongside every report-listed, hash-checked raw artifact.',
  'Earlier failed GNU and stack-overflow attempts, intermediate source cohorts, and development checks remain separate nonqualifying history.',
  'No recursive archive import is used. Explicit parser dependencies, including the retained Linux-helper calibration log, are archived; R124 baselines are bound to committed paths and hashes.',
  'All qualifying records use one admitted boot and monotonic timing. Current-boot groups are observed absent; endpoint checks do not authenticate continuous process or source history.',
  'The boxed-ledger measurements concern inline type sizes, not maximum call-stack or aggregate retained-memory bounds.',
  'CPU fixtures and checker calibrations do not establish live Linux/KFD execution, authenticated formal refinement, total memory bounds, multi-device acceptance or HIP/HSA performance parity.',
  'Cold three-binding output admission, A1/A2 and issue #182 remain incomplete.', '',
].join('\n');
const destination = path.join(repo, 'docs/evidence/local-r125-live-persistent-cancel-2026-09-15');
assert.throws(() => fs.lstatSync(destination), error => error.code === 'ENOENT', 'archive must not already exist');
fs.mkdirSync(destination); fs.mkdirSync(path.join(destination, 'raw'));
for (const item of artifactList)
  fs.writeFileSync(path.join(destination, 'raw', item.name), bytes(item.name), {flag: 'wx'});
fs.writeFileSync(path.join(destination, 'summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
fs.writeFileSync(path.join(destination, 'README.md'), readme, {flag: 'wx'});
assert.deepStrictEqual(fs.readdirSync(path.join(destination, 'raw')).sort(), artifactList.map(item => item.name).sort());
for (const item of artifactList)
  assert.deepStrictEqual(fs.readFileSync(path.join(destination, 'raw', item.name)), bytes(item.name));
assert.deepStrictEqual(JSON.parse(fs.readFileSync(path.join(destination, 'summary.json'))), summary);
assert.strictEqual(fs.readFileSync(path.join(destination, 'README.md'), 'utf8'), readme);
stable();
console.log(JSON.stringify({archive_prepared: true, archive_review_pending: true, destination,
  artifacts: artifactList.length, source_map_sha256: q.source_map_sha256,
  summary_sha256: hash(fs.readFileSync(path.join(destination, 'summary.json')))}));
