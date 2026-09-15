const assert = require('assert');
const {test} = require('node:test');
require('./r119-integrated-validation-v1.js').guard();
const e = require('./r119-integrated-qualification-evidence-v1.js');
const p = require('./r119-integrated-qualification-plan-v1.js');
const f = require('./r119-integrated-freeze-v1.js');
const v = require('./r119-integrated-validation-v1.js');
const source = e.read('r119-integrated-gnu-all-source.json');
// Exercise freeze custody boundaries; history semantics have their own suite.
function fixture() {
  const initial = e.sample();
  const data = new Map(f.helpers.map(name => [name, Buffer.from(name)]));
  data.set('tail.json', Buffer.from(JSON.stringify({clock: {source_verified: initial}})));
  const writes = [];
  const binaries = ['cargo', 'rustc'].map(name => ({name, path: '/toolchain/' + name, sha256: e.hash(Buffer.from('binary ' + name))}));
  const io = {
    head: () => p.parent,
    identities: () => structuredClone(source), names: () => [...data.keys()].sort(),
    bytes: name => {assert(data.has(name)); return data.get(name);},
    git: () => JSON.stringify({binaries}), sample: e.sample, env: {},
    execute: command => command[0] === 'rustup' ? '/toolchain/' + command.at(-1) + '\n' : 'fixture\n',
    binary: name => Buffer.from('binary ' + name.split('/').at(-1)),
    checkHistory: (_map, snapshot) => {
      assert.strictEqual(snapshot.bytes(f.helpers[0]).toString(), f.helpers[0]);
      return {accepted: false, previous: 'tail.json'};
    },
    checkSupport: () => ({previous: 'tail.json'}),
    vacant: () => {}, write: (name, value) => writes.push([name, structuredClone(value)]),
  };
  return {io, data, writes};
}
test('captures inputs before validation and writes only after closing checks', () => {
  const state = fixture();
  const result = f.freeze(state.io);
  assert.strictEqual(result.accepted, false);
  assert.strictEqual(result.source_identities, 5693);
  assert.deepStrictEqual(state.writes.map(([name]) => name), f.outputs);
  assert.deepStrictEqual(state.writes[0][1], source);
  assert.strictEqual(state.writes[1][1].accepted, false);
  assert.strictEqual(state.writes[1][1].hardware_qualification, false);
  assert.strictEqual(state.writes[1][1].solver_rerun, false);
});
const cases = [
  ['opening HEAD', s => s.io.head = () => '0'.repeat(40)],
  ['closing HEAD', s => {let calls = 0; s.io.head = () => ++calls === 1 ? p.parent : '0'.repeat(40);}],
  ['opening source hash', s => s.io.identities = () => ({...source, 'Cargo.toml': '0'.repeat(64)})],
  ['closing source map', s => {let calls = 0; s.io.identities = () => ++calls === 1 ? structuredClone(source) : {...source, 'Cargo.toml': '0'.repeat(64)};}],
  ['missing helper', s => s.data.delete(f.helpers[0])],
  ['duplicate artifact names', s => s.io.names = () => [...s.data.keys(), f.helpers[0]].sort()],
  ['changed artifact membership', s => {const old = s.io.names; let calls = 0; s.io.names = () => ++calls === 1 ? old() : [...old(), 'new.json'].sort();}],
  ['changed captured input', s => {const old = s.io.bytes; let calls = 0; s.io.bytes = name => name === f.helpers[0] && ++calls > 1 ? Buffer.from('changed') : old(name);}],
  ['live input change during captured validation', s => s.io.checkHistory = (_map, snapshot) => {
    s.data.set(f.helpers[0], Buffer.from('changed'));
    assert.strictEqual(snapshot.bytes(f.helpers[0]).toString(), f.helpers[0]);
    return {accepted: false, previous: 'tail.json'};
  }],
  ['toolchain binary drift', s => s.io.binary = () => Buffer.from('changed binary')],
  ['ambient stack override', s => s.io.env.RUST_MIN_STACK = '16777216'],
  ['occupied opening output', s => s.io.vacant = () => {throw Error('occupied');}],
  ['output created before final write check', s => {let calls = 0; s.io.vacant = () => {if (++calls > f.outputs.length) throw Error('occupied');};}],
  ['history rejection', s => s.io.checkHistory = () => {throw Error('invalid history');}],
  ['support rejection', s => s.io.checkSupport = () => {throw Error('invalid support');}],
  ['toolchain observation failure', s => s.io.execute = () => {throw Error('toolchain unavailable');}],
];
for (const [name, edit] of cases) test('rejects ' + name + ' without writes', () => {
  const state = fixture();
  edit(state);
  assert.throws(() => f.freeze(state.io));
  assert.deepStrictEqual(state.writes, []);
});
function pinFixture() {
  const data = new Map(v.helperNames.map(name => [name, Buffer.from(name)]));
  const manifest = {parent: p.parent, source_map_sha256: p.sourceMapSha256,
    helpers: v.helperNames.map(name => ({name, sha256: e.hash(data.get(name))}))};
  data.set(v.manifestName, Buffer.from(JSON.stringify(manifest)));
  const io = {bytes: name => {assert(data.has(name)); return data.get(name);}};
  return {data, io};
}
const tap = () => ['TAP version 13', ...v.freezeNames.map((name, index) => 'ok ' + (index + 1) + ' - ' + name),
  '1..26', '# tests 26', '# pass 26', '# fail 0', '# cancelled 0', '# skipped 0', '# todo 0', ''].join('\n');
