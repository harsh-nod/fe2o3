const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = 'e6ac41c7fe61fbbb3e9a7003a8e9fd9a8a0c97a4';
const previous = 'docs/evidence/local-r99-auxiliary-local-platform-2026-09-11/';
const dest = path.join(repo, 'docs/evidence/local-r100-auxiliary-create-outcomes-2026-09-11');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 16 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const committed = name => git(['show', parent + ':' + previous + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
const baseline = read('r100-frozen-source.json');
const paths = git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/'));
const current = Object.fromEntries([...new Set(paths)].sort().map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.deepStrictEqual(current, baseline);
assert.strictEqual(Object.keys(baseline).length, 5656);
const prior = JSON.parse(committed('raw/r99-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...Object.keys(baseline)])]
  .filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_create_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_prefix_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_tests.rs',
]);
const environment = read('r100-environment.json');
assert.deepStrictEqual(environment.binaries.map(b => b.name), ['cargo', 'rustc']);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r100-final-source-gate.json');
const auxiliary = read('r100-auxiliary-results.json');
const oldGates = JSON.parse(committed('raw/r99-final-source-gate.json'));
const oldAuxiliary = JSON.parse(committed('raw/r99-auxiliary-results.json'));
assert.deepStrictEqual(gates.map(r => r.name), oldGates.map(r => r.name));
assert.deepStrictEqual(auxiliary.map(r => r.name), oldAuxiliary.map(r => r.name));
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 10);
for (const [rows, previousRows, prefix] of [[gates, oldGates, 'r100-final-'], [auxiliary, oldAuxiliary, 'r100-']]) {
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, previousRows[i].command.map(s => s.replace('r99-production-metadata.log', 'r100-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, previousRows[i].cwd, row.name);
    assert.strictEqual(row.log, path.join(root, prefix + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
  }
}
for (const [prefix, count] of [['r100-final', 17], ['r100-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5656});
}
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const prefix = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::create_cases::';
const newTests = [prefix + 'same_engine_auxiliary_create_outcomes_retain_admitted_prefix_and_exact_history',
  prefix + 'same_engine_auxiliary_malformed_create_outputs_retain_admitted_prefix_and_exact_history'];
const positiveCommands = {
  'r100-create-second': [...kfd, 'prefix_cases::create_cases'],
  'r100-frozen-construction': [...kfd, 'queue::live::construction'],
  'r100-restored-construction': [...kfd, 'queue::live::construction'],
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
const mutations = read('r100-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['create-result-propagation', 'returned-uncertain-id', 'invalid-output-phase', 'failed-noeffect-outputs']);
assert.deepStrictEqual(mutations.map(m => m.test), [newTests[0], newTests[0], newTests[1], newTests[1]]);
assert.deepStrictEqual(mutations.map(m => m.path), [
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
  'crates/fe2o3-runtime-model/src/queue_lifecycle.rs',
  'crates/fe2o3-kfd/src/queue.rs', 'crates/fe2o3-kfd/src/queue.rs',
]);
for (const m of mutations) {
  const name = 'r100-mut-' + m.name;
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
const oldConstruction = passingNames(committed('raw/r99-restored-construction.log'));
assert.strictEqual(oldConstruction.length, 54);
for (const [name, expected] of [
  ['r100-frozen-construction', [...oldConstruction, ...newTests]],
  ['r100-restored-construction', [...oldConstruction, ...newTests]],
  ['r100-create-second', newTests.slice()],
]) {
  assert.deepStrictEqual(passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8')).sort(), expected.sort(), name);
  assert.deepStrictEqual(totals(path.join(root, name + '.log')), {harnesses: 1, passed: expected.length, failed: 0, ignored: 0});
}
for (const name of ['r100-final-gnu-tests', 'r100-final-musl-tests']) {
  const names = passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8'));
  for (const test of [...oldConstruction, ...newTests]) assert(names.includes(test), name + ': ' + test);
}
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(r.log)]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const row of [...gates, ...auxiliary].filter(r => r.command.includes('test'))) assert.deepStrictEqual(tests[row.name], {
  ...oldTests[row.name], passed: oldTests[row.name].passed + (['gnu-tests', 'musl-tests'].includes(row.name) ? 2 : 0),
}, row.name);
const first = read('r100-create-first.json');
assert.deepStrictEqual(first.command, positiveCommands['r100-create-second']);
assert.strictEqual(first.returncode, 101);
assert.strictEqual(first.source_unchanged, true);
assert.strictEqual(first.cwd, repo);
assert.strictEqual(first.source, path.join(root, 'r100-create-first-source.json'));
assert.strictEqual(first.log, path.join(root, 'r100-create-first.log'));
const preliminary = read('r100-create-first-source.json');
assert.deepStrictEqual(Object.keys(preliminary).sort(), Object.keys(baseline).sort());
assert.deepStrictEqual(Object.keys(baseline).filter(p => baseline[p] !== preliminary[p]), [changed[0]]);
const corrected = fs.readFileSync(path.join(repo, changed[0]), 'utf8');
const replacement = '    assert!(root.authority.is_none());\n    assert!(\n        root.resource_prefix\n            .as_ref()\n            .unwrap()\n            .primary_fixture_identities_v1()\n            .is_empty()\n    );';
assert.strictEqual(corrected.split(replacement).length, 2);
assert.strictEqual(hash(corrected.replace(replacement, '    assert!(root.authority.is_none() && root.resource_prefix.is_none());')), preliminary[changed[0]]);
const firstLog = fs.readFileSync(first.log, 'utf8');
for (const marker of ['Finished `test` profile', '0 passed; 2 failed',
  'assertion failed: root.authority.is_none() && root.resource_prefix.is_none()',
  ...newTests.map(t => 'test ' + t + ' ... FAILED')]) assert(firstLog.includes(marker));
const note = read('r100-preliminary-note.json');
assert.strictEqual(note.attempt, 'r100-create-first');
assert.strictEqual(note.accepted_followup, 'r100-create-second');
assert.strictEqual(note.production_change, false);
const summary = {
  source_parent: parent, scope: 'NATIVE-2B.5B-3A named CPU/local-helper admitted CREATE outcome matrix',
  source_identities: 5656, changed_test_files: changed, source_gates: 17, auxiliary_checks: 10,
  compiled_behavioral_mutations: 4, new_test_functions: newTests,
  new_matrix: {scenarios: 7, original_runtime_routes: 2, auxiliary_runs: 14, successes: 0, failures: 14},
  frozen_construction_tests: 56, restored_construction_tests: 56, tests,
  preliminary_failure: 'raw/r100-create-first.json', environment_record: 'raw/r100-environment.json',
  live_kfd: false, solver_rerun: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r100-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
