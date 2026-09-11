const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '7506596f805af49e432aaa4ef66ec9a586ca4734';
const dest = path.join(repo, 'docs/evidence/local-r99-auxiliary-local-platform-2026-09-11');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 16 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const committed = (revision, name) => git(['show', parent + ':docs/evidence/' +
  (revision === 98 ? 'local-r98-completion-contract-2026-09-11/' : 'local-r97-auxiliary-outer-settlement-2026-09-11/') + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
const baseline = read('r99-frozen-source.json');
const paths = git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/'));
const current = Object.fromEntries([...new Set(paths)].sort().map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.deepStrictEqual(current, baseline);
assert.strictEqual(Object.keys(baseline).length, 5655);
const prior = JSON.parse(committed(98, 'raw/r98-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...Object.keys(baseline)])]
  .filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/queue_linux/primary_fixture.rs',
  'crates/fe2o3-kfd/src/queue_linux/primary_fixture_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_platform_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary/integration_prefix_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_platform.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_platform_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_tests.rs',
].sort());
const environment = read('r99-environment.json');
assert.deepStrictEqual(environment.binaries.map(b => b.name), ['cargo', 'rustc']);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r99-final-source-gate.json');
const auxiliary = read('r99-auxiliary-results.json');
assert.deepStrictEqual(gates.map(r => r.name), [
  'gnu-tests', 'musl-tests', 'gnu-docs', 'musl-docs', 'musl-host-docs', 'gnu-host',
  'musl-host', 'macro-fixtures', 'clippy-all', 'clippy-production', 'python', 'fmt',
  'whitespace', 'dependency-policy', 'dependency-tests', 'ci-test-gate', 'standalone-lockfiles',
]);
const previousGates = JSON.parse(committed(98, 'raw/r98-final-source-gate.json'));
for (const [i, row] of gates.entries()) {
  assert.deepStrictEqual(row.command, previousGates[i].command, row.name);
  assert.strictEqual(row.cwd, previousGates[i].cwd, row.name);
  assert.strictEqual(row.log, path.join(root, 'r99-final-' + row.name + '.log'));
}
const cargo = ['cargo', '+nightly-2026-04-03'];
const kfd = [...cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const previousAuxiliary = JSON.parse(committed(97, 'raw/r97-auxiliary-results.json'));
assert.deepStrictEqual(auxiliary.map(r => r.name), ['registration', 'ordinary', 'linux-helpers',
  'initialization', 'transitions', 'preparation', 'bind', 'proof-inventory', 'production-metadata', 'production-audit']);
for (const row of auxiliary) {
  const expected = row.name === 'registration' ? [...kfd, 'queue_linux::primary_fixture::tests::']
    : previousAuxiliary.find(r => r.name === row.name).command.map(s => s.replace('r97-production-metadata.log', 'r99-production-metadata.log'));
  assert.deepStrictEqual(row.command, expected, row.name);
  assert.strictEqual(row.cwd, repo);
  assert.strictEqual(row.log, path.join(root, 'r99-' + row.name + '.log'));
}
for (const row of [...gates, ...auxiliary]) assert.strictEqual(row.returncode, 0, row.name);
for (const [prefix, count] of [['r99-final', 17], ['r99-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5655});
}
const matrix = 'queue::live::construction_primary::integration_tests::auxiliary_cases::prefix_cases::platform_cases::same_engine_auxiliary_local_platform_matrix_retains_exact_primary_and_auxiliary_owners';
const registration = 'queue_linux::primary_fixture::tests::';
const newTests = [matrix, registration + 'local_runtime_registration_rejects_foreign_binding_without_mutation',
  registration + 'local_runtime_registrations_share_gate_with_single_queue_transition'];
const positiveCommands = {
  'r99-platform-first': [...kfd, matrix.split('::').at(-1)],
  'r99-registration-first': [...kfd, 'local_runtime_registration'],
  'r99-clippy-first': previousGates.find(r => r.name === 'clippy-all').command,
  'r99-frozen-construction': [...kfd, 'construction_primary::integration_tests'],
  'r99-restored-construction': [...kfd, 'queue::live::construction'],
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
const mutations = read('r99-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['lease-count', 'queue-phase', 'payload-cleanup', 'event-binding', 'auxiliary-phase']);
assert.deepStrictEqual(mutations.map(m => m.test), [newTests[2], matrix, matrix, matrix, matrix]);
for (const m of mutations) {
  assert.strictEqual(m.path, m.name === 'auxiliary-phase'
    ? 'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs' : 'crates/fe2o3-kfd/src/queue_linux.rs');
  const name = 'r99-mut-' + m.name;
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
const oldConstruction = passingNames(committed(97, 'raw/r97-frozen.log'));
assert.strictEqual(oldConstruction.length, 53);
for (const [name, expected] of [
  ['r99-frozen-construction', [...oldConstruction.filter(n => n.includes('construction_primary::integration_tests')), matrix]],
  ['r99-restored-construction', [...oldConstruction, matrix]],
  ['r99-platform-first', [matrix]], ['r99-registration-first', newTests.slice(1)],
]) {
  assert.deepStrictEqual(passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8')).sort(), expected.sort(), name);
  assert.deepStrictEqual(totals(path.join(root, name + '.log')), {harnesses: 1, passed: expected.length, failed: 0, ignored: 0});
}
for (const name of ['r99-final-gnu-tests', 'r99-final-musl-tests']) {
  const names = passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8'));
  for (const test of [...oldConstruction, ...newTests]) assert(names.includes(test), name + ': ' + test);
}
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(r.log)]));
const oldFull = JSON.parse(committed(98, 'test-summary.json')).tests;
const oldAuxiliary = JSON.parse(committed(97, 'test-summary.json')).tests;
for (const row of gates.filter(r => r.command.includes('test'))) assert.deepStrictEqual(tests[row.name], {
  ...oldFull[row.name], passed: oldFull[row.name].passed + (['gnu-tests', 'musl-tests'].includes(row.name) ? 3 : 0),
}, row.name);
for (const row of auxiliary.filter(r => r.command.includes('test'))) assert.deepStrictEqual(tests[row.name],
  row.name === 'registration' ? {harnesses: 1, passed: 2, failed: 0, ignored: 0} : oldAuxiliary[row.name], row.name);
assert.deepStrictEqual(read('r99-preflight-note.json'), {
  phase: 'runner invocation before frozen construction suite', attempt_name: 'r99-frozen',
  result: 'exclusive creation of r99-frozen-source.json rejected because that file already holds the frozen baseline',
  test_process_started: false, source_changed: false,
  correction: 'use the distinct r99-frozen-construction attempt name; preserve the existing baseline',
});
const summary = {
  source_parent: parent, scope: 'NATIVE-2B.5B-2 named CPU/local Linux helper composition',
  source_identities: 5655, changed_test_files: changed, source_gates: 17, auxiliary_checks: 10,
  compiled_behavioral_mutations: 5, new_test_functions: newTests,
  new_matrix: {scenarios: 17, original_runtime_routes: 2, auxiliary_runs: 34, successes: 2, failures: 32},
  frozen_integration_tests: 32, restored_construction_tests: 54, tests,
  environment_record: 'raw/r99-environment.json', live_kfd: false, solver_rerun: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r99-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
