const assert = require('assert');
const e = require('./r118-qualification-evidence.js');
const p = require('./r118b-mutation-plan-v1.js');
const h = require('./r118b-current-history-v1.js');
const suffixes = ['.json', '.log', '-source.json', '-source-after.json'];
const runs = [
  ['r118b-current-history-contracts-v1', ['node', '--test', e.file('r118b-current-history-tests-v1.js')]],
  ['r118b-current-history-validation-v1', ['node', e.file('r118b-current-history-v1.js')]],
  ['r118b-runner-contracts-v1', ['node', e.file('r118b-runner-tests.js')]],
];
const inputManifests = [
  ['r118b-current-history-inputs-v1.json', '4ade429f47f12ad9cbf7b9173e2f95dcf29f608233a9550999d12360b6e15324'],
  ['r118b-runner-inputs-v1.json', 'd13ab6f1b8d2f3b8e92edf2b5493c7705e0e0ad1cc8ad0a1cf2e473c98fc4f48'],
];
const fixtureKinds = ['clean', 'success', 'failure', 'inner-timeout', 'spawn'];
const runnerCases = ['real-clean', 'real-success', 'real-failure', 'real-inner-timeout', 'real-spawn',
  'cleanup-eperm-retains-success', 'outer-timeout-retains-signal', 'cleanup-esrch-is-benign',
  'timeout-eperm-unclosed-is-bounded'];
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
  return ['/no-such-r118b-qualification-command'];
}
function inputNames(io = e) {
  return [...new Set([...runs.flatMap(([name]) => suffixes.map(suffix => name + suffix)),
    ...fixtureKinds.flatMap(kind => suffixes.map(suffix => 'r118b-runner-v1-fixture-' + kind + suffix)),
    ...inputManifests.flatMap(([name]) => [name, ...io.read(name).helpers.map(pin => pin.name)])])].sort();
}
function check(map, history, io = e) {
  assert.strictEqual(e.hash(JSON.stringify(map)), p.sourceMapSha256);
  assert.strictEqual(history.previous, 'r118b-reviewed-musl-all.json');
  assert.strictEqual(history.full_campaign_accepted, false);
  assert.strictEqual(history.stopped_prior.accepted, false);
  for (const [name, digest] of inputManifests) {
    assert.strictEqual(e.hash(io.bytes(name)), digest, 'support input manifest');
    const manifest = io.read(name);
    assert.strictEqual(manifest.parent, p.parent);
    assert.strictEqual(manifest.source_map_sha256, p.sourceMapSha256);
    for (const pin of manifest.helpers) assert.strictEqual(e.hash(io.bytes(pin.name)), pin.sha256, pin.name);
  }
  let previous = history.previous;
  const results = runs.map(([name, command]) => {
    const result = h.currentRun(io, name, command, map, previous);
    previous = name + '.json';
    return result;
  });
  h.checkTap(results[0], 27, 'acc5718b45ac2cf7838c7a26ef260b33c17e8e3d45931f1e81dca25b8231f567');
  assert.deepStrictEqual(JSON.parse(results[1].log), history, 'closed current-history transcript');
  assert.deepStrictEqual(results[2].log.trimEnd().split('\n'), [
    ...runnerCases.map(name => 'PASS ' + name), 'PASS: 9 runner contract tests',
  ], 'exact runner contract transcript');
  // Spawn failure has no process group and cannot use the ordinary run checker.
  for (const kind of fixtureKinds) {
    const name = 'r118b-runner-v1-fixture-' + kind;
    const record = io.read(name + '.json');
    const expected = {clean: 0, success: 0, failure: 17, 'inner-timeout': 1, spawn: 127}[kind];
    if (kind !== 'spawn') {
      e.checkRecord(record, name, fixtureCommand(kind), map, null, expected, io,
        {head: p.parent, cwd: e.repo, contract: h.contract, runner: 'r118b-run-v1.js', after: map});
    } else {
      assert.strictEqual(record.child_returncode, -2, 'raw ENOENT result');
      assert.strictEqual(record.spawn_error, 'Error: spawn /no-such-r118b-qualification-command ENOENT');
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
    assert.strictEqual(record.clock.contract, h.contract);
    assert.strictEqual(record.clock.predecessor, null);
    assert.strictEqual(record.clock.error, null);
    assert.deepStrictEqual(record.runner, {name: 'r118b-run-v1.js', sha256: e.hash(io.bytes('r118b-run-v1.js'))});
    assert.deepStrictEqual(io.read(name + '-source.json'), map);
    assert.deepStrictEqual(io.read(name + '-source-after.json'), map);
    e.ordered(results[2].record.clock.start, record.clock.start);
    e.ordered(record.clock.start, record.clock.finish);
    e.ordered(record.clock.finish, record.process_group_cleanup.close.observation);
    e.ordered(record.process_group_cleanup.close.observation, record.clock.source_verified);
    e.ordered(record.clock.source_verified, results[2].record.clock.finish);
    assert.strictEqual(record.started_at, new Date(record.clock.start.utc_ms).toISOString());
    assert.strictEqual(record.finished_at, new Date(record.clock.finish.utc_ms).toISOString());
    assert.strictEqual(record.elapsed_seconds, Number(BigInt(record.clock.finish.monotonic_ns) - BigInt(record.clock.start.monotonic_ns)) / 1e9);
    assert.strictEqual(record.verification_elapsed_seconds, Number(BigInt(record.clock.source_verified.monotonic_ns) - BigInt(record.clock.finish.monotonic_ns)) / 1e9);
    const log = io.bytes(name + '.log').toString();
    if (kind === 'inner-timeout') assert(log.includes('TimeoutExpired'));
    if (['success', 'failure', 'inner-timeout'].includes(kind)) assert.strictEqual([...log.matchAll(/^SURVIVOR=\d+$/gm)].length, 1);
  }
  return {runs: runs.map(([name]) => name), current_history_contracts: 27,
    current_history_validation: true, runner_contracts: 9, retained_runner_fixtures: 5, previous};
}
module.exports = {runs, inputManifests, inputNames, runnerCases, fixtureCommand, check};
