const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '1b53ef417d0f4184e2b4e6024b37271b5f719832';
const dest = path.join(repo, 'docs/evidence/local-r98-completion-contract-2026-09-11');
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const committed = name => JSON.parse(cp.execFileSync('git', ['show', parent + ':' +
  'docs/evidence/local-r97-auxiliary-outer-settlement-2026-09-11/' + name],
  {cwd: repo, maxBuffer: 8 * 1024 * 1024}).toString());
assert.strictEqual(cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim(), parent);
const baseline = read('r98-frozen-source.json');
const paths = cp.execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'],
  {cwd: repo}).toString().split('\0').filter(p => p && !p.startsWith('docs/'));
const current = Object.fromEntries([...new Set(paths)].sort().map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.deepStrictEqual(current, baseline);
assert.strictEqual(Object.keys(baseline).length, 5653);
const environment = read('r98-environment.json');
assert.deepStrictEqual(environment.binaries.map(b => b.name), ['cargo', 'rustc']);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r98-final-source-gate.json');
const auxiliary = read('r98-auxiliary-results.json');
assert.deepStrictEqual(gates.map(r => r.name), [
  'gnu-tests', 'musl-tests', 'gnu-docs', 'musl-docs', 'musl-host-docs', 'gnu-host',
  'musl-host', 'macro-fixtures', 'clippy-all', 'clippy-production', 'python', 'fmt',
  'whitespace', 'dependency-policy', 'dependency-tests', 'ci-test-gate', 'standalone-lockfiles',
]);
const previousGates = committed('raw/r97-final-source-gate.json');
for (const [i, row] of gates.entries()) {
  assert.deepStrictEqual(row.command, previousGates[i].command, row.name);
  assert.strictEqual(row.cwd, previousGates[i].cwd, row.name);
  assert.strictEqual(row.log, path.join(root, 'r98-final-' + row.name + '.log'));
}
const cargo = ['cargo', '+nightly-2026-04-03'];
const tests = [...cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-runtime', '--all-features', '--lib'];
const terminalTest = 'async_engine::tests::owned_tests::local_operation_tests::terminal_context_cannot_promote_driver_retirement';
const auxiliaryCommands = [
  ...['co1_', 'async_engine::tests::owned_tests::control_tests::',
    'async_engine::tests::owned_tests::', 'drn3a_'].map(filter => [...tests, filter]),
  [...tests, terminalTest, '--', '--exact'],
  ['python3', '-I', 'crates/fe2o3-runtime-model/verus/check-negative-quality.py',
    'crates/fe2o3-runtime-model/verus/negative', 'crates/fe2o3-runtime-model/verus/verify-verus.sh'],
  [...cargo, 'metadata', '--locked', '--offline', '--no-default-features', '--format-version', '1',
    '--filter-platform', 'x86_64-unknown-linux-musl'],
  ['python3', '-B', 'scripts/runtime_pure_rust_audit.py', 'metadata', '--input',
    path.join(root, 'r98-production-metadata.log'), '--root', 'fe2o3-runtime'],
];
assert.deepStrictEqual(auxiliary.map(r => r.name), ['completion', 'controls', 'owned',
  'drain-rejection', 'terminal-retirement', 'proof-inventory', 'production-metadata', 'production-audit']);
for (const [i, row] of auxiliary.entries()) {
  assert.deepStrictEqual(row.command, auxiliaryCommands[i], row.name);
  assert.strictEqual(row.cwd, repo);
  assert.strictEqual(row.log, path.join(root, 'r98-' + row.name + '.log'));
}
for (const row of [...gates, ...auxiliary]) assert.strictEqual(row.returncode, 0, row.name);
for (const [prefix, count] of [['r98-final', 17], ['r98-auxiliary', 8]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'),
    {completed: true, gates: count, source_identities: Object.keys(baseline).length});
}
for (const name of ['r98-clippy', 'r98-frozen', 'r98-restored']) {
  const run = read(name + '.json');
  assert.deepStrictEqual(run.command, name === 'r98-clippy'
    ? previousGates.find(r => r.name === 'clippy-all').command : [...tests, 'co1_']);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  assert.strictEqual(run.returncode, 0);
  assert.strictEqual(run.source_unchanged, true);
  assert.deepStrictEqual(read(name + '-source.json'), baseline);
}
const prefix = 'async_engine::tests::owned_tests::control_tests::completion_tests::';
const integratedTests = [
  'co1_classifier_covers_all_observation_families_without_allocation',
  'co1_classifier_borrows_non_clone_errors_without_early_drop',
  'co1_existing_reply_gate_is_sticky_and_preserves_credit',
  'co1_ordinary_rejections_keep_one_issue_and_exact_last_error',
  'co1_ordinary_failure_and_quiescent_error_preserve_distinct_raw_results',
  'co1_terminal_raw_reply_does_not_retire_registry_or_context_custody',
].map(name => prefix + name);
const mutations = read('r98-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['rejected-class', 'pending-class', 'error-precedence', 'terminal-class']);
assert.deepStrictEqual(mutations.map(m => m.test), [integratedTests[3], integratedTests[3], integratedTests[4], integratedTests[5]]);
for (const m of mutations) {
  const name = 'r98-mut-' + m.name;
  const run = read(name + '.json');
  assert.strictEqual(run.returncode, 101);
  assert.strictEqual(run.source_unchanged, true);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  assert.deepStrictEqual(run.command, [...tests, m.test, '--', '--exact']);
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), Object.keys(baseline).sort());
  assert.deepStrictEqual(Object.keys(baseline).filter(p => inputs[p] !== baseline[p]), [m.path]);
  let candidate = fs.readFileSync(path.join(repo, m.path), 'utf8');
  for (const [before, after] of m.edits) {
    assert.strictEqual(candidate.split(before).length, 2);
    candidate = candidate.replace(before, after);
  }
  assert.strictEqual(hash(candidate), inputs[m.path]);
  const log = fs.readFileSync(run.log, 'utf8');
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test',
    'test ' + m.test + ' ... FAILED', ...m.expected, '0 passed; 1 failed']) assert(log.includes(marker), name + ': ' + marker);
}
const totals = logfile => {
  const text = fs.readFileSync(logfile, 'utf8');
  const matches = [...text.matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)];
  assert(matches.length, 'nonempty suite: ' + logfile);
  return matches.reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}),
  {harnesses: 0, passed: 0, failed: 0, ignored: 0});
};
for (const name of ['r98-frozen', 'r98-restored', 'r98-final-gnu-tests', 'r98-final-musl-tests']) {
  const log = fs.readFileSync(path.join(root, name + '.log'), 'utf8');
  for (const test of integratedTests) assert(log.includes('test ' + test + ' ... ok'), name + ': ' + test);
}
for (const name of ['r98-frozen', 'r98-restored']) assert.deepStrictEqual(totals(path.join(root, name + '.log')),
  {harnesses: 1, passed: 6, failed: 0, ignored: 0});
