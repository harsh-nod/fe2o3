// SPDX-License-Identifier: GPL-3.0-or-later
// Exact source-body binding only. Execution of these inert fixtures is separate.
import test from 'node:test';
import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import {SourceSession} from './source-files.mjs';
const session = new SourceSession();
const contract = JSON.parse(session.packageText('tests/fixture-contracts.json'));
const hash = b => crypto.createHash('sha256').update(b).digest('hex');
function part(p) {
  assert(Number.isSafeInteger(p.offset) && p.offset >= 0 && Number.isSafeInteger(p.bytes) && p.bytes > 0);
  const source = Buffer.from(session.packageText(p.source));
  assert(p.offset + p.bytes <= source.length);
  const bytes = source.subarray(p.offset, p.offset + p.bytes);
  assert.equal(hash(bytes), p.sha256);
  return bytes.toString('utf8');
}
test('closed fixtures state exact inert scope and no private stock-context test credit', () => {
  assert.equal(contract.schema, 'fe2o3-disabled-physical-v5-exact-inert-fixtures-v1');
  assert.equal(contract.actual_native_execution, false);
  assert.equal(contract.private_context_controls_included, 0);
  assert.deepEqual(contract.fixtures.map(f => [f.name, f.parts.length, f.groups]),
    [['breakpoint-refresh', 15, 46], ['unload-maintenance', 13, 54]]);
});
for (const name of ['breakpoint-refresh', 'unload-maintenance']) {
  test(name + ' complete body is exact current source ranges, including separators', () => {
    const f = contract.fixtures.find(f => f.name === name);
    const actual = f.prefix + f.parts.map(part).join(f.separator);
    assert.equal(actual, session.packageText(f.body));
    assert.equal(hash(session.packageText(f.mock)), f.mock_sha256);
    assert(session.packageText(f.mock).includes('assert(groups==' + f.groups + ')'));
  });
}
test('breakpoint lexical guard is exact and uniquely embedded in the inert mock', () => {
  const f = contract.fixtures[0], guard = part(f.guard), mock = session.packageText(f.mock);
  const at = mock.indexOf(guard);
  assert(at >= 0);
  assert.equal(mock.indexOf(guard, at + guard.length), -1);
});
test('resume mock donor differs only by one public include relocation', () => {
  const f = contract.fixtures[1], mock = session.packageText(f.mock), change = f.include_change;
  const at = mock.indexOf(change.after);
  assert(at >= 0 && mock.indexOf(change.after, at + change.after.length) === -1);
  const original = mock.slice(0, at) + change.before + mock.slice(at + change.after.length);
  assert.equal(Buffer.byteLength(original), f.donor.bytes);
  assert.equal(hash(original), f.donor.sha256);
});
test('changed offsets lengths or stale digests cannot authenticate a method body', () => {
  const p = contract.fixtures[0].parts[0];
  assert.throws(() => part({...p, offset: p.offset + 1}), Error);
  assert.throws(() => part({...p, bytes: p.bytes - 1}), Error);
  assert.throws(() => part({...p, sha256: '0'.repeat(64)}), Error);
  assert.throws(() => part({...p, offset: Number.MAX_SAFE_INTEGER}), Error);
});
const declared = new Set(session.manifest().payloads.map(r => r.path));
function includeTarget(name) {
  // The refresh donor deliberately uses this one bare include with -I src.
  // Do not turn an arbitrary bare header into an implicit source dependency.
  const target = name === 'amd-dbgapi-owned-one-stop-v1.h' ? 'src/' + name
    : name.startsWith('../src/') ? 'src/' + name.slice(7) : 'tests/' + name;
  assert(declared.has(target), 'declared fixture include: ' + target);
  return target;
}
test('fixture includes reference only declared local payloads or C++ system headers', () => {
  for (const f of contract.fixtures) {
    const text = session.packageText(f.mock);
    for (const match of text.matchAll(/^#include "([^"]+)"$/gm)) includeTarget(match[1]);
  }
});
test('only the selected bare owner header gains the explicit src search path', () => {
  assert.equal(includeTarget('amd-dbgapi-owned-one-stop-v1.h'), 'src/amd-dbgapi-owned-one-stop-v1.h');
  assert.equal(includeTarget('../src/amd-dbgapi-owned-one-stop-v1.h'), 'src/amd-dbgapi-owned-one-stop-v1.h');
  for (const name of ['amd-dbgapi-one-stop-native-v1.h', 'foreign.h', '../outside',
    '/etc/passwd', '../../src/amd-dbgapi-owned-one-stop-v1.h'])
    assert.throws(() => includeTarget(name), Error);
});
