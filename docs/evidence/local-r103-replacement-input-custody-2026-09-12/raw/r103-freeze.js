const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const cp = require('child_process');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const execute = command => cp.execFileSync(command[0], command.slice(1), {cwd: repo}).toString();
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
assert.strictEqual(execute(['git', 'rev-parse', 'HEAD']).trim(), '50c4eb075013fde0a984a003c0b5b90eae562847');
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const environment = {
  recorded_at: new Date().toISOString(),
  records: commands.map(command => ({command, output: execute(command)})),
  binaries: ['cargo', 'rustc'].map(name => {
    const binary = execute(['rustup', 'which', '--toolchain', 'nightly-2026-04-03', name]).trim();
    return {name, path: binary, sha256: hash(fs.readFileSync(binary))};
  }),
  build_jobs: 4, incremental: false, xdg_runtime_dir: 'unset', hardware_qualification: false,
};
const paths = [...new Set(execute(['git', 'ls-files', '--cached', '--others', '--exclude-standard', '-z'])
  .split('\0').filter(p => p && !p.startsWith('docs/')))].sort();
const baseline = Object.fromEntries(paths.map(p => [p, hash(fs.readFileSync(path.join(repo, p)))]));
assert.strictEqual(paths.length, 5659);
for (const [name, data] of [['r103-environment.json', environment], ['r103-frozen-source.json', baseline]]) {
  fs.writeFileSync(path.join(root, name), JSON.stringify(data, null, 2) + '\n', {flag: 'wx'});
}
console.log('Recorded environment and ' + paths.length + ' frozen source hashes');
