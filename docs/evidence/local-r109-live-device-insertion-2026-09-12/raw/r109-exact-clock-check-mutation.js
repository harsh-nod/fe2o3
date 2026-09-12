const fs = require('fs');
const assert = require('assert');
const crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const name = process.argv[2];
const prefix = 'exact-clock-';
const read = file => JSON.parse(fs.readFileSync(root + file, 'utf8'));
const mutation = read('r109-exact-mutations.json').find(m => m.name === name);
assert(mutation, 'known mutation');
const run = read('r109-' + prefix + 'mut-' + name + '.json');
assert.strictEqual(run.clock.error, null);
assert.strictEqual(run.returncode, 101);
assert.strictEqual(run.timed_out, false);
assert.strictEqual(run.signal, null);
assert.strictEqual(run.source_unchanged, true);
assert.strictEqual(run.source_head, '17d0be5476a4f71c473a619903c62aa316a96fc7');
assert.deepStrictEqual(run.command, ['cargo', '+nightly-2026-04-03', 'test', '--locked',
  '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib', mutation.test, '--', '--exact']);
const log = fs.readFileSync(run.log, 'utf8');
assert(log.includes(mutation.oracle_path + ':' + mutation.oracle_line + ':'), 'exact behavioral assertion location');
assert.deepStrictEqual([...log.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(m => m[1]), [mutation.test]);
assert.strictEqual([...log.matchAll(/^test result:/gm)].length, 1);
for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test',
  'test result: FAILED. 0 passed; 1 failed; 0 ignored;', ...mutation.expected]) {
  assert(log.includes(marker), name + ': ' + marker);
}
const baseline = read('r109-frozen-source.json');
const inputs = read('r109-' + prefix + 'mut-' + name + '-source.json');
const paths = Object.keys(baseline).sort();
assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
assert.deepStrictEqual(paths.filter(p => inputs[p] !== baseline[p]), [mutation.path]);
const current = fs.readFileSync(root + 'fe2o3-r61-execution/' + mutation.path);
assert.strictEqual(crypto.createHash('sha256').update(current).digest('hex'), inputs[mutation.path]);
console.log('PASS: ' + name + ' compiled and failed its exact behavioral oracle');
