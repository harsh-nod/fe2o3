const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = '5ba89a0e642cf02effc8e462e33be93b19aec1c2';
const accepted = 'ad6f71304ddb0f41cc102f9e6ecabd7ed7840efe';
const previous = 'docs/evidence/local-r106-coherent-initialization-custody-2026-09-12/';
const dest = path.join(repo, 'docs/evidence/local-r107-device-initialization-custody-2026-09-12');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const file = name => path.join(root, name);
const read = name => JSON.parse(fs.readFileSync(file(name), 'utf8'));
const committed = name => git(['show', accepted + ':' + previous + name]);
const passing = text => [...text.matchAll(/^test (\S+) \.\.\. ok$/gm)].map(m => m[1]).sort();
const failed = text => [...text.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(m => m[1]);
function log(run) {
  assert(fs.lstatSync(run.log).isFile(), 'regular log: ' + run.log);
  return fs.readFileSync(run.log, 'utf8');
}
function totals(text) {
  const rows = [...text.matchAll(/^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;/gm)];
  assert(rows.length, 'nonempty test summary');
  return rows.reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}), {harnesses: 0, passed: 0, failed: 0, ignored: 0});
}
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
assert.strictEqual(git(['rev-parse', parent + '^']).trim(), accepted);
assert.deepStrictEqual(git(['diff', '--name-only', accepted, parent]).trim().split('\n'), ['docs/runtime-a1-a2-swarm-current.md']);
const baseline = read('r107-frozen-source.json');
const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
  .filter(p => p && !p.startsWith('docs/')).sort();
