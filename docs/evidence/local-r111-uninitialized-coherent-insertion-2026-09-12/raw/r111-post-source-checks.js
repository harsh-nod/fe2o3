// Run only after the completed, passing source campaign; stop on the first failure.
const fs = require('fs');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r111-exact-qualification-plan.js');
const root = '/home/harsh/.codex-tmp/';
const source = JSON.parse(fs.readFileSync(root + 'r111-source-campaign.json'));
assert.strictEqual(source.returncode, 0);
assert.strictEqual(source.signal, null);
assert.strictEqual(source.timed_out, false);
assert.strictEqual(source.source_unchanged, true);
const checks = [
  ['r111-campaign-run.js', 'r111-auxiliary-campaign', 'python3', '-B', root + 'r111-auxiliary-gates.py'],
  ...p.focused.map(([name, filter]) => ['r111-run.js', 'r111-frozen-' + name, ...p.tests, filter]),
  ['r111-run.js', 'r111-exact-clock-contract-tests', 'node', root + 'r111-exact-clock-evidence-tests.js'],
];
for (const [runner, ...args] of checks) {
  const result = cp.spawnSync('node', [root + runner, ...args], {stdio: 'inherit'});
  if (result.error) throw result.error;
  assert.strictEqual(result.signal, null);
  if (result.status !== 0) process.exit(result.status ?? 127);
}
