const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '5cdeedd8290fac0bf01ca53b01cc12e828a2e20e';
const tested = '0bb5d1d35c89336f40e2ee311240e66f659fc323';
const accepted = '61a1479348ec3b744a8881108e059f540f324364';
const previous = 'docs/evidence/local-r103-replacement-input-custody-2026-09-12/';
const dest = path.join(repo, 'docs/evidence/local-r104-ordinary-rebind-custody-2026-09-12');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 32 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const committed = name => git(['show', accepted + ':' + previous + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
for (const [from, to] of [[accepted, tested], [tested, parent]]) {
  const changes = git(['diff', '--name-only', from, to]).trim().split('\n').filter(Boolean);
  assert(changes.length && changes.every(p => p.startsWith('docs/')));
}
const baseline = read('r104-frozen-source.json');
const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
assert.deepStrictEqual(Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), baseline);
assert.strictEqual(paths.length, 5662);
const prior = JSON.parse(committed('raw/r103-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...paths])].filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/queue_dispatch_binding.rs',
  'crates/fe2o3-kfd/src/queue_dispatch_binding/preparation_tests.rs',
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_auxiliary.rs',
  'crates/fe2o3-kfd/src/queue_live/fixed_dispatch.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/preparation.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/primary_construction.rs',
]);
const environment = read('r104-environment.json');
const oldEnvironment = JSON.parse(committed('raw/r103-environment.json'));
assert.deepStrictEqual(environment.binaries, oldEnvironment.binaries);
assert.deepStrictEqual(environment.records, oldEnvironment.records);
assert.strictEqual(environment.publication_parent, parent);
assert.strictEqual(environment.tested_source_checkpoint, tested);
assert.strictEqual(environment.accepted_runtime_checkpoint, accepted);
assert.deepStrictEqual(environment.environment_overrides, {
  CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1',
});
assert.deepStrictEqual(environment.environment_removed, ['XDG_RUNTIME_DIR']);
assert.strictEqual(environment.test_deadlines_changed, false);
assert.strictEqual(environment.hardware_qualification, false);
assert.strictEqual(environment.solver_rerun, false);
const issueStatus = read('r104-issue-status.json');
assert.deepStrictEqual(issueStatus.command, ['gh', 'issue', 'view', '182', '--repo',
  'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url']);
assert.strictEqual(issueStatus.issue.number, 182);
assert.strictEqual(issueStatus.issue.url, 'https://github.com/harsh-nod/fe2o3/issues/182');
assert.strictEqual(issueStatus.issue.state, 'OPEN');
assert(Number.isFinite(Date.parse(issueStatus.observed_at)));
assert(Number.isFinite(Date.parse(issueStatus.issue.updatedAt)));
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r104-final-source-gate.json');
const auxiliary = read('r104-auxiliary-results.json');
const oldGates = JSON.parse(committed('raw/r103-accepted-source-gate.json'));
const oldAuxiliary = JSON.parse(committed('raw/r103-auxiliary-results.json'));
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 10);
for (const [rows, oldRows, prefix] of [[gates, oldGates, 'r104-final-'], [auxiliary, oldAuxiliary, 'r104-']]) {
  assert.strictEqual(new Set(rows.map(r => r.name)).size, rows.length);
  assert.deepStrictEqual(rows.map(r => r.name), oldRows.map(r => r.name));
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[i].command.map(s => s.replace('r103-production-metadata.log', 'r104-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, oldRows[i].cwd, row.name);
    assert.strictEqual(row.log, path.join(root, prefix + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
  }
}
for (const [prefix, count] of [['r104-final', 17], ['r104-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5662});
}
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const prefix = 'queue::live::rebind_tests::';
const newTests = [
  'ordinary_rebind_caught_internal_preflight_panic_still_transports',
  'ordinary_rebind_healthy_preflight_retains_parent_without_new_poison',
  'ordinary_rebind_production_routing_roots_before_loan_and_commits_after_validation',
  'ordinary_rebind_public_direct_wrapper_transports_terminal_parent',
  'ordinary_rebind_terminal_facade_restores_exact_parent_before_transport',
  'preparation::ordinary_rebind_all_preparation_stages_retain_original_prefixes',
  'preparation::ordinary_rebind_generation_and_suppressed_failure_never_install_invalid_owner',
  'preparation::ordinary_rebind_opening_and_validation_faults_keep_original_owners',
  'preparation::ordinary_rebind_preflight_retains_exact_inputs_and_preserves_rejection_class',
  'preparation::ordinary_rebind_real_loan_operation_retake_matrix_preserves_custody',
].map(n => prefix + n);
const passingNames = text => [...text.matchAll(/^test (\S+) \.\.\. ok$/gm)].map(m => m[1]);
const failedNames = text => [...text.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(m => m[1]);
const totals = text => {
  const matches = [...text.matchAll(/^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;/gm)];
  assert(matches.length, 'nonempty suite');
  return matches.reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}), {harnesses: 0, passed: 0, failed: 0, ignored: 0});
};
const logText = run => fs.readFileSync(run.log, 'utf8');
const runEnvironment = {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null};
function checkRun(name, command, returncode) {
  const run = read(name + '.json');
  assert.deepStrictEqual(run.command, command, name);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  const inputs = read(name + '-source.json');
  assert(inputs && typeof inputs === 'object' && !Array.isArray(inputs));
  assert(Object.keys(inputs).length > 0);
  for (const [file, digest] of Object.entries(inputs)) {
    assert(!file.startsWith('docs/') && /^[0-9a-f]{64}$/.test(digest), name + ': valid source identity');
  }
  assert.strictEqual(run.returncode, returncode, name);
  assert.strictEqual(run.source_unchanged, true, name);
  assert.strictEqual(run.timed_out, false, name);
  assert.strictEqual(run.signal, null, name);
  assert.deepStrictEqual(run.environment, runEnvironment, name);
  return run;
}
const oldConstruction = passingNames(committed('raw/r103-restored-construction.log')).sort();
assert.strictEqual(oldConstruction.length, 69);
for (const [kind, filter, names] of [['rebind', 'ordinary_rebind', newTests], ['construction', 'queue::live::construction', oldConstruction]]) {
  for (const phase of ['frozen', 'restored']) {
    const name = 'r104-' + phase + '-' + kind;
    const run = checkRun(name, [...kfd, filter], 0);
    assert.deepStrictEqual(read(name + '-source.json'), baseline);
    assert.deepStrictEqual(passingNames(logText(run)).sort(), [...names].sort());
    assert.deepStrictEqual(totals(logText(run)), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
  }
}
const mutations = read('r104-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['drop-input-root', 'skip-validation', 'extract-failed-complete',
  'overwrite-transport', 'omit-lane-restoration', 'globalize-returned-error']);
const mutantHashes = new Set();
for (const m of mutations) {
  assert(newTests.includes(m.test) && !m.test.includes('production_routing'));
  const name = 'r104-mut-' + m.name;
  const run = checkRun(name, [...kfd, m.test, '--', '--exact'], 101);
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
  assert.deepStrictEqual(paths.filter(p => inputs[p] !== baseline[p]), [m.path]);
  let candidate = fs.readFileSync(path.join(repo, m.path), 'utf8');
  for (const [before, after] of m.edits) {
    assert.strictEqual(candidate.split(before).length, 2);
    candidate = candidate.replace(before, after);
  }
  assert.strictEqual(hash(candidate), inputs[m.path]);
  mutantHashes.add(inputs[m.path]);
  const log = logText(run);
  assert.deepStrictEqual(failedNames(log), [m.test]);
  assert.deepStrictEqual(totals(log), {harnesses: 1, passed: 0, failed: 1, ignored: 0});
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test', ...m.expected]) {
    assert(log.includes(marker), name + ': ' + marker);
  }
  const restored = read('r104-restoration-' + m.name + '.json');
  assert.strictEqual(restored.mutation, m.name);
  assert.strictEqual(restored.source_identities, paths.length);
  assert.strictEqual(restored.source_unchanged, true);
  assert.strictEqual(restored.source_map_sha256, hash(JSON.stringify(baseline)));
}
assert.strictEqual(mutantHashes.size, 6);
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test'))
  .map(r => [r.name, totals(logText(r))]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const [name, result] of Object.entries(tests)) assert.deepStrictEqual(result, {
  ...oldTests[name], passed: oldTests[name].passed + (['gnu-tests', 'musl-tests'].includes(name) ? 10 : 0),
}, name);
for (const target of ['gnu', 'musl']) {
  const oldNames = passingNames(committed('raw/r103-accepted-' + target + '-tests.log'));
  const names = passingNames(fs.readFileSync(path.join(root, 'r104-final-' + target + '-tests.log'), 'utf8'));
  assert.deepStrictEqual(names.sort(), [...oldNames, ...newTests].sort(), target + ' exact passing multiset');
}
const historical = [];
for (const [suffix, code, category] of [
  ['01', 101, 'compile-failure'], ['02', 101, 'compile-failure'],
  ['03', 101, 'incorrect-probe-poison-test-oracle'], ['04', 0, 'preliminary-pass'],
  ['05', 101, 'compile-failure'], ['06', 0, 'frozen-source-preliminary-pass'],
]) {
  const name = 'r104-focused-' + suffix;
  const run = checkRun(name, [...kfd, 'ordinary_rebind'], code);
  const text = logText(run);
  if (category === 'compile-failure') {
    assert(text.includes('could not compile'));
    assert(!text.includes('running 1 test') && !text.includes('test result:'));
  } else {
    const expected = suffix === '03' ? [7, 1] : suffix === '04' ? [9, 0] : [10, 0];
    assert.deepStrictEqual(totals(text), {harnesses: 1, passed: expected[0], failed: expected[1], ignored: 0});
  }
  if (suffix === '06') {
    assert.deepStrictEqual(read(name + '-source.json'), baseline);
    assert.deepStrictEqual(passingNames(text).sort(), [...newTests].sort());
  }
  historical.push({name, category, returncode: code, accepted_campaign: false});
}
const clippy = checkRun('r104-clippy-01', gates.find(r => r.name === 'clippy-all').command, 0);
assert.deepStrictEqual(read('r104-clippy-01-source.json'), baseline);
assert(logText(clippy).includes('Finished `dev` profile'));
assert(fs.readFileSync(path.join(root, 'r104-final-python.log'), 'utf8').includes('Ran 151 tests'));
assert(fs.readFileSync(path.join(root, 'r104-final-dependency-tests.log'), 'utf8').includes('Ran 8 tests'));
const summary = {
  publication_parent: parent, tested_source_checkpoint: tested, accepted_runtime_checkpoint: accepted,
  scope: 'Ordinary live-rebind input/preparation custody and deferred parent retention; CPU/shared-sequence acceptance only',
  source_identities: paths.length, changed_source_files: changed,
  source_gates: gates.length, auxiliary_checks: auxiliary.length, compiled_behavioral_mutations: mutations.length,
  new_test_functions: newTests,
  dynamic_scenarios: {preflight: 8, operation_retake: 18, opening_validation: 5,
    generation_suppression: 6, preparation_stages: 62, healthy_facade: 2,
    terminal_facade: 8, caught_internal_panic: 2, public_direct: 1, total: 112},
  dynamic_test_functions: 9, textual_source_guards: 1,
  preparation_helper_outcomes: {runs: 91, successful_installs: 3, returned_errors: 45, caught_panics: 43},
  facade_outcomes: {scenarios: 13, healthy_rejections: 2, terminal_transports: 11},
  focused_tests: {frozen_rebind: 10, restored_rebind: 10, frozen_construction: 69, restored_construction: 69},
  tests, python_tests: {benchmark_harnesses: 151, dependency_policy: 8},
  historical_attempts: historical, preliminary_clippy: 'r104-clippy-01',
  environment_record: 'raw/r104-environment.json', issue_status_record: 'raw/r104-issue-status.json',
  environment_record_timing: environment.recorded_phase,
  metadata_stderr_retained: false, initial_formatter_diagnostic_retained: false,
  no_new_formal_source: true, proof_inventory_only: true, solver_rerun: false,
  original_engine_composition_qualified: false, later_auxiliary_vector_slots_qualified: false,
  live_kfd: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r104-.*\.(json|log|py|js)$/.test(p)).sort()) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
