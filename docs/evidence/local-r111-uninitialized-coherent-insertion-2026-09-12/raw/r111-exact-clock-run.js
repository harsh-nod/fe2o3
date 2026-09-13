const fs = require('fs');
const path = require('path');
const cp = require('child_process');
const assert = require('assert');
const c = require('./r111-exact-clock-evidence.js');
const repo = c.root + 'fe2o3-r61-execution';
const [name, previous, field, command, ...args] = process.argv.slice(2);
assert(/^r111-exact-clock-[a-z0-9-]+$/.test(name) && command);
function identities() {
  const paths = cp.execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {cwd: repo});
  return Object.fromEntries([...new Set(paths.toString().split('\0'))].filter(p => p && !p.startsWith('docs/')).sort()
    .map(p => [p, c.hash(fs.readFileSync(path.join(repo, p)))]));
}
(async () => {
  const planned = c.plan(name + '.json');
  assert.deepStrictEqual([previous, field], [planned.expected.name, planned.expected.field]);
  assert.deepStrictEqual([command, ...args], planned.entry.command);
  const predecessor = c.predecessor(previous, field, planned.expected);
  const before = identities();
  assert.strictEqual(c.hash(JSON.stringify(before)), planned.entry.source_map_sha256);
  const source = c.root + name + '-source.json';
  fs.writeFileSync(source, JSON.stringify(before, null, 2) + '\n', {flag: 'wx'});
  const log = c.root + name + '.log';
  const fd = fs.openSync(log, 'wx');
  const env = {...process.env, CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'};
  delete env.XDG_RUNTIME_DIR;
  const sourceHead = cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim();
  assert.strictEqual(sourceHead, planned.manifest.source_head);
  let start;
  try { start = await c.gate(predecessor.utc_ms); } catch (error) { fs.closeSync(fd); throw error; }
  const started = start.samples.at(-1);
  const child = cp.spawn(command, args, {cwd: repo, env, detached: true, stdio: ['ignore', fd, fd]});
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
  }, 1800000);
  child.on('error', error => fs.writeSync(fd, String(error) + '\n'));
  const result = await new Promise(resolve => child.on('close', (code, signal) => {
    const finish = c.sample();
    clearTimeout(timer);
    fs.closeSync(fd);
    resolve({code, signal, finish});
  }));
  const after = identities();
  const verified = c.sample();
  let clockError = null;
  try {c.verifyFinish(started, result.finish);} catch (error) {clockError = String(error);}
  const record = {source_head: sourceHead, source_map_sha256: c.hash(JSON.stringify(before)), manifest_sha256: planned.manifest_sha256,
    started_at: new Date(started.utc_ms).toISOString(),
    finished_at: new Date(result.finish.utc_ms).toISOString(), command: [command, ...args], cwd: repo, log, source,
    source_unchanged: JSON.stringify(before) === JSON.stringify(after),
    returncode: timedOut ? 124 : result.code ?? 127, signal: result.signal, timed_out: timedOut,
    elapsed_seconds: Number(BigInt(result.finish.monotonic_ns) - BigInt(started.monotonic_ns)) / 1e9,
    verification_elapsed_seconds: Number(BigInt(verified.monotonic_ns) - BigInt(result.finish.monotonic_ns)) / 1e9,
    environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null},
    clock: {contract: c.contract, policy: c.policy, kind: 'run', predecessor, start, finish: result.finish, source_verified: verified, error: clockError}};
  fs.writeFileSync(c.root + name + '.json', JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify(record));
  console.log(fs.readFileSync(log, 'utf8').slice(-16000));
  process.exitCode = clockError ? 125 : record.returncode;
})().catch(error => {
  fs.writeFileSync(c.root + name + '-gate-failure.json', JSON.stringify({accepted: false, error: String(error), clock_failure: error.clock_failure || null}, null, 2) + '\n', {flag: 'wx'});
  console.error(error); process.exitCode = 125;
});
