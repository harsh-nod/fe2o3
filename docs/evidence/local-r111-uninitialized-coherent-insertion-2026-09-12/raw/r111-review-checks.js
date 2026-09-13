// Serialized preliminary checks; each child retains its own source map and output.
const cp = require('child_process');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const tests = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const checks = [
  ['reviewed-tests', [...tests, 'coherent_allocation_insertion_']],
  ['coherent-regressions', [...tests, 'coherent_insertion_']],
  ['device-regressions', [...tests, 'device_insertion_']],
  ['review-clippy', ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--all-targets', '--', '-D', 'warnings']],
];
for (const [name, command] of checks) {
  const result = cp.spawnSync('node', [root + 'r111-run.js', 'r111-' + name, ...command], {stdio: 'inherit'});
  if (result.error) throw result.error;
  assert.strictEqual(result.signal, null);
  if (result.status !== 0) process.exit(result.status ?? 127);
}
