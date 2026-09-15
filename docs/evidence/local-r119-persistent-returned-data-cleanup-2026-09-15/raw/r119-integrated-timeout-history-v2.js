const assert = require('assert');
const e = require('./r119-integrated-qualification-evidence-v1.js');
const p = require('./r119-integrated-qualification-plan-v2.js');
const name = 'r119-integrated-musl-all';
const recordHash = '45cc664330884b635e762122cd606e0d660e7596a9967eabce7a991d3d928052';
const logHash = '54a51986daa7488c7f36dd0ae3ef6bcc5677b533df0c66a5c89a7d7df92d4160';
function checkRecord(record, map, io = e) {
  assert.strictEqual(record.source_head, p.parent);
  assert.strictEqual(record.cwd, e.repo);
  assert.deepStrictEqual(record.command, [...p.fullTest, '--target', 'x86_64-unknown-linux-musl']);
  assert.strictEqual(record.log, e.file(name + '.log'));
  assert.strictEqual(record.source, e.file(name + '-source.json'));
  assert.strictEqual(record.source_after, e.file(name + '-source-after.json'));
  assert.strictEqual(record.source_unchanged, true);
  assert.strictEqual(record.source_map_sha256, e.hash(JSON.stringify(map)));
  assert.deepStrictEqual(io.read(name + '-source.json'), map);
  assert.deepStrictEqual(io.read(name + '-source-after.json'), map);
  assert.strictEqual(record.returncode, 124, 'timeout wrapper status');
  assert.strictEqual(record.child_returncode, null, 'signal-only child termination');
  assert.strictEqual(record.child_closed, true, 'observed timeout child closure');
  assert.strictEqual(record.spawn_error, null);
  assert.strictEqual(record.signal, 'SIGKILL', 'owned timeout signal');
  assert.strictEqual(record.timed_out, true);
  assert.strictEqual(record.deadline_ms, 1800000, 'unchanged outer deadline');
  assert.deepStrictEqual(record.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  assert.deepStrictEqual(record.runner, {name: 'r119-integrated-run-v1.js', sha256: e.hash(io.bytes('r119-integrated-run-v1.js'))});
  assert.strictEqual(record.clock.contract, e.contract);
  assert.strictEqual(record.clock.error, null);
  const previousName = 'r119-integrated-gnu-all.json';
  const previous = io.read(previousName);
  assert.strictEqual(previous.source_head, p.parent);
  assert.strictEqual(previous.clock.contract, e.contract);
  assert.strictEqual(previous.clock.error, null);
  assert.deepStrictEqual(record.clock.predecessor, {name: previousName,
    sha256: e.hash(io.bytes(previousName)), observation: previous.clock.source_verified});
  e.ordered(previous.clock.source_verified, record.clock.start);
  const {timeout, close} = record.process_group_cleanup;
  assert(timeout, 'timeout cleanup is recorded');
  assert.strictEqual(timeout.status, 'signaled', 'timeout group signaled');
  assert.strictEqual(close.status, 'signaled', 'closing group signaled');
  assert(Number.isSafeInteger(timeout.pgid) && timeout.pgid > 1, 'valid owned PGID');
  assert.strictEqual(close.pgid, timeout.pgid, 'same owned group');
  assert.strictEqual(timeout.error, undefined);
  assert.strictEqual(close.error, undefined);
  assert.deepStrictEqual(close.live_members, [], 'no surviving owned members');
  e.ordered(record.clock.start, timeout.observation);
  assert(BigInt(timeout.observation.monotonic_ns) - BigInt(record.clock.start.monotonic_ns)
    >= BigInt(record.deadline_ms) * 1000000n, 'timeout cannot precede its deadline');
  e.ordered(timeout.observation, record.clock.finish);
  e.ordered(record.clock.finish, close.observation);
  e.ordered(close.observation, record.clock.source_verified);
  assert.strictEqual(record.started_at, new Date(record.clock.start.utc_ms).toISOString());
  assert.strictEqual(record.finished_at, new Date(record.clock.finish.utc_ms).toISOString());
  assert.strictEqual(record.elapsed_seconds, Number(BigInt(record.clock.finish.monotonic_ns) - BigInt(record.clock.start.monotonic_ns)) / 1e9);
  assert.strictEqual(record.verification_elapsed_seconds, Number(BigInt(record.clock.source_verified.monotonic_ns) - BigInt(record.clock.finish.monotonic_ns)) / 1e9);
}
function checkHistory(map, io = e) {
  checkRecord(io.read(name + '.json'), map, io);
  assert.strictEqual(e.hash(io.bytes(name + '.json')), recordHash, 'immutable timeout record');
  assert.strictEqual(e.hash(io.bytes(name + '.log')), logHash, 'immutable partial transcript');
  const log = io.bytes(name + '.log').toString();
  assert.deepStrictEqual(e.failed(log), []);
  assert.deepStrictEqual(e.totals(log), {harnesses: 3, passed: 62, failed: 0, ignored: 0});
  assert.strictEqual(e.passing(log).length, 477);
  return {accepted: false, kind: 'closed-timeout', record: name + '.json', record_sha256: recordHash,
    log_sha256: logHash, completed_harnesses: 3, completed_tests: 62,
    partial_kfd_passing_rows: 415, full_suite_complete: false};
}
module.exports = {name, recordHash, logHash, checkRecord, checkHistory};
