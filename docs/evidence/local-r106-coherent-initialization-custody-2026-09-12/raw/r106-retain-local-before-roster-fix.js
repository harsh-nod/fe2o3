const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const cp = require('child_process');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '42d614d352f39fb13b4026b6d2c2b9af94a41200';
const accepted = '3ae84f8542ef82d4615f5e5ff377e2a2b076e29a';
const previous = 'docs/evidence/local-r105-pristine-rebind-custody-2026-09-12/';
const dest = path.join(repo, 'docs/evidence/local-r106-coherent-initialization-custody-2026-09-12');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 32 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(path.join(root, name), 'utf8'));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
function requireFile(file) {
  assert(fs.lstatSync(file).isFile(), file + ': regular retained file');
  fs.readFileSync(file);
}
const mutationToolPins = {
  'r106-mutations.json': '2ddbf64291676d735bb7188548015339c2ee0f2c8dc6081ebdcc99caa5e553d6',
  'r106-mutation-patch.js': '475b3a304d20afcd8bce9894cd4b8844bc4558beb4dae2a3f60796ab74b78c31',
  'r106-check-mutation.js': 'b9f48b2d96ce4367d45091cced43cca3a53393623d49ab52374e5a1392dc46c2',
  'r106-assert-frozen.js': 'e345c7fefc6a96c18de5175de4d19659f94ddd1b2979ddd1bad1a8c559612224',
};
for (const [name, digest] of Object.entries(mutationToolPins)) assert.strictEqual(hash(fs.readFileSync(path.join(root, name))), digest);
const committed = name => git(['show', accepted + ':' + previous + name]);
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
assert.strictEqual(git(['rev-parse', parent + '^']).trim(), accepted);
const planningFiles = git(['diff', '--name-only', accepted, parent]).trim().split('\n').sort();
assert.deepStrictEqual(planningFiles, ['docs/runtime-a1-a2-next-wave.md', 'docs/runtime-a1-a2-swarm-current.md']);
const baseline = read('r106-frozen-source.json');
const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
assert.deepStrictEqual(Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), baseline);
assert.strictEqual(paths.length, 5665);
const prior = JSON.parse(committed('raw/r105-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...paths])].filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/shared_memory.rs',
  'crates/fe2o3-kfd/src/shared_memory/coherent_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/dispatch_retention.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/host_backing/borrowed_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/transitions.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/transitions/coherent_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/transitions.rs',
]);
const environment = read('r106-environment.json');
const oldEnvironment = JSON.parse(committed('raw/r105-environment.json'));
assert.deepStrictEqual(environment.binaries, oldEnvironment.binaries);
assert.deepStrictEqual(environment.records, oldEnvironment.records);
for (const record of environment.records) assert.strictEqual(cp.execFileSync(record.command[0],
  record.command.slice(1), {cwd: repo, encoding: 'utf8'}), record.output);
