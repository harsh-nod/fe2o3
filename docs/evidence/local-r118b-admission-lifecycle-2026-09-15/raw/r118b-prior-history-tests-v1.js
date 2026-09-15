const fs = require('fs');
const assert = require('assert');
const {test} = require('node:test');
const H = require('./r118b-prior-history-v1.js');
const E = require('./r118-qualification-evidence.js');
const P = require('./r118-qualification-plan.js');
const root = '/home/harsh/.codex-tmp/';
const origin = E.read(H.originName);
const names = fs.readdirSync(root);
const bytes = new Map(H.inputNames(origin).map(name => [name, E.bytes(name)]));
function fixture(changes = new Map(), list = names) {
  const io = {bytes: name => {
    const value = changes.has(name) ? changes.get(name) : bytes.get(name);
    assert(Buffer.isBuffer(value), 'missing fixture input: ' + name);
    return Buffer.from(value);
  }, names: () => [...list], git: E.git};
  io.read = name => JSON.parse(io.bytes(name));
  return io;
}
function changedJson(name, change) {
  const value = JSON.parse(bytes.get(name));
  change(value);
  return fixture(new Map([[name, Buffer.from(JSON.stringify(value, null, 2) + '\n')]]));
}
const oldMap = E.read('r118-frozen-source.json');
const manifest = E.read('r118-qualification-manifest.json');
const failedMutation = P.mutations[72];
const failedName = 'r118-qualified-mut-' + failedMutation.name;
const failedFiles = E.mutationSources(oldMap, manifest.original_sources, failedMutation);
const failedResult = {record: E.read(failedName + '.json'), log: E.bytes(failedName + '.log').toString()};
const diagnostic = E.read('r118-qualification-failure-' + failedMutation.name + '.json');
const supplementName = 'r118-qualification-snapshot-contract-tests-v1';
const supplement = {record: E.read(supplementName + '.json'), log: E.bytes(supplementName + '.log').toString()};
const helpers = P.helpers.map(name => ({name, sha256: E.hash(E.bytes(name))}));
const launcher = {name: 'r118-qualification-launch-v1.js', sha256: E.hash(E.bytes('r118-qualification-launch-v1.js'))};

