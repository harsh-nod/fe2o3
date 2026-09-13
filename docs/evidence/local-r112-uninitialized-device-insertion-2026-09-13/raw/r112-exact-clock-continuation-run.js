const cp = require('child_process');
const c = require('./r112-exact-clock-continuation-evidence.js');
const {fs, assert} = c;
const [name, command, ...args] = process.argv.slice(2);
assert(name && name.startsWith(c.prefix) && command);
(async () => {
  const planned = c.plan(name);
  assert.deepStrictEqual([command, ...args], planned.entry.command);
  const before = c.identities();
  assert.strictEqual(c.hash(JSON.stringify(before)), planned.manifest.source_map_sha256);
  assert.strictEqual(c.git(['rev-parse', 'HEAD']).trim(), planned.manifest.source_head);
  const source = c.file(name + '-source.json');
  fs.writeFileSync(source, JSON.stringify(before, null, 2) + '\n', {flag: 'wx'});
  const log = c.file(name + '.log');
  const fd = fs.openSync(log, 'wx');
  const env = {...process.env, CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'};
  delete env.XDG_RUNTIME_DIR;
  assert.strictEqual(process.env.RUST_MIN_STACK ?? null, null);
  const start = c.observation(c.boot());
  c.ordered(planned.previous.observation, start, planned.manifest.boot_id);
  const child = cp.spawn(command, args, {cwd: c.repo, env, detached: true, stdio: ['ignore', fd, fd]});
  let timedOut = false;
  const timer = setTimeout(() => {
    timedOut = true;
    try {process.kill(-child.pid, 'SIGKILL');} catch (error) {if (error.code !== 'ESRCH') throw error;}
  }, 1800000);
  child.on('error', error => fs.writeSync(fd, String(error) + '\n'));
  const result = await new Promise(resolve => child.on('close', (code, signal) => {
    const finish = c.observation(c.boot());
    clearTimeout(timer); fs.closeSync(fd); resolve({code, signal, finish});
  }));
  const after = c.identities();
  const sourceAfter = c.file(name + '-source-after.json');
  fs.writeFileSync(sourceAfter, JSON.stringify(after, null, 2) + '\n', {flag: 'wx'});
  const verified = c.observation(c.boot());
  const record = {source_head: planned.manifest.source_head, source_map_sha256: c.hash(JSON.stringify(before)),
    manifest_sha256: planned.manifestHash, started_at: new Date(start.utc_ms).toISOString(),
    finished_at: new Date(result.finish.utc_ms).toISOString(), command: [command, ...args], cwd: c.repo, log, source, source_after: sourceAfter,
    source_unchanged: JSON.stringify(before) === JSON.stringify(after), returncode: timedOut ? 124 : result.code ?? 127,
    signal: result.signal, timed_out: timedOut,
    elapsed_seconds: Number(BigInt(result.finish.monotonic_ns) - BigInt(start.monotonic_ns)) / 1e9,
    verification_elapsed_seconds: Number(BigInt(verified.monotonic_ns) - BigInt(result.finish.monotonic_ns)) / 1e9,
    environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null},
    clock: {contract: c.contract, kind: 'run', predecessor: planned.previous, start, finish: result.finish, source_verified: verified, error: null}};
  try {c.verifyRun(record, planned.entry, planned.previous, planned.manifest, planned.manifestHash);}
  catch (error) {record.clock.error = String(error);}
  fs.writeFileSync(c.file(name + '.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify(record));
  console.log(fs.readFileSync(log, 'utf8').slice(-16000));
  process.exitCode = record.clock.error ? 125 : record.returncode;
})().catch(error => {
  fs.writeFileSync(c.file(name + '-gate-failure.json'), JSON.stringify({accepted: false, error: String(error)}, null, 2) + '\n', {flag: 'wx'});
  console.error(error); process.exitCode = 125;
});
