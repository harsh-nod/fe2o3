// R119 transcript parsers remain pinned; only the R123 record policy is adapted.
const fs = require('fs'), path = require('path'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const inputs = {
  "r119-integrated-qualification-evidence-v1.js": "fbbfdd4862e0fe1aabbe386afff7ad521ea563f3729f246cd7b1b3ad0e065bf2",
  "r119-integrated-qualification-plan-v1.js": "41c3488e37b9532752ee85387e7744660290dfedb618add692ddb1f105be11d7",
  "r119-integrated-mutation-core-v1.js": "8e7cd26968fc9c0de65cecc10f644717febdfcd1af75e13845201ebfd2859583",
  "r119-persistent-mutations-v1.js": "31ade03bb907f7a7a8f0de2f457dcbc473ac821223ab9d516a0bb0bc1aba6cad",
  "r119-persistent-mutations-v2.js": "416c1c0f1e0dd75b87780c0d2fa88dd5ec07aa14f5876714fcbf978c6bd3098a"
};
for (const [name, digest] of Object.entries(inputs)) {
  assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(path.join(root, name))).digest('hex'), digest);
}
const E = require('./r119-integrated-qualification-evidence-v1.js');
const {file, hash, ordered} = E;
function checkRecord(r, name, command, expectedMap, previousName, code, io, context) {
  assert(/^r123-development-[a-z0-9-]+$/.test(name), 'R123 record namespace');
  assert.strictEqual(r.source_head, context.head);
  assert.strictEqual(r.cwd, context.cwd);
  assert.deepStrictEqual(r.command, command, 'exact command');
  assert.strictEqual(r.log, file(name + '.log'));
  assert.strictEqual(r.source, file(name + '-source.json'));
  assert.strictEqual(r.source_after, file(name + '-source-after.json'));
  assert.strictEqual(r.source_unchanged, hash(JSON.stringify(expectedMap)) === hash(JSON.stringify(context.after)));
  assert.strictEqual(r.returncode, code);
  assert.strictEqual(r.child_returncode, code, 'raw child result');
  assert.strictEqual(r.child_closed, true, 'observed child closure');
  assert.strictEqual(r.spawn_error, null);
  assert.strictEqual(r.signal, null);
  assert.strictEqual(r.timed_out, false);
  const full = ['r123-development-gnu-all-v1', 'r123-development-musl-all-v1'].includes(name);
  const campaign = ['r123-development-source-campaign', 'r123-development-auxiliary-campaign'].includes(name);
  assert.strictEqual(r.deadline_ms, full ? 3600000 : campaign ? 7200000 : 1800000, 'declared deadline policy');
  assert.deepStrictEqual(r.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
  assert.deepStrictEqual(r.runner, {name: context.runner, sha256: hash(io.bytes(context.runner))});
  assert.deepStrictEqual(io.read(name + '-source.json'), expectedMap);
  assert.deepStrictEqual(io.read(name + '-source-after.json'), context.after);
  assert.strictEqual(r.source_map_sha256, hash(JSON.stringify(expectedMap)));
  assert.strictEqual(r.clock.contract, context.contract);
  assert.strictEqual(r.clock.error, null);
  if (previousName === null) {
    assert.strictEqual(r.clock.predecessor, null, 'independent anchor has no predecessor');
  } else {
    const previous = io.read(previousName);
    assert.strictEqual(previous.source_head, context.head);
    assert.strictEqual(previous.clock.contract, context.contract);
    assert.strictEqual(previous.clock.error, null);
    assert.deepStrictEqual(r.clock.predecessor, {name: previousName, sha256: hash(io.bytes(previousName)), observation: previous.clock.source_verified});
    ordered(previous.clock.source_verified, r.clock.start);
  }
  ordered(r.clock.start, r.clock.finish);
  const cleanup = r.process_group_cleanup;
  assert.strictEqual(cleanup.timeout, null);
  assert(['absent', 'signaled'].includes(cleanup.close.status), 'successful group cleanup');
  assert(Number.isSafeInteger(cleanup.close.pgid) && cleanup.close.pgid > 1);
  assert.deepStrictEqual(cleanup.close.live_members, [], 'no live owned-group members');
  if (cleanup.close.status === 'absent') assert(cleanup.close.error.includes('ESRCH'));
  else assert.strictEqual(cleanup.close.error, undefined);
  ordered(r.clock.finish, cleanup.close.observation);
  ordered(cleanup.close.observation, r.clock.source_verified);
  assert.strictEqual(r.started_at, new Date(r.clock.start.utc_ms).toISOString());
  assert.strictEqual(r.finished_at, new Date(r.clock.finish.utc_ms).toISOString());
  assert.strictEqual(r.elapsed_seconds, Number(BigInt(r.clock.finish.monotonic_ns) - BigInt(r.clock.start.monotonic_ns)) / 1e9);
  assert.strictEqual(r.verification_elapsed_seconds, Number(BigInt(r.clock.source_verified.monotonic_ns) - BigInt(r.clock.finish.monotonic_ns)) / 1e9);
}

module.exports = {...E, checkRecord};
