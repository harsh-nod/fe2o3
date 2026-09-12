const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '50c4eb075013fde0a984a003c0b5b90eae562847';
const previous = 'docs/evidence/local-r102-auxiliary-roster-slots-2026-09-12/';
const dest = path.join(repo, 'docs/evidence/local-r103-replacement-input-custody-2026-09-12');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 16 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const committed = name => git(['show', parent + ':' + previous + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
const baseline = read('r103-frozen-source.json');
const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
assert.deepStrictEqual(Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), baseline);
assert.strictEqual(paths.length, 5659);
const prior = JSON.parse(committed('raw/r102-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...paths])].filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/queue_dispatch_binding.rs',
  'crates/fe2o3-kfd/src/queue_dispatch_binding/preparation_tests.rs',
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_replacement_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/construction_primary/integration_tests.rs',
]);
const environment = read('r103-environment.json');
const oldEnvironment = JSON.parse(committed('raw/r102-environment.json'));
assert.deepStrictEqual(environment.binaries, oldEnvironment.binaries);
assert.deepStrictEqual(environment.records, oldEnvironment.records);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r103-accepted-source-gate.json');
const auxiliary = read('r103-auxiliary-results.json');
const oldGates = JSON.parse(committed('raw/r102-final-source-gate.json'));
const oldAuxiliary = JSON.parse(committed('raw/r102-auxiliary-results.json'));
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 10);
for (const [rows, oldRows, prefix] of [[gates, oldGates, 'r103-accepted-'], [auxiliary, oldAuxiliary, 'r103-']]) {
  assert.deepStrictEqual(rows.map(r => r.name), oldRows.map(r => r.name));
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[i].command.map(s => s.replace('r102-production-metadata.log', 'r103-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, oldRows[i].cwd, row.name);
    assert.strictEqual(row.log, path.join(root, prefix + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
  }
}
for (const [prefix, count] of [['r103-accepted', 17], ['r103-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5659});
}
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const prefix = 'queue::live::construction_primary::integration_tests::replacement_cases::';
const newTests = [
  'replacement_create_uncertainty_retains_all_published_owners_without_cleanup',
  'replacement_early_validation_and_planning_retain_inputs_before_any_native_work',
  'replacement_invalid_generations_record_failed_custody_without_native_work',
  'replacement_invalid_recipe_retains_original_program_and_packet_rosters',
  'replacement_late_failures_retain_completed_dispatch_and_original_inputs',
  'replacement_native_preparation_failures_preserve_pending_and_returned_owners',
  'replacement_preparation_stage_failures_preserve_inputs_generation_and_control_prefixes',
  'replacement_public_entry_roots_inputs_before_the_shared_construction_sequence',
  'replacement_success_preserves_exact_predecessor_and_last_issuable_generation',
].map(n => prefix + n);
const passingNames = text => [...text.matchAll(/^test (\S+) \.\.\. ok$/gm)].map(m => m[1]);
const totals = logfile => {
  const matches = [...fs.readFileSync(logfile, 'utf8').matchAll(/test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out/g)];
  assert(matches.length, 'nonempty suite: ' + logfile);
  return matches.reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}), {harnesses: 0, passed: 0, failed: 0, ignored: 0});
};
const oldConstruction = passingNames(committed('raw/r102-restored-construction.log'));
assert.strictEqual(oldConstruction.length, 60);
for (const name of ['r103-frozen-construction', 'r103-restored-construction']) {
  const run = read(name + '.json');
  assert.deepStrictEqual(run.command, [...kfd, 'queue::live::construction']);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  assert.strictEqual(run.returncode, 0);
  assert.strictEqual(run.source_unchanged, true);
  assert.strictEqual(run.timed_out, false);
  assert.deepStrictEqual(read(name + '-source.json'), baseline);
  assert.deepStrictEqual(passingNames(fs.readFileSync(run.log, 'utf8')).sort(), [...oldConstruction, ...newTests].sort());
  assert.deepStrictEqual(totals(run.log), {harnesses: 1, passed: 69, failed: 0, ignored: 0});
}
const mutations = read('r103-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), ['zero-pristine', 'reset-generation', 'generation-custody', 'completed-owner', 'original-programs', 'native-quarantine']);
for (const m of mutations) {
  assert(newTests.includes(m.test));
  const name = 'r103-mut-' + m.name;
  const run = read(name + '.json');
  assert.strictEqual(run.returncode, 101);
  assert.strictEqual(run.source_unchanged, true);
  assert.strictEqual(run.timed_out, false);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  assert.deepStrictEqual(run.command, [...kfd, m.test, '--', '--exact']);
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
  assert.deepStrictEqual(paths.filter(p => inputs[p] !== baseline[p]), [m.path]);
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
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(r.log)]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const row of [...gates, ...auxiliary].filter(r => r.command.includes('test'))) assert.deepStrictEqual(tests[row.name], {
  ...oldTests[row.name], passed: oldTests[row.name].passed + (['gnu-tests', 'musl-tests'].includes(row.name) ? 9 : 0),
}, row.name);
for (const name of ['r103-accepted-gnu-tests', 'r103-accepted-musl-tests']) {
  const names = passingNames(fs.readFileSync(path.join(root, name + '.log'), 'utf8'));
  for (const test of [...oldConstruction, ...newTests]) assert(names.includes(test), name + ': ' + test);
}
assert.strictEqual(read('r103-initial-replacement.json').returncode, 101);
assert.strictEqual(read('r103-replacement-first.json').returncode, 0);
assert.strictEqual(read('r103-preflight-clippy.json').returncode, 0);
const failedCampaign = read('r103-final-source-gate.json');
assert.deepStrictEqual(failedCampaign.map(r => [r.name, r.returncode]), [['gnu-tests', 0], ['musl-tests', 101]]);
assert.deepStrictEqual(read('r103-final-source-inputs.json'), baseline);
assert(!fs.existsSync(path.join(root, 'r103-final-source-after.json')));
assert(!fs.existsSync(path.join(root, 'r103-final-complete.json')));
for (const [i, row] of failedCampaign.entries()) {
  assert.deepStrictEqual(row.command, gates[i].command);
  assert.strictEqual(row.cwd, repo);
  assert.strictEqual(row.log, path.join(root, 'r103-final-' + row.name + '.log'));
}
const failedLog = fs.readFileSync(path.join(root, 'r103-final-musl-tests.log'), 'utf8');
const diagnosticTests = [
  ['drain', 'async_engine::tests::owned_tests::drain_tests::r65_executor_drains_2048_accepted_operations_with_full_reply_budget', 'drain watchdog expired'],
  ['worker', 'worker::tests::blocked_request_write_obeys_absolute_deadline_and_reaps_the_worker', 'worker deadline exceeded scheduler tolerance'],
];
assert.deepStrictEqual([...failedLog.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(m => m[1]).sort(),
  diagnosticTests.map(t => t[1]).sort());
const historicalTotals = [...failedLog.matchAll(/test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;/g)]
  .reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}),
  {harnesses: 0, passed: 0, failed: 0, ignored: 0});
