const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const clock = require('./r111-exact-clock-evidence.js');
const p = require('./r111-exact-qualification-plan.js');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const dest = path.join(repo, 'docs/evidence/local-r111-uninitialized-coherent-insertion-2026-09-12');
const file = name => path.join(root, name);
const hash = clock.hash;
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const read = name => JSON.parse(fs.readFileSync(file(name), 'utf8'));
const committed = name => git(['show', p.accepted + ':' + p.previous + name]);
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
assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), p.parent);
assert.strictEqual(p.parent, p.accepted);
const baseline = read('r111-frozen-source.json');
const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
  .filter(n => n && !n.startsWith('docs/')).sort();
assert.strictEqual(paths.length, 5676);
assert.deepStrictEqual(Object.fromEntries(paths.map(n => [n, hash(fs.readFileSync(path.join(repo, n)))])), baseline);
const prior = JSON.parse(committed('raw/r110-frozen-source.json'));
const changed = [...new Set([...Object.keys(prior), ...paths])].filter(n => prior[n] !== baseline[n]).sort();
assert.deepStrictEqual(changed, p.sourceDelta);
const environment = read('r111-environment.json');
assert.strictEqual(environment.publication_parent, p.parent);
assert.strictEqual(environment.accepted_runtime_checkpoint, p.accepted);
assert.deepStrictEqual(environment.binaries, JSON.parse(committed('raw/r110-environment.json')).binaries);
for (const binary of environment.binaries) assert.strictEqual(hash(fs.readFileSync(binary.path)), binary.sha256);
assert.deepStrictEqual(environment.runners.map(r => r.name),
  ['r111-run.js', 'r111-campaign-run.js', 'r111-source-gate.py', 'r111-auxiliary-gates.py', 'r111-freeze.js']);
