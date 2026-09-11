// Check the complete accepted source inventory after restoring each sequence mutation.
const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const cp = require('child_process');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const baseline = JSON.parse(fs.readFileSync(path.join(root, 'r94-clippy-corrected-source.json')));
const names = [...new Set(cp.execFileSync('git', [
  'ls-files', '--cached', '--others', '--exclude-standard', '-z',
], { cwd: repo }).toString().split('\0').filter(n => n && !n.startsWith('docs/')))];
const now = Object.fromEntries(names.sort().map(n => [n,
  crypto.createHash('sha256').update(fs.readFileSync(path.join(repo, n))).digest('hex'),
]));
assert.deepStrictEqual(now, baseline);
for (const mutation of JSON.parse(fs.readFileSync(path.join(root, 'r94-mutations.json')))) {
  const stem = path.join(root, 'r94-mutation-' + mutation.name);
  const record = JSON.parse(fs.readFileSync(stem + '.json'));
  const snapshot = JSON.parse(fs.readFileSync(stem + '-source.json'));
  const log = fs.readFileSync(stem + '.log', 'utf8');
  assert.strictEqual(record.returncode, 101);
  assert.strictEqual(record.source_unchanged, true);
  assert(log.includes('Finished `test`') && log.includes(mutation.assertion));
  assert(log.includes('0 passed; 1 failed; 0 ignored;'));
  assert.deepStrictEqual(Object.keys(snapshot), Object.keys(now));
  assert.deepStrictEqual(names.filter(n => now[n] !== snapshot[n]), [mutation.path]);
  let source = fs.readFileSync(path.join(repo, mutation.path), 'utf8');
  for (const [before, after] of mutation.edits) {
    assert.strictEqual(source.split(before).length, 2);
    source = source.replace(before, after);
  }
  assert.strictEqual(crypto.createHash('sha256').update(source).digest('hex'), snapshot[mutation.path]);
}
console.log('PASS: 5 compiled negative mutations rejected; ' + names.length + ' source identities exactly restored');

