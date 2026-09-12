const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '77ce1196f2867e79eb450b5a9ba5924ed13152fa';
const previous = 'docs/evidence/local-r101-auxiliary-recovery-currentness-2026-09-11/';
const dest = path.join(repo, 'docs/evidence/local-r102-auxiliary-roster-slots-2026-09-12');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 16 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const committed = name => git(['show', parent + ':' + previous + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
const baseline = read('r102-frozen-source.json');
const paths = git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/'));
const current = Object.fromEntries([...new Set(paths)].sort().map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.deepStrictEqual(current, baseline);
assert.strictEqual(Object.keys(baseline).length, 5658);
const prior = JSON.parse(committed('raw/r101-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...Object.keys(baseline)])]
  .filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_prefix_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_roster_slot_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_tests.rs',
  'crates/fe2o3-kfd/src/sdma.rs',
]);
const environment = read('r102-environment.json');
const resumedEnvironment = read('r102-environment-resumed.json');
assert.deepStrictEqual(resumedEnvironment.binaries, environment.binaries);
assert.deepStrictEqual(resumedEnvironment.records, environment.records);
assert.deepStrictEqual(environment.binaries.map(b => b.name), ['cargo', 'rustc']);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r102-final-source-gate.json');
const auxiliary = read('r102-auxiliary-results.json');
const oldGates = JSON.parse(committed('raw/r101-final-source-gate.json'));
const oldAuxiliary = JSON.parse(committed('raw/r101-auxiliary-results.json'));
assert.deepStrictEqual(gates.map(r => r.name), oldGates.map(r => r.name));
assert.deepStrictEqual(auxiliary.map(r => r.name), oldAuxiliary.map(r => r.name));
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 10);
for (const [rows, oldRows, prefix] of [[gates, oldGates, 'r102-final-'], [auxiliary, oldAuxiliary, 'r102-']]) {
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[i].command.map(s => s.replace('r101-production-metadata.log', 'r102-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, oldRows[i].cwd, row.name);
    assert.strictEqual(row.log, path.join(root, prefix + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
  }
}
for (const [prefix, count] of [['r102-final', 17], ['r102-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5658});
}
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const prefix = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::roster_slot_cases::';
const newTests = [prefix + 'same_engine_auxiliary_retained_roster_collisions_keep_exact_unassembled_owners',
  prefix + 'same_engine_auxiliary_slot_reuse_and_destination_rejections_keep_exact_owners'];
const positiveCommands = {
  'r102-roster-slot-first': [...kfd, 'prefix_cases::roster_slot_cases'],
  'r102-frozen-construction-final': [...kfd, 'queue::live::construction'],
  'r102-restored-construction': [...kfd, 'queue::live::construction'],
};
for (const [name, command] of Object.entries(positiveCommands)) {
  const run = read(name + '.json');
  assert.deepStrictEqual(run.command, command, name);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  assert.strictEqual(run.returncode, 0);
  assert.strictEqual(run.source_unchanged, true);
  assert.deepStrictEqual(read(name + '-source.json'), baseline);
}
for (const [attempt, record] of [
  ['r102-frozen-construction', 'r102-frozen-interruption.json'],
  ['r102-frozen-construction-retry', 'r102-frozen-retry-interruption.json'],
]) {
  assert.deepStrictEqual(read(attempt + '-source.json'), baseline);
  assert.strictEqual(read(record).accepted, false);
  assert.strictEqual(read(record).attempt, attempt);
  assert(!fs.existsSync(path.join(root, attempt + '.json')), 'no completed runner record');
  assert(!fs.readFileSync(path.join(root, attempt + '.log'), 'utf8').includes('test result:'), 'interrupted, not accepted');
}
const mutations = read('r102-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['auxiliary-collision', 'directional-collision', 'striped-collision', 'prepared-generation', 'append-reservation', 'reuse-generation']);
assert.deepStrictEqual(mutations.map(m => m.test), [newTests[0], newTests[0], newTests[0], newTests[1], newTests[1], newTests[1]]);
assert.deepStrictEqual(mutations.map(m => m.path), [
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live.rs',
]);
for (const m of mutations) {
  const name = 'r102-mut-' + m.name;
  const run = read(name + '.json');
  assert.strictEqual(run.returncode, 101);
  assert.strictEqual(run.source_unchanged, true);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  assert.deepStrictEqual(run.command, [...kfd, m.test, '--', '--exact']);
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), Object.keys(baseline).sort());
  assert.deepStrictEqual(Object.keys(baseline).filter(p => inputs[p] !== baseline[p]), [m.path]);
  let candidate = fs.readFileSync(path.join(repo, m.path), 'utf8');
  for (const [before, after] of m.edits) {
    assert.strictEqual(candidate.split(before).length, 2);
    candidate = candidate.replace(before, after);
    assert.strictEqual(candidate.split(after).length, 2);
  }
  assert.strictEqual(hash(candidate), inputs[m.path]);
  const log = fs.readFileSync(run.log, 'utf8');
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test',
    'test ' + m.test + ' ... FAILED', ...m.expected, '0 passed; 1 failed']) assert(log.includes(marker), name + ': ' + marker);
}
const totals = logfile => {
  const matches = [...fs.readFileSync(logfile, 'utf8').matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)];
  assert(matches.length, 'nonempty suite: ' + logfile);
  return matches.reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}), {harnesses: 0, passed: 0, failed: 0, ignored: 0});
};
const passingNames = text => [...text.matchAll(/^test (\S+) \.\.\. ok$/gm)].map(m => m[1]);
const oldConstruction = passingNames(committed('raw/r101-restored-construction.log'));
assert.strictEqual(oldConstruction.length, 58);
for (const [name, expected] of [
  ['r102-frozen-construction-final', [...oldConstruction, ...newTests]],
  ['r102-restored-construction', [...oldConstruction, ...newTests]],
  ['r102-roster-slot-first', newTests.slice()],
]) {
  assert.deepStrictEqual(passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8')).sort(), expected.sort(), name);
  assert.deepStrictEqual(totals(path.join(root, name + '.log')), {harnesses: 1, passed: expected.length, failed: 0, ignored: 0});
}
for (const name of ['r102-final-gnu-tests', 'r102-final-musl-tests']) {
  const names = passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8'));
  for (const test of [...oldConstruction, ...newTests]) assert(names.includes(test), name + ': ' + test);
}
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(r.log)]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const row of [...gates, ...auxiliary].filter(r => r.command.includes('test'))) assert.deepStrictEqual(tests[row.name], {
  ...oldTests[row.name], passed: oldTests[row.name].passed + (['gnu-tests', 'musl-tests'].includes(row.name) ? 2 : 0),
}, row.name);
const summary = {
  source_parent: parent, scope: 'NATIVE-2B.5B-3C named CPU/local-helper roster/destination-slot matrix',
  source_identities: 5658, changed_test_files: changed, source_gates: 17, auxiliary_checks: 10,
  compiled_behavioral_mutations: 6, new_test_functions: newTests,
  new_matrix: {original_runtime_routes: 2, retained_roster_rejections: 6, late_slot_rejections: 8,
    successful_reuse_baselines: 2, auxiliary_runs: 16, borrowed_preflight_rejections: 4},
  frozen_construction_tests: 60, restored_construction_tests: 60, tests,
  environment_record: 'raw/r102-environment.json',
  resumed_environment_record: 'raw/r102-environment-resumed.json',
  interrupted_construction_attempts_not_accepted: 2,
  live_kfd: false, solver_rerun: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r102-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
