const fs = require('fs');
const assert = require('assert');
const crypto = require('crypto');
const root = '/home/harsh/.codex-tmp/';
const name = process.argv[2];
const prefix = process.argv[3] || '';
assert(['', 'repeat-'].includes(prefix));
const read = file => JSON.parse(fs.readFileSync(root + file, 'utf8'));
const mutation = read('r105-mutations.json').find(m => m.name === name);
assert(mutation, 'known mutation');
const run = read('r105-' + prefix + 'mut-' + name + '.json');
assert.strictEqual(run.returncode, 101);
assert.strictEqual(run.timed_out, false);
assert.strictEqual(run.signal, null);
assert.strictEqual(run.source_unchanged, true);
assert.strictEqual(run.source_head, 'c70773b9c9d9383f79a1c2fe829f1e1163596750');
assert.deepStrictEqual(run.command, ['cargo', '+nightly-2026-04-03', 'test', '--locked',
  '--offline', '-p', 'fe2o3-kfd', '--all-features', '--lib', mutation.test, '--', '--exact']);
const log = fs.readFileSync(run.log, 'utf8');
assert.deepStrictEqual([...log.matchAll(/^test (\S+) \.\.\. FAILED$/gm)].map(m => m[1]), [mutation.test]);
assert.strictEqual([...log.matchAll(/^test result:/gm)].length, 1);
for (const marker of ['Finished `test` profile', 'Running unittests', 'running 1 test',
  'test result: FAILED. 0 passed; 1 failed; 0 ignored;', ...mutation.expected]) {
  assert(log.includes(marker), name + ': ' + marker);
}
const baseline = read('r105-frozen-source.json');
const inputs = read('r105-' + prefix + 'mut-' + name + '-source.json');
const paths = Object.keys(baseline).sort();
assert.deepStrictEqual(Object.keys(inputs).sort(), paths);
assert.deepStrictEqual(paths.filter(p => inputs[p] !== baseline[p]), [mutation.path]);
const current = fs.readFileSync(root + 'fe2o3-r61-execution/' + mutation.path);
assert.strictEqual(crypto.createHash('sha256').update(current).digest('hex'), inputs[mutation.path]);
console.log('PASS: ' + name + ' compiled and failed its exact behavioral oracle');
