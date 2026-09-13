const assert = require('assert');
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const vm = require('vm');
const {EventEmitter} = require('events');
const root = '/home/harsh/.codex-tmp';
const runner = path.join(root, 'r113-run-v3.js');
const source = fs.readFileSync(runner, 'utf8');
const parent = 'a2feef229758381b66f962ad8be2b87843eb33a3';
let passed = 0;
function pass(name) {passed++; console.log('PASS ' + name);}
function live(pid) {
  try {
    const stat = fs.readFileSync('/proc/' + pid + '/stat', 'utf8');
    return !['Z', 'X'].includes(stat.slice(stat.lastIndexOf(')') + 2).split(' ')[0]);
  } catch (error) {if (error.code === 'ENOENT') return false; throw error;}
}
async function mocked(name, mode) {
  const writes = new Map();
  const signals = [];
  let time = 1000n;
  let unreferenced = false;
  const timedOut = ['timeout', 'unclosed'].includes(mode);
  const denied = ['eperm', 'unclosed'].includes(mode);
  const fakeProcess = {
    argv: ['node', runner, 'r113-mocked-' + name, '-', 'fixture'], env: {},
    hrtime: {bigint: () => time++}, exitCode: null,
    kill: (pid, signal) => {
      signals.push([pid, signal]);
      if (denied) throw Object.assign(new Error('denied'), {code: 'EPERM'});
      throw Object.assign(new Error('gone'), {code: 'ESRCH'});
    },
  };
  const fakeFs = {
    readFileSync: file => {
      if (file === '/proc/sys/kernel/random/boot_id') return '00000000-0000-0000-0000-000000000001';
      if (file === runner) return source;
      if (String(file).endsWith('/source.rs')) return 'unchanged';
      if (String(file).endsWith('.log')) return '';
      throw Error('unexpected read ' + file);
    },
    writeFileSync: (file, bytes, options) => {assert.strictEqual(options.flag, 'wx'); assert(!writes.has(file)); writes.set(file, String(bytes));},
    openSync: () => 99, closeSync: () => {}, writeSync: () => {}, readdirSync: () => [],
  };
  const child = new EventEmitter();
  child.pid = 424242;
  child.unref = () => {unreferenced = true;};
  const fakeCp = {
    execFileSync: (_command, args) => Buffer.from(args[0] === 'rev-parse' ? parent : 'source.rs\0'),
    spawn: () => {
      setImmediate(() => child.emit('close', mode === 'timeout' ? null : 0, mode === 'timeout' ? 'SIGKILL' : null));
      return child;
    },
  };
  const fakeTimer = (callback, ms) => {
    assert([1800000, 3000].includes(ms));
    if (ms === 3000) return setTimeout(callback, 10);
    if (timedOut) return setImmediate(callback);
    return null;
  };
  // The timeout callback must precede child close in this synthetic timeout.
  if (mode === 'timeout') fakeCp.spawn = () => {
    setImmediate(() => setImmediate(() => child.emit('close', null, 'SIGKILL')));
    return child;
  };
  if (mode === 'unclosed') fakeCp.spawn = () => child;
  await vm.runInNewContext(source, {
    require: module => module === 'fs' ? fakeFs : module === 'child_process' ? fakeCp : require(module),
    process: fakeProcess, __filename: runner, setTimeout: fakeTimer, clearTimeout: handle => {clearTimeout(handle); clearImmediate(handle);},
    console: {log: () => {}, error: () => {}},
  }, {filename: runner});
  const record = JSON.parse(writes.get(path.join(root, 'r113-mocked-' + name + '.json')));
  assert.strictEqual(record.child_returncode, timedOut ? null : 0);
  assert.strictEqual(record.child_closed, mode !== 'unclosed');
  assert.strictEqual(unreferenced, mode === 'unclosed');
  assert.strictEqual(record.signal, mode === 'timeout' ? 'SIGKILL' : null);
  assert.strictEqual(record.timed_out, timedOut);
  assert.strictEqual(record.returncode, timedOut ? 124 : 0);
  assert.strictEqual(record.clock.error, null);
  assert.strictEqual(fakeProcess.exitCode, denied ? 125 : timedOut ? 124 : 0);
  assert.strictEqual(record.process_group_cleanup.close.status, denied ? 'error' : 'absent');
  assert(signals.length === (timedOut ? 2 : 1));
  assert(signals.every(([pid, signal]) => pid === -424242 && signal === 'SIGKILL'));
  pass(name);
}
(async () => {
  const sentinel = cp.spawn(process.execPath, ['-e', 'setInterval(() => {}, 1000)'], {detached: true, stdio: 'ignore'});
  const known = new Set();
  try {
    const survivor = code => `const cp=require('child_process'); const child=cp.spawn(process.execPath,['-e','setInterval(()=>{},1000)'],{stdio:'inherit'}); console.log('SURVIVOR='+child.pid); process.exit(${code});`;
    const middle = "import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); print('SURVIVOR='+str(p.pid),flush=True); time.sleep(60)";
    const timeout = 'import subprocess,sys; subprocess.run([sys.executable,"-c",' + JSON.stringify(middle) + '],timeout=0.5)';
    const cases = [
      ['clean', process.execPath, ['-e', 'process.exit(0)'], 0],
      ['success', process.execPath, ['-e', survivor(0)], 0],
      ['failure', process.execPath, ['-e', survivor(17)], 17],
      ['inner-timeout', 'python3', ['-c', timeout], 1],
      ['spawn', '/no-such-r113-qualification-command', [], 127],
    ];
    for (const [name, command, args, expected] of cases) {
      const label = 'r113-runner-v2-fixture-' + name;
      let result;
      try {
        result = cp.spawnSync(process.execPath, [runner, label, '-', command, ...args], {encoding: 'utf8', timeout: 30000, maxBuffer: 1024 * 1024});
      } finally {
        const fixtureLog = path.join(root, label + '.log');
        if (fs.existsSync(fixtureLog)) {
          for (const match of fs.readFileSync(fixtureLog, 'utf8').matchAll(/SURVIVOR=(\d+)/g)) known.add(Number(match[1]));
        }
      }
      assert.ifError(result.error);
      assert.strictEqual(result.status, expected, result.stdout + result.stderr);
      const record = JSON.parse(fs.readFileSync(path.join(root, label + '.json'), 'utf8'));
      const log = fs.readFileSync(record.log, 'utf8');
      for (const match of log.matchAll(/SURVIVOR=(\d+)/g)) known.add(Number(match[1]));
      assert.strictEqual(record.returncode, expected);
      assert.strictEqual(record.timed_out, false);
      assert.strictEqual(record.child_closed, true);
      assert.strictEqual(record.signal, null);
      assert.strictEqual(record.clock.error, null);
      assert.strictEqual(record.source_unchanged, true);
      assert.strictEqual(record.source_head, parent);
      assert.strictEqual(record.runner.name, path.basename(runner));
      const cleanup = record.process_group_cleanup.close;
      if (name === 'spawn') {
        assert.strictEqual(cleanup.status, 'not_spawned');
        assert(record.spawn_error.includes('ENOENT'));
        assert(record.child_returncode < 0);
        assert(!Object.hasOwn(cleanup, 'pgid'));
      } else {
        assert.strictEqual(record.child_returncode, expected);
        assert.strictEqual(record.spawn_error, null);
        assert(['absent', 'signaled'].includes(cleanup.status));
        assert.deepStrictEqual(cleanup.live_members, []);
      }
      if (name === 'inner-timeout') assert(log.includes('TimeoutExpired'));
      assert(BigInt(record.clock.finish.monotonic_ns) <= BigInt(cleanup.observation.monotonic_ns));
      assert(BigInt(cleanup.observation.monotonic_ns) <= BigInt(record.clock.source_verified.monotonic_ns));
      for (const pid of known) assert(!live(pid), 'descendant remains live: ' + pid);
      assert(live(sentinel.pid), 'unrelated process group was affected');
      pass('real-' + name);
    }
    assert.strictEqual(known.size, 3);
  } finally {
    for (const pid of known) if (live(pid)) process.kill(pid, 'SIGKILL');
    const closed = new Promise(resolve => sentinel.once('close', resolve));
    try {process.kill(-sentinel.pid, 'SIGKILL');} catch (error) {if (error.code !== 'ESRCH') throw error;}
    await closed;
  }
  await mocked('cleanup-eperm-retains-success', 'eperm');
  await mocked('outer-timeout-retains-signal', 'timeout');
  await mocked('cleanup-esrch-is-benign', 'esrch');
  await mocked('timeout-eperm-unclosed-is-bounded', 'unclosed');
  assert.strictEqual(passed, 9);
  console.log('PASS: ' + passed + ' runner contract tests');
})().catch(error => {console.error(error); process.exitCode = 1;});

