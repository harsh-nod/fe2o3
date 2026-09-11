const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const dest = path.join(repo, 'docs/evidence/local-r97-auxiliary-outer-settlement-2026-09-11');
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const baseline = read('r97-frozen-source.json');
const environment = read('r97-environment.json');
assert.deepStrictEqual(environment.binaries.map(b => b.name), ['cargo', 'rustc']);
for (const binary of environment.binaries) {
  assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256, 'unchanged toolchain binary: ' + binary.name);
}
const planningParent = '82c8cd854bc6cb8b300a4f5a6b9a2467827dcc6f';
assert.strictEqual(cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim(), planningParent);
assert.deepStrictEqual(read('r97-final-source-inputs.json'), baseline);
assert.deepStrictEqual(read('r97-final-source-after.json'), baseline);
const paths = cp.execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {cwd: repo}).toString().split('\0').filter(p => p && !p.startsWith('docs/'));
const current = Object.fromEntries([...new Set(paths)].sort().map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.deepStrictEqual(current, baseline, 'final non-documentation source must match tested source');
const gates = read('r97-final-source-gate.json');
const auxiliary = read('r97-auxiliary-results.json');
assert.deepStrictEqual(read('r97-auxiliary-source-inputs.json'), baseline);
assert.deepStrictEqual(read('r97-auxiliary-source-after.json'), baseline);
assert.deepStrictEqual(gates.map(r => r.name), [
  'gnu-tests', 'musl-tests', 'gnu-docs', 'musl-docs', 'musl-host-docs',
  'gnu-host', 'musl-host', 'macro-fixtures', 'clippy-all', 'clippy-production',
  'python', 'fmt', 'whitespace', 'dependency-policy', 'dependency-tests',
  'ci-test-gate', 'standalone-lockfiles',
]);
assert.deepStrictEqual(auxiliary.map(r => r.name), [
  'auxiliary-construction', 'primary', 'ordinary', 'construction', 'linux-helpers',
  'initialization', 'transitions', 'preparation', 'bind', 'proof-inventory',
  'production-metadata', 'production-audit',
]);
assert.deepStrictEqual(read('r97-final-complete.json'),
  {completed: true, gates: 17, source_identities: Object.keys(baseline).length});
assert.deepStrictEqual(read('r97-auxiliary-complete.json'),
  {completed: true, gates: 12, source_identities: Object.keys(baseline).length});
const cargo = ['cargo', '+nightly-2026-04-03'];
const packages = ['fe2o3-completion', 'fe2o3-runtime-model', 'fe2o3-resource-accounting',
  'fe2o3-kfd', 'fe2o3-runtime'].flatMap(p => ['-p', p]);
const tests = [...cargo, 'test', '--locked', '--offline', '--no-fail-fast', ...packages, '--all-features'];
const musl = ['--target', 'x86_64-unknown-linux-musl'];
const lint = [...cargo, 'clippy', '--locked', '--offline', ...packages, '-p', 'fe2o3-host', '-p', 'fe2o3-macros'];
const kfd = [...cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const gnuHost = [...cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-host'];
const expectedGates = [
  [...tests, '--all-targets'], [...tests, '--all-targets', ...musl],
  [...tests, '-p', 'fe2o3-host', '--doc'], [...tests, '--doc', ...musl],
  [...gnuHost, '--doc', ...musl], [...gnuHost, '--all-features', '--lib'],
  [...gnuHost, '--lib', ...musl],
  [...cargo, 'test', '--locked', '--offline', '-p', 'fe2o3-macros', '--test', 'typed_kernel_fixtures'],
  [...lint, '--all-features', '--all-targets', '--', '-D', 'warnings'],
  [...lint, '--no-default-features', '--lib', '--', '-D', 'warnings'],
  ['python3', '-B', '-m', 'unittest', 'test_run_r26_inplace_runner', 'test_check_r40_striped',
    'test_run_r40_striped_runner', 'test_run_r60_pipeline', 'test_check_r60_pipeline',
    'test_run_r61_owner', 'test_run_r62_control', 'test_run_r63_graph', 'test_run_r65_drain_versions',
    'test_run_r66_coexistence', 'test_check_scale3_protocol', 'test_check_drain_capture', 'test_run_drain_capture'],
  [...cargo, 'fmt', '--all', '--', '--check'], ['git', 'diff', '--check'],
  ['python3', '-B', 'scripts/workspace_dependency_policy.py'],
  ['python3', '-B', 'scripts/tests/workspace_dependency_policy.py'],
  ['bash', 'scripts/tests/ci-local-test-gate.sh'], ['bash', 'scripts/check-standalone-lockfiles.sh'],
];
for (const [i, row] of gates.entries()) {
  assert.deepStrictEqual(row.command, expectedGates[i], row.name);
  assert.strictEqual(row.cwd, row.name === 'python' ? path.join(repo, 'benchmarks/runtime_gfx942') : repo);
  assert.strictEqual(row.log, path.join(root, 'r97-final-' + row.name + '.log'));
}
const expectedAuxiliary = [
  ...['queue::live::construction_auxiliary::', 'queue::live::construction_primary::',
    'ordinary_constructor_root_', 'queue::live::construction::', 'queue_linux::tests::',
    'queue::tests::initialization::', 'shared_memory::tests::transitions::',
    'preparation::tests::', 'persistent_bind_'].map(filter => [...kfd, filter]),
  ['python3', '-I', 'crates/fe2o3-runtime-model/verus/check-negative-quality.py',
    'crates/fe2o3-runtime-model/verus/negative', 'crates/fe2o3-runtime-model/verus/verify-verus.sh'],
  [...cargo, 'metadata', '--locked', '--offline', '--no-default-features', '--format-version', '1',
    '--filter-platform', 'x86_64-unknown-linux-musl'],
  ['python3', '-B', 'scripts/runtime_pure_rust_audit.py', 'metadata', '--input',
    path.join(root, 'r97-production-metadata.log'), '--root', 'fe2o3-runtime'],
];
for (const [i, row] of auxiliary.entries()) {
  assert.deepStrictEqual(row.command, expectedAuxiliary[i], row.name);
  assert.strictEqual(row.cwd, repo);
  assert.strictEqual(row.log, path.join(root, 'r97-' + row.name + '.log'));
}
for (const row of [...gates, ...auxiliary]) assert.strictEqual(row.returncode, 0, row.name);
for (const name of ['r97-clippy-final', 'r97-frozen', 'r97-restored']) {
  const result = read(name + '.json');
  assert.deepStrictEqual(result.command, name === 'r97-clippy-final'
    ? [...lint, '--all-features', '--all-targets', '--', '-D', 'warnings']
    : [...kfd, 'queue::live::construction']);
  assert.strictEqual(result.cwd, repo);
  assert.strictEqual(result.log, path.join(root, name + '.log'));
  assert.strictEqual(result.source, path.join(root, name + '-source.json'));
  assert.strictEqual(result.returncode, 0, name);
  assert.strictEqual(result.source_unchanged, true, name);
  assert.deepStrictEqual(read(name + '-source.json'), baseline, name);
}
const mutations = read('r97-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['opening', 'retake', 'operation-result', 'parent-transport']);
for (const m of mutations) {
  const name = 'r97-mut-' + m.name;
  const run = read(name + '.json');
  assert.strictEqual(run.returncode, 101, name);
  assert.strictEqual(run.source_unchanged, true, name);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  assert.deepStrictEqual(run.command, ['cargo', '+nightly-2026-04-03', 'test',
    '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib',
    m.test, '--', '--exact']);
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
    'test ' + m.test + ' ... FAILED', ...m.expected, '0 passed; 1 failed']) {
    assert(log.includes(marker), name + ': ' + marker);
  }
}
const totals = logfile => {
  const text = fs.readFileSync(logfile, 'utf8');
  const matches = [...text.matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)];
  assert(matches.length, 'nonempty test results: ' + logfile);
  return matches.reduce((a, m) => ({harnesses: a.harnesses + 1,
    passed: a.passed + Number(m[1]), failed: a.failed + Number(m[2]),
    ignored: a.ignored + Number(m[3])}), {harnesses: 0, passed: 0, failed: 0, ignored: 0});
};
const prefix = 'queue::live::construction_primary::integration_tests::auxiliary_cases::';
const testFiles = [
  ['integration_tests.rs', prefix],
  ['integration_prefix_tests.rs', prefix + 'prefix_cases::'],
  ['integration_memory_tests.rs', prefix + 'prefix_cases::memory_cases::'],
];
const integratedTests = testFiles.flatMap(([name, prefix]) => {
  const text = fs.readFileSync(path.join(repo, 'crates/fe2o3-kfd/src/queue_live/construction_auxiliary', name), 'utf8');
  return [...text.matchAll(/#\[test\]\s+fn\s+(\w+)\s*\(/g)].map(m => prefix + m[1]);
});
assert.strictEqual(integratedTests.length, 14);
assert.strictEqual(new Set(integratedTests).size, 14);
for (const name of ['r97-frozen', 'r97-restored', 'r97-final-gnu-tests', 'r97-final-musl-tests']) {
  const log = fs.readFileSync(path.join(root, name + '.log'), 'utf8');
  for (const test of integratedTests) assert(log.includes('test ' + test + ' ... ok'), name + ': ' + test);
}
for (const name of ['r97-frozen', 'r97-restored']) {
  assert.deepStrictEqual(totals(path.join(root, name + '.log')),
    {harnesses: 1, passed: 53, failed: 0, ignored: 0});
}
const summary = {
  source_parent: '369f99835cfb2af5df9fda45cc828d462ef6b156',
  planning_parent: planningParent,
  scope: 'NATIVE-2B.5B-1 production-used outer settlement and named CPU/fake-native prefix matrix; not full NATIVE-2B.5B',
  source_identities: Object.keys(baseline).length,
  environment_record: 'raw/r97-environment.json',
  source_gates: gates.length,
  auxiliary_checks: auxiliary.length,
  compiled_negative_mutations: mutations.length,
  mutation_command_identity_checked: true,
  integrated_test_functions: integratedTests,
  source_counted_matrix: {
    auxiliary_driver_runs: 369, failure_runs: 366, success_runs: 3,
    separate_capacity_fixtures: 1, repeated_borrowed_preflight_rejections: 2,
  },
  preliminary_runs: ['r97-shared-driver', 'r97-prefix-first', 'r97-prefix-second',
    'r97-native-prefix-first', 'r97-native-prefix-second', 'r97-clippy-preflight'],
  tests: Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(r.log)])),
  live_kfd: false, solver_rerun: false, performance_measurement: false,
};
const previous = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r96-auxiliary-shared-engine-2026-09-11/test-summary.json'), 'utf8')).tests;
assert.deepStrictEqual(Object.keys(summary.tests).sort(), Object.keys(previous).sort());
const increments = {'gnu-tests': 12, 'musl-tests': 12, primary: 12};
for (const [name, counts] of Object.entries(summary.tests)) {
  assert(previous[name], 'known test suite: ' + name);
  assert.deepStrictEqual(counts, {...previous[name], passed: previous[name].passed + (increments[name] || 0)}, name);
}
assert.deepStrictEqual(summary.tests['gnu-tests'], summary.tests['musl-tests']);
const priorSource = JSON.parse(fs.readFileSync(path.join(repo,
  'docs/evidence/local-r96-auxiliary-shared-engine-2026-09-11/raw/r96-frozen-source.json'), 'utf8'));
const protectedPath = name => /^crates\/fe2o3-(runtime-model|resource-accounting|completion)\//.test(name)
  || /(^|\/)Cargo\.(toml|lock)$/.test(name);
assert.deepStrictEqual(Object.keys(baseline).filter(protectedPath).sort(),
  Object.keys(priorSource).filter(protectedPath).sort());
for (const name of Object.keys(baseline).filter(protectedPath)) {
  assert.strictEqual(baseline[name], priorSource[name], 'unchanged model/accounting/completion/Cargo: ' + name);
}
for (const name of summary.preliminary_runs) {
  for (const suffix of ['.json', '.log', '-source.json']) {
    assert(fs.statSync(path.join(root, name + suffix)).isFile(), 'retained preliminary attempt: ' + name + suffix);
  }
}
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r97-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
