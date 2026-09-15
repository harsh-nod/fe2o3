// Resume after the closed fifteenth source gate; revalidate, never replace, its evidence.
const fs = require('fs'), cp = require('child_process'), crypto = require('crypto'), assert = require('assert');
const root = '/home/harsh/.codex-tmp/', repo = root + 'fe2o3-r61-execution';
const sha = b => crypto.createHash('sha256').update(b).digest('hex');
function pin(name, digest) {
  const b = fs.readFileSync(root + name); assert.strictEqual(sha(b), digest, name); return b;
}
const plan = JSON.parse(pin('r121-development-gates-v2.json', '7db1c9671038939caf37860e43a81fd4ad864ba979babc370f71714c4518f28f'));
const checker = 'r121-development-gate-check-v4.js';
const checkerSha = '1f1cb46d0a77385a4af82bca13af9bbfb312a4b698af1d933791ec39f2792217';
function run(args) {
  const result = cp.spawnSync('node', args, {cwd: repo, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024});
  if (result.error || result.signal || result.status !== 0) {
    process.stderr.write(result.stdout || '');
    process.stderr.write(result.stderr || '');
    throw result.error || new Error('closed child rejected: ' + JSON.stringify({args, status: result.status, signal: result.signal}));
  }
  return result.stdout;
}
function check(mode, selected) {
  pin(checker, checkerSha); pin(plan.runner, plan.runner_sha256);
  const report = JSON.parse(run([root + checker, mode, selected]));
  assert.strictEqual(report.development_only, true);
  assert.strictEqual(report.packet_accepted, false);
  assert.strictEqual(report.source_map_sha256, plan.source_map_sha256);
  return report;
}
assert.strictEqual(plan.gates.length, 25);
for (const spec of plan.gates.slice(15)) {
  for (const suffix of ['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']) {
    assert(!fs.existsSync(root + spec.run + suffix), 'leaf already has evidence: ' + spec.run + suffix);
  }
  if (spec.stderr_log) assert(!fs.existsSync(spec.stderr_log), 'metadata stderr already exists');
}
const full = check('leaf', 'standalone-lockfiles');
assert.deepStrictEqual(full.checked, [...plan.full_runs.map(s => s.name), ...plan.gates.slice(0, 15).map(s => s.run)]);
for (const [index, spec] of plan.gates.entries()) {
  if (index < 15) continue;
  pin(plan.runner, plan.runner_sha256);
  console.log(JSON.stringify({starting: spec.run, command: spec.command}));
  run([root + plan.runner, spec.run, spec.predecessor, ...spec.command]);
  const report = check('leaf', spec.name);
  assert.deepStrictEqual(report.checked,
    [...plan.full_runs.map(s => s.name), ...plan.gates.slice(0, index + 1).map(s => s.run)]);
  console.log(JSON.stringify({completed: spec.run, completed_leaves: index + 1, source_map_sha256: plan.source_map_sha256}));
}
console.log(JSON.stringify({development_only: true, packet_accepted: false, completed_leaves: 25,
  next: 'Run the separately declared complete gate checker and collector (thirty-case calibration already run); then assemble and independently review the archive.'}));


