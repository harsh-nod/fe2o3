// Validate corrected-source runs separately from the stopped original campaign.
const fs = require('fs');
const assert = require('assert');
const E = require('./r118-qualification-evidence.js');
const P = require('./r118-qualification-plan.js');
const H = require('./r118-history-v3.js');
const prior = require('./r118b-prior-history-v1.js');
const mutations = require('./r118b-mutation-plan-v1.js');
const contract = 'r118b-raw-utc-boot-monotonic-v1';
const defaults = {bytes: E.bytes, read: E.read, git: E.git, names: () => fs.readdirSync(E.root)};
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const regressionInputs = 'r118b-retirement-regression-inputs-v1.json';
const regressionHash = '73c52f11f7a024592f48e8ebe72b6ea6e0c310a0c4da4b3e807a19fe83432892';
const runs = [
  'r118b-format', 'r118b-focused', 'r118b-runtime', 'r118b-clippy',
  'r118b-retirement-regression', 'r118b-restored-completion',
  'r118b-mutation-core-contracts-v1', 'r118b-reviewed-gnu-all',
  'r118b-prior-history-contracts-v1', 'r118b-prior-history-validation-v1', 'r118b-reviewed-musl-all',
];
const regressionNewInputs = [
  'r118b-c3-oracles-v1.js', 'r118b-retirement-regression-v1.js',
  'r118b-origin-v2.json', 'r118b-origin-v1.js', 'r118b-origin-v2.js',
  'r118b-prior-completion-tests.rs', 'r118b-initial-completion-tests.rs',
  'r118b-initial-source.json', 'r118b-run-v1.js',
  ...['format', 'focused', 'runtime', 'clippy'].flatMap(name => suffixes.map(suffix => 'r118b-' + name + suffix)),
].sort();
const helperManifests = [
  ['r118b-mutation-core-inputs-v1.json', '2fc0d01c32e231fab65adb9aec7fc25d6638a99359c7f02081e4c786b064ceaf'],
  ['r118b-prior-history-inputs-v1.json', '88a96285d7b471e3be7b5ccacdb6b122429ca471b28ca294c8a691e4ba5aaae1'],
];
function inputNames(io = defaults) {
  const origin = io.read(prior.originName);
  const regression = io.read(regressionInputs);
  const helpers = helperManifests.flatMap(([name]) => [name, ...io.read(name).helpers.map(item => item.name)]);
  return [...new Set([...prior.inputNames(origin), regressionInputs, ...regression.inputs.map(item => item.name),
    ...helpers, ...runs.flatMap(name => suffixes.map(suffix => name + suffix)),
    'r118b-retirement-regression-restoration.json', 'r118b-retirement-regression-restoration-source.json'])].sort();
}
function currentRun(io, name, command, map, previous, code = 0) {
  const record = io.read(name + '.json');
  E.checkRecord(record, name, command, map, previous, code, io,
    {head: P.parent, cwd: E.repo, contract, runner: 'r118b-run-v1.js', after: map});
  return {record, log: io.bytes(name + '.log').toString()};
}
function assertPositive(result, names) {
  E.assertPassing(result.log);
  assert.deepStrictEqual(E.passing(result.log), [...names].sort());
  assert.deepStrictEqual(E.ignored(result.log), []);
  assert.deepStrictEqual(E.totals(result.log), {harnesses: 1, passed: names.length, failed: 0, ignored: 0});
}
function checkTap(result, count, rosterHash) {
  H.tapNames(result.log, count);
  const names = [...result.log.matchAll(/^ok \d+ - (.+)$/gm)].map(match => match[1]);
  assert.strictEqual(E.hash(JSON.stringify(names)), rosterHash, 'exact ordered contract roster');
}
function checkRegression(io, map) {
  assert.strictEqual(E.hash(io.bytes(regressionInputs)), regressionHash);
  const inputs = io.read(regressionInputs);
  assert.strictEqual(inputs.source_head, P.parent);
  assert.strictEqual(inputs.source_map_sha256, mutations.sourceMapSha256);
  assert.strictEqual(inputs.preliminary, true);
  assert.deepStrictEqual(inputs.mutation, mutations.mutations[72]);
  const origin = io.read(prior.originName);
  assert.strictEqual(inputs.inputs.length, 760);
  assert.deepStrictEqual(inputs.inputs.map(item => item.name),
    [...origin.historical_artifacts.map(item => item.name), ...regressionNewInputs].sort());
  for (const pin of inputs.inputs) assert.strictEqual(E.hash(io.bytes(pin.name)), pin.sha256, pin.name);
  const previous = 'r118b-clippy.json';
  const previousRecord = io.read(previous);
  assert.strictEqual(inputs.clock.contract, contract);
  assert.strictEqual(inputs.clock.kind, 'regression-inputs');
  assert.strictEqual(inputs.clock.error, null);
  assert.deepStrictEqual(inputs.clock.predecessor,
    {name: previous, sha256: E.hash(io.bytes(previous)), observation: previousRecord.clock.source_verified});
  E.ordered(previousRecord.clock.source_verified, inputs.clock.source_verified);
  const files = E.mutationSources(map, inputs.original_sources, inputs.mutation);
  const result = currentRun(io, 'r118b-retirement-regression',
    [...P.test, inputs.mutation.test, '--', '--exact', '--test-threads=1'],
    E.mutationMap(map, files), regressionInputs, 101);
  E.checkMutation(result.log, inputs.mutation);
  for (const marker of ['SIGABRT', 'PoisonError', 'panic in a destructor']) assert(!result.log.includes(marker));
  const restoration = io.read('r118b-retirement-regression-restoration.json');
  const terminal = {name: 'r118b-retirement-regression.json', sha256: E.hash(io.bytes('r118b-retirement-regression.json'))};
  assert.strictEqual(restoration.source_head, P.parent);
  assert.strictEqual(restoration.source_map_sha256, mutations.sourceMapSha256);
  assert.strictEqual(restoration.manifest_sha256, regressionHash);
  assert.strictEqual(restoration.preliminary, true);
  assert.strictEqual(restoration.oracle_passed, true);
  assert.strictEqual(restoration.terminal_authenticated, true);
  assert.deepStrictEqual(restoration.terminal_identity, terminal);
  assert.deepStrictEqual(restoration.runner_pin, {name: 'r118b-run-v1.js', sha256: E.hash(io.bytes('r118b-run-v1.js'))});
  assert.deepStrictEqual(restoration.application_attempts, files.map(file => ({path: file.path, verified: true})));
  assert.strictEqual(restoration.source_unchanged, true);
  assert.strictEqual(restoration.source_identities, 5692);
  assert.strictEqual(restoration.failure, null);
  assert.deepStrictEqual(restoration.restoration, {restored: true, quiescent: true,
    all_mutant_before_restore: true, live_members: [], external_edit: false, error: null,
    files: files.map(file => ({path: file.path, before_sha256: file.changed_sha256,
      after_sha256: file.original_sha256, write_attempted: true}))});
  assert.deepStrictEqual(io.read('r118b-retirement-regression-restoration-source.json'), map);
  assert.strictEqual(restoration.clock.contract, contract);
  assert.strictEqual(restoration.clock.kind, 'regression-restoration');
  assert.strictEqual(restoration.clock.error, null);
  assert.deepStrictEqual(restoration.clock.predecessor, {...terminal, observation: result.record.clock.source_verified});
  E.ordered(result.record.clock.source_verified, restoration.clock.start);
  E.ordered(restoration.clock.start, restoration.clock.source_verified);
  return {preliminary: true, normal_named_failure: true, exact_restoration: true,
    mutation: inputs.mutation.name, counts_toward_full_campaign: false};
}
function checkCurrentRecords(map, io, stopped) {
  assert.strictEqual(E.hash(JSON.stringify(map)), mutations.sourceMapSha256, 'corrected source cohort only');
  assert.strictEqual(Object.keys(map).length, 5692);
  assert.deepStrictEqual(map, io.read('r118b-initial-source.json'));
  assert.strictEqual(stopped.accepted, false);
  for (const [name, digest] of helperManifests) {
    assert.strictEqual(E.hash(io.bytes(name)), digest, 'frozen helper input manifest');
    const inputs = io.read(name);
    assert.strictEqual(inputs.parent, P.parent);
    assert.strictEqual(inputs.source_map_sha256, mutations.sourceMapSha256);
    for (const pin of inputs.helpers) assert.strictEqual(E.hash(io.bytes(pin.name)), pin.sha256, pin.name);
  }
  const gnu = currentRun(io, 'r118b-reviewed-gnu-all', P.fullTest, map, 'r118b-mutation-core-contracts-v1.json');
  const musl = currentRun(io, 'r118b-reviewed-musl-all', [...P.fullTest, '--target', 'x86_64-unknown-linux-musl'],
    map, 'r118b-prior-history-validation-v1.json');
  const full = {};
  for (const [kind, result] of [['gnu', gnu], ['musl', musl]]) {
    const retained = io.git(['show', P.parent + ':' + P.previous + 'raw/r117-reviewed-' + kind + '-all.log']);
    full[kind] = E.checkFull(result.log, retained, mutations.newTests);
  }
  const runtime = E.executables(gnu.log).filter(target => target.kind === 'libtest' && /fe2o3_runtime(?:-|\))/.test(target.name));
  assert.strictEqual(runtime.length, 1);
  assert.strictEqual(runtime[0].passing.length, 733);
  const c3 = mutations.newTests.filter(name => name.startsWith('async_engine::tests::owned_tests::preparation_tests::completion_tests::'));
  assert.strictEqual(c3.length, 5);
  const focused = [...P.test, P.focused[2][1], '--', '--test-threads=1'];
  const format = currentRun(io, 'r118b-format', ['cargo', '+nightly-2026-04-03', 'fmt', '--all'], map, null);
  assert.strictEqual(format.log, '');
  assertPositive(currentRun(io, 'r118b-focused', focused, map, 'r118b-format.json'), c3);
  assertPositive(currentRun(io, 'r118b-runtime', P.test, map, 'r118b-focused.json'), runtime[0].passing);
  const clippy = currentRun(io, 'r118b-clippy', ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline',
    '-p', 'fe2o3-runtime', '--all-features', '--all-targets', '--', '-D', 'warnings'], map, 'r118b-runtime.json');
  assert(clippy.log.includes('Finished `dev` profile'));
  const regression = checkRegression(io, map);
  assertPositive(currentRun(io, 'r118b-restored-completion', focused, map, 'r118b-retirement-regression-restoration.json'), c3);
  checkTap(currentRun(io, 'r118b-mutation-core-contracts-v1', ['node', '--test', E.file('r118b-mutation-core-tests-v1.js')],
    map, 'r118b-restored-completion.json'), 48, '53c1f4174edd5efbb773ce35508791eabf83174de30e713a0a576399fba30ce5');
  checkTap(currentRun(io, 'r118b-prior-history-contracts-v1', ['node', '--test', E.file('r118b-prior-history-tests-v1.js')],
    map, 'r118b-reviewed-gnu-all.json'), 45, 'fbd82a39cc50cfc4c90542ce351e63b6c51dbde95136eba16e3136392d4d513b');
  const validation = currentRun(io, 'r118b-prior-history-validation-v1', ['node', E.file('r118b-prior-history-v1.js')],
    map, 'r118b-prior-history-contracts-v1.json');
  assert.deepStrictEqual(JSON.parse(validation.log), stopped, 'closed stopped-history transcript');
  return {source_map_sha256: mutations.sourceMapSha256, source_identities: 5692,
    full, stopped_prior: stopped, corrected_runtime_tests: 733, focused_completion_tests: 5,
    core_contracts: 48, prior_history_contracts: 45, preliminary_regression: regression,
    runs: [...runs], previous: 'r118b-reviewed-musl-all.json', full_campaign_accepted: false};
}
function checkCurrentHistory(map, io = defaults) {
  const names = inputNames(io);
  const captured = new Map(names.map(name => [name, Buffer.from(io.bytes(name))]));
  const snapshot = {bytes: name => {assert(captured.has(name), 'captured current-history input: ' + name); return Buffer.from(captured.get(name));},
    git: io.git, names: io.names};
  snapshot.read = name => JSON.parse(snapshot.bytes(name));
  const stopped = prior.checkPriorHistory(snapshot);
  const result = checkCurrentRecords(map, snapshot, stopped);
  assert.deepStrictEqual(inputNames(io), names, 'current-history input membership');
  for (const [name, bytes] of captured) assert.strictEqual(E.hash(io.bytes(name)), E.hash(bytes), 'unchanged current-history input: ' + name);
  return result;
}
if (require.main === module) console.log(JSON.stringify(checkCurrentHistory(E.read('r118b-initial-source.json')), null, 2));
module.exports = {contract, runs, inputNames, currentRun, assertPositive, checkTap,
  checkRegression, checkCurrentRecords, checkCurrentHistory};