assert.strictEqual(environment.publication_parent, parent);
assert.strictEqual(environment.accepted_runtime_checkpoint, accepted);
assert(environment.recorded_phase.startsWith('Before frozen/full/auxiliary/mutation campaigns;'));
assert(Number.isFinite(Date.parse(environment.recorded_at)));
assert.deepStrictEqual(environment.runners.map(r => r.name), [
  'r106-run.js', 'r106-source-gate.py', 'r106-auxiliary-gates.py', 'r106-freeze.js',
]);
for (const runner of environment.runners) assert.strictEqual(hash(fs.readFileSync(path.join(root, runner.name))), runner.sha256);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
assert.deepStrictEqual(environment.environment_overrides, {
  CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1',
});
assert.deepStrictEqual(environment.environment_removed, ['XDG_RUNTIME_DIR']);
for (const name of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(environment[name], false);
const issueStatus = read('r106-issue-status.json');
assert.deepStrictEqual(issueStatus.command, ['gh', 'issue', 'view', '182', '--repo',
  'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url']);
assert.strictEqual(issueStatus.issue.number, 182);
assert.strictEqual(issueStatus.issue.url, 'https://github.com/harsh-nod/fe2o3/issues/182');
assert.strictEqual(issueStatus.issue.state, 'OPEN');
assert(Number.isFinite(Date.parse(issueStatus.observed_at)) && Number.isFinite(Date.parse(issueStatus.issue.updatedAt)));
const gates = read('r106-final-source-gate.json');
const auxiliary = read('r106-auxiliary-results.json');
const oldGates = JSON.parse(committed('raw/r105-final-source-gate.json'));
const oldAuxiliary = JSON.parse(committed('raw/r105-auxiliary-results.json'));
assert.strictEqual(gates.length, 17);
assert.strictEqual(auxiliary.length, 10);
for (const [rows, oldRows, prefix] of [[gates, oldGates, 'r106-final-'], [auxiliary, oldAuxiliary, 'r106-']]) {
  assert.strictEqual(new Set(rows.map(r => r.name)).size, rows.length);
  assert.deepStrictEqual(rows.map(r => r.name), oldRows.map(r => r.name));
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, oldRows[i].command.map(s => s.replace('r105-production-metadata.log', 'r106-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, oldRows[i].cwd, row.name);
    assert.strictEqual(row.log, path.join(root, prefix + row.name + '.log'));
    requireFile(row.log);
    assert.strictEqual(row.returncode, 0, row.name);
    assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
  }
}
for (const [prefix, count] of [['r106-final', 17], ['r106-auxiliary', 10]]) {
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: 5665});
}
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const prefix = 'shared_memory::tests::transitions::coherent_initialization::';
const newTests = [
  'coherent_initializer_success_preserves_exact_storage_source_and_padded_charge',
  'coherent_initializer_empty_capacity_and_revision_reject_before_native_effects',
  'coherent_initializer_copy_currentness_keeps_actual_cpu_owner_and_written_prefix',
  'coherent_initializer_copy_panics_preserve_first_payload_without_closing_check',
  'coherent_initializer_projection_failures_keep_exact_allocation_or_mapped_successor',
  'coherent_initializer_native_allocation_failures_retain_original_pending_prefix',
  'coherent_initializer_map_failures_retain_cpu_authority_without_duplicate_custody',
  'coherent_initializer_preserves_revision_headroom_and_rejects_map_exhaustion',
  'coherent_initializer_allocation_currentness_preserves_preflight_and_pending_policy',
  'coherent_copy_invalid_private_inputs_reject_before_currentness_without_poison',
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
function checkRun(name, command, returncode, unchanged = true, expectedHead = parent) {
  const run = read(name + '.json');
  assert.deepStrictEqual(run.command, command, name);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, path.join(root, name + '.log'));
  requireFile(run.log);
  assert.strictEqual(run.source, path.join(root, name + '-source.json'));
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
  for (const digest of Object.values(inputs)) assert(/^[0-9a-f]{64}$/.test(digest));
  assert.strictEqual(run.returncode, returncode, name);
  assert.strictEqual(run.source_unchanged, unchanged, name);
  assert.strictEqual(run.source_head, expectedHead);
  assert(Number.isFinite(Date.parse(run.started_at)) && Number.isFinite(Date.parse(run.finished_at)));
  assert(Date.parse(run.started_at) <= Date.parse(run.finished_at));
  assert(Number.isFinite(run.elapsed_seconds) && run.elapsed_seconds >= 0);
  assert.strictEqual(run.timed_out, false, name);
  assert.strictEqual(run.signal, null, name);
  assert.deepStrictEqual(run.environment, runEnvironment, name);
  return run;
}
const oldNames = passingNames(committed('raw/r105-final-gnu-tests.log'));
const borrowed = oldNames.filter(n => n.includes('borrowed_initialization'));
const transitions = passingNames(committed('raw/r105-transitions.log'));
assert.strictEqual(borrowed.length, 8);
assert.strictEqual(transitions.length, 20);
const focusedTiming = {frozen: [], restored: []};
for (const [kind, filter, names] of [
  ['coherent', prefix, newTests], ['borrowed', 'borrowed_initialization', borrowed],
  ['transitions', 'shared_memory::tests::transitions::', [...transitions, ...newTests]],
]) {
  for (const phase of ['frozen', 'restored']) {
    const name = 'r106-' + phase + '-' + kind;
    const run = checkRun(name, [...kfd, filter], 0);
    focusedTiming[phase].push(run);
    assert.deepStrictEqual(read(name + '-source.json'), baseline);
    assert.deepStrictEqual(passingNames(logText(run)).sort(), [...names].sort());
    assert.deepStrictEqual(totals(logText(run)), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
  }
}
const mutations = read('r106-mutations.json');
assert.deepStrictEqual(mutations.map(m => m.name), [
  'drop-copy-owner', 'skip-copy', 'ignore-owned-panic-policy', 'skip-copy-preflight',
  'skip-map-commit', 'drop-mapped-successor', 'omit-copy-quarantine',
]);
const mutantHashes = new Set();
const mutationTiming = [];
for (const m of mutations) {
  assert([...newTests, ...borrowed].includes(m.test));
  assert.strictEqual(m.path, 'crates/fe2o3-kfd/src/shared_memory/transitions.rs');
  assert(m.oracle && m.expected.length >= 2);
  const name = 'r106-mut-' + m.name;
  const run = checkRun(name, [...kfd, m.test, '--', '--exact'], 101);
  const inputs = read(name + '-source.json');
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
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test', ...m.expected]) assert(log.includes(marker), name + ': ' + marker);
  const restored = read('r106-restoration-' + m.name + '.json');
  assert.strictEqual(restored.mutation, m.name);
  assert.strictEqual(restored.source_identities, paths.length);
  assert.strictEqual(restored.source_unchanged, true);
  assert.strictEqual(restored.source_map_sha256, hash(JSON.stringify(baseline)));
  const verifiedAt = Date.parse(restored.verified_at);
  assert(Number.isFinite(verifiedAt));
  assert(verifiedAt >= Date.parse(run.finished_at), name + ': restoration follows mutation');
  mutationTiming.push({name: m.name, started: Date.parse(run.started_at), restored: verifiedAt});
}
assert.strictEqual(mutantHashes.size, 7);
for (const run of focusedTiming.frozen) assert(Date.parse(run.finished_at) <= mutationTiming[0].started, 'frozen before mutations');
for (let i = 1; i < mutationTiming.length; i++) assert(mutationTiming[i].started >= mutationTiming[i - 1].restored, 'restore before next mutation');
for (const run of focusedTiming.restored) assert(Date.parse(run.started_at) >= mutationTiming.at(-1).restored, 'restored gates after final restoration');
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(logText(r))]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const [name, result] of Object.entries(tests)) assert.deepStrictEqual(result, {
  ...oldTests[name], passed: oldTests[name].passed + (['gnu-tests', 'musl-tests', 'transitions'].includes(name) ? 10 : 0),
}, name);
for (const row of auxiliary.filter(r => r.command.includes('test'))) assert.deepStrictEqual(passingNames(logText(row)).sort(),
  [...passingNames(committed('raw/r105-' + row.name + '.log')), ...(row.name === 'transitions' ? newTests : [])].sort(), row.name + ' exact auxiliary multiset');
