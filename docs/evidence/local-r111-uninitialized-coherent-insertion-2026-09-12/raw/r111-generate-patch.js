// Emit a reversible apply_patch hunk; this helper never writes runtime source.
const fs = require('fs');
const assert = require('assert');
const p = require('./r111-exact-qualification-plan.js');
const [name, direction] = process.argv.slice(2);
assert(['apply', 'restore'].includes(direction));
const mutation = p.mutations.find(m => m.name === name);
assert(mutation);
const filename = '/home/harsh/.codex-tmp/fe2o3-r61-execution/' + mutation.path;
const source = fs.readFileSync(filename, 'utf8');
let result = source;
const edits = direction === 'apply' ? mutation.edits : [...mutation.edits].reverse().map(([from, to]) => [to, from]);
for (const [from, to] of edits) {
  assert.strictEqual(result.split(from).length, 2, 'unique edit anchor');
  result = result.replace(from, to);
}
assert.notStrictEqual(source, result);
const before = source.split('\n');
const after = result.split('\n');
let start = 0;
while (start < before.length && start < after.length && before[start] === after[start]) start++;
let endBefore = before.length;
let endAfter = after.length;
while (endBefore > start && endAfter > start && before[endBefore - 1] === after[endAfter - 1]) {
  endBefore--;
  endAfter--;
}
const lines = [
  ...before.slice(Math.max(0, start - 3), start).map(line => ' ' + line),
  ...before.slice(start, endBefore).map(line => '-' + line),
  ...after.slice(start, endAfter).map(line => '+' + line),
  ...before.slice(endBefore, Math.min(before.length, endBefore + 3)).map(line => ' ' + line),
];
process.stdout.write('*** Begin Patch\n*** Update File: ' + filename + '\n@@\n' + lines.join('\n') + '\n*** End Patch\n');
