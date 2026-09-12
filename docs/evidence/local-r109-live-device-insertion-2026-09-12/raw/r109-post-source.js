const fs = require('fs');
const cp = require('child_process');
const assert = require('assert');
const p = require('./r109-qualification-plan.js');
const root = '/home/harsh/.codex-tmp/';
const record = JSON.parse(fs.readFileSync(root + 'r109-source-campaign.json'));
assert.strictEqual(record.returncode, 0);
assert.strictEqual(record.source_unchanged, true);
assert.strictEqual(record.signal, null);
assert.strictEqual(record.timed_out, false);
const commands = [
  ['node', root + 'r109-run.js', 'r109-auxiliary-campaign', 'python3', '-B', root + 'r109-auxiliary-gates.py'],
  ...p.focused.map(([name, filter]) => ['node', root + 'r109-run.js', 'r109-frozen-' + name, ...p.tests, filter]),
  ['node', root + 'r109-run.js', 'r109-clock-contract-tests', 'node', root + 'r109-clock-evidence-tests.js'],
  ['node', root + 'r109-clock-prepare.js'],
];
for (const command of commands) {
  const result = cp.spawnSync(command[0], command.slice(1), {stdio: 'inherit', cwd: root + 'fe2o3-r61-execution'});
  if (result.status !== 0) process.exit(result.status ?? 127);
}
console.log('PASS: serial auxiliary, frozen focused and helper-contract campaigns completed');