for (const target of ['gnu', 'musl']) {
  const old = passingNames(committed('raw/r105-final-' + target + '-tests.log'));
  const names = passingNames(fs.readFileSync(path.join(root, 'r106-final-' + target + '-tests.log'), 'utf8'));
  assert.deepStrictEqual(names.sort(), [...old, ...newTests].sort(), target + ' exact passing multiset');
}
const historical = [];
for (const [name, command, count, unchanged] of [
  ['r106-focused-01', [...kfd, 'coherent_initializer'], 9, true],
  ['r106-format-02', ['cargo', '+nightly-2026-04-03', 'fmt', '--all'], null, false],
  ['r106-focused-02', [...kfd, prefix], 10, true],
  ['r106-borrowed-01', [...kfd, 'borrowed_initialization'], 8, true],
  ['r106-clippy-01', gates.find(r => r.name === 'clippy-all').command, null, true],
]) {
  const run = checkRun(name, command, 0, unchanged, accepted);
  if (count !== null) assert.deepStrictEqual(totals(logText(run)), {harnesses: 1, passed: count, failed: 0, ignored: 0});
  if (['r106-focused-02', 'r106-borrowed-01', 'r106-clippy-01'].includes(name)) assert.deepStrictEqual(read(name + '-source.json'), baseline);
  if (name === 'r106-clippy-01') assert(logText(run).includes('Finished `dev` profile'));
  historical.push({name, returncode: 0, source_unchanged: unchanged, accepted_campaign: false});
}
assert(fs.readFileSync(path.join(root, 'r106-final-python.log'), 'utf8').includes('Ran 151 tests'));
assert(fs.readFileSync(path.join(root, 'r106-final-dependency-tests.log'), 'utf8').includes('Ran 8 tests'));
const metadata = auxiliary.find(r => r.name === 'production-metadata');
assert.strictEqual(metadata.stderr_log, path.join(root, 'r106-production-metadata.stderr.log'));
requireFile(metadata.stderr_log);
assert(JSON.parse(logText(metadata)).packages.length > 0);
const summary = {
  publication_parent: parent, accepted_runtime_checkpoint: accepted,
  planning_only_transition: {from: accepted, to: parent, files: planningFiles},
  scope: 'Coherent initializer CPU/mapped custody through existing copy/map transitions; CPU/shared-sequence acceptance only',
  source_identities: paths.length, changed_source_files: changed,
  source_gates: gates.length, auxiliary_checks: auxiliary.length, compiled_behavioral_mutations: mutations.length,
  mutation_tool_pins: mutationToolPins,
  accepted_mutation_prefix: 'r106-mut-', accepted_restored_prefix: 'r106-restored-',
  new_test_functions: newTests, tests,
  focused_tests: {frozen_coherent: 10, restored_coherent: 10, frozen_borrowed: 8, restored_borrowed: 8,
    frozen_transitions: 30, restored_transitions: 30},
  dynamic_scenarios: {success: 6, preflight: 6, copy_currentness: 8, copy_panic: 24, projection: 32,
    native_allocation: 16, native_map: 22, revision: 4, allocation_currentness: 12, private_inputs: 5,
    total: 135, additional_successful_pre_effect_retry_controls: 1},
  initializer_outcomes_excluding_anchors_and_retry_control: {success: 8, returned_error: 60, caught_panic: 62, total: 130},
  direct_private_copy_outcomes: {returned_error: 5},
  python_tests: {benchmark_harnesses: 151, dependency_policy: 8}, historical_attempts: historical,
  environment_record: 'raw/r106-environment.json', issue_status_record: 'raw/r106-issue-status.json',
  metadata_stderr_retained: true, initial_formatter_output_retained: false,
  no_new_formal_source: true, proof_inventory_only: true, solver_rerun: false,
  original_engine_composition_qualified: false, live_kfd: false, performance_measurement: false,
};
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of fs.readdirSync(root).filter(p => /^r106-.*\.(json|log|py|js)$/.test(p)).sort()) {
  fs.copyFileSync(path.join(root, name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