for (const runner of environment.runners) assert.strictEqual(hash(fs.readFileSync(file(runner.name))), runner.sha256);
assert.deepStrictEqual(environment.environment_overrides,
  {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'});
assert.deepStrictEqual(environment.environment_removed, ['XDG_RUNTIME_DIR']);
for (const field of ['test_deadlines_changed', 'hardware_qualification', 'solver_rerun']) assert.strictEqual(environment[field], false);
assert.deepStrictEqual(environment.campaign_envelope, {previous_deadline_ms: 7200000, deadline_ms: 7200000,
  individual_gate_deadline_ms: 1800000, reason: "Retain R110's serialized campaign envelope; individual gate and test deadlines are unchanged."});
const preliminary = ['initial-format', 'initial-build', 'root-format', 'root-public-tests',
  'root-public-tests-v2', 'matrix-format', 'constructed-tests', 'review-format', 'reviewed-tests',
  'coherent-regressions', 'device-regressions', 'review-clippy', 'final-format', 'clean-clippy',
  'closing-hook', 'model-wrapper', 'format-check', 'whitespace-check'];
assert.strictEqual(preliminary.length, 18);
assert.deepStrictEqual(environment.prior_artifacts.map(r => r.name), preliminary.flatMap(name =>
  ['.json', '.log', '-source.json'].map(suffix => 'r111-' + name + suffix)).sort());
for (const artifact of environment.prior_artifacts) assert.strictEqual(hash(fs.readFileSync(file(artifact.name))), artifact.sha256);
const preliminaryFailures = [];
for (const [index, name] of preliminary.entries()) {
  const run = read('r111-' + name + '.json');
  const rejected = ['initial-build', 'root-public-tests', 'review-clippy'].includes(name);
  assert.strictEqual(run.returncode, rejected ? 101 : 0);
  assert.strictEqual(run.signal, null);
  assert.strictEqual(run.timed_out, false);
  assert.strictEqual(run.source_head, p.parent);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, file('r111-' + name + '.log'));
  assert.strictEqual(run.source, file('r111-' + name + '-source.json'));
  assert.strictEqual(Object.keys(read('r111-' + name + '-source.json')).length, index < 5 ? 5675 : 5676);
  assert.strictEqual(run.source_unchanged, !['initial-format', 'root-format', 'matrix-format', 'review-format'].includes(name));
  assert(time(run.finished_at) <= time(environment.recorded_at));
  if (rejected) preliminaryFailures.push({name, returncode: run.returncode});
}
assert(output(read('r111-initial-build.json')).includes('error[E0405]'));
assert(output(read('r111-initial-build.json')).includes('DataInsertionRootV1'));
assert(output(read('r111-root-public-tests.json')).includes('error[E0599]'));
assert(output(read('r111-root-public-tests.json')).includes('is_none'));
const lintFailure = output(read('r111-review-clippy.json'));
assert(lintFailure.includes('coherent_assert_model_v1'));
assert(lintFailure.includes('dead-code'));
const priorPassing = passing(committed('raw/r110-final-gnu-tests.log'));
const rootPublic = p.newTests.filter(n => n.startsWith('shared_memory::') || n.startsWith('queue::live::rebind_tests::'));
assert.strictEqual(rootPublic.length, 7);
const preliminaryRosters = [
  ['root-public-tests-v2', rootPublic],
  ['constructed-tests', p.newTests],
  ['reviewed-tests', p.newTests],
  ['coherent-regressions', priorPassing.filter(n => n.includes('coherent_insertion_'))],
  ['device-regressions', priorPassing.filter(n => n.includes('device_insertion_'))],
  ['closing-hook', p.newTests.filter(n => n.endsWith('operation_panic_wins_secondary_retake_error_or_panic'))],
  ['model-wrapper', priorPassing.filter(n => n.endsWith('coherent_insertion_constructed_success_preserves_exact_order_source_and_accounts'))],
];
assert.deepStrictEqual(preliminaryRosters.map(([, names]) => names.length), [7, 19, 19, 19, 14, 1, 1]);
for (const [name, names] of preliminaryRosters) {
  const text = output(read('r111-' + name + '.json'));
  assert.deepStrictEqual(passing(text), names);
  assert.deepStrictEqual(totals(text), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
}
for (const name of ['reviewed-tests', 'coherent-regressions', 'device-regressions']) {
  assert(output(read('r111-' + name + '.json')).includes('coherent_assert_model_v1'));
}
const formatting = read('r111-matrix-format.json');
const overlapping = read('r111-constructed-tests.json');
assert(time(formatting.started_at) < time(overlapping.started_at));
assert(time(formatting.finished_at) < time(overlapping.finished_at));
assert.strictEqual(time(formatting.finished_at) - time(overlapping.started_at), 12534);
const preliminaryExclusions = [{
  name: 'constructed-tests', returncode: 0, passed: 19, overlap_ms: 12534,
  reason: 'Overlapped source-writing matrix-format; unchanged endpoint hashes do not prove continuously immutable source.',
}];
const issue = read('r111-issue-status.json');
assert.deepStrictEqual(issue.command, ['gh', 'issue', 'view', '182', '--repo', 'harsh-nod/fe2o3', '--json', 'number,title,state,updatedAt,url']);
assert.strictEqual(issue.issue.number, 182);
assert.strictEqual(issue.issue.state, 'OPEN');
assert.strictEqual(issue.issue.url, 'https://github.com/harsh-nod/fe2o3/issues/182');
time(issue.observed_at);
const manifest = read('r111-exact-clock-manifest.json');
assert.deepStrictEqual(manifest.helpers.map(r => r.name), p.helpers);
for (const item of [...manifest.helpers, ...manifest.historical_artifacts, ...manifest.contract_tests]) {
  assert(fs.lstatSync(file(item.name)).isFile());
  assert.strictEqual(hash(fs.readFileSync(file(item.name))), item.sha256);
}
const historicalNames = fs.readdirSync(root).filter(n => /^r111-.*\.(json|log|py|js)$/.test(n) && !n.startsWith('r111-exact-clock-')).sort();
assert.deepStrictEqual(manifest.historical_artifacts.map(r => r.name), historicalNames);
assert.strictEqual(new Set(historicalNames).size, historicalNames.length);
assert.strictEqual(manifest.source_head, p.parent);
assert.strictEqual(manifest.source_map_sha256, hash(JSON.stringify(baseline)));
assert.strictEqual(manifest.test_environment_changed, false);
assert.strictEqual(manifest.test_deadlines_changed, false);
assert.deepStrictEqual(manifest.clock.policy, clock.policy);
assert.strictEqual(manifest.clock.contract, clock.contract);
assert.strictEqual(manifest.clock.kind, 'manifest');
const floorRoster = [
  ['r111-source-campaign.json', 'finished_at'], ['r111-auxiliary-campaign.json', 'finished_at'],
  ...p.focused.map(([name]) => ['r111-frozen-' + name + '.json', 'finished_at']),
  ['r111-environment.json', 'recorded_at'], ['r111-exact-clock-contract-tests.json', 'finished_at'],
];
assert.deepStrictEqual(manifest.prerequisite_floors.map(r => [r.name, r.field]), floorRoster);
for (const floor of manifest.prerequisite_floors) {
  assert.strictEqual(hash(fs.readFileSync(file(floor.name))), floor.sha256);
  assert.strictEqual(time(read(floor.name)[floor.field]), floor.utc_ms);
}
clock.verifyGate(manifest.clock.observation, Math.max(...manifest.prerequisite_floors.map(f => f.utc_ms)));
assert.strictEqual(time(manifest.recorded_at), manifest.clock.observation.samples.at(-1).utc_ms);
const mutations = read('r111-exact-mutations.json');
assert.strictEqual(mutations.length, 14);
assert.deepStrictEqual(mutations, p.mutations);
const docPath = 'crates/fe2o3-kfd/src/queue_live.rs';
const oldDocSource = git(['show', p.accepted + ':' + docPath]).split('\n');
const newDocSource = fs.readFileSync(path.join(repo, docPath), 'utf8').split('\n');
const docNames = new Map();
for (const [type, oldLine, newLine] of [
  ['Gfx942ComputeDependencyEventV1', 3827, 3827],
  ['Gfx942ComputeDependencyDispatchV1', 3882, 3882],
  ['ComputeAqlQueueLaneDispatchV1', 4366, 4366],
]) {
  const fence = (lines, line) => {
    assert.strictEqual(lines[line - 1], '/// ```compile_fail');
    const end = lines.indexOf('/// ```', line);
    assert(end > line);
    return lines.slice(line - 1, end + 1);
  };
  assert.deepStrictEqual(fence(newDocSource, newLine), fence(oldDocSource, oldLine));
  const name = line => docPath + ' - queue::live::' + type + ' (line ' + line + ') - compile fail';
  docNames.set(name(oldLine), name(newLine));
}
const plannedNames = mutations.flatMap(m => ['r111-exact-clock-mut-' + m.name + '.json', 'r111-exact-clock-restoration-' + m.name + '.json'])
  .concat(p.focused.map(([name]) => 'r111-exact-clock-restored-' + name + '.json'), ['r111-exact-clock-collector-validation.json']);
assert.deepStrictEqual(Object.keys(manifest.entries), plannedNames);
let expectedPredecessor = {name: 'r111-exact-clock-manifest.json', field: 'recorded_at'};
for (const name of plannedNames) {
  const entry = manifest.entries[name];
  assert.deepStrictEqual(entry.predecessor, expectedPredecessor);
  const restoration = name.startsWith('r111-exact-clock-restoration-');
  assert.strictEqual(entry.kind, restoration ? 'restoration' : 'run');
  expectedPredecessor = {name, field: restoration ? 'verified_at' : 'finished_at'};
}
function checkClock(name, record, restoration = false) {
  const planned = clock.plan(name + '.json');
  assert.strictEqual(record.manifest_sha256, hash(fs.readFileSync(file('r111-exact-clock-manifest.json'))));
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
function checkRun(name, command, returncode = 0, frozen = true) {
  const run = read(name + '.json');
  assert.deepStrictEqual(run.command, command, name);
  assert.strictEqual(run.cwd, repo);
  assert.strictEqual(run.log, file(name + '.log'));
  assert.strictEqual(run.source, file(name + '-source.json'));
  assert.strictEqual(run.source_head, p.parent);
  assert.strictEqual(run.source_unchanged, true);
  assert.strictEqual(run.returncode, returncode);
  assert.strictEqual(run.signal, null);
  assert.strictEqual(run.timed_out, false);
  assert(Number.isFinite(run.elapsed_seconds) && run.elapsed_seconds >= 0);
  assert(time(run.started_at) >= time(environment.recorded_at));
  assert(time(run.finished_at) >= time(run.started_at));
  assert.deepStrictEqual(run.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  if (frozen) assert.deepStrictEqual(read(name + '-source.json'), baseline);
  if (manifest.entries[name + '.json']) {
    assert.strictEqual(run.source_map_sha256, hash(JSON.stringify(read(name + '-source.json'))));
    checkClock(name, run);
  }
  output(run);
  return run;
}
const gates = read('r111-final-source-gate.json');
const auxiliary = read('r111-auxiliary-results.json');
for (const [rows, oldName, count, prefix, logPrefix, oldPrefix] of [
  [gates, 'raw/r110-final-source-gate.json', 17, 'r111-final', 'r111-final', 'r110-final'],
  [auxiliary, 'raw/r110-auxiliary-results.json', 10, 'r111-auxiliary', 'r111', 'r110'],
]) {
  const old = JSON.parse(committed(oldName));
  assert.strictEqual(rows.length, count);
  assert.deepStrictEqual(rows.map(r => r.name), old.map(r => r.name));
  for (const [index, row] of rows.entries()) {
    assert.deepStrictEqual(row.command, old[index].command.map(s => s.replace('r110-production-metadata.log', 'r111-production-metadata.log')), row.name);
    assert.strictEqual(row.cwd, old[index].cwd);
    assert.strictEqual(row.log, file(logPrefix + '-' + row.name + '.log'));
    assert.strictEqual(row.returncode, 0, row.name);
    assert(Number.isFinite(row.elapsed_seconds) && row.elapsed_seconds >= 0);
    const text = output(row);
    if (row.command[0] === 'cargo' && row.command[2] === 'test' && !['gnu-tests', 'musl-tests'].includes(row.name)) {
      const oldText = committed('raw/' + oldPrefix + '-' + row.name + '.log');
      const oldPassing = passing(oldText);
      if (['gnu-docs', 'musl-docs'].includes(row.name)) {
        for (const name of docNames.keys()) assert(oldPassing.includes(name));
        assert.deepStrictEqual(passing(text), oldPassing.map(n => docNames.get(n) || n).sort(), row.name + ' exact relocated doctest roster');
      } else assert.deepStrictEqual(passing(text), oldPassing, row.name + ' exact passing roster');
      assert.deepStrictEqual(totals(text), totals(oldText), row.name + ' exact harness totals');
    }
    if (row.name === 'production-metadata') {
      assert.strictEqual(row.stderr_log, file('r111-production-metadata.stderr.log'));
      assert(fs.lstatSync(row.stderr_log).isFile());
      const metadata = JSON.parse(text);
      assert.strictEqual(metadata.workspace_root, repo);
      assert(metadata.packages.some(item => item.name === 'fe2o3-runtime'));
      assert(Array.isArray(metadata.resolve.nodes) && metadata.resolve.nodes.length);
    } else assert.strictEqual(row.stderr_log, undefined);
  }
  assert.deepStrictEqual(read(prefix + '-source-inputs.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-source-after.json'), baseline);
  assert.deepStrictEqual(read(prefix + '-complete.json'), {completed: true, gates: count, source_identities: paths.length});
}
const full = {};
for (const target of ['gnu', 'musl']) {
  const text = output(gates.find(r => r.name === target + '-tests'));
  const old = committed('raw/r110-final-' + target + '-tests.log');
  assert.deepStrictEqual(passing(text), [...passing(old), ...p.newTests].sort());
  const expected = {...totals(old), passed: totals(old).passed + p.newTests.length};
  assert.deepStrictEqual(totals(text), expected);
  assert.deepStrictEqual(expected, {harnesses: 48, passed: 2657, failed: 0, ignored: 5});
  full[target] = expected;
}
const sourceCampaign = checkRun('r111-source-campaign', ['python3', '-B', file('r111-source-gate.py'), 'r111-final']);
const auxCampaign = checkRun('r111-auxiliary-campaign', ['python3', '-B', file('r111-auxiliary-gates.py')]);
for (const campaign of [sourceCampaign, auxCampaign]) assert.strictEqual(campaign.campaign_deadline_ms, 7200000);
assert(time(auxCampaign.started_at) >= time(sourceCampaign.finished_at));
const clockTests = checkRun('r111-exact-clock-contract-tests', ['node', file('r111-exact-clock-evidence-tests.js')]);
assert.deepStrictEqual(manifest.contract_tests.map(r => r.name),
  ['r111-exact-clock-contract-tests.json', 'r111-exact-clock-contract-tests.log', 'r111-exact-clock-contract-tests-source.json']);
assert(output(clockTests).endsWith('PASS: all 16 evidence-clock contract tests\n'));
assert.strictEqual(output(clockTests).split('\n').filter(line => line.startsWith('PASS: ')).length, 17);
const testedPins = [...output(clockTests).matchAll(/^HELPER_PINS: (.+)$/gm)];
assert.strictEqual(testedPins.length, 1);
assert.deepStrictEqual(JSON.parse(testedPins[0][1]), manifest.helpers);
const oldNames = passing(committed('raw/r110-final-gnu-tests.log'));
const focused = {frozen: [], restored: []};
for (const [kind, filter, count] of p.focused) for (const phase of ['frozen', 'restored']) {
  const names = kind === 'insertion' ? p.newTests : oldNames.filter(n => n.includes(filter)
    && (kind !== 'borrowed' || n.startsWith('queue::live::tests::') || n.startsWith('shared_memory::tests::')));
  assert.strictEqual(names.length, count);
  const run = checkRun((phase === 'restored' ? 'r111-exact-clock-' : 'r111-') + phase + '-' + kind, [...p.tests, filter]);
  assert.deepStrictEqual(passing(output(run)), names);
  assert.deepStrictEqual(totals(output(run)), {harnesses: 1, passed: count, failed: 0, ignored: 0});
  focused[phase].push(run);
}
let lastRestoration = Math.max(time(sourceCampaign.finished_at), time(auxCampaign.finished_at),
  ...focused.frozen.map(r => time(r.finished_at)), time(manifest.recorded_at));
const mutatedHashes = new Set();
for (const mutation of mutations) {
  assert(p.newTests.includes(mutation.test));
  const name = 'r111-exact-clock-mut-' + mutation.name;
  const run = checkRun(name, [...p.tests, mutation.test, '--', '--exact'], 101, false);
  assert(time(run.started_at) >= lastRestoration);
  const inputs = read(name + '-source.json');
  assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
  assert.deepStrictEqual(paths.filter(n => inputs[n] !== baseline[n]), [mutation.path]);
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
  assert(text.includes(mutation.oracle_path + ':' + mutation.oracle_line + ':'), 'exact behavioral assertion location');
  const restored = read('r111-exact-clock-restoration-' + mutation.name + '.json');
  checkClock('r111-exact-clock-restoration-' + mutation.name, restored, true);
  assert.strictEqual(restored.source_head, p.parent);
  assert.strictEqual(restored.mutation, mutation.name);
  assert.strictEqual(restored.source_identities, paths.length);
  assert.strictEqual(restored.source_unchanged, true);
  assert.strictEqual(restored.source_map_sha256, hash(JSON.stringify(baseline)));
  assert(time(restored.verified_at) >= time(run.finished_at));
  lastRestoration = time(restored.verified_at);
}
assert.strictEqual(mutatedHashes.size, mutations.length);
for (const run of focused.restored) assert(time(run.started_at) >= lastRestoration);
const summary = {
  scope: 'N3-L3-C uninitialized-coherent insertion at named CPU/shared-sequencer and two direct-session preflight/missing-engine boundaries; no formal, native or performance qualification',
  collector_sha256: hash(fs.readFileSync(__filename)),
  publication_parent: p.parent, accepted_runtime_checkpoint: p.accepted, source_identities: paths.length,
  source_delta: changed, new_tests: p.newTests, full, source_gates: gates.length, auxiliary_gates: auxiliary.length,
  unchanged_doctest_relocations: [...docNames.entries()],
  focused: Object.fromEntries(p.focused.map(([name, , count]) => [name, count])), compiled_negatives: mutations.length,
  dynamic_new_tests: p.newTests.filter(n => !n.endsWith('production_wiring_never_grants_initialized_authority')),
  reused_substrate_negatives: mutations.filter(m => m.reused_substrate).map(m => m.name),
  campaign_envelope: environment.campaign_envelope,
  preliminary_artifacts: environment.prior_artifacts, preliminary_failures: preliminaryFailures,
  preliminary_exclusions: preliminaryExclusions,
  evidence_clock: {contract: clock.contract, policy: clock.policy, contract_tests: 16,
    historical_artifacts: manifest.historical_artifacts, helpers: manifest.helpers},
  accepted_mutation_cohort: 'r111-exact-clock-mut-',
  limits: ['constructed native leaves are scripted, not Linux GPU execution',
    'primary detached metadata is fixture-local; later ordinal relocates the same actual auxiliary behind a vacancy',
    'two concrete direct-session APIs exercise preflight and missing-engine failure, not native success; no selected-lane API additions',
    'configured accounting only for constructed composition; direct allocation-root controls cover configured/unconfigured modes',
    'dynamic commit-entry custody and source-guarded full commit-before-extraction order are distinct evidence',
    'source guards are not authenticated formal refinement; no new solver, native, performance or aggregate-memory acceptance',
    'uninitialized device insertion N3-L3-D, cleanup custody, generated adoption and the remaining roadmap stay open'],
};
const validationText = JSON.stringify(summary, null, 2) + '\n'
  + 'PASS: full/focused/auxiliary rosters, 14 compiled negatives, exact restoration and chronology\n';
if (process.argv[2] === '--check') {
  process.stdout.write(validationText);
  process.exit(0);
}
assert.strictEqual(process.argv.length, 2);
const validation = checkRun('r111-exact-clock-collector-validation', ['node', file('r111-exact-clock-retain-local.js'), '--check']);
assert(time(validation.started_at) >= Math.max(...focused.restored.map(r => time(r.finished_at))));
assert.strictEqual(output(validation), validationText, 'closed transcript binds exact collector and summary');
const names = fs.readdirSync(root).filter(n => /^r111-.*\.(json|log|py|js)$/.test(n)).sort();
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
