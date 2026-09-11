const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const [name, direction] = process.argv.slice(2);
assert(['apply', 'restore', 'verify'].includes(direction));
const mutation = JSON.parse(fs.readFileSync(path.join(root, 'r95-mutations.json')))
  .find(m => m.name === name);
assert(mutation);
const baseline = JSON.parse(fs.readFileSync(path.join(root, 'r95-formatted-source.json')));
const digest = s => crypto.createHash('sha256').update(s).digest('hex');
let source = fs.readFileSync(path.join(repo, mutation.path), 'utf8');
const original = source;
if (direction === 'apply') assert.strictEqual(digest(source), baseline[mutation.path]);
const edits = direction === 'apply' ? mutation.edits
  : [...mutation.edits].reverse().map(([before, after]) => [after, before]);
for (const [before, after] of edits) {
  assert.strictEqual(source.split(before).length, 2);
  source = source.replace(before, after);
}
if (direction !== 'apply') assert.strictEqual(digest(source), baseline[mutation.path]);
if (direction === 'verify') {
  console.log('PASS: exact intended mutation ' + name);
  process.exit(0);
}
const before = original.split('\n');
const after = source.split('\n');
let first = 0;
while (first < before.length && first < after.length && before[first] === after[first]) first++;
let tail = 0;
while (tail < before.length - first && tail < after.length - first
    && before[before.length - 1 - tail] === after[after.length - 1 - tail]) tail++;
first = Math.max(0, first - 3);
tail = Math.max(0, tail - 3);
const patch = '*** Begin Patch\n*** Update File: ' + path.join(repo, mutation.path) + '\n@@\n'
  + before.slice(first, before.length - tail).map(line => '-' + line).join('\n') + '\n'
  + after.slice(first, after.length - tail).map(line => '+' + line).join('\n') + '\n';
process.stdout.write(patch + '*** End Patch\n');
