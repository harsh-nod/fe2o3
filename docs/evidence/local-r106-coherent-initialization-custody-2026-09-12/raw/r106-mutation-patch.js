const fs = require('fs');
const path = require('path');
const crypto = require('crypto');
const assert = require('assert');
const root = '/home/harsh/.codex-tmp';
const repo = path.join(root, 'fe2o3-r61-execution');
const [name, direction] = process.argv.slice(2);
assert(['apply', 'restore'].includes(direction));
const mutation = JSON.parse(fs.readFileSync(path.join(root, 'r106-mutations.json')))
  .find(m => m.name === name);
assert(mutation, 'named mutation');
const baseline = JSON.parse(fs.readFileSync(path.join(root, 'r106-frozen-source.json')));
const hash = bytes => crypto.createHash('sha256').update(bytes).digest('hex');
const file = path.join(repo, mutation.path);
const original = fs.readFileSync(file, 'utf8');
let candidate = original;
for (const edit of mutation.edits) {
  const [from, to] = direction === 'apply' ? edit : [...edit].reverse();
  assert.strictEqual(candidate.split(from).length, 2, 'exact unique replacement');
  candidate = candidate.replace(from, to);
}
assert.strictEqual(hash(direction === 'apply' ? original : candidate), baseline[mutation.path]);
const oldLines = original.trimEnd().split('\n');
const newLines = candidate.trimEnd().split('\n');
let prefix = 0;
while (prefix < oldLines.length && oldLines[prefix] === newLines[prefix]) prefix++;
let suffix = 0;
while (suffix < oldLines.length - prefix && suffix < newLines.length - prefix
    && oldLines[oldLines.length - suffix - 1] === newLines[newLines.length - suffix - 1]) suffix++;
const start = Math.max(0, prefix - 3);
const before = oldLines.slice(start, Math.min(oldLines.length, oldLines.length - suffix + 3))
  .map(line => '-' + line).join('\n');
const after = newLines.slice(start, Math.min(newLines.length, newLines.length - suffix + 3))
  .map(line => '+' + line).join('\n');
process.stdout.write(`*** Begin Patch\n*** Update File: ${file}\n@@\n${before}\n${after}\n*** End Patch\n`);
