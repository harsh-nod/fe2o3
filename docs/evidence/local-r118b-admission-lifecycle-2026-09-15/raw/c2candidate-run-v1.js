// Preserve raw UTC observations; elapsed time and ordering use one boot's monotonic clock.
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-c2-identity');
const parent = '4756971168f6b4f2c33d217f5d476ccde8ea2740';
const [name, previousName, command, ...args] = process.argv.slice(2);
assert(/^c2candidate-[a-z0-9-]+$/.test(name) && previousName && command);
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const git = args => cp.execFileSync('git', args, {cwd: repo, maxBuffer: 64 * 1024 * 1024}).toString();
const boot = () => fs.readFileSync('/proc/sys/kernel/random/boot_id', 'utf8').trim();
const clockSource = 'node-process-hrtime-linux-monotonic';
const sample = () => ({utc_ms: Date.now(), monotonic_ns: process.hrtime.bigint().toString(), boot_id: boot(), clock_source: clockSource});
function ordered(a, b) {
  for (const s of [a, b]) {
    assert(Number.isSafeInteger(s.utc_ms) && s.utc_ms >= 0);
    assert(typeof s.monotonic_ns === 'string' && /^\d+$/.test(s.monotonic_ns));
    assert(/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(s.boot_id));
    assert.strictEqual(s.clock_source, clockSource);
  }
  assert.strictEqual(a.boot_id, b.boot_id, 'same execution boot');
  assert(BigInt(b.monotonic_ns) >= BigInt(a.monotonic_ns), 'monotonic observation order');
}
function identities() {
  return Object.fromEntries([...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(n => n && !n.startsWith('docs/')).sort().map(n => [n, hash(fs.readFileSync(path.join(repo, n)))]));
}
function signalGroup(pid) {
  if (!Number.isSafeInteger(pid) || pid <= 1) return {status: 'not_spawned'};
  try {
    process.kill(-pid, 'SIGKILL');
    return {status: 'signaled', pgid: pid};
  } catch (error) {
    return {status: error.code === 'ESRCH' ? 'absent' : 'error', pgid: pid, error: String(error)};
  }
}
function liveGroupMembers(pgid) {
  const members = [];
  for (const entry of fs.readdirSync('/proc')) {
    if (!/^[1-9][0-9]*$/.test(entry)) continue;
    let stat;
    try {stat = fs.readFileSync('/proc/' + entry + '/stat', 'utf8');}
    catch (error) {if (['ENOENT', 'ESRCH'].includes(error.code)) continue; throw error;}
    const fields = stat.slice(stat.lastIndexOf(')') + 2).trim().split(/\s+/);
    if (Number(fields[2]) === pgid && !['Z', 'X'].includes(fields[0])) members.push(Number(entry));
  }
  return members.sort((a, b) => a - b);
}
async function closeGroup(pid) {
  const action = signalGroup(pid);
  if (['not_spawned', 'error'].includes(action.status)) return {...action, observation: sample()};
  const deadline = process.hrtime.bigint() + 3000000000n;
  try {
    for (;;) {
      const members = liveGroupMembers(pid);
      if (members.length === 0) return {...action, live_members: members, observation: sample()};
      if (process.hrtime.bigint() >= deadline) return {...action, status: 'error',
        error: 'owned group still has live members after cleanup deadline', live_members: members, observation: sample()};
      await new Promise(resolve => setTimeout(resolve, 10));
    }
  } catch (error) {return {...action, status: 'error', error: String(error), observation: sample()};}
}
(async () => {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
  let previous = null;
  if (previousName !== '-') {
    assert(/^c2candidate-[a-z0-9-]+\.json$/.test(previousName));
    const previousFile = path.join(root, previousName);
    assert(fs.lstatSync(previousFile).isFile());
    const bytes = fs.readFileSync(previousFile);
    const record = JSON.parse(bytes);
    assert.strictEqual(record.source_head, parent);
    assert.strictEqual(record.clock.contract, 'c2candidate-raw-utc-boot-monotonic-v1');
    assert.strictEqual(record.clock.error, null);
    previous = {name: previousName, sha256: hash(bytes), observation: record.clock.source_verified};
  }
  const before = identities();
  const source = path.join(root, name + '-source.json');
  fs.writeFileSync(source, JSON.stringify(before, null, 2) + '\n', {flag: 'wx'});
  const log = path.join(root, name + '.log');
  const fd = fs.openSync(log, 'wx');
  const env = {...process.env, CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'};
  delete env.XDG_RUNTIME_DIR;
  assert.strictEqual(process.env.RUST_MIN_STACK ?? null, null);
  const start = sample();
  ordered(start, start);
  if (previous) ordered(previous.observation, start);
  const child = cp.spawn(command, args, {cwd: repo, env, detached: true, stdio: ['ignore', fd, fd]});
  const deadline = ['c2candidate-source-campaign', 'c2candidate-auxiliary-campaign'].includes(name) ? 7200000 : 1800000;
  let timedOut = false;
  let timeoutCleanup = null;
  let spawnError = null;
  let closeGuard = null;
  let resolveChild;
  let childSettled = false;
  const finishChild = (code, signal, observed) => {
    if (childSettled) return;
    childSettled = true;
    const finish = sample();
    clearTimeout(timer);
    if (closeGuard) clearTimeout(closeGuard);
    if (!observed) child.unref();
    resolveChild({code, signal, finish, observed});
  };
  const timer = setTimeout(() => {
    timedOut = true;
    timeoutCleanup = {...signalGroup(child.pid), observation: sample()};
    closeGuard = setTimeout(() => finishChild(null, null, false), 3000);
  }, deadline);
  child.on('error', error => {spawnError = String(error); fs.writeSync(fd, spawnError + '\n');});
  const result = await new Promise(resolve => {
    resolveChild = resolve;
    child.on('close', (code, signal) => finishChild(code, signal, true));
  });
  // A completed campaign leader may leave compiler or test descendants behind.
  const cleanup = await closeGroup(child.pid);
  fs.closeSync(fd);
  const after = identities();
  const sourceAfter = path.join(root, name + '-source-after.json');
  fs.writeFileSync(sourceAfter, JSON.stringify(after, null, 2) + '\n', {flag: 'wx'});
  const verified = sample();
  let clockError = null;
  try {
    ordered(start, result.finish); ordered(result.finish, cleanup.observation);
    ordered(cleanup.observation, verified);
    if (timeoutCleanup) {ordered(start, timeoutCleanup.observation); ordered(timeoutCleanup.observation, result.finish);}
  } catch (error) {clockError = String(error);}
  const record = {source_head: parent, source_map_sha256: hash(JSON.stringify(before)),
    started_at: new Date(start.utc_ms).toISOString(), finished_at: new Date(result.finish.utc_ms).toISOString(),
    command: [command, ...args], cwd: repo, log, source, source_after: sourceAfter,
    source_unchanged: JSON.stringify(before) === JSON.stringify(after), returncode: timedOut ? 124 : spawnError ? 127 : result.code ?? 127,
    child_returncode: result.code, child_closed: result.observed, spawn_error: spawnError,
    signal: result.signal, timed_out: timedOut, deadline_ms: deadline,
    runner: {name: path.basename(__filename), sha256: hash(fs.readFileSync(__filename))},
    process_group_cleanup: {timeout: timeoutCleanup, close: cleanup},
    elapsed_seconds: Number(BigInt(result.finish.monotonic_ns) - BigInt(start.monotonic_ns)) / 1e9,
    verification_elapsed_seconds: Number(BigInt(verified.monotonic_ns) - BigInt(result.finish.monotonic_ns)) / 1e9,
    environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null},
    clock: {contract: 'c2candidate-raw-utc-boot-monotonic-v1', predecessor: previous, start, finish: result.finish, source_verified: verified, error: clockError}};
  fs.writeFileSync(path.join(root, name + '.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify(record));
  console.log(fs.readFileSync(log, 'utf8').slice(-16000));
  process.exitCode = !result.observed || clockError || cleanup.status === 'error' || timeoutCleanup?.status === 'error' ? 125 : record.returncode;
})().catch(error => {
  fs.writeFileSync(path.join(root, name + '-gate-failure.json'), JSON.stringify({accepted: false, error: String(error)}, null, 2) + '\n', {flag: 'wx'});
  console.error(error); process.exitCode = 125;
});






