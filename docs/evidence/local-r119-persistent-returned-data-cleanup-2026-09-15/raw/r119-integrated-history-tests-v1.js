const assert = require('assert');
const {test} = require('node:test');
require('./r119-integrated-validation-v1.js').guard();
const e = require('./r119-integrated-qualification-evidence-v1.js');
const h = require('./r119-integrated-history-v1.js');
const names = h.inputNames();
const inputs = new Map(names.map(name => [name, e.bytes(name)]));
const source = e.read('r119-integrated-gnu-all-source.json');
const gitCache = new Map();
function fixture() {
  const data = new Map(inputs);
  const io = {bytes: name => {
    assert(data.has(name), 'missing fixture input: ' + name);
    return data.get(name);
  }, git: command => {
    const key = JSON.stringify(command);
    if (!gitCache.has(key)) gitCache.set(key, e.git(command));
    return gitCache.get(key);
  }};
  io.read = name => JSON.parse(io.bytes(name));
  return {io, data, map: structuredClone(source),
    change(name, edit) {
      const value = io.read(name);
      edit(value);
      data.set(name, Buffer.from(JSON.stringify(value, null, 2) + '\n'));
    }};
}
const gnu = 'r119-integrated-gnu-all';
const musl = 'r119-integrated-musl-all';
test('accepts exact isolated and integrated cohorts without packet acceptance', () => {
  const f = fixture();
  const result = h.checkHistory(f.map, f.io);
  assert.strictEqual(result.accepted, false);
  assert.deepStrictEqual(result.full.gnu, {harnesses: 48, passed: 2795, failed: 0, ignored: 5});
  assert.deepStrictEqual(result.full.musl, result.full.gnu);
  assert.strictEqual(result.isolated_zero_test_run, 'compile-only');
  assert.strictEqual(result.previous, musl + '.json');
});
const cases = [
  ['missing origin input', f => f.data.delete(h.originName)],
  ['rewritten origin acceptance', f => f.change(h.originName, r => r.accepted = true)],
  ['missing isolated log', f => f.data.delete('r119-persistent-focused.log')],
  ['relabelled zero-test isolated history', f => f.data.set('r119-persistent-focused.log', f.data.get('r119-persistent-focused-v2.log'))],
  ['changed integrated runner', f => f.data.set('r119-integrated-run-v1.js', Buffer.from('process.exit(0);'))],
  ['wrong current source', f => f.map['Cargo.toml'] = '0'.repeat(64)],
  ['missing current source', f => delete f.map['Cargo.toml']],
  ['wrong integrated parent', f => f.change(musl + '.json', r => r.source_head = '0'.repeat(40))],
  ['wrong integrated cwd', f => f.change(musl + '.json', r => r.cwd = '/tmp/other')],
  ['wrong musl command', f => f.change(musl + '.json', r => r.command.pop())],
  ['forged predecessor hash', f => f.change(musl + '.json', r => r.clock.predecessor.sha256 = '0'.repeat(64))],
  ['wrong execution boot', f => f.change(musl + '.json', r => r.clock.finish.boot_id = '00000000-0000-0000-0000-000000000000')],
  ['unclosed child', f => f.change(musl + '.json', r => r.child_closed = false)],
  ['surviving owned process', f => f.change(musl + '.json', r => r.process_group_cleanup.close.live_members = [12345])],
  ['timed out execution', f => f.change(musl + '.json', r => r.timed_out = true)],
  ['changed source after execution', f => f.change(musl + '-source-after.json', r => r['Cargo.toml'] = '0'.repeat(64))],
  ['false successful command status', f => f.change(musl + '.json', r => r.returncode = 101)],
  ['lost test with unchanged summary', f => f.data.set(musl + '.log', Buffer.from(f.io.bytes(musl + '.log').toString().replace(/^test .+ \.\.\. ok\n/m, '')))],
  ['changed ignored roster', f => f.data.set(musl + '.log', Buffer.from(f.io.bytes(musl + '.log').toString().replace(/^test (.+) \.\.\. ignored/m, 'test wrong::ignored ... ignored')))],
  ['contradictory failed result', f => f.data.set(gnu + '.log', Buffer.concat([f.io.bytes(gnu + '.log'), Buffer.from('\ntest wrong::result ... FAILED\n')]))],
  ['missing preliminary Clippy record', f => f.data.delete('r119-integrated-clippy.json')],
];
for (const [name, edit] of cases) test('rejects ' + name, () => {
  const f = fixture();
  edit(f);
  assert.throws(() => h.checkHistory(f.map, f.io));
});
test('rejects an input changed after capture', () => {
  const f = fixture();
  const original = f.io.bytes;
  let reads = 0;
  f.io.bytes = name => {
    const bytes = original(name);
    if (name === musl + '.log' && ++reads > 1) return Buffer.concat([bytes, Buffer.from('\nchanged')]);
    return bytes;
  };
  assert.throws(() => h.checkHistory(f.map, f.io), /unchanged history input/);
});
test('rejects a coherent Clippy branch after the advertised musl tail', () => {
  const f = fixture();
  const tail = f.io.read(musl + '.json').clock.source_verified;
  f.change('r119-integrated-clippy.json', record => {
    const delta = BigInt(tail.monotonic_ns) - BigInt(record.clock.start.monotonic_ns) + 1000000000n;
    const utcDelta = tail.utc_ms - record.clock.start.utc_ms + 1000;
    for (const sample of [record.clock.start, record.clock.finish, record.clock.source_verified,
      record.process_group_cleanup.close.observation]) {
      sample.monotonic_ns = (BigInt(sample.monotonic_ns) + delta).toString();
      sample.utc_ms += utcDelta;
    }
    record.started_at = new Date(record.clock.start.utc_ms).toISOString();
    record.finished_at = new Date(record.clock.finish.utc_ms).toISOString();
  });
  const record = f.io.read('r119-integrated-clippy.json');
  e.checkRecord(record, 'r119-integrated-clippy', record.command, f.map,
    'r119-integrated-gnu-all.json', 0, f.io);
  assert.throws(() => h.checkHistory(f.map, f.io), /monotonic order/);
});
