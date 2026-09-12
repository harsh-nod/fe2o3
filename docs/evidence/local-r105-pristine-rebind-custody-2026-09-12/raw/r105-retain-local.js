const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = 'c70773b9c9d9383f79a1c2fe829f1e1163596750';
const tested = 'b69a6f21c2beb0aad870d3f4b8cdc2eb183f456f';
const accepted = 'b69a6f21c2beb0aad870d3f4b8cdc2eb183f456f';
const previous = 'docs/evidence/local-r104-ordinary-rebind-custody-2026-09-12/';
const dest = path.join(repo, 'docs/evidence/local-r105-pristine-rebind-custody-2026-09-12');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 32 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const rejectedCollectorSha256 = '1dbd62d4c44d1698887910c53c3cbb3044898d379793db75736356efa4c344a2';
assert.strictEqual(hash(fs.readFileSync(path.join(root, 'r105-retain-local-before-repeat.js'))),
  rejectedCollectorSha256);
assert.deepStrictEqual(read('r105-collector-rejection.json'), {
  command: ['node', '/home/harsh/.codex-tmp/r105-retain-local.js'], returncode: 1,
  assertion: 'restore before next mutation', location: 'r105-retain-local.js:220',
  prior_restoration: '2026-09-12T13:46:05.206Z',
  next_mutation_start: '2026-09-12T13:46:05.146Z', inversion_ms: 60,
  cause_established: false, archive_created: false,
  disposition: 'Preserve original records. Repeat all nine negatives and restored focused gates; retain strict chronology for acceptance.',
});
const committed = name => git(['show', accepted + ':' + previous + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
assert.strictEqual(tested, accepted);
assert.strictEqual(git(['rev-parse', parent + '^']).trim(), accepted);
const planningFiles = git(['diff', '--name-only', accepted, parent]).trim().split('\n').sort();
assert.deepStrictEqual(planningFiles, [
  'docs/runtime-a1-a2-next-wave.md',
  'docs/runtime-a1-a2-swarm-current.md',
  'docs/runtime-swarm-dispatch-r105-candidate.md',
]);
const baseline = read('r105-frozen-source.json');
const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
assert.deepStrictEqual(Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), baseline);
assert.strictEqual(paths.length, 5664);
const prior = JSON.parse(committed('raw/r104-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...paths])].filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/queue_dispatch_binding.rs',
  'crates/fe2o3-kfd/src/queue_dispatch_binding/preparation_tests.rs',
  'crates/fe2o3-kfd/src/queue_dispatch_binding/pristine_abort.rs',
  'crates/fe2o3-kfd/src/queue_live.rs',
  'crates/fe2o3-kfd/src/queue_live/pristine_abort.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/preparation.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/preparation/pristine.rs',
  'crates/fe2o3-kfd/src/queue_live/rebind_tests/preparation/pristine/facade.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/preparation.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/primary_construction.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/pristine_abort.rs',
]);
const environment = read('r105-environment.json');
const oldEnvironment = JSON.parse(committed('raw/r104-environment.json'));
assert.deepStrictEqual(environment.binaries, oldEnvironment.binaries);
assert.deepStrictEqual(environment.records.map(r => r.command), oldEnvironment.records.map(r => r.command));
const environmentChanges = environment.records.flatMap((record, i) =>
  record.output === oldEnvironment.records[i].output ? [] : [{
    command: record.command, previous: oldEnvironment.records[i].output, current: record.output,
  }]);
assert.deepStrictEqual(environmentChanges, [{
  command: ['python3', '-VV'],
  previous: 'Python 3.12.3 (main, Jun 19 2026, 12:46:00) [GCC 13.3.0]\n',
  current: 'Python 3.12.3 (main, Aug 31 2026, 10:18:26) [GCC 13.3.0]\n',
}]);
for (const record of environment.records) {
  assert.strictEqual(cp.execFileSync(record.command[0], record.command.slice(1),
    {cwd: repo, encoding: 'utf8'}), record.output, record.command.join(' '));
}
assert.strictEqual(environment.publication_parent, tested);
assert.strictEqual(environment.accepted_runtime_checkpoint, accepted);
assert(environment.recorded_phase.startsWith('Before frozen/full/auxiliary/mutation campaigns;'));
assert.deepStrictEqual(environment.runners.map(r => r.name), [
  'r105-run.js', 'r105-source-gate.py', 'r105-auxiliary-gates.py', 'r105-freeze.js',
]);
for (const runner of environment.runners) assert.strictEqual(hash(fs.readFileSync(path.join(root, runner.name))), runner.sha256);
assert.deepStrictEqual(environment.environment_overrides, {
  CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1',
});
assert.deepStrictEqual(environment.environment_removed, ['XDG_RUNTIME_DIR']);
assert.strictEqual(environment.test_deadlines_changed, false);
assert.strictEqual(environment.hardware_qualification, false);
assert.strictEqual(environment.solver_rerun, false);
const issueStatus = read('r105-issue-status.json');
assert.deepStrictEqual(issueStatus.command, ['gh', 'issue', 'view', '182', '--repo',
  'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url']);
assert.strictEqual(issueStatus.issue.number, 182);
assert.strictEqual(issueStatus.issue.url, 'https://github.com/harsh-nod/fe2o3/issues/182');
assert.strictEqual(issueStatus.issue.state, 'OPEN');
assert(Number.isFinite(Date.parse(issueStatus.observed_at)));
assert(Number.isFinite(Date.parse(issueStatus.issue.updatedAt)));
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
const gates = read('r105-final-source-gate.json');
const auxiliary = read('r105-auxiliary-results.json');
const oldGates = JSON.parse(committed('raw/r104-final-source-gate.json'));
const oldAuxiliary = JSON.parse(committed('raw/r104-auxiliary-results.json'));
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 10);
for (const [rows, oldRows, prefix] of [[gates, oldGates, 'r105-final-'], [auxiliary, oldAuxiliary, 'r105-']]) {
  assert.strictEqual(new Set(rows.map(r => r.name)).size, rows.length);
  assert.deepStrictEqual(rows.map(r => r.name), oldRows.map(r => r.name));
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[i].command.map(s => s.replace('r104-production-metadata.log', 'r105-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, oldRows[i].cwd, row.name);
    assert.strictEqual(row.log, path.join(root, prefix + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
  }
}
for (const [prefix, count] of [['r105-final', 17], ['r105-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5664});
}
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const prefix = 'queue::live::rebind_tests::';
const newTests = [
  'facade::pristine_rebind_facade_retains_restored_primary_and_later_auxiliary_slots',
  'pristine_rebind_all_preparation_stages_retain_original_prefixes',
  'pristine_rebind_corrupted_continuation_records_failed_generation',
  'pristine_rebind_failed_complete_cannot_be_suppressed',
  'pristine_rebind_opening_failure_retains_unconsumed_continuation',
  'pristine_rebind_preflight_retains_exact_inputs_without_entered_poison_policy',
  'pristine_rebind_preserves_real_continuation_generation_and_fresh_occurrence',
  'pristine_rebind_real_loan_operation_retake_matrix_preserves_custody',
].map(n => prefix + 'preparation::pristine::' + n);
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
function checkRun(name, command, returncode, unchanged = true, expectedHead = parent) {
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
  assert.strictEqual(run.source_unchanged, unchanged, name);
  assert.strictEqual(run.source_head, expectedHead);
  assert(Number.isFinite(Date.parse(run.started_at)) && Number.isFinite(Date.parse(run.finished_at)));
  assert(Date.parse(run.started_at) <= Date.parse(run.finished_at));
  assert.strictEqual(run.timed_out, false, name);
  assert.strictEqual(run.signal, null, name);
  assert.deepStrictEqual(run.environment, runEnvironment, name);
  return run;
}
const oldConstruction = passingNames(committed('raw/r104-restored-construction.log')).sort();
assert.strictEqual(oldConstruction.length, 69);
const oldNames = passingNames(committed('raw/r104-final-gnu-tests.log'));
const oldPristine = oldNames.filter(n => n.includes('pristine'));
const oldRebind = oldNames.filter(n => n.includes('ordinary_rebind'));
assert.strictEqual(oldPristine.length, 20);
assert.strictEqual(oldRebind.length, 10);
const focusedTiming = {frozen: [], restored: [], 'repeat-restored': []};
for (const [kind, filter, names] of [
  ['pristine', 'pristine', [...oldPristine, ...newTests]],
  ['rebind', 'ordinary_rebind', oldRebind], ['construction', 'queue::live::construction', oldConstruction],
]) {
  for (const phase of ['frozen', 'restored', 'repeat-restored']) {
    const name = 'r105-' + phase + '-' + kind;
    const run = checkRun(name, [...kfd, filter], 0);
    focusedTiming[phase].push(run);
    assert.deepStrictEqual(read(name + '-source.json'), baseline);
    assert.deepStrictEqual(passingNames(logText(run)).sort(), [...names].sort());
    assert.deepStrictEqual(totals(logText(run)), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
  }
}
const mutations = read('r105-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), [
  'increment-continuation', 'early-resume-error', 'drop-continuation', 'drop-input-root',
  'skip-validation', 'extract-failed-complete', 'omit-pristine-process-poison',
  'overwrite-transport', 'omit-lane-restoration',
]);
const migratedTests = ['pristine_rebind_settlement_roots_prepared_owner_before_closing_or_validation_failure',
  'pristine_rebind_constructor_rejection_is_terminal_without_retry_authority']
  .map(n => 'queue::live::pristine_abort::tests::' + n);
const campaignRecords = [];
for (const campaign of ['', 'repeat-']) {
const mutantHashes = new Set();
const mutationTiming = [];
for (const m of mutations) {
  assert([...newTests, ...migratedTests].includes(m.test) && !m.test.includes('production_routing'));
  const name = 'r105-' + campaign + 'mut-' + m.name;
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
  const restored = read('r105-restoration-' + campaign + m.name + '.json');
  assert.strictEqual(restored.mutation, campaign + m.name);
  assert.strictEqual(restored.source_identities, paths.length);
  assert.strictEqual(restored.source_unchanged, true);
  assert.strictEqual(restored.source_map_sha256, hash(JSON.stringify(baseline)));
  const verifiedAt = Date.parse(restored.verified_at);
  assert(Number.isFinite(verifiedAt), name + ': finite restoration timestamp');
  assert(verifiedAt >= Date.parse(run.finished_at), name + ': restoration follows mutation');
  mutationTiming.push({name: m.name, started: Date.parse(run.started_at), restored: verifiedAt});
}
assert.strictEqual(mutantHashes.size, 9);
for (const run of focusedTiming.frozen) {
  assert(Date.parse(run.finished_at) <= mutationTiming[0].started, 'frozen checks precede mutations');
}
const violations = mutationTiming.slice(1).flatMap((entry, i) =>
  entry.started >= mutationTiming[i].restored ? [] : [{
    previous_mutation: mutationTiming[i].name, next_mutation: entry.name,
    prior_restored_at: new Date(mutationTiming[i].restored).toISOString(),
    next_started_at: new Date(entry.started).toISOString(),
    delta_ms: entry.started - mutationTiming[i].restored,
  }]);
assert.deepStrictEqual(violations, campaign ? [] : [{
  previous_mutation: 'omit-pristine-process-poison', next_mutation: 'overwrite-transport',
  prior_restored_at: '2026-09-12T13:46:05.206Z',
  next_started_at: '2026-09-12T13:46:05.146Z', delta_ms: -60,
}], 'restore before next mutation');
for (const run of focusedTiming[campaign + 'restored']) {
  assert(Date.parse(run.started_at) >= mutationTiming.at(-1).restored, 'restored checks follow final restoration');
}
if (campaign) {
  for (const run of focusedTiming.restored) {
    assert(Date.parse(run.finished_at) <= mutationTiming[0].started, 'repeat follows original campaign');
  }
}
campaignRecords.push({prefix: 'r105-' + campaign, accepted: Boolean(campaign),
  compiled_behavioral_mutations: mutations.length, chronology_violations: violations});
}
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test'))
  .map(r => [r.name, totals(logText(r))]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const [name, result] of Object.entries(tests)) assert.deepStrictEqual(result, {
  ...oldTests[name], passed: oldTests[name].passed + (['gnu-tests', 'musl-tests'].includes(name) ? 8 : 0),
}, name);
for (const row of auxiliary.filter(r => r.command.includes('test'))) {
  assert.deepStrictEqual(passingNames(logText(row)).sort(),
    passingNames(committed('raw/r104-' + row.name + '.log')).sort(),
    row.name + ' exact auxiliary passing multiset');
}
for (const target of ['gnu', 'musl']) {
  const oldNames = passingNames(committed('raw/r104-final-' + target + '-tests.log'));
  const names = passingNames(fs.readFileSync(path.join(root, 'r105-final-' + target + '-tests.log'), 'utf8'));
  assert.deepStrictEqual(names.sort(), [...oldNames, ...newTests].sort(), target + ' exact passing multiset');
}
const historical = [];
for (const [suffix, code, category] of [
  ['01', 101, 'compile-failure-format-overlap'], ['02', 101, 'incorrect-disposal-and-preflight-oracles'],
  ['03', 101, 'compile-failure'], ['04', 0, 'preliminary-pass-before-vm-alignment'],
  ['05', 0, 'frozen-source-preliminary-pass'],
]) {
  const name = 'r105-focused-' + suffix;
  const command = ['cargo', '+nightly-2026-04-03', 'test', '-p', 'fe2o3-kfd', '--all-features', 'pristine', '--locked', '--offline'];
  const run = checkRun(name, command, code, suffix !== '01', tested);
  const text = logText(run);
  if (category.startsWith('compile-failure')) {
    assert(text.includes('could not compile'));
    assert(!text.includes('running 1 test') && !text.includes('test result:'));
  } else {
    assert.deepStrictEqual(totals(text), suffix === '02'
      ? {harnesses: 1, passed: 18, failed: 9, ignored: 0}
      : {harnesses: 4, passed: 28, failed: 0, ignored: 0});
  }
  if (suffix === '05') {
    assert.deepStrictEqual(read(name + '-source.json'), baseline);
    assert.deepStrictEqual(passingNames(text).sort(), [...oldPristine, ...newTests].sort());
  }
  historical.push({name, category, returncode: code, source_unchanged: run.source_unchanged, accepted_campaign: false});
}
const ordinary = checkRun('r105-ordinary-01', ['cargo', '+nightly-2026-04-03', 'test', '-p',
  'fe2o3-kfd', '--all-features', 'ordinary_rebind', '--locked', '--offline'], 0, true, tested);
assert.deepStrictEqual(read('r105-ordinary-01-source.json'), baseline);
assert.deepStrictEqual(passingNames(logText(ordinary)).sort(), oldRebind.sort());
assert.deepStrictEqual(totals(logText(ordinary)), {harnesses: 4, passed: 10, failed: 0, ignored: 0});
const clippy = checkRun('r105-clippy-01', gates.find(r => r.name === 'clippy-all').command, 0, true, tested);
assert.deepStrictEqual(read('r105-clippy-01-source.json'), baseline);
assert(logText(clippy).includes('Finished `dev` profile'));
assert(fs.readFileSync(path.join(root, 'r105-final-python.log'), 'utf8').includes('Ran 151 tests'));
assert(fs.readFileSync(path.join(root, 'r105-final-dependency-tests.log'), 'utf8').includes('Ran 8 tests'));
const metadata = auxiliary.find(r => r.name === 'production-metadata');
assert.strictEqual(metadata.stderr_log, path.join(root, 'r105-production-metadata.stderr.log'));
assert(fs.statSync(metadata.stderr_log).isFile());
assert(JSON.parse(logText(metadata)).packages.length > 0);
const summary = {
  publication_parent: parent, tested_source_checkpoint: tested, accepted_runtime_checkpoint: accepted,
  planning_only_transition: {from: tested, to: parent, files: planningFiles},
  environment_record_changes_from_r104: environmentChanges,
  scope: 'Pristine rebind continuation/preparation custody through shared settlement; CPU/shared-sequence acceptance only',
  source_identities: paths.length, changed_source_files: changed,
  source_gates: gates.length, auxiliary_checks: auxiliary.length, compiled_behavioral_mutations: mutations.length,
  mutation_campaigns: campaignRecords, historical_compiled_mutation_runs: 9,
  total_compiled_mutation_runs: 18, accepted_mutation_prefix: 'r105-repeat-mut-',
  accepted_restored_prefix: 'r105-repeat-restored-',
  collector_rejection_record: 'raw/r105-collector-rejection.json',
  rejected_collector: {path: 'raw/r105-retain-local-before-repeat.js', sha256: rejectedCollectorSha256},
  new_test_functions: newTests, retained_name_migrated_test_functions: migratedTests,
  dynamic_scenarios: {preflight: 8, operation_retake: 18, opening: 3, generation: 4,
    suppressed_complete: 2, corrupted_generation: 1, preparation_stages: 62,
    migrated_settlement: 5, migrated_constructor_rejection: 1, terminal_facade: 15, total: 119},
  new_dynamic_test_functions: 8, migrated_dynamic_test_functions: 2, extended_textual_source_guards: 1,
  preparation_helper_outcomes: {runs: 96, successful_installs: 6, returned_errors: 46, caught_panics: 44},
  facade_outcomes: {scenarios: 15, terminal_transports: 15, auxiliary_vector_indices: [0, 1],
    successful_public_bind_qualified: false, transport_request_injected: true},
  focused_tests: {frozen_pristine: 28, restored_pristine: 28, frozen_rebind: 10, restored_rebind: 10,
    frozen_construction: 69, restored_construction: 69},
  tests, python_tests: {benchmark_harnesses: 151, dependency_policy: 8},
  historical_attempts: historical, preliminary_clippy: 'r105-clippy-01',
  environment_record: 'raw/r105-environment.json', issue_status_record: 'raw/r105-issue-status.json',
  environment_record_timing: environment.recorded_phase,
  metadata_stderr_retained: true, initial_formatter_output_retained: false,
  no_new_formal_source: true, proof_inventory_only: true, solver_rerun: false,
  original_engine_composition_qualified: false, later_auxiliary_vector_slots_native_qualified: false,
  live_kfd: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r105-.*\.(json|log|py|js)$/.test(p)).sort()) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
