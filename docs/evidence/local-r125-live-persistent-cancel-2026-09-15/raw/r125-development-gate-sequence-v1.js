// Run the frozen leaf commands serially; never retry or replace an existing attempt.
const fs = require('fs'), path = require('path'), cp = require('child_process');
const assert = require('assert'), crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const hash = value => crypto.createHash('sha256').update(value).digest('hex');
const planName = 'r125-development-qualification-plan-v1.json';
const planHash = '9760e1c22a227b52d90df87d99aeeaad6cc38803ccf57669b23af00a3de0be07';
const planBytes = fs.readFileSync(root + planName);
assert.strictEqual(hash(planBytes), planHash);
const q = JSON.parse(planBytes);
for (const [name, digest] of Object.entries(q.inputs))
  assert.strictEqual(hash(fs.readFileSync(root + name)), digest, name);
assert.strictEqual(hash(fs.readFileSync(root + q.runner.name)), q.runner.sha256);
const G = require(root + q.gate_checker), C = G.C;
assert.deepStrictEqual(C.pin(planName, planHash), planBytes);
for (const [name, digest] of Object.entries(q.inputs)) C.pin(name, digest);
C.pin(q.runner.name, q.runner.sha256);
C.bytes(__filename);
assert.deepStrictEqual(C.identities(), C.map);
const full = C.completed(q.full_check.name, q.full_check.command, q.full_check.predecessor);
assert.deepStrictEqual(JSON.parse(cp.execFileSync('node', q.full_check.command.slice(1),
  {cwd: C.repo, maxBuffer: 64 * 1024 * 1024}).toString()), JSON.parse(full.log));
assert.strictEqual(G.gates.gates.length, 25);
for (const spec of G.gates.gates) {
  for (const suffix of ['.json', '.log', '-source.json', '-source-after.json', '-gate-failure.json']) {
    assert.throws(() => fs.lstatSync(root + spec.run + suffix), error => error.code === 'ENOENT',
      'fresh leaf attempt required: ' + spec.run + suffix);
  }
  if (spec.stderr_log)
    assert.throws(() => fs.lstatSync(spec.stderr_log), error => error.code === 'ENOENT', 'fresh stderr file');
}
const completed = [];
for (const spec of G.gates.gates) {
  assert.deepStrictEqual(C.identities(), C.map);
  C.stable();
  console.log(JSON.stringify({starting: spec.run, predecessor: spec.predecessor}));
  const child = cp.spawnSync('node', [root + q.runner.name, spec.run, spec.predecessor, ...spec.command],
    {cwd: C.repo, encoding: 'utf8', maxBuffer: 64 * 1024 * 1024});
  if (child.error) throw child.error;
  assert.strictEqual(child.signal, null, 'runner exited normally: ' + spec.run);
  if (child.status !== 0) {
    process.stderr.write(child.stdout || ''); process.stderr.write(child.stderr || '');
    throw new Error('Leaf runner failed; retain this attempt and inspect it: ' + spec.run);
  }
  const result = C.completed(spec.run, spec.command, spec.predecessor);
  G.leaf(spec, result.log);
  assert.deepStrictEqual(C.identities(), C.map); C.stable();
  completed.push(spec.run);
  console.log(JSON.stringify({completed: spec.run, elapsed_seconds: result.record.elapsed_seconds,
    record_sha256: C.hash(C.bytes(spec.run + '.json'))}));
}
assert.strictEqual(completed.length, 25);
console.log(JSON.stringify({development_only: true, packet_accepted: false,
  source_map_sha256: C.p.source_map_sha256, completed}));
