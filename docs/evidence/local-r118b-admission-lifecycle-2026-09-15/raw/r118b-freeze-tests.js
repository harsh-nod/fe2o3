const assert = require('assert');
const fs = require('fs');
const path = require('path');
const p = require('./r118b-qualification-plan.js');
const e = require('./r118b-qualification-evidence.js');
const f = require('./r118b-freeze.js');
const history = require('./r118b-current-history-v1.js');
const support = require('./r118b-history-support-v1.js');
const cases = [];
const tests = [];
const check = (name, run) => {cases.push(name); tests.push(run);};
let captured;
function inputs() {
  if (captured) return captured;
  const listing = fs.readdirSync(e.root);
  const prior = e.historicalNames();
  const names = [...new Set([...history.inputNames(), ...support.inputNames(), ...p.freezeHelpers, ...prior])].sort();
  const map = e.identities();
  captured = {listing, prior, values: new Map(names.map(name => [name, e.bytes(name)])), map,
    sources: new Map(Object.keys(map).map(name => [name, fs.readFileSync(path.join(e.repo, name))])),
    history: JSON.parse(e.bytes('r118b-current-history-validation-v1.log')),
    latest: e.read('r118b-runner-contracts-v1.json').clock.source_verified};
  return captured;
}
function fixture(options = {}) {
  const initial = inputs();
  const state = {sources: new Map(initial.sources), values: new Map(initial.values),
    listed: new Set(Object.keys(initial.map)), listing: [...initial.listing], prior: [...initial.prior],
    head: p.parent, occupied: new Set(), writes: [], validated: 0, observations: 0, toolCalls: 0};
  state.changeJson = (name, change) => {
    const value = JSON.parse(state.values.get(name)); change(value);
    state.values.set(name, Buffer.from(JSON.stringify(value)));
  };
  const binaries = options.real ? JSON.parse(e.committed('raw/r117-environment.json')).binaries
    : ['cargo', 'rustc'].map(name => ({name, path: '/fixture/bin/' + name, sha256: e.hash(name)}));
  const io = {...f.defaults,
    head: () => state.head,
    inventory: () => [...state.listed],
    source: name => {assert(state.sources.has(name), 'missing source: ' + name); return state.sources.get(name);},
    bytes: name => {
      assert(state.values.has(name), 'missing artifact: ' + name);
      assert.notStrictEqual(name, options.nonregular, 'regular artifact');
      return Buffer.from(state.values.get(name));
    },
    names: () => [...state.listing], historicalNames: () => [...state.prior],
    env: options.stack ? {RUST_MIN_STACK: options.stack} : {},
    git: args => args[1] === p.parent + ':' + p.previous + 'raw/r117-environment.json'
      ? JSON.stringify({binaries}) : e.git(args),
    execute: command => {
      state.toolCalls++;
      if (command[0] === 'rustup') return binaries.find(binary => binary.name === command.at(-1)).path;
      if (command[0] === 'gh') {
        state.observations++; options.late?.(state);
        return JSON.stringify({number: 182, state: 'OPEN'});
      }
      return 'fixture tool observation';
    },
    binary: file => options.real ? fs.readFileSync(file) : Buffer.from(options.wrongBinary ? 'changed' : path.basename(file)),
    sample: () => ({...initial.latest, utc_ms: initial.latest.utc_ms + 100,
      monotonic_ns: (BigInt(initial.latest.monotonic_ns) + 1000000n).toString(), ...options.clock}),
    vacant: name => assert(!state.occupied.has(name), 'freeze output already exists: ' + name),
    write: (name, value) => {assert(!state.occupied.has(name)); state.occupied.add(name); state.writes.push([name, value]);},
    checkHistory: (map, snapshot) => {
      state.validated++; options.validate?.(state, snapshot);
      return options.real ? history.checkCurrentHistory(map, snapshot) : structuredClone(initial.history);
    },
    checkSupport: (map, current, snapshot) => {
      if (options.rejectSupport) throw new Error('support validator rejected');
      return support.check(map, current, snapshot);
    },
  };
  options.initial?.(state);
  let result;
  let error;
  try {result = f.freeze(io);} catch (caught) {error = caught;}
  if (options.rejects) {
    assert(error, 'freeze unexpectedly accepted');
    assert.deepStrictEqual(state.writes, [], 'rejected freeze wrote output');
    if (options.oracle) assert.match(error.message, options.oracle);
  } else {
    assert.ifError(error);
    assert.strictEqual(result.source_identities, 5692);
    assert.strictEqual(result.history.stopped_prior.accepted, false);
    assert.strictEqual(result.history.full_campaign_accepted, false);
    assert.strictEqual(result.support_history.current_history_contracts, 27);
    assert.strictEqual(result.support_history.runner_contracts, 9);
    assert.deepStrictEqual(state.writes.map(([name]) => name), f.outputs);
    assert.deepStrictEqual(state.writes[0][1], initial.map);
    const environment = state.writes[1][1];
    assert.deepStrictEqual(environment.source_delta, p.sourceDelta);
    for (const pin of environment.prerequisite_validation_inputs) assert.strictEqual(pin.sha256, e.hash(state.values.get(pin.name)));
  }
  return state;
}
const rejected = (name, options) => check(name, () => fixture({...options, rejects: true}));
const late = (name, change, oracle) => rejected(name, {late: change, oracle});
check('real-current-history-and-support-with-captured-writes', () => fixture({real: true}));
check('unchanged-boundary', () => fixture());
rejected('wrong-opening-head', {initial: state => {state.head = 'f'.repeat(40);}, oracle: /opening HEAD/});
rejected('wrong-opening-source-map', {initial: state => state.sources.set('Cargo.lock', Buffer.from('changed')), oracle: /opening source map/});
rejected('wrong-opening-inventory', {initial: state => state.listed.delete(p.addedSource[0])});
late('added-source', state => {state.listed.add('scripts/new.rs'); state.sources.set('scripts/new.rs', Buffer.from('new'));}, /complete source inventory/);
late('deleted-untracked-source', state => {state.listed.delete(p.addedSource[0]); state.sources.delete(p.addedSource[0]);}, /complete source inventory/);
late('deleted-tracked-source', state => state.sources.delete('Cargo.lock'), /missing source/);
late('modified-lockfile', state => state.sources.set('Cargo.lock', Buffer.from('changed')), /closing source map/);
late('same-count-replacement', state => {
  state.listed.delete(p.addedSource[0]); state.sources.delete(p.addedSource[0]);
  state.listed.add('scripts/replacement.rs'); state.sources.set('scripts/replacement.rs', Buffer.from('new'));
}, /complete source inventory/);
late('closing-head-change', state => {state.head = 'e'.repeat(40);}, /closing HEAD/);
check('docs-only-change', () => fixture({late: state => {
  state.listed.add('docs/not-source.md'); state.sources.set('docs/not-source.md', Buffer.from('new'));
}}));
rejected('missing-current-artifact', {initial: state => state.values.delete('r118b-reviewed-musl-all.log'), oracle: /missing artifact/});
rejected('missing-old-artifact', {initial: state => state.values.delete('c1-initial-tests.log'), oracle: /missing artifact/});
rejected('nonregular-artifact', {nonregular: 'r118b-reviewed-musl-all.log', oracle: /regular artifact/});
check('snapshot-buffers-and-listing-are-isolated', () => fixture({validate: (_state, snapshot) => {
  const name = 'r118b-reviewed-musl-all.log';
  const before = snapshot.bytes(name); snapshot.bytes(name).fill(0);
  assert.deepStrictEqual(snapshot.bytes(name), before);
  const names = snapshot.names(); snapshot.names().push('r118-unexpected.json');
  assert.deepStrictEqual(snapshot.names(), names);
}}));
rejected('inputs-captured-before-validation', {validate: (state, snapshot) => {
  const name = 'r118b-reviewed-musl-all.log'; const before = snapshot.bytes(name);
  state.values.set(name, Buffer.from('changed after capture'));
  assert.deepStrictEqual(snapshot.bytes(name), before);
}, oracle: /validated input unchanged/});
late('late-input-substitution', state => state.values.set('r118b-reviewed-gnu-all.log', Buffer.from('changed')), /validated input unchanged/);
late('late-whitespace-only-json-substitution', state => {
  const name = 'r118b-reviewed-musl-all-source-after.json';
  state.values.set(name, Buffer.concat([state.values.get(name), Buffer.from('\n')]));
}, /validated input unchanged/);
for (const name of ['r118-unexpected.json', 'r118b-unexpected.rs', 'r118-unexpected.md']) {
  late('late-artifact-membership-' + name, state => state.listing.push(name), /artifact namespace membership/);
}
late('late-artifact-deletion', state => {state.listing = state.listing.filter(name => name !== 'r118b-reviewed-musl-all.log');}, /artifact namespace membership/);
late('late-nonprefix-historical-membership', state => state.prior.push('c1-unexpected.log'), /historical membership/);
rejected('history-validator-rejection', {validate: () => {throw new Error('history validator rejected');}, oracle: /history validator rejected/});
rejected('support-validator-rejection', {rejectSupport: true, oracle: /support validator rejected/});
rejected('future-prerequisite-observation', {clock: {monotonic_ns: '0'}, oracle: /monotonic order/});
check('latest-support-tail-must-precede-freeze', () => {
  const latest = inputs().latest;
  const before = BigInt(latest.monotonic_ns) - 1n;
  for (const [name] of p.prerequisites) assert(BigInt(e.read(name + '.json').clock.source_verified.monotonic_ns) < before);
  fixture({rejects: true, clock: {monotonic_ns: before.toString()}, oracle: /monotonic order/});
});
rejected('different-freeze-boot', {clock: {boot_id: '00000000-0000-0000-0000-000000000001'}, oracle: /same boot/});
check('raw-utc-regression-does-not-change-monotonic-order', () => fixture({clock: {utc_ms: 1}}));
check('stopped-history-remains-independent-and-unaccepted', () => {
  const state = fixture();
  assert.strictEqual(state.writes[1][1].history.stopped_prior.rejected_execution_qualified_restoration, false);
});
rejected('binary-pin-mismatch', {wrongBinary: true, oracle: /accepted toolchain binaries/});
rejected('forbidden-stack-override', {stack: '16777216'});
for (const name of f.outputs) rejected('occupied-output-' + name, {
  initial: state => state.occupied.add(name), oracle: /freeze output already exists/,
});
late('late-occupied-output', state => state.occupied.add(f.outputs[1]), /freeze output already exists/);
for (const [name, change, oracle] of [
  ['unclosed', record => {record.child_closed = false;}, /observed child closure/],
  ['wrong-predecessor', record => {record.clock.predecessor.sha256 = '0'.repeat(64);}],
  ['wrong-source', record => {record.source_map_sha256 = '0'.repeat(64);}],
  ['wrong-command', record => {record.command = ['true'];}, /exact command/],
]) rejected('support-record-' + name, {initial: state => state.changeJson('r118b-runner-contracts-v1.json', change), oracle});
rejected('support-input-pin-change', {initial: state => state.changeJson('r118b-runner-inputs-v1.json', value => {value.scope = 'changed';}), oracle: /support input manifest/});
rejected('support-wrong-contract-roster', {initial: state => {
  const name = 'r118b-current-history-contracts-v1.log';
  state.values.set(name, Buffer.from(state.values.get(name).toString().replace(/^ok 1 - .+$/m, 'ok 1 - unrelated')));
}, oracle: /exact ordered contract roster/});
rejected('support-current-transcript-relabel', {initial: state => state.changeJson('r118b-current-history-validation-v1.log', value => {value.full_campaign_accepted = true;}), oracle: /closed current-history transcript/});
rejected('support-runner-transcript-relabel', {initial: state => {
  const name = 'r118b-runner-contracts-v1.log'; state.values.set(name, Buffer.from('PASS: 9 runner contract tests\n'));
}, oracle: /exact runner contract transcript/});
rejected('support-fixture-source-drift', {initial: state => state.changeJson('r118b-runner-v1-fixture-clean-source-after.json', value => {value['Cargo.lock'] = '0'.repeat(64);})});
for (const [name, kind, change, oracle] of [
  ['cleanup-error', 'success', record => {record.process_group_cleanup.close.status = 'error';}, /successful group cleanup/],
  ['live-group', 'success', record => {record.process_group_cleanup.close.live_members = [123];}, /no live owned-group members/],
  ['wrong-exit', 'failure', record => {record.returncode = record.child_returncode = 0;}],
  ['wrong-command', 'clean', record => {record.command = ['true'];}, /exact command/],
  ['timed-out', 'clean', record => {record.timed_out = true;}],
  ['environment', 'clean', record => {record.environment.RUST_TEST_THREADS = '1';}],
  ['spawn-child-code', 'spawn', record => {record.child_returncode = 0;}, /raw ENOENT result/],
  ['spawn-invented-group', 'spawn', record => {record.process_group_cleanup.close.pgid = 123;}],
  ['spawn-wrong-result', 'spawn', record => {record.returncode = 0;}, /exact fixture result/],
  ['spawn-path', 'spawn', record => {record.log += '.other';}],
  ['spawn-duration', 'spawn', record => {record.elapsed_seconds++;}],
]) rejected('support-fixture-' + name, {initial: state => state.changeJson('r118b-runner-v1-fixture-' + kind + '.json', change), oracle});
function main() {
  const frozen = e.read('r118b-freeze-inputs-v1.json');
  assert.strictEqual(frozen.parent, p.parent);
  assert.strictEqual(frozen.source_map_sha256, p.sourceMapSha256);
  assert.deepStrictEqual(frozen.helpers.map(pin => pin.name), p.freezeHelpers);
  for (const pin of frozen.helpers) assert.strictEqual(e.hash(e.bytes(pin.name)), pin.sha256, pin.name);
  assert.strictEqual(cases.length, p.freezeContractCount);
  for (const [index, run] of tests.entries()) {run(); console.log('PASS ' + cases[index]);}
  console.log('PASS: ' + cases.length + ' freeze contract tests');
}
if (require.main === module) main();
module.exports = {cases, fixture, main};