const signed = (digest, text) => '# R119_VALIDATION_INPUTS_START ' + digest + '\n' + text + '# R119_VALIDATION_INPUTS_END ' + digest + '\n';
test('accepts exact input pins and TAP roster', () => {
  const state = pinFixture();
  const digest = v.inputs(state.io).sha256;
  v.checkTap(v.transcript(signed(digest, tap()), digest, true), v.freezeNames);
});
test('rejects changed helper bytes under old pins', () => {
  const state = pinFixture();
  state.data.set(v.helperNames[0], Buffer.from('changed'));
  assert.throws(() => v.inputs(state.io), /validation input pin/);
});
test('rejects an old transcript after helper repinning', () => {
  const state = pinFixture();
  const old = v.inputs(state.io).sha256;
  state.data.set(v.helperNames[0], Buffer.from('changed'));
  const manifest = JSON.parse(state.data.get(v.manifestName));
  manifest.helpers[0].sha256 = e.hash(state.data.get(v.helperNames[0]));
  state.data.set(v.manifestName, Buffer.from(JSON.stringify(manifest)));
  const current = v.inputs(state.io).sha256;
  assert.notStrictEqual(old, current);
  assert.throws(() => v.transcript(signed(old, tap()), current, true), /input pins/);
});
test('rejects same-count substituted TAP cases', () => {
  assert.throws(() => v.checkTap(tap().replace(v.freezeNames[0], 'substituted case'), v.freezeNames), /TAP roster/);
});
test('rejects repeated successful TAP summaries', () => {
  assert.throws(() => v.checkTap(tap() + '# fail 0\n', v.freezeNames), /unique exact TAP fail/);
});
test('rejects contradictory TAP summaries', () => {
  assert.throws(() => v.checkTap(tap() + '# fail 1\n', v.freezeNames), /unique exact TAP fail/);
});
test('rejects duplicate TAP case numbers', () => {
  assert.throws(() => v.checkTap(tap().replace('ok 2 - ', 'ok 1 - '), v.freezeNames), /TAP roster/);
});
test('rejects missing input pin markers', () => {
  assert.throws(() => v.transcript(tap(), '0'.repeat(64), true), /input pins/);
});
test('rejects reversed input pin markers', () => {
  const digest = '0'.repeat(64);
  const text = '# R119_VALIDATION_INPUTS_END ' + digest + '\n' + tap() + '# R119_VALIDATION_INPUTS_START ' + digest + '\n';
  assert.throws(() => v.transcript(text, digest, true), /input pins/);
});
