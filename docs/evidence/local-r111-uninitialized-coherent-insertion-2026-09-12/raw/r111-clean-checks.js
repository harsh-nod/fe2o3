// Validate the narrow review fixes before freezing a qualification candidate.
const cp = require('child_process');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp/';
const tests = ['cargo', '+nightly-2026-04-03', 'test', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib'];
const checks = [
  ['clean-clippy', ['cargo', '+nightly-2026-04-03', 'clippy', '--locked', '--offline', '-p', 'fe2o3-kfd', '--all-features', '--all-targets', '--', '-D', 'warnings']],
  ['closing-hook', [...tests, 'coherent_allocation_insertion_operation_panic_wins_secondary_retake_error_or_panic']],
  ['model-wrapper', [...tests, 'coherent_insertion_constructed_success_preserves_exact_order_source_and_accounts']],
  ['format-check', ['cargo', '+nightly-2026-04-03', 'fmt', '--all', '--', '--check']],
  ['whitespace-check', ['git', 'diff', '--check']],
];
for (const [name, command] of checks) {
  const result = cp.spawnSync('node', [root + 'r111-run.js', 'r111-' + name, ...command], {stdio: 'inherit'});
  if (result.error) throw result.error;
  assert.strictEqual(result.signal, null);
  if (result.status !== 0) process.exit(result.status ?? 127);
}
