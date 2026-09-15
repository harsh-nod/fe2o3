const assert = require('assert');
const e = require('./r119-integrated-qualification-evidence-v1.js');
const p = require('./r119-integrated-qualification-plan-v1.js');
const fixtureKinds = ['clean', 'success', 'failure', 'inner-timeout', 'spawn'];
const fixtureNames = fixtureKinds.flatMap(kind =>
  ['.json', '.log', '-source.json', '-source-after.json'].map(suffix => 'r119-integrated-runner-v1-fixture-' + kind + suffix));
function fixtureCommand(kind) {
  if (kind === 'clean') return [process.execPath, '-e', 'process.exit(0)'];
  if (['success', 'failure'].includes(kind)) {
    const code = kind === 'success' ? 0 : 17;
    return [process.execPath, '-e', `const cp=require('child_process'); const child=cp.spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'}); console.log('SURVIVOR='+child.pid); process.exit(${code});`];
  }
  if (kind === 'inner-timeout') {
    const middle = "import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); print('SURVIVOR='+str(p.pid),flush=True); time.sleep(60)";
    return ['python3', '-c', 'import subprocess,sys; subprocess.run([sys.executable,"-c",' + JSON.stringify(middle) + '],timeout=0.5)'];
  }
  assert.strictEqual(kind, 'spawn');
  return ['/no-such-r119-integrated-qualification-command'];
}

function checkFixtures(map, outer, io = e) {
  // Spawn failure has no process group and cannot use the ordinary run checker.
  for (const kind of fixtureKinds) {
    const name = 'r119-integrated-runner-v1-fixture-' + kind;
    const record = io.read(name + '.json');
    const expected = {clean: 0, success: 0, failure: 17, 'inner-timeout': 1, spawn: 127}[kind];
    if (kind !== 'spawn') {
      e.checkRecord(record, name, fixtureCommand(kind), map, null, expected, io,
        {head: p.parent, cwd: e.repo, contract: e.contract, runner: 'r119-integrated-run-v1.js', after: map});
    } else {
      assert.strictEqual(record.child_returncode, -2, 'raw ENOENT result');
      assert.strictEqual(record.spawn_error, 'Error: spawn /no-such-r119-integrated-qualification-command ENOENT');
      assert.strictEqual(record.process_group_cleanup.close.status, 'not_spawned');
      assert(!Object.hasOwn(record.process_group_cleanup.close, 'pgid'));
      assert(!Object.hasOwn(record.process_group_cleanup.close, 'live_members'));
    }
    assert.deepStrictEqual(record.command, fixtureCommand(kind), 'exact fixture command');
    assert.strictEqual(record.returncode, expected, 'exact fixture result');
    assert.strictEqual(record.signal, null);
    assert.strictEqual(record.timed_out, false);
    assert.strictEqual(record.deadline_ms, 1800000);
    assert.deepStrictEqual(record.environment, {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null});
    assert.strictEqual(record.process_group_cleanup.timeout, null);
    assert.strictEqual(record.log, e.file(name + '.log'));
    assert.strictEqual(record.source, e.file(name + '-source.json'));
    assert.strictEqual(record.source_after, e.file(name + '-source-after.json'));
    assert.strictEqual(record.source_head, p.parent);
    assert.strictEqual(record.source_map_sha256, p.sourceMapSha256);
    assert.strictEqual(record.cwd, e.repo);
    assert.strictEqual(record.source_unchanged, true);
    assert.strictEqual(record.child_closed, true);
    assert.strictEqual(record.clock.contract, e.contract);
    assert.strictEqual(record.clock.predecessor, null);
    assert.strictEqual(record.clock.error, null);
    assert.deepStrictEqual(record.runner, {name: 'r119-integrated-run-v1.js', sha256: e.hash(io.bytes('r119-integrated-run-v1.js'))});
    assert.deepStrictEqual(io.read(name + '-source.json'), map);
    assert.deepStrictEqual(io.read(name + '-source-after.json'), map);
    e.ordered(outer.clock.start, record.clock.start);
    e.ordered(record.clock.start, record.clock.finish);
    e.ordered(record.clock.finish, record.process_group_cleanup.close.observation);
    e.ordered(record.process_group_cleanup.close.observation, record.clock.source_verified);
    e.ordered(record.clock.source_verified, outer.clock.finish);
    assert.strictEqual(record.started_at, new Date(record.clock.start.utc_ms).toISOString());
    assert.strictEqual(record.finished_at, new Date(record.clock.finish.utc_ms).toISOString());
    assert.strictEqual(record.elapsed_seconds, Number(BigInt(record.clock.finish.monotonic_ns) - BigInt(record.clock.start.monotonic_ns)) / 1e9);
    assert.strictEqual(record.verification_elapsed_seconds, Number(BigInt(record.clock.source_verified.monotonic_ns) - BigInt(record.clock.finish.monotonic_ns)) / 1e9);
    const log = io.bytes(name + '.log').toString();
    if (kind === 'inner-timeout') assert(log.includes('TimeoutExpired'));
    if (['success', 'failure', 'inner-timeout'].includes(kind)) assert.strictEqual([...log.matchAll(/^SURVIVOR=\d+$/gm)].length, 1);
  }

  return {fixtures: fixtureKinds.length};
}
module.exports = {fixtureKinds, fixtureNames, fixtureCommand, checkFixtures};