test('executed helpers match the separately frozen prior-history input manifest', () => {
  const inputs = E.read('r118b-prior-history-inputs-v1.json');
  assert.strictEqual(inputs.parent, P.parent);
  assert.strictEqual(inputs.source_map_sha256, '3aa1ab32e9e6e6ffd0474ec9055d3c29b02bf53e45129406617e50f0b3c15126');
  assert.deepStrictEqual(inputs.helpers.map(item => item.name), [
    'r118b-prior-history-v1.js', 'r118b-prior-history-tests-v1.js', 'r118b-run-v1.js', H.originName,
  ]);
  for (const pin of inputs.helpers) assert.strictEqual(E.hash(E.bytes(pin.name)), pin.sha256);
});
test('real retained campaign validates only as stopped historical evidence', () => {
  const result = H.checkPriorHistory();
  assert.strictEqual(result.accepted, false);
  assert.strictEqual(result.preserved_artifacts, 735);
  assert.strictEqual(result.planned_entries, 163);
  assert.strictEqual(result.qualified_negatives, 72);
  assert.strictEqual(result.rejected_execution, 73);
  assert.deepStrictEqual(result.unexecuted, [74, 75, 76, 77, 78]);
  assert.strictEqual(result.rejected_execution_physical_recovery, true);
  assert.strictEqual(result.rejected_execution_qualified_restoration, false);
});
test('exact membership includes all 83 non-prefixed originals and both Rust snapshots', () => {
  assert.deepStrictEqual(H.expectedNames(manifest), origin.historical_artifacts.map(item => item.name));
  assert.strictEqual(origin.historical_artifacts.filter(item => !item.name.startsWith('r118-')).length, 83);
  for (const name of ['r118b-prior-completion-tests.rs', 'r118b-initial-completion-tests.rs']) assert(H.inputNames(origin).includes(name));
  assert.strictEqual(H.inputNames(origin).length, 746);
});
for (const [label, name] of [
  ['non-prefixed precursor', 'c1-corrected-tests.log'],
  ['qualified mutation', 'r118-qualified-mut-' + P.mutations[0].name + '.log'],
  ['qualified restoration', 'r118-qualified-restoration-' + P.mutations[0].name + '.json'],
  ['rejected mutation', failedName + '.log'],
  ['prior Rust snapshot', 'r118b-prior-completion-tests.rs'],
  ['corrected Rust snapshot', 'r118b-initial-completion-tests.rs'],
]) test('missing preserved input rejects: ' + label, () => {
  assert.throws(() => H.checkPriorHistory(fixture(new Map([[name, null]]))), /missing fixture input/);
});
for (const [label, change] of [
  ['acceptance relabeling', value => {value.prior_campaign.accepted = true;}],
  ['omitted historical artifact', value => {value.historical_artifacts.pop();}],
  ['substituted historical pin', value => {value.historical_artifacts[0].sha256 = '0'.repeat(64);}],
]) test('changed origin rejects: ' + label, () => {
  assert.throws(() => H.checkPriorHistory(changedJson(H.originName, change)), /pinned correction origin/);
});
test('semantically identical rewritten source-map bytes reject the original pin', () => {
  const name = 'r118-frozen-source.json';
  const replacement = Buffer.from(JSON.stringify(JSON.parse(bytes.get(name))));
  assert.throws(() => H.checkPriorHistory(fixture(new Map([[name, replacement]]))), /historical input pin/);
});
for (const name of [
  'r118-qualified-restoration-' + failedMutation.name + '.json',
  'r118-qualified-restoration-' + failedMutation.name + '-source.json',
  'r118-qualified-mut-' + P.mutations[73].name + '.json',
  'r118-qualified-collector-validation.json',
]) test('fabricated completion artifact rejects: ' + name, () => {
  assert.throws(() => H.checkPriorHistory(fixture(new Map(), [...names, name])), /stopped historical membership/);
});
test('new source cohort cannot stand in for the original source map', () => {
  const io = fixture(new Map([['r118-frozen-source.json', bytes.get('r118b-initial-source.json')]]));
  assert.throws(() => H.checkRecords(io, origin), /original source cohort only/);
});
test('first qualified negative must reach its original behavioral oracle', () => {
  const name = 'r118-qualified-mut-' + P.mutations[0].name + '.log';
  const replacement = Buffer.from(bytes.get(name).toString().replace(' panicked at ', ' panicked elsewhere '));
  assert.throws(() => H.checkRecords(fixture(new Map([[name, replacement]])), origin), /exact failing-test panic location/);
});
test('last qualified restoration must retain exact physical recovery flags', () => {
  const name = 'r118-qualified-restoration-' + P.mutations[71].name + '.json';
  const io = changedJson(name, value => {value.restoration.all_mutant_before_restore = false;});
  assert.throws(() => H.checkRecords(io, origin));
});
test('last qualified restoration must restore the original complete source map', () => {
  const name = 'r118-qualified-restoration-' + P.mutations[71].name + '-source.json';
  const io = changedJson(name, value => {value['Cargo.lock'] = '0'.repeat(64);});
  assert.throws(() => H.checkRecords(io, origin));
});
for (const [label, change] of [
  ['wrong source parent', value => {value.source_head = '0'.repeat(40);}],
  ['wrong source map', value => {value.source_map_sha256 = '0'.repeat(64);}],
  ['wrong predecessor', value => {value.clock.predecessor.name = 'r118-qualified-restoration-invented.json';}],
  ['wrong cwd', value => {value.cwd += '-other';}],
  ['wrong namespace', value => {value.clock.contract = 'r118b-raw-utc-boot-monotonic-v1';}],
  ['unclosed child', value => {value.child_closed = false;}],
  ['recorded live process', value => {value.process_group_cleanup.close.live_members = [12345];}],
]) test('rejected execution terminal identity rejects: ' + label, () => {
  const io = changedJson(failedName + '.json', change);
  const entry = manifest.entries[failedName + '.json'];
  assert.throws(() => H.oldRun(io, failedName, entry.command, E.mutationMap(oldMap, failedFiles), entry.predecessor, 101));
});
test('retained abort rejects as a normal named failure but validates as a rejected attempt', () => {
  H.checkRejected(failedResult, failedMutation, failedFiles, diagnostic);
  assert.throws(() => E.checkMutation(failedResult.log, failedMutation), /one named failure/);
});
for (const [label, alter] of [
  ['named failure fabricated', result => {result.log += '\ntest ' + failedMutation.test + ' ... FAILED\n';}],
  ['summary fabricated', result => {result.log += '\ntest result: FAILED. 0 passed; 1 failed; 0 ignored;\n';}],
  ['abort marker removed', result => {result.log = result.log.replaceAll('SIGABRT', 'removed');}],
  ['poison diagnostic removed', result => {result.log = result.log.replaceAll('PoisonError', 'removed');}],
]) test('rejected transcript cannot be relabeled: ' + label, () => {
  const result = structuredClone(failedResult);
  alter(result);
  assert.throws(() => H.checkRejected(result, failedMutation, failedFiles, diagnostic));
});
for (const [label, alter] of [
  ['accepted flag', value => {value.accepted = true;}],
  ['oracle passed flag', value => {value.oracle_passed = true;}],
  ['lost physical recovery', value => {value.restoration.restored = false;}],
  ['different retained source', value => {value.restoration.files[0].after_sha256 = '0'.repeat(64);}],
  ['missing recovery source', value => {value.recovery_record.sources = [];}],
]) test('recovery diagnostic rejects: ' + label, () => {
  const value = structuredClone(diagnostic);
  alter(value);
  assert.throws(() => H.checkRejected(failedResult, failedMutation, failedFiles, value));
});
test('snapshot supplement retains its branch after main166 and before manifest', () => {
  const result = H.oldRun(fixture(), supplementName, ['node', E.file('r118-qualification-snapshot-tests-v1.js')],
    oldMap, 'r118-qualification-contract-tests.json');
  assert.strictEqual(H.checkSupplement(result, helpers, launcher, manifest).length, 20);
  assert.strictEqual(manifest.clock.predecessor.name, 'r118-qualification-contract-tests.json');
});
for (const [label, alter] of [
  ['unqualified extra PASS', value => {value.log += 'PASS invented\n';}],
  ['wrong singular trailer', value => {value.log = value.log.replace('1 snapshot contract test\n', '1 snapshot contract tests\n');}],
  ['observation after manifest', value => {value.record.clock.source_verified.monotonic_ns = (BigInt(manifest.clock.source_verified.monotonic_ns) + 1n).toString();}],
]) test('snapshot supplement rejects: ' + label, () => {
  const value = structuredClone(supplement);
  alter(value);
  assert.throws(() => H.checkSupplement(value, helpers, launcher, manifest));
});
test('snapshot supplement cannot silently change its predecessor to the manifest', () => {
  const io = changedJson(supplementName + '.json', value => {value.clock.predecessor.name = 'r118-qualification-manifest.json';});
  assert.throws(() => H.oldRun(io, supplementName, ['node', E.file('r118-qualification-snapshot-tests-v1.js')],
    oldMap, 'r118-qualification-contract-tests.json'));
});
test('late captured-byte substitution rejects even when JSON meaning is unchanged', () => {
  const io = fixture();
  const originalRead = io.bytes;
  let reads = 0;
  io.bytes = name => {
    const value = originalRead(name);
    if (name === 'r118-frozen-source.json' && ++reads > 1) return Buffer.from(JSON.stringify(JSON.parse(value)));
    return value;
  };
  assert.throws(() => H.checkPriorHistory(io), /unchanged history input/);
});
test('late historical membership change rejects', () => {
  const io = fixture();
  let reads = 0;
  io.names = () => ++reads === 1 ? [...names] : [...names, 'r118-invented-final.json'];
  assert.throws(() => H.checkPriorHistory(io), /stopped historical membership/);
});
