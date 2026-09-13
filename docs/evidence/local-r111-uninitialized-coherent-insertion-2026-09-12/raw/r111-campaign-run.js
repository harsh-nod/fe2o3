// The multi-gate campaign needs its own envelope; individual gates retain 1800s deadlines.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const cp = require('child_process');
const assert = require('assert');
const repo = '/home/harsh/.codex-tmp/fe2o3-r61-execution';
const root = path.dirname(repo);
const [name, command, ...args] = process.argv.slice(2);
assert(/^r111-[a-z0-9-]+$/.test(name) && command);
function identities() {
  const paths = cp.execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z'], {cwd: repo});
  return Object.fromEntries([...new Set(paths.toString().split('\0'))]
    .filter(p => p && !p.startsWith('docs/')).sort()
    .map(p => [p, crypto.createHash('sha256').update(fs.readFileSync(path.join(repo, p))).digest('hex')]));
}
const before = identities();
const source = path.join(root, name + '-source.json');
fs.writeFileSync(source, JSON.stringify(before, null, 2) + '\n', {flag: 'wx'});
const log = path.join(root, name + '.log');
const fd = fs.openSync(log, 'wx');
const env = {...process.env, CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'};
delete env.XDG_RUNTIME_DIR;
const sourceHead = cp.execFileSync('git', ['rev-parse', 'HEAD'], {cwd: repo}).toString().trim();
const started = Date.now();
const ticks = process.hrtime.bigint();
const child = cp.spawn(command, args, {cwd: repo, env, detached: true, stdio: ['ignore', fd, fd]});
let timedOut = false;
const timer = setTimeout(() => {
  timedOut = true;
  try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
}, 7200000);
child.on('error', error => fs.writeSync(fd, String(error) + '\n'));
child.on('close', (code, signal) => {
  clearTimeout(timer);
  fs.closeSync(fd);
  const record = {source_head: sourceHead, started_at: new Date(started).toISOString(), finished_at: new Date().toISOString(), command: [command, ...args], cwd: repo, log, source,
    source_unchanged: JSON.stringify(before) === JSON.stringify(identities()),
    returncode: timedOut ? 124 : code ?? 127, signal, timed_out: timedOut, campaign_deadline_ms: 7200000,
    elapsed_seconds: Number(process.hrtime.bigint() - ticks) / 1e9,
    environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null}};
  fs.writeFileSync(path.join(root, name + '.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify(record));
  console.log(fs.readFileSync(log, 'utf8').slice(-16000));
  process.exitCode = record.returncode;
});
