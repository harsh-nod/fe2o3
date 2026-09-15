const assert = require('assert');
const {test} = require('node:test');
require('./r119-integrated-validation-v2.js').guard();
const e = require('./r119-integrated-qualification-evidence-v1.js');
const h = require('./r119-integrated-history-v2.js');
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
const musl = 'r119-integrated-musl-retry1';
const timeout = require('./r119-integrated-timeout-history-v2.js');
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

test('classifies the original timeout only as rejected closed history', () => {
  const f = fixture();
  const result = timeout.checkHistory(f.map, f.io);
  assert.strictEqual(result.accepted, false);
  assert.strictEqual(result.full_suite_complete, false);
  assert.strictEqual(result.completed_tests, 62);
  assert.strictEqual(result.partial_kfd_passing_rows, 415);
});
const timeoutCases = [
  ['relabelled success', r => r.returncode = 0],
  ['wrong raw child result', r => r.child_returncode = 0],
  ['wrong signal', r => r.signal = 'SIGTERM'],
  ['unclosed child', r => r.child_closed = false],
  ['spawn failure', r => r.spawn_error = 'ENOENT'],
  ['missing cleanup', r => r.process_group_cleanup.timeout = null],
  ['failed timeout cleanup', r => r.process_group_cleanup.timeout.status = 'error'],
  ['failed closing cleanup', r => r.process_group_cleanup.close.error = 'EPERM'],
  ['mismatched process groups', r => r.process_group_cleanup.close.pgid++],
  ['surviving members', r => r.process_group_cleanup.close.live_members = [12345]],
  ['shortened deadline', r => r.deadline_ms = 1000],
  ['coherent early timeout', r => {
    r.process_group_cleanup.timeout.observation.monotonic_ns =
      (BigInt(r.clock.start.monotonic_ns) + BigInt(r.deadline_ms) * 1000000n - 1n).toString();
  }],
  ['timeout after finish', r => {
    r.process_group_cleanup.timeout.observation.monotonic_ns = (BigInt(r.clock.finish.monotonic_ns) + 1n).toString();
  }],
  ['changed command', r => r.command.pop()],
  ['changed predecessor', r => r.clock.predecessor.sha256 = '0'.repeat(64)],
];
for (const [name, edit] of timeoutCases) test('rejects timeout ' + name + ' before artifact-pin checks', () => {
  const f = fixture();
  const record = f.io.read(timeout.name + '.json');
  edit(record);
  assert.throws(() => timeout.checkRecord(record, f.map, f.io));
});
test('rejects timeout changed source before artifact-pin checks', () => {
  const f = fixture();
  f.change(timeout.name + '-source-after.json', map => map['Cargo.toml'] = '0'.repeat(64));
  assert.throws(() => timeout.checkRecord(f.io.read(timeout.name + '.json'), f.map, f.io));
});
test('rejects missing original timeout record', () => {
  const f = fixture();
  f.data.delete(timeout.name + '.json');
  assert.throws(() => h.checkHistory(f.map, f.io));
});
test('rejects retry bypassing the timeout predecessor', () => {
  const f = fixture();
  const previous = f.io.read(gnu + '.json');
  f.change(musl + '.json', record => record.clock.predecessor = {name: gnu + '.json',
    sha256: e.hash(f.io.bytes(gnu + '.json')), observation: previous.clock.source_verified});
  assert.throws(() => h.checkHistory(f.map, f.io));
});
test('rejects a coherent retry before timeout closure', () => {
  const f = fixture();
  const previous = f.io.read(timeout.name + '.json').clock.source_verified;
  f.change(musl + '.json', record => {
    const delta = BigInt(previous.monotonic_ns) - BigInt(record.clock.start.monotonic_ns) - 1000000000n;
    const utcDelta = previous.utc_ms - record.clock.start.utc_ms - 1000;
    for (const sample of [record.clock.start, record.clock.finish, record.clock.source_verified,
      record.process_group_cleanup.close.observation]) {
      sample.monotonic_ns = (BigInt(sample.monotonic_ns) + delta).toString();
      sample.utc_ms += utcDelta;
    }
    record.started_at = new Date(record.clock.start.utc_ms).toISOString();
    record.finished_at = new Date(record.clock.finish.utc_ms).toISOString();
  });
  assert.throws(() => h.checkHistory(f.map, f.io), /monotonic order/);
});
test('rejects changed historical timeout bytes', () => {
  const f = fixture();
  f.change(timeout.name + '.json', record => record.extra = true);
  timeout.checkRecord(f.io.read(timeout.name + '.json'), f.map, f.io);
  assert.throws(() => h.checkHistory(f.map, f.io), /immutable timeout record/);
});
test('rejects changed historical helper bytes', () => {
  const f = fixture();
  f.data.set('r119-integrated-freeze-tests-v1.js', Buffer.from('changed helper'));
  assert.throws(() => h.checkHistory(f.map, f.io));
});
