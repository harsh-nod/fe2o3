// Emit an apply_patch payload only; the primary owns each edit and its restoration.
const fs = require('fs'), assert = require('assert'), crypto = require('crypto');
const helper = '/home/harsh/.codex-tmp/r124-development-mutation-check-v1.js';
assert.strictEqual(crypto.createHash('sha256').update(fs.readFileSync(helper)).digest('hex'),
  '0d72aad2e3d8bed06cd6e954c0cbe2c7f13c1704d624dd702e059fa7c2a190cc');
const M = require(helper), C = M.C;
C.pin(helper, '0d72aad2e3d8bed06cd6e954c0cbe2c7f13c1704d624dd702e059fa7c2a190cc');
C.bytes(__filename);
const [mode, id] = process.argv.slice(2);
assert(['apply', 'restore'].includes(mode));
const mutation = M.p.mutations.find(item => item.id === id);
assert(mutation, 'declared mutation');
const changedMap = {...C.map, [mutation.patch.path]: mutation.expected_file_sha256};
assert.strictEqual(C.hash(JSON.stringify(changedMap)), mutation.expected_source_map_sha256);
const expectedBefore = mode === 'apply' ? C.map : changedMap;
const expectedAfter = mode === 'apply' ? changedMap : C.map;
assert.deepStrictEqual(C.identities(), expectedBefore);
const file = C.repo + '/' + mutation.patch.path, before = C.bytes(file).toString();
const reverse = {...mutation.patch, edits: mutation.patch.edits.toReversed().map(([a, b]) => [b, a])};
const after = C.E.core.mutatedSource(before, mode === 'apply' ? mutation.patch : reverse);
M.assertMutationSource(mutation, mode === 'apply' ? after : before);
assert.strictEqual(C.hash(after), expectedAfter[mutation.patch.path]);
function scope(text) {
  const start = mutation.patch.scope.start, end = mutation.patch.scope.end;
  assert.strictEqual(text.split(start).length, 2);
  const from = text.indexOf(start), to = text.indexOf(end, from + start.length);
  assert(to > from);
  return {from, to, text: text.slice(from, to)};
}
const oldScope = scope(before), newScope = scope(after);
assert.strictEqual(before.slice(0, oldScope.from), after.slice(0, newScope.from));
assert.strictEqual(before.slice(oldScope.to), after.slice(newScope.to));
const patch = '*** Begin Patch\n*** Update File: ' + file + '\n@@\n'
  + oldScope.text.split('\n').map(line => '-' + line).join('\n') + '\n'
  + newScope.text.split('\n').map(line => '+' + line).join('\n') + '\n*** End Patch';
assert.deepStrictEqual(C.identities(), expectedBefore); C.stable();
console.log(JSON.stringify({emitter_only: true, source_changed: false, mode, id,
  before_map_sha256: C.hash(JSON.stringify(expectedBefore)),
  after_map_sha256: C.hash(JSON.stringify(expectedAfter)), patch}));