assert.strictEqual(paths.length, 5667);
assert.deepStrictEqual(Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), baseline);
const prior = JSON.parse(committed('raw/r106-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...paths])].filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-kfd/src/shared_memory.rs',
  'crates/fe2o3-kfd/src/shared_memory/device_initialization.rs',
  'crates/fe2o3-kfd/src/shared_memory/resource_accounting.rs',
  'crates/fe2o3-kfd/src/shared_memory/tests/device_initialization.rs',
]);
const environment = read('r107-environment.json');
const oldEnvironment = JSON.parse(committed('raw/r106-environment.json'));
assert.deepStrictEqual(environment.binaries, oldEnvironment.binaries);
assert.deepStrictEqual(environment.records, oldEnvironment.records);
assert.strictEqual(environment.publication_parent, parent);
assert.strictEqual(environment.accepted_runtime_checkpoint, accepted);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
assert.deepStrictEqual(environment.runners.map(r => r.name), ['r107-run.js', 'r107-source-gate.py', 'r107-auxiliary-gates.py', 'r107-freeze.js']);
for (const runner of environment.runners) assert.strictEqual(hash(fs.readFileSync(file(runner.name))), runner.sha256);
assert.deepStrictEqual(environment.environment_overrides, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
assert.deepStrictEqual(environment.environment_removed, ['XDG_RUNTIME_DIR']);
for (const field of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(environment[field], false);
const issue = read('r107-issue-status.json');
assert.deepStrictEqual(issue.command, ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url']);
assert.strictEqual(issue.issue.number, 182);
assert.strictEqual(issue.issue.state, 'OPEN');
assert.strictEqual(issue.issue.url, 'https://github.com/harsh-nod/fe2o3/issues/182');
assert(Number.isFinite(Date.parse(issue.observed_at)) && Number.isFinite(Date.parse(issue.issue.updatedAt)));
const gates = read('r107-final-source-gate.json');
const auxiliary = read('r107-auxiliary-results.json');
for (const [rows, oldName, count, prefix] of [
  [gates, 'raw/r106-final-source-gate.json', 17, 'r107-final'],
  [auxiliary, 'raw/r106-auxiliary-results.json', 10, 'r107-auxiliary'],
]) {
  const old = JSON.parse(committed(oldName));
  assert.strictEqual(rows.length, count);
  assert.deepStrictEqual(rows.map(r => r.name), old.map(r => r.name));
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, old[i].command.map(s => s.replace('r106-production-metadata.log', 'r107-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, old[i].cwd);
    assert.strictEqual(row.log, file((prefix === 'r107-final' ? 'r107-final-' : 'r107-') + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
    assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
    log(row);
  }
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: paths.length});
}
const testPrefix = 'shared_memory::tests::device_initialization::';
const newTests = [
  'device_initializer_complete_entry_preserves_exact_success_and_readback_policy',
  'device_initializer_currentness_matrix_retains_only_admitted_owners',
  'device_initializer_native_error_and_panic_prefixes_preserve_original_source',
  'device_initializer_gpu_prefixes_and_readback_rejection_never_produce_output',
  'device_initializer_preflight_rejects_before_native_effects_despite_prior_activity',
  'device_initializer_existing_lease_preflight_retains_genuine_native_input',
  'device_initializer_in_place_core_keeps_external_custody_until_extraction',
  'device_initializer_capacity_rejection_keeps_source_host_only_and_allows_later_retry',
  'device_initializer_malformed_allocations_retain_source_without_fabricated_lease',
  'device_initializer_lower_entry_rejects_foreign_coordinates_and_account_before_effects',
].map(n => testPrefix + n).sort();
const kfd = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
function checkRun(name, command, returncode, frozen = false) {
  const run = read(name + '.json');
  assert.deepStrictEqual(run.command, command, name);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, file(name + '.log'));
  assert.strictEqual(run.source, file(name + '-source.json'));
  assert.strictEqual(run.source_head, parent);
  assert.strictEqual(run.source_unchanged, true);
  assert.strictEqual(run.returncode, returncode);
  assert.strictEqual(run.signal, null);
  assert.strictEqual(run.timed_out, false);
  assert(Number.isFinite(run.elapsed_seconds) && run.elapsed_seconds >= 0);
  assert(Number.isFinite(Date.parse(run.started_at)) && Number.isFinite(Date.parse(run.finished_at)));
  assert(Date.parse(run.finished_at) >= Date.parse(run.started_at));
  assert.deepStrictEqual(run.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  const inputs = read(name + '-source.json');
  for (const digest of Object.values(inputs)) assert(/^[0-9a-f]{64}$/.test(digest));
  if (frozen) assert.deepStrictEqual(inputs, baseline);
  log(run);
  return run;
}
const oldNames = passing(committed('raw/r106-final-gnu-tests.log'));
const borrowed = oldNames.filter(n => n.includes('borrowed_initialization') && (n.startsWith('shared_memory::') || n.startsWith('queue::')));
const transitions = passing(committed('raw/r106-transitions.log'));
assert.strictEqual(borrowed.length, 8);
assert.strictEqual(transitions.length, 30);
const focused = {frozen: [], restored: []};
for (const [kind, filter, names] of [
  ['matrix', 'shared_memory::tests::device_initialization', newTests],
  ['borrowed', 'borrowed_initialization', borrowed],
  ['transitions', 'shared_memory::tests::transitions::', transitions],
]) for (const phase of ['frozen', 'restored']) {
  const run = checkRun('r107-' + phase + '-' + kind, [...kfd, filter], 0, true);
  focused[phase].push(run);
  assert.deepStrictEqual(passing(log(run)), names);
  assert.deepStrictEqual(totals(log(run)), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
}
const pinRecord = read('r107-mutation-tool-pins.json');
assert.deepStrictEqual(Object.keys(pinRecord.pins), ['r107-mutations.json', 'r107-mutation-patch.js', 'r107-check-mutation.js', 'r107-assert-frozen.js']);
for (const [name, digest] of Object.entries(pinRecord.pins)) assert.strictEqual(hash(fs.readFileSync(file(name))), digest);
const mutations = read('r107-mutations.json');
const oracleLines = {'drop-terminal-owner': 285, 'drop-source': 171, 'omit-native-admission': 285,
  'late-input-admission': 769, 'omit-gpu-prefix': 659, 'skip-final-map': 422, 'omit-in-place-quarantine': 850};
assert.deepStrictEqual(mutations.map(m => m.name), Object.keys(oracleLines));
const mutationTimes = [];
const mutantHashes = new Set();
for (const mutation of mutations) {
  assert(newTests.includes(mutation.test));
  assert(changed.includes(mutation.path));
  assert(mutation.oracle && mutation.expected.length > 0);
  const name = 'r107-mut-' + mutation.name;
  const run = checkRun(name, [...kfd, mutation.test, '--', '--exact'], 101);
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
  assert.deepStrictEqual(paths.filter(p => inputs[p] !== baseline[p]), [mutation.path]);
  let candidate = fs.readFileSync(path.join(repo, mutation.path), 'utf8');
  for (const [from, to] of mutation.edits) {
    assert.strictEqual(candidate.split(from).length, 2);
    candidate = candidate.replace(from, to);
  }
  assert.strictEqual(hash(candidate), inputs[mutation.path]);
  mutantHashes.add(inputs[mutation.path]);
  const text = log(run);
  assert.deepStrictEqual(failed(text), [mutation.test]);
  assert.deepStrictEqual(totals(text), {harnesses: 1, passed: 0, failed: 1, ignored: 0});
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test',
    'crates/fe2o3-kfd/src/shared_memory/tests/device_initialization.rs:' + oracleLines[mutation.name] + ':',
    ...mutation.expected]) assert(text.includes(marker), mutation.name + ': ' + marker);
  const restored = read('r107-restoration-' + mutation.name + '.json');
  assert.strictEqual(restored.mutation, mutation.name);
  assert.strictEqual(restored.source_identities, paths.length);
  assert.strictEqual(restored.source_unchanged, true);
  assert.strictEqual(restored.source_map_sha256, hash(JSON.stringify(baseline)));
  assert(Date.parse(restored.verified_at) >= Date.parse(run.finished_at));
  mutationTimes.push({started: Date.parse(run.started_at), restored: Date.parse(restored.verified_at)});
}
assert.strictEqual(mutantHashes.size, 7);
assert(Date.parse(pinRecord.recorded_at) <= mutationTimes[0].started);
for (const run of focused.frozen) assert(Date.parse(run.finished_at) <= mutationTimes[0].started);
for (let i = 1; i < mutationTimes.length; i++) assert(mutationTimes[i].started >= mutationTimes[i - 1].restored);
for (const run of focused.restored) assert(Date.parse(run.started_at) >= mutationTimes.at(-1).restored);
const tests = Object.fromEntries([...gates, ...auxiliary].filter(r => r.command.includes('test')).map(r => [r.name, totals(log(r))]));
const oldTests = JSON.parse(committed('test-summary.json')).tests;
for (const [name, result] of Object.entries(tests)) assert.deepStrictEqual(result, {
  ...oldTests[name], passed: oldTests[name].passed + (['gnu-tests', 'musl-tests'].includes(name) ? 10 : 0),
}, name);
for (const row of auxiliary.filter(r => r.command.includes('test'))) assert.deepStrictEqual(passing(log(row)), passing(committed('raw/r106-' + row.name + '.log')));
for (const target of ['gnu', 'musl']) assert.deepStrictEqual(passing(fs.readFileSync(file('r107-final-' + target + '-tests.log'), 'utf8')),
  [...passing(committed('raw/r106-final-' + target + '-tests.log')), ...newTests].sort());
const historical = [];
for (const [name, command, code, count] of [
  ['r107-initial-kfd', kfd, 0, 997],
  ['r107-matrix-initial', [...kfd, 'shared_memory::tests::device_initialization'], 101, null],
  ['r107-matrix-second', [...kfd, 'shared_memory::tests::device_initialization'], 0, 10],
  ['r107-preflight-clippy', ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline', '-p', 'fe2o3-kfd', '-p', 'fe2o3-runtime', '-p', 'fe2o3-host', '--all-features', '--all-targets', '--', '-D', 'warnings'], 0, null],
  ['r107-routing', [...kfd, 'shared_memory::tests::arbitrary_source_has_one_preflight_hash_and_parallel_exact_readback', '--', '--exact'], 0, 1],
]) {
  const run = checkRun(name, command, code);
  assert(Date.parse(run.finished_at) <= Date.parse(environment.recorded_at));
  if (count !== null) assert.deepStrictEqual(totals(log(run)), {harnesses: 1, passed: count, failed: 0, ignored: 0});
  if (name === 'r107-matrix-initial') assert(log(run).includes('error[E0599]'));
  historical.push({name, returncode: code, accepted_campaign: false});
}
assert(fs.readFileSync(file('r107-final-python.log'), 'utf8').includes('Ran 151 tests'));
assert(fs.readFileSync(file('r107-final-dependency-tests.log'), 'utf8').includes('Ran 8 tests'));
const metadata = auxiliary.find(r => r.name === 'production-metadata');
assert.strictEqual(metadata.stderr_log, file('r107-production-metadata.stderr.log'));
assert(fs.lstatSync(metadata.stderr_log).isFile());
assert(JSON.parse(log(metadata)).packages.length > 0);
const summary = {publication_parent: parent, accepted_runtime_checkpoint: accepted,
  scope: 'Device initializer source/lease custody through allocation, CPU initialization and final GPU map; CPU/shared-sequence acceptance only',
  source_identities: paths.length, changed_source_files: changed,
  source_gates: gates.length, auxiliary_checks: auxiliary.length, compiled_behavioral_mutations: mutations.length,
  mutation_tool_pins: pinRecord.pins, mutation_oracle_test_lines: oracleLines,
  new_test_functions: newTests, tests,
  focused_tests: {frozen_matrix: 10, restored_matrix: 10, frozen_borrowed: 8, restored_borrowed: 8, frozen_transitions: 30, restored_transitions: 30},
  historical_attempts: historical, initial_formatter_output_retained: false,
  preliminary_catalog_executed: false, configured_capacity_case: 'combined byte and record saturation',
  no_new_formal_source: true, proof_inventory_only: true, solver_rerun: false,
  original_engine_composition_qualified: false, live_kfd: false, performance_measurement: false};
assert(!fs.existsSync(dest), 'never overwrite an evidence archive');
const retained = fs.readdirSync(root).filter(n => /^r107-.*\.(json|log|py|js)$/.test(n)).sort();
for (const name of retained) { assert(fs.lstatSync(file(name)).isFile()); fs.readFileSync(file(name)); }
fs.mkdirSync(dest);
fs.mkdirSync(path.join(dest, 'raw'));
for (const name of retained) fs.copyFileSync(file(name), path.join(dest, 'raw', name), fs.constants.COPYFILE_EXCL);
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log(JSON.stringify(summary, null, 2));
