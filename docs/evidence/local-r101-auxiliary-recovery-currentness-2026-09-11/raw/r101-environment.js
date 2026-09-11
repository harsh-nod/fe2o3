const fs = require('fs');
const cp = require('child_process');
const crypto = require('crypto');
const commands = [
  ['cargo', '+nightly-2026-04-03', '--version', '--verbose'],
  ['rustc', '+nightly-2026-04-03', '--version', '--verbose'],
  ['python3', '-VV'], ['node', '--version'], ['git', '--version'], ['uname', '-srmo'],
];
const records = commands.map(([program, ...args]) => ({
  command: [program, ...args], output: cp.execFileSync(program, args).toString(),
}));
const binaries = ['cargo', 'rustc'].map(name => {
  const path = cp.execFileSync('rustup', ['which', '--toolchain', 'nightly-2026-04-03', name]).toString().trim();
  return {name, path, sha256: crypto.createHash('sha256').update(fs.readFileSync(path)).digest('hex')};
});
fs.writeFileSync('/home/harsh/.codex-tmp/r101-environment.json', JSON.stringify({
  recorded_at: new Date().toISOString(), records, binaries,
  build_jobs: 4, incremental: false, xdg_runtime_dir: 'unset', hardware_qualification: false,
}, null, 2) + '\n', {flag: 'wx'});
console.log('Recorded local environment and exact toolchain binaries');
