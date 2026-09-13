// Preserve raw UTC observations; elapsed time and ordering use one boot's monotonic clock.
const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const parent = 'a2feef229758381b66f962ad8be2b87843eb33a3';
const [name, previousName, command, ...args] = process.argv.slice(2);
assert(/^r113-[a-z0-9-]+$/.test(name) && previousName && command);
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
(async () => {
  assert.strictEqual(git(['rev-parse', 'HEAD']).trim(), parent);
  let previous = null;
  if (previousName !== '-') {
    assert(/^r113-[a-z0-9-]+\.json$/.test(previousName));
    const previousFile = path.join(root, previousName);
    assert(fs.lstatSync(previousFile).isFile());
    const bytes = fs.readFileSync(previousFile);
    const record = JSON.parse(bytes);
    assert.strictEqual(record.source_head, parent);
    assert.strictEqual(record.clock.contract, 'r113-raw-utc-boot-monotonic-v1');
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
  const deadline = ['r113-source-campaign', 'r113-auxiliary-campaign'].includes(name) ? 7200000 : 1800000;
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    try {process.kill(-child.pid, 'SIGKILL');} catch (error) {if (error.code !== 'ESRCH') throw error;}
  }, deadline);
  child.on('error', error => fs.writeSync(fd, String(error) + '\n'));
  const result = await new Promise(resolve => child.on('close', (code, signal) => {
    const finish = sample();
    clearTimeout(timer); fs.closeSync(fd); resolve({code, signal, finish});
  }));
  const after = identities();
  const sourceAfter = path.join(root, name + '-source-after.json');
  fs.writeFileSync(sourceAfter, JSON.stringify(after, null, 2) + '\n', {flag: 'wx'});
  const verified = sample();
  let clockError = null;
  try {ordered(start, result.finish); ordered(result.finish, verified);} catch (error) {clockError = String(error);}
  const record = {source_head: parent, source_map_sha256: hash(JSON.stringify(before)),
    started_at: new Date(start.utc_ms).toISOString(), finished_at: new Date(result.finish.utc_ms).toISOString(),
    command: [command, ...args], cwd: repo, log, source, source_after: sourceAfter,
    source_unchanged: JSON.stringify(before) === JSON.stringify(after), returncode: timedOut ? 124 : result.code ?? 127,
    signal: result.signal, timed_out: timedOut, deadline_ms: deadline,
    elapsed_seconds: Number(BigInt(result.finish.monotonic_ns) - BigInt(start.monotonic_ns)) / 1e9,
    verification_elapsed_seconds: Number(BigInt(verified.monotonic_ns) - BigInt(result.finish.monotonic_ns)) / 1e9,
    environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null},
    clock: {contract: 'r113-raw-utc-boot-monotonic-v1', predecessor: previous, start, finish: result.finish, source_verified: verified, error: clockError}};
  fs.writeFileSync(path.join(root, name + '.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify(record));
  console.log(fs.readFileSync(log, 'utf8').slice(-16000));
  process.exitCode = clockError ? 125 : record.returncode;
})().catch(error => {
  fs.writeFileSync(path.join(root, name + '-gate-failure.json'), JSON.stringify({accepted: false, error: String(error)}, null, 2) + '\n', {flag: 'wx'});
  console.error(error); process.exitCode = 125;
});
