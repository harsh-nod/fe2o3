const fs = require('fs');
const assert = require('assert');
const {test} = require('node:test');
const C = require('./r118b-current-history-v1.js');
const E = require('./r118-qualification-evidence.js');
const P = require('./r118-qualification-plan.js');
const root = '/home/harsh/.codex-tmp/';
const map = E.read('r118b-initial-source.json');
const names = C.inputNames();
const captured = new Map(names.map(name => [name, E.bytes(name)]));
const stopped = JSON.parse(E.bytes('r118b-prior-history-validation-v1.log'));
function fixture(changes = new Map()) {
  const io = {bytes: name => {
    const value = changes.has(name) ? changes.get(name) : captured.get(name);
    assert(Buffer.isBuffer(value), 'missing fixture input: ' + name);
    return Buffer.from(value);
  }, git: E.git, names: () => fs.readdirSync(root)};
  io.read = name => JSON.parse(io.bytes(name));
  return io;
}
function changedJson(name, change) {
  const value = JSON.parse(captured.get(name));
  change(value);
  return fixture(new Map([[name, Buffer.from(JSON.stringify(value, null, 2) + '\n')]]));
}

test('current-history helper and tests match separately frozen input pins', () => {
  const inputs = E.read('r118b-current-history-inputs-v1.json');
  assert.strictEqual(inputs.parent, P.parent);
  assert.strictEqual(inputs.source_map_sha256, E.hash(JSON.stringify(map)));
  assert.deepStrictEqual(inputs.helpers.map(item => item.name), [
    'r118b-current-history-v1.js', 'r118b-current-history-tests-v1.js', 'r118b-run-v1.js',
    'r118b-prior-history-inputs-v1.json', 'r118b-mutation-core-inputs-v1.json',
  ]);
  for (const pin of inputs.helpers) assert.strictEqual(E.hash(E.bytes(pin.name)), pin.sha256);
});
test('real current history requires fresh full runs and retains stopped history separately', () => {
  const result = C.checkCurrentHistory(map);
  for (const value of Object.values(result.full)) assert.deepStrictEqual(value, {harnesses: 48, passed: 2780, failed: 0, ignored: 5});
  assert.strictEqual(result.full_campaign_accepted, false);
  assert.strictEqual(result.stopped_prior.accepted, false);
  assert.strictEqual(result.stopped_prior.qualified_negatives, 72);
  assert.strictEqual(result.preliminary_regression.counts_toward_full_campaign, false);
  assert.strictEqual(result.previous, 'r118b-reviewed-musl-all.json');
});
test('exact input roster preserves old735 and corrected66 including Rust snapshots', () => {
  assert.strictEqual(names.length, 801);
  assert.strictEqual(E.hash(JSON.stringify(names)), 'e093110e680df6b4801a7c19de063933b53fc64b9eb07fb18da122c60f554657');
  for (const name of ['r118b-prior-completion-tests.rs', 'r118b-initial-completion-tests.rs',
    'r118b-reviewed-musl-all.json', 'r118b-reviewed-musl-all-source-after.json']) assert(names.includes(name));
});
test('original source cannot qualify the corrected current-history boundary', () => {
  assert.throws(() => C.checkCurrentRecords(E.read('r118-frozen-source.json'), fixture(), stopped), /corrected source cohort only/);
});
test('stopped history cannot be relabeled accepted', () => {
  assert.throws(() => C.checkCurrentRecords(map, fixture(), {...stopped, accepted: true}));
});
for (const [label, change] of [
  ['old namespace', value => {value.clock.contract = 'r118-raw-utc-boot-monotonic-v1';}],
  ['wrong parent', value => {value.source_head = '0'.repeat(40);}],
  ['old source map', value => {value.source_map_sha256 = 'df2e27085ee920fdd07d2a6955503b1ecdcadc2d7c10c6fd4cec6768cecdadbb';}],
  ['wrong predecessor', value => {value.clock.predecessor.name = 'r118b-reviewed-gnu-all.json';}],
  ['unclosed child', value => {value.child_closed = false;}],
  ['live process group', value => {value.process_group_cleanup.close.live_members = [12345];}],
]) test('fresh musl record rejects: ' + label, () => {
  const io = changedJson('r118b-reviewed-musl-all.json', change);
  assert.throws(() => C.currentRun(io, 'r118b-reviewed-musl-all', [...P.fullTest, '--target', 'x86_64-unknown-linux-musl'],
    map, 'r118b-prior-history-validation-v1.json'));
});
test('missing fresh musl output rejects instead of falling back to old full results', () => {
  assert.throws(() => C.checkCurrentHistory(map, fixture(new Map([['r118b-reviewed-musl-all.log', null]]))), /missing fixture input/);
});
test('fresh GNU endpoint drift rejects even when the run result is zero', () => {
  const io = changedJson('r118b-reviewed-gnu-all-source-after.json', value => {value['Cargo.lock'] = '0'.repeat(64);});
  assert.throws(() => C.checkCurrentRecords(map, io, stopped));
});
test('equal aggregate counts cannot hide tests swapped between executable rosters', () => {
  const log = captured.get('r118b-reviewed-gnu-all.log').toString();
  const markers = [...log.matchAll(/^     Running (.+)$/gm)];
  const parts = markers.map((marker, index) => ({name: marker[1], text: log.slice(marker.index, markers[index + 1]?.index ?? log.length)}));
  const runtime = parts.find(part => /fe2o3_runtime-/.test(part.name));
  const kfd = parts.find(part => /fe2o3_kfd-/.test(part.name));
  assert(runtime && kfd);
  const a = runtime.text.match(/^test .+ \.\.\. ok$/m)[0];
  const b = kfd.text.match(/^test .+ \.\.\. ok$/m)[0];
  runtime.text = runtime.text.replace(a, b);
  kfd.text = kfd.text.replace(b, a);
  const changed = log.slice(0, markers[0].index) + parts.map(part => part.text).join('');
  assert.deepStrictEqual(E.passing(changed), E.passing(log));
  assert.deepStrictEqual(E.totals(changed), E.totals(log));
  const io = fixture(new Map([['r118b-reviewed-gnu-all.log', Buffer.from(changed)]]));
  assert.throws(() => C.checkCurrentRecords(map, io, stopped), /exact executable roster/);
});
test('old abort with remapped assertion line is not a corrected normal failure', () => {
  const old = E.bytes('r118-qualified-mut-c3-continue-retirement-after-failure.log').toString().replace(':317:17:', ':338:17:');
  assert.throws(() => C.checkRegression(fixture(new Map([['r118b-retirement-regression.log', Buffer.from(old)]])), map), /one named failure/);
});
test('otherwise valid corrected failure cannot include an abort diagnostic', () => {
  const log = captured.get('r118b-retirement-regression.log').toString() + '\nSIGABRT\n';
  assert.throws(() => C.checkRegression(fixture(new Map([['r118b-retirement-regression.log', Buffer.from(log)]])), map));
});
for (const [label, change] of [
  ['missing preflight custody', value => {value.restoration.all_mutant_before_restore = false;}],
  ['external edits', value => {value.restoration.external_edit = true;}],
  ['wrong terminal digest', value => {value.terminal_identity.sha256 = '0'.repeat(64);}],
  ['wrong runner pin', value => {value.runner_pin.sha256 = '0'.repeat(64);}],
  ['lost authentication', value => {value.terminal_authenticated = false;}],
  ['non-preliminary claim', value => {value.preliminary = false;}],
]) test('corrected regression restoration rejects: ' + label, () => {
  const io = changedJson('r118b-retirement-regression-restoration.json', change);
  assert.throws(() => C.checkRegression(io, map));
});
test('corrected regression must restore every source identity', () => {
  const io = changedJson('r118b-retirement-regression-restoration-source.json', value => {value['Cargo.lock'] = '0'.repeat(64);});
  assert.throws(() => C.checkRegression(io, map));
});
test('replaced TAP case cannot hide behind an unchanged passing total', () => {
  const log = captured.get('r118b-prior-history-contracts-v1.log').toString().replace(/^ok 1 - .+$/m, 'ok 1 - unrelated replacement');
  assert.throws(() => C.checkTap({log}, 45, 'fbd82a39cc50cfc4c90542ce351e63b6c51dbde95136eba16e3136392d4d513b'), /exact ordered contract roster/);
});
test('closed stopped-history transcript cannot relabel its rejected execution', () => {
  const value = {...stopped, rejected_execution: null};
  const io = fixture(new Map([['r118b-prior-history-validation-v1.log', Buffer.from(JSON.stringify(value))]]));
  assert.throws(() => C.checkCurrentRecords(map, io, stopped), /closed stopped-history transcript/);
});
test('late whitespace-only source-after substitution rejects captured-byte equality', () => {
  const io = fixture();
  const read = io.bytes;
  let observations = 0;
  io.bytes = name => {
    const value = read(name);
    if (name === 'r118b-reviewed-gnu-all-source-after.json' && ++observations > 1) return Buffer.from(JSON.stringify(JSON.parse(value)));
    return value;
  };
  assert.throws(() => C.checkCurrentHistory(map, io), /unchanged current-history input/);
});
test('late manifest-derived input membership substitution rejects', () => {
  const io = fixture();
  const read = io.read;
  let observations = 0;
  io.read = name => {
    const value = read(name);
    if (name === 'r118b-mutation-core-inputs-v1.json' && ++observations > 1) value.helpers.push({name: 'r118b-fabricated-extra-helper.js', sha256: '0'.repeat(64)});
    return value;
  };
  assert.throws(() => C.checkCurrentHistory(map, io), /current-history input membership/);
});
