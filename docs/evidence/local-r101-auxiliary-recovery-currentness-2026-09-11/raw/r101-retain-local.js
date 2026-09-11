const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '4424f4607d8a64677556b32713a74b1ca5c6557a';
const previous = 'docs/evidence/local-r100-auxiliary-create-outcomes-2026-09-11/';
const dest = path.join(repo, 'docs/evidence/local-r101-auxiliary-recovery-currentness-2026-09-11');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 16 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const committed = name => git(['show', parent + ':' + previous + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
const baseline = read('r101-frozen-source.json');
const paths = git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/'));
const current = Object.fromEntries([...new Set(paths)].sort().map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.deepStrictEqual(current, baseline);
assert.strictEqual(Object.keys(baseline).length, 5657);
const prior = JSON.parse(committed('raw/r100-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...Object.keys(baseline)])]
  .filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_prefix_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_recovery_tests.rs',
]);
const environment = read('r101-environment.json');
assert.deepStrictEqual(environment.binaries.map(b => b.name), ['cargo', 'rustc']);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r101-final-source-gate.json');
const auxiliary = read('r101-auxiliary-results.json');
const oldGates = JSON.parse(committed('raw/r100-final-source-gate.json'));
const oldAuxiliary = JSON.parse(committed('raw/r100-auxiliary-results.json'));
assert.deepStrictEqual(gates.map(r => r.name), oldGates.map(r => r.name));
assert.deepStrictEqual(auxiliary.map(r => r.name), oldAuxiliary.map(r => r.name));
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 10);
for (const [rows, oldRows, prefix] of [[gates, oldGates, 'r101-final-'], [auxiliary, oldAuxiliary, 'r101-']]) {
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[i].command.map(s => s.replace('r100-production-metadata.log', 'r101-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, oldRows[i].cwd, row.name);
    assert.strictEqual(row.log, path.join(root, prefix + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
  }
}
for (const [prefix, count] of [['r101-final', 17], ['r101-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5657});
}
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const prefix = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::recovery_cases::';
const newTests = [prefix + 'same_engine_auxiliary_currentness_boundaries_retain_exact_history_and_phase_owners',
  prefix + 'same_engine_auxiliary_recovery_failures_keep_exact_outputs_before_owner_assembly'];
const positiveCommands = {
  'r101-recovery-first': [...kfd, 'prefix_cases::recovery_cases'],
  'r101-frozen-construction': [...kfd, 'queue::live::construction'],
  'r101-restored-construction': [...kfd, 'queue::live::construction'],
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
const mutations = read('r101-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['pre-doorbell-error', 'post-doorbell-error', 'primary-quarantine', 'recovered-output-placement']);
assert.deepStrictEqual(mutations.map(m => m.test), [newTests[0], newTests[0], newTests[0], newTests[1]]);
assert.deepStrictEqual(mutations.map(m => m.path), [
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
  'crates/fe2o3-kfd/src/queue.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
]);
for (const m of mutations) {
  const name = 'r101-mut-' + m.name;
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
const oldConstruction = passingNames(committed('raw/r100-restored-construction.log'));
assert.strictEqual(oldConstruction.length, 56);
for (const [name, expected] of [
  ['r101-frozen-construction', [...oldConstruction, ...newTests]],
  ['r101-restored-construction', [...oldConstruction, ...newTests]],
  ['r101-recovery-first', newTests.slice()],
]) {
  assert.deepStrictEqual(passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8')).sort(), expected.sort(), name);
  assert.deepStrictEqual(totals(path.join(root, name + '.log')), {harnesses: 1, passed: expected.length, failed: 0, ignored: 0});
}
for (const name of ['r101-final-gnu-tests', 'r101-final-musl-tests']) {
  const names = passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8'));
  for (const test of [...oldConstruction, ...newTests]) assert(names.includes(test), name + ': ' + test);
}
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(r.log)]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const row of [...gates, ...auxiliary].filter(r => r.command.includes('test'))) assert.deepStrictEqual(tests[row.name], {
  ...oldTests[row.name], passed: oldTests[row.name].passed + (['gnu-tests', 'musl-tests'].includes(row.name) ? 2 : 0),
}, row.name);
const summary = {
  source_parent: parent, scope: 'NATIVE-2B.5B-3B named CPU/local-helper recovery/currentness matrix',
  source_identities: 5657, changed_test_files: changed, source_gates: 17, auxiliary_checks: 10,
  compiled_behavioral_mutations: 4, new_test_functions: newTests,
  new_matrix: {original_runtime_routes: 2, currentness_failures: 16, callback_failures: 14,
    successful_trace_baselines: 2, auxiliary_runs: 32},
  frozen_construction_tests: 58, restored_construction_tests: 58, tests,
  environment_record: 'raw/r101-environment.json',
  live_kfd: false, solver_rerun: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r101-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