const summary = {
  source_parent: parent, scope: 'CO-1 production-used borrowed completion classification; VER-1A.1 contract/inventory only',
  source_identities: Object.keys(baseline).length, source_gates: 17, auxiliary_checks: 8,
  environment_record: 'raw/r98-environment.json', compiled_behavioral_mutations: 4,
  integrated_test_functions: integratedTests, preliminary_runs: ['r98-co1-first'],
  tests: Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(r.log)])),
  live_kfd: false, solver_rerun: false, performance_measurement: false,
};
const previous = committed('test-summary.json').tests;
const gateTestNames = gates.filter(r => r.command.includes('test')).map(r => r.name);
assert.deepStrictEqual(Object.keys(summary.tests).sort(), [...gateTestNames,
  'completion', 'controls', 'owned', 'drain-rejection', 'terminal-retirement'].sort());
for (const name of gateTestNames) assert.deepStrictEqual(summary.tests[name], {...previous[name],
  passed: previous[name].passed + (['gnu-tests', 'musl-tests'].includes(name) ? 6 : 0)}, name);
for (const [name, passed] of Object.entries({completion: 6, controls: 23, owned: 178, 'drain-rejection': 3, 'terminal-retirement': 1})) {
  assert.deepStrictEqual(summary.tests[name], {harnesses: 1, passed, failed: 0, ignored: 0}, name);
}
const prior = committed('raw/r97-frozen-source.json');
const protectedPath = name => /^crates\/fe2o3-(kfd|runtime-model|resource-accounting|completion)\//.test(name)
  || /^crates\/fe2o3-runtime\/src\/context(\.rs|\/)/.test(name) || /(^|\/)Cargo\.(toml|lock)$/.test(name);
assert.deepStrictEqual(Object.keys(baseline).filter(protectedPath).sort(), Object.keys(prior).filter(protectedPath).sort());
for (const name of Object.keys(baseline).filter(protectedPath)) assert.strictEqual(baseline[name], prior[name], name);
for (const suffix of ['.json', '.log', '-source.json']) assert(fs.statSync(path.join(root, 'r98-co1-first' + suffix)).isFile());
assert.strictEqual(read('r98-co1-first.json').source_unchanged, false);
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r98-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
