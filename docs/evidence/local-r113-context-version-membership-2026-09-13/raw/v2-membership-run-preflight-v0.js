// Preliminary isolated-draft attempts; these do not qualify or modify R112.
const fs = require('node:fs');
const path = require('node:path');
const cp = require('node:child_process');
const crypto = require('node:crypto');
const assert = require('node:assert/strict');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-v2-membership');
const [name, command, ...args] = process.argv.slice(2);
assert(/^v2-membership-[a-z0-9-]+$/.test(name) && command);
const git = args => cp.execFileSync('git', args, {cwd: repo}).toString();
function identities() {
  const index = new Map(git(['ls-files', '--stage', '-z']).split('\0').filter(Boolean).map(row => {
    const split = row.indexOf('\t');
    return [row.slice(split + 1), row.slice(0, split)];
  }));
  const paths = [...new Set(git(['ls-files', '--cached', '--others', '--exclude-standard', '-z']).split('\0'))]
    .filter(name => name && !name.startsWith('docs/')).sort();
  return Object.fromEntries(paths.map(name => {
    const filename = path.join(repo, name);
    if (fs.existsSync(filename)) {
      return [name, {sha256: crypto.createHash('sha256').update(fs.readFileSync(filename)).digest('hex')}];
    }
    assert(index.has(name), 'unmaterialized source must have an index identity');
    return [name, {unmaterialized_index_entry: index.get(name)}];
  }));
}
const before = identities();
const source = path.join(root, name + '-source.json');
fs.writeFileSync(source, JSON.stringify(before, null, 2) + '\n', {flag: 'wx'});
const log = path.join(root, name + '.log');
const fd = fs.openSync(log, 'wx');
const env = {...process.env, CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', PYTHONDONTWRITEBYTECODE: '1'};
delete env.XDG_RUNTIME_DIR;
const head = git(['rev-parse', 'HEAD']).trim();
const started = Date.now();
const ticks = process.hrtime.bigint();
const child = cp.spawn(command, args, {cwd: repo, env, detached: true, stdio: ['ignore', fd, fd]});
let timedOut = false;
const timer = setTimeout(() => {
  timedOut = true;
  try { process.kill(-child.pid, 'SIGKILL'); } catch (error) { if (error.code !== 'ESRCH') throw error; }
}, 1800000);
child.on('error', error => fs.writeSync(fd, String(error) + '\n'));
child.on('close', (code, signal) => {
  clearTimeout(timer);
  fs.closeSync(fd);
  const after = identities();
  fs.writeFileSync(path.join(root, name + '-source-after.json'), JSON.stringify(after, null, 2) + '\n', {flag: 'wx'});
  const record = {
    scope: 'isolated V2 preliminary draft, not packet acceptance', source_head: head,
    started_at: new Date(started).toISOString(), finished_at: new Date().toISOString(),
    command: [command, ...args], cwd: repo, source, log,
    source_unchanged: JSON.stringify(before) === JSON.stringify(after),
    unmaterialized_sources: Object.values(before).filter(value => value.unmaterialized_index_entry).length,
    returncode: timedOut ? 124 : code ?? 127, signal, timed_out: timedOut,
    elapsed_seconds: Number(process.hrtime.bigint() - ticks) / 1e9,
    environment: {CARGO_BUILD_JOBS: '4', CARGO_INCREMENTAL: '0', RUST_TEST_THREADS: '4', XDG_RUNTIME_DIR: null, RUST_MIN_STACK: env.RUST_MIN_STACK ?? null},
  };
  fs.writeFileSync(path.join(root, name + '.json'), JSON.stringify(record, null, 2) + '\n', {flag: 'wx'});
  console.log(JSON.stringify(record));
  console.log(fs.readFileSync(log, 'utf8').slice(-12000));
  process.exitCode = record.returncode;
});