assert.deepStrictEqual(historicalTotals, {harnesses: 48, passed: 2553, failed: 2, ignored: 5});
assert.deepStrictEqual(totals(failedCampaign[0].log), {harnesses: 48, passed: 2555, failed: 0, ignored: 5});
for (const [name, test, marker] of diagnosticTests) {
  assert(failedLog.includes(marker));
  const runName = 'r103-diagnostic-' + name;
  const run = read(runName + '.json');
  assert.deepStrictEqual(run.command, ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline',
    '-p', 'fe2o3-runtime', '--all-features', '--lib', '--target', 'x86_64-unknown-linux-musl', test, '--', '--exact', '--nocapture']);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, runName + '.log'));
  assert.strictEqual(run.source, path.join(root, runName + '-source.json'));
  assert.strictEqual(run.returncode, 0);
  assert.strictEqual(run.timed_out, false);
  assert.strictEqual(run.source_unchanged, true);
  assert.deepStrictEqual(read(runName + '-source.json'), baseline);
  assert.deepStrictEqual(passingNames(fs.readFileSync(run.log, 'utf8')), [test]);
  assert.deepStrictEqual(totals(run.log), {harnesses: 1, passed: 1, failed: 0, ignored: 0});
}
const acceptedEnvironment = read('r103-accepted-environment.json');
assert.strictEqual(acceptedEnvironment.attempt, 'r103-accepted');
assert.deepStrictEqual(acceptedEnvironment.environment_overrides, {
  RUST_TEST_THREADS: '4', CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', PYTHONDONTWRITEBYTECODE: '1',
});
assert.deepStrictEqual(acceptedEnvironment.environment_removed, ['XDG_RUNTIME_DIR']);
assert.strictEqual(acceptedEnvironment.test_deadlines_changed, false);
assert.strictEqual(acceptedEnvironment.test_filters_added, false);
const driver = read('r103-accepted-driver-results.json');
assert.strictEqual(driver.length, 2);
assert.deepStrictEqual(driver.map(r => r.command), [
  ['/usr/bin/python3', '-B', path.join(root, 'r103-source-gate.py'), 'r103-accepted'],
  ['/usr/bin/python3', '-B', path.join(root, 'r103-auxiliary-gates.py')],
]);
assert(driver.every(r => r.returncode === 0));
const summary = {
  source_parent: parent, scope: 'NATIVE-2C replacement-input custody; CPU/shared-sequence acceptance only',
  planning_only: 'Next Native/Admission/Resources handoffs and VER-1A.2a activation/capacity freeze',
  issue_status_record: 'raw/r103-issue-status.json',
  source_identities: paths.length, changed_source_files: changed,
  source_gates: gates.length, auxiliary_checks: auxiliary.length,
  compiled_behavioral_mutations: mutations.length, new_test_functions: newTests,
  replacement_matrix: {early_rejections: 7, invalid_generations: 3, invalid_recipe: 1,
    preparation_stage_failures: 62, preparation_native_failures: 126, late_failures: 48,
    create_uncertainty: 5, successes: 2, total_runs: 254, failures: 252},
  frozen_construction_tests: 69, restored_construction_tests: 69, tests,
  environment_record: 'raw/r103-environment.json',
  accepted_campaign: 'r103-accepted',
  accepted_campaign_environment: 'raw/r103-accepted-environment.json',
  historical_failed_campaign: {name: 'r103-final', accepted: false,
    gnu: totals(failedCampaign[0].log), musl: historicalTotals,
    failures: diagnosticTests.map(t => ({test: t[1], assertion: t[2]})),
    cause: 'Undetermined; scheduling contention is consistent with observations but not proven.'},
  diagnostic_runs: diagnosticTests.map(t => 'r103-diagnostic-' + t[0]),
  diagnostic_scope: 'Focused musl reruns only; not substitutes for full-source acceptance or additional unique tests.',
  historical_pre_freeze_runs: ['r103-initial-replacement', 'r103-replacement-first', 'r103-preflight-clippy'],
  live_kfd: false, solver_rerun: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r103-.*\.(json|log|py|js)$/.test(p))) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
