const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const clock = require('./r108-clock-evidence.js');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = 'f767fad35d24f585e8ecb95558bce97a9d543cfe';
const accepted = '589c6f6fd90048643a57cc4d0e250cb9698550a6';
const previous = 'docs/evidence/local-r107-device-initialization-custody-2026-09-12/';
const dest = path.join(repo, 'docs/evidence/local-r108-context-writer-issuance-2026-09-12');
const file = name => path.join(root, name);
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(file(name), 'utf8'));
const committed = name => git(['show', accepted + ':' + previous + name]);
const passing = text => [...text.matchAll(/^test (.+) \.\.\. ok$/gm)].map(m => m[1]).sort();
const failed = text => [...text.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(m => m[1]);
const time = value => { const n = Date.parse(value); assert(Number.isFinite(n)); return n; };
const output = row => {
  assert(fs.lstatSync(row.log).isFile());
  return fs.readFileSync(row.log, 'utf8');
};
function totals(text) {
  const rows = [...text.matchAll(/^test result: (?:ok|FAILED)\. (\d+) passed; (\d+) failed; (\d+) ignored;/gm)];
  assert(rows.length, 'nonempty test summary');
  return rows.reduce((a, m) => ({harnesses: a.harnesses + 1, passed: a.passed + Number(m[1]),
    failed: a.failed + Number(m[2]), ignored: a.ignored + Number(m[3])}),
  {harnesses: 0, passed: 0, failed: 0, ignored: 0});
}
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
assert.strictEqual(git(['rev-parse', parent + '^']).trim(), accepted);
assert.deepStrictEqual(git(['diff', '--name-only', accepted, parent]).trim().split('\n'),
  ['docs/runtime-a1-a2-next-wave.md', 'docs/runtime-a1-a2-swarm-current.md']);
const baseline = read('r108-frozen-source.json');
const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
  .filter(p => p && !p.startsWith('docs/')).sort();
assert.strictEqual(paths.length, 5669);
assert.deepStrictEqual(Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))])), baseline);
const prior = JSON.parse(committed('raw/r107-accepted-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...paths])].filter(p => prior[p] !== baseline[p]).sort();
assert.deepStrictEqual(changed, [
  'crates/fe2o3-runtime-model/src/context_version_journal.rs',
  'crates/fe2o3-runtime-model/src/context_version_journal/tests.rs',
  'crates/fe2o3-runtime-model/src/lib.rs',
]);
const environment = read('r108-environment.json');
assert.strictEqual(environment.publication_parent, parent);
assert.strictEqual(environment.accepted_runtime_checkpoint, accepted);
assert.deepStrictEqual(environment.binaries, JSON.parse(committed('raw/r107-accepted-environment.json')).binaries);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
assert.deepStrictEqual(environment.runners.map(r => r.name),
  ['r108-run.js', 'r108-source-gate.py', 'r108-auxiliary-gates.py', 'r108-freeze.js']);
for (const runner of environment.runners) assert.strictEqual(hash(fs.readFileSync(file(runner.name))), runner.sha256);
assert.deepStrictEqual(environment.environment_overrides,
  {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
assert.deepStrictEqual(environment.environment_removed, ['XDG_RUNTIME_DIR']);
for (const field of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(environment[field], false);
const preliminary = ['initial-format', 'boundary-format', 'initial-model', 'initial-clippy', 'unmasked-format'];
assert.deepStrictEqual(environment.prior_artifacts.map(r => r.name), preliminary.flatMap(name =>
  ['.json', '.log', '-source.json'].map(suffix => 'r108-' + name + suffix)).sort());
for (const artifact of environment.prior_artifacts) assert.strictEqual(hash(fs.readFileSync(file(artifact.name))), artifact.sha256);
for (const name of preliminary) {
  const run = read('r108-' + name + '.json');
  assert.strictEqual(run.returncode, 0);
  assert.strictEqual(run.signal, null);
  assert.strictEqual(run.timed_out, false);
  assert.strictEqual(run.source_head, name === 'initial-format' ? accepted : parent);
  assert.strictEqual(run.source_unchanged, !['initial-format', 'boundary-format'].includes(name));
  assert(time(run.finished_at) <= time(environment.recorded_at));
  assert.deepStrictEqual(Object.keys(read('r108-' + name + '-source.json')).sort(), paths);
}
const issue = read('r108-issue-status.json');
assert.deepStrictEqual(issue.command, ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url']);
assert.strictEqual(issue.issue.number, 182);
assert.strictEqual(issue.issue.state, 'OPEN');
assert.strictEqual(issue.issue.url, 'https://github.com/harsh-nod/fe2o3/issues/182');
time(issue.observed_at);
const revalidation = read('r108-accepted-revalidation.json');
assert.strictEqual(revalidation.source_map_sha256, hash(JSON.stringify(baseline)));
assert.strictEqual(revalidation.between_mutations_ms, 5000);
assert.strictEqual(revalidation.test_environment_changed, false);
assert.strictEqual(revalidation.test_deadlines_changed, false);
const priorNames = fs.readdirSync(root).filter(name => /^r108-.*\.(json|log|py|js)$/.test(name)
  && !name.startsWith('r108-accepted-') && !name.startsWith('r108-repeat-')
  && !name.startsWith('r108-restoration-repeat-') && !name.startsWith('r108-clock-')).sort();
assert.deepStrictEqual(revalidation.prior_artifacts.map(r => r.name), priorNames);
assert.strictEqual(priorNames.length, 140);
assert.deepStrictEqual(revalidation.runners.map(r => r.name),
  ['r108-accepted-prepare.js', 'r108-accepted-auxiliary-gates.py', 'r108-accepted-retain-local.js']);
for (const artifact of revalidation.prior_artifacts) assert.strictEqual(hash(fs.readFileSync(file(artifact.name))), artifact.sha256);
for (const runner of revalidation.runners) assert.strictEqual(hash(fs.readFileSync(file(runner.name))), runner.sha256);
const originalMutations = read('r108-mutations.json');
const inversions = [];
for (let i = 1; i < originalMutations.length; i++) {
  const previous = originalMutations[i - 1].name;
  const current = originalMutations[i].name;
  const restored = read('r108-restoration-' + previous + '.json').verified_at;
  const started = read('r108-mut-' + current + '.json').started_at;
  const delta_ms = time(started) - time(restored);
  if (delta_ms < 0) inversions.push({previous, current, restored, started, delta_ms});
}
assert.deepStrictEqual(inversions, revalidation.original_clock_inversions);
assert.deepStrictEqual(inversions.map(r => [r.previous, r.current, r.delta_ms]),
  [['slot-only', 'omit-kind', -55], ['linear-lookup', 'registration-growth', -349]]);
const originalLinux = fs.readFileSync(file('r108-linux-helpers.log'), 'utf8');
assert.strictEqual(passing(originalLinux).length, 19);
assert.strictEqual(totals(originalLinux).passed, 20);
assert(originalLinux.includes('test queue_linux::tests::teardown_arm_attempt_after_admission_check_linearizes_after_lease ... \nrunning 1 test\nok\n'));
const originalValidation = read('r108-original-collector-validation.json');
assert.strictEqual(originalValidation.returncode, 1);
assert.strictEqual(originalValidation.source_unchanged, true);
assert(output(originalValidation).includes('linux-helpers exact passing roster'));
assert(time(originalValidation.finished_at) <= time(revalidation.recorded_at));
const clockManifest = read('r108-clock-manifest.json');
const helperNames = ['r108-clock-check-mutation.js', 'r108-clock-evidence-tests.js', 'r108-clock-evidence.js',
  'r108-clock-prepare.js', 'r108-clock-restoration.js', 'r108-clock-retain-local.js', 'r108-clock-run.js'];
assert.deepStrictEqual(clockManifest.helpers.map(r => r.name), helperNames);
for (const item of [...clockManifest.helpers, ...clockManifest.historical_artifacts, ...clockManifest.contract_tests]) {
  assert(fs.lstatSync(file(item.name)).isFile());
  assert.strictEqual(hash(fs.readFileSync(file(item.name))), item.sha256);
}
const historicalNames = fs.readdirSync(root).filter(n => /^r108-.*\.(json|log|py|js)$/.test(n) && !n.startsWith('r108-clock-')).sort();
assert.deepStrictEqual(clockManifest.historical_artifacts.map(r => r.name), historicalNames);
assert.strictEqual(historicalNames.length, 210);
assert.strictEqual(new Set(historicalNames).size, historicalNames.length);
assert.strictEqual(clockManifest.source_head, parent);
assert.strictEqual(clockManifest.source_map_sha256, hash(JSON.stringify(baseline)));
assert.strictEqual(clockManifest.test_environment_changed, false);
assert.strictEqual(clockManifest.test_deadlines_changed, false);
assert.deepStrictEqual(clockManifest.clock.policy, clock.policy);
assert.strictEqual(clockManifest.clock.contract, clock.contract);
assert.strictEqual(clockManifest.clock.kind, 'manifest');
const floorRoster = [
  ['r108-source-campaign.json', 'finished_at'], ['r108-accepted-auxiliary-campaign.json', 'finished_at'],
  ...['model', 'credits', 'batch'].map(n => ['r108-frozen-' + n + '.json', 'finished_at']),
  ['r108-environment.json', 'recorded_at'], ['r108-mutation-tool-pins.json', 'recorded_at'],
  ['r108-accepted-revalidation.json', 'recorded_at'],
  ...originalMutations.flatMap(m => [['r108-repeat-mut-' + m.name + '.json', 'finished_at'], ['r108-restoration-repeat-' + m.name + '.json', 'verified_at']]),
  ['r108-clock-contract-tests.json', 'finished_at'],
];
assert.deepStrictEqual(clockManifest.prerequisite_floors.map(r => [r.name, r.field]), floorRoster);
for (const floor of clockManifest.prerequisite_floors) {
  assert.strictEqual(hash(fs.readFileSync(file(floor.name))), floor.sha256);
  assert.strictEqual(time(read(floor.name)[floor.field]), floor.utc_ms);
}
clock.verifyGate(clockManifest.clock.observation, Math.max(...clockManifest.prerequisite_floors.map(f => f.utc_ms)));
assert.strictEqual(time(clockManifest.recorded_at), clockManifest.clock.observation.samples.at(-1).utc_ms);
const repeatedInversions = [];
let priorRepeatRestore = 0;
for (const m of originalMutations) {
  const run = read('r108-repeat-mut-' + m.name + '.json');
  const receipt = read('r108-restoration-repeat-' + m.name + '.json');
  assert(time(run.started_at) >= priorRepeatRestore);
  assert(time(run.finished_at) >= time(run.started_at));
  const delta_ms = time(receipt.verified_at) - time(run.finished_at);
  if (delta_ms < 0) repeatedInversions.push({name: m.name, finished_at: run.finished_at, verified_at: receipt.verified_at, delta_ms});
  priorRepeatRestore = time(receipt.verified_at);
}
assert.deepStrictEqual(repeatedInversions.map(r => [r.name, r.delta_ms]), [['slot-only', -236]]);
const plannedNames = originalMutations.flatMap(m => ['r108-clock-mut-' + m.name + '.json', 'r108-clock-restoration-' + m.name + '.json'])
  .concat(['model', 'credits', 'batch'].map(n => 'r108-clock-restored-' + n + '.json'), ['r108-clock-collector-validation.json']);
assert.deepStrictEqual(Object.keys(clockManifest.entries), plannedNames);
let expectedPredecessor = {name: 'r108-clock-manifest.json', field: 'recorded_at'};
for (const name of plannedNames) {
  const entry = clockManifest.entries[name];
  assert.deepStrictEqual(entry.predecessor, expectedPredecessor);
  const restoration = name.startsWith('r108-clock-restoration-');
  assert.strictEqual(entry.kind, restoration ? 'restoration' : 'run');
  expectedPredecessor = {name, field: restoration ? 'verified_at' : 'finished_at'};
}
function checkClock(name, record, restoration = false) {
  const planned = clock.plan(name + '.json');
  assert.strictEqual(record.manifest_sha256, hash(fs.readFileSync(file('r108-clock-manifest.json'))));
  assert.strictEqual(record.source_map_sha256, planned.entry.source_map_sha256);
  assert.strictEqual(record.clock.contract, clock.contract);
  assert.deepStrictEqual(record.clock.policy, clock.policy);
  assert.strictEqual(record.clock.kind, restoration ? 'restoration' : 'run');
  const previous = clock.predecessor(planned.expected.name, planned.expected.field, planned.expected);
  assert.deepStrictEqual(record.clock.predecessor, previous);
  if (restoration) {
    clock.verifyGate(record.clock.observation, previous.utc_ms);
    assert.strictEqual(time(record.verified_at), record.clock.observation.samples.at(-1).utc_ms);
  } else {
    assert.strictEqual(record.clock.error, null);
    clock.verifyGate(record.clock.start, previous.utc_ms);
    const start = record.clock.start.samples.at(-1);
    clock.verifyFinish(start, record.clock.finish);
    assert.strictEqual(time(record.started_at), start.utc_ms);
    assert.strictEqual(time(record.finished_at), record.clock.finish.utc_ms);
    assert(BigInt(record.clock.source_verified.monotonic_ns) >= BigInt(record.clock.finish.monotonic_ns));
    assert.strictEqual(record.elapsed_seconds, Number(BigInt(record.clock.finish.monotonic_ns) - BigInt(start.monotonic_ns)) / 1e9);
    assert.strictEqual(record.verification_elapsed_seconds,
      Number(BigInt(record.clock.source_verified.monotonic_ns) - BigInt(record.clock.finish.monotonic_ns)) / 1e9);
    assert.deepStrictEqual(record.command, planned.entry.command);
    assert.strictEqual(record.returncode, planned.entry.returncode);
  }
}
const gates = read('r108-final-source-gate.json');
const auxiliary = read('r108-accepted-auxiliary-results.json');
for (const [rows, oldName, count, prefix, logPrefix] of [
  [gates, 'raw/r107-accepted-source-gate.json', 17, 'r108-final', 'r108-final'],
  [auxiliary, 'raw/r107-accepted-auxiliary-results.json', 10, 'r108-accepted-auxiliary', 'r108-accepted'],
]) {
  const old = JSON.parse(committed(oldName));
  assert.strictEqual(rows.length, count);
  assert.deepStrictEqual(rows.map(r => r.name), old.map(r => r.name));
  for (const [i, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, old[i].command.map(s => s.replace('r107-accepted-production-metadata.log', 'r108-accepted-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, old[i].cwd);
    assert.strictEqual(row.log, file(logPrefix + '-' + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
    assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
    const text = output(row);
    const cargoTest = row.command[0] === 'cargo' && row.command[2] === 'test';
    if (cargoTest && !['gnu-tests', 'musl-tests'].includes(row.name)) {
      const oldText = committed('raw/r107-accepted-' + row.name + '.log');
      assert.deepStrictEqual(passing(text), passing(oldText), row.name + ' exact passing roster');
      assert.deepStrictEqual(totals(text), totals(oldText), row.name + ' exact harness totals');
    }
    if (row.name === 'production-metadata') {
      assert.strictEqual(row.stderr_log, file('r108-accepted-production-metadata.stderr.log'));
      assert(fs.lstatSync(row.stderr_log).isFile());
      fs.readFileSync(row.stderr_log);
      const metadata = JSON.parse(text);
      assert.strictEqual(metadata.workspace_root, repo);
      assert(metadata.packages.some(p => p.name === 'fe2o3-runtime'));
      assert(Array.isArray(metadata.resolve.nodes) && metadata.resolve.nodes.length);
    } else {
      assert.strictEqual(row.stderr_log, undefined);
    }
  }
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: paths.length});
}
const testPrefix = 'context_version_journal::tests::';
const newTests = [
  'constructor_has_explicit_independent_bounds_and_zero_watermark',
  'existing_ids_preserve_gaps_and_older_reserved_lookup_and_abort',
  'abort_never_rolls_back_watermark_or_revives_old_keys',
  'dropped_reference_retains_capacity_and_full_rejection_keeps_watermark',
  'full_key_and_slot_checks_reject_replay_and_every_substitution',
  'checked_add_allocator_edges_accept_max_minus_one_and_reject_max',
  'indexed_work_is_constant_and_storage_does_not_grow',
  'rejection_work_is_bounded_in_full_journals',
  'internal_slot_and_capacity_invariants_reject_before_mutation',
  'short_traces_match_independent_active_writer_map',
  'runtime_identity_and_operation_routing_contract_is_explicit',
].map(n => testPrefix + n).sort();
const model = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-runtime-model', '--lib'];
function checkRun(name, command, returncode = 0, frozen = true) {
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
  assert(time(run.started_at) >= time(environment.recorded_at));
  assert(time(run.finished_at) >= time(run.started_at));
  assert.deepStrictEqual(run.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  if (frozen) assert.deepStrictEqual(read(name + '-source.json'), baseline);
  if (clockManifest.entries[name + '.json']) {
    assert.strictEqual(run.source_map_sha256, hash(JSON.stringify(read(name + '-source.json'))));
    checkClock(name, run);
  }
  output(run);
  return run;
}
const full = {};
for (const target of ['gnu', 'musl']) {
  const text = output(gates.find(r => r.name === target + '-tests'));
  const old = committed('raw/r107-accepted-' + target + '-tests.log');
  assert.deepStrictEqual(passing(text), [...passing(old), ...newTests].sort());
  const expected = {...totals(old), passed: totals(old).passed + newTests.length};
  assert.deepStrictEqual(totals(text), expected);
  assert.deepStrictEqual(expected, {harnesses: 48, passed: 2605, failed: 0, ignored: 5});
  full[target] = expected;
}
const sourceCampaign = checkRun('r108-source-campaign', ['python3', '-B', file('r108-source-gate.py'), 'r108-final']);
const clockTests = checkRun('r108-clock-contract-tests', ['node', file('r108-clock-evidence-tests.js')]);
assert.deepStrictEqual(clockManifest.contract_tests.map(r => r.name),
  ['r108-clock-contract-tests.json', 'r108-clock-contract-tests.log', 'r108-clock-contract-tests-source.json']);
assert(output(clockTests).endsWith('PASS: all 16 evidence-clock contract tests\n'));
assert.strictEqual(output(clockTests).split('\n').filter(line => line.startsWith('PASS: ')).length, 17);
const testedHelperPins = [...output(clockTests).matchAll(/^HELPER_PINS: (.+)$/gm)];
assert.strictEqual(testedHelperPins.length, 1);
assert.deepStrictEqual(JSON.parse(testedHelperPins[0][1]), clockManifest.helpers, 'tested external helper identities');
const auxCampaign = checkRun('r108-accepted-auxiliary-campaign', ['python3', '-B', file('r108-accepted-auxiliary-gates.py')]);
assert(time(auxCampaign.started_at) >= time(sourceCampaign.finished_at));
assert(time(auxCampaign.started_at) >= time(revalidation.recorded_at));
const oldNames = passing(committed('raw/r107-accepted-gnu-tests.log'));
const focused = {frozen: [], restored: []};
for (const [kind, filter, names] of [
  ['model', 'context_version_journal', newTests],
  ['credits', 'r67_resource_credits', oldNames.filter(n => n.startsWith('r67_resource_credits::'))],
  ['batch', 'r70_resource_batch', oldNames.filter(n => n.startsWith('r70_resource_batch::'))],
]) for (const phase of ['frozen', 'restored']) {
  assert(names.length);
  const run = checkRun((phase === 'restored' ? 'r108-clock-' : 'r108-') + phase + '-' + kind, [...model, filter]);
  assert.deepStrictEqual(passing(output(run)), names);
  assert.deepStrictEqual(totals(output(run)), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
  focused[phase].push(run);
}
const pins = read('r108-mutation-tool-pins.json');
assert.deepStrictEqual(Object.keys(pins.pins), ['r108-mutations.json', 'r108-prepare-mutations.js',
  'r108-mutation-patch.js', 'r108-check-mutation.js', 'r108-assert-frozen.js']);
for (const [name, digest] of Object.entries(pins.pins)) assert.strictEqual(hash(fs.readFileSync(file(name))), digest);
const mutations = read('r108-mutations.json');
const oracleLines = {'lookup-watermark': 153, 'abort-watermark': 176, 'capacity-watermark': 85,
  'slot-only': 91, 'omit-kind': 91, 'retain-free-slot': 70, 'reject-max-minus-one': 277,
  'admit-max': 84, 'linear-lookup': 324, 'registration-growth': 328,
  'omit-writer-capacity': 407, 'omit-storage-capacity': 407};
assert.deepStrictEqual(mutations.map(m => m.name), Object.keys(oracleLines));
let lastRestoration = Math.max(time(sourceCampaign.finished_at), time(auxCampaign.finished_at),
  ...focused.frozen.map(r => time(r.finished_at)), time(pins.recorded_at), time(revalidation.recorded_at), time(clockManifest.recorded_at));
const mutatedHashes = new Set();
for (const mutation of mutations) {
  assert(newTests.includes(mutation.test));
  assert.strictEqual(mutation.path, 'crates/fe2o3-runtime-model/src/context_version_journal.rs');
  assert.strictEqual(mutation.oracle_line, oracleLines[mutation.name]);
  assert(mutation.oracle && mutation.expected.length);
  const name = 'r108-clock-mut-' + mutation.name;
  const run = checkRun(name, [...model, mutation.test, '--', '--exact'], 101, false);
  assert(time(run.started_at) >= lastRestoration);
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
  assert.deepStrictEqual(paths.filter(p => inputs[p] !== baseline[p]), [mutation.path]);
  let candidate = fs.readFileSync(path.join(repo, mutation.path), 'utf8');
  for (const [from, to] of mutation.edits) {
    assert.strictEqual(candidate.split(from).length, 2);
    candidate = candidate.replace(from, to);
  }
  assert.strictEqual(hash(candidate), inputs[mutation.path]);
  mutatedHashes.add(inputs[mutation.path]);
  const text = output(run);
  assert.deepStrictEqual(failed(text), [mutation.test]);
  assert.deepStrictEqual(totals(text), {harnesses: 1, passed: 0, failed: 1, ignored: 0});
  for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test', ...mutation.expected]) assert(text.includes(marker), name + ': ' + marker);
  assert(text.includes('crates/fe2o3-runtime-model/src/context_version_journal/tests.rs:' + mutation.oracle_line + ':'));
  const restoration = read('r108-clock-restoration-' + mutation.name + '.json');
  checkClock('r108-clock-restoration-' + mutation.name, restoration, true);
  assert.strictEqual(restoration.source_head, parent);
  assert.strictEqual(restoration.mutation, mutation.name);
  assert.strictEqual(restoration.source_identities, paths.length);
  assert.strictEqual(restoration.source_unchanged, true);
  assert.strictEqual(restoration.source_map_sha256, hash(JSON.stringify(baseline)));
  assert(time(restoration.verified_at) >= time(run.finished_at));
  lastRestoration = time(restoration.verified_at);
}
assert.strictEqual(mutatedHashes.size, mutations.length);
for (const run of focused.restored) assert(time(run.started_at) >= lastRestoration);
const summary = {
  scope: 'V1 model-only local source acceptance; no production journal, authenticated formal, native or performance qualification',
  collector_sha256: hash(fs.readFileSync(__filename)),
  publication_parent: parent, accepted_runtime_checkpoint: accepted, source_identities: paths.length,
  source_delta: changed, new_tests: newTests, full, source_gates: gates.length, auxiliary_gates: auxiliary.length,
  focused: {model: 11, credits: 3, batch: 6}, compiled_negatives: mutations.length,
  finite_trace_domain: {actions: 9, depth: 4, writer_capacities: [1, 3], traces: 13122, steps: 52488},
  preliminary_artifacts: environment.prior_artifacts,
  original_cohort: {accepted: false, clock_inversions: inversions, linux_helper_named_passes: 19,
    linux_helper_summary_passes: 20, collector_returncode: originalValidation.returncode,
    preserved_artifacts: revalidation.prior_artifacts},
  repeated_mutation_cohort: {accepted: false, clock_inversions: repeatedInversions},
  evidence_clock: {contract: clock.contract, policy: clock.policy, contract_tests: 16,
    historical_artifacts: clockManifest.historical_artifacts, helpers: clockManifest.helpers},
  accepted_mutation_cohort: 'r108-clock-mut-',
  limits: ['inert identity projections, not authentic Context extraction', 'constructor allocation failure not injected',
    'primitive counters and textual source guards, not a general complexity proof or allocator measurement',
    'no membership/settlement, production journal, mutation hooks or cross-run leases',
    'proof inventory is not a solver run; no new formal, Linux/KFD, performance or aggregate-memory acceptance'],
};
const validationText = JSON.stringify(summary, null, 2) + '\n'
  + 'PASS: full/focused/auxiliary rosters, 12 compiled negatives, exact restoration and chronology\n';
if (process.argv[2] === '--check') {
  process.stdout.write(validationText);
  process.exit(0);
}
assert.strictEqual(process.argv.length, 2);
const validation = checkRun('r108-clock-collector-validation', ['node', file('r108-clock-retain-local.js'), '--check']);
assert(time(validation.started_at) >= Math.max(...focused.restored.map(r => time(r.finished_at))));
assert.strictEqual(output(validation), validationText, 'closed transcript binds this exact collector and summary');
const names = fs.readdirSync(root).filter(name => /^r108-.*\.(json|log|py|js)$/.test(name)).sort();
summary.raw_artifacts = names.map(name => {
  assert(fs.lstatSync(file(name)).isFile());
  return {name, sha256: hash(fs.readFileSync(file(name)))};
});
fs.mkdirSync(dest, {recursive: true});
fs.mkdirSync(path.join(dest, 'raw'));
for (const artifact of summary.raw_artifacts) {
  const target = path.join(dest, 'raw', artifact.name);
  fs.copyFileSync(file(artifact.name), target, fs.constants.COPYFILE_EXCL);
  assert.strictEqual(hash(fs.readFileSync(target)), artifact.sha256);
}
fs.writeFileSync(path.join(dest, 'test-summary.json'), JSON.stringify(summary, null, 2) + '\n', {flag: 'wx'});
console.log('Retained ' + names.length + ' exact raw artifacts at ' + dest);
